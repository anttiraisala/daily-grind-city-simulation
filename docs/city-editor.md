# City editor

A single-file browser editor for the map JSON (`editor/index.html`). It edits the map only; the scenario (population, schedules, traits) is a separate file and is still edited by hand.

## Run

```bash
cd daily-grind-city-simulation
python3 -m http.server 8000
# open http://localhost:8000/editor/index.html
```

No build step and no dependencies. The server is only needed so the "Load example" menu can fetch `cities/*/map.json`; "Open JSON…" works from a plain file too.

## Tools

| Key | Tool | Behaviour |
|---|---|---|
| V | Select / move | Click to select, drag a node to move it; the right panel edits the selection (lanes, speed, width, building capacity, light timing) |
| R | Road | Click to place nodes; each click adds a road edge from the previous node. Clicking an existing road node connects to it; clicking the middle of a road splits it. Esc or double-click ends the chain |
| F | Footpath | Same as roads, on the pedestrian network |
| C | Pedestrian crossing | Click a road: creates a crossing edge with a footpath node on each side, at the clicked position along the road |
| L | Traffic light | Click a road node to add or remove a light |
| B | Building | Drag a rectangle; the entrance is the nearest road node, the door the nearest footpath-only node |
| X | Delete | Delete the node, edge or building under the cursor (a road takes its crossings with it) |

Wheel zooms, right or middle drag (or Space + drag) pans, Ctrl+Z and Ctrl+Y undo and redo (100 steps). Snap to a grid (default 10 m) can be switched off.

## What is derived

- **Traffic-light phases** are generated on save: roads at the node are grouped by axis (mostly horizontal / mostly vertical), the two groups alternate, and a crossing next to the light walks while the road it crosses is red. The cycle and green time come from the light's properties.
- **Crossing geometry**: the two foot nodes are placed perpendicular to the road at half the road width plus 1.5 m.
- **Building capacity** is a starting guess from the footprint area (a home holds one resident per 60 m2, a workplace one job per 20 m2); edit it in the right panel.

## Validation

The panel at the right lists problems as you edit. It mirrors the loader in `crates/core/src/map.rs` (unknown nodes, duplicate ids, crossings without a road, buildings without a road entrance, lights with fewer than three roads, crossing ends without a footpath). The authoritative check is

```bash
cargo run --release -p citysim-cli -- check-map map.json
```

## Test

`editor/test_editor.py` drives the editor headlessly with Playwright: it draws a small city by clicking, exports it, and loads the tiny town. Run it as described in the file header, then run `check-map` on the output.

## Known limits

- The whole map is one JSON document in memory; the big city (about 800 KB) loads but is not pleasant to edit. Chunked storage is a Phase 3 item.
- Roads are straight segments (no curves yet), one lane per direction per edge is simulated whatever the lane count says (see the design plan).
- A road with crossings cannot be split by connecting into its middle; delete the crossings first.
- No scenario editing, no live edits of a running simulation (those become logged commands in Phase 3).
