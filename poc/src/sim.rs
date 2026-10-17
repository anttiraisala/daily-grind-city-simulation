//! Core of the proof of concept: a deterministic, fixed-step commuting simulation.
//!
//! What it demonstrates (a deliberately small slice of `docs/design-plan.md`):
//! - struct-of-arrays agent storage, one RNG per agent
//! - event-driven decisions: idle agents cost nothing between departures
//! - a 24-hour clock with schedules drawn from distributions (7:00 / 8:00 / 10:00
//!   starts, ~8 h days, lunch-less), a per-agent punctuality trait, rare skipped days
//! - learned commute estimates (agents leave earlier after slow trips)
//! - cars slowing down with road occupancy (Greenshields), bikes and walkers unaffected
//! - agents inside buildings take no space; only moving agents exist on the map
//! - a command queue applied at tick boundaries and logged against the tick
//!
//! Not modelled yet: intersections, traffic lights, IDM car following, crashes,
//! free movement, emergency services, needs and moods.

use crate::rng::Rng;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub const DAY: u64 = 86_400;
/// Distance between neighbouring intersections, in meters.
pub const BLOCK_M: f32 = 100.0;
/// Seconds of simulated time per tick (the real design uses 0.1 s).
const DT: f32 = 1.0;
/// Road space taken by one car including the gap to the next one.
const VEHICLE_SPACE_M: f32 = 7.5;
/// Number of cars on one directed edge at which traffic stands still.
pub const JAM_VEHICLES: f32 = BLOCK_M / VEHICLE_SPACE_M;
const MIN_SPEED_FACTOR: f32 = 0.05;
const LATE_TOLERANCE_S: u64 = 60;
const SKIP_PROBABILITY: f32 = 0.01;
const FREE_FLOW_FUDGE: f32 = 1.2;
const LEARNING_RATE: f32 = 0.3;

const IDLE_HOME: u8 = 0;
const TO_TARGET: u8 = 1;
const AT_TARGET: u8 = 2;
const TO_HOME: u8 = 3;

const KIND_WORKER: u8 = 0;
const KIND_ERRAND: u8 = 1;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Car,
    Bike,
    Walk,
}

impl Mode {
    fn from_u8(v: u8) -> Mode {
        match v {
            0 => Mode::Car,
            1 => Mode::Bike,
            _ => Mode::Walk,
        }
    }
    fn speed(self) -> f32 {
        match self {
            Mode::Car => 13.9,
            Mode::Bike => 5.0,
            Mode::Walk => 1.4,
        }
    }
    /// Sideways offset from the street centre line, so directions and modes are visible.
    fn lane_offset(self) -> f32 {
        match self {
            Mode::Car => 2.0,
            Mode::Bike => 3.5,
            Mode::Walk => 5.0,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Mode::Car => "car",
            Mode::Bike => "bike",
            Mode::Walk => "walk",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub agents: usize,
    pub seed: u64,
    /// Residents per intersection: the city grid grows with the population.
    pub agents_per_node: f32,
    pub car_share: f32,
    pub worker_share: f32,
    /// Share of workers whose job is in the downtown cluster.
    pub core_share: f32,
    /// Standard deviation of the downtown cluster, as a fraction of the city width.
    pub core_spread: f32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            agents: 100_000,
            seed: 1,
            agents_per_node: 40.0,
            car_share: 0.7,
            worker_share: 0.8,
            core_share: 0.85,
            core_spread: 0.08,
        }
    }
}

/// A manual change. Applied at a tick boundary and logged against that tick.
#[derive(Clone, Debug)]
pub enum Command {
    /// Postpone an idle agent's next departure.
    Delay { agent: u32, seconds: u32 },
}

#[derive(Clone, Copy, Default, Debug)]
pub struct HourStats {
    pub moving_ticks: u64,
    pub car_ticks: u64,
    pub speed_permille: u64,
    pub arrivals: u64,
    pub late: u64,
}

#[derive(Clone, Default, Debug)]
pub struct Stats {
    pub hourly: [HourStats; 24],
    pub trips_done: u64,
    pub late_today: u32,
    pub cars_moving: u32,
    pub avg_speed_permille: u32,
    /// Largest number of cars seen on one directed edge so far.
    pub peak_occupancy: u16,
}

