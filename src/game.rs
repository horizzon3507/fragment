use std::time::{Duration, Instant};

use rand::RngExt;
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

use crate::types::Track;

/// Snippet lengths in seconds, escalating per level.
pub const LEVELS: [u64; 6] = [1, 2, 4, 8, 16, 30];

const LAST_LEVEL: usize = LEVELS.len() - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuessOutcome {
    Correct,
    Wrong,
}

/// One guessing round: a track, a random start offset and the current
/// escalation level.
pub struct Round {
    track: Track,
    offset: Duration,
    level_idx: usize,
    solved: bool,
    exhausted: bool,
    misses: Vec<String>,
    started: Instant,
}

impl Round {
    /// New round with a random start offset inside the track.
    pub fn new(track: Track) -> Self {
        // Reserve room for the longest snippet, capped at a third of the
        // track so short tracks still leave a playable offset range.
        let tail = Duration::from_secs(LEVELS[LAST_LEVEL] + 2).min(track.duration / 3);
        let max = track.duration.saturating_sub(tail);
        let offset = if max.is_zero() {
            Duration::ZERO
        } else {
            let millis = u64::try_from(max.as_millis()).unwrap_or(u64::MAX);
            Duration::from_millis(rand::rng().random_range(0..=millis))
        };
        Self::at(track, offset)
    }

    /// New round with a fixed offset (deterministic; used by tests).
    pub fn with_offset(track: Track, offset: Duration) -> Self {
        let offset = offset.min(track.duration.saturating_sub(Duration::from_secs(1)));
        Self::at(track, offset)
    }

    fn at(track: Track, offset: Duration) -> Self {
        Self {
            track,
            offset,
            level_idx: 0,
            solved: false,
            exhausted: false,
            misses: Vec::new(),
            started: Instant::now(),
        }
    }

    /// Submit a guess. Correct guesses solve the round; wrong guesses are
    /// recorded as misses and, on the final level, exhaust the round.
    /// Blank guesses are ignored and a finished round accepts nothing.
    /// Escalating the level after a wrong guess is the caller's job.
    pub fn submit(&mut self, guess: &str) -> GuessOutcome {
        if self.solved || self.exhausted {
            return GuessOutcome::Wrong;
        }
        if matches(guess, &self.track) {
            self.solved = true;
            return GuessOutcome::Correct;
        }
        if guess.trim().is_empty() {
            return GuessOutcome::Wrong;
        }
        self.misses.push(guess.to_string());
        if self.level_idx == LAST_LEVEL {
            self.exhausted = true;
        }
        GuessOutcome::Wrong
    }

    /// Grow the snippet to the next level. On the final level this marks
    /// the round exhausted instead.
    pub fn escalate(&mut self) {
        if self.solved {
            return;
        }
        if self.level_idx == LAST_LEVEL {
            self.exhausted = true;
        } else {
            self.level_idx += 1;
        }
    }

    /// Current snippet length in seconds.
    pub fn level_secs(&self) -> u64 {
        LEVELS[self.level_idx]
    }

    /// Current index into [`LEVELS`].
    pub fn level_idx(&self) -> usize {
        self.level_idx
    }

    /// True once the final level has been consumed without a correct guess.
    pub fn is_exhausted(&self) -> bool {
        self.exhausted
    }

    /// True once the player guessed correctly.
    pub fn is_solved(&self) -> bool {
        self.solved
    }

    /// Elapsed time since the round started.
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    pub fn track(&self) -> &Track {
        &self.track
    }

    /// Start offset of the snippet inside the track.
    pub fn offset(&self) -> Duration {
        self.offset
    }

    /// Wrong guesses submitted so far, in order.
    pub fn misses(&self) -> &[String] {
        &self.misses
    }
}

/// Normalize a title or guess for comparison: lowercase, strip bracketed
/// segments such as "(feat. ...)" or "[remaster]", drop punctuation and
/// accents, collapse whitespace.
pub fn normalize(input: &str) -> String {
    let mut stripped = String::with_capacity(input.len());
    let mut depth = 0u32;
    for ch in input.chars() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => stripped.push(ch),
            _ => {}
        }
    }

    let mut out = String::with_capacity(stripped.len());
    for ch in stripped.nfkd() {
        if is_combining_mark(ch) {
            continue;
        }
        for lower in ch.to_lowercase() {
            if lower.is_alphanumeric() {
                out.push(lower);
            } else {
                out.push(' ');
            }
        }
    }

    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// True when `guess` matches the track: after normalization, an exact match
/// or Jaro-Winkler >= 0.85 against the title or the combined
/// "artist title" string.
pub fn matches(guess: &str, track: &Track) -> bool {
    let guess = normalize(guess);
    if guess.is_empty() {
        return false;
    }

    let mut candidates = vec![normalize(&track.title)];
    if let Some(artist) = &track.artist {
        candidates.push(normalize(&format!("{artist} {}", track.title)));
    }

    candidates.iter().any(|cand| {
        !cand.is_empty() && (guess == *cand || strsim::jaro_winkler(&guess, cand) >= 0.85)
    })
}
