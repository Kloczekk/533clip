use crate::ffmpeg::command::hidden_command;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ThumbnailError {
    #[error("ffmpeg failed: {0}")]
    Failed(String),
}

/// Extract a single frame at ~3 seconds as a lightweight JPEG.
pub fn generate_thumbnail(input: &Path, output: &Path) -> Result<(), ThumbnailError> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ThumbnailError::Failed(e.to_string()))?;
    }

    // -ss before -i for fast seek; frame at ~1s works for short replays too
    let output = hidden_command("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-ss",
            "1",
            "-i",
        ])
        .arg(input)
        .args(["-frames:v", "1", "-q:v", "4", "-f", "image2"])
        .arg(output)
        .output()
        .map_err(|e| ThumbnailError::Failed(format!("could not run ffmpeg: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ThumbnailError::Failed(stderr.trim().to_string()));
    }

    Ok(())
}
