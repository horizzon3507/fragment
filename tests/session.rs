// Session and streak accumulation coverage — implemented in wave 2.
use std::path::PathBuf;
use std::time::Duration;

use fragment::session::{RoundResult, Session};
use fragment::types::Track;

fn track(title: &str, artist: Option<&str>, secs: u64) -> Track {
    Track {
        path: PathBuf::from("/music/placeholder.flac"),
        title: title.to_string(),
        artist: artist.map(str::to_string),
        duration: Duration::from_secs(secs),
    }
}

fn result(title: &str, solved: bool, level_idx: usize, secs: u64, score: u32) -> RoundResult {
    RoundResult::new(
        track(title, Some("Some Artist"), 200),
        solved,
        level_idx,
        Duration::from_secs(secs),
        score,
    )
}

#[test]
fn fresh_session_is_empty_and_not_aborted() {
    let session = Session::new(3);
    assert_eq!(session.rounds_total(), 3);
    assert_eq!(session.rounds_played(), 0);
    assert!(session.results().is_empty());
    assert_eq!(session.total_score(), 0);
    assert_eq!(session.current_streak(), 0);
    assert_eq!(session.best_streak(), 0);
    assert!(!session.aborted());
}

#[test]
fn round_result_getters_round_trip() {
    let r = result("Bohemian Rhapsody", true, 2, 7, 640);
    assert!(r.solved());
    assert_eq!(r.level_idx(), 2);
    assert_eq!(r.elapsed(), Duration::from_secs(7));
    assert_eq!(r.score(), 640);
    assert_eq!(r.track().title, "Bohemian Rhapsody");
}

#[test]
fn recording_solved_rounds_builds_the_streak() {
    let mut session = Session::new(3);
    session.record(result("Song A", true, 0, 1, 500));
    assert_eq!(session.total_score(), 500);
    assert_eq!(session.current_streak(), 1);
    assert_eq!(session.best_streak(), 1);
    assert_eq!(session.rounds_played(), 1);

    session.record(result("Song B", true, 0, 1, 300));
    assert_eq!(session.total_score(), 800);
    assert_eq!(session.current_streak(), 2);
    assert_eq!(session.best_streak(), 2);
    assert_eq!(session.rounds_played(), 2);
}

#[test]
fn unsolved_round_resets_streak_but_keeps_best_and_total() {
    let mut session = Session::new(3);
    session.record(result("Song A", true, 0, 1, 500));
    session.record(result("Song B", true, 0, 1, 300));
    session.record(result("Song C", false, 5, 30, 0));

    assert_eq!(session.current_streak(), 0);
    assert_eq!(session.best_streak(), 2);
    assert_eq!(session.total_score(), 800);
    assert_eq!(session.rounds_played(), 3);
}

#[test]
fn results_preserve_recording_order() {
    let mut session = Session::new(3);
    session.record(result("First", true, 0, 1, 500));
    session.record(result("Second", false, 5, 30, 0));
    session.record(result("Third", true, 1, 4, 700));

    let results = session.results();
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].track().title, "First");
    assert_eq!(results[1].track().title, "Second");
    assert_eq!(results[2].track().title, "Third");
    assert!(results[0].solved());
    assert!(!results[1].solved());
    assert!(results[2].solved());
}

#[test]
fn full_session_arc_tracks_totals_and_best_streak() {
    let mut session = Session::new(4);
    session.record(result("W1", true, 0, 2, 900));
    session.record(result("W2", true, 1, 3, 700));
    session.record(result("L", false, 5, 30, 0));
    session.record(result("W3", true, 2, 5, 500));

    assert_eq!(session.rounds_played(), 4);
    assert_eq!(session.total_score(), 2100);
    assert_eq!(session.current_streak(), 1);
    assert_eq!(session.best_streak(), 2);
    assert!(!session.aborted());
}

#[test]
fn record_round_scores_with_the_pre_round_streak() {
    let mut session = Session::new(3);
    session.record_round(track("Song A", Some("Some Artist"), 200), true, 0, Duration::ZERO);
    // First solve scores with streak 0: level-1 base × 1.0.
    assert_eq!(
        session.results()[0].score(),
        fragment::scoring::score(0, Duration::ZERO, 0)
    );
    assert_eq!(session.results()[0].score(), 1000);

    session.record_round(track("Song B", Some("Some Artist"), 200), true, 0, Duration::ZERO);
    // Second solve scores with streak 1: ×1.1, so the multiplier uses
    // the streak the round started with, not the streak after record.
    assert_eq!(session.results()[1].score(), 1100);
    assert_eq!(session.total_score(), 2100);
}

#[test]
fn abort_marks_the_session_as_aborted() {
    let mut session = Session::new(3);
    session.record(result("Song A", true, 0, 1, 500));
    assert!(!session.aborted());
    session.abort();
    assert!(session.aborted());
    // Recorded rounds and totals survive the abort.
    assert_eq!(session.rounds_played(), 1);
    assert_eq!(session.total_score(), 500);
}
