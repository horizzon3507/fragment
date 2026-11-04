use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    pub artist: Option<String>,
    pub duration: Duration,
}
