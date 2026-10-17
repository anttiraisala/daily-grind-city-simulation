//! Deterministic, headless city simulation core.
//!
//! The simulation is authoritative and knows nothing about rendering or networking.
//! Same seed + same machine + same program version must give the same run.

/// Length of one fixed simulation step in seconds.
pub const TICK_SECONDS: f64 = 0.1;

/// Number of fixed steps in one simulated day.
pub const TICKS_PER_DAY: u64 = 864_000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_day_has_the_expected_number_of_ticks() {
        assert_eq!((TICKS_PER_DAY as f64 * TICK_SECONDS) as u64, 86_400);
    }
}
