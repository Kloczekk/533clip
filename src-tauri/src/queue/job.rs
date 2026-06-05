use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;

static JOB_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub enum JobKind {
    Probe { clip_id: String, path: PathBuf },
    Thumbnail { clip_id: String, path: PathBuf },
    Trim {
        source_clip_id: String,
        input: PathBuf,
        output: PathBuf,
        start_secs: f64,
        end_secs: f64,
    },
}

#[derive(Debug, Clone)]
pub struct Job {
    pub id: u64,
    pub kind: JobKind,
    pub attempt: u32,
}

impl Job {
    pub fn new(kind: JobKind) -> Self {
        Self {
            id: JOB_COUNTER.fetch_add(1, Ordering::Relaxed),
            kind,
            attempt: 0,
        }
    }
}

#[derive(Clone)]
pub struct JobQueue {
    tx: mpsc::Sender<Job>,
}

impl JobQueue {
    pub fn new(tx: mpsc::Sender<Job>) -> Self {
        Self { tx }
    }

    pub async fn enqueue(&self, job: Job) -> Result<(), mpsc::error::SendError<Job>> {
        self.tx.send(job).await
    }
}
