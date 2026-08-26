use crate::models::clip::{ClipDetectedPayload, ClipStatus};
use crate::obs::{is_obs_clip_file, remove_remux_source_if_mp4_ready, resolve_obs_clip_path};
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

    pub fn app_handle(&self) -> &AppHandle {
        &self.app
    }

    pub fn queue(&self) -> &JobQueue {
        &self.queue
    }

    pub async fn on_clip_ready(&self, path: PathBuf, notify: bool) -> Result<(), String> {
        self.on_clip_ready_with_overlay(path, notify, true).await
    }

    pub async fn on_clip_ready_with_overlay(
        &self,
        path: PathBuf,
        notify: bool,
        show_overlay: bool,
    ) -> Result<(), String> {
        if !path.exists() {
            return Err("file does not exist".into());
        }

        let path_str = path.to_string_lossy();
        let mut reused_processing = false;
        if let Some(existing) = self.store.by_file_path(&path_str) {
            if existing.status == ClipStatus::Ready {
                tracing::debug!(id = %existing.id, "clip already tracked, skipping duplicate");
                return Ok(());
            }
            if existing.status == ClipStatus::Processing {
                reused_processing = true;
                crate::emit_debug(
                    &self.app,
                    format!("clip was stuck processing, requeueing: {}", existing.file_name),
                );
            }
        }

        let clip = if reused_processing {
            self.store
                .by_file_path(&path_str)
                .ok_or_else(|| "clip not found".to_string())?
        } else {
            let mut next = crate::models::clip::clip_from_path(&path, ClipStatus::Processing)
                .map_err(|e| e.to_string())?;
            if notify {
                next.game_name = crate::active_window::last_game_name()
                    .or_else(crate::active_window::detect_game_name);
                if let Some(game) = &next.game_name {
                    next.display_name = Some(clean_clip_title(game));
                }
            }
            self.store.upsert(next).map_err(|e| e.to_string())?
        };
        crate::emit_debug(
            &self.app,
            format!(
                "clip queued: {}{}",
                clip.file_name,
                clip.game_name
                    .as_ref()
                    .map(|g| format!(" ({g})"))
                    .unwrap_or_default()
            ),
        );

        if notify {
            let payload = ClipDetectedPayload {
                id: clip.id.clone(),
                file_path: clip.file_path.clone(),
                file_name: clip.file_name.clone(),
                created_at: clip.created_at.clone(),
                game_name: clip.game_name.clone(),
            };
            if show_overlay {
                crate::show_clip_saved_overlay(
                    &self.app,
                    clip.game_name.clone(),
                    clip.file_name.clone(),
                    clip.display_name.clone(),
                );
            }
            let _ = self.app.emit(CLIP_DETECTED, &payload);
            let app = self.app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                let _ = app.emit(CLIP_DETECTED, &payload);
            });
        }
        let _ = self.app.emit(CLIP_UPDATED, &clip);

        let clip_id = clip.id.clone();
        let thumb_ok = clip
            .thumbnail_path
            .as_ref()
            .is_some_and(|p| PathBuf::from(p).is_file());
        if reused_processing && clip.duration.is_some() && thumb_ok {
            let updated = self
                .store
                .update(&clip.id, |c| c.status = ClipStatus::Ready)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "clip not found".to_string())?;
            let _ = self.app.emit(CLIP_UPDATED, &updated);
            return Ok(());
        }
        if clip.duration.is_none() {
            self.queue
                .enqueue(Job::new(JobKind::Probe {
                    clip_id: clip_id.clone(),
                    path: path.clone(),
                }))
                .await
                .map_err(|e| e.to_string())?;
        }
        if !thumb_ok {
            self.queue
                .enqueue(Job::new(JobKind::Thumbnail { clip_id, path }))
                .await
                .map_err(|e| e.to_string())?;
        }

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
        let mut files: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
        collect_clip_files(dir, &mut files, 0);

        files.sort_by_key(|(t, _)| *t);
        for (_, path) in files {
            if let Err(e) = self.on_clip_ready(path, false).await {
                tracing::debug!(error = %e, "scan skipped clip");
            }
        }
    }
}

fn clean_clip_title(game: &str) -> String {
    let stamp = chrono::Local::now().format("%d %b %H:%M");
    format!("{game} {stamp}")
}

fn collect_clip_files(dir: &Path, files: &mut Vec<(std::time::SystemTime, PathBuf)>, depth: u8) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_clip_files(&path, files, depth + 1);
            continue;
        }
        if !path.is_file() || !is_obs_clip_file(&path) {
            continue;
        }
        let path = resolve_obs_clip_path(&path);
        remove_remux_source_if_mp4_ready(&path);
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
}
