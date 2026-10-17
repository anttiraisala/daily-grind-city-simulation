// Traffic congestion simulation: core data structures (draft v0.1)
// Time: minutes since midnight (0..1440). Lengths in meters, speeds in m/s.

export type Minutes = number;
export type Level = number; // z-level: 0 = ground level, 1 = overpass, etc.
export type Id = number;

export interface Vec2 { x: number; y: number }

// ---------------------------------------------------------------- MAP

// road: carries vehicles (lanes per direction). footpath: carries pedestrians.
// crossing: a footpath edge that crosses a road edge (see RoadEdge.crossing).
// ramp: a road edge that connects two levels. bikelane: reserved for later.
export type EdgeKind = "road" | "footpath" | "crossing" | "bikelane" | "ramp";

export interface MapDefaults {
  laneWidthM: number;        // road width = lanes * laneWidthM (per direction, drawn both ways)
  footpathWidthM: number;
}

export interface RoadNode {
  id: Id;
  pos: Vec2;
  level: Level;
  trafficLightId?: Id;
}

export interface RoadEdge {
  id: Id;
  from: Id;
  to: Id;
  kind: EdgeKind;
  lanes: number;
  speedLimit: number;          // m/s
  oneWay: boolean;
  laneWidthM?: number;         // overrides the map default; road width = lanes * laneWidthM
  // Only for kind "crossing": which road edge it crosses and where (t = 0..1 along that
  // edge, from its "from" node). Pedestrians cross on foot; vehicles on that road edge
  // (both directions if two-way) must stop while the crossing is occupied or its walk
  // signal is active. A crossing that appears in no traffic-light phase is a zebra crossing.
  crossing?: { edgeId: Id; t: number };
  widthM?: number;             // crossing width along the road, default 3 m
  // A ramp connects levels: the level values of its end nodes tell from where to where.
}

export interface TrafficLight {
  id: Id;
  nodeId: Id;
  cycleSeconds: number;
  // edgeIds may name road edges (vehicles of that approach get green) and crossing edges
  // (pedestrians may walk). An edge may appear in at most one phase for now.
  phases: { edgeIds: Id[]; startSecond: number; greenSeconds: number }[];
}

export type BuildingKind =
  | "home" | "office" | "factory" | "shop" | "restaurant" | "school"
  | "hospital" | "police" | "firestation" | "other";

export interface OpeningWindow { from: Minutes; to: Minutes } // 0..1440; 24h = {0,1440}

export interface Building {
  id: Id;
  kind: BuildingKind;
  footprint: Vec2[];           // polygon
  entranceNodeId: Id;          // road node where vehicles start and end trips here
  doorNodeId?: Id;             // footpath node where pedestrians enter and leave (default: entranceNodeId)
  capacity: { residents?: number; jobs?: number; visitors?: number };
  open: OpeningWindow[];       // opening hours of a workplace or shop
  flammability: number;        // 0..1
  fire?: { startedAtTick: number; intensity: number }; // runtime state
}

// A mobile workplace (e.g. a bus): the driver works inside the vehicle.
export interface MobileWorkplace {
  id: Id;
  vehicleId: Id;
  routeEdgeIds: Id[];
  open: OpeningWindow[];
}

// ---------------------------------------------------------------- AGENT

// Persistent traits (do not change on their own)
export interface Traits {
  bodyDiameterM: number;  // body seen from above, drawn as a circle (default 0.5 m)
  spaceNeed: number;      // wanted free gap between this person's body edge and others, m (typical 0.2..1.0, any value >= 0 allowed)
  punctuality: number;    // 0..1
  carelessness: number;   // 0..1, carelessness in traffic
  aggression: number;     // 0..1
  sociability: number;    // 0..1
  impulsivity: number;    // 0..1
  medicalRisk: number;    // 0..1, susceptibility to medical episodes
  violentIntent: boolean; // rare: wants to harm others
}

// Needs (change within minutes), 0 = satisfied, 1 = critical
export interface Needs {
  hunger: number;
  bladder: number;
  sleep: number;
  social: number;     // desire to meet others
  solitude: number;   // desire to be alone
}

// Mood (changes within minutes or faster)
export interface Mood {
  anger: number;
  stress: number;
}

// Binding commitment: this is what creates the rhythm of congestion.
export interface Commitment {
  kind: "work" | "lunch" | "school" | "shift";
  start: Minutes;
  end: Minutes;
  buildingId?: Id;
  mobileWorkplaceId?: Id;
  remote: boolean;     // remote work: no commute
}

