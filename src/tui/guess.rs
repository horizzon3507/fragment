use anyhow::Result;

use crate::game::Round;
use crate::player::Player;
use crate::tui::Tui;

/// Header strip above the play area (round n/N · score · streak).
#[derive(Debug, Clone, Copy)]
pub struct Header {
    pub round: usize,
    pub total: usize,
    pub score: u32,
    pub streak: u32,
}

/// How an interactive round ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundEnd {
    /// Correct guess.
    Solved,
    /// Esc give-up.
    GaveUp,
    /// Final level consumed without a correct guess.
    Exhausted,
    /// Ctrl+C — abort the whole session.
    Quit,
}

/// Run one interactive round: play the snippet, handle guessing,
/// escalation and the reveal screen. Returns when the round is over.
pub fn play_round(
    term: &mut Tui,
    round: &mut Round,
    player: &Player,
    header: &Header,
) -> Result<RoundEnd> {
    let _ = (term, round, player, header);
    Ok(RoundEnd::Quit)
}
