//! Seeded random numbers. Every agent owns its own stream so that results do not
//! depend on the order in which agents are updated.

/// SplitMix64 generator: small, fast and good enough for simulation decisions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng(u64);

fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(mix(seed))
    }

    /// Independent stream for one agent (or any other numbered entity).
    pub fn for_agent(seed: u64, agent: u64) -> Self {
        Rng(mix(seed ^ mix(agent.wrapping_add(0x9E37_79B9_7F4A_7C15))))
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix(self.0)
    }

    /// Uniform in [0, 1).
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform integer in [0, n). `n` must be positive.
    pub fn below(&mut self, n: u64) -> u64 {
        assert!(n > 0, "below(0)");
        ((self.next_u64() as u128 * n as u128) >> 64) as u64
    }

    /// Approximately normal (Irwin-Hall, 12 uniforms), mean 0, standard deviation 1.
    pub fn normal(&mut self) -> f64 {
        (0..12).map(|_| self.next_f64()).sum::<f64>() - 6.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn agent_streams_differ_and_do_not_depend_on_order() {
        let first = Rng::for_agent(1, 5).next_u64();
        let _ = Rng::for_agent(1, 6).next_u64();
        assert_eq!(first, Rng::for_agent(1, 5).next_u64());
        assert_ne!(first, Rng::for_agent(1, 6).next_u64());
        assert_ne!(first, Rng::for_agent(2, 5).next_u64());
    }

    #[test]
    fn ranges_hold() {
        let mut r = Rng::new(3);
        for _ in 0..10_000 {
            let f = r.next_f64();
            assert!((0.0..1.0).contains(&f));
            assert!(r.below(10) < 10);
        }
        let mean: f64 = (0..20_000).map(|_| r.normal()).sum::<f64>() / 20_000.0;
        assert!(mean.abs() < 0.05);
    }
}
