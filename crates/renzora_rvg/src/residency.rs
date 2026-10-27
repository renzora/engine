//! Which parts of a mesh's DAG can be resident, and what is drawn from them.
//!
//! Streaming keeps a mesh's clusters on the GPU group by group: a group's
//! clusters are loaded and evicted together. Each cluster names the group it
//! belongs to and the group whose simplification produced it (`refined`), and
//! drawing follows one rule (the GPU's cull pass and [`GroupGraph::cut`] apply
//! the same one):
//!
//! ```text
//! drawn(c) = resident(c.group) && parent(c) > t && (lod(c) <= t || !resident(c.refined))
//! ```
//!
//! With everything resident that is the ordinary cut ([`crate::Cluster::selected`]).
//! When the finer clusters that would replace `c` are missing, `c` stays as
//! their coarser stand-in. Between them, the stand-ins and the clusters they
//! stand in for cover each region exactly once, provided residency is closed
//! upward: a group is only resident while every group holding the clusters its
//! simplification produced (its *parents*) is too. Otherwise a region whose
//! fine group is resident but too detailed for the camera would have no coarse
//! clusters to draw. [`GroupGraph`] records those relations, and the streaming
//! manager loads parents before children and evicts children before parents.

use std::ops::Range;

use crate::RvgMesh;

/// The group relations of one mesh, derived from its clusters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GroupGraph {
    /// Per group: its clusters, a range of [`RvgMesh::clusters`].
    pub clusters: Vec<Range<u32>>,
    /// Per group: the groups holding the clusters its simplification
    /// produced. Each must be resident before this one is.
    pub parents: Vec<Vec<u32>>,
    /// Per group: the groups whose simplification produced its clusters. Each
    /// must be evicted before this one is.
    pub children: Vec<Vec<u32>>,
}

impl GroupGraph {
    /// Groups nothing coarser was built from. They are always resident:
    /// they are the stand-in of last resort for everything below them.
    pub fn roots(&self) -> impl Iterator<Item = u32> + '_ {
        (0..self.parents.len() as u32).filter(|&g| self.parents[g as usize].is_empty())
    }

    /// Whether `resident` keeps every resident group's parents resident.
    pub fn is_closed(&self, resident: &[bool]) -> bool {
        resident
            .iter()
            .enumerate()
            .filter(|(_, &r)| r)
            .all(|(g, _)| self.parents[g].iter().all(|&p| resident[p as usize]))
    }

    /// The clusters drawn at a world-space error threshold, given which groups
    /// are resident: the CPU form of the cull pass's rule (module docs).
    pub fn cut<'a>(&'a self, mesh: &'a RvgMesh, threshold: f32, resident: &'a [bool]) -> impl Iterator<Item = usize> + 'a {
        mesh.clusters.iter().enumerate().filter_map(move |(i, c)| {
            let finer_missing = c.refined >= 0 && !resident[c.refined as usize];
            let drawn = resident[c.group as usize]
                && c.parent.error > threshold
                && (c.lod.error <= threshold || finer_missing);
            drawn.then_some(i)
        })
    }
}

impl RvgMesh {
    /// The relations between this mesh's groups (see the module docs).
    pub fn group_graph(&self) -> GroupGraph {
        let groups = self.groups.len();
        let mut graph = GroupGraph {
            clusters: vec![0..0; groups],
            parents: vec![Vec::new(); groups],
            children: vec![Vec::new(); groups],
        };
        for (i, c) in self.clusters.iter().enumerate() {
            let i = i as u32;
            let range = &mut graph.clusters[c.group as usize];
            if range.start == range.end {
                *range = i..i + 1;
            } else {
                range.end = i + 1;
            }
            if c.refined >= 0 {
                let (group, refined) = (c.group, c.refined as u32);
                if !graph.parents[refined as usize].contains(&group) {
                    graph.parents[refined as usize].push(group);
                }
                if !graph.children[group as usize].contains(&refined) {
                    graph.children[group as usize].push(refined);
                }
            }
        }
        graph
    }
}
