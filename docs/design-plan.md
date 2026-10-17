# Traffic Congestion Simulation: Design Plan

Status: draft v0.3 (2026-10-17). Companion files: `types.ts`, `scenario.example.json`, `map.example.json`.

## 1. Purpose

A semi-serious, game-like research tool: a top-down 2D/2.5D city where residents with individual personalities live through a 24-hour cycle (commuting, shopping, skipping work, crashing cars) while the user watches, inspects and intervenes.

First research question: **where and when does congestion form, and why?**

Non-goals for now: calibration against real data, bit-exact reproducibility across machines, physics inside buildings.

## 2. Decisions so far

### Simulation

- Fixed time step (0.1 s by default). The simulation is authoritative. Rendering is decoupled and may run at any rate, including 1 fps or not at all. Runs are not real-time: the user trades speed for size.
- Uniform fidelity across the whole city. No level of detail in the simulation itself: an accident must be able to happen where nobody is looking.
- Reproducibility: same seed + same machine + same program version = same run. Each agent has its own RNG. Parallel runs use different seeds and their results are aggregated statistically.
- Every manual intervention (edit needs, force a goal, add a road, start a fire, pause) is a command logged against a tick.

### Scale

- Realistic target: 10k-100k agents at full fidelity on a desktop; 100k-300k with an optimized native core (multi-threaded, GPU later). 1M is aspirational and needs coarser time steps or several machines. The proof of concept (`../poc/`, a much simpler model with 1 s ticks) simulated 1M residents for one day in about 3 minutes on one core, so the architecture is not the bottleneck; the full model will be far heavier.
- Phones, in this order (decided 2026-10-17): (1) a viewer for a run computed on a desktop, over the local network; (2) a WASM build of the same core for small offline runs of 10-100 agents; (3) a native mobile app, only if it turns out to be needed.
- One "city size" control scales the map and the population together, so density, and therefore congestion behavior, is preserved. Separate controls remain available for deliberate density experiments.

### Agents

- Traits (persistent), needs (change within minutes), mood (minutes or faster).
- Hybrid decisions: binding commitments (work, school, shifts, lunch) set the rhythm; needs fill the gaps and cause deviations (skipping work, stopping to eat).
- Decisions are event-driven (an activity ends or a need crosses a threshold), not made every tick. Needs update once per simulated minute.
- Work start times are mixture distributions (e.g. 07:00 / 08:00 / 10:00), with varying shift lengths, lunch breaks, remote work and punctuality.
- Commute duration is estimated and learned from completed trips; punctuality sets the safety margin.
- The user can pause, click an agent, read its current goal and the reason for it, and edit its needs and mood or force a goal.
- Rare traits: careless driving, medical episodes, violent intent (deliberately harming others, including with a vehicle).

### Movement

- Lane mode (cheap, the default): IDM car-following for vehicles; pedestrians on footpaths with social-force avoidance and individual personal-space needs.
- Body and personal space: a person is a circle 0.5 m across (`bodyDiameterM`, fixed at 0.5 m by default, still configurable). Around the body each person wants free space, `spaceNeed`, a personality trait: the wanted free gap between their body edge and anyone else's body edge. Typical values 0.2 to 1.0 m; the scenario can use any distribution and any value of 0 or more. A person's personal zone is a circle of radius `0.25 + spaceNeed` m around the center. The need is asymmetric: when someone enters your zone, you feel pushed away, regardless of how big their own zone is, so a person with a large need keeps clear of others while a person with a small need does not mind. The need is a preference, not a hard wall: in crowds (rush hour, a full bus, a narrow footpath) people accept a smaller gap, which costs comfort and raises stress. The only hard limit is body contact (gap 0 m). Both values are shown to scale in the viewer; the inspector can draw the personal zone as a ring, and a user can change a person's traits by command (`setTraits`).
- Free mode (rare): a vehicle or pedestrian leaves its lane because of loss of control, a medical episode or a deliberate act. It collides with agents and buildings through the spatial hash.
- Inside mode: agents inside buildings take up no space and have no physics.
- Z-levels: roads and agents have a level; the spatial hash key is (cell, level); ramps, stairs and lifts connect levels.

### World

