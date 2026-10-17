//! Headless runner. Real commands (run, check, bench) arrive with the Phase 1 core loop.

fn main() {
    println!(
        "citysim: {} ticks per simulated day at {} s per tick (no simulation yet, see docs/roadmap.md)",
        citysim_core::TICKS_PER_DAY,
        citysim_core::TICK_SECONDS
    );
}