/// Visible rectangle in world meters.
pub struct View {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

pub struct Composition {
    pub workers: usize,
    pub errands: usize,
    pub cars: usize,
    pub bikes: usize,
    pub walkers: usize,
}

pub struct Sim {
    pub cfg: Config,
    pub w: u32,
    pub h: u32,
    pub time: u64,
    n: usize,
    // Agent columns (struct of arrays).
    kind: Vec<u8>,
    mode: Vec<u8>,
    state: Vec<u8>,
    home_x: Vec<u16>,
    home_y: Vec<u16>,
    tgt_x: Vec<u16>,
    tgt_y: Vec<u16>,
    cx: Vec<u16>,
    cy: Vec<u16>,
    progress: Vec<f32>,
    punctuality: Vec<f32>,
    est: Vec<f32>,
    plan_arrive: Vec<u64>,
    plan_leave: Vec<u64>,
    trip_start: Vec<u64>,
    next_event: Vec<u64>,
    rng: Vec<Rng>,
    // Dynamic structures.
    moving: Vec<u32>,
    events: BinaryHeap<Reverse<(u64, u32)>>,
    occ: Vec<u16>,
    scratch: Vec<u32>,
    /// Car-seconds spent in slow traffic (speed factor below 0.5), per intersection.
    pub slow: Vec<u32>,
    pub stats: Stats,
    pending: Vec<Command>,
    pub command_log: Vec<(u64, Command)>,
}

/// Direction of the next step from (cx, cy) towards (tx, ty): dx, dy, direction index.
/// Streets are travelled along x first, then along y.
#[inline]
fn step_dir(cx: u16, cy: u16, tx: u16, ty: u16) -> Option<(i32, i32, usize)> {
    if cx < tx {
        Some((1, 0, 0))
    } else if cx > tx {
        Some((-1, 0, 1))
    } else if cy < ty {
        Some((0, 1, 2))
    } else if cy > ty {
        Some((0, -1, 3))
    } else {
        None
    }
}

pub fn hhmm(secs: u64) -> String {
    format!("{:02}:{:02}", (secs % DAY) / 3600, (secs % 3600) / 60)
}

fn clamp_node(v: f32, max: u32) -> u16 {
    v.round().clamp(0.0, (max - 1) as f32) as u16
}

impl Sim {
    pub fn new(cfg: Config) -> Sim {
        let n = cfg.agents.max(10);
        let side = ((n as f32 / cfg.agents_per_node).sqrt().ceil() as u32).max(8);
        let (w, h) = (side, side);
        let mut sim = Sim {
            cfg: cfg.clone(),
            w,
            h,
            time: 0,
            n,
            kind: Vec::with_capacity(n),
            mode: Vec::with_capacity(n),
            state: vec![IDLE_HOME; n],
            home_x: Vec::with_capacity(n),
            home_y: Vec::with_capacity(n),
            tgt_x: Vec::with_capacity(n),
            tgt_y: Vec::with_capacity(n),
            cx: vec![0; n],
            cy: vec![0; n],
            progress: vec![0.0; n],
            punctuality: Vec::with_capacity(n),
            est: Vec::with_capacity(n),
            plan_arrive: vec![0; n],
            plan_leave: vec![0; n],
            trip_start: vec![0; n],
            next_event: vec![u64::MAX; n],
            rng: Vec::with_capacity(n),
            moving: Vec::new(),
            events: BinaryHeap::new(),
            occ: vec![0; (w * h * 4) as usize],
            scratch: Vec::new(),
            slow: vec![0; (w * h) as usize],
            stats: Stats {
                avg_speed_permille: 1000,
                ..Stats::default()
            },
            pending: Vec::new(),
            command_log: Vec::new(),
        };

        for i in 0..n {
            let mut r = Rng::for_agent(cfg.seed, i as u64);
            let hx = r.below(w) as u16;
            let hy = r.below(h) as u16;
            let kind = if r.next_f32() < cfg.worker_share {
                KIND_WORKER
            } else {
                KIND_ERRAND
            };
            let (mut tx, mut ty) = if kind == KIND_WORKER {
                if r.next_f32() < cfg.core_share {
                    // Downtown job: clustered around the city centre.
                    let sigma = cfg.core_spread as f64 * w as f64;
                    let x = w as f64 / 2.0 + sigma * r.normal();
                    let y = h as f64 / 2.0 + sigma * r.normal();
                    (clamp_node(x as f32, w), clamp_node(y as f32, h))
                } else {
                    (r.below(w) as u16, r.below(h) as u16)
                }
            } else {
                // Errand: a regular shop within a few blocks of home.
                let x = hx as i32 + r.below(17) as i32 - 8;
                let y = hy as i32 + r.below(17) as i32 - 8;
                (
                    x.clamp(0, w as i32 - 1) as u16,
                    y.clamp(0, h as i32 - 1) as u16,
                )
            };
            if tx == hx && ty == hy {
                if tx + 1 < w as u16 {
                    tx += 1;
                } else {
                    tx -= 1;
                }
                ty = ty.min(h as u16 - 1);
            }
            let dist = (tx as i32 - hx as i32).abs() + (ty as i32 - hy as i32).abs();
            let mode = if dist <= 4 {
                Mode::Walk
            } else if r.next_f32() < cfg.car_share {
                Mode::Car
            } else {
                Mode::Bike
            };
            let punctuality = r.next_f32().sqrt();
            let est = dist as f32 * BLOCK_M / mode.speed() * FREE_FLOW_FUDGE;

            sim.kind.push(kind);
            sim.mode.push(mode as u8);
            sim.home_x.push(hx);
            sim.home_y.push(hy);
            sim.tgt_x.push(tx);
            sim.tgt_y.push(ty);
            sim.punctuality.push(punctuality);
            sim.est.push(est);
            sim.rng.push(r);
        }
        for i in 0..n {
            sim.plan_next_trip(i, 0);
        }
        sim
    }

