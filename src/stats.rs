use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::game::LEVELS;
use crate::session::Session;

/// How many recent per-game totals are kept for the score history.
const RECENT_SCORES_CAP: usize = 30;

/// Lifetime stats persisted to `$XDG_DATA_HOME/fragment/stats.json`
/// (~/.local/share/fragment/stats.json).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Stats {
    pub games: u64,
    pub rounds_won: u64,
    pub rounds_played: u64,
    pub best_streak: u32,
    /// Average seconds-to-guess across solved rounds.
    pub avg_guess_time: f64,
    /// Wins per escalation level, indexed like [`LEVELS`].
    pub wins_by_level: [u64; LEVELS.len()],
    /// Total score of the most recent games, oldest first.
    pub recent_scores: Vec<u32>,
}

impl Stats {
    /// Load stats; a missing or unreadable file yields defaults. A file
    /// that fails to parse is renamed to `stats.json.bak` and also
    /// yields defaults, so a corrupt state never fails or loses data.
    pub fn load() -> Result<Self> {
        let Some(path) = path() else {
            return Ok(Self::default());
        };
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(_) => return Ok(Self::default()),
        };
        match serde_json::from_str(&text) {
            Ok(stats) => Ok(stats),
            Err(_) => {
                backup(&path);
                Ok(Self::default())
            }
        }
    }

    /// Persist stats atomically: write `stats.json.tmp` in the data dir,
    /// then rename over the target so readers never see a partial file.
    pub fn save(&self) -> Result<()> {
        let Some(path) = path() else {
            anyhow::bail!("failed to resolve stats data directory");
        };
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| {
                format!("failed to create stats directory {}", dir.display())
            })?;
        }
        let text = serde_json::to_string_pretty(self).context("failed to serialize stats")?;
        let tmp = path.with_extension("json.tmp");
        // fs::write is create + write_all + close, so the temp file is
        // flushed to the kernel before the rename.
        fs::write(&tmp, text)
            .with_context(|| format!("failed to write stats file {}", tmp.display()))?;
        fs::rename(&tmp, &path)
            .with_context(|| format!("failed to replace stats file {}", path.display()))?;
        Ok(())
    }

    /// Fold one finished session into the running totals.
    pub fn record(&mut self, session: &Session) {
        let prev_won = self.rounds_won;
        self.games += 1;
        self.rounds_played += session.results().len() as u64;

        let mut new_won = 0u64;
        let mut new_secs = 0.0;
        for result in session.results() {
            if result.solved() {
                new_won += 1;
                new_secs += result.elapsed().as_secs_f64();
                let level = result.level_idx().min(LEVELS.len() - 1);
                self.wins_by_level[level] += 1;
            }
        }
        self.rounds_won += new_won;
        self.best_streak = self.best_streak.max(session.best_streak());

        self.recent_scores.push(session.total_score());
        let overflow = self.recent_scores.len().saturating_sub(RECENT_SCORES_CAP);
        if overflow > 0 {
            self.recent_scores.drain(..overflow);
        }

        // Running mean of solve times across every solved round ever.
        let total_won = prev_won + new_won;
        if total_won > 0 {
            self.avg_guess_time =
                (self.avg_guess_time * prev_won as f64 + new_secs) / total_won as f64;
        }
    }

    /// Print lifetime stats to stdout (`fragment --stats`).
    pub fn print(&self) {
        println!("fragment stats");
        if let Some(path) = path() {
            println!("{}", path.display());
        }
        println!();
        println!("games played      {}", self.games);
        let pct = if self.rounds_played > 0 {
            format!(" ({:.0}%)", self.rounds_won as f64 / self.rounds_played as f64 * 100.0)
        } else {
            String::new()
        };
        println!("rounds won        {}/{}{pct}", self.rounds_won, self.rounds_played);
        println!("best streak       {}", self.best_streak);
        if self.rounds_won > 0 {
            println!("avg guess time    {:.1}s", self.avg_guess_time);
        }

        if self.wins_by_level.iter().any(|w| *w > 0) {
            let max = self.wins_by_level.iter().copied().max().unwrap_or(1).max(1);
            println!("\nwins per level");
            for (secs, wins) in LEVELS.iter().zip(self.wins_by_level.iter()) {
                let width = (*wins * 30 / max) as usize;
                println!("  {secs:>2}s  {:<width$} {wins}", "█".repeat(width), width = 30);
            }
        }

        if !self.recent_scores.is_empty() {
            let scores: Vec<String> =
                self.recent_scores.iter().map(u32::to_string).collect();
            println!("\nrecent scores     {}", scores.join(" · "));
        }
    }
}

/// Path of the stats file: `$XDG_DATA_HOME/fragment/stats.json`.
/// `None` when the platform data directory cannot be resolved.
pub fn path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "fragment")
        .map(|dirs| dirs.data_dir().join("stats.json"))
}

