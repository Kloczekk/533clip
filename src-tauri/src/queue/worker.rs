use crate::ffmpeg::{generate_thumbnail, probe_video, trim_lossless};
use crate::models::clip::ClipStatus;
use crate::pipeline::ClipPipeline;
use crate::queue::{Job, JobKind, JobQueue};
use crate::storage::ClipStore;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, Semaphore};
use tracing::{error, info, warn};

const MAX_CONCURRENT: usize = 2;
const MAX_RETRIES: u32 = 2;
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

            let clip = maybe_mark_ready(store, clip_id)
                .map_err(|e| e.to_string())?
                .unwrap_or(clip);
            let _ = app.emit(CLIP_UPDATED, &clip);
            Ok(())
        }
        JobKind::Thumbnail { clip_id, path } => {
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
        } => {
            info!(%source_clip_id, start = %start_secs, end = %end_secs, "trimming clip (lossless)");
            if store.get(source_clip_id).is_none() {
                return Err("source clip not found".into());
            }
            trim_lossless(input, output, *start_secs, *end_secs).map_err(|e| e.to_string())?;
            pipeline
                .on_clip_ready(output.clone())
                .await
                .map_err(|e| e.to_string())?;
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
