use std::borrow::Cow;
use std::path::Path;

use anyhow::{Context, Result};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::Accessor;
use walkdir::{DirEntry, WalkDir};

use crate::types::Track;

const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "ogg", "opus", "m4a", "mp4", "aac", "wav", "aiff", "ape", "wv",
];

/// Scan `dir` recursively for audio files, reading tags via lofty.
/// Falls back to "Artist - Title.ext" filename parsing when tags are absent.
/// Returns tracks sorted by artist/title for stable output.
pub fn scan(dir: &Path) -> Result<Vec<Track>> {
    if !dir.is_dir() {
        anyhow::bail!("'{}' does not exist or is not a directory", dir.display());
    }

    // Canonicalize so track paths are absolute: mpv reports the resolved
    // path on `path`, and a relative path here would never match.
    let dir = dir
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", dir.display()))?;

    let mut tracks = Vec::new();
    for entry in WalkDir::new(&dir)
        .follow_links(true)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !is_hidden(e))
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        // Non-UTF-8 paths are skipped: mpv can't be sent them over IPC.
        if !entry.file_type().is_file() || !has_audio_extension(path) || path.to_str().is_none() {
            continue;
        }
        if let Some(track) = read_track(path) {
            tracks.push(track);
        }
    }

    tracks.sort_by_cached_key(|t| {
        (
            t.artist.as_deref().unwrap_or_default().to_lowercase(),
            t.title.to_lowercase(),
        )
    });
    Ok(tracks)
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry.file_name().to_string_lossy().starts_with('.')
}

fn has_audio_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO_EXTENSIONS.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

fn read_track(path: &Path) -> Option<Track> {
    let tagged = Probe::open(path).ok()?.read().ok()?;
    let duration = tagged.properties().duration();

    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    let tag_title = tag.and_then(|t| cleaned_tag_value(t.title()));
    let tag_artist = tag.and_then(|t| cleaned_tag_value(t.artist()));

    let (title, artist) = match tag_title {
        Some(title) => (title, tag_artist),
        None => {
            let (file_artist, file_title) = parse_stem(&stem(path));
            (file_title, tag_artist.or(file_artist))
        }
    };

    Some(Track {
        path: path.to_path_buf(),
        title,
        artist,
        duration,
    })
}

fn cleaned_tag_value(value: Option<Cow<'_, str>>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn stem(path: &Path) -> Cow<'_, str> {
    path.file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default()
}

fn parse_stem(stem: &str) -> (Option<String>, String) {
    let cleaned = strip_trailing_brackets(stem);
    let base = if cleaned.is_empty() {
        stem.trim()
    } else {
        cleaned
    };
    match base
        .split_once(" - ")
        .map(|(a, t)| (a.trim(), t.trim()))
        .filter(|(a, t)| !a.is_empty() && !t.is_empty())
    {
        Some((artist, title)) => (Some(artist.to_string()), title.to_string()),
        None => (None, base.to_string()),
    }
}

fn strip_trailing_brackets(stem: &str) -> &str {
    let mut s = stem.trim();
    loop {
        let open = if s.ends_with(')') {
            '('
        } else if s.ends_with(']') {
            '['
        } else {
            break;
        };
        match s.rfind(open) {
            Some(i) => s = s[..i].trim_end(),
            None => break,
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::{parse_stem, strip_trailing_brackets};

    #[test]
    fn parse_stem_splits_artist_dash_title() {
        assert_eq!(
            parse_stem("Artist - Title"),
            (Some("Artist".to_string()), "Title".to_string())
        );
    }

    #[test]
    fn parse_stem_strips_chained_trailing_brackets() {
        assert_eq!(
            parse_stem("Artist - Title (Live) [Remaster]"),
            (Some("Artist".to_string()), "Title".to_string())
        );
        assert_eq!(strip_trailing_brackets("Song [a] (b) [c]"), "Song");
    }

    #[test]
    fn parse_stem_without_separator_has_no_artist() {
        assert_eq!(
            parse_stem("Just a Title"),
            (None, "Just a Title".to_string())
        );
    }

    #[test]
    fn parse_stem_trailing_separator_keeps_whole_stem() {
        assert_eq!(parse_stem("Artist - "), (None, "Artist -".to_string()));
    }

    #[test]
    fn parse_stem_all_brackets_falls_back_to_raw_stem() {
        assert_eq!(parse_stem("(Live)"), (None, "(Live)".to_string()));
    }

    #[test]
    fn strip_trailing_brackets_leaves_unbalanced_alone() {
        assert_eq!(strip_trailing_brackets("Song (Live"), "Song (Live");
        assert_eq!(strip_trailing_brackets("Song )"), "Song )");
        assert_eq!(strip_trailing_brackets("(only)"), "");
    }
}
