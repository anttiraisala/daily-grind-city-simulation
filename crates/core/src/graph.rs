//! Road graph: nodes at intersections, directed edges for road segments.
//!
//! Ids are dense indices (`0..n`). Importing a map from JSON will remap its ids.
//! Lanes are stored but a segment is still simulated as one lane (gap G4).

pub type NodeId = u32;
pub type EdgeId = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeKind {
    Road,
    /// Connects two levels (overpass access).
    Ramp,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub x: f64,
    pub y: f64,
    /// Vertical level; nodes on different levels never interact unless joined by a ramp.
    pub level: i8,
}

/// Fixed-time signal for the approach at the end of an edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Signal {
    pub cycle_s: f64,
    pub offset_s: f64,
    pub green_s: f64,
}

impl Signal {
    pub fn is_green(&self, t: f64) -> bool {
        (t - self.offset_s).rem_euclid(self.cycle_s) < self.green_s
    }
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
    pub length: f64,
    pub speed_limit: f64,
    pub lanes: u8,
    pub kind: EdgeKind,
    pub signal: Option<Signal>,
}

#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl Graph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, x: f64, y: f64, level: i8) -> NodeId {
        self.nodes.push(Node { x, y, level });
        (self.nodes.len() - 1) as NodeId
    }

    /// Adds a directed edge whose length is the straight-line distance between its nodes.
    pub fn add_edge(
        &mut self,
        from: NodeId,
        to: NodeId,
        speed_limit: f64,
        lanes: u8,
        kind: EdgeKind,
    ) -> EdgeId {
        let a = &self.nodes[from as usize];
        let b = &self.nodes[to as usize];
        let length = ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt();
        self.edges.push(Edge {
            from,
            to,
            length,
            speed_limit,
            lanes,
            kind,
            signal: None,
        });
        (self.edges.len() - 1) as EdgeId
    }

    pub fn set_signal(&mut self, edge: EdgeId, signal: Signal) {
        self.edges[edge as usize].signal = Some(signal);
    }

    /// Checks structural rules; returns the first problem found.
    pub fn validate(&self) -> Result<(), String> {
        for (i, e) in self.edges.iter().enumerate() {
            if e.from as usize >= self.nodes.len() || e.to as usize >= self.nodes.len() {
                return Err(format!("edge {i}: unknown node"));
            }
            if e.from == e.to {
                return Err(format!("edge {i}: starts and ends at the same node"));
            }
            if e.length <= 0.0 || e.length.is_nan() {
                return Err(format!("edge {i}: length must be positive"));
            }
            if e.speed_limit <= 0.0 || e.speed_limit.is_nan() {
                return Err(format!("edge {i}: speed limit must be positive"));
            }
            if e.lanes == 0 {
                return Err(format!("edge {i}: needs at least one lane"));
            }
            if let Some(s) = e.signal {
                if s.cycle_s <= 0.0 || s.green_s < 0.0 || s.green_s > s.cycle_s {
                    return Err(format!("edge {i}: invalid signal timing"));
                }
            }
        }
        Ok(())
    }

    /// True if `b` can follow `a` on a route.
    pub fn connects(&self, a: EdgeId, b: EdgeId) -> bool {
        self.edges[a as usize].to == self.edges[b as usize].from
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_comes_from_node_positions() {
        let mut g = Graph::new();
        let a = g.add_node(0.0, 0.0, 0);
        let b = g.add_node(30.0, 40.0, 0);
        let e = g.add_edge(a, b, 10.0, 1, EdgeKind::Road);
        assert!((g.edges[e as usize].length - 50.0).abs() < 1e-9);
        assert!(g.validate().is_ok());
    }

    #[test]
    fn validation_rejects_bad_edges() {
        let mut g = Graph::new();
        let a = g.add_node(0.0, 0.0, 0);
        let b = g.add_node(0.0, 0.0, 0);
        g.add_edge(a, b, 10.0, 1, EdgeKind::Road);
        assert!(g.validate().is_err()); // zero length
        let mut g = Graph::new();
        let a = g.add_node(0.0, 0.0, 0);
        let b = g.add_node(10.0, 0.0, 0);
        let e = g.add_edge(a, b, 10.0, 1, EdgeKind::Road);
        g.set_signal(
            e,
            Signal {
                cycle_s: 10.0,
                offset_s: 0.0,
                green_s: 20.0,
            },
        );
        assert!(g.validate().is_err()); // green longer than cycle
    }

    #[test]
    fn signal_cycles() {
        let s = Signal {
            cycle_s: 60.0,
            offset_s: 10.0,
            green_s: 30.0,
        };
        assert!(!s.is_green(5.0));
        assert!(s.is_green(10.0));
        assert!(s.is_green(39.9));
        assert!(!s.is_green(40.0));
        assert!(s.is_green(70.0));
    }
}
