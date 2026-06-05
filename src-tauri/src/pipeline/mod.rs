use crate::models::clip::{clip_detected_payload, ClipStatus};
use crate::obs::{is_obs_clip_file, resolve_obs_clip_path};
use crate::queue::{Job, JobKind, JobQueue};
use crate::storage::ClipStore;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tracing::info;

const CLIP_DETECTED: &str = "clip://detected";
const CLIP_UPDATED: &str = "clip://updated";

#[derive(Clone)]
pub struct ClipPipeline {
    app: AppHandle,
    store: ClipStore,
    queue: JobQueue,
}

impl ClipPipeline {
    pub fn new(app: AppHandle, store: ClipStore, queue: JobQueue) -> Self {
        Self { app, store, queue }
    }

    pub async fn on_clip_ready(&self, path: PathBuf) -> Result<(), String> {
        if !path.exists() {
            return Err("file does not exist".into());
        }

        let path_str = path.to_string_lossy();
        if let Some(existing) = self.store.by_file_path(&path_str) {
            if existing.status != ClipStatus::Failed {
                info!(id = %existing.id, "clip already tracked, skipping duplicate");
                return Ok(());
            }
        }

        let mut clip =
            crate::models::clip::clip_from_path(&path, ClipStatus::Processing).map_err(|e| e.to_string())?;
        clip = self.store.upsert(clip).map_err(|e| e.to_string())?;

        if let Ok(payload) = clip_detected_payload(&path) {
            let _ = self.app.emit(CLIP_DETECTED, &payload);
        }
        let _ = self.app.emit(CLIP_UPDATED, &clip);

        let clip_id = clip.id.clone();
        self.queue
            .enqueue(Job::new(JobKind::Probe {
                clip_id: clip_id.clone(),
                path: path.clone(),
            }))
            .await
            .map_err(|e| e.to_string())?;
        self.queue
            .enqueue(Job::new(JobKind::Thumbnail {
                clip_id,
                path,
            }))
            .await
            .map_err(|e| e.to_string())?;

        info!(id = %clip.id, file = %clip.file_name, "clip queued for processing");
        Ok(())
    }

    /// If the watched path vanished (OBS rename), pick the newest clip file in the folder.
    pub fn find_newest_clip_in_dir(dir: &Path) -> Option<PathBuf> {
        let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
        let entries = std::fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() || !is_obs_clip_file(&path) {
                continue;
            }
            let path = resolve_obs_clip_path(&path);
            let modified = std::fs::metadata(&path).ok()?.modified().ok()?;
            match &newest {
                Some((t, _)) if modified <= *t => {}
                _ => newest = Some((modified, path)),
            }
        }
        newest.map(|(_, p)| p)
    }

    /// Import clips already on disk (e.g. after restart or first folder pick).
    pub async fn scan_folder(&self, dir: &Path) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };

        let mut files: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() || !is_obs_clip_file(&path) {
                continue;
            }
            let path = resolve_obs_clip_path(&path);
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            if meta.len() == 0 {
                continue;
            }
            let Ok(modified) = meta.modified() else {
                continue;
            };
            files.push((modified, path));
        }

        files.sort_by_key(|(t, _)| *t);
        for (_, path) in files {
            if let Err(e) = self.on_clip_ready(path).await {
                tracing::debug!(error = %e, "scan skipped clip");
            }
        }
    }
}
