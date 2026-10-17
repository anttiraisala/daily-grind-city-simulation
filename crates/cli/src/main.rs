//! Headless runner for the city simulation.
//!
//! `citysim bench` runs a synthetic grid city with traffic lights and reports how many
//! vehicle updates per second the core achieves. Scenario and map loading arrive with
//! the rest of Phase 1 (see docs/roadmap.md).

use citysim_core::graph::{EdgeId, EdgeKind, Graph, Signal};
use citysim_core::idm::IdmParams;
use citysim_core::rng::Rng;
use citysim_core::traffic::Traffic;
use citysim_core::{clock, TICK_SECONDS};
use std::time::Instant;

const HELP: &str = "citysim: headless city simulation runner

USAGE
  citysim bench [--size N] [--hours H] [--rate R] [--seed S]
  citysim help

OPTIONS (bench)
  --size N    grid of N x N intersections, 100 m apart (default 8)
  --hours H   simulated hours (default 1)
  --rate R    new vehicles per second, whole city (default 2.0)
  --seed S    random seed (default 1)
";

struct Grid {
    graph: Graph,
    /// Edge id of the segment from node (x, y) going east, west, south, north.
    east: Vec<Option<EdgeId>>,
    west: Vec<Option<EdgeId>>,
    south: Vec<Option<EdgeId>>,
    north: Vec<Option<EdgeId>>,
    n: usize,
}

fn build_grid(n: usize) -> Grid {
    let mut g = Graph::new();
    let mut node = vec![0u32; n * n];
    for y in 0..n {
        for x in 0..n {
            node[y * n + x] = g.add_node(x as f64 * 100.0, y as f64 * 100.0, 0);
        }
    }
    let mut grid = Grid {
        graph: Graph::new(),
        east: vec![None; n * n],
        west: vec![None; n * n],
        south: vec![None; n * n],
        north: vec![None; n * n],
        n,
    };
    for y in 0..n {
        for x in 0..n {
            let i = y * n + x;
            if x + 1 < n {
                grid.east[i] = Some(g.add_edge(node[i], node[i + 1], 13.9, 1, EdgeKind::Road));
                grid.west[i + 1] = Some(g.add_edge(node[i + 1], node[i], 13.9, 1, EdgeKind::Road));
            }
            if y + 1 < n {
                grid.south[i] = Some(g.add_edge(node[i], node[i + n], 13.9, 1, EdgeKind::Road));
                grid.north[i + n] = Some(g.add_edge(node[i + n], node[i], 13.9, 1, EdgeKind::Road));
            }
        }
    }
    // Fixed-time signals: horizontal approaches green first, vertical approaches second.
    for i in 0..n * n {
        for (edge, offset) in [(grid.east[i], 0.0), (grid.west[i], 0.0)] {
            if let Some(e) = edge {
                g.set_signal(
                    e,
                    Signal {
                        cycle_s: 60.0,
                        offset_s: offset,
                        green_s: 28.0,
                    },
                );
            }
        }
        for (edge, offset) in [(grid.south[i], 30.0), (grid.north[i], 30.0)] {
            if let Some(e) = edge {
                g.set_signal(
                    e,
                    Signal {
                        cycle_s: 60.0,
                        offset_s: offset,
                        green_s: 28.0,
                    },
                );
            }
        }
    }
    grid.graph = g;
    grid
}

/// Route from one intersection to another: along x first, then along y.
fn route(grid: &Grid, from: (usize, usize), to: (usize, usize)) -> Vec<EdgeId> {
    let n = grid.n;
    let (mut x, mut y) = from;
    let mut r = Vec::new();
    while x != to.0 {
        let i = y * n + x;
        if to.0 > x {
            r.push(grid.east[i].expect("edge exists"));
            x += 1;
        } else {
            r.push(grid.west[i].expect("edge exists"));
            x -= 1;
        }
    }
    while y != to.1 {
        let i = y * n + x;
        if to.1 > y {
            r.push(grid.south[i].expect("edge exists"));
            y += 1;
        } else {
            r.push(grid.north[i].expect("edge exists"));
            y -= 1;
        }
    }
    r
}

fn arg<T: std::str::FromStr>(args: &[String], name: &str, default: T) -> T {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn bench(args: &[String]) {
    let size: usize = arg(args, "--size", 8).max(2);
    let hours: f64 = arg(args, "--hours", 1.0);
    let rate: f64 = arg(args, "--rate", 2.0);
    let seed: u64 = arg(args, "--seed", 1);

    let grid = build_grid(size);
    let mut traffic = Traffic::new(grid.graph.clone(), IdmParams::default()).expect("valid graph");
    let mut rng = Rng::new(seed);
    let ticks = (hours * 3600.0 / TICK_SECONDS) as u64;
    let spawn_p = rate * TICK_SECONDS;

    let started = Instant::now();
    let mut vehicle_updates: u64 = 0;
    for _ in 0..ticks {
        let mut p = spawn_p;
        while p > 0.0 {
            if rng.next_f64() < p.min(1.0) {
                let a = (
                    rng.below(size as u64) as usize,
                    rng.below(size as u64) as usize,
                );
                let b = (
                    rng.below(size as u64) as usize,
                    rng.below(size as u64) as usize,
                );
                if a != b {
                    traffic.spawn(route(&grid, a, b));
                }
            }
            p -= 1.0;
        }
        traffic.step();
        vehicle_updates += traffic.vehicle_count() as u64;
    }
    let wall = started.elapsed().as_secs_f64();

    let s = &traffic.stats;
    println!(
        "{size}x{size} intersections, {hours} h simulated ({}), seed {seed}",
        clock::hhmm(traffic.tick)
    );
    println!(
        "spawned {}, blocked at entry {}, finished {}, on the road now {}",
        s.spawned,
        s.spawn_blocked,
        s.finished,
        traffic.vehicle_count()
    );
    println!("safety clamps: {} (must be 0)", s.clamps);
    println!(
        "wall time {wall:.2} s, {:.1}x real time, {:.2} million vehicle updates per second",
        hours * 3600.0 / wall,
        vehicle_updates as f64 / wall / 1e6
    );
    println!("state hash {:016x}", traffic.state_hash());

    let mut worst: Vec<(usize, f64)> = s.slow_seconds.iter().copied().enumerate().collect();
    worst.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    println!("slowest segments (vehicle-seconds below 20 % of the limit):");
    for (e, secs) in worst.iter().take(5) {
        let edge = &traffic.graph.edges[*e];
        let n = &traffic.graph.nodes[edge.to as usize];
        println!(
            "  segment {e} towards ({:.0}, {:.0}): {secs:.0} s",
            n.x, n.y
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("bench") => bench(&args[1..]),
        _ => print!("{HELP}"),
    }
}
