use crate::ffmpeg::command::hidden_command;
use serde::Deserialize;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProbeError {
    #[error("ffprobe failed: {0}")]
    Failed(String),
    #[error("invalid ffprobe output: {0}")]
    Parse(String),
}

#[derive(Debug)]
pub struct VideoMetadata {
    pub duration_secs: f64,
    pub width: u32,
    pub height: u32,
}

#[derive(Deserialize)]
struct FfprobeRoot {
    format: Option<FfprobeFormat>,
    streams: Option<Vec<FfprobeStream>>,
}

#[derive(Deserialize)]
struct FfprobeFormat {
    duration: Option<String>,
}

#[derive(Deserialize)]
struct FfprobeStream {
    codec_type: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
}

pub fn probe_video(path: &Path) -> Result<VideoMetadata, ProbeError> {
    let output = hidden_command("ffprobe")
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .output()
        .map_err(|e| ProbeError::Failed(format!("could not run ffprobe: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ProbeError::Failed(stderr.trim().to_string()));
    }

    let parsed: FfprobeRoot = serde_json::from_slice(&output.stdout)
        .map_err(|e| ProbeError::Parse(e.to_string()))?;

    let duration_secs = parsed
        .format
        .as_ref()
        .and_then(|f| f.duration.as_ref())
        .and_then(|d| d.parse::<f64>().ok())
        .unwrap_or(0.0);

    let video = parsed
        .streams
        .as_ref()
        .and_then(|streams| {
            streams
                .iter()
                .find(|s| s.codec_type.as_deref() == Some("video"))
        })
        .ok_or_else(|| ProbeError::Parse("no video stream".into()))?;

    let width = video.width.ok_or_else(|| ProbeError::Parse("no width".into()))?;
    let height = video.height.ok_or_else(|| ProbeError::Parse("no height".into()))?;

    Ok(VideoMetadata {
        duration_secs,
        width,
        height,
    })
}
