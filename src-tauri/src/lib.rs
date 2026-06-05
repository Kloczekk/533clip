mod ffmpeg;
mod models;
mod obs;
mod obs_websocket;
mod pipeline;
mod queue;
mod storage;
mod trim_paths;
mod watcher;

use crate::models::clip::Clip;
use crate::queue::{init_job_queue, Job, JobKind};
use crate::obs_websocket::{ObsConnectionStatus, ObsWebSocketManager};
use crate::storage::{
    merge_tags_from_clips, ClipStore, ObsWebSocketSettingsResponse, ObsWebSocketSettingsUpdate,
    SettingsStore, TagRegistryStore,
};
use crate::trim_paths::trimmed_output_path;
use crate::queue::JobQueue;
use crate::watcher::WatcherService;
use std::path::PathBuf;
use tauri::{Emitter, Manager};
use tracing::info;
use tracing_subscriber::EnvFilter;

pub struct AppState {
    pub store: ClipStore,
    pub tags: TagRegistryStore,
    pub settings: SettingsStore,
    pub watcher: WatcherService,
    pub queue: JobQueue,
    pub obs_ws: ObsWebSocketManager,
}

fn normalize_tag(tag: &str) -> Result<String, String> {
    let t = tag.trim().to_lowercase();
    if t.is_empty() {
        return Err("tag cannot be empty".into());
    }
    if t.len() > 32 {
        return Err("tag too long (max 32)".into());
    }
    Ok(t)
}

