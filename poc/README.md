# Proof of concept: native Rust simulation + browser viewer

A small, throwaway-quality demo that proves the architecture chosen in `docs/design-plan.md`:

```
 Rust simulation (native process)  <--HTTP-->  browser viewer (desktop or phone)
   deterministic, fixed step                     pan / zoom / pinch, click a resident,
   event-driven decisions                        pause, speed, edit (delay a departure)
```

It is **not** the real simulation. The model is deliberately simple; the point is the stack: a native core that runs fast, a viewer that is only a viewer, and a phone that can watch a run on a desktop over the local network.

## Run it

Needs a Rust toolchain (see the top-level `README.md`). There are no dependencies, so it builds offline.

```sh
cd poc
cargo run --release -- serve            # this computer only
cargo run --release -- serve --lan      # also reachable from a phone on the same Wi-Fi
```

The program prints the viewer address, including an access token: `http://localhost:8080/?t=...`. With `--lan` it also prints the address to open on the phone. Open it, then:

- drag to pan, scroll or pinch to zoom (from the whole city down to a single person)
- tap a resident to inspect them; **Follow** zooms in and tracks them; **Delay departure** sends a command
- buttons set the simulation speed (x1 ... x3600) and pause/resume; **Slow-traffic heat** shows where cars have been slow

Other commands:

```sh
cargo run --release -- bench --agents 300000   # headless throughput, hash and an hourly table
cargo run --release -- check                   # determinism check: same seed -> same run
cargo test --release                           # unit tests
cargo run --release -- help
```

## What it demonstrates

| Idea from the design plan | In the PoC |
|---|---|
| Simulation authoritative, rendering decoupled | The simulation runs on its own thread; the viewer only reads snapshots |
| Deterministic runs (same seed, same machine, same build) | One RNG per agent; `check` compares state hashes; tests cover it |
| Event-driven decisions | Idle agents cost nothing between departures (a heap of departure events) |
| Struct-of-arrays agent storage | Plain `Vec` columns; 1M residents use about 80 MB |
| Agents inside buildings take no space | Only moving agents exist on the map and in snapshots |
| 24-hour clock, schedules from distributions, punctuality, skipped days | 7:00 / 8:00 / 10:00 starts, ~8 h days, errand-runners, 1 % skipped days |
| Learned commute estimates | Agents leave earlier after slow trips |
| Cars slow down in traffic; bikes and walkers do not | Greenshields speed-density relation per directed street segment |
| Cars have real size; people take up space | Drawn at real size when zoomed in |
| Manual changes are commands logged against a tick | `Delay` command, applied at a tick boundary, listed at `/api/commands` |
| One city-size control scales map and population together | `--agents` and `--density` set the grid size |
| Phone as a viewer first | `--lan`, touch and pinch support, compact layout, access token |
| Slow-traffic maps | Accumulated per intersection, shown as a heat overlay |

## Measured throughput

One simulated day, 1 s ticks, single thread, on a 2-vCPU cloud VM (so a modern desktop should be faster). Default scenario: 40 residents per intersection, 85 % of workers with a downtown job.

| Residents | City grid | Wall time per simulated day | Speed vs real time | Memory |
|---:|---|---:|---:|---:|
| 100,000 | 50 x 50 intersections (4.9 km) | 1.7 s | 51,000x | 11 MB |
| 300,000 | 87 x 87 (8.6 km) | 12.9 s | 6,700x | 26 MB |
| 1,000,000 | 159 x 159 (15.8 km) | 190 s | 450x | 82 MB |

Read these numbers with care. The model is far lighter than the real one: 1 s ticks instead of 0.1 s, no car following, no intersections, no traffic lights, no needs or moods, no incidents. The real simulation will be much slower per resident. What the numbers do show is that the architecture (event-driven decisions, columnar data, no rendering in the loop) is not the bottleneck.

## Known limits (deliberate)

- **No dependencies.** The crates.io registry was not reachable where this was built, so the HTTP server is hand-written and the viewer polls instead of using a WebSocket. The real version should use `axum` and a WebSocket, and a proper JSON library.
- **No spillback.** More cars than jam density can pile onto one street segment (the 1M run peaks at 153 cars on a segment that holds 13). Real queues must block the roads behind them (design plan gap G10).
- **Congestion only appears in large, dense cities.** Without intersections and traffic lights, streets carry too much. In this model a rush hour needs roughly 100,000 residents, 40 per intersection and a tight downtown. Expect it to appear much earlier once junctions are modelled (G2).
- **Simple routing.** Cars go along x first, then along y, so traffic forms vertical stripes along the downtown columns. That is a routing artefact, not a property of the city.
- **1 s tick.** The real design uses 0.1 s.
- **No saved state.** There are no snapshots on disk, no event log and no heat map export yet.
- **Unencrypted HTTP.** `--lan` should only be used on a network you trust. Every API call needs the printed token.

## Files

| File | Purpose |
|---|---|
| `src/sim.rs` | The simulation core: agents, schedules, movement, commands, snapshots |
| `src/rng.rs` | Deterministic per-agent random number generator |
| `src/server.rs` | Minimal HTTP server and API |
| `src/main.rs` | Command line: `serve`, `bench`, `check` |
| `web/index.html` | The viewer (single file, no build step) |

## API (all calls need `?t=<token>`)

| Call | Returns |
|---|---|
| `GET /` | the viewer page |
| `GET /api/info` | city size, resident count, seed |
| `GET /api/status` | clock, speed, counts, statistics (JSON) |
| `GET /api/snapshot?x0&y0&x1&y1&cap` | binary snapshot of moving agents in the rectangle (32-byte header, 13 bytes per agent) |
| `GET /api/heat` | one byte per intersection, slow-traffic heat |
| `GET /api/agent?id=N` | details of one resident (JSON) |
| `GET /api/commands` | the command log |
| `POST /api/cmd?kind=pause\|resume\|speed\|delay` | queue a command (`speed` needs `value`, `delay` needs `agent` and `seconds`) |

## What to carry over to the real implementation

Keep: columnar agent storage, per-agent RNG, event-driven decisions, tick-stamped commands, the snapshot-plus-viewport protocol, the idea that the viewer is only a viewer.

Replace: the hand-written server (use `axum` and WebSocket), the grid-only world (road graph from `schema/`), the speed-density formula (car following with intersections and traffic lights), the routing, the viewer (TypeScript and PixiJS), configuration (JSON files from `schema/`).
