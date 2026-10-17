# Progress log

Dated, newest first. One entry per working session or milestone: what was done, what was decided, what is next.

## 2026-10-17

**Done**
- Reviewed the whole design once more and wrote `design-plan.md`: decisions so far, nine gaps found (G1-G9), phased roadmap, and a backlog of 75 numbered ideas (I-01 to I-84).
- Adopted the English-only repository rule; translated `types.ts` and the example files; renamed the example map to `example_block`.
- Set up the GitHub repository `anttiraisala/daily-grind-city-simulation`: default branch `master`, first commit with docs, schema, `README.md` and `CLAUDE.md`.
- Added `roadmap.md`, `progress-log.md` and `notes.md`.
- Added the MIT `LICENSE` and a getting-started section in `README.md` (Git install per OS, clone, download without Git, data file checks).
- Built the proof of concept in `poc/` (std-only Rust): deterministic event-driven commuting simulation, HTTP server, single-file browser viewer with pan/zoom/pinch, resident inspector, speed control, a logged `Delay` command and a slow-traffic heat map. 10 unit tests pass; the determinism check passes; the viewer was driven with a headless browser (desktop and phone layouts, tap, follow, pinch, commands, wrong token).
- Expanded the README getting-started section into detailed per-OS steps: install, test, determinism check, run (local and `--lan`), options, update, troubleshooting, firewall.
- Measured the PoC: 100k residents = 1.7 s per simulated day, 300k = 12.9 s, 1M = 190 s (single thread, 2-vCPU VM, much simpler model than the real one).

**Decided**
- Default branch is `master`, on purpose.
- License is MIT for the project's own code and docs. Data imported from OpenStreetMap stays under ODbL and must be kept separate and attributed.
- Reproducibility only on the same machine with the same program version; no bit-exact results across machines.
- Hybrid agent model: binding schedules plus needs and mood that fill the gaps and cause deviations.
- Two movement modes (lane, free) plus inside-building agents that take no space.
- Architecture: native Rust core plus a browser viewer. Phones in the order (1) LAN viewer, (2) WASM build for small offline runs, (3) native app only if needed. TypeScript is limited to the viewer and the map editor.

- Maintainer confirmed: Ubuntu 24.04 desktop, repository stays public, copyright holder "Antti Raisala".
- Started the real core (`crates/core`): per-agent RNG, fixed 0.1 s clock, road graph with levels and signals, IDM car-following on finite segments with queue spillback, hand-over between segments, slow-time statistics, state hash. 14 tests, including: no overlaps, red light holds traffic, queue reaches the entrance (G10 solved in the model), same input gives same hash. `citysim bench` runs a synthetic signalised grid.
- Added stop/cleanup commands for running simulations to the README (Linux, macOS, Windows).
- Added the Rust workspace skeleton (`crates/core`, `crates/cli`, `crates/server`; `poc/` excluded) and GitHub Actions CI.
- Phase 1 gaps: G2, G3, G6 only; G1 moves to Phase 4, G5 and G7 before the first year-long run. Population constant; long runs sequential.

**Found**
- G1: IDM is collision-free by design, so crashes need a driver error model on top of it.
- G2/G3/G5/G6/G7 must be in Phase 1; G4/G8 can wait. (Proposal, not yet confirmed.)
- G10 (new): no queue spillback in the PoC; more than jam-density cars pile onto one segment.
- In the simple PoC model, rush-hour congestion appears only in large, dense cities (about 100k residents, 40 per intersection, tight downtown) because there are no junctions or traffic lights yet.
- The sandbox used for this work cannot reach crates.io, so the PoC uses only the Rust standard library (hand-written HTTP server, polling instead of WebSocket).

**Next**
- Run `citysim bench` on the maintainer's machine and compare with the sandbox numbers.
- Junction model beyond "enter if room": conflict zones, right of way, turning (G2); JSON map loading (needs a JSON parser: serde works in CI and on the maintainer's machine but not in this sandbox).
- Answer the remaining open questions in `design-plan.md` (which gaps go into Phase 1, population model for year-long runs, sequential vs parallel weeks).
- Start the Phase 1 core loop with the real road graph from `schema/`.

## 2026-10-16

**Done**
- Requirements discussion in chat: top-down city simulation with a 24-hour cycle, residents with personalities, vehicles, buildings, traffic lights, emergency services, incidents, map editor, JSON configuration, long-run heatmaps.
- Drafted the first data model and examples (`types.ts`, scenario and map JSON).

**Decided**
- Uniform fidelity everywhere; simulation step rate matters more than rendering.
- Scale targets: 10k-100k agents at full fidelity on a desktop, aspirational 1M; 10-100 on a phone.
- One "city size" control scales map and population together.
- First research question: where does congestion form?
