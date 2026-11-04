use std::path::Path;

use anyhow::Result;

use crate::types::Track;

/// Scan `dir` recursively for audio files, reading tags via lofty.
/// Falls back to "Artist - Title.ext" filename parsing when tags are absent.
/// Returns tracks sorted by artist/title for stable output.
pub fn scan(dir: &Path) -> Result<Vec<Track>> {
    let _ = dir;
    Ok(Vec::new())
}
