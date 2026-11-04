pub mod board;
pub mod guess;

use anyhow::Result;
use ratatui::DefaultTerminal;

/// The shared terminal type used by every screen.
pub type Tui = DefaultTerminal;

/// Enter the alternate screen and raw mode.
pub fn init() -> Result<Tui> {
    Ok(ratatui::try_init()?)
}

/// Leave raw mode and the alternate screen.
pub fn restore() {
    ratatui::restore();
}
