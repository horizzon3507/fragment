// Guess normalization and fuzzy matching coverage — implemented in wave 1.
use std::path::PathBuf;
use std::time::Duration;

use fragment::game::{matches, normalize};
use fragment::types::Track;

fn track(title: &str, artist: Option<&str>, secs: u64) -> Track {
    Track {
        path: PathBuf::from("/music/placeholder.flac"),
        title: title.to_string(),
        artist: artist.map(str::to_string),
        duration: Duration::from_secs(secs),
    }
}

#[test]
fn normalize_strips_bracketed_segments() {
    assert_eq!(normalize("Song Title (feat. Someone)"), "song title");
    assert_eq!(normalize("Track [Remaster]"), "track");
    assert_eq!(normalize("Live (Remastered) [Deluxe Edition]"), "live");
}

#[test]
fn normalize_drops_accents_and_punctuation() {
    assert_eq!(normalize("Beyoncé"), "beyonce");
    // Punctuation becomes a space rather than vanishing outright.
    assert_eq!(normalize("Don't Stop!"), "don t stop");
    assert_eq!(normalize("AC/DC"), "ac dc");
}

#[test]
fn normalize_lowercases_and_collapses_whitespace() {
    assert_eq!(normalize("  Mixed   CASE Title  "), "mixed case title");
    assert_eq!(normalize("tabs\tand\nnewlines"), "tabs and newlines");
}

#[test]
fn exact_title_guess_matches() {
    let t = track("Bohemian Rhapsody", Some("Queen"), 354);
    assert!(matches("Bohemian Rhapsody", &t));
}

#[test]
fn matching_is_case_and_accent_insensitive() {
    let t = track("Beyoncé", None, 200);
    assert!(matches("beyonce", &t));
    assert!(matches("BEYONCÉ", &t));

    let t = track("Bohemian Rhapsody", Some("Queen"), 354);
    assert!(matches("bOhEmIaN rHaPsOdY", &t));
}

#[test]
fn featuring_suffix_is_ignored_on_either_side() {
    let t = track("Song Title (feat. Someone)", Some("Artist"), 200);
    assert!(matches("song title", &t));

    let t = track("Song Title", None, 200);
    assert!(matches("Song Title (feat. Someone)", &t));
}

#[test]
fn near_miss_above_threshold_matches() {
    // "bohemian rhapsod" sits around 0.99 Jaro-Winkler against the title,
    // comfortably above the 0.85 cutoff.
    let t = track("Bohemian Rhapsody", Some("Queen"), 354);
    assert!(matches("bohemian rhapsod", &t));
}

#[test]
fn clearly_different_title_fails() {
    let t = track("Bohemian Rhapsody", Some("Queen"), 354);
    assert!(!matches("Stairway to Heaven", &t));
    assert!(!matches("xylophone thunder pants", &t));
}

#[test]
fn artist_title_combination_matches() {
    let t = track("Bohemian Rhapsody", Some("Queen"), 354);
    assert!(matches("queen bohemian rhapsody", &t));
    // The bare title remains a candidate when an artist is tagged.
    assert!(matches("bohemian rhapsody", &t));
}

#[test]
fn empty_guess_never_matches() {
    let t = track("Bohemian Rhapsody", Some("Queen"), 354);
    assert!(!matches("", &t));
    assert!(!matches("   ", &t));
    assert!(!matches("(((", &t));
}

#[test]
fn artist_alone_does_not_match() {
    // Candidates are the title and "artist title" — never the artist alone.
    let t = track("Bohemian Rhapsody", Some("Queen"), 354);
    assert!(!matches("queen", &t));
}

#[test]
fn title_with_bracketed_segment_matches_clean_guess() {
    let t = track("X (Live)", None, 200);
    assert!(matches("x", &t));

    let t = track("Song Title (Live) [Remaster]", Some("Artist"), 200);
    assert!(matches("song title", &t));
}
