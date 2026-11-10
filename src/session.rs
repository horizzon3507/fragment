use std::time::Duration;

use anyhow::Result;
use rand::seq::SliceRandom;

use crate::config::Config;
use crate::game::Round;
use crate::player::Player;
use crate::scoring;
use crate::stats::Stats;
use crate::suggest::Candidates;
use crate::tui::guess::{Header, RoundEnd, play_round};
use crate::tui::{self, Tui};
use crate::types::Track;

/// One finished round as shown on the scoreboard.
pub struct RoundResult {
    track: Track,
    solved: bool,
    level_idx: usize,
    elapsed: Duration,
    score: u32,
}

impl RoundResult {
    pub fn new(track: Track, solved: bool, level_idx: usize, elapsed: Duration, score: u32) -> Self {
        Self {
            track,
            solved,
            level_idx,
            elapsed,
            score,
        }
    }

    pub fn track(&self) -> &Track {
        &self.track
    }

    pub fn solved(&self) -> bool {
        self.solved
    }

    pub fn level_idx(&self) -> usize {
        self.level_idx
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    pub fn score(&self) -> u32 {
        self.score
    }
}

/// A session of N rounds with streak tracking.
pub struct Session {
    results: Vec<RoundResult>,
    streak: u32,
    best_streak: u32,
    total_score: u32,
    rounds_total: usize,
    aborted: bool,
}

impl Session {
    pub fn new(rounds_total: usize) -> Self {
        Self {
            results: Vec::new(),
            streak: 0,
            best_streak: 0,
            total_score: 0,
            rounds_total,
            aborted: false,
        }
    }

    /// Score a finished round against the streak it started with, then
    /// fold it into the totals. Keeping the score computation here makes
    /// the streak-before-record ordering impossible to get wrong.
    pub fn record_round(
        &mut self,
        track: Track,
        solved: bool,
        level_idx: usize,
        elapsed: Duration,
    ) {
        let score = if solved {
            scoring::score(level_idx, elapsed, self.streak)
        } else {
            0
        };
        self.record(RoundResult::new(track, solved, level_idx, elapsed, score));
    }

    /// Fold a finished round into the totals and update the streak.
    pub fn record(&mut self, result: RoundResult) {
        self.total_score += result.score;
        if result.solved {
            self.streak += 1;
            self.best_streak = self.best_streak.max(self.streak);
        } else {
            self.streak = 0;
        }
        self.results.push(result);
    }

    pub fn total_score(&self) -> u32 {
        self.total_score
    }

    pub fn current_streak(&self) -> u32 {
        self.streak
    }

    pub fn best_streak(&self) -> u32 {
        self.best_streak
    }

    pub fn results(&self) -> &[RoundResult] {
        &self.results
    }

    pub fn rounds_total(&self) -> usize {
        self.rounds_total
    }

    pub fn rounds_played(&self) -> usize {
        self.results.len()
    }

    /// Mark the session as ended early via Ctrl+C.
    pub fn abort(&mut self) {
        self.aborted = true;
    }

    /// True when the session ended via Ctrl+C before all rounds played.
    pub fn aborted(&self) -> bool {
        self.aborted
    }
}

/// Run the whole session: terminal setup, the round loop and the final
/// scoreboard. `stats` is the lifetime record before this session; the
/// scoreboard merges it with the session's own numbers. Returns the
/// finished session for stats recording.
pub fn run(cfg: &Config, tracks: Vec<Track>, stats: &Stats) -> Result<Session> {
    if tracks.is_empty() {
        anyhow::bail!("no tracks to play");
    }
    // Spawn mpv before entering the TUI so a missing binary prints cleanly
    // on stderr.
    let player = Player::spawn()?;
    let mut term = tui::init()?;
    // Everything below runs in raw mode: capture the result, restore, then
    // propagate so the terminal always comes back.
    let result = play_rounds(cfg, &tracks, &player, &mut term, stats);
    tui::restore();
    result
}

/// The raw-mode part of [`run`]: pick tracks, play each round, show the
/// scoreboard.
fn play_rounds(
    cfg: &Config,
    tracks: &[Track],
    player: &Player,
    term: &mut Tui,
    stats: &Stats,
) -> Result<Session> {
    // Shuffled indices cycled round-robin: every round gets a distinct
    // track unless rounds outnumber the library.
    let mut order: Vec<usize> = (0..tracks.len()).collect();
    order.shuffle(&mut rand::rng());
    let cands = Candidates::build(tracks);

    let mut session = Session::new(cfg.rounds as usize);
    for i in 0..session.rounds_total() {
        let mut round = Round::new(tracks[order[i % order.len()]].clone());
        let header = Header {
            round: i + 1,
            total: session.rounds_total(),
            score: session.total_score(),
            streak: session.current_streak(),
        };
        match play_round(term, &mut round, player, &header, &cands)? {
            // Ctrl+C ends the session; the interrupted round is not recorded.
            (RoundEnd::Quit, _) => {
                session.abort();
                break;
            }
            // The elapsed returned here is frozen at reveal time, so dwell
            // on the reveal screen never leaks into scoring.
            (end, elapsed) => {
                let solved = end == RoundEnd::Solved;
                session.record_round(round.track().clone(), solved, round.level_idx(), elapsed);
            }
        }
    }
    // An aborted session quits straight away: no scoreboard.
    if !session.aborted() {
        // The board's all-time numbers should already include this session.
        let mut shown = stats.clone();
        shown.record(&session);
        tui::board::show(term, &session, &shown)?;
    }
    Ok(session)
}
