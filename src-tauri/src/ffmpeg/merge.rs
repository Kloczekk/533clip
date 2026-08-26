use crate::ffmpeg::command::hidden_command;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MergeError {
    #[error("ffmpeg merge failed: {0}")]
    Failed(String),
}

/// Joins two clips whose recorded time windows overlap (consecutive OBS
/// replay-buffer saves of the same moment) into one continuous clip:
/// `prev` in full, followed by `next` with its first `next_trim_start_secs`
/// seconds skipped (the portion `prev` already covers). Re-encodes via a
/// concat filter rather than stream-copy concat, since `prev` (the original
/// OBS output) and `next` are not guaranteed to share identical codec
/// parameters.
pub fn merge_overlapping_clips(
    prev: &Path,
    next: &Path,
    next_trim_start_secs: f64,
    output: &Path,
) -> Result<(), MergeError> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|e| MergeError::Failed(e.to_string()))?;
    }

    let trim_start = format!("{:.3}", next_trim_start_secs.max(0.0));

    let result = hidden_command("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .arg("-i")
        .arg(prev)
        .args(["-ss", &trim_start])
        .arg("-i")
        .arg(next)
        .args([
            "-filter_complex",
            "[0:v:0][0:a:0][1:v:0][1:a:0]concat=n=2:v=1:a=1[v][a]",
            "-map",
            "[v]",
            "-map",
            "[a]",
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "18",
            "-c:a",
            "aac",
            "-b:a",
            "192k",
            "-movflags",
            "+faststart",
        ])
        .arg(output)
        .output()
        .map_err(|e| MergeError::Failed(format!("could not run ffmpeg: {e}")))?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(MergeError::Failed(stderr.trim().to_string()));
    }

    if !output.exists() || output.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
        return Err(MergeError::Failed("merge output missing or empty".into()));
    }

    Ok(())
}
