use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, warn};

const POLL_INTERVAL: Duration = Duration::from_millis(250);
const STABLE_SAMPLES: u32 = 8;
const MAX_WAIT: Duration = Duration::from_secs(120);

/// Waits until the file size is unchanged across several consecutive samples.
/// OBS may temporarily remove/rename files (e.g. `.mkv` → `.mp4`); missing files are retried.
pub async fn wait_until_stable(path: &Path) -> bool {
    let started = std::time::Instant::now();
    let mut last_size: Option<u64> = None;
    let mut stable_count = 0u32;

    while started.elapsed() < MAX_WAIT {
        match std::fs::metadata(path) {
            Ok(meta) => {
                let size = meta.len();
                if size == 0 {
                    stable_count = 0;
                    last_size = Some(0);
                } else if Some(size) == last_size {
                    stable_count += 1;
                    if stable_count >= STABLE_SAMPLES {
                        debug!(?path, size, "file size stable");
                        return true;
                    }
                } else {
                    stable_count = 0;
                    last_size = Some(size);
                }
            }
            Err(e) if e.kind() == ErrorKind::NotFound => {
                // OBS often deletes/renames while finalizing — keep waiting quietly.
                stable_count = 0;
                last_size = None;
                debug!(?path, "file not found yet, still waiting");
            }
            Err(e) => {
                debug!(?path, error = %e, "metadata read failed, retrying");
                stable_count = 0;
                last_size = None;
            }
        }
        sleep(POLL_INTERVAL).await;
    }

    if path.exists() {
        warn!(?path, "timed out waiting for file stability");
    } else {
        debug!(?path, "file never stabilized (likely renamed by OBS)");
    }
    false
}
