use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
};
use tracing::{debug, warn};

use crate::storage::ObsSettings;

/// Video extensions OBS commonly writes (replay buffer, recording, remux).
pub const CLIP_EXTENSIONS: &[&str] = &["mp4", "mkv", "mov"];

/// OBS WebSocket is a localhost loopback connection — a healthy one responds
/// in milliseconds. These used to be 3-5s each (up to ~11s worst case across
/// connect+hello+response), which made routine status polling (settings tab,
/// the recording monitor) feel like it hung whenever the connection wasn't
/// actually working, instead of failing fast.
const WS_HANDSHAKE_TIMEOUT: Duration = Duration::from_millis(1200);
const WS_RESPONSE_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObsRuntimeStatus {
    pub installed: bool,
    pub running: bool,
    pub websocket_connected: bool,
    pub replay_buffer_active: bool,
    pub path: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObsAudioInput {
    pub name: String,
    pub kind: String,
    pub muted: bool,
    pub volume_mul: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObsStats {
    pub cpu_usage: f64,
    pub memory_usage_mb: f64,
    pub active_fps: f64,
    pub render_skipped_frames: u64,
    pub render_total_frames: u64,
    pub output_skipped_frames: u64,
    pub output_total_frames: u64,
}

pub fn is_obs_clip_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| {
            CLIP_EXTENSIONS
                .iter()
                .any(|allowed| ext.eq_ignore_ascii_case(allowed))
        })
        .unwrap_or(false)
}

/// If OBS remuxed `.mkv` → `.mp4`, prefer the finished `.mp4` when both exist.
pub fn resolve_obs_clip_path(path: &Path) -> PathBuf {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());

    if ext.as_deref() == Some("mkv") {
        let mp4 = path.with_extension("mp4");
        if mp4.is_file() {
            return mp4;
        }
    }
    path.to_path_buf()
}

pub fn remove_remux_source_if_mp4_ready(path: &Path) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());

    let (mp4, mkv) = match ext.as_deref() {
        Some("mp4") => (path.to_path_buf(), path.with_extension("mkv")),
        Some("mkv") => (path.with_extension("mp4"), path.to_path_buf()),
        _ => return,
    };

    if !mp4.is_file() || !mkv.is_file() {
        return;
    }

    match std::fs::remove_file(&mkv) {
        Ok(()) => debug!(path = %mkv.display(), "removed OBS remux source mkv"),
        Err(e) => warn!(path = %mkv.display(), error = %e, "failed to remove OBS remux source mkv"),
    }
}

/// Read `FilePath` / `RecFilePath` from OBS Studio profile `basic.ini` files.
pub fn detect_recording_paths() -> Vec<String> {
    let mut paths = BTreeSet::new();

    let base = std::env::var("APPDATA")
        .ok()
        .map(PathBuf::from)
        .map(|p| p.join("obs-studio").join("basic").join("profiles"));

    let Some(profiles_root) = base else {
        return Vec::new();
    };

    if !profiles_root.is_dir() {
        return Vec::new();
    }

    let Ok(entries) = std::fs::read_dir(&profiles_root) else {
        return Vec::new();
    };

    for profile in entries.flatten() {
        let ini = profile.path().join("basic.ini");
        if !ini.is_file() {
            continue;
        }
        if let Ok(raw) = std::fs::read_to_string(&ini) {
            for line in raw.lines() {
                let line = line.trim();
                if let Some(value) = line
                    .strip_prefix("FilePath=")
                    .or_else(|| line.strip_prefix("RecFilePath="))
                    .or_else(|| line.strip_prefix("FFFilePath="))
                {
                    let normalized = normalize_obs_path(value);
                    if normalized.is_dir() {
                        paths.insert(normalized.to_string_lossy().into_owned());
                    }
                }
            }
        }
    }

    // Common Windows default (OBS installer often uses this).
    if let Ok(home) = std::env::var("USERPROFILE") {
        let videos = PathBuf::from(home).join("Videos");
        if videos.is_dir() {
            paths.insert(videos.to_string_lossy().into_owned());
        }
    }

    paths.into_iter().collect()
}

fn normalize_obs_path(raw: &str) -> PathBuf {
    let trimmed = raw.trim().trim_matches('"');
    let unescaped = trimmed.replace("\\\\", "\\");
    PathBuf::from(unescaped)
}

pub fn detect_obs_executable() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    for key in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
        if let Ok(base) = std::env::var(key) {
            candidates.push(
                PathBuf::from(base)
                    .join("obs-studio")
                    .join("bin")
                    .join("64bit")
                    .join("obs64.exe"),
            );
        }
    }
    candidates.into_iter().find(|path| path.is_file())
}

pub fn is_obs_running() -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
        use windows::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        };

        unsafe {
            let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
                return false;
            };
            if snapshot == INVALID_HANDLE_VALUE {
                return false;
            }
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            let mut found = false;
            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    let end = entry
                        .szExeFile
                        .iter()
                        .position(|c| *c == 0)
                        .unwrap_or(entry.szExeFile.len());
                    let exe = String::from_utf16_lossy(&entry.szExeFile[..end]).to_ascii_lowercase();
                    if exe == "obs64.exe" || exe == "obs32.exe" {
                        found = true;
                        break;
                    }
                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snapshot);
            found
        }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

