//! The living city: residents with daily plans, driving or walking between buildings.
//!
//! First slice of the activity layer. Each resident has a home, maybe a workplace and
//! maybe a car. Workers leave early enough to arrive on time (using a learned estimate of
//! their commute) and return after work; the others make one shop errand a day.
//! Needs, moods, calendar and parking are not modelled yet (see docs/roadmap.md).
//! Pedestrians do not yet avoid each other (no personal space yet); they do obey
//! pedestrian crossings and walk signals.

use crate::clock::{seconds, TICKS_PER_DAY, TICK_SECONDS};
use crate::idm::IdmParams;
use crate::map::{foot_incoming, road_incoming, MapData};
use crate::path::{build_tree, Incoming, RouteTree};
use crate::rng::Rng;
use crate::scenario::Scenario;
use crate::traffic::Traffic;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

const DT: f64 = TICK_SECONDS;
const TICKS_PER_SECOND: u64 = 10;
const WALK_SPEED: f64 = 1.4;
const BIKE_SPEED: f64 = 5.0;
/// Ticks to wait before trying again when the street at the start is full.
const RETRY_TICKS: u64 = 10;
/// Learned commute estimates keep this margin over the last trip.
const LEARNED_MARGIN: f64 = 1.1;
/// A worker counts as late if arriving more than this many seconds after the start.
const LATE_TOLERANCE_S: f64 = 180.0;

#[derive(Clone, Debug)]
enum Kind {
    /// Goes to a workplace; `dur` is (mean, sd) hours at work.
    Worker {
        start_min: f64,
        dur: (f64, f64),
        buffer_s: f64,
    },
    /// One shop visit per day.
    Errand,
    /// Stays at home (remote work, no reachable destination ...).
    Stay,
}

#[derive(Clone, Debug)]
struct Person {
    rng: Rng,
    home: usize,
    dest: usize,
    kind: Kind,
    has_car: bool,
    speed: f64,
    /// 0 = at home, next trip goes out; 1 = at the destination, next trip goes home.
    stage: u8,
    trip_start: u64,
    est_s: f64,
    work_start_s: f64,
}

struct Walker {
    person: u32,
    /// Pedestrian edge tokens: `(edge << 1) | reversed`.
    steps: Vec<u32>,
    i: usize,
    s: f64,
    entered: bool,
    speed: f64,
}

#[derive(Clone, Debug, Default)]
pub struct WorldStats {
    pub trips_started: u64,
    pub trips_done: u64,
    pub car_trips: u64,
    pub foot_trips: u64,
    pub late_arrivals: u64,
    pub commute_seconds_car: f64,
    pub commute_seconds_foot: f64,
    pub commutes_car: u64,
    pub commutes_foot: u64,
    pub spawn_retries: u64,
    pub unreachable: u64,
    pub crossing_wait_seconds: f64,
    pub crossings_made: u64,
    /// Ticks in which a vehicle drove over an occupied crossing. Must stay 0.
    pub crossing_violations: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityKind {
    Person,
    Vehicle,
}

#[derive(Clone, Copy, Debug)]
pub struct EntityPos {
    pub kind: EntityKind,
    pub id: u32,
    pub x: f32,
    pub y: f32,
    /// Direction of travel in radians (0 = east, counter-clockwise).
    pub heading: f32,
}

pub struct World {
    pub map: MapData,
    pub traffic: Traffic,
    pub scenario: Scenario,
    pub stats: WorldStats,
    pub warnings: Vec<String>,
    people: Vec<Person>,
    walkers: Vec<Walker>,
    events: BinaryHeap<Reverse<(u64, u32)>>,
    vehicle_person: HashMap<u32, u32>,
    road_in: Incoming,
    foot_in: Incoming,
    road_trees: HashMap<u32, RouteTree>,
    foot_trees: HashMap<u32, RouteTree>,
}

fn pick_weighted(rng: &mut Rng, weights: &[f64]) -> usize {
    let total: f64 = weights.iter().sum();
    let mut x = rng.next_f64() * total;
    for (i, w) in weights.iter().enumerate() {
        if x < *w {
            return i;
        }
        x -= w;
    }
    weights.len().saturating_sub(1)
}

fn preferred_kinds(occupation: &str) -> &'static [&'static str] {
    match occupation {
        "office" => &["office", "factory"],
        "parttime" => &["shop", "restaurant", "office"],
        "shift" => &["hospital", "factory", "shop", "police", "firestation"],
        "student" => &["school"],
        _ => &[],
    }
}

