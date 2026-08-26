use crate::ffmpeg::{detect_audio_peaks, generate_thumbnail, probe_video, trim_lossless};
use crate::models::clip::ClipStatus;
use crate::pipeline::ClipPipeline;
use crate::queue::{Job, JobKind, JobQueue};
use crate::storage::ClipStore;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, Semaphore};
use tracing::{error, info, warn};

const MAX_CONCURRENT: usize = 1;
const MAX_RETRIES: u32 = 2;
/// Short clips don't need highlight markers — the whole clip already is the highlight.
const MIN_DURATION_FOR_PEAKS_SECS: f64 = 25.0;

/// Whether a clip is eligible for (and missing) audio highlight markers —
/// used both when a clip is first probed and to backfill older clips that
/// predate this feature (or hit an analysis bug before a fix shipped).
pub fn needs_audio_peaks(clip: &crate::models::clip::Clip) -> bool {
    clip.audio_peaks.is_none()
        && clip
            .duration
            .is_some_and(|d| d >= MIN_DURATION_FOR_PEAKS_SECS)
}
const CLIP_UPDATED: &str = "clip://updated";
const TRIM_COMPLETE: &str = "trim://complete";
const TRIM_FAILED: &str = "trim://failed";

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TrimEventPayload {
    source_clip_id: String,
    output_path: String,
    error: Option<String>,
}

/// Starts the background worker. `rx` must pair with the `JobQueue`'s sender.
pub fn spawn_queue_worker(
    app: AppHandle,
    store: ClipStore,
    data_dir: PathBuf,
    pipeline: ClipPipeline,
    mut rx: mpsc::Receiver<Job>,
    retry_tx: mpsc::Sender<Job>,
) {
    tauri::async_runtime::spawn(async move {
        let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT));
        while let Some(mut job) = rx.recv().await {
            let permit = match semaphore.clone().acquire_owned().await {
                Ok(p) => p,
                Err(_) => continue,
            };
            let app = app.clone();
            let store = store.clone();
            let data_dir = data_dir.clone();
            let pipeline = pipeline.clone();
            let retry_tx = retry_tx.clone();

            tauri::async_runtime::spawn(async move {
                let _permit = permit;
                if let Err(e) = run_job(&app, &store, &data_dir, &pipeline, &mut job).await {
                    if job.attempt < MAX_RETRIES {
                        job.attempt += 1;
                        warn!(job_id = job.id, error = %e, attempt = job.attempt, "retrying job");
                        let _ = retry_tx.send(job).await;
                    } else {
                        error!(job_id = job.id, error = %e, "job failed permanently");
                        handle_job_failure(&app, &store, &job, &e).await;
                    }
                }
            });
        }
    });
}

pub fn init_job_queue(
    app: AppHandle,
    store: ClipStore,
    data_dir: PathBuf,
) -> (JobQueue, ClipPipeline) {
    let (tx, rx) = mpsc::channel(256);
    let queue = JobQueue::new(tx.clone());
    let pipeline = ClipPipeline::new(app.clone(), store.clone(), queue.clone());
    spawn_queue_worker(app, store, data_dir, pipeline.clone(), rx, tx);
    (queue, pipeline)
}

async fn handle_job_failure(app: &AppHandle, store: &ClipStore, job: &Job, err: &str) {
    match &job.kind {
        JobKind::Trim {
            source_clip_id,
            output,
            ..
        } => {
            let _ = std::fs::remove_file(output);
            let _ = app.emit(
                TRIM_FAILED,
                &TrimEventPayload {
                    source_clip_id: source_clip_id.clone(),
                    output_path: output.to_string_lossy().into_owned(),
                    error: Some(err.to_string()),
                },
            );
        }
        JobKind::Probe { clip_id, .. } | JobKind::Thumbnail { clip_id, .. } => {
            if let Ok(Some(clip)) = store.update(clip_id, |c| c.status = ClipStatus::Failed) {
                let _ = app.emit(CLIP_UPDATED, &clip);
            }
        }
        JobKind::AudioPeaks { .. } => {
            // Highlight markers are a nice-to-have, not core clip data —
            // a failure here should never mark an otherwise-fine clip as Failed.
        }
    }
}

