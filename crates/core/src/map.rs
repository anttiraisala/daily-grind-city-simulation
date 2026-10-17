//! Loads and validates a map file (see `schema/types.ts` and `docs/city-editor.md`).
//!
//! Two-way road edges become two directed edges. Footpaths and crossings form a separate
//! pedestrian graph over the same nodes. Ids in the file are arbitrary and are remapped to
//! dense indices; `MapData::node_ids` maps back.

use crate::graph::{EdgeId, EdgeKind, Graph, NodeId, Signal};
use crate::json::{self, Value};
use crate::path::Incoming;
use crate::traffic::Crossing;
use std::collections::HashMap;

pub const DEFAULT_LANE_WIDTH_M: f64 = 3.5;
pub const DEFAULT_FOOTPATH_WIDTH_M: f64 = 2.0;
pub const DEFAULT_CROSSING_WIDTH_M: f64 = 3.0;
pub const DEFAULT_ROAD_SPEED: f64 = 13.9;

#[derive(Clone, Debug)]
pub struct BuildingData {
    pub id: u32,
    pub kind: String,
    pub footprint: Vec<(f64, f64)>,
    /// Road node for vehicles.
    pub entrance: NodeId,
    /// Pedestrian node.
    pub door: NodeId,
    pub residents: u32,
    pub jobs: u32,
    pub visitors: u32,
    pub open: Vec<(u32, u32)>,
}

#[derive(Clone, Debug)]
pub struct FootEdge {
    pub a: NodeId,
    pub b: NodeId,
    pub length: f64,
    /// Index into `MapData::crossings` if this edge is a pedestrian crossing.
    pub crossing: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct FootGraph {
    pub edges: Vec<FootEdge>,
}

#[derive(Clone, Debug)]
pub struct MapData {
    pub name: String,
    pub graph: Graph,
    pub foot: FootGraph,
    pub crossings: Vec<Crossing>,
    pub buildings: Vec<BuildingData>,
    /// Dense node index to the id used in the file.
    pub node_ids: Vec<u32>,
    /// True if the file had no footpaths and they were derived from the roads
    /// (such implied footpaths never conflict with traffic).
    pub derived_footpaths: bool,
    pub warnings: Vec<String>,
    pub lane_width_m: f64,
    pub footpath_width_m: f64,
}

fn req_f64(v: &Value, key: &str, what: &str) -> Result<f64, String> {
    v.get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("{what}: missing number \"{key}\""))
}

fn opt_f64(v: &Value, key: &str, default: f64) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(default)
}

fn req_id(v: &Value, key: &str, what: &str) -> Result<u32, String> {
    let n = req_f64(v, key, what)?;
    if n < 0.0 || n.fract() != 0.0 || n > u32::MAX as f64 {
        return Err(format!("{what}: \"{key}\" must be a whole number"));
    }
    Ok(n as u32)
}

fn list<'a>(root: &'a Value, key: &str) -> Result<&'a [Value], String> {
    match root.get(key) {
        None => Ok(&[]),
        Some(v) => v
            .as_array()
            .ok_or_else(|| format!("\"{key}\" must be a list")),
    }
}

