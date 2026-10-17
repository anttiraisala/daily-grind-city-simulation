# Project instructions

## Language

- Chat with the maintainer in the language they write in (currently Finnish).
- Everything that goes into the repository is written in English: code and identifiers, file names, comments, UI text, docs, config, tests, data files, commit messages, branch names, PR titles and descriptions.
- Translate non-English requirements into idiomatic English before using them. Do not add Finnish (or other languages) unless localization is explicitly requested; then English stays the source language and translations go into separate resource files.
- Before reporting a task as done, check that no non-English text slipped into project files.

## Working style

- Before executing a request, check it for contradictions, ambiguity, hidden assumptions and conflicting goals. Flag problems directly, propose a fix, then execute. If the request is already clear, go straight to the work.
- Be direct. Do not praise weak ideas.

## Repository

- The default branch is `master` (a deliberate choice by the maintainer; do not rename it to `main`).
- `docs/design-plan.md` is the source of truth for decisions, gaps and the ideas backlog. Update it when a decision changes.
- `docs/roadmap.md` is the live tracker: tick off tasks as they are done. `docs/progress-log.md` gets a dated entry (done / decided / found / next) at the end of each working session. `docs/notes.md` is the informal scratchpad.
- `schema/` holds the data model. The map and the scenario are separate JSON files. JSON is the single source of truth for configuration.

## Simulation invariants

- Fixed time step; the simulation is authoritative and rendering is decoupled.
- Same seed + same machine + same program version must give the same run. Each agent has its own RNG.
- Every manual change (needs, goals, road edits, fires, pause) is a command logged against a tick.
- No simulation level of detail: all areas are simulated at the same fidelity.
