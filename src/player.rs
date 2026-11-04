use std::path::Path;
use std::time::Duration;

use anyhow::Result;

/// Plays snippets through a persistent `mpv --idle` process driven over its
/// JSON IPC socket. `play_snippet` loads the file, seeks to `offset`,
/// unpauses, and pauses again once `length` has elapsed. Dropping the player
/// quits mpv.
pub struct Player;

impl Player {
    /// Spawn mpv (`--idle --no-video --really-quiet --input-ipc-server=…`).
    /// Errors cleanly when the `mpv` binary is not installed.
    pub fn spawn() -> Result<Self> {
        anyhow::bail!("implemented in wave 1")
    }

    /// Play `length` seconds of `path` starting at `offset`.
    pub fn play_snippet(&self, path: &Path, offset: Duration, length: Duration) -> Result<()> {
        let _ = (path, offset, length);
        Ok(())
    }

    /// Pause playback and cancel any pending pause timer.
    pub fn stop(&self) -> Result<()> {
        Ok(())
    }
}
