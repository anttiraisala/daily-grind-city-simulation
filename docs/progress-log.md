# Progress log

Dated, newest first. One entry per working session or milestone: what was done, what was decided, what is next.

## 2026-10-17

**Done**
- Reviewed the whole design once more and wrote `design-plan.md`: decisions so far, nine gaps found (G1-G9), phased roadmap, and a backlog of 75 numbered ideas (I-01 to I-84).
- Adopted the English-only repository rule; translated `types.ts` and the example files; renamed the example map to `example_block`.
- Set up the GitHub repository `anttiraisala/daily-grind-city-simulation`: default branch `master`, first commit with docs, schema, `README.md` and `CLAUDE.md`.
- Added `roadmap.md`, `progress-log.md` and `notes.md`.
- Added the MIT `LICENSE` and a getting-started section in `README.md` (Git install per OS, clone, download without Git, data file checks).

**Decided**
- Default branch is `master`, on purpose.
- License is MIT for the project's own code and docs. Data imported from OpenStreetMap stays under ODbL and must be kept separate and attributed.
- Reproducibility only on the same machine with the same program version; no bit-exact results across machines.
- Hybrid agent model: binding schedules plus needs and mood that fill the gaps and cause deviations.
- Two movement modes (lane, free) plus inside-building agents that take no space.

**Found**
- G1: IDM is collision-free by design, so crashes need a driver error model on top of it.
- G2/G3/G5/G6/G7 must be in Phase 1; G4/G8 can wait. (Proposal, not yet confirmed.)

**Next**
- Answer the open questions in `design-plan.md`.
- Choose the toolchain and set up the project skeleton and CI.
- Start the Phase 1 core loop.

## 2026-10-16

**Done**
- Requirements discussion in chat: top-down city simulation with a 24-hour cycle, residents with personalities, vehicles, buildings, traffic lights, emergency services, incidents, map editor, JSON configuration, long-run heatmaps.
- Drafted the first data model and examples (`types.ts`, scenario and map JSON).

**Decided**
- Uniform fidelity everywhere; simulation step rate matters more than rendering.
- Scale targets: 10k-100k agents at full fidelity on a desktop, aspirational 1M; 10-100 on a phone.
- One "city size" control scales map and population together.
- First research question: where does congestion form?
