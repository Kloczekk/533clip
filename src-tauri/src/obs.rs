use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Video extensions OBS commonly writes (replay buffer, recording, remux).
pub const CLIP_EXTENSIONS: &[&str] = &["mp4", "mkv", "mov"];

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

/// Local OBS obs-websocket plugin config (`config.json` on Windows).
#[derive(Debug, Clone)]
pub struct LocalObsWebSocketConfig {
    pub port: u16,
    pub password: Option<String>,
    pub server_enabled: bool,
}

pub fn read_local_websocket_config() -> Option<LocalObsWebSocketConfig> {
    let path = std::env::var("APPDATA")
        .ok()
        .map(PathBuf::from)
        .map(|p| {
            p.join("obs-studio")
                .join("plugin_config")
                .join("obs-websocket")
                .join("config.json")
        })?;

    if !path.is_file() {
        return None;
    }

    #[derive(serde::Deserialize)]
    struct Raw {
        #[serde(default)]
        server_enabled: bool,
        #[serde(default = "default_port")]
        server_port: u16,
        #[serde(default)]
        server_password: Option<String>,
    }

    fn default_port() -> u16 {
        4455
    }

    let raw = std::fs::read_to_string(&path).ok()?;
    let cfg: Raw = serde_json::from_str(&raw).ok()?;
    let password = cfg
        .server_password
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty());

    Some(LocalObsWebSocketConfig {
        port: cfg.server_port,
        password,
        server_enabled: cfg.server_enabled,
    })
}
