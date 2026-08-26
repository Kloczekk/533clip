use chrono::Utc;
use std::path::{Path, PathBuf};

/// Builds a unique trimmed output path next to the source file (never overwrites the original).
pub fn trimmed_output_path(source: &Path) -> PathBuf {
    let parent = source.parent().unwrap_or_else(|| Path::new("."));
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "clip".into());
    let ts = Utc::now().format("%Y%m%d-%H%M%S");
    parent.join(format!("{stem}_trim_{ts}.mp4"))
}

/// Builds a unique merged-clip output path next to the newer source file.
pub fn merged_output_path(newer_source: &Path) -> PathBuf {
    let parent = newer_source.parent().unwrap_or_else(|| Path::new("."));
    let stem = newer_source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "clip".into());
    let ts = Utc::now().format("%Y%m%d-%H%M%S");
    parent.join(format!("{stem}_merged_{ts}.mp4"))
}