- The road network is an editable graph. Live edits are commands applied at tick boundaries. Each edit starts a new network epoch, and statistics accumulate per epoch.
- Traffic lights; buildings with capacities and opening hours; hospitals, police stations and fire stations.
- Incidents: crashes, pedestrians hit by vehicles, medical episodes, crime, house fires. The chance of surviving a fire decays until the fire brigade arrives, and spread becomes more likely the longer a fire burns. Emergency vehicles ignore red lights and other traffic yields, so response times depend on congestion.
- The number of cars is an outcome (ownership share + mode choice), with an optional override.

### Data and tools

- JSON is the single source of truth. The map and the scenario are separate files. Traits are distributions with optional correlations. Configs are validated before a run.
- A map editor that can, in principle, handle cities of any size; OSM import with manual edits stored as a diff; large maps stored in chunks.
- Outputs: event log, periodic snapshots, accumulation layers (dwell time, speed and slow time per cell per hour).
- Architecture (decided 2026-10-17): the core is a deterministic Rust library with no I/O, run natively: a headless command line for batches, plus a server. A browser viewer renders snapshots and sends commands. Commands are tick-stamped messages, so the command log doubles as the network protocol. Phones follow the order under Scale. The proof of concept in `../poc/` demonstrates this slice (see `../poc/README.md`).
- Tooling consequence: Rust (cargo, clippy, rustfmt, `cargo test`, property tests later) for the core; schemas defined in Rust (serde + schemars) with TypeScript types generated for the viewer; TypeScript, Vite and PixiJS only for the viewer and the map editor. This replaces the earlier TypeScript-only toolchain idea.

### Presentation (decided 2026-10-17)

- The viewer offers several visual styles and the user switches between them on the fly. The ten candidates are in the style sheet (blueprint, flat map, neon, schematic, isometric, top-down realistic, speed map, ink, pixel, state colors); `schema/viewer.example.json` and `ViewerConfig` in `schema/types.ts` define the configuration: the default style, the allowed styles, and the scale behavior.
- **The default style is blueprint** (thin lines on a dark background, cars as boxes, people as small circles). It is the cheapest to draw and the clearest for reading congestion.
- **Geometry is true to scale in meters in every style.** A person is a circle about 0.5 m across (`bodyDiameterM` in `Traits`), a car is 4.5 x 1.8 m (`vehicleSizes` in the scenario), a road is `lanes * laneWidthM` wide (3.5 m by default, from the map), buildings use their real footprints. Styles change only how this geometry looks, never its size or position.
- **True scale conflicts with visibility when zoomed out.** At the scale of a whole city one pixel is many meters, so a 0.5 m person is far smaller than a pixel. Decision: by default the renderer clamps the on-screen size of people and vehicles to a minimum (`minScreenPx`, 3 px and 6 px), so they stay visible; `strictScale: true` turns the clamp off and they vanish when zoomed out. Zoomed in far enough, the clamp has no effect and sizes are exact. The clamp is a rendering detail only. The simulation never uses it. The viewer shows a scale bar, and marks the view as "enlarged" while the clamp is active.
- **Style is a viewer setting, not simulation state.** Switching it changes nothing in the run, is not a command and is not in the command log (the rule "every manual change is a command" concerns the simulation). Phone and desktop can use different styles of the same run. The last choice can be remembered per browser (`rememberChoice`).
- Config rules: `allowedStyles` must not be empty, `defaultStyle` must be one of them, unknown style names are an error. The configuration is validated before use like the other JSON files.
- Levels of detail in drawing are allowed (the simulation stays uniform): far away a vehicle is a dot, nearer a box, closest the full detail of the chosen style. See the cost notes in the style sheet.

## 3. Gaps found in review

These are not optional extras. Each one affects the core model or the schema.

**G1. IDM cannot crash.** The Intelligent Driver Model is designed to be collision-free, so careless drivers on plain IDM never crash. Crashes need a driver error model on top: reaction delay, perception noise and anticipation errors (the Human Driver Model by Treiber, Kesting and Helbing is a ready-made extension of IDM), distraction episodes (eyes off the road for a few seconds), gap-acceptance errors at intersections, and running red lights. A crash is a geometric overlap detected in the spatial hash, after which the vehicles involved switch to free mode. Schema impact: driver-error parameters in `Traits` and in the scenario.