async fn run_job(
    app: &AppHandle,
    store: &ClipStore,
    data_dir: &PathBuf,
    pipeline: &ClipPipeline,
    job: &mut Job,
) -> Result<(), String> {
    match &job.kind {
        JobKind::Probe { clip_id, path } => {
            if store.get(clip_id).is_none() {
                // Clip was deleted (or merged away, see try_merge_overlapping)
                // while this job was queued; nothing to do.
                return Ok(());
            }
            info!(%clip_id, "probing clip metadata");
            let meta = probe_video(path).map_err(|e| e.to_string())?;
            let resolution = format!("{}x{}", meta.width, meta.height);
            let duration = meta.duration_secs;

            let clip = store
                .update(clip_id, |c| {
                    c.duration = Some(duration);
                    c.resolution = Some(resolution.clone());
                })
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "clip not found".to_string())?;

            if try_merge_overlapping(app, store, pipeline, &clip).await? {
                return Ok(());
            }

            if duration >= MIN_DURATION_FOR_PEAKS_SECS {
                let _ = pipeline
                    .queue()
                    .enqueue(Job::new(JobKind::AudioPeaks {
                        clip_id: clip_id.clone(),
                        path: path.clone(),
                    }))
                    .await;
            }

            let clip = maybe_mark_ready(store, clip_id)
                .map_err(|e| e.to_string())?
                .unwrap_or(clip);
            let _ = app.emit(CLIP_UPDATED, &clip);
            Ok(())
        }
        JobKind::Thumbnail { clip_id, path } => {
            if store.get(clip_id).is_none() {
                return Ok(());
            }
            let thumb_dir = data_dir.join("thumbnails");
            let thumb_path = thumb_dir.join(format!("{clip_id}.jpg"));
            info!(%clip_id, "generating thumbnail");
            generate_thumbnail(path, &thumb_path).map_err(|e| e.to_string())?;

            let thumb_str = thumb_path.to_string_lossy().into_owned();
            let clip = store
                .update(clip_id, |c| {
                    c.thumbnail_path = Some(thumb_str.clone());
                })
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "clip not found".to_string())?;

            let clip = maybe_mark_ready(store, clip_id)
                .map_err(|e| e.to_string())?
                .unwrap_or(clip);
            let _ = app.emit(CLIP_UPDATED, &clip);
            Ok(())
        }
        JobKind::Trim {
            source_clip_id,
            input,
            output,
            start_secs,
            end_secs,
            delete_original,
        } => {
            info!(%source_clip_id, start = %start_secs, end = %end_secs, "trimming clip");
            let source_clip = store
                .get(source_clip_id)
                .ok_or_else(|| "source clip not found".to_string())?;
            trim_lossless(input, output, *start_secs, *end_secs).map_err(|e| e.to_string())?;
            pipeline
                .on_clip_ready(output.clone(), false)
                .await
                .map_err(|e| e.to_string())?;
            let output_path = output.to_string_lossy().into_owned();
            if let Some(new_clip) = store.by_file_path(&output_path) {
                let title = source_clip
                    .game_name
                    .as_ref()
                    .map(|game| format!("{game} {}", chrono::Local::now().format("%d %b %H:%M")))
                    .or_else(|| source_clip.display_name.clone());
                let updated_new = store.update(&new_clip.id, |c| {
                        c.created_at = source_clip.created_at.clone();
                        c.game_name = source_clip.game_name.clone();
                        c.display_name = title.clone();
                    })
                    .map_err(|e| e.to_string())?
                    .unwrap_or(new_clip);
                let _ = app.emit(CLIP_UPDATED, &updated_new);
                crate::show_clip_saved_overlay(
                    app,
                    updated_new.game_name.clone(),
                    updated_new.file_name.clone(),
                    updated_new.display_name.clone(),
                );
                if *delete_original {
                    if let Ok(Some(removed)) = store.remove(source_clip_id) {
                        let _ = app.emit("clip://deleted", removed.id);
                    }
                }
            }
            let _ = app.emit(
                TRIM_COMPLETE,
                &TrimEventPayload {
                    source_clip_id: source_clip_id.clone(),
                    output_path: output.to_string_lossy().into_owned(),
                    error: None,
                },
            );
            Ok(())
        }
        JobKind::AudioPeaks { clip_id, path } => {
            if store.get(clip_id).is_none() {
                return Ok(());
            }
            info!(%clip_id, "analyzing audio for highlight markers");
            match detect_audio_peaks(path) {
                Ok(analysis) => {
                    info!(
                        %clip_id,
                        peak_count = analysis.peaks.len(),
                        waveform_points = analysis.waveform.len(),
                        ?analysis.peaks,
                        "audio peak analysis complete"
                    );
                    if let Some(clip) = store
                        .update(clip_id, |c| {
                            c.audio_peaks = Some(analysis.peaks.clone());
                            c.waveform = Some(analysis.waveform.clone());
                        })
                        .map_err(|e| e.to_string())?
                    {
                        let _ = app.emit(CLIP_UPDATED, &clip);
                    }
                }
                Err(e) => {
                    warn!(%clip_id, error = %e, "audio peak analysis failed, skipping highlight markers");
                }
            }
            Ok(())
        }
    }
}

fn maybe_mark_ready(
    store: &ClipStore,
    clip_id: &str,
) -> Result<Option<crate::models::clip::Clip>, crate::storage::StoreError> {
    store.update(clip_id, |c| {
        if c.duration.is_some() && c.thumbnail_path.is_some() {
            c.status = ClipStatus::Ready;
        }
    })
}

// Two OBS replay-buffer saves taken moments apart both capture the trailing
// N seconds of the same session, so their recorded windows can genuinely
// overlap in real time. Require a bit of unambiguous overlap (not just
// clock-rounding noise) and at least a bit of genuinely new footage before
// merging, so a merge is never triggered on a coincidence.
const MIN_OVERLAP_SECS: f64 = 0.75;
const MIN_NEW_CONTENT_SECS: f64 = 0.75;