pub fn load_map(text: &str) -> Result<MapData, String> {
    let root = json::parse(text)?;
    let name = root
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("unnamed")
        .to_string();
    let lane_width_m = root
        .at(&["defaults", "laneWidthM"])
        .and_then(Value::as_f64)
        .unwrap_or(DEFAULT_LANE_WIDTH_M);
    let footpath_width_m = root
        .at(&["defaults", "footpathWidthM"])
        .and_then(Value::as_f64)
        .unwrap_or(DEFAULT_FOOTPATH_WIDTH_M);
    let mut warnings = Vec::new();

    // Nodes.
    let mut graph = Graph::new();
    let mut node_ids = Vec::new();
    let mut node_index: HashMap<u32, NodeId> = HashMap::new();
    let mut node_light: Vec<Option<u32>> = Vec::new();
    for n in list(&root, "nodes")? {
        let id = req_id(n, "id", "node")?;
        let what = format!("node {id}");
        let pos = n
            .get("pos")
            .ok_or_else(|| format!("{what}: missing \"pos\""))?;
        let level = opt_f64(n, "level", 0.0);
        if level.fract() != 0.0 || !(-100.0..=100.0).contains(&level) {
            return Err(format!("{what}: bad level"));
        }
        let x = req_f64(pos, "x", &what)?;
        let y = req_f64(pos, "y", &what)?;
        if node_index.insert(id, graph.nodes.len() as NodeId).is_some() {
            return Err(format!("{what}: duplicate id"));
        }
        graph.add_node(x, y, level as i8);
        node_ids.push(id);
        node_light.push(
            n.get("trafficLightId")
                .and_then(Value::as_f64)
                .map(|f| f as u32),
        );
    }
    let node_of = |id: u32, what: &str| -> Result<NodeId, String> {
        node_index
            .get(&id)
            .copied()
            .ok_or_else(|| format!("{what}: unknown node {id}"))
    };

    // Road edges first (crossings need them).
    let edges = list(&root, "edges")?;
    let mut seen_edge_ids: HashMap<u32, ()> = HashMap::new();
    let mut road_dirs: HashMap<u32, (EdgeId, Option<EdgeId>)> = HashMap::new();
    for e in edges {
        let id = req_id(e, "id", "edge")?;
        let what = format!("edge {id}");
        if seen_edge_ids.insert(id, ()).is_some() {
            return Err(format!("{what}: duplicate id"));
        }
        let kind = e.get("kind").and_then(Value::as_str).unwrap_or("road");
        match kind {
            "road" | "ramp" => {
                let from = node_of(req_id(e, "from", &what)?, &what)?;
                let to = node_of(req_id(e, "to", &what)?, &what)?;
                let lanes = opt_f64(e, "lanes", 1.0);
                if !(1.0..=8.0).contains(&lanes) || lanes.fract() != 0.0 {
                    return Err(format!("{what}: lanes must be 1..8"));
                }
                let limit = opt_f64(e, "speedLimit", DEFAULT_ROAD_SPEED);
                let ek = if kind == "ramp" {
                    EdgeKind::Ramp
                } else {
                    EdgeKind::Road
                };
                let one_way = e.get("oneWay").and_then(Value::as_bool).unwrap_or(false);
                let fwd = graph.add_edge(from, to, limit, lanes as u8, ek);
                let bwd = if one_way {
                    None
                } else {
                    Some(graph.add_edge(to, from, limit, lanes as u8, ek))
                };
                road_dirs.insert(id, (fwd, bwd));
            }
            "footpath" | "crossing" => {}
            "bikelane" => {
                warnings.push(format!("{what}: bike lanes are not simulated yet; ignored"))
            }
            other => return Err(format!("{what}: unknown kind \"{other}\"")),
        }
    }
    graph.validate()?;

    // Footpaths and crossings.
    let mut foot = FootGraph::default();
    let mut crossings: Vec<Crossing> = Vec::new();
    let mut crossing_of_map_edge: HashMap<u32, usize> = HashMap::new();
    for e in edges {
        let kind = e.get("kind").and_then(Value::as_str).unwrap_or("road");
        if kind != "footpath" && kind != "crossing" {
            continue;
        }
        let id = req_id(e, "id", "edge")?;
        let what = format!("edge {id}");
        let a = node_of(req_id(e, "from", &what)?, &what)?;
        let b = node_of(req_id(e, "to", &what)?, &what)?;
        let (na, nb) = (&graph.nodes[a as usize], &graph.nodes[b as usize]);
        let length = ((na.x - nb.x).powi(2) + (na.y - nb.y).powi(2)).sqrt();
        if length <= 0.0 {
            return Err(format!("{what}: zero length"));
        }
        let mut crossing = None;
        if kind == "crossing" {
            let c = e
                .get("crossing")
                .ok_or_else(|| format!("{what}: a crossing needs a \"crossing\" object"))?;
            let road_id = req_id(c, "edgeId", &what)?;
            let t = req_f64(c, "t", &what)?;
            if !(t > 0.0 && t < 1.0) {
                return Err(format!("{what}: crossing t must be between 0 and 1"));
            }
            let &(fwd, bwd) = road_dirs
                .get(&road_id)
                .ok_or_else(|| format!("{what}: crosses unknown road edge {road_id}"))?;
            let len = graph.edges[fwd as usize].length;
            let mut road = vec![(fwd, t * len)];
            if let Some(b) = bwd {
                road.push((b, (1.0 - t) * len));
            }
            let width = opt_f64(e, "widthM", DEFAULT_CROSSING_WIDTH_M);
            if width <= 0.0 || width > 20.0 {
                return Err(format!("{what}: bad crossing width"));
            }
            crossing_of_map_edge.insert(id, crossings.len());
            crossing = Some(crossings.len());
            crossings.push(Crossing {
                road,
                width,
                walk: None,
            });
        }
        foot.edges.push(FootEdge {
            a,
            b,
            length,
            crossing,
        });
    }
    let mut derived_footpaths = false;
    if foot.edges.is_empty() {
        derived_footpaths = true;
        let mut ids: Vec<&u32> = road_dirs.keys().collect();
        ids.sort();
        for id in ids {
            let (fwd, _) = road_dirs[id];
            let ed = &graph.edges[fwd as usize];
            foot.edges.push(FootEdge {
                a: ed.from,
                b: ed.to,
                length: ed.length,
                crossing: None,
            });
        }
    }

    // Traffic lights.
    let mut light_ids: HashMap<u32, NodeId> = HashMap::new();
    for l in list(&root, "trafficLights")? {
        let id = req_id(l, "id", "traffic light")?;
        let what = format!("traffic light {id}");
        let node = node_of(req_id(l, "nodeId", &what)?, &what)?;
        if light_ids.insert(id, node).is_some() {
            return Err(format!("{what}: duplicate id"));
        }
        let cycle = req_f64(l, "cycleSeconds", &what)?;
        if cycle <= 0.0 {
            return Err(format!("{what}: cycleSeconds must be positive"));
        }
        for ph in l.get("phases").and_then(Value::as_array).unwrap_or(&[]) {
            let start = opt_f64(ph, "startSecond", 0.0);
            let green = req_f64(ph, "greenSeconds", &what)?;
            if green < 0.0 || green > cycle || start < 0.0 || start >= cycle {
                return Err(format!("{what}: bad phase timing"));
            }
            let signal = Signal {
                cycle_s: cycle,
                offset_s: start,
                green_s: green,
            };
            for eid in ph.get("edgeIds").and_then(Value::as_array).unwrap_or(&[]) {
                let eid = eid
                    .as_f64()
                    .ok_or_else(|| format!("{what}: edgeIds must be numbers"))?
                    as u32;
                if let Some(&(fwd, bwd)) = road_dirs.get(&eid) {
                    let approach = [Some(fwd), bwd]
                        .into_iter()
                        .flatten()
                        .find(|&d| graph.edges[d as usize].to == node)
                        .ok_or_else(|| {
                            format!("{what}: edge {eid} does not end at the light's node")
                        })?;
                    if graph.edges[approach as usize]
                        .signal
                        .replace(signal)
                        .is_some()
                    {
                        return Err(format!("{what}: edge {eid} is in more than one phase"));
                    }
                } else if let Some(&ci) = crossing_of_map_edge.get(&eid) {
                    if crossings[ci].walk.replace(signal).is_some() {
                        return Err(format!("{what}: crossing {eid} is in more than one phase"));
                    }
                } else {
                    return Err(format!("{what}: unknown edge {eid} in a phase"));
                }
            }
        }
    }
    for (i, l) in node_light.iter().enumerate() {
        if let Some(lid) = l {
            match light_ids.get(lid) {
                Some(&n) if n as usize == i => {}
                _ => {
                    return Err(format!(
                        "node {}: trafficLightId {lid} does not match a light at this node",
                        node_ids[i]
                    ))
                }
            }
        }
    }
    for (&lid, &node) in &light_ids {
        for (i, e) in graph.edges.iter().enumerate() {
            if e.to == node && e.signal.is_none() && e.kind == EdgeKind::Road {
                warnings.push(format!(
                    "traffic light {lid}: approach edge {i} (directed) has no phase and is always green"
                ));
            }
        }
    }
    warnings.sort();

    // Buildings.
    let mut buildings = Vec::new();
    for b in list(&root, "buildings")? {
        let id = req_id(b, "id", "building")?;
        let what = format!("building {id}");
        let kind = b
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("other")
            .to_string();
        let mut footprint = Vec::new();
        for p in b.get("footprint").and_then(Value::as_array).unwrap_or(&[]) {
            footprint.push((req_f64(p, "x", &what)?, req_f64(p, "y", &what)?));
        }
        if footprint.len() < 3 {
            return Err(format!("{what}: footprint needs at least 3 points"));
        }
        let entrance = node_of(req_id(b, "entranceNodeId", &what)?, &what)?;
        let door = match b.get("doorNodeId").and_then(Value::as_f64) {
            Some(d) => node_of(d as u32, &what)?,
            None => entrance,
        };
        let cap = |k: &str| {
            b.at(&["capacity", k])
                .and_then(Value::as_f64)
                .unwrap_or(0.0) as u32
        };
        let mut open = Vec::new();
        for w in b.get("open").and_then(Value::as_array).unwrap_or(&[]) {
            open.push((
                opt_f64(w, "from", 0.0) as u32,
                opt_f64(w, "to", 1440.0) as u32,
            ));
        }
        buildings.push(BuildingData {
            id,
            kind,
            footprint,
            entrance,
            door,
            residents: cap("residents"),
            jobs: cap("jobs"),
            visitors: cap("visitors"),
            open,
        });
    }

    Ok(MapData {
        name,
        graph,
        foot,
        crossings,
        buildings,
        node_ids,
        derived_footpaths,
        warnings,
        lane_width_m,
        footpath_width_m,
    })
}

