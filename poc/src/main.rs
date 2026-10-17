//! City simulation proof of concept: headless benchmark, determinism check, and a
//! server that streams the running simulation to a browser (also a phone on the
//! same network).

// Plain `%` keeps the code building on older toolchains.
#![allow(clippy::manual_is_multiple_of)]

mod rng;
mod server;
mod sim;

use sim::{Config, Sim, DAY};
use std::process::exit;
use std::time::Instant;

const HELP: &str = "\
city-poc: deterministic commuting simulation with a browser viewer

USAGE
  city-poc bench [options]    run headless and print throughput, hash and an hourly table
  city-poc check [options]    run twice and compare state hashes (determinism check)
  city-poc serve [options]    run live and serve the viewer in a browser

OPTIONS
  --agents N       number of residents (default 100000)
  --seed S         run seed (default 1)
  --days D         simulated days for bench/check (default 1)
  --density X      residents per intersection; sets the city size (default 40)
  --car-share X    share of long commuters who drive, 0..1 (default 0.7)
  --core-share X   share of workers with a downtown job, 0..1 (default 0.85)
  --core-spread X  downtown size as a fraction of city width (default 0.08)
  --speed X        serve: simulated seconds per real second (default 600)
  --port P         serve: port (default 8080)
  --lan            serve: listen on the local network (for a phone); default is this computer only
  --bind ADDR      serve: full listen address, e.g. 0.0.0.0:9000
  --token T        serve: access token (default: random, printed in the URL)
";

struct Opts {
    cfg: Config,
    days: u64,
    speed: f64,
    port: u16,
    lan: bool,
    bind: Option<String>,
    token: Option<String>,
}

fn fail(msg: &str) -> ! {
    eprintln!("error: {msg}\n\n{HELP}");
    exit(2);
}

fn parse(args: &[String]) -> Opts {
    let mut o = Opts {
        cfg: Config::default(),
        days: 1,
        speed: 600.0,
        port: 8080,
        lan: false,
        bind: None,
        token: None,
    };
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = |name: &str| -> String {
            it.next()
                .cloned()
                .unwrap_or_else(|| fail(&format!("{name} needs a value")))
        };
        match flag.as_str() {
            "--agents" => {
                o.cfg.agents = value(flag).parse().unwrap_or_else(|_| fail("bad --agents"))
            }
            "--seed" => o.cfg.seed = value(flag).parse().unwrap_or_else(|_| fail("bad --seed")),
            "--days" => o.days = value(flag).parse().unwrap_or_else(|_| fail("bad --days")),
            "--density" => {
                o.cfg.agents_per_node = value(flag)
                    .parse()
                    .unwrap_or_else(|_| fail("bad --density"))
            }
            "--car-share" => {
                o.cfg.car_share = value(flag)
                    .parse()
                    .unwrap_or_else(|_| fail("bad --car-share"))
            }
            "--core-share" => {
                o.cfg.core_share = value(flag)
                    .parse()
                    .unwrap_or_else(|_| fail("bad --core-share"))
            }
            "--core-spread" => {
                o.cfg.core_spread = value(flag)
                    .parse()
                    .unwrap_or_else(|_| fail("bad --core-spread"))
            }
            "--speed" => o.speed = value(flag).parse().unwrap_or_else(|_| fail("bad --speed")),
            "--port" => o.port = value(flag).parse().unwrap_or_else(|_| fail("bad --port")),
            "--bind" => o.bind = Some(value(flag)),
            "--token" => o.token = Some(value(flag)),
            "--lan" => o.lan = true,
            other => fail(&format!("unknown option {other}")),
        }
    }
    if o.cfg.agents < 10 {
        fail("--agents must be at least 10");
    }
    o
}

fn describe(sim: &Sim) {
    let c = sim.composition();
    let km = (sim.w as f32 - 1.0) * sim::BLOCK_M / 1000.0;
    println!(
        "city: {}x{} intersections (~{:.1} km x {:.1} km), {} residents (seed {})",
        sim.w,
        sim.h,
        km,
        km,
        sim.agent_count(),
        sim.cfg.seed
    );
    println!(
        "residents: {} workers, {} errand-runners | long commutes by car {}, bike {}, short ones on foot {}",
        c.workers, c.errands, c.cars, c.bikes, c.walkers
    );
}

