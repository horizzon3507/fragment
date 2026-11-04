use std::time::Duration;

/// Base points per escalation level (index into `game::LEVELS`).
pub const LEVEL_BASE: [u32; 6] = [1000, 800, 600, 400, 250, 100];

/// Score for a solved round: the level's base minus a per-second time
/// penalty, scaled by the streak multiplier.
pub fn score(level_idx: usize, elapsed: Duration, streak: u32) -> u32 {
    let _ = (level_idx, elapsed, streak);
    0
}
