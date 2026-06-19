//! Layered left → right auto-layout for blueprint graphs.
//!
//! Assigns each node an x-column by its longest dependency chain (a connection
//! `A → B` puts `A` left of `B`, covering both exec and data wires) and a y-row
//! by a single barycenter pass (each node sits near the average height of the
//! nodes feeding it), which keeps wires mostly short and untangled. It's a
//! deliberately simple Sugiyama-style pass — good enough to turn a pile of
//! overlapping nodes into a readable diagram, which is all "Auto Layout" needs.

use crate::graph::{BlueprintGraph, NodeId};
use std::collections::{BTreeMap, HashMap};

/// Horizontal gap between columns (node width + margin).
const COL_W: f32 = 320.0;
/// Vertical gap between rows within a column.
const ROW_H: f32 = 150.0;

/// Re-position every node in `graph` into tidy dependency-ordered columns.
/// Connections and node identities are untouched — only `position` changes.
pub fn auto_layout(graph: &mut BlueprintGraph) {
    if graph.nodes.is_empty() {
        return;
    }
    let ids: Vec<NodeId> = graph.nodes.iter().map(|n| n.id).collect();

    // Longest-path rank via bounded relaxation (handles accidental cycles by
    // capping iterations at the node count rather than looping forever).
    let mut rank: HashMap<NodeId, i32> = ids.iter().map(|id| (*id, 0)).collect();
    for _ in 0..ids.len() {
        let mut changed = false;
        for c in &graph.connections {
            if c.from_node == c.to_node {
                continue;
            }
            let fr = *rank.get(&c.from_node).unwrap_or(&0);
            if rank.get(&c.to_node).copied().unwrap_or(0) < fr + 1 {
                rank.insert(c.to_node, fr + 1);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    // Predecessors per node, for barycenter ordering within a column.
    let mut preds: HashMap<NodeId, Vec<NodeId>> = ids.iter().map(|id| (*id, Vec::new())).collect();
    for c in &graph.connections {
        if c.from_node != c.to_node {
            if let Some(v) = preds.get_mut(&c.to_node) {
                v.push(c.from_node);
            }
        }
    }

    // Group node ids by column (rank), preserving a stable base order.
    let mut columns: BTreeMap<i32, Vec<NodeId>> = BTreeMap::new();
    for id in &ids {
        let r = rank.get(id).copied().unwrap_or(0).max(0);
        columns.entry(r).or_default().push(*id);
    }

    // Place column by column, left to right. Within each column, sort by the
    // average y of already-placed predecessors so related nodes line up.
    let mut y_of: HashMap<NodeId, f32> = HashMap::new();
    for (col_rank, members) in columns.iter() {
        let mut ordered: Vec<NodeId> = members.clone();
        ordered.sort_by(|a, b| {
            let bary = |n: &NodeId| -> f32 {
                let ps = &preds[n];
                let placed: Vec<f32> = ps.iter().filter_map(|p| y_of.get(p).copied()).collect();
                if placed.is_empty() {
                    f32::MAX // unconnected/source nodes sink to the bottom of the column
                } else {
                    placed.iter().sum::<f32>() / placed.len() as f32
                }
            };
            bary(a)
                .partial_cmp(&bary(b))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.cmp(b))
        });
        let x = *col_rank as f32 * COL_W;
        for (row, id) in ordered.iter().enumerate() {
            let y = row as f32 * ROW_H;
            y_of.insert(*id, y);
            if let Some(node) = graph.get_node_mut(*id) {
                node.position = [x, y];
            }
        }
    }
}
