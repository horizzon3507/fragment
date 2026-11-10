use std::collections::HashSet;

use crate::game::normalize;
use crate::types::Track;

/// A deduplicated "Artist - Title" entry shown in the guess screen's
/// autocomplete dropdown.
pub struct Candidate {
    /// What the dropdown shows.
    pub display: String,
    /// What accepting the entry puts into the input: matches the
    /// "artist title" guess candidate after normalization.
    fill: String,
    /// Normalized title only.
    title_key: String,
    /// Normalized "artist title" (same as `title_key` when no artist).
    full_key: String,
}

/// The whole library indexed for autocomplete. Built once per session;
/// matching is a normalized substring search, ranked prefix-first.
pub struct Candidates {
    entries: Vec<Candidate>,
}

impl Candidates {
    /// Index the library. Tracks that normalize to the same text collapse
    /// into one entry.
    pub fn build(tracks: &[Track]) -> Self {
        let mut seen = HashSet::new();
        let mut entries = Vec::with_capacity(tracks.len());
        for track in tracks {
            let display = match &track.artist {
                Some(artist) => format!("{artist} - {}", track.title),
                None => track.title.clone(),
            };
            let title_key = normalize(&track.title);
            let full_key = match &track.artist {
                Some(artist) => normalize(&format!("{artist} {}", track.title)),
                None => title_key.clone(),
            };
            if full_key.is_empty() || !seen.insert(full_key.clone()) {
                continue;
            }
            let fill = display.clone();
            entries.push(Candidate {
                display,
                fill,
                title_key,
                full_key,
            });
        }
        Self { entries }
    }

    /// Up to `limit` entries whose title or artist+title contains the
    /// normalized input. Ranked: title-prefix, then full-string prefix,
    /// then substring, each group shortest-title-first. An empty input
    /// matches nothing.
    pub fn search(&self, input: &str, limit: usize) -> Vec<&Candidate> {
        let query = normalize(input);
        if query.is_empty() {
            return Vec::new();
        }
        let mut hits: Vec<(u8, &Candidate)> = self
            .entries
            .iter()
            .filter_map(|entry| {
                let rank = if entry.title_key.starts_with(&query) {
                    0
                } else if entry.full_key.starts_with(&query) {
                    1
                } else if entry.title_key.contains(&query) || entry.full_key.contains(&query) {
                    2
                } else {
                    return None;
                };
                Some((rank, entry))
            })
            .collect();
        hits.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(a.1.title_key.len().cmp(&b.1.title_key.len()))
                .then_with(|| a.1.title_key.cmp(&b.1.title_key))
        });
        hits.truncate(limit);
        hits.into_iter().map(|(_, entry)| entry).collect()
    }
}

impl Candidate {
    /// Text inserted into the input when the entry is accepted.
    pub fn fill(&self) -> &str {
        &self.fill
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use super::Candidates;
    use crate::types::Track;

    fn track(title: &str, artist: Option<&str>) -> Track {
        Track {
            path: PathBuf::from(format!("/music/{title}.mp3")),
            title: title.to_string(),
            artist: artist.map(str::to_string),
            duration: Duration::from_secs(200),
        }
    }

    fn build(tracks: &[Track]) -> Candidates {
        Candidates::build(tracks)
    }

    #[test]
    fn empty_input_matches_nothing() {
        let cands = build(&[track("Skyfall", Some("Adele"))]);
        assert!(cands.search("", 5).is_empty());
        assert!(cands.search("   ", 5).is_empty());
    }

    #[test]
    fn matches_title_substring_case_and_accents() {
        let cands = build(&[
            track("Skyfall", Some("Adele")),
            track("Café del Mar", Some("Energy 52")),
            track("Blue Monday", Some("New Order")),
        ]);
        let hits: Vec<&str> = cands.search("SKYF", 5).iter().map(|c| c.display.as_str()).collect();
        assert_eq!(hits, ["Adele - Skyfall"]);
        // Accents and case fold away on both sides.
        let hits: Vec<&str> = cands.search("cafe", 5).iter().map(|c| c.display.as_str()).collect();
        assert_eq!(hits, ["Energy 52 - Café del Mar"]);
    }

    #[test]
    fn matches_via_artist_then_title() {
        let cands = build(&[
            track("Skyfall", Some("Adele")),
            track("Blue Monday", Some("New Order")),
        ]);
        let hits: Vec<&str> = cands.search("adele sky", 5).iter().map(|c| c.display.as_str()).collect();
        assert_eq!(hits, ["Adele - Skyfall"]);
    }

    #[test]
    fn prefix_hits_rank_before_substring_hits() {
        let cands = build(&[
            track("Another Night", Some("A")),
            track("Nightcall", Some("B")),
            track("Nightingale", Some("C")),
        ]);
        let hits: Vec<&str> = cands.search("night", 5).iter().map(|c| c.display.as_str()).collect();
        assert_eq!(hits[0], "B - Nightcall");
        assert_eq!(hits[1], "C - Nightingale");
        assert_eq!(hits[2], "A - Another Night");
    }

    #[test]
    fn same_normalized_entry_dedupes() {
        let cands = build(&[
            track("Skyfall", Some("Adele")),
            track("skyfall", Some("adele")),
            track("Skyfall (Live)", Some("Adele")),
        ]);
        // "(Live)" normalizes away: all three collide into one entry.
        assert_eq!(cands.search("sky", 5).len(), 1);
    }

    #[test]
    fn search_respects_the_limit() {
        let tracks: Vec<Track> = (0..10).map(|i| track(&format!("Song{i}"), None)).collect();
        let cands = build(&tracks);
        assert_eq!(cands.search("song", 3).len(), 3);
    }

    #[test]
    fn fill_is_the_display_string() {
        let cands = build(&[track("Skyfall", Some("Adele"))]);
        let hit = cands.search("sky", 5)[0];
        assert_eq!(hit.fill(), "Adele - Skyfall");
    }
}
