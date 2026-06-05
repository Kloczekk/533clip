use crate::ffmpeg::command::hidden_command;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TrimError {
    #[error("ffmpeg trim failed: {0}")]
    Failed(String),
}

/// Lossless trim using stream copy (`-c copy`). Never modifies the source file.
pub fn trim_lossless(
    input: &Path,
    output: &Path,
    start_secs: f64,
    end_secs: f64,
) -> Result<(), TrimError> {
    if end_secs <= start_secs {
        return Err(TrimError::Failed("end must be after start".into()));
    }

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|e| TrimError::Failed(e.to_string()))?;
    }

    let duration = end_secs - start_secs;
    let start = format!("{start_secs:.3}");
    let dur = format!("{duration:.3}");

    let output_status = hidden_command("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-ss",
            &start,
            "-i",
        ])
        .arg(input)
        .args(["-t", &dur, "-c", "copy", "-avoid_negative_ts", "make_zero"])
        .arg(output)
        .output()
        .map_err(|e| TrimError::Failed(format!("could not run ffmpeg: {e}")))?;

    if !output_status.status.success() {
        let stderr = String::from_utf8_lossy(&output_status.stderr);
        return Err(TrimError::Failed(stderr.trim().to_string()));
    }

    if !output.exists() || output.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
        return Err(TrimError::Failed("trim output missing or empty".into()));
    }

    Ok(())
}
