//! Deterministic, headless city simulation core.
//!
//! The simulation is authoritative and knows nothing about rendering or networking.
//! Same seed + same machine + same program version must give the same run.

pub mod clock;
pub mod graph;
pub mod idm;
pub mod rng;
pub mod traffic;

pub use clock::{TICKS_PER_DAY, TICK_SECONDS};