/// Rename a bad stats file to `stats.json.bak`, best-effort.
fn backup(path: &Path) {
    let _ = fs::rename(path, path.with_extension("json.bak"));
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use crate::session::{RoundResult, Session};
    use crate::types::Track;

    use super::{Stats, path};

    fn result(solved: bool, secs: u64) -> RoundResult {
        RoundResult::new(
            Track {
                path: PathBuf::from("/music/placeholder.flac"),
                title: "Song".to_string(),
                artist: None,
                duration: Duration::from_secs(200),
            },
            solved,
            0,
            Duration::from_secs(secs),
            0,
        )
    }

    #[test]
    fn path_is_fragment_stats_json() {
        let Some(path) = path() else {
            return;
        };
        assert_eq!(path.file_name().unwrap(), "stats.json");
        assert_eq!(path.parent().unwrap().file_name().unwrap(), "fragment");
    }

    #[test]
    fn partial_json_deserializes_with_defaults() {
        let stats: Stats = serde_json::from_str(r#"{"games": 3, "unknown": true}"#).unwrap();
        assert_eq!(stats.games, 3);
        assert_eq!(stats.rounds_won, 0);
        assert_eq!(stats.rounds_played, 0);
        assert_eq!(stats.best_streak, 0);
        assert_eq!(stats.avg_guess_time, 0.0);
    }

    #[test]
    fn json_round_trip_preserves_every_field() {
        let stats = Stats {
            games: 4,
            rounds_won: 9,
            rounds_played: 12,
            best_streak: 3,
            avg_guess_time: 4.25,
            wins_by_level: [1, 2, 3, 2, 1, 0],
            recent_scores: vec![1200, 800],
        };
        let text = serde_json::to_string_pretty(&stats).unwrap();
        let back: Stats = serde_json::from_str(&text).unwrap();
        assert_eq!(back.games, 4);
        assert_eq!(back.rounds_won, 9);
        assert_eq!(back.rounds_played, 12);
        assert_eq!(back.best_streak, 3);
        assert_eq!(back.avg_guess_time, 4.25);
        assert_eq!(back.wins_by_level, [1, 2, 3, 2, 1, 0]);
        assert_eq!(back.recent_scores, vec![1200, 800]);
    }

    #[test]
    fn record_counts_wins_per_level_and_tracks_recent_scores() {
        let mut session = Session::new(3);
        // Solved at level idx 1, then unsolved, then solved at idx 0.
        session.record(RoundResult::new(
            Track {
                path: PathBuf::from("/music/a.flac"),
                title: "A".to_string(),
                artist: None,
                duration: Duration::from_secs(200),
            },
            true,
            1,
            Duration::from_secs(3),
            500,
        ));
        session.record(result(true, 5));
        session.record(result(false, 30));

        let mut stats = Stats::default();
        stats.record(&session);

        assert_eq!(stats.wins_by_level[0], 1);
        assert_eq!(stats.wins_by_level[1], 1);
        assert_eq!(stats.wins_by_level.iter().sum::<u64>(), 2);
        assert_eq!(stats.recent_scores.len(), 1);
    }

    #[test]
    fn recent_scores_is_capped() {
        let mut stats = Stats::default();
        for _ in 0..40 {
            stats.recent_scores.push(100);
            let overflow = stats.recent_scores.len().saturating_sub(30);
            stats.recent_scores.drain(..overflow);
        }
        assert_eq!(stats.recent_scores.len(), 30);
    }

    #[test]
    fn record_folds_a_session_into_the_totals() {
        let mut session = Session::new(3);
        session.record(result(true, 2));
        session.record(result(true, 4));
        session.record(result(false, 30));

        let mut stats = Stats::default();
        stats.record(&session);

        assert_eq!(stats.games, 1);
        assert_eq!(stats.rounds_played, 3);
        assert_eq!(stats.rounds_won, 2);
        assert_eq!(stats.best_streak, 2);
        // Unsolved elapsed never leaks into the solve-time mean.
        assert_eq!(stats.avg_guess_time, 3.0);
    }

    #[test]
    fn record_keeps_a_running_mean_across_sessions() {
        let mut stats = Stats::default();

        let mut first = Session::new(2);
        first.record(result(true, 2));
        first.record(result(true, 4));
        stats.record(&first);

        let mut second = Session::new(1);
        second.record(result(true, 9));
        stats.record(&second);

        assert_eq!(stats.games, 2);
        assert_eq!(stats.rounds_played, 3);
        assert_eq!(stats.rounds_won, 3);
        assert_eq!(stats.avg_guess_time, 5.0);
    }

    #[test]
    fn record_with_no_solved_rounds_never_divides_by_zero() {
        let mut session = Session::new(2);
        session.record(result(false, 30));
        session.record(result(false, 20));

        let mut stats = Stats::default();
        stats.record(&session);

        assert_eq!(stats.games, 1);
        assert_eq!(stats.rounds_played, 2);
        assert_eq!(stats.rounds_won, 0);
        assert_eq!(stats.best_streak, 0);
        assert_eq!(stats.avg_guess_time, 0.0);
    }

    #[test]
    fn record_keeps_the_lifetime_best_streak() {
        let mut stats = Stats {
            best_streak: 5,
            ..Stats::default()
        };
        let mut session = Session::new(1);
        session.record(result(true, 2));
        stats.record(&session);
        assert_eq!(stats.best_streak, 5);
    }
}