impl World {
    pub fn new(map: MapData, scenario: Scenario) -> Result<World, String> {
        let mut warnings = map.warnings.clone();
        let mut traffic = Traffic::new(map.graph.clone(), IdmParams::default())?;
        traffic.set_crossings(map.crossings.clone());
        let road_in = road_incoming(&map.graph);
        let foot_in = foot_incoming(&map.foot, map.graph.nodes.len());

        // Homes, filled in order up to their capacity.
        let mut home_slots: Vec<usize> = Vec::new();
        for (i, b) in map.buildings.iter().enumerate() {
            for _ in 0..b.residents {
                home_slots.push(i);
            }
        }
        let wanted = scenario.residents as usize;
        if wanted > home_slots.len() {
            warnings.push(format!(
                "scenario wants {wanted} residents but homes hold {}; using {}",
                home_slots.len(),
                home_slots.len()
            ));
        }
        let n = wanted.min(home_slots.len());

        let mut jobs_left: Vec<u32> = map.buildings.iter().map(|b| b.jobs).collect();
        let shops: Vec<usize> = (0..map.buildings.len())
            .filter(|&i| {
                map.buildings[i].visitors > 0
                    && matches!(map.buildings[i].kind.as_str(), "shop" | "restaurant")
            })
            .collect();
        let shops = if shops.is_empty() {
            (0..map.buildings.len())
                .filter(|&i| map.buildings[i].visitors > 0)
                .collect()
        } else {
            shops
        };

        let shares: Vec<f64> = scenario.occupations.iter().map(|o| o.share).collect();
        let mut people = Vec::with_capacity(n);
        for (i, &home) in home_slots.iter().take(n).enumerate() {
            let mut rng = Rng::for_agent(scenario.seed, i as u64);
            let occ = &scenario.occupations[pick_weighted(&mut rng, &shares)];
            let mut kind = Kind::Stay;
            let mut dest = home;
            if occ.has_workplace() {
                if rng.next_f64() >= occ.remote_share {
                    // Candidate workplaces with free jobs, preferring the occupation's kinds.
                    let pref = preferred_kinds(&occ.kind);
                    let mut cands: Vec<usize> = (0..map.buildings.len())
                        .filter(|&b| {
                            jobs_left[b] > 0 && pref.contains(&map.buildings[b].kind.as_str())
                        })
                        .collect();
                    if cands.is_empty() && !pref.contains(&"school") {
                        cands = (0..map.buildings.len())
                            .filter(|&b| jobs_left[b] > 0)
                            .collect();
                    }
                    if !cands.is_empty() {
                        let w: Vec<f64> = cands.iter().map(|&b| jobs_left[b] as f64).collect();
                        let b = cands[pick_weighted(&mut rng, &w)];
                        jobs_left[b] -= 1;
                        dest = b;
                        let (start_min, dur) = if !occ.shifts.is_empty() {
                            let (s, d) = occ.shifts[rng.below(occ.shifts.len() as u64) as usize];
                            (s, (d, 0.0))
                        } else {
                            let w: Vec<f64> = occ.starts.iter().map(|s| s.weight).collect();
                            let st = &occ.starts[pick_weighted(&mut rng, &w)];
                            (st.minute + st.sd_minutes * rng.normal(), occ.duration_hours)
                        };
                        let buffer_s = (scenario.buffer_minutes.0
                            + scenario.buffer_minutes.1 * rng.normal())
                        .max(0.0)
                            * 60.0;
                        kind = Kind::Worker {
                            start_min,
                            dur,
                            buffer_s,
                        };
                    }
                }
            } else if !shops.is_empty() {
                dest = shops[rng.below(shops.len() as u64) as usize];
                kind = Kind::Errand;
            }
            let speed = (WALK_SPEED + 0.15 * rng.normal().clamp(-2.0, 2.0)).max(0.8);
            people.push(Person {
                rng,
                home,
                dest,
                kind,
                has_car: false,
                speed,
                stage: 0,
                trip_start: 0,
                est_s: 0.0,
                work_start_s: 0.0,
            });
        }

        // Cars.
        if let Some(k) = scenario.force_car_count {
            let mut commuters: Vec<usize> = (0..people.len())
                .filter(|&i| matches!(people[i].kind, Kind::Worker { .. }))
                .collect();
            let mut r = Rng::new(scenario.seed ^ 0xCA12_5EED);
            for i in (1..commuters.len()).rev() {
                let j = r.below(i as u64 + 1) as usize;
                commuters.swap(i, j);
            }
            if (k as usize) > commuters.len() {
                warnings.push(format!(
                    "forceCarCount {k} exceeds the {} commuters",
                    commuters.len()
                ));
            }
            for &i in commuters.iter().take(k as usize) {
                people[i].has_car = true;
            }
        } else {
            for p in &mut people {
                p.has_car = p.rng.next_f64() < scenario.car_share;
            }
        }
        for p in &mut people {
            if !p.has_car && p.rng.next_f64() < scenario.bike_share {
                p.speed = BIKE_SPEED;
            }
        }

        let mut w = World {
            map,
            traffic,
            scenario,
            stats: WorldStats::default(),
            warnings,
            people,
            walkers: Vec::new(),
            events: BinaryHeap::new(),
            vehicle_person: HashMap::new(),
            road_in,
            foot_in,
            road_trees: HashMap::new(),
            foot_trees: HashMap::new(),
        };

        // Initial commute estimates and first departures.
        for i in 0..w.people.len() {
            match w.people[i].kind {
                Kind::Stay => continue,
                Kind::Worker { .. } | Kind::Errand => {}
            }
            let free = w.free_flow_seconds(i);
            match free {
                Some(f) => w.people[i].est_s = f * w.scenario.commute_initial_factor,
                None => {
                    w.people[i].kind = Kind::Stay;
                    w.stats.unreachable += 1;
                    continue;
                }
            }
            let t = w.next_departure(i, 0);
            w.events.push(Reverse((t, i as u32)));
        }
        if w.stats.unreachable > 0 {
            let msg = format!(
                "{} residents cannot reach their destination and stay home",
                w.stats.unreachable
            );
            w.warnings.push(msg);
        }
        Ok(w)
    }