**G2. Intersections.** Most crashes and most congestion happen at intersections, and the plan has no intersection model yet. Needed: conflict zones where movements cross, right-of-way rules (traffic lights, yield signs, and the right-hand priority rule at uncontrolled intersections, as in Finland), turning movements, and left turns across oncoming traffic.

**G3. Pedestrian crossings.** Accidents between pedestrians and vehicles happen where footpaths cross roads. Needed: crossing edges with or without signals, conflict points against road edges on the same level, and jaywalking as a trait.

**G4. Lane changes.** IDM models a single lane. Multi-lane roads need a lane-change model such as MOBIL.

**G5. Parking.** A car that is not moving has to be somewhere; without parking, cars vanish and reappear. Needed: parking capacity per building or lot, street parking, and searching for a parking spot when lots are full (a well-known source of downtown traffic). Shared household cars follow from this.

**G6. Initial state and burn-in.** At 00:00 on day 1, where is everyone? Proposal: everyone starts at home except night-shift workers. Learned commute estimates and habits need several simulated days to settle, so the first days must be excluded from the statistics. Efficient pattern: run the burn-in once, take a snapshot, then fork many seeds from that snapshot.

**G7. Calendar.** Weekdays, weekends and holidays have different schedules. The scenario has `daysToRun` but no calendar.

**G8. Long runs and population.** Over a simulated year, deaths and injuries deplete the population. Choose between a constant population (agents are replaced) and a demographic model (see I-07). Injured agents need hospital stays and time off work.

**G9. Independence of parallel weeks.** Running 52 weeks in parallel instead of one sequential year is only valid if nothing carries over from week to week. Learning, demographics, seasons and road wear all carry over. Backlog items that break this assumption are marked "Breaks G9".

**G10. Queue spillback.** A full street must block the streets behind it, and cars cannot exceed jam density. The proof of concept does not do this: its 1M run puts up to 153 cars on a segment that holds 13, which makes congestion look milder and more local than it should. Solved by car following on finite-length lanes (IDM) together with junctions (G2), not by tuning the speed-density formula. **Status: the core (`crates/core/src/traffic.rs`) now does this; a test fills a bottleneck queue back to the entrance with no overlaps and no vehicle above jam density.**

## 4. Roadmap

The phased roadmap with task-level status lives in [`roadmap.md`](roadmap.md); dated history is in [`progress-log.md`](progress-log.md); informal thoughts are in [`notes.md`](notes.md). In short: headless core first, then viewer and inspector, then map editor and OSM import, then incidents and emergency services, then fires, then the backlog.

**MVP:** one district, a few hundred agents, homes, workplaces and shops, walking and cars, a few traffic lights, a 24-hour clock, a congestion heatmap and an agent inspector.

## 5. Ideas backlog

Every idea has an ID so it can be picked up later. Effort is relative: S < M < L < XL. Wildness: 1 = practical, 2 = ambitious, 3 = crazy. "Breaks G9" means the idea carries state across weeks, so a long run must be sequential.

### 5.1 Top picks

The best value for the effort:

- **I-40** What-if fork
- **I-42** Butterfly effect
- **I-44** Braess paradox finder
- **I-50** Desire paths
- **I-60** Event director camera
- **I-62** City newspaper
- **I-20** Weather and winter
- **I-28** Power outage
- **I-34** Epidemic layer
- **I-70** Event log query console

### 5.2 Agents and behavior

