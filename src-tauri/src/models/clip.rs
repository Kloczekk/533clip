use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipStatus {
    Processing,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: String,
    pub file_path: String,
    pub file_name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    pub created_at: String,
    pub duration: Option<f64>,
    pub resolution: Option<String>,
    pub thumbnail_path: Option<String>,
    pub is_favorite: bool,
    pub tags: Vec<String>,
    pub status: ClipStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipDetectedPayload {
    pub id: String,
    pub file_path: String,
    pub file_name: String,
    pub created_at: String,
}

pub fn stable_clip_id(path: &Path, created_at: DateTime<Utc>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.to_string_lossy().as_bytes());
    hasher.update(created_at.timestamp_millis().to_le_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn clip_from_path(path: &Path, status: ClipStatus) -> std::io::Result<Clip> {
    let metadata = std::fs::metadata(path)?;
    let created_at: DateTime<Utc> = metadata
        .created()
        .or_else(|_| metadata.modified())
        .map(DateTime::<Utc>::from)
        .unwrap_or_else(|_| Utc::now());

    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown".into());

    Ok(Clip {
        id: stable_clip_id(path, created_at),
        file_path: path.to_string_lossy().into_owned(),
        file_name,
        display_name: None,
        created_at: created_at.to_rfc3339(),
        duration: None,
        resolution: None,
        thumbnail_path: None,
        is_favorite: false,
        tags: Vec::new(),
        status,
    })
}

pub fn clip_detected_payload(path: &Path) -> std::io::Result<ClipDetectedPayload> {
    let clip = clip_from_path(path, ClipStatus::Processing)?;
    Ok(ClipDetectedPayload {
        id: clip.id,
        file_path: clip.file_path,
        file_name: clip.file_name,
        created_at: clip.created_at,
    })
}