/// Outplayed-style auto-merge: if `current`'s recorded time window
/// (`created_at - duration` .. `created_at`) overlaps the most recent other
/// clip's window, splice them into one continuous clip via ffmpeg instead of
/// keeping two separate, partially-duplicate clips. Returns `true` if a
/// merge happened (in which case `current`'s own clip record has already
/// been removed, replaced by the merged clip).
async fn try_merge_overlapping(
    app: &AppHandle,
    store: &ClipStore,
    pipeline: &ClipPipeline,
    current: &crate::models::clip::Clip,
) -> Result<bool, String> {
    let Some(cur_duration) = current.duration else {
        return Ok(false);
    };
    if cur_duration <= 0.0 {
        return Ok(false);
    }
    // Manually-trimmed clips inherit their source clip's created_at (see the
    // Trim job below) rather than getting a fresh end timestamp, so their
    // computed window can look artificially short and spuriously "overlap"
    // an unrelated nearby clip. Only raw watcher-detected clips are eligible
    // to trigger a merge; a trimmed/merged clip can still be matched as
    // someone else's `prev`, since its inherited/kept timestamp is accurate.
    if current.file_name.contains("_trim_") {
        return Ok(false);
    }
    let Ok(cur_end) = chrono::DateTime::parse_from_rfc3339(&current.created_at) else {
        return Ok(false);
    };
    let cur_end = cur_end.with_timezone(&chrono::Utc);
    let cur_start = cur_end - chrono::Duration::milliseconds((cur_duration * 1000.0) as i64);

    let mut best: Option<(crate::models::clip::Clip, chrono::DateTime<chrono::Utc>)> = None;
    for candidate in store.list() {
        if candidate.id == current.id {
            continue;
        }
        let Some(prev_duration) = candidate.duration else {
            continue;
        };
        if prev_duration <= 0.0 {
            continue;
        }
        let Ok(prev_end) = chrono::DateTime::parse_from_rfc3339(&candidate.created_at) else {
            continue;
        };
        let prev_end = prev_end.with_timezone(&chrono::Utc);
        if prev_end >= cur_end {
            continue;
        }
        let is_better = match &best {
            Some((_, best_end)) => prev_end > *best_end,
            None => true,
        };
        if is_better {
            best = Some((candidate, prev_end));
        }
    }
    let Some((prev, prev_end)) = best else {
        return Ok(false);
    };
    let prev_duration = prev.duration.unwrap_or(0.0);

    let overlap_secs = (prev_end - cur_start).num_milliseconds() as f64 / 1000.0;
    if overlap_secs < MIN_OVERLAP_SECS || overlap_secs >= cur_duration - MIN_NEW_CONTENT_SECS {
        return Ok(false);
    }
    let trim_start = overlap_secs.min(prev_duration);

    let prev_path = PathBuf::from(&prev.file_path);
    let cur_path = PathBuf::from(&current.file_path);
    if !prev_path.is_file() || !cur_path.is_file() {
        return Ok(false);
    }

    let output = crate::trim_paths::merged_output_path(&cur_path);
    if let Err(e) =
        crate::ffmpeg::merge_overlapping_clips(&prev_path, &cur_path, trim_start, &output)
    {
        warn!(error = %e, "overlap merge failed, keeping clips separate");
        return Ok(false);
    }

    pipeline
        .on_clip_ready(output.clone(), false)
        .await
        .map_err(|e| e.to_string())?;
    let output_str = output.to_string_lossy().into_owned();
    let Some(merged) = store.by_file_path(&output_str) else {
        return Ok(true);
    };

    let mut tags = prev.tags.clone();
    for t in &current.tags {
        if !tags.contains(t) {
            tags.push(t.clone());
        }
    }
    let game_name = prev.game_name.clone().or_else(|| current.game_name.clone());
    let display_name = prev
        .display_name
        .clone()
        .or_else(|| current.display_name.clone());
    let is_favorite = prev.is_favorite || current.is_favorite;
    let created_at = current.created_at.clone();

    let merged = store
        .update(&merged.id, |c| {
            c.created_at = created_at.clone();
            c.game_name = game_name.clone();
            c.display_name = display_name.clone();
            c.tags = tags.clone();
            c.is_favorite = is_favorite;
        })
        .map_err(|e| e.to_string())?
        .unwrap_or(merged);

    // No overlay popup here: the user already saw a "saved" notification for
    // each raw OBS save (prev and current), fired instantly by the watcher
    // before either was even probed. A third popup announcing the merge
    // itself is just noise on top of those two real ones.
    let _ = app.emit(CLIP_UPDATED, &merged);
    crate::emit_debug(app, format!("merged overlapping clips into {}", merged.file_name));

    let removed = store
        .remove_many(&[prev.id.clone(), current.id.clone()])
        .map_err(|e| e.to_string())?;
    for r in removed {
        let _ = app.emit("clip://deleted", r.id);
    }

    Ok(true)
}