    pub fn resident_count(&self) -> usize {
        self.people.len()
    }

    pub fn car_owner_count(&self) -> usize {
        self.people.iter().filter(|p| p.has_car).count()
    }

    pub fn walker_count(&self) -> usize {
        self.walkers.len()
    }

    pub fn tick(&self) -> u64 {
        self.traffic.tick
    }

    fn road_tree(&mut self, dest: u32) -> &RouteTree {
        let road_in = &self.road_in;
        self.road_trees
            .entry(dest)
            .or_insert_with(|| build_tree(road_in, dest))
    }

    fn foot_tree(&mut self, dest: u32) -> &RouteTree {
        let foot_in = &self.foot_in;
        self.foot_trees
            .entry(dest)
            .or_insert_with(|| build_tree(foot_in, dest))
    }

    /// Free-flow travel time home to destination in seconds, using the person's usual mode.
    fn free_flow_seconds(&mut self, i: usize) -> Option<f64> {
        let (home, dest, has_car, speed) = {
            let p = &self.people[i];
            (
                self.map.buildings[p.home].clone(),
                self.map.buildings[p.dest].clone(),
                p.has_car,
                p.speed,
            )
        };
        if has_car {
            let t = self.road_tree(dest.entrance);
            let c = t.cost[home.entrance as usize];
            if c.is_finite() {
                return Some(c);
            }
        }
        let t = self.foot_tree(dest.door);
        let c = t.cost[home.door as usize];
        c.is_finite().then(|| c / speed)
    }

    /// First departure tick strictly after `after` for the person's next outbound trip.
    fn next_departure(&mut self, i: usize, after: u64) -> u64 {
        let mut day = after / TICKS_PER_DAY;
        loop {
            let t = self.departure_on(i, day);
            if t > after {
                return t;
            }
            day += 1;
        }
    }

    fn departure_on(&mut self, i: usize, day: u64) -> u64 {
        let p = &mut self.people[i];
        let day_s = day as f64 * 86_400.0;
        let dep_s = match p.kind {
            Kind::Worker {
                start_min,
                buffer_s,
                ..
            } => {
                let start_s = day_s + start_min * 60.0 + 180.0 * p.rng.normal();
                p.work_start_s = start_s;
                start_s - p.est_s - buffer_s
            }
            Kind::Errand => day_s + 9.0 * 3600.0 + 7.0 * 3600.0 * p.rng.next_f64(),
            Kind::Stay => f64::MAX,
        };
        (dep_s.max(0.0) * TICKS_PER_SECOND as f64) as u64
    }