pub fn launch_minimized_to_tray(start_replay_buffer: bool) -> Result<(), String> {
    ensure_websocket_enabled_config()?;
    let _ = disable_auto_remux_profiles();
    let _ = remove_legacy_533clip_scripts();
    let exe = detect_obs_executable().ok_or_else(|| "OBS install not found".to_string())?;
    let mut command = std::process::Command::new(&exe);
    command.arg("--disable-shutdown-check");
    command.arg("--minimize-to-tray");
    if start_replay_buffer {
        command.arg("--startreplaybuffer");
    }
    if let Some(parent) = exe.parent() {
        command.current_dir(parent);
    }
    command.spawn().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn remove_legacy_533clip_scripts() -> Result<Vec<String>, String> {
    let scenes_root = std::env::var("APPDATA")
        .ok()
        .map(PathBuf::from)
        .ok_or_else(|| "APPDATA not found".to_string())?
        .join("obs-studio")
        .join("basic")
        .join("scenes");
    if !scenes_root.is_dir() {
        return Ok(Vec::new());
    }
    let mut updated = Vec::new();
    for entry in std::fs::read_dir(scenes_root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let mut json: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let Some(scripts) = json
            .get_mut("modules")
            .and_then(|m| m.get_mut("scripts-tool"))
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        let before = scripts.len();
        scripts.retain(|script| {
            let script_path = script
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_ascii_lowercase();
            !(script_path.contains("silent_notifier.py") || script_path.contains("mkv") && script_path.contains("remov"))
        });
        if scripts.len() != before {
            std::fs::write(&path, serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            updated.push(path.to_string_lossy().into_owned());
        }
    }
    Ok(updated)
}

pub fn stop_processes() {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        for exe in ["obs64.exe", "obs32.exe"] {
            let _ = std::process::Command::new("taskkill")
                .args(["/IM", exe, "/T", "/F"])
                .creation_flags(CREATE_NO_WINDOW)
                .output();
        }
    }
}

pub async fn runtime_status(settings: &ObsSettings) -> ObsRuntimeStatus {
    let path = detect_obs_executable();
    let running = is_obs_running();
    let websocket_was_disabled = ensure_websocket_enabled_config().unwrap_or(false);
    let mut status = ObsRuntimeStatus {
        installed: path.is_some(),
        running,
        websocket_connected: false,
        replay_buffer_active: false,
        path: path.map(|p| p.to_string_lossy().into_owned()),
        error: websocket_was_disabled
            .then(|| "OBS WebSocket was off. 533clip enabled it; restart OBS.".to_string()),
    };

    if running {
        match request(settings, "GetReplayBufferStatus", json!({})).await {
            Ok(v) => {
                status.websocket_connected = true;
                status.replay_buffer_active = v
                    .get("outputActive")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
            }
            Err(e) => {
                status.error = Some(connection_error_message(&e));
            }
        }
    }

    status
}

pub async fn stats(settings: &ObsSettings) -> Result<ObsStats, String> {
    let v = request(settings, "GetStats", json!({})).await?;
    Ok(ObsStats {
        cpu_usage: v.get("cpuUsage").and_then(Value::as_f64).unwrap_or(0.0),
        memory_usage_mb: v.get("memoryUsage").and_then(Value::as_f64).unwrap_or(0.0),
        active_fps: v.get("activeFps").and_then(Value::as_f64).unwrap_or(0.0),
        render_skipped_frames: v.get("renderSkippedFrames").and_then(Value::as_u64).unwrap_or(0),
        render_total_frames: v.get("renderTotalFrames").and_then(Value::as_u64).unwrap_or(0),
        output_skipped_frames: v.get("outputSkippedFrames").and_then(Value::as_u64).unwrap_or(0),
        output_total_frames: v.get("outputTotalFrames").and_then(Value::as_u64).unwrap_or(0),
    })
}

pub fn set_replay_save_hotkey(hotkey: &str) -> Result<Vec<String>, String> {
    let entry = obs_hotkey_entry(hotkey)?;
    let value = serde_json::to_string(&json!({
        "ReplayBuffer.Save": [entry],
    }))
    .map_err(|e| e.to_string())?;

    let profiles_root = obs_profiles_root().ok_or_else(|| "APPDATA not found".to_string())?;

    if !profiles_root.is_dir() {
        return Err("OBS profiles folder not found".into());
    }

    let mut updated = Vec::new();
    for entry in std::fs::read_dir(&profiles_root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let ini = entry.path().join("basic.ini");
        if !ini.is_file() {
            continue;
        }
        let raw = std::fs::read_to_string(&ini).map_err(|e| e.to_string())?;
        let next = upsert_ini_key(&raw, "Hotkeys", "ReplayBuffer", &value);
        if next != raw {
            std::fs::write(&ini, next).map_err(|e| e.to_string())?;
            updated.push(ini.to_string_lossy().into_owned());
        }
    }

    if updated.is_empty() {
        Err("No OBS profile basic.ini updated".into())
    } else {
        Ok(updated)
    }
}

pub fn set_recording_toggle_hotkey(hotkey: &str) -> Result<Vec<String>, String> {
    let entry = obs_hotkey_entry(hotkey)?;
    let start_value = serde_json::to_string(&json!([entry.clone()])).map_err(|e| e.to_string())?;
    let stop_value = serde_json::to_string(&json!([entry])).map_err(|e| e.to_string())?;

    let profiles_root = obs_profiles_root().ok_or_else(|| "APPDATA not found".to_string())?;

    if !profiles_root.is_dir() {
        return Err("OBS profiles folder not found".into());
    }

    let mut updated = Vec::new();
    for entry in std::fs::read_dir(&profiles_root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let ini = entry.path().join("basic.ini");
        if !ini.is_file() {
            continue;
        }
        let raw = std::fs::read_to_string(&ini).map_err(|e| e.to_string())?;
        let next = upsert_ini_key(&raw, "Hotkeys", "OBSBasic.StartRecording", &start_value);
        let next = upsert_ini_key(&next, "Hotkeys", "OBSBasic.StopRecording", &stop_value);
        if next != raw {
            std::fs::write(&ini, next).map_err(|e| e.to_string())?;
            updated.push(ini.to_string_lossy().into_owned());
        }
    }

    if updated.is_empty() {
        Err("No OBS profile basic.ini updated".into())
    } else {
        Ok(updated)
    }
}

pub fn set_replay_duration(seconds: u32) -> Result<Vec<String>, String> {
    let profiles_root = obs_profiles_root().ok_or_else(|| "APPDATA not found".to_string())?;
    if !profiles_root.is_dir() {
        return Err("OBS profiles folder not found".into());
    }
    let seconds = seconds.clamp(5, 600).to_string();
    let mut updated = Vec::new();
    for entry in std::fs::read_dir(&profiles_root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let ini = entry.path().join("basic.ini");
        if !ini.is_file() {
            continue;
        }
        let raw = std::fs::read_to_string(&ini).map_err(|e| e.to_string())?;
        let next = upsert_ini_key(&raw, "SimpleOutput", "RecRBTime", &seconds);
        let next = upsert_ini_key(&next, "AdvOut", "RecRBTime", &seconds);
        if next != raw {
            std::fs::write(&ini, next).map_err(|e| e.to_string())?;
            updated.push(ini.to_string_lossy().into_owned());
        }
    }
    if updated.is_empty() {
        Err("No OBS profile basic.ini updated".into())
    } else {
        Ok(updated)
    }
}

pub fn set_capture_source_mode(mode: &str) -> Result<Vec<String>, String> {
    let mode = mode.trim().to_ascii_lowercase();
    if mode != "display" && mode != "game" {
        return Err("capture mode must be display or game".into());
    }
    let scenes_root = std::env::var("APPDATA")
        .ok()
        .map(PathBuf::from)
        .ok_or_else(|| "APPDATA not found".to_string())?
        .join("obs-studio")
        .join("basic")
        .join("scenes");
    if !scenes_root.is_dir() {
        return Err("OBS scenes folder not found".into());
    }

    let mut updated = Vec::new();
    for entry in std::fs::read_dir(scenes_root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let mut root: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let sources = root
            .get_mut("sources")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| "OBS scene collection missing sources".to_string())?;

        let capture_index = sources.iter().position(|source| {
            source
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| id == "monitor_capture" || id == "game_capture")
        });
        let source_uuid = capture_index
            .and_then(|idx| sources[idx].get("uuid").and_then(Value::as_str).map(str::to_string))
            .unwrap_or_else(new_obs_uuid);
        let source_name = if mode == "game" { "Game Capture" } else { "Display Capture" };
        let source = if mode == "game" {
            json!({
                "balance": 0.5,
                "deinterlace_field_order": 0,
                "deinterlace_mode": 0,
                "enabled": true,
                "flags": 0,
                "hotkeys": {},
                "id": "game_capture",
                "mixers": 0,
                "monitoring_type": 0,
                "muted": false,
                "name": source_name,
                "private_settings": {},
                "push-to-mute": false,
                "push-to-mute-delay": 0,
                "push-to-talk": false,
                "push-to-talk-delay": 0,
                "settings": {
                    "capture_mode": "any_fullscreen",
                    "capture_cursor": true,
                    "allow_transparency": false,
                    "limit_framerate": false,
                    "capture_overlays": true,
                    "anti_cheat_hook": true
                },
                "sync": 0,
                "uuid": source_uuid,
                "versioned_id": "game_capture",
                "volume": 1.0
            })
        } else {
            let monitor_id = capture_index
                .and_then(|idx| sources[idx].get("settings"))
                .and_then(|settings| settings.get("monitor_id"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            json!({
                "balance": 0.5,
                "deinterlace_field_order": 0,
                "deinterlace_mode": 0,
                "enabled": true,
                "flags": 0,
                "hotkeys": {},
                "id": "monitor_capture",
                "mixers": 0,
                "monitoring_type": 0,
                "muted": false,
                "name": source_name,
                "private_settings": {},
                "push-to-mute": false,
                "push-to-mute-delay": 0,
                "push-to-talk": false,
                "push-to-talk-delay": 0,
                "settings": { "monitor_id": monitor_id },
                "sync": 0,
                "uuid": source_uuid,
                "versioned_id": "monitor_capture",
                "volume": 1.0
            })
        };

        if let Some(idx) = capture_index {
            sources[idx] = source;
        } else {
            sources.push(source);
        }

        for scene in sources.iter_mut().filter(|source| {
            source.get("id").and_then(Value::as_str) == Some("scene")
        }) {
            if let Some(items) = scene
                .get_mut("settings")
                .and_then(|settings| settings.get_mut("items"))
                .and_then(Value::as_array_mut)
            {
                if let Some(item) = items.iter_mut().find(|item| {
                    item.get("name")
                        .and_then(Value::as_str)
                        .is_some_and(|name| name == "Display Capture" || name == "Game Capture")
                }) {
                    item["name"] = Value::String(source_name.to_string());
                    item["source_uuid"] = Value::String(source_uuid.clone());
                    item["pos"] = json!({ "x": 0.0, "y": 0.0 });
                    item["scale"] = json!({ "x": 1.0, "y": 1.0 });
                    item["bounds"] = json!({ "x": 1920.0, "y": 1080.0 });
                    item["bounds_type"] = Value::String("OBS_BOUNDS_SCALE_INNER".into());
                    item["scale_filter"] = Value::String("area".into());
                    item["visible"] = Value::Bool(true);
                }
            }
        }

        std::fs::write(&path, serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        updated.push(path.to_string_lossy().into_owned());
    }

    if updated.is_empty() {
        Err("No OBS scene collection updated".into())
    } else {
        Ok(updated)
    }
}

pub fn disable_auto_remux_profiles() -> Result<Vec<String>, String> {
    let profiles_root = obs_profiles_root().ok_or_else(|| "OBS profiles folder not found".to_string())?;
    if !profiles_root.is_dir() {
        return Err("OBS profiles folder not found".into());
    }

    let mut updated = Vec::new();
    for entry in std::fs::read_dir(&profiles_root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let ini = entry.path().join("basic.ini");
        if !ini.is_file() {
            continue;
        }
        let raw = std::fs::read_to_string(&ini).map_err(|e| e.to_string())?;
        let next = upsert_ini_key(&raw, "Video", "AutoRemux", "false");
        let next = upsert_ini_key(&next, "SimpleOutput", "RecFormat2", "mkv");
        let next = upsert_ini_key(&next, "AdvOut", "RecFormat2", "mkv");
        if next != raw {
            std::fs::write(&ini, next).map_err(|e| e.to_string())?;
            updated.push(ini.to_string_lossy().into_owned());
        }
    }
    Ok(updated)
}

pub fn apply_quality_preset(preset: &str) -> Result<Vec<String>, String> {
    let profiles_root = obs_profiles_root().ok_or_else(|| "OBS profiles folder not found".to_string())?;
    if !profiles_root.is_dir() {
        return Err("OBS profiles folder not found".into());
    }

    let preset = preset.trim().to_ascii_lowercase();
    let (
        v_bitrate,
        a_bitrate,
        nven_c_preset,
        rec_quality,
        rb_size,
        rb_time,
        out_w,
        out_h,
        base_w,
        base_h,
        fps,
        scale,
    ) = match preset.as_str() {
        "high" => ("8000", "192", "p3", "Stream", "3072", "60", "1536", "864", "1920", "1080", "60", "bicubic"),
        "medium" => ("3500", "160", "p2", "Stream", "1024", "45", "1280", "720", "1920", "1080", "30", "bilinear"),
        "low" => ("1400", "128", "p1", "Small", "512", "30", "960", "540", "1920", "1080", "30", "bilinear"),
        "potato" => ("350", "64", "p1", "Small", "256", "20", "426", "240", "1920", "1080", "15", "bilinear"),
        "533" => ("120", "32", "p1", "Small", "64", "12", "160", "90", "1920", "1080", "8", "bilinear"),
        _ => return Err("Unknown OBS preset. Use high, medium, low, potato, or 533.".into()),
    };

    let mut updated = Vec::new();
    for entry in std::fs::read_dir(&profiles_root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let ini = entry.path().join("basic.ini");
        if !ini.is_file() {
            continue;
        }
        let raw = std::fs::read_to_string(&ini).map_err(|e| e.to_string())?;
        let next = upsert_ini_key(&raw, "SimpleOutput", "VBitrate", v_bitrate);
        let next = upsert_ini_key(&next, "SimpleOutput", "ABitrate", a_bitrate);
        let next = upsert_ini_key(&next, "SimpleOutput", "NVENCPreset2", nven_c_preset);
        let next = upsert_ini_key(&next, "SimpleOutput", "NVENCTuning", if preset == "high" { "hq" } else { "ll" });
        let next = upsert_ini_key(&next, "SimpleOutput", "NVENCMultipass", if preset == "high" { "qres" } else { "disabled" });
        let next = upsert_ini_key(&next, "SimpleOutput", "NVENCLookAhead", "false");
        let next = upsert_ini_key(&next, "SimpleOutput", "NVENCPsychoVisualTuning", if preset == "high" { "true" } else { "false" });
        let next = upsert_ini_key(&next, "SimpleOutput", "RecQuality", rec_quality);
        let next = upsert_ini_key(&next, "SimpleOutput", "RecRBSize", rb_size);
        let next = upsert_ini_key(&next, "SimpleOutput", "RecRBTime", rb_time);
        let next = upsert_ini_key(&next, "SimpleOutput", "StreamEncoder", "nvenc");
        let next = upsert_ini_key(&next, "SimpleOutput", "RecEncoder", "nvenc");
        let next = upsert_ini_key(&next, "SimpleOutput", "RecFormat2", "mkv");
        let next = upsert_ini_key(&next, "SimpleOutput", "x264Preset", if preset == "533" { "ultrafast" } else { "veryfast" });
        let next = upsert_ini_key(&next, "SimpleOutput", "StreamEncoder", if preset == "533" { "x264" } else { "nvenc" });
        let next = upsert_ini_key(&next, "SimpleOutput", "RecEncoder", if preset == "533" { "x264" } else { "nvenc" });
        let next = upsert_ini_key(&next, "AdvOut", "FFVBitrate", v_bitrate);
        let next = upsert_ini_key(&next, "AdvOut", "FFABitrate", a_bitrate);
        let next = upsert_ini_key(&next, "AdvOut", "Track1Bitrate", a_bitrate);
        let next = upsert_ini_key(&next, "AdvOut", "RecBitrate", v_bitrate);
        let next = upsert_ini_key(&next, "AdvOut", "Encoder", if preset == "533" { "obs_x264" } else { "ffmpeg_nvenc" });
        let next = upsert_ini_key(&next, "AdvOut", "RecEncoder", if preset == "533" { "obs_x264" } else { "ffmpeg_nvenc" });
        let next = upsert_ini_key(&next, "AdvOut", "NVENCPreset2", nven_c_preset);
        let next = upsert_ini_key(&next, "AdvOut", "NVENCTuning", if preset == "high" { "hq" } else { "ll" });
        let next = upsert_ini_key(&next, "AdvOut", "NVENCMultipass", if preset == "high" { "qres" } else { "disabled" });
        let next = upsert_ini_key(&next, "AdvOut", "NVENCLookAhead", "false");
        let next = upsert_ini_key(&next, "AdvOut", "NVENCPsychoVisualTuning", if preset == "high" { "true" } else { "false" });
        let next = upsert_ini_key(&next, "AdvOut", "x264Preset", if preset == "533" { "ultrafast" } else { "veryfast" });
        let next = upsert_ini_key(&next, "AdvOut", "RecProfile", if preset == "533" { "baseline" } else { "high" });
        let next = upsert_ini_key(&next, "AdvOut", "KeyframeSec", if preset == "533" { "8" } else { "2" });
        let next = upsert_ini_key(
            &next,
            "AdvOut",
            "x264Settings",
            if preset == "533" {
                "keyint=64:min-keyint=64:scenecut=0:aq-mode=0:deblock=-6,-6:no-cabac=1:ref=1:subme=0:me=dia"
            } else {
                ""
            },
        );
        let next = upsert_ini_key(&next, "AdvOut", "RecRBSize", rb_size);
        let next = upsert_ini_key(&next, "AdvOut", "RecRBTime", rb_time);
        let next = upsert_ini_key(&next, "AdvOut", "RecFormat2", "mkv");
        let next = upsert_ini_key(&next, "Video", "BaseCX", base_w);
        let next = upsert_ini_key(&next, "Video", "BaseCY", base_h);
        let next = upsert_ini_key(&next, "Video", "OutputCX", out_w);
        let next = upsert_ini_key(&next, "Video", "OutputCY", out_h);
        let next = upsert_ini_key(&next, "Video", "FPSCommon", fps);
        let next = upsert_ini_key(&next, "Video", "FPSInt", fps);
        let next = upsert_ini_key(&next, "Video", "FPSNum", fps);
        let next = upsert_ini_key(&next, "Video", "FPSDen", "1");
        let next = upsert_ini_key(&next, "Video", "ScaleType", scale);
        let next = upsert_ini_key(&next, "Video", "ColorFormat", if preset == "533" { "I420" } else { "NV12" });
        let next = upsert_ini_key(&next, "Video", "ColorSpace", if preset == "533" { "601" } else { "709" });
        let next = upsert_ini_key(&next, "Video", "ColorRange", if preset == "533" { "Full" } else { "Partial" });
        let next = upsert_ini_key(&next, "Video", "AutoRemux", "false");
        if next != raw {
            std::fs::write(&ini, next).map_err(|e| e.to_string())?;
            updated.push(ini.to_string_lossy().into_owned());
        }
    }

    if updated.is_empty() {
        Err("No OBS profile basic.ini updated".into())
    } else {
        Ok(updated)
    }
}

pub async fn start_replay_buffer(settings: &ObsSettings) -> Result<(), String> {
    request(settings, "StartReplayBuffer", json!({})).await?;
    Ok(())
}

pub async fn stop_replay_buffer(settings: &ObsSettings) -> Result<(), String> {
    request(settings, "StopReplayBuffer", json!({})).await?;
    Ok(())
}

pub async fn recording_active(settings: &ObsSettings) -> Result<bool, String> {
    let response = request(settings, "GetRecordStatus", json!({})).await?;
    Ok(response
        .get("outputActive")
        .and_then(Value::as_bool)
        .unwrap_or(false))
}

pub async fn toggle_recording(settings: &ObsSettings) -> Result<(), String> {
    if recording_active(settings).await.unwrap_or(false) {
        request(settings, "StopRecord", json!({})).await?;
    } else {
        request(settings, "StartRecord", json!({})).await?;
    }
    Ok(())
}

pub async fn exit_obs(settings: &ObsSettings) -> Result<(), String> {
    request(settings, "Exit", json!({})).await?;
    Ok(())
}

pub async fn save_replay_buffer(settings: &ObsSettings) -> Result<(), String> {
    if let Ok(status) = request(settings, "GetReplayBufferStatus", json!({})).await {
        if !status
            .get("outputActive")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            let _ = request(settings, "StartReplayBuffer", json!({})).await;
            tokio::time::sleep(Duration::from_millis(900)).await;
        }
    }
    request(settings, "SaveReplayBuffer", json!({})).await?;
    Ok(())
}

pub async fn audio_inputs(settings: &ObsSettings) -> Result<Vec<ObsAudioInput>, String> {
    let response = request(settings, "GetInputList", json!({})).await?;
    let inputs = response
        .get("inputs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    for input in inputs {
        let name = input
            .get("inputName")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let kind = input
            .get("inputKind")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let lower = format!("{} {}", name.to_lowercase(), kind.to_lowercase());
        if name.is_empty()
            || !(lower.contains("audio")
                || lower.contains("mic")
                || lower.contains("desktop")
                || lower.contains("wasapi"))
        {
            continue;
        }
        let muted = request(settings, "GetInputMute", json!({ "inputName": name }))
            .await
            .ok()
            .and_then(|v| v.get("inputMuted").and_then(Value::as_bool))
            .unwrap_or(false);
        let volume_mul = request(settings, "GetInputVolume", json!({ "inputName": name }))
            .await
            .ok()
            .and_then(|v| v.get("inputVolumeMul").and_then(Value::as_f64))
            .unwrap_or(1.0);
        out.push(ObsAudioInput {
            name,
            kind,
            muted,
            volume_mul,
        });
    }
    Ok(out)
}

pub async fn set_input_mute(settings: &ObsSettings, input_name: &str, muted: bool) -> Result<(), String> {
    request(
        settings,
        "SetInputMute",
        json!({ "inputName": input_name, "inputMuted": muted }),
    )
    .await?;
    Ok(())
}

pub async fn set_input_volume(settings: &ObsSettings, input_name: &str, volume_mul: f64) -> Result<(), String> {
    request(
        settings,
        "SetInputVolume",
        json!({ "inputName": input_name, "inputVolumeMul": volume_mul.clamp(0.0, 4.0) }),
    )
    .await?;
    Ok(())
}

async fn request(settings: &ObsSettings, request_type: &str, request_data: Value) -> Result<Value, String> {
    let mut req = resolve_websocket_url(settings)
        .into_client_request()
        .map_err(|e| e.to_string())?;
    req.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        HeaderValue::from_static("obswebsocket.json"),
    );

    let (ws, _) = tokio::time::timeout(WS_HANDSHAKE_TIMEOUT, connect_async(req))
        .await
        .map_err(|_| "OBS WebSocket connect timed out".to_string())?
        .map_err(|e| e.to_string())?;
    let (mut write, mut read) = ws.split();

    while let Some(msg) = tokio::time::timeout(WS_HANDSHAKE_TIMEOUT, read.next())
        .await
        .map_err(|_| "OBS WebSocket hello timed out".to_string())?
    {
        let msg = msg.map_err(|e| e.to_string())?;
        if !msg.is_text() {
            continue;
        }
        let v: Value = serde_json::from_str(msg.to_text().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        if v.get("op").and_then(Value::as_u64) != Some(0) {
            continue;
        }

        let rpc_version = v
            .get("d")
            .and_then(|d| d.get("rpcVersion"))
            .and_then(Value::as_i64)
            .unwrap_or(1);
        let auth_payload = v
            .get("d")
            .and_then(|d| d.get("authentication"))
            .filter(|a| !a.is_null());
        let password = settings
            .password
            .trim()
            .to_string()
            .or_else_nonempty(read_password_from_obs_config);

        let mut d = json!({
            "rpcVersion": rpc_version,
            "eventSubscriptions": 1,
            "ignoreInvalidMessages": true,
        });
        if let Some(auth) = auth_payload {
            let password = password.ok_or_else(|| {
                "OBS WebSocket needs password. Save it in 533clip OBS settings.".to_string()
            })?;
            let challenge = auth
                .get("challenge")
                .and_then(Value::as_str)
                .ok_or_else(|| "OBS WebSocket auth challenge missing".to_string())?;
            let salt = auth
                .get("salt")
                .and_then(Value::as_str)
                .ok_or_else(|| "OBS WebSocket auth salt missing".to_string())?;
            d["authentication"] = Value::String(auth_token(&password, salt, challenge));
        }

        let identify = json!({ "op": 1, "d": d });
        write
            .send(Message::Text(identify.to_string().into()))
            .await
            .map_err(|e| e.to_string())?;
        break;
    }

    wait_for_identified(&mut read).await?;

    let request_id = format!(
        "533clip-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );
    let payload = json!({
        "op": 6,
        "d": {
            "requestType": request_type,
            "requestId": request_id,
            "requestData": request_data,
        }
    });
    write
        .send(Message::Text(payload.to_string().into()))
        .await
        .map_err(|e| e.to_string())?;

    while let Some(msg) = tokio::time::timeout(WS_RESPONSE_TIMEOUT, read.next())
        .await
        .map_err(|_| format!("OBS request timed out: {request_type}"))?
    {
        let msg = msg.map_err(|e| e.to_string())?;
        if !msg.is_text() {
            continue;
        }
        let v: Value = serde_json::from_str(msg.to_text().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        if v.get("op").and_then(Value::as_u64) != Some(7) {
            continue;
        }
        let d = v.get("d").cloned().unwrap_or_default();
        if d.get("requestId").and_then(Value::as_str) != Some(request_id.as_str()) {
            continue;
        }
        let status = d.get("requestStatus").cloned().unwrap_or_default();
        if status
            .get("result")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Ok(d.get("responseData").cloned().unwrap_or_else(|| json!({})));
        }
        let comment = status
            .get("comment")
            .and_then(Value::as_str)
            .unwrap_or("OBS request failed");
        return Err(comment.to_string());
    }

    Err(format!("OBS request failed: {request_type}"))
}

pub fn detect_websocket_url() -> Option<String> {
    let v = read_obs_websocket_config()?;
    let port = v
        .get("server_port")
        .and_then(Value::as_u64)
        .filter(|p| *p > 0 && *p <= u16::MAX as u64)
        .unwrap_or(4455);
    Some(format!("ws://127.0.0.1:{port}"))
}

pub fn ensure_websocket_enabled_config() -> Result<bool, String> {
    let path = obs_websocket_config_path()
        .ok_or_else(|| "OBS WebSocket config path not found".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut v = if path.is_file() {
        let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        serde_json::from_str::<Value>(&raw).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };
    let was_enabled = v
        .get("server_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if was_enabled {
        return Ok(false);
    }
    v["server_enabled"] = Value::Bool(true);
    if v.get("server_port").and_then(Value::as_u64).is_none() {
        v["server_port"] = Value::Number(4455.into());
    }
    if v.get("auth_required").and_then(Value::as_bool).is_none() {
        v["auth_required"] = Value::Bool(false);
    }
    if v.get("alerts_enabled").and_then(Value::as_bool).is_none() {
        v["alerts_enabled"] = Value::Bool(false);
    }
    let json = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(true)
}

fn connection_error_message(error: &str) -> String {
    if error.contains("10061") || error.to_ascii_lowercase().contains("actively refused") {
        "OBS WebSocket refused connection. 533clip enabled it in config; restart OBS, then Check again.".to_string()
    } else {
        error.to_string()
    }
}

fn resolve_websocket_url(settings: &ObsSettings) -> String {
    let configured = settings.websocket_url.trim();
    if configured.is_empty() || configured == "ws://127.0.0.1:4455" {
        detect_websocket_url().unwrap_or_else(|| "ws://127.0.0.1:4455".to_string())
    } else {
        configured.to_string()
    }
}

async fn wait_for_identified<S>(read: &mut S) -> Result<(), String>
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    while let Some(msg) = tokio::time::timeout(WS_HANDSHAKE_TIMEOUT, read.next())
        .await
        .map_err(|_| "OBS identify timed out".to_string())?
    {
        let msg = msg.map_err(|e| e.to_string())?;
        if !msg.is_text() {
            continue;
        }
        let v: Value = serde_json::from_str(msg.to_text().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        if v.get("op").and_then(Value::as_u64) == Some(2) {
            return Ok(());
        }
        if v.get("op").and_then(Value::as_u64) == Some(5) {
            continue;
        }
    }
    Err("OBS identify failed".to_string())
}

fn auth_token(password: &str, salt: &str, challenge: &str) -> String {
    let mut h = Sha256::new();
    h.update(password.as_bytes());
    h.update(salt.as_bytes());
    let secret = B64.encode(h.finalize());
    let mut h = Sha256::new();
    h.update(secret.as_bytes());
    h.update(challenge.as_bytes());
    B64.encode(h.finalize())
}

fn read_password_from_obs_config() -> Option<String> {
    let v = read_obs_websocket_config()?;
    v.get("server_password")?
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn read_obs_websocket_config() -> Option<Value> {
    let path = obs_websocket_config_path()?;
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn obs_websocket_config_path() -> Option<PathBuf> {
    Some(
        std::env::var("APPDATA")
            .ok()
            .map(PathBuf::from)?
            .join("obs-studio")
            .join("plugin_config")
            .join("obs-websocket")
            .join("config.json"),
    )
}

fn obs_profiles_root() -> Option<PathBuf> {
    Some(
        std::env::var("APPDATA")
            .ok()
            .map(PathBuf::from)?
            .join("obs-studio")
            .join("basic")
            .join("profiles"),
    )
}

fn new_obs_uuid() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut h = Sha256::new();
    h.update(nanos.to_le_bytes());
    h.update(std::process::id().to_le_bytes());
    let digest = h.finalize();
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        digest[0], digest[1], digest[2], digest[3],
        digest[4], digest[5],
        digest[6], digest[7],
        digest[8], digest[9],
        digest[10], digest[11], digest[12], digest[13], digest[14], digest[15]
    )
}

fn obs_hotkey_entry(hotkey: &str) -> Result<Value, String> {
    let parts = hotkey
        .split('+')
        .map(|part| part.trim().to_ascii_uppercase())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let mut entry = serde_json::Map::new();
    let mut key = None;

    for part in parts {
        match part.as_str() {
            "CTRL" | "CONTROL" => {
                entry.insert("control".into(), Value::Bool(true));
            }
            "SHIFT" => {
                entry.insert("shift".into(), Value::Bool(true));
            }
            "ALT" => {
                entry.insert("alt".into(), Value::Bool(true));
            }
            "CMD" | "COMMAND" | "META" | "WIN" => {
                entry.insert("command".into(), Value::Bool(true));
            }
            value => {
                key = Some(obs_key_name(value)?);
            }
        }
    }

    let key = key.ok_or_else(|| "Hotkey needs a key, example F8".to_string())?;
    entry.insert("key".into(), Value::String(key));
    Ok(Value::Object(entry))
}

fn obs_key_name(value: &str) -> Result<String, String> {
    if let Some(rest) = value.strip_prefix('F') {
        if let Ok(n) = rest.parse::<u8>() {
            if (1..=24).contains(&n) {
                return Ok(format!("OBS_KEY_F{n}"));
            }
        }
    }
    if value.len() == 1 {
        let c = value.chars().next().unwrap();
        if c.is_ascii_alphanumeric() {
            return Ok(format!("OBS_KEY_{c}"));
        }
    }
    match value {
        "SPACE" => Ok("OBS_KEY_SPACE".into()),
        "TAB" => Ok("OBS_KEY_TAB".into()),
        "ENTER" => Ok("OBS_KEY_RETURN".into()),
        "ESC" | "ESCAPE" => Ok("OBS_KEY_ESCAPE".into()),
        _ => Err(format!("Unsupported hotkey key: {value}")),
    }
}

fn upsert_ini_key(raw: &str, section: &str, key: &str, value: &str) -> String {
    let newline = if raw.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines = raw.lines().map(str::to_string).collect::<Vec<_>>();
    let section_header = format!("[{section}]");
    let key_prefix = format!("{key}=");
    let mut section_start = None;
    let mut section_end = lines.len();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case(&section_header) {
            section_start = Some(i);
            continue;
        }
        if section_start.is_some() && i > section_start.unwrap() && trimmed.starts_with('[') {
            section_end = i;
            break;
        }
    }

    if let Some(start) = section_start {
        for line in lines.iter_mut().take(section_end).skip(start + 1) {
            if line.trim_start().starts_with(&key_prefix) {
                *line = format!("{key}={value}");
                return format!("{}{}", lines.join(newline), newline);
            }
        }
        lines.insert(section_end, format!("{key}={value}"));
    } else {
        if !lines.last().map(|line| line.trim().is_empty()).unwrap_or(true) {
            lines.push(String::new());
        }
        lines.push(section_header);
        lines.push(format!("{key}={value}"));
    }

    format!("{}{}", lines.join(newline), newline)
}

trait NonEmptyString {
    fn or_else_nonempty<F: FnOnce() -> Option<String>>(self, f: F) -> Option<String>;
}

impl NonEmptyString for String {
    fn or_else_nonempty<F: FnOnce() -> Option<String>>(self, f: F) -> Option<String> {
        if self.trim().is_empty() {
            f()
        } else {
            Some(self)
        }
    }
}
