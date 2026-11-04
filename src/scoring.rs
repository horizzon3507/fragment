use std::time::Duration;

/// Base points per escalation level (index into `game::LEVELS`).
pub const LEVEL_BASE: [u32; 6] = [1000, 800, 600, 400, 250, 100];

/// Score for a solved round: the level's base minus a per-second time
/// penalty, scaled by the streak multiplier.
pub fn score(level_idx: usize, elapsed: Duration, streak: u32) -> u32 {
    let base = LEVEL_BASE[level_idx.min(LEVEL_BASE.len() - 1)];
    // 10 points per second, never more than half the base.
    let penalty = (elapsed.as_secs_f64() * 10.0).min(base as f64 * 0.5);
    // `streak` is the session streak before this round.
    let multiplier = 1.0 + 0.10 * streak as f64;
    ((base as f64 - penalty) * multiplier).round().max(0.0) as u32
}