- **I-01 Households and social graph.** Families, friends, coworkers. Shared cars and dropping children at school or daycare create trip chains; friends meet up. Effort M, wildness 1. Needs G5.
- **I-02 Habits and spatial memory.** A favorite cafe, a preferred route, "that intersection is always jammed, so I avoid it". Effort M, wildness 1. Breaks G9.
- **I-03 Information spread.** News of an accident spreads through the social graph and traffic radio, plus a navigation app used by a configurable share of drivers that reroutes them through residential streets. Research question: does the app make the city better or worse? Effort M, wildness 2.
- **I-04 Emotional contagion.** Anger spreads in jams (road rage, aggressive driving, more crashes); panic spreads during an evacuation. Effort S, wildness 2.
- **I-05 Chronotypes.** Early birds and night owls; shifts the whole daily schedule and sleep need. Effort S, wildness 1.
- **I-06 Perception.** Field of view, line of sight blocked by buildings and large vehicles, blind corners; distracted pedestrians looking at their phones have a narrower field of view. Many crashes come from not seeing. Effort M, wildness 1. Works with G1-G3.
- **I-07 Life course.** Aging, births, deaths, job changes, moving house, buying a car. Turns a year-long run into a slowly evolving city. Effort L, wildness 2. Breaks G9.
- **I-08 Children.** Walking to school, holding a parent's hand (two bodies with a constraint), unpredictable movement near roads. Effort M, wildness 1.
- **I-09 Reduced mobility.** Wheelchairs, strollers, the elderly: slower, need ramps and lifts, and may not make it across before the pedestrian light turns red. A real question for signal timing. Effort S, wildness 1.
- **I-10 Nightlife.** Bars and closing time, intoxicated walking and driving, night buses, a late-night peak. Effort S, wildness 2.
- **I-11 Tourists.** Don't know the map, take wrong turns, walk in slow groups, stop suddenly to take photos. Effort S, wildness 2.
- **I-12 Dog walkers.** Morning and evening walking peaks; the dog is a second body on a leash tied to its owner. Effort S, wildness 3.
- **I-13 Crime ecosystem.** Pickpockets in crowds, police patrol routes with a deterrence effect, car chases. Depiction stays abstract (events and injuries, nothing graphic). Effort L, wildness 2.
- **I-14 Delivery and service workers.** Food couriers on bikes and mopeds, parcel vans double parking, garbage trucks at dawn, snowplows. Effort M, wildness 1.
- **I-15 Inner voice.** Template-generated text from the agent's state: "Late again. This light takes forever." Effort S, wildness 2.
- **I-16 LLM agents.** A handful of agents (only the inspected ones) make decisions or talk through a language model, in the style of "Generative Agents" (Park et al., 2023). Responses are logged as commands to keep runs reproducible. Expensive and slow; never for the whole population. Effort L, wildness 3.
- **I-17 Possess mode.** Take control of one agent and drive or walk yourself. Inputs are logged as commands, so the run stays replayable. Effort M, wildness 3.
- **I-18 Agent biography.** A timeline of one agent's day, week or year: every decision, trip and event, and the reason for each. Effort M, wildness 1.
- **I-19 Elections.** Residents vote on road projects based on how their own commute changed; a political feedback loop for mayor mode (I-80). Effort M, wildness 3.

### 5.3 World and environment

- **I-20 Weather and seasons.** Rain, snow, ice and fog change friction, braking distances, visibility and mode choice (fewer cyclists in snow). Snow accumulates, plows clear priority routes first, snowbanks narrow the lanes. Effort L, wildness 1. Breaks G9.
- **I-21 Daylight by latitude.** Sunrise and sunset computed for the city's latitude (in Helsinki, about six hours of daylight in December). Darkness raises the risk of pedestrian accidents. Effort S, wildness 1.
- **I-22 Sun glare.** When a low sun lines up with the road, drivers' perception drops. Uses the sun's azimuth and the road heading. Effort S, wildness 2.
- **I-23 Daylight saving time.** Clocks shift by an hour twice a year, and everyone's sleep shifts with them. Effort S, wildness 2.
- **I-24 Public transport.** Scheduled buses and trams, stops and boarding times; emergent bus bunching; a metro on an underground level. Effort L, wildness 1.
- **I-25 Micromobility.** Rental e-scooters left at random on sidewalks become obstacles. Effort S, wildness 2.
- **I-26 Autonomous vehicles.** A configurable share of cars drive conservatively and make no errors. Research question: how does mixed autonomy change congestion and crashes? Effort M, wildness 2.
- **I-27 Taxis and ride-hailing.** Empty cruising, pick-ups that stop traffic. Effort M, wildness 1.
- **I-28 Power outage.** Traffic lights go dark and intersections fall back to priority rules; lifts stop. Effort S, wildness 2. Needs G2.
- **I-29 Road works and wear.** Heavy traffic wears roads down, potholes slow traffic, maintenance crews close lanes. Effort M, wildness 2. Breaks G9.
- **I-30 Opening bridges and ferries.** A bridge opens for boats on a schedule and blocks traffic. Effort S, wildness 2.
- **I-31 Wildlife.** Moose and deer crossing roads on the outskirts. Effort S, wildness 2.
- **I-32 Noise and air quality.** Emissions per vehicle, simple dispersion with wind, an exposure heatmap per resident. Effort M, wildness 1.
- **I-33 Mass events.** A concert or match ends and tens of thousands of people leave at once. Effort S, wildness 1.
- **I-34 Epidemic layer.** The simulation already knows who is near whom and for how long. Add an SIR-style infection model: contacts transmit disease, sick agents stay home, the hospital fills up. Test lockdown and remote-work policies. Effort M, wildness 2.
- **I-35 Zombie outbreak.** I-34 with aggressive infected agents chasing others. A playful stress test for evacuation and crowd dynamics. Effort S on top of I-34, wildness 3.
- **I-36 Kaiju mode.** A giant creature walks across the city, destroys buildings and blocks roads while crowds flee. An evacuation stress test in disguise. Effort M, wildness 3.
- **I-37 Disasters.** Flood (low-lying roads close), bridge collapse, earthquake, meteor strike. Effort M each, wildness 3.
- **I-38 Holiday calendar.** The Midsummer exodus when people leave the city, Christmas, May Day crowds, school holidays. Effort S, wildness 1. Needs G7.
- **I-39 Light economy.** Shops earn from visitors and close if nobody comes; new shops open where foot traffic is high; the city slowly reshapes itself. Effort XL, wildness 3. Breaks G9.