    pub fn agent_count(&self) -> usize {
        self.n
    }

    pub fn composition(&self) -> Composition {
        let mut c = Composition {
            workers: 0,
            errands: 0,
            cars: 0,
            bikes: 0,
            walkers: 0,
        };
        for i in 0..self.n {
            if self.kind[i] == KIND_WORKER {
                c.workers += 1;
            } else {
                c.errands += 1;
            }
            match Mode::from_u8(self.mode[i]) {
                Mode::Car => c.cars += 1,
                Mode::Bike => c.bikes += 1,
                Mode::Walk => c.walkers += 1,
            }
        }
        c
    }

    /// Queue a manual change; it takes effect at the start of the next tick.
    pub fn submit(&mut self, c: Command) {
        self.pending.push(c);
    }

    pub fn run_ticks(&mut self, ticks: u64) {
        for _ in 0..ticks {
            self.step();
        }
    }

    pub fn moving_count(&self) -> usize {
        self.moving.len()
    }

    // ------------------------------------------------------------------ decisions

    fn schedule(&mut self, i: usize, t: u64) {
        self.next_event[i] = t;
        self.events.push(Reverse((t, i as u32)));
    }

    /// Draw the plan for the next outing (first possible day: `earliest_day`) and
    /// schedule the departure. Workers occasionally skip a whole day.
    fn plan_next_trip(&mut self, i: usize, earliest_day: u64) {
        let mut r = self.rng[i];
        let mut day = earliest_day;
        let worker = self.kind[i] == KIND_WORKER;
        if worker {
            while r.next_f32() < SKIP_PROBABILITY {
                day += 1;
            }
        }
        let (start_s, dur_s) = if worker {
            let u = r.next_f32();
            let (mean, sd) = if u < 0.2 {
                (7.0 * 3600.0, 1200.0)
            } else if u < 0.7 {
                (8.0 * 3600.0, 1800.0)
            } else {
                (10.0 * 3600.0, 1800.0)
            };
            let s = (mean + sd * r.normal()).clamp(4.0 * 3600.0, 14.0 * 3600.0);
            let d = (8.0 * 3600.0 + 0.7 * 3600.0 * r.normal()).clamp(4.0 * 3600.0, 11.0 * 3600.0);
            (s, d)
        } else {
            (
                36_000.0 + 21_600.0 * r.next_f64(),
                1_800.0 + 3_600.0 * r.next_f64(),
            )
        };
        let arrive = day * DAY + start_s as u64;
        let leave = arrive + dur_s as u64;
        // Punctual agents plan a bigger safety margin and vary less.
        let p = self.punctuality[i] as f64;
        let margin = 0.9 + 0.6 * p;
        let noise = 300.0 * (1.0 - p) * r.normal();
        let depart_f = arrive as f64 - self.est[i] as f64 * margin + noise;
        let depart = (depart_f.max(0.0) as u64).max(self.time + 1);
        self.rng[i] = r;
        self.plan_arrive[i] = arrive;
        self.plan_leave[i] = leave;
        self.schedule(i, depart);
    }

