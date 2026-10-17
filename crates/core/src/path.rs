//! Shortest paths. One Dijkstra run per destination gives a "route tree": from any
//! node, follow the stored next edges to the destination. Many residents share a
//! destination, so this is far cheaper than a search per trip.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Reverse adjacency: `incoming[v]` lists `(u, cost, token)` meaning that taking the
/// edge `token` from `u` leads to `v` at the given cost.
pub struct Incoming {
    pub incoming: Vec<Vec<(u32, f64, u32)>>,
}

pub struct RouteTree {
    pub dest: u32,
    /// Edge token to take from each node towards `dest` (`u32::MAX` = no route).
    pub next_edge: Vec<u32>,
    pub next_node: Vec<u32>,
    /// Cost from each node to `dest` (infinite = no route).
    pub cost: Vec<f64>,
}

pub fn build_tree(g: &Incoming, dest: u32) -> RouteTree {
    let n = g.incoming.len();
    let mut cost = vec![f64::INFINITY; n];
    let mut next_edge = vec![u32::MAX; n];
    let mut next_node = vec![u32::MAX; n];
    let mut heap: BinaryHeap<Reverse<(u64, u32)>> = BinaryHeap::new();
    cost[dest as usize] = 0.0;
    heap.push(Reverse((0f64.to_bits(), dest)));
    while let Some(Reverse((bits, v))) = heap.pop() {
        let c = f64::from_bits(bits);
        if c > cost[v as usize] {
            continue;
        }
        for &(u, w, token) in &g.incoming[v as usize] {
            let nc = c + w;
            // Strictly better, or equal with a smaller edge token: deterministic ties.
            let better =
                nc < cost[u as usize] || (nc == cost[u as usize] && token < next_edge[u as usize]);
            if better {
                cost[u as usize] = nc;
                next_edge[u as usize] = token;
                next_node[u as usize] = v;
                heap.push(Reverse((nc.to_bits(), u)));
            }
        }
    }
    RouteTree {
        dest,
        next_edge,
        next_node,
        cost,
    }
}

impl RouteTree {
    /// Edge tokens from `src` to the destination; `None` if unreachable.
    pub fn route(&self, src: u32) -> Option<Vec<u32>> {
        if self.cost[src as usize].is_infinite() {
            return None;
        }
        let mut out = Vec::new();
        let mut v = src;
        while v != self.dest {
            out.push(self.next_edge[v as usize]);
            v = self.next_node[v as usize];
            if out.len() > self.cost.len() {
                return None; // cannot happen with non-negative costs; guards against loops
            }
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_cheaper_route_and_reports_unreachable() {
        // 0 -> 1 -> 3 costs 2; 0 -> 2 -> 3 costs 5; node 4 is isolated.
        let mut inc = vec![Vec::new(); 5];
        inc[1].push((0, 1.0, 10));
        inc[3].push((1, 1.0, 11));
        inc[2].push((0, 2.0, 12));
        inc[3].push((2, 3.0, 13));
        let t = build_tree(&Incoming { incoming: inc }, 3);
        assert_eq!(t.route(0), Some(vec![10, 11]));
        assert_eq!(t.cost[0], 2.0);
        assert_eq!(t.route(3), Some(vec![]));
        assert_eq!(t.route(4), None);
    }

    #[test]
    fn ties_break_by_smaller_edge_token() {
        let mut inc = vec![Vec::new(); 4];
        inc[1].push((0, 1.0, 7));
        inc[2].push((0, 1.0, 5));
        inc[3].push((1, 1.0, 20));
        inc[3].push((2, 1.0, 21));
        let t = build_tree(&Incoming { incoming: inc }, 3);
        assert_eq!(t.route(0), Some(vec![5, 21]));
    }
}