    /// Ticks the person stays at the destination.
    fn stay_ticks(&mut self, i: usize) -> u64 {
        let p = &mut self.people[i];
        let secs = match p.kind {
            Kind::Worker { dur, .. } => (dur.0 + dur.1 * p.rng.normal()).clamp(0.5, 16.0) * 3600.0,
            _ => 1200.0 + 1200.0 * p.rng.next_f64(),
        };
        (secs * TICKS_PER_SECOND as f64) as u64
    }

    fn depart(&mut self, i: usize, t: u64) {
        let (from, to) = {
            let p = &self.people[i];
            if p.stage == 0 {
                (p.home, p.dest)
            } else {
                (p.dest, p.home)
            }
        };
        let (from_b, to_b) = (
            self.map.buildings[from].clone(),
            self.map.buildings[to].clone(),
        );
        let has_car = self.people[i].has_car;
        if has_car {
            let route = self.road_tree(to_b.entrance).route(from_b.entrance);
            if let Some(route) = route {
                if route.is_empty() {
                    self.people[i].trip_start = t;
                    self.stats.trips_started += 1;
                    self.arrive(i, t);
                    return;
                }
                match self.traffic.spawn(route) {
                    Some(vid) => {
                        self.vehicle_person.insert(vid, i as u32);
                        self.people[i].trip_start = t;
                        self.stats.trips_started += 1;
                        self.stats.car_trips += 1;
                    }
                    None => {
                        self.stats.spawn_retries += 1;
                        self.events.push(Reverse((t + RETRY_TICKS, i as u32)));
                    }
                }
                return;
            }
        }
        match self.foot_tree(to_b.door).route(from_b.door) {
            Some(steps) => {
                self.people[i].trip_start = t;
                self.stats.trips_started += 1;
                if steps.is_empty() {
                    self.arrive(i, t);
                    return;
                }
                self.stats.foot_trips += 1;
                let speed = self.people[i].speed;
                self.walkers.push(Walker {
                    person: i as u32,
                    steps,
                    i: 0,
                    s: 0.0,
                    entered: false,
                    speed,
                });
            }
            None => {
                self.stats.unreachable += 1;
                // Try again tomorrow.
                let next = self.next_departure(i, t);
                self.events.push(Reverse((next, i as u32)));
            }
        }
    }

    fn arrive(&mut self, i: usize, t: u64) {
        self.stats.trips_done += 1;
        let by_car = self.people[i].has_car;
        let actual = (t - self.people[i].trip_start) as f64 * DT;
        if self.people[i].stage == 0 {
            if let Kind::Worker { .. } = self.people[i].kind {
                let now_s = t as f64 * DT;
                if now_s > self.people[i].work_start_s + LATE_TOLERANCE_S {
                    self.stats.late_arrivals += 1;
                }
                let lr = self.scenario.commute_learning_rate;
                let p = &mut self.people[i];
                p.est_s = (1.0 - lr) * p.est_s + lr * actual * LEARNED_MARGIN;
                if by_car {
                    self.stats.commute_seconds_car += actual;
                    self.stats.commutes_car += 1;
                } else {
                    self.stats.commute_seconds_foot += actual;
                    self.stats.commutes_foot += 1;
                }
            }
            self.people[i].stage = 1;
            let stay = self.stay_ticks(i);
            self.events.push(Reverse((t + stay.max(1), i as u32)));
        } else {
            self.people[i].stage = 0;
            let next = self.next_departure(i, t);
            self.events.push(Reverse((next, i as u32)));
        }
    }

    /// May the walker step onto the crossing now?
    fn may_enter(&self, ci: usize, edge_len: f64, speed: f64) -> bool {
        match self.traffic.walk_allowed(ci) {
            Some(false) => return false,
            Some(true) if self.traffic.walk_remaining(ci) < 0.6 * edge_len / speed => {
                return false;
            }
            Some(true) | None => {}
        }
        !self.traffic.crossing_danger(ci)
    }

