use std::time::{Duration, Instant};

use crate::types::Track;

/// Snippet lengths in seconds, escalating per level.
pub const LEVELS: [u64; 6] = [1, 2, 4, 8, 16, 30];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuessOutcome {
    Correct,
    Wrong,
}

/// One guessing round: a track, a random start offset and the current
/// escalation level.
pub struct Round;

impl Round {
    /// New round with a random start offset inside the track.
    pub fn new(track: Track) -> Self {
        let _ = track;
        Self
    }

    /// New round with a fixed offset (deterministic; used by tests).
    pub fn with_offset(track: Track, offset: Duration) -> Self {
        let _ = (track, offset);
        Self
    }

    /// Submit a guess. Correct guesses solve the round; wrong guesses are
    /// recorded as misses and consume the current level (a wrong guess on
    /// the final level exhausts the round).
    pub fn submit(&mut self, guess: &str) -> GuessOutcome {
        let _ = guess;
        GuessOutcome::Wrong
    }

    /// Grow the snippet to the next level. On the final level this marks
    /// the round exhausted instead.
    pub fn escalate(&mut self) {}

    /// Current snippet length in seconds.
    pub fn level_secs(&self) -> u64 {
        LEVELS[0]
    }

    /// Current index into [`LEVELS`].
    pub fn level_idx(&self) -> usize {
        0
    }

    /// True once the final level has been consumed without a correct guess.
    pub fn is_exhausted(&self) -> bool {
        false
    }

    /// True once the player guessed correctly.
    pub fn is_solved(&self) -> bool {
        false
    }

    /// Elapsed time since the round started.
    pub fn elapsed(&self) -> Duration {
        let _ = Instant::now();
        Duration::ZERO
    }

    pub fn track(&self) -> &Track {
        unreachable!("implemented in wave 1")
    }

    /// Start offset of the snippet inside the track.
    pub fn offset(&self) -> Duration {
        Duration::ZERO
    }

    /// Wrong guesses submitted so far, in order.
    pub fn misses(&self) -> &[String] {
        &[]
    }
}

/// Normalize a title or guess for comparison: lowercase, strip bracketed
/// segments such as "(feat. ...)" or "[remaster]", drop punctuation and
/// accents, collapse whitespace.
pub fn normalize(input: &str) -> String {
    input.to_lowercase()
}

/// True when `guess` matches the track: after normalization, an exact match
/// or Jaro-Winkler >= 0.85 against the title or the combined
/// "artist title" string.
pub fn matches(guess: &str, track: &Track) -> bool {
    let _ = (guess, track);
    false
}
