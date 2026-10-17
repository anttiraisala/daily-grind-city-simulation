//! Vehicles on finite-length road segments with IDM car-following, signals and
//! queue spillback.
//!
//! This is the first slice of Phase 1. Known simplifications, each tracked in
//! `docs/design-plan.md`:
//! - one lane per segment (G4), no lane changes;
//! - a junction is "enter the next segment if there is room and the light is green";
//!   there are no conflict zones or right-of-way rules yet (G2);
//! - a vehicle only looks ahead one segment, so segments shorter than a braking
//!   distance can let a vehicle brake late for a red light further on.

use crate::clock::{seconds, TICK_SECONDS};
use crate::graph::{EdgeId, Graph};
use crate::idm::{accel, IdmParams};
use std::collections::VecDeque;

const DT: f64 = TICK_SECONDS;
/// Hard minimum bumper gap enforced after integration, m.
const HARD_GAP: f64 = 0.01;

#[derive(Clone, Debug)]
pub struct Vehicle {
    pub id: u32,
    pub route: Vec<EdgeId>,
    /// Index into `route` of the segment the vehicle is on.
    pub leg: usize,
    /// Position of the front bumper along the segment, m.
    pub pos: f64,
    pub speed: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub spawned: u64,
    pub spawn_blocked: u64,
    pub finished: u64,
    /// Times the safety clamp had to stop an overlap. Should stay 0.
    pub clamps: u64,
    /// Per segment: vehicle-seconds spent.
    pub vehicle_seconds: Vec<f64>,
    /// Per segment: vehicle-seconds spent at under 20 % of the speed limit.
    pub slow_seconds: Vec<f64>,
}

pub struct Traffic {
    pub graph: Graph,
    pub idm: IdmParams,
    /// Vehicles per segment, front-most first.
    lanes: Vec<VecDeque<Vehicle>>,
    pub tick: u64,
    next_id: u32,
    pub stats: Stats,
}

impl Traffic {
    pub fn new(graph: Graph, idm: IdmParams) -> Result<Self, String> {
        graph.validate()?;
        let n = graph.edges.len();
        Ok(Traffic {
            graph,
            idm,
            lanes: vec![VecDeque::new(); n],
            tick: 0,
            next_id: 0,
            stats: Stats {
                vehicle_seconds: vec![0.0; n],
                slow_seconds: vec![0.0; n],
                ..Stats::default()
            },
        })
    }

    pub fn vehicles_on(&self, edge: EdgeId) -> impl Iterator<Item = &Vehicle> {
        self.lanes[edge as usize].iter()
    }

    pub fn vehicle_count(&self) -> usize {
        self.lanes.iter().map(|l| l.len()).sum()
    }

    /// Puts a new vehicle at the start of the first segment of `route`.
    /// Returns its id, or `None` if there is no room (a full queue blocks entry).
    pub fn spawn(&mut self, route: Vec<EdgeId>) -> Option<u32> {
        assert!(!route.is_empty(), "empty route");
        for w in route.windows(2) {
            assert!(self.graph.connects(w[0], w[1]), "route is not connected");
        }
        let first = route[0] as usize;
        let limit = self.graph.edges[first].speed_limit;
        let len = self.idm.length;
        let mut speed = limit;
        if let Some(last) = self.lanes[first].back() {
            if last.pos - len < len + self.idm.s0 {
                self.stats.spawn_blocked += 1;
                return None;
            }
            speed = speed.min(last.speed);
        }
        let id = self.next_id;
        self.next_id += 1;
        self.stats.spawned += 1;
        self.lanes[first].push_back(Vehicle {
            id,
            route,
            leg: 0,
            pos: len,
            speed,
        });
        Some(id)
    }

    fn is_green(&self, edge: usize) -> bool {
        match self.graph.edges[edge].signal {
            Some(s) => s.is_green(seconds(self.tick)),
            None => true,
        }
    }