    fn step_walkers(&mut self) {
        let mut walkers = std::mem::take(&mut self.walkers);
        let mut arrived: Vec<u32> = Vec::new();
        walkers.retain_mut(|w| {
            let mut budget = w.speed * DT;
            loop {
                if w.i >= w.steps.len() {
                    arrived.push(w.person);
                    return false;
                }
                let edge = &self.map.foot.edges[(w.steps[w.i] >> 1) as usize];
                if w.s == 0.0 && !w.entered {
                    if let Some(ci) = edge.crossing {
                        if !self.may_enter(ci, edge.length, w.speed) {
                            self.stats.crossing_wait_seconds += DT;
                            return true;
                        }
                        self.stats.crossings_made += 1;
                    }
                    w.entered = true;
                }
                let remain = edge.length - w.s;
                if budget >= remain {
                    budget -= remain;
                    w.i += 1;
                    w.s = 0.0;
                    w.entered = false;
                } else {
                    w.s += budget;
                    return true;
                }
            }
        });
        self.walkers = walkers;
        let t = self.traffic.tick;
        for p in arrived {
            self.arrive(p as usize, t);
        }
    }

    /// Advances the whole city by one fixed step.
    pub fn step(&mut self) {
        let t = self.traffic.tick;
        while let Some(&Reverse((et, p))) = self.events.peek() {
            if et > t {
                break;
            }
            self.events.pop();
            self.depart(p as usize, t);
        }
        self.step_walkers();

        let mut counts = vec![0u32; self.map.crossings.len()];
        for w in &self.walkers {
            if w.entered && w.i < w.steps.len() {
                if let Some(ci) = self.map.foot.edges[(w.steps[w.i] >> 1) as usize].crossing {
                    counts[ci] += 1;
                }
            }
        }
        self.traffic.set_occupied(&counts);
        self.stats.crossing_violations += self.traffic.crossing_violations() as u64;

        self.traffic.step();
        let t_after = self.traffic.tick;
        for vid in self.traffic.take_finished() {
            if let Some(p) = self.vehicle_person.remove(&vid) {
                self.arrive(p as usize, t_after);
            }
        }
    }

    pub fn clock_seconds(&self) -> f64 {
        seconds(self.traffic.tick)
    }

    /// Positions of everything that moves, for viewers and tests.
    pub fn positions(&self) -> Vec<EntityPos> {
        let mut out = Vec::new();
        for (e, v) in self.traffic.all_vehicles() {
            let ed = &self.map.graph.edges[e as usize];
            let (a, b) = (
                &self.map.graph.nodes[ed.from as usize],
                &self.map.graph.nodes[ed.to as usize],
            );
            let (dx, dy) = ((b.x - a.x) / ed.length, (b.y - a.y) / ed.length);
            let c = (v.pos - self.traffic.idm.length / 2.0).max(0.0);
            out.push(EntityPos {
                kind: EntityKind::Vehicle,
                id: v.id,
                x: (a.x + dx * c) as f32,
                y: (a.y + dy * c) as f32,
                heading: dy.atan2(dx) as f32,
            });
        }
        for w in &self.walkers {
            let token = w.steps[w.i.min(w.steps.len() - 1)];
            let ed = &self.map.foot.edges[(token >> 1) as usize];
            let (from, to) = if token & 1 == 0 {
                (ed.a, ed.b)
            } else {
                (ed.b, ed.a)
            };
            let (a, b) = (
                &self.map.graph.nodes[from as usize],
                &self.map.graph.nodes[to as usize],
            );
            let f = (w.s / ed.length).clamp(0.0, 1.0);
            out.push(EntityPos {
                kind: EntityKind::Person,
                id: w.person,
                x: (a.x + (b.x - a.x) * f) as f32,
                y: (a.y + (b.y - a.y) * f) as f32,
                heading: (b.y - a.y).atan2(b.x - a.x) as f32,
            });
        }
        out
    }

    /// Hash of the whole city state, for determinism checks (FNV-1a).
    pub fn state_hash(&self) -> u64 {
        let mut h: u64 = self.traffic.state_hash();
        let mut feed = |x: u64| {
            for b in x.to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        };
        for w in &self.walkers {
            feed(w.person as u64);
            feed(w.i as u64);
            feed(w.s.to_bits());
        }
        for p in &self.people {
            feed(p.stage as u64);
            feed(p.est_s.to_bits());
        }
        feed(self.stats.trips_done);
        feed(self.stats.late_arrivals);
        feed(self.events.len() as u64);
        h
    }
}