// Learned estimate of commute duration, updated from completed trips.
export interface CommuteEstimate {
  routeKey: string;    // e.g. "home:12->work:40:car"
  minutes: number;
  samples: number;
}

export type GoalReason =
  | "commitment" | "need" | "mood" | "random" | "forcedByUser" | "emergency" | "crime";

export interface Goal {
  kind: "goto" | "stay" | "sleep" | "eat" | "work" | "socialize" | "flee" | "harm" | "none";
  targetBuildingId?: Id;
  targetAgentId?: Id;
  reason: GoalReason;
  forced: boolean;     // set by the user or by an exceptional state
  untilTick?: number;
}

// Two movement modes: following a lane (cheap) or moving as a free body (rare).
export type MovementMode = "lane" | "free" | "inside";

export interface Agent {
  id: Id;
  seed: number;                 // seed of the agent's own random number generator
  traits: Traits;
  needs: Needs;
  mood: Mood;
  homeId: Id;
  commitments: Commitment[];    // binding commitments for the day
  commuteEstimates: CommuteEstimate[];
  goal: Goal;
  mode: MovementMode;
  pos: Vec2;
  level: Level;
  vel: Vec2;
  vehicleId?: Id;
  insideBuildingId?: Id;        // inside: takes no space, no physics
  health: number;               // 0..1
}

export type VehicleKind = "car" | "bike" | "motorcycle" | "bus" | "ambulance" | "police" | "firetruck";

export interface Vehicle {
  id: Id;
  kind: VehicleKind;
  lengthM: number;
  widthM: number;
  ownerId?: Id;
  mode: MovementMode;
  pos: Vec2;
  heading: number;
  speed: number;
  level: Level;
  edgeId?: Id;                  // in lane mode
}

// ---------------------------------------------------------------- EVENTS AND COMMANDS

export type SimEventKind =
  | "arrived" | "late" | "skippedWork" | "medicalEpisode" | "crash"
  | "hitPedestrian" | "crimeAttempt" | "fireStarted" | "fireSpread"
  | "fireOut" | "emergencyDispatched" | "emergencyArrived" | "death";

export interface SimEvent {
  tick: number;
  kind: SimEventKind;
  pos: Vec2;
  level: Level;
  agentIds: Id[];
  vehicleIds: Id[];
  buildingId?: Id;
  data?: Record<string, number | string>;
}

// All manual changes are logged as commands at a specific tick, so a run
// is reproducible on the same machine with the same program version.
export type Command =
  | { tick: number; kind: "setNeeds"; agentId: Id; needs: Partial<Needs> }
  | { tick: number; kind: "setMood"; agentId: Id; mood: Partial<Mood> }
  | { tick: number; kind: "setTraits"; agentId: Id; traits: Partial<Traits> }
  | { tick: number; kind: "forceGoal"; agentId: Id; goal: Goal }
  | { tick: number; kind: "addEdge"; edge: RoadEdge }
  | { tick: number; kind: "removeEdge"; edgeId: Id }
  | { tick: number; kind: "startFire"; buildingId: Id }
  | { tick: number; kind: "setPaused"; paused: boolean };

// ---------------------------------------------------------------- ACCUMULATION MAP

// One array per metric: cells x time bins (e.g. 24 hourly bins).
export interface AccumulationLayer {
  cellSizeM: number;
  width: number;
  height: number;
  binsPerDay: number;
  dwellSeconds: Float32Array;   // agent-seconds
  speedSum: Float32Array;       // sum of speeds (divide by the sample count)
  speedSamples: Uint32Array;
  slowSeconds: Float32Array;    // time with speed below the threshold (congestion)
}

// ---------------------------------------------------------------- VIEWER

// How the world is drawn. Purely a viewer setting: it never changes the simulation,
// is not a command and is not part of the run log. Geometry is always true to scale
// (meters from the map and scenario); styles only change how that geometry looks.
export type VisualStyle =
  | "blueprint" | "flat-map" | "neon" | "schematic" | "isometric"
  | "top-down-realistic" | "speed-map" | "ink" | "pixel" | "state-colors";

export interface ViewerConfig {
  defaultStyle: VisualStyle;        // must be one of allowedStyles
  allowedStyles: VisualStyle[];     // the user can switch between these on the fly
  strictScale: boolean;             // true: never enlarge anything, small things vanish when zoomed out
  minScreenPx: { person: number; vehicle: number }; // when strictScale is false: smallest drawn size, in screen pixels
  rememberChoice: boolean;          // keep the user's last style per browser
}
