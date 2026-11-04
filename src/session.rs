use std::time::Duration;

use anyhow::Result;

use crate::config::Config;
use crate::types::Track;

/// One finished round as shown on the scoreboard.
pub struct RoundResult;

impl RoundResult {
    pub fn new(track: Track, solved: bool, level_idx: usize, elapsed: Duration, score: u32) -> Self {
        let _ = (track, solved, level_idx, elapsed, score);
        Self
    }

    pub fn track(&self) -> &Track {
        unreachable!("implemented in wave 2")
    }

    pub fn solved(&self) -> bool {
        false
    }

    pub fn level_idx(&self) -> usize {
        0
    }

    pub fn elapsed(&self) -> Duration {
        Duration::ZERO
    }

    pub fn score(&self) -> u32 {
        0
    }
}

/// A session of N rounds with streak tracking.
pub struct Session;

impl Session {
    pub fn new(rounds_total: usize) -> Self {
        let _ = rounds_total;
        Self
    }

    /// Fold a finished round into the totals and update the streak.
    pub fn record(&mut self, result: RoundResult) {
        let _ = result;
    }

    pub fn total_score(&self) -> u32 {
        0
    }

    pub fn current_streak(&self) -> u32 {
        0
    }

    pub fn best_streak(&self) -> u32 {
        0
    }

    pub fn results(&self) -> &[RoundResult] {
        &[]
    }

    pub fn rounds_total(&self) -> usize {
        0
    }

    pub fn rounds_played(&self) -> usize {
        0
    }
}

/// Run the whole session: terminal setup, the round loop and the final
/// scoreboard. Returns the finished session for stats recording.
pub fn run(cfg: &Config, tracks: Vec<Track>) -> Result<Session> {
    let _ = (cfg, tracks);
    anyhow::bail!("implemented in wave 2")
}