/// Reverse adjacency of the road graph for route trees; the token is the directed edge id.
pub fn road_incoming(graph: &Graph) -> Incoming {
    let mut incoming = vec![Vec::new(); graph.nodes.len()];
    for (i, e) in graph.edges.iter().enumerate() {
        incoming[e.to as usize].push((e.from, e.length / e.speed_limit, i as u32));
    }
    Incoming { incoming }
}

/// Reverse adjacency of the pedestrian graph; the token is `(edge << 1) | reversed`.
pub fn foot_incoming(foot: &FootGraph, nodes: usize) -> Incoming {
    let mut incoming = vec![Vec::new(); nodes];
    for (i, e) in foot.edges.iter().enumerate() {
        incoming[e.b as usize].push((e.a, e.length, (i as u32) << 1));
        incoming[e.a as usize].push((e.b, e.length, ((i as u32) << 1) | 1));
    }
    Incoming { incoming }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = include_str!("../../../schema/map.example.json");

    #[test]
    fn the_example_map_loads() {
        let m = load_map(EXAMPLE).unwrap();
        assert_eq!(m.name, "example_block");
        assert_eq!(m.graph.nodes.len(), 7);
        assert_eq!(m.graph.edges.len(), 14); // seven two-way edges
        assert_eq!(m.buildings.len(), 4);
        assert!(m.derived_footpaths);
        assert_eq!(m.lane_width_m, 3.5);
        // The light at node 2 gives green to the approaches of edges 1,2 and 3,4.
        let signalled = m.graph.edges.iter().filter(|e| e.signal.is_some()).count();
        assert_eq!(signalled, 4);
    }

    fn small(extra_edges: &str, lights: &str) -> String {
        format!(
            r#"{{"nodes":[
              {{"id":1,"pos":{{"x":0,"y":0}}}},{{"id":2,"pos":{{"x":100,"y":0}}}},
              {{"id":3,"pos":{{"x":50,"y":-4}}}},{{"id":4,"pos":{{"x":50,"y":4}}}}],
             "edges":[{{"id":1,"from":1,"to":2,"kind":"road","lanes":1,"speedLimit":10}}{extra_edges}],
             "trafficLights":[{lights}]}}"#
        )
    }

    #[test]
    fn a_crossing_links_to_both_directions_of_the_road() {
        let cross =
            r#",{"id":2,"from":3,"to":4,"kind":"crossing","crossing":{"edgeId":1,"t":0.3}}"#;
        let m = load_map(&small(cross, "")).unwrap();
        assert!(!m.derived_footpaths);
        assert_eq!(m.crossings.len(), 1);
        let c = &m.crossings[0];
        assert_eq!(c.road.len(), 2);
        assert!((c.road[0].1 - 30.0).abs() < 1e-9);
        assert!((c.road[1].1 - 70.0).abs() < 1e-9);
        assert!(c.walk.is_none());
        assert_eq!(m.foot.edges[0].crossing, Some(0));
    }

    #[test]
    fn a_signalised_crossing_gets_its_walk_signal() {
        let cross =
            r#",{"id":2,"from":3,"to":4,"kind":"crossing","crossing":{"edgeId":1,"t":0.5}}"#;
        let light = r#"{"id":1,"nodeId":1,"cycleSeconds":60,"phases":[{"edgeIds":[2],"startSecond":20,"greenSeconds":15}]}"#;
        let m = load_map(&small(cross, light)).unwrap();
        let s = m.crossings[0].walk.unwrap();
        assert_eq!((s.cycle_s, s.offset_s, s.green_s), (60.0, 20.0, 15.0));
    }

    #[test]
    fn bad_maps_are_rejected_with_a_reason() {
        let bad_cross =
            r#",{"id":2,"from":3,"to":4,"kind":"crossing","crossing":{"edgeId":99,"t":0.5}}"#;
        assert!(load_map(&small(bad_cross, ""))
            .unwrap_err()
            .contains("unknown road edge"));
        let bad_t =
            r#",{"id":2,"from":3,"to":4,"kind":"crossing","crossing":{"edgeId":1,"t":1.5}}"#;
        assert!(load_map(&small(bad_t, ""))
            .unwrap_err()
            .contains("between 0 and 1"));
        let dup = r#",{"id":1,"from":2,"to":1,"kind":"road"}"#;
        assert!(load_map(&small(dup, "")).unwrap_err().contains("duplicate"));
        let ghost = r#",{"id":2,"from":1,"to":77,"kind":"road"}"#;
        assert!(load_map(&small(ghost, ""))
            .unwrap_err()
            .contains("unknown node 77"));
        let wrong_node = r#"{"id":1,"nodeId":3,"cycleSeconds":60,"phases":[{"edgeIds":[1],"startSecond":0,"greenSeconds":30}]}"#;
        assert!(load_map(&small("", wrong_node))
            .unwrap_err()
            .contains("does not end at"));
        let twice = r#"{"id":1,"nodeId":2,"cycleSeconds":60,"phases":[{"edgeIds":[1],"startSecond":0,"greenSeconds":30},{"edgeIds":[1],"startSecond":30,"greenSeconds":30}]}"#;
        assert!(load_map(&small("", twice))
            .unwrap_err()
            .contains("more than one phase"));
        assert!(load_map("{").is_err());
    }
}