    fn depart(&mut self, i: usize) {
        match self.state[i] {
            IDLE_HOME => {
                self.state[i] = TO_TARGET;
                self.cx[i] = self.home_x[i];
                self.cy[i] = self.home_y[i];
            }
            AT_TARGET => {
                self.state[i] = TO_HOME;
                self.cx[i] = self.tgt_x[i];
                self.cy[i] = self.tgt_y[i];
            }
            _ => return,
        }
        self.progress[i] = 0.0;
        self.trip_start[i] = self.time;
        self.next_event[i] = u64::MAX;
        self.moving.push(i as u32);
    }

    fn on_arrive(&mut self, i: usize) {
        let now = self.time;
        // Learn from the trip: move the commute estimate towards what actually happened.
        let actual = (now - self.trip_start[i]) as f32;
        self.est[i] += LEARNING_RATE * (actual - self.est[i]);
        self.stats.trips_done += 1;
        let hour = ((now % DAY) / 3600) as usize;
        self.stats.hourly[hour].arrivals += 1;
        if self.state[i] == TO_TARGET {
            self.state[i] = AT_TARGET;
            if self.kind[i] == KIND_WORKER && now > self.plan_arrive[i] + LATE_TOLERANCE_S {
                self.stats.hourly[hour].late += 1;
                self.stats.late_today += 1;
            }
            let mut r = self.rng[i];
            let extra = (120.0 * r.normal()).max(0.0) as u64;
            self.rng[i] = r;
            let leave = (self.plan_leave[i] + extra).max(now + 1);
            self.schedule(i, leave);
        } else {
            self.state[i] = IDLE_HOME;
            let day = self.plan_arrive[i] / DAY + 1;
            self.plan_next_trip(i, day);
        }
    }

    fn apply_commands(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let cmds = std::mem::take(&mut self.pending);
        for c in cmds {
            match c {
                Command::Delay { agent, seconds } => {
                    let i = agent as usize;
                    if i < self.n && (self.state[i] == IDLE_HOME || self.state[i] == AT_TARGET) {
                        let t = self.next_event[i].max(self.time) + seconds as u64;
                        self.schedule(i, t);
                    }
                }
            }
            self.command_log.push((self.time, c));
        }
    }

    // ------------------------------------------------------------------ one tick

