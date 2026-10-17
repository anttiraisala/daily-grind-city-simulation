# Notes

Informal scratchpad for thoughts, hunches and half-formed ideas. Anything that becomes a decision moves to `design-plan.md`; anything that becomes work moves to `roadmap.md`.

## Vision

Watching small people live their lives on a map is the point. Congestion research is the first excuse; the longer-term appeal is a living city where you can pause, click a person, read their mind and nudge it. The project is half serious and game-like.

## Guiding principles

- Simulate first, draw later. The simulation must be correct and testable without a screen.
- Do not fake detail away from the camera. Incidents must be able to happen anywhere.
- Randomness is welcome, but it must be seeded so any interesting event can be replayed.
- Every user intervention is a logged command. That one rule gives replay, what-if forks, and time travel almost for free.
- Prefer emergent behavior over scripted behavior: people should cause congestion, not be told to.

## Hunches to test

- Learned commute estimates will make people leave earlier over time and flatten the rush-hour peak. Is the first simulated week therefore not representative?
- Learned, adaptive agents make parallel weeks invalid (see G9 in `design-plan.md`). Decide early whether learning is on in research runs.
- A small share of careless or aggressive drivers may dominate the crash statistics. Needs many seeds.
- Parking search may turn out to be a surprisingly large part of downtown congestion.
- In the PoC, congestion needed a big, dense city because streets had no junctions. Real streets lose most of their capacity at intersections and traffic lights, so expect congestion to appear much earlier once G2 exists. Compare the PoC's heat map with the real one then.
- Simple x-then-y routing makes traffic form straight stripes. Check that real route choice (and congestion-aware rerouting, I-03) breaks the stripes up.

## Open questions

See the end of `design-plan.md`. Copy new ones here first, promote them there when they block work.

## Parking lot (not now)

- Real-data calibration (I-49) once the model works.
- Multiplayer (I-77) and mod API (I-75): both depend on the command log being clean from the start.