    /// Advances the world by one fixed step.
    pub fn step(&mut self) {
        let n = self.lanes.len();

        // 1. Accelerations from the state at the start of the step.
        let mut acc: Vec<Vec<f64>> = Vec::with_capacity(n);
        for e in 0..n {
            let edge = &self.graph.edges[e];
            let green = self.is_green(e);
            let mut row = Vec::with_capacity(self.lanes[e].len());
            for (i, v) in self.lanes[e].iter().enumerate() {
                let mut ahead: Option<(f64, f64)> = None;
                let mut consider = |gap: f64, dv: f64| {
                    if ahead.is_none_or(|(g, _)| gap < g) {
                        ahead = Some((gap, dv));
                    }
                };
                if i > 0 {
                    let l = &self.lanes[e][i - 1];
                    consider(l.pos - self.idm.length - v.pos, v.speed - l.speed);
                } else if v.leg + 1 < v.route.len() {
                    let ne = v.route[v.leg + 1] as usize;
                    if let Some(l) = self.lanes[ne].back() {
                        let gap = (edge.length - v.pos) + (l.pos - self.idm.length);
                        consider(gap, v.speed - l.speed);
                    }
                    if !green {
                        consider(edge.length - v.pos, v.speed);
                    }
                }
                row.push(accel(&self.idm, v.speed, edge.speed_limit, ahead));
            }
            acc.push(row);
        }

        // 2. Integrate (front to back) and enforce a hard minimum gap.
        for (lane, accs) in self.lanes.iter_mut().zip(&acc) {
            let mut prev: Option<(f64, f64)> = None; // (rear bumper, speed) of the vehicle ahead
            for (v, a) in lane.iter_mut().zip(accs) {
                let new_speed = (v.speed + a * DT).max(0.0);
                let mut pos = v.pos + 0.5 * (v.speed + new_speed) * DT;
                let mut speed = new_speed;
                if let Some((rear, lead_speed)) = prev {
                    if pos > rear - HARD_GAP {
                        pos = (rear - HARD_GAP).max(v.pos);
                        speed = speed.min(lead_speed);
                        self.stats.clamps += 1;
                    }
                }
                v.pos = pos;
                v.speed = speed;
                prev = Some((v.pos - self.idm.length, v.speed));
            }
        }

        // 3. Hand vehicles over to the next segment. The starting segment rotates so that
        // no approach always wins when several feed the same one.
        let green_now: Vec<bool> = (0..n).map(|e| self.is_green(e)).collect();
        for k in 0..n {
            let e = (k + self.tick as usize) % n;
            let length = self.graph.edges[e].length;
            let Some(front) = self.lanes[e].front() else {
                continue;
            };
            if front.pos < length {
                continue;
            }
            if front.leg + 1 >= front.route.len() {
                self.lanes[e].pop_front();
                self.stats.finished += 1;
                continue;
            }
            let ne = front.route[front.leg + 1] as usize;
            let new_pos = front.pos - length;
            let room = match self.lanes[ne].back() {
                Some(l) => l.pos - self.idm.length - new_pos >= self.idm.s0,
                None => true,
            };
            if green_now[e] && room {
                let mut v = self.lanes[e].pop_front().expect("front exists");
                v.leg += 1;
                v.pos = new_pos;
                self.lanes[ne].push_back(v);
            } else if let Some(f) = self.lanes[e].front_mut() {
                f.pos = length;
                f.speed = 0.0;
            }
        }

        // 4. Statistics for the heat map.
        for e in 0..n {
            let slow_below = 0.2 * self.graph.edges[e].speed_limit;
            for v in &self.lanes[e] {
                self.stats.vehicle_seconds[e] += DT;
                if v.speed < slow_below {
                    self.stats.slow_seconds[e] += DT;
                }
            }
        }
        self.tick += 1;
    }

    /// Smallest bumper gap between consecutive vehicles on one segment, if any pair exists.
    pub fn min_gap(&self) -> Option<f64> {
        let mut best: Option<f64> = None;
        for lane in &self.lanes {
            for w in 0..lane.len().saturating_sub(1) {
                let gap = lane[w].pos - self.idm.length - lane[w + 1].pos;
                best = Some(best.map_or(gap, |b| b.min(gap)));
            }
        }
        best
    }

