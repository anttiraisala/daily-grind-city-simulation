//! Intelligent Driver Model (Treiber et al.): car-following acceleration.
//!
//! Plain IDM is collision-free by design; crashes need a driver error model on top (gap G1).

#[derive(Clone, Copy, Debug)]
pub struct IdmParams {
    /// Maximum acceleration, m/s^2.
    pub a_max: f64,
    /// Comfortable deceleration, m/s^2.
    pub b: f64,
    /// Desired time headway, s.
    pub headway_s: f64,
    /// Minimum standstill gap, m.
    pub s0: f64,
    /// Acceleration exponent.
    pub delta: f64,
    /// Vehicle length, m.
    pub length: f64,
}

impl Default for IdmParams {
    fn default() -> Self {
        IdmParams {
            a_max: 1.4,
            b: 2.0,
            headway_s: 1.5,
            s0: 2.0,
            delta: 4.0,
            length: 4.5,
        }
    }
}

/// Hardest braking the model allows, m/s^2.
pub const MAX_BRAKE: f64 = 9.0;

/// Acceleration for speed `v`, desired speed `v0`, and an optional obstacle ahead:
/// `(bumper gap in m, own speed minus obstacle speed)`.
pub fn accel(p: &IdmParams, v: f64, v0: f64, ahead: Option<(f64, f64)>) -> f64 {
    let free = 1.0 - (v / v0).powf(p.delta);
    let interaction = match ahead {
        Some((gap, dv)) => {
            let gap = gap.max(0.01);
            let s_star = p.s0 + v * p.headway_s + v * dv / (2.0 * (p.a_max * p.b).sqrt());
            (s_star.max(p.s0) / gap).powi(2)
        }
        None => 0.0,
    };
    (p.a_max * (free - interaction)).max(-MAX_BRAKE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_road_accelerates_then_levels_off() {
        let p = IdmParams::default();
        assert!((accel(&p, 0.0, 14.0, None) - p.a_max).abs() < 1e-9);
        assert!(accel(&p, 14.0, 14.0, None).abs() < 1e-9);
        assert!(accel(&p, 15.0, 14.0, None) < 0.0);
    }

    #[test]
    fn brakes_for_a_close_obstacle() {
        let p = IdmParams::default();
        assert!(accel(&p, 10.0, 14.0, Some((5.0, 10.0))) < -p.b);
        assert!(accel(&p, 10.0, 14.0, Some((500.0, 0.0))) > 0.0);
    }

    #[test]
    fn braking_is_capped() {
        let p = IdmParams::default();
        assert!(accel(&p, 30.0, 30.0, Some((0.5, 30.0))) >= -MAX_BRAKE);
    }
}