fn bench(o: &Opts) {
    let t0 = Instant::now();
    let mut sim = Sim::new(o.cfg.clone());
    let init = t0.elapsed();
    describe(&sim);
    println!("setup: {:.2} s", init.as_secs_f64());

    let ticks = o.days * DAY;
    let t1 = Instant::now();
    sim.run_ticks(ticks);
    let wall = t1.elapsed().as_secs_f64();

    let moving_updates: u64 = sim.stats.hourly.iter().map(|h| h.moving_ticks).sum();
    let all_updates = sim.agent_count() as f64 * ticks as f64;
    println!(
        "ran {} simulated day(s) = {} ticks of 1 s in {:.2} s wall time ({:.0}x real time)",
        o.days,
        ticks,
        wall,
        ticks as f64 / wall
    );
    println!(
        "throughput: {:.1} M moving-agent updates/s | {:.0} M agent-ticks/s counting idle agents",
        moving_updates as f64 / wall / 1e6,
        all_updates / wall / 1e6
    );
    println!("trips completed: {}", sim.stats.trips_done);
    println!(
        "most cars on one road segment: {} (standstill at {:.0})",
        sim.stats.peak_occupancy,
        sim::JAM_VEHICLES
    );
    println!("state hash: {:016x}", sim.state_hash());

    println!("\nhour  avg moving  car speed  arrivals  late (workers)");
    for (h, s) in sim.stats.hourly.iter().enumerate() {
        let moving = s.moving_ticks as f64 / (3600.0 * o.days as f64);
        let speed = if s.car_ticks > 0 {
            s.speed_permille as f64 / s.car_ticks as f64 / 10.0
        } else {
            100.0
        };
        println!(
            "{h:>4}  {moving:>10.0}  {speed:>8.1}%  {:>8}  {:>6}",
            s.arrivals, s.late
        );
    }
}

fn check(o: &Opts) {
    let run = |seed: u64| {
        let mut cfg = o.cfg.clone();
        cfg.seed = seed;
        let mut sim = Sim::new(cfg);
        sim.run_ticks(o.days * DAY);
        sim.state_hash()
    };
    let a = run(o.cfg.seed);
    let b = run(o.cfg.seed);
    let c = run(o.cfg.seed + 1);
    println!("seed {} run 1: {a:016x}", o.cfg.seed);
    println!("seed {} run 2: {b:016x}", o.cfg.seed);
    println!("seed {} run 3: {c:016x}", o.cfg.seed + 1);
    let ok = a == b && a != c;
    println!(
        "{}",
        if ok {
            "PASS: same seed gives the same run, a different seed gives a different run"
        } else {
            "FAIL"
        }
    );
    if !ok {
        exit(1);
    }
}

fn random_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mut s = String::new();
    for k in 0..2u64 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(nanos ^ k);
        s.push_str(&format!("{:016x}", h.finish()));
    }
    s
}

fn serve(o: Opts) {
    let sim = Sim::new(o.cfg.clone());
    describe(&sim);
    let bind = o.bind.clone().unwrap_or_else(|| {
        let host = if o.lan { "0.0.0.0" } else { "127.0.0.1" };
        format!("{host}:{}", o.port)
    });
    let opts = server::Options {
        bind: bind.clone(),
        speed: o.speed,
        token: o.token.clone().unwrap_or_else(random_token),
    };
    if let Err(e) = server::serve(sim, opts) {
        eprintln!("cannot listen on {bind}: {e}");
        exit(1);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        print!("{HELP}");
        return;
    };
    match cmd.as_str() {
        "bench" => bench(&parse(&args[1..])),
        "check" => check(&parse(&args[1..])),
        "serve" => serve(parse(&args[1..])),
        "help" | "--help" | "-h" => print!("{HELP}"),
        other => fail(&format!("unknown command {other}")),
    }
}
