use crate::ffmpeg::command::hidden_command;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ShareExportError {
    #[error("ffmpeg export failed: {0}")]
    Failed(String),
}

/// Stay safely under Discord's ~10MB free-tier attachment limit.
const TARGET_BYTES: f64 = 9.3 * 1024.0 * 1024.0;
const AUDIO_BITRATE_KBPS: f64 = 96.0;
const MIN_VIDEO_BITRATE_KBPS: f64 = 150.0;
const MAX_VIDEO_BITRATE_KBPS: f64 = 6000.0;

/// Exports a size-targeted, Discord-attachment-friendly copy of a clip.
///
/// The previous version just encoded at a fixed CRF with no size target —
/// anything longer than ~15-20s of gameplay came out well over Discord's
/// upload limit, so the export "worked" but the file couldn't actually be
/// attached. This computes a video bitrate from `duration_secs` so the
/// output lands under the target size, using a proper 2-pass encode (single
/// pass + `-b:v` alone can overshoot by a wide margin). Resolution also
/// scales down for longer clips instead of starving a fixed 1280px width of
/// bitrate, since lower-res-but-well-fed beats high-res-and-blocky.
pub fn export_for_discord(
    input: &Path,
    output: &Path,
    duration_secs: f64,
) -> Result<(), ShareExportError> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ShareExportError::Failed(e.to_string()))?;
    }

    let duration = duration_secs.max(0.5);
    let target_bits = TARGET_BYTES * 8.0;
    let audio_bits = AUDIO_BITRATE_KBPS * 1000.0 * duration;
    let video_bitrate_kbps = ((target_bits - audio_bits) / duration / 1000.0)
        .clamp(MIN_VIDEO_BITRATE_KBPS, MAX_VIDEO_BITRATE_KBPS);
    let video_bitrate = format!("{}k", video_bitrate_kbps.round() as i64);

    let scale_width: i32 = if video_bitrate_kbps < 700.0 {
        854
    } else if video_bitrate_kbps < 1800.0 {
        1024
    } else {
        1280
    };
    let scale_filter = format!("scale={scale_width}:-2:flags=lanczos");

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let passlog = std::env::temp_dir().join(format!(
        "533clip-share-{}-{nanos}",
        std::process::id()
    ));
    let passlog_arg = passlog.to_string_lossy().into_owned();

    let cleanup_passlogs = || {
        let _ = std::fs::remove_file(format!("{passlog_arg}-0.log"));
        let _ = std::fs::remove_file(format!("{passlog_arg}-0.log.mbtree"));
    };

    let pass1 = hidden_command("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(input)
        .args([
            "-map",
            "0:v:0",
            "-an",
            "-vf",
            &scale_filter,
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-b:v",
            &video_bitrate,
            "-pass",
            "1",
            "-passlogfile",
            &passlog_arg,
            "-f",
            "mp4",
        ])
        .arg(if cfg!(windows) { "NUL" } else { "/dev/null" })
        .output()
        .map_err(|e| ShareExportError::Failed(format!("could not run ffmpeg (pass 1): {e}")))?;

    if !pass1.status.success() {
        cleanup_passlogs();
        return Err(ShareExportError::Failed(
            String::from_utf8_lossy(&pass1.stderr).trim().to_string(),
        ));
    }

    let pass2 = hidden_command("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(input)
        .args([
            "-map",
            "0:v:0",
            "-map",
            "0:a?",
            "-vf",
            &scale_filter,
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-b:v",
            &video_bitrate,
            "-pass",
            "2",
            "-passlogfile",
            &passlog_arg,
            "-c:a",
            "aac",
            "-b:a",
            "96k",
            "-movflags",
            "+faststart",
        ])
        .arg(output)
        .output()
        .map_err(|e| ShareExportError::Failed(format!("could not run ffmpeg (pass 2): {e}")))?;

    cleanup_passlogs();

    if !pass2.status.success() {
        return Err(ShareExportError::Failed(
            String::from_utf8_lossy(&pass2.stderr).trim().to_string(),
        ));
    }

    if !output.exists() || output.metadata().map(|m| m.len()).unwrap_or(0) == 0 {
        return Err(ShareExportError::Failed("export output missing or empty".into()));
    }

    Ok(())
}
