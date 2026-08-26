use crate::ffmpeg::command::hidden_command;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioPeaksError {
    #[error("ffmpeg audio analysis failed: {0}")]
    Failed(String),
}

pub struct AudioAnalysis {
    /// Timestamps (seconds) of local loudness spikes — used for the
    /// "jump to next/previous highlight" navigation.
    pub peaks: Vec<f64>,
    /// Downsampled, normalized (0.0-1.0) amplitude series for rendering a
    /// waveform on the trim timeline, evenly spaced across the clip's
    /// duration regardless of clip length.
    pub waveform: Vec<f32>,
}

const SAMPLE_RATE: u32 = 44100;
/// Finer than 1s so short clips still render a readable waveform shape.
const BUCKET_SAMPLES: u32 = SAMPLE_RATE / 4; // 0.25s per analysis bucket
/// Peaks closer together than this are treated as one loud moment (keep the loudest).
const MIN_PEAK_GAP_SECS: f64 = 4.0;
/// Cap how many highlight markers a single clip can get, even a very loud one.
const MAX_PEAKS: usize = 10;
/// A bucket must be at least this many dB louder than the clip's own average
/// to count as a highlight, so a uniformly loud clip (e.g. constant music)
/// doesn't get marked everywhere.
const MIN_DB_ABOVE_AVERAGE: f64 = 6.0;
/// Waveform bar count is capped regardless of clip length, so a multi-minute
/// merged clip doesn't bloat clips.json or the timeline with thousands of bars.
const MAX_WAVEFORM_POINTS: usize = 240;
/// dB floor used to normalize amplitude for display — anything quieter than
/// this renders as silence (a flat/near-zero bar).
const DISPLAY_FLOOR_DB: f64 = -50.0;

/// Analyzes a clip's audio for highlight timestamps and a waveform shape —
/// a cheap, local heuristic (reactions, gunfire, sudden yelling tend to be
/// loudness spikes) without any ML/cloud dependency. Buckets audio into
/// short frames via ffmpeg's astats filter. Never fails the caller on a
/// parse or ffmpeg quirk in a way that loses clip data — worst case this
/// returns empty results and the UI just shows no waveform for that clip.
pub fn detect_audio_peaks(input: &Path) -> Result<AudioAnalysis, AudioPeaksError> {
    // ffmpeg's `ametadata=file=...` value goes through the filtergraph's own
    // suboption parser, which splits on `:` — a Windows absolute path
    // (`C:/Users/...`) breaks it even quoted or backslash-escaped ("No
    // option name near ..."). Sidestep this entirely by running ffmpeg with
    // its cwd set to the temp dir and passing a bare filename, so the `file=`
    // value never contains a colon or path separator to begin with.
    let temp_dir = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let metadata_name = format!("533clip-audio-{}-{nanos}.txt", std::process::id());
    let metadata_path = temp_dir.join(&metadata_name);

    let filter = format!(
        "aresample={SAMPLE_RATE},asetnsamples=n={BUCKET_SAMPLES},astats=metadata=1:reset=1,ametadata=mode=print:key=lavfi.astats.Overall.RMS_level:file={metadata_name}"
    );

    let mut cmd = hidden_command("ffmpeg");
    cmd.current_dir(&temp_dir);
    let result = cmd
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(input)
        .args(["-af", &filter, "-f", "null", "-"])
        .output()
        .map_err(|e| AudioPeaksError::Failed(format!("could not run ffmpeg: {e}")))?;

    if !result.status.success() {
        let _ = std::fs::remove_file(&metadata_path);
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(AudioPeaksError::Failed(stderr.trim().to_string()));
    }

    let raw = std::fs::read_to_string(&metadata_path).unwrap_or_default();
    let _ = std::fs::remove_file(&metadata_path);

    let samples = parse_rms_samples(&raw);
    Ok(AudioAnalysis {
        peaks: pick_peaks(&samples),
        waveform: build_waveform(&samples),
    })
}

fn parse_rms_samples(raw: &str) -> Vec<(f64, f64)> {
    let mut samples = Vec::new();
    let mut pending_time: Option<f64> = None;
    for line in raw.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("frame:") {
            if let Some(idx) = rest.find("pts_time:") {
                pending_time = rest[idx + "pts_time:".len()..]
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse::<f64>().ok());
            }
        } else if let Some(value) = line.strip_prefix("lavfi.astats.Overall.RMS_level=") {
            if let Some(time) = pending_time.take() {
                let db = if value.trim().eq_ignore_ascii_case("-inf") {
                    -100.0
                } else {
                    value.trim().parse::<f64>().unwrap_or(-100.0)
                };
                samples.push((time, db));
            }
        }
    }
    samples
}

fn normalize_db(db: f64) -> f32 {
    (((db.max(DISPLAY_FLOOR_DB) - DISPLAY_FLOOR_DB) / -DISPLAY_FLOOR_DB).clamp(0.0, 1.0)) as f32
}

fn build_waveform(samples: &[(f64, f64)]) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }
    if samples.len() <= MAX_WAVEFORM_POINTS {
        return samples.iter().map(|(_, db)| normalize_db(*db)).collect();
    }

    let group_size = (samples.len() as f64 / MAX_WAVEFORM_POINTS as f64).ceil() as usize;
    samples
        .chunks(group_size.max(1))
        .map(|chunk| {
            // Average in linear amplitude, not dB, so a brief loud spike
            // inside a mostly-quiet downsampled bucket still shows up.
            let linear_avg = chunk
                .iter()
                .map(|(_, db)| 10f64.powf(db.max(DISPLAY_FLOOR_DB) / 20.0))
                .sum::<f64>()
                / chunk.len() as f64;
            let db = 20.0 * linear_avg.log10();
            normalize_db(db)
        })
        .collect()
}

fn pick_peaks(samples: &[(f64, f64)]) -> Vec<f64> {
    if samples.len() < 6 {
        return Vec::new();
    }
    let avg = samples.iter().map(|(_, db)| db).sum::<f64>() / samples.len() as f64;
    let threshold = avg + MIN_DB_ABOVE_AVERAGE;

    let mut candidates: Vec<(f64, f64)> = Vec::new();
    for i in 1..samples.len() - 1 {
        let (t, db) = samples[i];
        if db < threshold {
            continue;
        }
        if db >= samples[i - 1].1 && db >= samples[i + 1].1 {
            candidates.push((t, db));
        }
    }

    candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut merged: Vec<(f64, f64)> = Vec::new();
    for (t, db) in candidates {
        match merged.last_mut() {
            Some(last) if t - last.0 < MIN_PEAK_GAP_SECS => {
                if db > last.1 {
                    *last = (t, db);
                }
            }
            _ => merged.push((t, db)),
        }
    }

    merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    merged.truncate(MAX_PEAKS);
    let mut peaks: Vec<f64> = merged.into_iter().map(|(t, _)| t).collect();
    peaks.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    peaks
}
