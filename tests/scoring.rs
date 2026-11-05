// Score model coverage — implemented in wave 2.
use std::time::Duration;

use fragment::game::LEVELS;
use fragment::scoring::{LEVEL_BASE, score};

#[test]
fn level_base_matches_the_snippet_ladder() {
    assert_eq!(LEVEL_BASE.len(), LEVELS.len());
    assert_eq!(LEVEL_BASE, [1000, 800, 600, 400, 250, 100]);
}

#[test]
fn instant_solve_awards_full_base() {
    assert_eq!(score(0, Duration::ZERO, 0), 1000);
    assert_eq!(score(3, Duration::ZERO, 0), 400);
}

#[test]
fn time_penalty_is_proportional() {
    // 10 points per second, off the level base.
    assert_eq!(score(0, Duration::from_secs(1), 0), 990);
    assert_eq!(score(5, Duration::from_secs(1), 0), 90);
    assert_eq!(score(0, Duration::from_secs(10), 0), 900);
    assert_eq!(score(0, Duration::from_millis(2500), 0), 975);
}

#[test]
fn time_penalty_caps_at_half_the_base() {
    assert_eq!(score(0, Duration::from_secs(100), 0), 500);
    assert_eq!(score(0, Duration::from_secs(10_000), 0), 500);
    // The cap scales with the base: 400 -> 200.
    assert_eq!(score(3, Duration::from_secs(10_000), 0), 200);
}

#[test]
fn streak_multiplies_the_round_score() {
    assert_eq!(score(0, Duration::ZERO, 3), 1300);
    assert_eq!(score(1, Duration::ZERO, 2), 960);
    // The multiplier applies after the time penalty.
    assert_eq!(score(0, Duration::from_secs(10), 1), 990);
}

#[test]
fn penalty_cap_applies_before_the_streak_multiplier() {
    // The 50% cap leaves 500 of the 1000 base, then streak 1 adds 10%.
    assert_eq!(score(0, Duration::from_secs(100), 1), 550);
}

#[test]
fn out_of_range_level_clamps_to_the_last_base() {
    assert_eq!(score(99, Duration::ZERO, 0), 100);
    assert_eq!(score(usize::MAX, Duration::ZERO, 0), 100);
}
