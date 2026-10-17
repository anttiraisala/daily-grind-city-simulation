//! Fixed-step simulation clock.

/// Seconds per simulation step.
pub const TICK_SECONDS: f64 = 0.1;
/// Steps in one simulated day.
pub const TICKS_PER_DAY: u64 = 864_000;

/// Simulated seconds since the start of the run.
pub fn seconds(tick: u64) -> f64 {
    tick as f64 * TICK_SECONDS
}

/// Zero-based simulated day.
pub fn day(tick: u64) -> u64 {
    tick / TICKS_PER_DAY
}

/// Minute of the day, 0..1440.
pub fn minute_of_day(tick: u64) -> u32 {
    ((tick % TICKS_PER_DAY) / 600) as u32
}

/// Clock text such as "07:45".
pub fn hhmm(tick: u64) -> String {
    let m = minute_of_day(tick);
    format!("{:02}:{:02}", m / 60, m % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_arithmetic() {
        assert_eq!(seconds(TICKS_PER_DAY), 86_400.0);
        assert_eq!(day(TICKS_PER_DAY - 1), 0);
        assert_eq!(day(TICKS_PER_DAY), 1);
        assert_eq!(hhmm(0), "00:00");
        assert_eq!(hhmm(7 * 36_000 + 45 * 600), "07:45");
        assert_eq!(hhmm(TICKS_PER_DAY + 600), "00:01");
    }
}
