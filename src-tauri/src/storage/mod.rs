mod json_store;
mod settings;
mod tags;

pub use json_store::{ClipStore, StoreError};
pub use settings::{
    ObsSettings, ObsSettingsResponse, ObsSettingsUpdate, R2Settings, R2SettingsResponse,
    R2SettingsUpdate, SettingsStore,
};
pub use tags::{merge_tags_from_clips, TagRegistryStore};

use std::path::Path;

/// Writes `contents` to `path` via a temp file + rename so a crash or power loss
/// mid-write can never leave a truncated/corrupt file at `path`.
pub(crate) fn atomic_write(path: &Path, contents: &str) -> std::io::Result<()> {
    let mut tmp_name = path.as_os_str().to_os_string();
    tmp_name.push(".tmp");
    let tmp_path = std::path::PathBuf::from(tmp_name);
    std::fs::write(&tmp_path, contents)?;
    std::fs::rename(&tmp_path, path)
}

/// Reads and parses a JSON file, tolerating a missing/empty file via `default`.
/// If the file exists but fails to parse (e.g. corrupted by a prior crash), the
/// bad file is moved aside instead of returning an error, so callers never fail
/// to start up over a corrupt store — a fresh/default store is used instead.
pub(crate) fn load_json_or_default<T>(path: &Path) -> std::io::Result<T>
where
    T: Default + serde::de::DeserializeOwned,
{
    if !path.exists() {
        return Ok(T::default());
    }
    let raw = std::fs::read_to_string(path)?;
    if raw.trim().is_empty() {
        return Ok(T::default());
    }
    match serde_json::from_str(&raw) {
        Ok(value) => Ok(value),
        Err(e) => {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "store file is corrupted, backing it up and starting fresh"
            );
            let mut backup_name = path.as_os_str().to_os_string();
            backup_name.push(".corrupt");
            let _ = std::fs::rename(path, std::path::PathBuf::from(backup_name));
            Ok(T::default())
        }
    }
}