### 5.4 Experiments and analysis

- **I-40 What-if fork.** Pause at tick T, fork the world into two branches, change one thing in one branch, run both and compare. Uses determinism directly: until the fork, both branches are identical. Effort M, wildness 1.
- **I-41 Diff view.** Two runs side by side, plus a difference heatmap. Effort S, wildness 1.
- **I-42 Butterfly effect.** Fork and move one agent's alarm clock by one minute. How different is the city by evening? Measure the divergence over time: how sensitive the city is to small changes. Effort S on top of I-40, wildness 3.
- **I-43 Time travel.** Rewind to any snapshot and replay deterministically from it; scrub along the timeline. Effort M, wildness 1.
- **I-44 Braess paradox finder.** Automatically remove each road one at a time, run headless batches, and report roads whose removal reduces total travel time. Effort M, wildness 2.
- **I-45 Optimizer.** Evolve traffic light timings or road layouts with a genetic algorithm or Bayesian optimization over many headless runs. Effort L, wildness 2.
- **I-46 Adaptive traffic lights.** Actuated signals, green waves, learned control (e.g. reinforcement learning), compared against fixed timing. Effort M-L, wildness 2.
- **I-47 Policy levers.** Congestion charge zone, low-emission zone, car-free day, speed limits, roundabouts instead of lights, parking prices. Effort M, wildness 1.
- **I-48 Emergency coverage maps.** Response-time isochrones from each station by hour of day. Where do people die because the ambulance was stuck in traffic? Effort M, wildness 1.
- **I-49 Calibration.** Later, if real data is wanted: open Finnish traffic counts (e.g. Fintraffic's Digitraffic service; availability to be checked) and parameter fitting. Effort L, wildness 1.

### 5.5 Visualization and UX

- **I-50 Desire paths.** Record where pedestrians want to walk but no path exists (they cut across or take a detour). Suggest new footpaths automatically; ties into live road editing. Effort M, wildness 2.
- **I-51 Trails.** Fading lines behind agents; one agent's whole year drawn as a single image. Effort S, wildness 2.
- **I-52 Flow views.** Streamlines and origin-destination Sankey diagrams. Effort M, wildness 1.
- **I-53 2.5D city.** Extruded buildings, shadows by time of day, street lights at night, windows lit when residents are home (occupancy is already known). Effort M, wildness 2.
- **I-54 Agent camera.** Follow an agent; a dashcam-style view; picture-in-picture of several agents at once. Effort S, wildness 1.
- **I-55 Ghosts of yesterday.** Overlay positions from the same time yesterday, or from another branch, as translucent ghosts. Effort S, wildness 2.
- **I-56 Sonification.** City ambience mixed from the simulation state: traffic density, horns when anger is high, sirens. Effort M, wildness 3.
- **I-57 Offline video render.** Since rendering is decoupled, render a 4K timelapse of any run afterwards from the snapshots. Effort M, wildness 1.
- **I-58 VR walk.** Walk among the agents in first person in VR. Effort L, wildness 3.
- **I-59 Minimap.** Heat overlay and event markers. Effort S, wildness 1.
- **I-60 Event director.** An automatic camera that, like a sports broadcast director, cuts to the most interesting thing happening right now: a crash, a growing jam, a fire. Effort M, wildness 2.
- **I-61 Dashboards.** Live charts: average commute, mode split, lateness, incidents per day. Effort S, wildness 1.
- **I-62 City newspaper.** Every simulated morning, a generated front page about yesterday built from the event log: "Three crashes at the Main Street junction", "House fire on Elm Street: fire brigade delayed by rush hour". Templates first, optional LLM. Effort S, wildness 2.

### 5.6 Technology

- **I-70 Event log query console.** Export the event log and layers to Parquet and query them with SQL in the browser (DuckDB-WASM): "agents who were late more than three times this week". Effort M, wildness 1.
- **I-71 Headless CLI runner.** The same core runs batches from the command line. Effort S, wildness 1. Belongs in Phase 1.
- **I-72 GPU core.** Social force, IDM and the spatial hash as WebGPU compute shaders. Effort L, wildness 2.
- **I-73 Volunteer computing.** Friends' browsers run batch seeds; independent seeds parallelize trivially. Effort L, wildness 3.
- **I-74 Replay codes.** Seed + config + command log is a tiny file that reproduces a whole run on the same program version. Share a crash as a code. Effort S, wildness 1.
- **I-75 Mod API.** User scripts (sandboxed JS or WASM) add new agent types, events and buildings. Effort L, wildness 2.
- **I-76 Surrogate model.** A neural network trained on many runs predicts the congestion heatmap from the map instantly, so the editor shows likely congestion while you draw roads. Effort XL, wildness 3.
- **I-77 Multiplayer.** Commands are already serializable and tick-stamped, which is the basis of lockstep multiplayer: several people edit one city, or one plays the mayor while others possess residents. Effort L, wildness 3.

### 5.7 Game layer

- **I-80 Mayor mode.** A budget, road construction costs, taxes from residents. Effort M, wildness 2.
- **I-81 Challenges.** "Get the average commute under 20 minutes without demolishing any homes." Effort S, wildness 2.
- **I-82 Scenario library.** Ready-made scenarios: "Friday rush hour in the rain", "Midsummer exodus", "Power outage at 17:00", "Stadium empties". Effort S, wildness 1.
- **I-83 Chaos dial.** One global slider scales all incident rates. Effort S, wildness 2.
- **I-84 God tools.** Start a fire, spawn a crowd, strike lightning, close a road, toggle rain. Each one is a command, so it is logged. Effort S, wildness 2.

## 6. Decisions on the former open questions (2026-10-17)

1. **Phase 1 gaps: G2, G3 and G6 only.** Intersections, pedestrian crossings and initial state/burn-in come first, because the first research question is where congestion forms. G1 (driver error model), G5 (parking) and G7 (calendar) move out of Phase 1; G1 goes with incidents (Phase 4), G5 and G7 are scheduled before the first year-long run. G4 and G8 stay later. Consequences to accept: until G5 exists, cars still vanish at their destination and parking search cannot cause congestion; until G7 exists, every simulated day is a weekday, so a year-long run is only meaningful after G7.
2. **Population: constant** over a run. Demographics (G8, I-07) come later.
3. **Long runs: sequential weeks.** Learned commute estimates carry over, so parallel weeks are not valid. Parallel runs are for different seeds or scenarios from a shared burn-in snapshot (G6), not for different weeks of one run.

## 7. Open questions

1. Should the data-coloring styles (speed map, state colors, and the by-type colors of the schematic) become overlays that work on top of any base style, instead of being separate styles? Proposal: yes. A speed overlay over blueprint, or state colors over top-down realistic, is more useful than a fixed pairing, and it keeps the base style list short. Config would then have `defaultOverlay` and `allowedOverlays` next to the style fields.
