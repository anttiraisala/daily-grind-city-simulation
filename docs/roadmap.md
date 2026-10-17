# Roadmap

Live tracker of what is planned, in progress and done. Decisions and rationale live in `design-plan.md`; dated history lives in `progress-log.md`.

Legend: `[x]` done, `[~]` in progress, `[ ]` not started. Gap IDs (G1-G9) and idea IDs (I-nn) refer to `design-plan.md`.

## Phase 0: Design (current)

- [x] Capture requirements and decisions in `design-plan.md`
- [x] Draft the data model (`schema/types.ts`)
- [x] Draft an example scenario and an example map (`schema/*.example.json`)
- [x] Create the repository and conventions (`README.md`, `CLAUDE.md`)
- [x] Choose a license (MIT) and add `LICENSE`
- [x] Getting-started instructions in `README.md` (clone and data checks; build and run pending the toolchain)
- [ ] Resolve the open questions at the end of `design-plan.md` (which gaps go into Phase 1; population model for year-long runs; sequential vs parallel weeks)
- [ ] Add a JSON Schema (or generated validators) for the scenario and map files
- [ ] Choose the toolchain: TypeScript project layout, test runner, formatter, CI

**Exit criteria:** open questions answered, toolchain chosen, schemas validate the example files in CI.

## Phase 1: Core simulation (headless)

Goal: a deterministic, headless simulation of one district that produces a congestion heatmap.

- [ ] Project skeleton: `packages/core`, test runner, CI
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
- [ ] Driver error model on top of IDM (G1)
- [ ] Intersections: right of way, conflict zones, traffic lights (G2)
- [ ] Pedestrian movement with social force; pedestrian crossings (G3)
- [ ] Parking (G5)
- [ ] Initial state and burn-in snapshot (G6)
- [ ] Calendar: weekdays, weekends, holidays (G7)
- [ ] Command log (needs, goals, road edits, pause)
- [ ] Event log and periodic snapshots
- [ ] Accumulation layers: dwell time, speed, slow time per cell per hour
- [ ] Headless CLI runner (I-71)
- [ ] Determinism test: same seed + same build gives an identical event log

**Exit criteria:** a 200-agent district runs a simulated week headlessly, the determinism test passes, and the congestion heatmap shows a visible morning and evening peak.

## Phase 1b: Measurement prototype

- [ ] Synthetic benchmark: agent updates per second on the maintainer's machine
- [ ] Decide whether the core stays in TypeScript or moves to Rust/WASM
- [ ] Record measured numbers in `design-plan.md` (replace the estimates)

## Phase 2: Viewer and inspector

- [ ] 2D renderer reading simulation snapshots (any frame rate, including 1 fps)
- [ ] Pan and continuous zoom from whole city to a single person
- [ ] Pause, resume, speed control
- [ ] Click an agent: current goal, reason, needs, mood
- [ ] Edit needs and mood, force a goal (logged as commands)
- [ ] Heatmap overlay from accumulation layers
- [ ] Settings panel for population, vehicles, trait distributions (writes the scenario JSON)

## Phase 3: Map editor and OSM import

- [ ] Draw roads, footpaths, ramps; place buildings and traffic lights
- [ ] Live road edits as commands, with network epochs
- [ ] OpenStreetMap import with manual edits stored as a diff
- [ ] Chunked storage for large maps

## Phase 4: Incidents and emergency services

- [ ] Free movement mode and collisions with buildings
- [ ] Crashes, pedestrians hit, medical episodes
- [ ] Ambulance, police and hospital with patient flow
- [ ] Crime events (abstract)

## Phase 5: Fires

- [ ] House fires, spread, survival decay
- [ ] Fire stations and fire trucks

## Phase 6 and later: Backlog

Pick from the ideas backlog in `design-plan.md`. Suggested first picks: I-40 what-if fork, I-42 butterfly effect, I-44 Braess paradox finder, I-50 desire paths, I-62 city newspaper.

## Notes on ordering

- Phase 1 comes before the viewer on purpose: the simulation is authoritative and must be testable without rendering.
- Phase 1b can run in parallel with the second half of Phase 1; its numbers decide the core language, so do it before the code grows large.
- The data model must leave room for the map editor, overpasses and free-body movement from the start (see `design-plan.md`, decisions on movement and world).
