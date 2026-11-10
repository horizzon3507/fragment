pub mod board;
pub mod guess;

use anyhow::Result;
use ratatui::DefaultTerminal;

/// The shared terminal type used by every screen.
pub type Tui = DefaultTerminal;

/// Enter the alternate screen and raw mode. The first successful call also
/// installs a panic hook that restores the terminal before unwinding.
pub fn init() -> Result<Tui> {
    let term = ratatui::try_init()?;
    static PANIC_HOOK: std::sync::Once = std::sync::Once::new();
    PANIC_HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            ratatui::restore();
            previous(info);
        }));
    });
    Ok(term)
}

/// Leave raw mode and the alternate screen.
pub fn restore() {
    ratatui::restore();
}
