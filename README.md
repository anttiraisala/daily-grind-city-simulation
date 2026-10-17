# Daily Grind City Simulation

A top-down 2D/2.5D city simulation in which residents with individual personalities live through a 24-hour cycle: commuting, working, eating, shopping, skipping work, and sometimes crashing cars. It is a semi-serious, game-like research tool for studying where and when congestion forms and why.

**Status:** design phase. No simulation code yet.

## Key ideas

- Fixed-step, deterministic simulation. Rendering is decoupled and optional.
- Uniform fidelity across the whole city, so incidents can happen anywhere.
- Residents have persistent traits, short-term needs and moods, and binding schedules (work, school, shifts).
- Lane-following movement by default, free-body movement for loss of control and deliberate acts.
- Editable road graph with traffic lights, z-levels (overpasses), emergency services and fires.
- Everything configurable through JSON; a map editor and OpenStreetMap import are planned.

## Repository layout

| Path | Contents |
|---|---|
| `docs/design-plan.md` | Decisions so far, known gaps, roadmap and the numbered ideas backlog |
| `schema/types.ts` | Core data structures (draft) |
| `schema/scenario.example.json` | Example scenario: population, traits, schedules, events |
| `schema/map.example.json` | Example map: roads, traffic lights, buildings |

## Branching

The default branch is `master`.

## Language

All project content (code, comments, docs, UI text, commit messages) is written in English.
