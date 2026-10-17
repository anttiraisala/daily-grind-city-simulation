//! Small deterministic random number generator (SplitMix64).
//!
//! Every agent owns one generator, so an agent's random choices never depend on
//! the order in which other agents are processed. Only integer arithmetic and
//! plain floating-point additions are used, so results do not depend on a
//! platform's math library.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rng(pub u64);

impl Rng {
    /// Generator for one agent, derived from the run seed and the agent id.
    pub fn for_agent(seed: u64, id: u64) -> Rng {
        let mut r = Rng(seed ^ id.wrapping_mul(0xD1B5_4A32_D192_ED03) ^ 0x9E37_79B9_7F4A_7C15);
        r.next_u64();
        r.next_u64();
        r
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform in [0, 1).
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u32 << 24) as f32
    }

    /// Uniform integer in [0, n).
    #[inline]
    pub fn below(&mut self, n: u32) -> u32 {
        (((self.next_u64() >> 32) * n as u64) >> 32) as u32
    }

    /// Approximately standard normal (Irwin-Hall: sum of 12 uniforms minus 6).
    pub fn normal(&mut self) -> f64 {
        let mut s = 0.0;
        for _ in 0..12 {
            s += self.next_f64();
        }
        s - 6.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let mut a = Rng::for_agent(7, 3);
        let mut b = Rng::for_agent(7, 3);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_agents_differ() {
        let mut a = Rng::for_agent(7, 3);
        let mut b = Rng::for_agent(7, 4);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn ranges_hold() {
        let mut r = Rng::for_agent(1, 1);
        for _ in 0..10_000 {
            let f = r.next_f64();
            assert!((0.0..1.0).contains(&f));
            assert!(r.below(17) < 17);
        }
        let mean: f64 = (0..20_000).map(|_| r.normal()).sum::<f64>() / 20_000.0;
        assert!(mean.abs() < 0.05, "normal mean was {mean}");
    }
}
