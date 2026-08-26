mod audio_peaks;
mod command;
mod merge;
mod probe;
mod share;
mod thumbnail;
mod trim;

pub use audio_peaks::detect_audio_peaks;
pub use merge::merge_overlapping_clips;
pub use probe::probe_video;
pub use share::export_for_discord;
pub use thumbnail::generate_thumbnail;
pub use trim::trim_lossless;
