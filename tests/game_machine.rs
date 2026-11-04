// Escalation state machine coverage — implemented in wave 1.
use std::path::PathBuf;
use std::time::Duration;

use fragment::game::{GuessOutcome, LEVELS, Round};
use fragment::types::Track;

fn track(title: &str, artist: Option<&str>, secs: u64) -> Track {
    Track {
        path: PathBuf::from("/music/placeholder.flac"),
        title: title.to_string(),
        artist: artist.map(str::to_string),
        duration: Duration::from_secs(secs),
    }
}

fn round() -> Round {
    Round::with_offset(
        track("Bohemian Rhapsody", Some("Queen"), 354),
        Duration::from_secs(30),
    )
}

#[test]
fn fresh_round_starts_unsolved_at_first_level() {
    let r = round();
    assert_eq!(r.level_idx(), 0);
    assert_eq!(r.level_secs(), 1);
    assert!(!r.is_solved());
    assert!(!r.is_exhausted());
    assert!(r.misses().is_empty());
    assert_eq!(r.offset(), Duration::from_secs(30));
    assert_eq!(r.track().title, "Bohemian Rhapsody");
}

#[test]
fn escalate_walks_the_snippet_ladder() {
    assert_eq!(LEVELS, [1, 2, 4, 8, 16, 30]);
    let mut r = round();
    for (idx, &secs) in LEVELS.iter().enumerate().skip(1) {
        r.escalate();
        assert_eq!(r.level_idx(), idx);
        assert_eq!(r.level_secs(), secs);
    }
    assert_eq!(r.level_secs(), 30);
    assert!(!r.is_exhausted());
}

#[test]
fn escalating_past_final_level_exhausts_round() {
    let mut r = round();
    for _ in 0..LEVELS.len() - 1 {
        r.escalate();
    }
    assert_eq!(r.level_secs(), 30);
    assert!(!r.is_exhausted());

    r.escalate();
    assert!(r.is_exhausted());
    assert_eq!(r.level_idx(), LEVELS.len() - 1);
    assert_eq!(r.level_secs(), 30);

    // Escalating an exhausted round is a no-op, still pinned at 30s.
    r.escalate();
    assert!(r.is_exhausted());
    assert_eq!(r.level_secs(), 30);
}

#[test]
fn wrong_submit_records_miss_without_escalating() {
    let mut r = round();
    assert_eq!(r.submit("stairway to heaven"), GuessOutcome::Wrong);
    assert_eq!(r.submit("hotel california"), GuessOutcome::Wrong);

    assert_eq!(r.misses(), ["stairway to heaven", "hotel california"]);
    assert_eq!(r.level_idx(), 0);
    assert_eq!(r.level_secs(), 1);
    assert!(!r.is_solved());
    assert!(!r.is_exhausted());
}

#[test]
fn wrong_submit_on_final_level_exhausts_round() {
    let mut r = round();
    for _ in 0..LEVELS.len() - 1 {
        r.escalate();
    }
    assert_eq!(r.level_secs(), 30);

    assert_eq!(r.submit("not the song"), GuessOutcome::Wrong);
    assert!(r.is_exhausted());
    assert_eq!(r.misses(), ["not the song"]);

    // An exhausted round stops recording misses.
    assert_eq!(r.submit("still not the song"), GuessOutcome::Wrong);
    assert_eq!(r.misses(), ["not the song"]);
    assert_eq!(r.level_secs(), 30);
}

#[test]
fn correct_submit_solves_round_and_stays_solved() {
    let mut r = round();
    assert_eq!(r.submit("bohemian rhapsody"), GuessOutcome::Correct);
    assert!(r.is_solved());
    assert!(r.misses().is_empty());

    // A later wrong guess can't un-solve the round and isn't recorded.
    assert_eq!(r.submit("stairway to heaven"), GuessOutcome::Wrong);
    assert!(r.is_solved());
    assert!(r.misses().is_empty());

    // Escalation is a no-op on a solved round.
    r.escalate();
    assert_eq!(r.level_secs(), 1);
    assert!(!r.is_exhausted());
}

#[test]
fn blank_guess_is_wrong_but_not_recorded() {
    let mut r = round();
    assert_eq!(r.submit(""), GuessOutcome::Wrong);
    assert_eq!(r.submit("   "), GuessOutcome::Wrong);
    assert_eq!(r.submit(" \t\n "), GuessOutcome::Wrong);

    assert!(r.misses().is_empty());
    assert!(!r.is_solved());
    assert!(!r.is_exhausted());
    assert_eq!(r.level_idx(), 0);
}

#[test]
fn elapsed_is_monotonic_and_non_negative() {
    let r = round();
    let first = r.elapsed();
    std::thread::sleep(Duration::from_millis(2));
    let second = r.elapsed();
    assert!(second >= first);
    assert!(second >= Duration::ZERO);
}

#[test]
fn new_round_offsets_stay_inside_the_track() {
    // Round::new reserves tail room, so on a normal-length track the random
    // offset always lands strictly before the end.
    for _ in 0..25 {
        let r = Round::new(track("Any Track", None, 120));
        assert!(r.offset() < r.track().duration);
    }
}

#[test]
fn with_offset_clamps_to_one_second_before_the_end() {
    let r = Round::with_offset(track("Any Track", None, 120), Duration::from_secs(200));
    assert_eq!(r.offset(), Duration::from_secs(119));

    // On a track shorter than a second the clamp bottoms out at zero.
    let r = Round::with_offset(track("Tiny", None, 0), Duration::from_secs(200));
    assert_eq!(r.offset(), Duration::ZERO);
}

#[test]
fn new_round_on_tiny_tracks_stays_in_bounds() {
    for secs in [0, 5] {
        let r = Round::new(track("Tiny", None, secs));
        assert!(r.offset() <= r.track().duration);
    }
}
