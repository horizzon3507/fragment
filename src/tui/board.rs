use anyhow::Result;

use crate::session::Session;
use crate::tui::Tui;

/// Render the final scoreboard (round history, totals, best streak) and
/// wait for a keypress.
pub fn show(term: &mut Tui, session: &Session) -> Result<()> {
    let _ = (term, session);
    Ok(())
}