    /// Hash of the whole traffic state, for determinism checks (FNV-1a).
    pub fn state_hash(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut feed = |x: u64| {
            for b in x.to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        };
        feed(self.tick);
        feed(self.stats.finished);
        for (e, lane) in self.lanes.iter().enumerate() {
            for v in lane {
                feed(e as u64);
                feed(v.id as u64);
                feed(v.pos.to_bits());
                feed(v.speed.to_bits());
            }
        }
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{EdgeKind, Signal};
    use crate::rng::Rng;

    /// Straight chain of `n` segments of `len` metres each, returns graph and edge ids.
    fn chain(n: usize, len: f64, limit: f64) -> (Graph, Vec<EdgeId>) {
        let mut g = Graph::new();
        let nodes: Vec<_> = (0..=n)
            .map(|i| g.add_node(i as f64 * len, 0.0, 0))
            .collect();
        let edges = (0..n)
            .map(|i| g.add_edge(nodes[i], nodes[i + 1], limit, 1, EdgeKind::Road))
            .collect();
        (g, edges)
    }

    #[test]
    fn lone_vehicle_reaches_the_speed_limit_and_finishes() {
        let (g, e) = chain(2, 500.0, 13.9);
        let mut t = Traffic::new(g, IdmParams::default()).unwrap();
        t.spawn(e.clone()).unwrap();
        let mut top = 0.0f64;
        for _ in 0..(120.0 / TICK_SECONDS) as usize {
            t.step();
            if let Some(v) = t.vehicles_on(0).next() {
                top = top.max(v.speed);
            }
        }
        assert!(top > 13.5 && top < 14.0, "top speed {top}");
        assert_eq!(t.stats.finished, 1);
        assert_eq!(t.vehicle_count(), 0);
    }

    #[test]
    fn red_light_holds_traffic_and_green_releases_it() {
        let (mut g, e) = chain(2, 300.0, 13.9);
        g.set_signal(
            e[0],
            Signal {
                cycle_s: 1000.0,
                offset_s: 100.0,
                green_s: 500.0,
            },
        ); // red for the first 100 s
        let mut t = Traffic::new(g, IdmParams::default()).unwrap();
        for _ in 0..4 {
            while t.spawn(e.clone()).is_none() {
                t.step();
            }
            for _ in 0..30 {
                t.step();
            }
        }
        for _ in 0..(90.0 / TICK_SECONDS) as usize - 120 {
            t.step();
        }
        assert_eq!(t.lanes[1].len(), 0, "nobody may pass a red light");
        assert_eq!(t.stats.finished, 0);
        assert_eq!(t.stats.clamps, 0);
        let gap = t.min_gap().unwrap();
        assert!(gap > 1.0, "queue gap {gap}");
        for _ in 0..(150.0 / TICK_SECONDS) as usize {
            t.step();
        }
        assert_eq!(t.stats.finished, 4, "green releases the queue");
    }

    #[test]
    fn bottleneck_queue_spills_back_and_never_overlaps() {
        // A (300 m) feeds B (300 m); B's exit is green only 10 s of every 60 s.
        let (mut g, e) = chain(2, 300.0, 13.9);
        g.set_signal(
            e[1],
            Signal {
                cycle_s: 60.0,
                offset_s: 0.0,
                green_s: 10.0,
            },
        );
        let mut t = Traffic::new(g, IdmParams::default()).unwrap();
        let mut worst_gap = f64::MAX;
        for _ in 0..(1800.0 / TICK_SECONDS) as usize {
            if t.tick.is_multiple_of(20) {
                t.spawn(e.clone());
            }
            t.step();
            if let Some(g) = t.min_gap() {
                worst_gap = worst_gap.min(g);
            }
        }
        assert!(
            t.stats.spawn_blocked > 0,
            "the queue must reach the entrance"
        );
        assert_eq!(t.stats.clamps, 0, "IDM alone must keep vehicles apart");
        assert!(worst_gap > 0.5, "worst gap {worst_gap}");
        // Jam density: a 300 m segment holds at most 300 / (4.5 + 2) = 46 vehicles.
        for edge in 0..2 {
            assert!(t.vehicles_on(edge).count() <= 47);
        }
        assert!(
            t.stats.slow_seconds[0] > 0.0,
            "queue shows up in the slow-time map"
        );
    }

    #[test]
    fn same_inputs_give_the_same_state() {
        fn run() -> u64 {
            let (mut g, e) = chain(3, 200.0, 13.9);
            g.set_signal(
                e[1],
                Signal {
                    cycle_s: 40.0,
                    offset_s: 5.0,
                    green_s: 15.0,
                },
            );
            let mut t = Traffic::new(g, IdmParams::default()).unwrap();
            let mut rng = Rng::new(42);
            for _ in 0..20_000 {
                if rng.next_f64() < 0.03 {
                    t.spawn(e.clone());
                }
                t.step();
            }
            t.state_hash()
        }
        assert_eq!(run(), run());
    }
}