    /// Advance the world by one fixed step (one simulated second).
    pub fn step(&mut self) {
        if self.time % DAY == 0 {
            self.stats.late_today = 0;
        }
        self.apply_commands();

        // 1. Departures that are due.
        while let Some(&Reverse((t, i))) = self.events.peek() {
            if t > self.time {
                break;
            }
            self.events.pop();
            let i = i as usize;
            if self.next_event[i] == t {
                self.depart(i);
            }
        }

        // 2. Count cars on every directed edge. Done before anybody moves, so the
        //    result does not depend on the processing order.
        for o in self.occ.iter_mut() {
            *o = 0;
        }
        let w = self.w as usize;
        for &a in &self.moving {
            let i = a as usize;
            if self.mode[i] == Mode::Car as u8 {
                let (tx, ty) = if self.state[i] == TO_TARGET {
                    (self.tgt_x[i], self.tgt_y[i])
                } else {
                    (self.home_x[i], self.home_y[i])
                };
                if let Some((_, _, d)) = step_dir(self.cx[i], self.cy[i], tx, ty) {
                    let e = (((self.cy[i] as usize) * w + self.cx[i] as usize) << 2) | d;
                    self.occ[e] += 1;
                    if self.occ[e] > self.stats.peak_occupancy {
                        self.stats.peak_occupancy = self.occ[e];
                    }
                }
            }
        }

        // 3. Move everybody who is on the road.
        let hour = ((self.time % DAY) / 3600) as usize;
        let mut arrived = std::mem::take(&mut self.scratch);
        let n_moving = self.moving.len();
        let mut keep = 0usize;
        let mut car_ticks = 0u64;
        let mut speed_sum = 0u64;
        for k in 0..n_moving {
            let a = self.moving[k];
            let i = a as usize;
            let (tx, ty) = if self.state[i] == TO_TARGET {
                (self.tgt_x[i], self.tgt_y[i])
            } else {
                (self.home_x[i], self.home_y[i])
            };
            let (dx, dy, d) = match step_dir(self.cx[i], self.cy[i], tx, ty) {
                Some(v) => v,
                None => {
                    arrived.push(a);
                    continue;
                }
            };
            let mode = Mode::from_u8(self.mode[i]);
            let node = (self.cy[i] as usize) * w + self.cx[i] as usize;
            let mut factor = 1.0f32;
            if mode == Mode::Car {
                // Other cars on the same directed edge slow this one down.
                let others = (self.occ[(node << 2) | d] - 1) as f32;
                factor = (1.0 - others / JAM_VEHICLES).max(MIN_SPEED_FACTOR);
                car_ticks += 1;
                speed_sum += (factor * 1000.0) as u64;
                if factor < 0.5 {
                    self.slow[node] += 1;
                }
            }
            let mut p = self.progress[i] + mode.speed() * factor * DT / BLOCK_M;
            if p >= 1.0 {
                p -= 1.0;
                self.cx[i] = (self.cx[i] as i32 + dx) as u16;
                self.cy[i] = (self.cy[i] as i32 + dy) as u16;
                if self.cx[i] == tx && self.cy[i] == ty {
                    self.progress[i] = 0.0;
                    arrived.push(a);
                    continue;
                }
            }
            self.progress[i] = p;
            self.moving[keep] = a;
            keep += 1;
        }
        self.moving.truncate(keep);

        let hs = &mut self.stats.hourly[hour];
        hs.moving_ticks += n_moving as u64;
        hs.car_ticks += car_ticks;
        hs.speed_permille += speed_sum;
        self.stats.cars_moving = car_ticks as u32;
        self.stats.avg_speed_permille = speed_sum.checked_div(car_ticks).map_or(1000, |v| v as u32);

        // 4. Arrivals make new decisions (schedule the way back, learn, plan tomorrow).
        for a in arrived.drain(..) {
            self.on_arrive(a as usize);
        }
        self.scratch = arrived;

        self.time += 1;
    }

    // ------------------------------------------------------------------ read side

    fn dest(&self, i: usize) -> (u16, u16) {
        if self.state[i] == TO_TARGET {
            (self.tgt_x[i], self.tgt_y[i])
        } else {
            (self.home_x[i], self.home_y[i])
        }
    }

    /// World position in meters of a moving agent, offset to its side of the street.
    fn world_pos(&self, i: usize) -> (f32, f32) {
        let (tx, ty) = self.dest(i);
        let (dx, dy, _) = step_dir(self.cx[i], self.cy[i], tx, ty).unwrap_or((0, 0, 0));
        let off = Mode::from_u8(self.mode[i]).lane_offset();
        let x = (self.cx[i] as f32 + dx as f32 * self.progress[i]) * BLOCK_M - dy as f32 * off;
        let y = (self.cy[i] as f32 + dy as f32 * self.progress[i]) * BLOCK_M + dx as f32 * off;
        (x, y)
    }

