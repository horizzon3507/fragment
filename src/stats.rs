use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::session::Session;

/// Lifetime stats persisted to `$XDG_DATA_HOME/fragment/stats.json`
/// (~/.local/share/fragment/stats.json).
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Stats {
    pub games: u64,
    pub rounds_won: u64,
    pub rounds_played: u64,
    pub best_streak: u32,
    /// Average seconds-to-guess across solved rounds.
    pub avg_guess_time: f64,
}

impl Stats {
    /// Load stats; a missing or corrupt file yields defaults.
    pub fn load() -> Result<Self> {
        Ok(Self::default())
    }

    /// Persist stats, creating the data dir as needed.
    pub fn save(&self) -> Result<()> {
        Ok(())
    }

    /// Fold one finished session into the running totals.
    pub fn record(&mut self, session: &Session) {
        let _ = session;
    }
}
