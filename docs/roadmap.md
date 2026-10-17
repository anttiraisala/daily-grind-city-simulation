# Roadmap

Live tracker of what is planned, in progress and done. Decisions and rationale live in `design-plan.md`; dated history lives in `progress-log.md`.

Legend: `[x]` done, `[~]` in progress, `[ ]` not started. Gap IDs (G1-G9) and idea IDs (I-nn) refer to `design-plan.md`.

## Phase 0: Design (current)

- [x] Capture requirements and decisions in `design-plan.md`
- [x] Draft the data model (`schema/types.ts`)
- [x] Draft an example scenario and an example map (`schema/*.example.json`)
- [x] Create the repository and conventions (`README.md`, `CLAUDE.md`)
- [x] Choose a license (MIT) and add `LICENSE`
- [x] Getting-started instructions in `README.md` (clone, data checks, building and running the proof of concept)
- [x] Resolve the open questions in `design-plan.md` section 6 (Phase 1 = G2, G3, G6; constant population; sequential weeks)
- [ ] Add a JSON Schema (or generated validators) for the scenario and map files
- [x] Decide the architecture: native Rust core + browser viewer; phones in the order LAN viewer, WASM build, native app
- [x] Proof of concept in `poc/`: native Rust simulation, browser viewer, phone over the local network (std only; see `poc/README.md`)
- [~] Choose the toolchain: Rust core (cargo, clippy, rustfmt, tests) and TypeScript viewer decided; CI exists for Rust and the example JSON; the viewer's build setup is still open

**Exit criteria:** open questions answered, toolchain chosen, schemas validate the example files in CI.

## Phase 1: Core simulation (headless)

Goal: a deterministic, headless simulation of one district that produces a congestion heatmap.

- [x] Project skeleton: Rust workspace (`crates/core`, `crates/cli`, `crates/server`), tests, CI (GitHub Actions: fmt, clippy, tests, PoC check, JSON validity)
- [ ] Seeded RNG with one stream per agent
- [ ] Fixed-step simulation loop and 24-hour clock
- [ ] Config loading and validation (scenario + map)
- [ ] Road graph, z-levels, ramps
- [ ] Spatial hash keyed by (cell, level)
- [ ] Agents: traits, needs, mood, commitments, goals
- [ ] Schedule generation from the scenario distributions (start times, shifts, lunch, remote work)
- [ ] Event-driven decisions; needs updated once per simulated minute
- [ ] Pathfinding and learned commute estimates
- [ ] Lane-following vehicles: IDM car-following
- [ ] Intersections: right of way, conflict zones, traffic lights (G2)
- [ ] Pedestrian movement with social force; pedestrian crossings (G3)
- [ ] Initial state and burn-in snapshot (G6)
- [ ] Command log (needs, goals, road edits, pause)
- [ ] Event log and periodic snapshots
- [ ] Accumulation layers: dwell time, speed, slow time per cell per hour
- [ ] Headless CLI runner (I-71)
- [ ] Determinism test: same seed + same build gives an identical event log

**Exit criteria:** a 200-agent district runs a simulated week headlessly, the determinism test passes, and the congestion heatmap shows a visible morning and evening peak.

## Phase 1b: Measurement prototype

- [ ] Synthetic benchmark: agent updates per second on the maintainer's machine
- [x] Core language: Rust, native (decided). Re-measure with the real model: the PoC numbers are for a much simpler one
- [ ] Record measured numbers in `design-plan.md` (replace the estimates)

## Phase 2: Viewer and inspector

- [ ] 2D renderer reading simulation snapshots (any frame rate, including 1 fps)
- [ ] Pan and continuous zoom from whole city to a single person
- [ ] Pause, resume, speed control
- [ ] Click an agent: current goal, reason, needs, mood
- [ ] Edit needs and mood, force a goal (logged as commands)
- [ ] Heatmap overlay from accumulation layers
- [ ] Settings panel for population, vehicles, trait distributions (writes the scenario JSON)
- [ ] Replace HTTP polling with a WebSocket (axum); TypeScript and PixiJS viewer with generated types
- [ ] Phone step 2: WASM build of the core for small offline runs (10-100 agents)
- [ ] Phone step 3: native mobile app, only if needed

## Phase 3: Map editor and OSM import

- [ ] Draw roads, footpaths, ramps; place buildings and traffic lights
- [ ] Live road edits as commands, with network epochs
- [ ] OpenStreetMap import with manual edits stored as a diff
- [ ] Chunked storage for large maps

## Phase 4: Incidents and emergency services

- [ ] Driver error model on top of IDM (G1)

- [ ] Free movement mode and collisions with buildings
- [ ] Crashes, pedestrians hit, medical episodes
- [ ] Ambulance, police and hospital with patient flow
- [ ] Crime events (abstract)

## Phase 5: Fires

- [ ] House fires, spread, survival decay
- [ ] Fire stations and fire trucks

## Before the first year-long run

- [ ] Parking (G5)
- [ ] Calendar: weekdays, weekends, holidays (G7)

## Phase 6 and later: Backlog

Pick from the ideas backlog in `design-plan.md`. Suggested first picks: I-40 what-if fork, I-42 butterfly effect, I-44 Braess paradox finder, I-50 desire paths, I-62 city newspaper.

## Notes on ordering

- Phase 1 comes before the viewer on purpose: the simulation is authoritative and must be testable without rendering.
- Phase 1b can run in parallel with the second half of Phase 1; its numbers decide the core language, so do it before the code grows large.
- The data model must leave room for the map editor, overpasses and free-body movement from the start (see `design-plan.md`, decisions on movement and world).