    /// Binary snapshot of the moving agents inside `view`.
    ///
    /// Header (little endian): time u64, moving_total u32, records u32, cars u32,
    /// avg_speed_permille u32, late_today u32, trips_done u32 (32 bytes).
    /// Record (13 bytes): id u32, x f32, y f32, mode u8.
    /// If more than `cap` agents are inside the view, a stable subset (by id) is sent.
    pub fn snapshot(&self, view: &View, cap: usize) -> Vec<u8> {
        let inside = |x: f32, y: f32| x >= view.x0 && x <= view.x1 && y >= view.y0 && y <= view.y1;
        let mut count = 0usize;
        for &a in &self.moving {
            let (x, y) = self.world_pos(a as usize);
            if inside(x, y) {
                count += 1;
            }
        }
        let stride = if count > cap {
            count.div_ceil(cap) as u32
        } else {
            1
        };
        let mut out = Vec::with_capacity(32 + count.min(cap * 2) * 13);
        out.extend_from_slice(&self.time.to_le_bytes());
        out.extend_from_slice(&(self.moving.len() as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // records, patched below
        out.extend_from_slice(&self.stats.cars_moving.to_le_bytes());
        out.extend_from_slice(&self.stats.avg_speed_permille.to_le_bytes());
        out.extend_from_slice(&self.stats.late_today.to_le_bytes());
        out.extend_from_slice(&(self.stats.trips_done.min(u32::MAX as u64) as u32).to_le_bytes());
        let mut records = 0u32;
        for &a in &self.moving {
            if a % stride != 0 {
                continue;
            }
            let i = a as usize;
            let (x, y) = self.world_pos(i);
            if inside(x, y) {
                out.extend_from_slice(&a.to_le_bytes());
                out.extend_from_slice(&x.to_le_bytes());
                out.extend_from_slice(&y.to_le_bytes());
                out.push(self.mode[i]);
                records += 1;
            }
        }
        out[12..16].copy_from_slice(&records.to_le_bytes());
        out
    }

    /// Slow-traffic heat map, one byte per intersection (square-root scaled).
    pub fn heat_bytes(&self) -> Vec<u8> {
        let max = self.slow.iter().copied().max().unwrap_or(0);
        if max == 0 {
            return vec![0; self.slow.len()];
        }
        self.slow
            .iter()
            .map(|&v| (255.0 * (v as f32 / max as f32).sqrt()) as u8)
            .collect()
    }

    pub fn agent_json(&self, id: usize) -> Option<String> {
        if id >= self.n {
            return None;
        }
        let state = match self.state[id] {
            IDLE_HOME => "at home",
            TO_TARGET => "travelling to destination",
            AT_TARGET => "at destination",
            _ => "travelling home",
        };
        let kind = if self.kind[id] == KIND_WORKER {
            "worker"
        } else {
            "errand-runner"
        };
        let moving = self.state[id] == TO_TARGET || self.state[id] == TO_HOME;
        let next = if self.next_event[id] == u64::MAX {
            "-".to_string()
        } else {
            format!(
                "day {} {}",
                self.next_event[id] / DAY + 1,
                hhmm(self.next_event[id])
            )
        };
        Some(format!(
            "{{\"id\":{},\"kind\":\"{}\",\"mode\":\"{}\",\"state\":\"{}\",\"moving\":{},\
\"home\":[{},{}],\"destination\":[{},{}],\"punctuality\":{:.2},\"commute_estimate_min\":{:.1},\
\"planned_arrival\":\"day {} {}\",\"planned_leave\":\"day {} {}\",\"next_departure\":\"{}\"}}",
            id,
            kind,
            Mode::from_u8(self.mode[id]).name(),
            state,
            moving,
            self.home_x[id],
            self.home_y[id],
            self.tgt_x[id],
            self.tgt_y[id],
            self.punctuality[id],
            self.est[id] / 60.0,
            self.plan_arrive[id] / DAY + 1,
            hhmm(self.plan_arrive[id]),
            self.plan_leave[id] / DAY + 1,
            hhmm(self.plan_leave[id]),
            next
        ))
    }

    /// Hash of the full simulation state. Equal hashes mean identical runs.
    pub fn state_hash(&self) -> u64 {
        fn mix(h: &mut u64, v: u64) {
            *h = (*h ^ v).wrapping_mul(0x0000_0100_0000_01B3);
            *h ^= *h >> 29;
        }
        let mut h = 0xCBF2_9CE4_8422_2325u64;
        mix(&mut h, self.time);
        mix(&mut h, self.stats.trips_done);
        mix(&mut h, self.stats.peak_occupancy as u64);
        for s in &self.stats.hourly {
            mix(&mut h, s.moving_ticks);
            mix(&mut h, s.car_ticks);
            mix(&mut h, s.speed_permille);
            mix(&mut h, s.arrivals);
            mix(&mut h, s.late);
        }
        for i in 0..self.n {
            mix(&mut h, self.state[i] as u64);
            mix(&mut h, ((self.cx[i] as u64) << 16) | self.cy[i] as u64);
            mix(&mut h, self.progress[i].to_bits() as u64);
            mix(&mut h, self.est[i].to_bits() as u64);
            mix(&mut h, self.next_event[i]);
            mix(&mut h, self.plan_arrive[i]);
            mix(&mut h, self.rng[i].0);
        }
        for &s in &self.slow {
            mix(&mut h, s as u64);
        }
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small(seed: u64) -> Sim {
        Sim::new(Config {
            agents: 2_000,
            seed,
            ..Config::default()
        })
    }

    #[test]
    fn same_seed_gives_identical_run() {
        let mut a = small(42);
        let mut b = small(42);
        a.run_ticks(DAY);
        b.run_ticks(DAY);
        assert_eq!(a.state_hash(), b.state_hash());
        assert!(a.stats.trips_done > 1_000);
    }

    #[test]
    fn different_seed_gives_different_run() {
        let mut a = small(1);
        let mut b = small(2);
        a.run_ticks(DAY);
        b.run_ticks(DAY);
        assert_ne!(a.state_hash(), b.state_hash());
    }

    #[test]
    fn rush_hour_exists_and_nights_are_empty() {
        let mut s = small(3);
        s.run_ticks(DAY);
        let night = s.stats.hourly[3].moving_ticks;
        let morning = s.stats.hourly[8].moving_ticks;
        assert!(
            morning > night * 10 + 1_000,
            "morning {morning} night {night}"
        );
    }

    #[test]
    fn positions_stay_inside_the_city() {
        let mut s = small(4);
        s.run_ticks(2 * DAY);
        for i in 0..s.n {
            assert!((s.cx[i] as u32) < s.w && (s.cy[i] as u32) < s.h);
        }
    }

    #[test]
    fn delay_command_is_applied_and_logged_at_a_tick_boundary() {
        let mut s = small(5);
        let agent = (0..s.n).find(|&i| s.state[i] == IDLE_HOME).unwrap();
        let before = s.next_event[agent];
        s.submit(Command::Delay {
            agent: agent as u32,
            seconds: 3_600,
        });
        s.step();
        assert_eq!(s.next_event[agent], before + 3_600);
        assert_eq!(s.command_log.len(), 1);
        assert_eq!(s.command_log[0].0, 0);
    }

    #[test]
    fn delay_changes_the_run_but_stays_deterministic() {
        let run = || {
            let mut s = small(6);
            let worker = (0..s.n).find(|&i| s.kind[i] == KIND_WORKER).unwrap() as u32;
            s.submit(Command::Delay {
                agent: worker,
                seconds: 5_000,
            });
            s.run_ticks(DAY);
            s.state_hash()
        };
        let plain = {
            let mut s = small(6);
            s.run_ticks(DAY);
            s.state_hash()
        };
        assert_eq!(run(), run());
        assert_ne!(run(), plain);
    }

    #[test]
    fn snapshot_header_and_record_count_match() {
        let mut s = small(8);
        s.run_ticks(8 * 3600);
        let view = View {
            x0: -1e9,
            y0: -1e9,
            x1: 1e9,
            y1: 1e9,
        };
        let snap = s.snapshot(&view, 1_000_000);
        let records = u32::from_le_bytes(snap[12..16].try_into().unwrap()) as usize;
        assert_eq!(snap.len(), 32 + records * 13);
        assert_eq!(records, s.moving_count());
    }
}