#[tauri::command]
async fn set_watch_path(state: tauri::State<'_, AppState>, path: String) -> Result<(), String> {
    let pb = PathBuf::from(&path);
    state
        .watcher
        .set_watch_path(pb)
        .await
        .map_err(|e| e.to_string())?;
    state
        .settings
        .set_watch_path(Some(&path))
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn detect_obs_recording_paths() -> Vec<String> {
    obs::detect_recording_paths()
}

#[tauri::command]
fn get_obs_websocket_settings(
    state: tauri::State<'_, AppState>,
) -> Result<ObsWebSocketSettingsResponse, String> {
    state
        .settings
        .obs_websocket_response()
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_obs_websocket_status(state: tauri::State<'_, AppState>) -> ObsConnectionStatus {
    state.obs_ws.status()
}

#[tauri::command]
fn reconnect_obs_websocket(state: tauri::State<'_, AppState>) -> ObsConnectionStatus {
    state.obs_ws.restart();
    state.obs_ws.status()
}

#[tauri::command]
fn import_obs_websocket_settings(
    state: tauri::State<'_, AppState>,
) -> Result<ObsWebSocketSettingsResponse, String> {
    let imported = state
        .settings
        .import_obs_websocket_from_plugin()
        .map_err(|e| e.to_string())?;
    if !imported {
        return Err(
            "Could not read OBS WebSocket config. Is OBS installed with WebSocket enabled?"
                .into(),
        );
    }
    state.obs_ws.restart();
    state
        .settings
        .obs_websocket_response()
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn set_obs_websocket_settings(
    state: tauri::State<'_, AppState>,
    settings: ObsWebSocketSettingsUpdate,
) -> Result<ObsWebSocketSettingsResponse, String> {
    state
        .settings
        .set_obs_websocket(settings)
        .map_err(|e| e.to_string())?;
    state.obs_ws.restart();
    state
        .settings
        .obs_websocket_response()
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_watch_path(state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    Ok(state
        .watcher
        .watch_path()
        .await
        .map(|p| p.to_string_lossy().into_owned()))
}

#[tauri::command]
async fn list_clips(state: tauri::State<'_, AppState>) -> Result<Vec<Clip>, String> {
    Ok(state.store.list())
}

/// Returns a `data:image/jpeg;base64,...` URL the WebView can always display.
#[tauri::command]
fn get_thumbnail_data_url(path: String) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    if bytes.is_empty() {
        return Err("thumbnail file is empty".into());
    }
    Ok(format!(
        "data:image/jpeg;base64,{}",
        STANDARD.encode(bytes)
    ))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TrimQueuedResponse {
    output_path: String,
}

#[tauri::command]
async fn queue_trim_clip(
    state: tauri::State<'_, AppState>,
    clip_id: String,
    start_secs: f64,
    end_secs: f64,
) -> Result<TrimQueuedResponse, String> {
    let clip = state
        .store
        .get(&clip_id)
        .ok_or_else(|| "clip not found".to_string())?;

    let duration = clip.duration.unwrap_or(0.0);
    if duration <= 0.0 {
        return Err("clip duration unknown — wait for processing to finish".into());
    }
    if start_secs < 0.0 || end_secs > duration + 0.05 || end_secs <= start_secs + 0.1 {
        return Err("invalid trim range".into());
    }

    let input = PathBuf::from(&clip.file_path);
    if !input.exists() {
        return Err("source video file is missing".into());
    }

    let output = trimmed_output_path(&input);
    state
        .queue
        .enqueue(Job::new(JobKind::Trim {
            source_clip_id: clip_id,
            input,
            output: output.clone(),
            start_secs,
            end_secs,
        }))
        .await
        .map_err(|e| e.to_string())?;

    Ok(TrimQueuedResponse {
        output_path: output.to_string_lossy().into_owned(),
    })
}

#[tauri::command]
async fn rename_clip(
    state: tauri::State<'_, AppState>,
    id: String,
    display_name: String,
) -> Result<Clip, String> {
    let name = display_name.trim();
    if name.is_empty() {
        return Err("name cannot be empty".into());
    }
    let clip = state
        .store
        .update(&id, |c| c.display_name = Some(name.to_string()))
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "clip not found".to_string())?;
    Ok(clip)
}

fn all_tags(state: &AppState) -> Result<Vec<String>, String> {
    let known = state.tags.list().map_err(|e| e.to_string())?;
    let from_clips: Vec<String> = state
        .store
        .list()
        .into_iter()
        .flat_map(|c| c.tags)
        .collect();
    Ok(merge_tags_from_clips(known, from_clips.into_iter()))
}

#[tauri::command]
async fn list_tags(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    all_tags(&state)
}

#[tauri::command]
async fn create_tag(state: tauri::State<'_, AppState>, tag: String) -> Result<Vec<String>, String> {
    let t = normalize_tag(&tag)?;
    state.tags.ensure_tag(&t).map_err(|e| e.to_string())?;
    all_tags(&state)
}

#[tauri::command]
async fn delete_tag(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    tag: String,
) -> Result<Vec<String>, String> {
    let t = normalize_tag(&tag)?;
    state.tags.remove_tag(&t).map_err(|e| e.to_string())?;
    let updated = state
        .store
        .remove_tag_from_all_clips(&t)
        .map_err(|e| e.to_string())?;
    for clip in updated {
        let _ = app.emit("clip://updated", &clip);
    }
    all_tags(&state)
}

#[tauri::command]
async fn add_clip_tag(state: tauri::State<'_, AppState>, id: String, tag: String) -> Result<Clip, String> {
    let t = normalize_tag(&tag)?;
    state.tags.ensure_tag(&t).map_err(|e| e.to_string())?;
    let clip = state
        .store
        .update(&id, |c| {
            if !c.tags.contains(&t) {
                c.tags.push(t.clone());
                c.tags.sort();
            }
        })
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "clip not found".to_string())?;
    Ok(clip)
}

#[tauri::command]
async fn remove_clip_tag(state: tauri::State<'_, AppState>, id: String, tag: String) -> Result<Clip, String> {
    let t = normalize_tag(&tag)?;
    let clip = state
        .store
        .update(&id, |c| {
            c.tags.retain(|x| x != &t);
        })
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "clip not found".to_string())?;
    Ok(clip)
}

#[tauri::command]
async fn delete_clips(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
) -> Result<(), String> {
    let removed = state
        .store
        .remove_many(&ids)
        .map_err(|e| e.to_string())?;
    for clip in removed {
        let _ = app.emit("clip://deleted", clip.id);
    }
    Ok(())
}

#[tauri::command]
async fn delete_clip(app: tauri::AppHandle, state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    let removed = state
        .store
        .remove(&id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "clip not found".to_string())?;
    let _ = app.emit("clip://deleted", removed.id);
    Ok(())
}

#[tauri::command]
async fn toggle_favorite(state: tauri::State<'_, AppState>, id: String) -> Result<Clip, String> {
    let clip = state
        .store
        .update(&id, |c| c.is_favorite = !c.is_favorite)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "clip not found".to_string())?;
    Ok(clip)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("five33clip_lib=info".parse().unwrap()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| e.to_string())?
                .join("533clip");

            let store = ClipStore::open(&data_dir).map_err(|e| e.to_string())?;
            let tags = TagRegistryStore::open(&data_dir).map_err(|e| e.to_string())?;
            let settings = SettingsStore::open(&data_dir).map_err(|e| e.to_string())?;
            let (queue, pipeline) =
                init_job_queue(handle.clone(), store.clone(), data_dir.clone());
            let watcher = WatcherService::new(pipeline.clone());

            let saved_watch = settings.watch_path().map_err(|e| e.to_string())?;
            if settings
                .import_obs_websocket_from_plugin()
                .map_err(|e| e.to_string())?
            {
                info!("imported OBS WebSocket credentials from local OBS config");
            }
            let obs_ws = ObsWebSocketManager::new(handle.clone(), settings.clone());

            app.manage(AppState {
                store,
                tags,
                settings,
                watcher: watcher.clone(),
                queue,
                obs_ws,
            });

            if let Some(path) = saved_watch {
                let pb = PathBuf::from(&path);
                if pb.is_dir() {
                    if let Err(e) = tauri::async_runtime::block_on(watcher.set_watch_path(pb)) {
                        tracing::warn!(error = %e, "failed to restore saved watch folder");
                    }
                } else {
                    tracing::warn!(path = %path, "saved watch folder no longer exists");
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            set_watch_path,
            get_watch_path,
            detect_obs_recording_paths,
            get_obs_websocket_settings,
            get_obs_websocket_status,
            set_obs_websocket_settings,
            reconnect_obs_websocket,
            import_obs_websocket_settings,
            list_clips,
            toggle_favorite,
            rename_clip,
            delete_clip,
            delete_clips,
            list_tags,
            create_tag,
            delete_tag,
            add_clip_tag,
            remove_clip_tag,
            get_thumbnail_data_url,
            queue_trim_clip
        ])
        .run(tauri::generate_context!())
        .expect("error while running 533clip");
}
