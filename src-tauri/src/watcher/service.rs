use crate::obs::{is_obs_clip_file, resolve_obs_clip_path};
use crate::pipeline::ClipPipeline;
use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{info, warn};

// OBS replay buffer can write in bursts; slightly longer debounce reduces partial reads.
const DEBOUNCE_MS: u64 = 1200;

#[derive(Debug, Error)]
pub enum WatcherError {
    #[error("watch path not set")]
    NotConfigured,
    #[error("watcher error: {0}")]
    Notify(String),
}

pub struct WatcherHandle {
    shutdown: mpsc::Sender<()>,
}

impl WatcherHandle {
    pub async fn stop(self) {
        let _ = self.shutdown.send(()).await;
    }
}

#[derive(Clone)]
pub struct WatcherService {
    watch_path: Arc<RwLock<Option<PathBuf>>>,
    active: Arc<Mutex<Option<WatcherHandle>>>,
    pipeline: ClipPipeline,
    in_flight: Arc<Mutex<HashSet<PathBuf>>>,
}

impl WatcherService {
    pub fn new(pipeline: ClipPipeline) -> Self {
        Self {
            watch_path: Arc::new(RwLock::new(None)),
            active: Arc::new(Mutex::new(None)),
            pipeline,
            in_flight: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub async fn watch_path(&self) -> Option<PathBuf> {
        self.watch_path.read().await.clone()
    }

    pub async fn set_watch_path(&self, path: PathBuf) -> Result<(), WatcherError> {
        if !path.is_dir() {
            return Err(WatcherError::Notify(format!(
                "path is not a directory: {}",
                path.display()
            )));
        }

        self.stop().await;
        *self.watch_path.write().await = Some(path.clone());
        self.in_flight.lock().await.clear();
        self.start_watching(path.clone()).await?;
        self.scan_existing_clips(path).await;
        Ok(())
    }

    async fn scan_existing_clips(&self, dir: PathBuf) {
        let pipeline = self.pipeline.clone();
        tauri::async_runtime::spawn(async move {
            info!(path = %dir.display(), "scanning folder for existing OBS clips");
            pipeline.scan_folder(&dir).await;
        });
    }

    pub async fn stop(&self) {
        let mut guard = self.active.lock().await;
        if let Some(handle) = guard.take() {
            handle.stop().await;
        }
    }

    async fn start_watching(&self, path: PathBuf) -> Result<(), WatcherError> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        let (event_tx, mut event_rx) = mpsc::channel::<PathBuf>(64);

        let pipeline = self.pipeline.clone();
        let in_flight = self.in_flight.clone();
        let watch_root = path.clone();

        let debouncer = spawn_notify_debouncer(watch_root.clone(), event_tx)
            .map_err(|e| WatcherError::Notify(e.to_string()))?;

        tauri::async_runtime::spawn(async move {
            let _debouncer = debouncer;
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        info!("file watcher shutting down");
                        break;
                    }
                    Some(path) = event_rx.recv() => {
                        process_new_clip(path, watch_root.clone(), pipeline.clone(), in_flight.clone()).await;
                    }
                }
            }
        });

        *self.active.lock().await = Some(WatcherHandle {
            shutdown: shutdown_tx,
        });

        info!(path = %path.display(), "watching OBS clip folder");
        Ok(())
    }
}

fn spawn_notify_debouncer(
    root: PathBuf,
    event_tx: mpsc::Sender<PathBuf>,
) -> Result<Debouncer<notify::RecommendedWatcher, FileIdMap>, notify::Error> {
    let root_for_filter = root.clone();
    let mut debouncer = new_debouncer(
        Duration::from_millis(DEBOUNCE_MS),
        None,
        move |result: DebounceEventResult| {
            let Ok(events) = result else {
                return;
            };
            for event in events {
                for path in &event.paths {
                    if is_clip_candidate(path) && path.starts_with(&root_for_filter) {
                        let _ = event_tx.blocking_send(path.clone());
                    }
                }
            }
        },
    )?;

    debouncer.watch(&root, RecursiveMode::NonRecursive)?;

    Ok(debouncer)
}

fn is_clip_candidate(path: &Path) -> bool {
    is_obs_clip_file(path)
}

async fn process_new_clip(
    path: PathBuf,
    watch_root: PathBuf,
    pipeline: ClipPipeline,
    in_flight: Arc<Mutex<HashSet<PathBuf>>>,
) {
    {
        let mut set = in_flight.lock().await;
        if !set.insert(path.clone()) {
            return;
        }
    }

    let path = resolve_obs_clip_path(&path);

    let stable = super::stability::wait_until_stable(&path).await;
    let final_path = if stable && path.exists() {
        path.clone()
    } else if let Some(found) = ClipPipeline::find_newest_clip_in_dir(&watch_root) {
        // OBS may finalize as a different file (e.g. .mkv → .mp4).
        if !super::stability::wait_until_stable(&found).await {
            in_flight.lock().await.remove(&path);
            return;
        }
        found
    } else {
        in_flight.lock().await.remove(&path);
        return;
    };

    match pipeline.on_clip_ready(final_path.clone()).await {
        Ok(()) => info!(path = %final_path.display(), "clip accepted for processing"),
        Err(e) => warn!(path = %final_path.display(), error = %e, "failed to process clip"),
    }

    in_flight.lock().await.remove(&path);
}
