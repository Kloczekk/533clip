use crate::obs::{is_obs_clip_file, remove_remux_source_if_mp4_ready, resolve_obs_clip_path};
use crate::pipeline::ClipPipeline;
use notify::{EventKind, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{info, warn};

// OBS replay buffer writes quickly after SaveReplayBuffer; keep delay low.
const DEBOUNCE_MS: u64 = 150;
const REMUX_WAIT_MS: u64 = 500;

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

        let debouncer = spawn_notify_debouncer(watch_root.clone(), event_tx, self.pipeline.app_handle().clone())
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
                        let watch_root = watch_root.clone();
                        let pipeline = pipeline.clone();
                        let in_flight = in_flight.clone();
                        tauri::async_runtime::spawn(async move {
                            process_new_clip(path, watch_root, pipeline, in_flight).await;
                        });
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
    app: tauri::AppHandle,
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
                let kind_label = event_kind_label(&event.kind);
                if !is_new_clip_event(&event.kind) {
                    if matches!(event.kind, EventKind::Remove(_)) {
                        for path in &event.paths {
                            crate::emit_debug(
                                &app,
                                format!("watcher ignored delete: {}", path.display()),
                            );
                        }
                    }
                    continue;
                }
                for path in &event.paths {
                    if !path.starts_with(&root_for_filter) {
                        continue;
                    }
                    if is_clip_candidate(path) {
                        let _ = event_tx.blocking_send(path.clone());
                    } else {
                        crate::emit_debug(
                            &app,
                            format!("watcher {kind_label} ignored (not a clip file): {}", path.display()),
                        );
                    }
                }
            }
        },
    )?;

    debouncer.watch(&root, RecursiveMode::Recursive)?;

    Ok(debouncer)
}

fn event_kind_label(kind: &EventKind) -> &'static str {
    match kind {
        EventKind::Create(_) => "create",
        EventKind::Modify(_) => "modify",
        EventKind::Remove(_) => "remove",
        EventKind::Any => "any",
        EventKind::Access(_) => "access",
        EventKind::Other => "other",
    }
}

fn is_clip_candidate(path: &Path) -> bool {
    is_obs_clip_file(path)
}

fn is_new_clip_event(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Any
    )
}

fn dedupe_clip_key(path: &Path) -> PathBuf {
    path.with_extension("")
}

async fn process_new_clip(
    path: PathBuf,
    watch_root: PathBuf,
    pipeline: ClipPipeline,
    in_flight: Arc<Mutex<HashSet<PathBuf>>>,
) {
    let original_path = path.clone();
    crate::emit_debug(
        pipeline.app_handle(),
        format!("watch event: {}", original_path.display()),
    );
    if !original_path.exists() {
        return;
    }
    {
        let mut set = in_flight.lock().await;
        if !set.insert(dedupe_clip_key(&original_path)) {
            crate::emit_debug(
                pipeline.app_handle(),
                format!("duplicate stem skipped: {}", original_path.display()),
            );
            return;
        }
    }
    let early_game = crate::active_window::last_game_name()
        .or_else(crate::active_window::detect_game_name);
    crate::show_clip_saved_overlay(
        pipeline.app_handle(),
        early_game.clone(),
        original_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "clip".to_string()),
        early_game,
    );

    if has_ext(&original_path, "mkv") {
        crate::emit_debug(
            pipeline.app_handle(),
            format!("mkv detected: {}", original_path.display()),
        );
        process_remuxed_mkv(original_path.clone(), pipeline, in_flight).await;
        return;
    }

    let path = resolve_obs_clip_path(&original_path);

    let stable = super::stability::wait_until_stable(&path).await;
    let final_path = if stable && path.exists() {
        path.clone()
    } else if let Some(found) = ClipPipeline::find_newest_clip_in_dir(&watch_root) {
        // OBS may finalize as a different file (e.g. .mkv → .mp4).
        if !super::stability::wait_until_stable(&found).await {
            in_flight.lock().await.remove(&dedupe_clip_key(&original_path));
            return;
        }
        found
    } else {
        in_flight.lock().await.remove(&dedupe_clip_key(&original_path));
        return;
    };

    remove_remux_source_if_mp4_ready(&final_path);

    match pipeline.on_clip_ready_with_overlay(final_path.clone(), true, false).await {
        Ok(()) => info!(path = %final_path.display(), "clip accepted for processing"),
        Err(e) => warn!(path = %final_path.display(), error = %e, "failed to process clip"),
    }

    in_flight.lock().await.remove(&dedupe_clip_key(&original_path));
}

async fn process_remuxed_mkv(
    mkv: PathBuf,
    pipeline: ClipPipeline,
    in_flight: Arc<Mutex<HashSet<PathBuf>>>,
) {
    let mp4 = mkv.with_extension("mp4");
    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_millis(REMUX_WAIT_MS) && !mp4.is_file() {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    if mp4.is_file() && super::stability::wait_until_stable(&mp4).await {
        remove_remux_source_if_mp4_ready(&mp4);
        crate::emit_debug(
            pipeline.app_handle(),
            format!("remux mp4 ready: {}", mp4.display()),
        );
        match pipeline.on_clip_ready_with_overlay(mp4.clone(), true, false).await {
            Ok(()) => info!(path = %mp4.display(), "remuxed mp4 accepted for processing"),
            Err(e) => warn!(path = %mp4.display(), error = %e, "failed to process remuxed mp4"),
        }
    } else {
        if !super::stability::wait_until_stable(&mkv).await {
            crate::emit_debug(
                pipeline.app_handle(),
                format!("mkv not stable yet: {}", mkv.display()),
            );
            in_flight.lock().await.remove(&dedupe_clip_key(&mkv));
            return;
        }
        crate::emit_debug(
            pipeline.app_handle(),
            format!("mkv ready, importing without remux: {}", mkv.display()),
        );
        match pipeline.on_clip_ready_with_overlay(mkv.clone(), true, false).await {
            Ok(()) => info!(path = %mkv.display(), "mkv accepted for processing"),
            Err(e) => warn!(path = %mkv.display(), error = %e, "failed to process mkv"),
        }
    }

    in_flight.lock().await.remove(&dedupe_clip_key(&mkv));
}

fn has_ext(path: &Path, wanted: &str) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case(wanted))
}
