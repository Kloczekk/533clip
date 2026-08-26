use crate::ffmpeg::command::hidden_command;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ThumbnailError {
    #[error("ffmpeg failed: {0}")]
    Failed(String),
}

/// Extract a single frame as a lightweight JPEG.
///
/// Two real-world quirks handled here, both confirmed against actual
/// captured files rather than assumed:
/// - OBS's h264 output is limited/"tv" color range, which ffmpeg's MJPEG
///   encoder refuses to write directly ("Non full-range YUV is
///   non-standard, set strict_std_compliance..."). Converting to JPEG's
///   full-range pixel format first avoids the encoder ever seeing it.
/// - Some recordings (seen from Roblox's built-in recorder) report a
///   corrupted/near-zero container duration despite having real video
///   content. Seeking to a fixed offset like 1s then silently seeks past
///   what ffmpeg believes is EOF and produces no frame at all (exit 0, no
///   output file). If the seeked attempt doesn't produce a file, retry
///   once from frame 0, which works for these regardless of duration
///   metadata.
pub fn generate_thumbnail(input: &Path, output: &Path) -> Result<(), ThumbnailError> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ThumbnailError::Failed(e.to_string()))?;
    }

    if let Err(e) = try_extract_frame(input, output, Some("1")) {
        if output.exists() {
            let _ = std::fs::remove_file(output);
        }
        try_extract_frame(input, output, None).map_err(|_| e)?;
    }

    if !output.exists() || output.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
        return Err(ThumbnailError::Failed("thumbnail output missing or empty".into()));
    }

    Ok(())
}

fn try_extract_frame(
    input: &Path,
    output: &Path,
    seek_secs: Option<&str>,
) -> Result<(), ThumbnailError> {
    let mut cmd = hidden_command("ffmpeg");
    cmd.args(["-hide_banner", "-loglevel", "error", "-y"]);
    if let Some(secs) = seek_secs {
        cmd.args(["-ss", secs]);
    }
    let result = cmd
        .arg("-i")
        .arg(input)
        .args([
            "-frames:v",
            "1",
            "-vf",
            "format=yuvj420p",
            "-q:v",
            "4",
            "-f",
            "image2",
        ])
        .arg(output)
        .output()
        .map_err(|e| ThumbnailError::Failed(format!("could not run ffmpeg: {e}")))?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(ThumbnailError::Failed(stderr.trim().to_string()));
    }
    if !output.exists() || output.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
        return Err(ThumbnailError::Failed(
            "ffmpeg reported success but wrote no frame".into(),
        ));
    }

    Ok(())
}
