//! The scene side: which graph a terrain is generated from, the request that
//! builds it, and what a build leaves behind.
//!
//! **For the Terrain inspector** (the Manage tab's generator slot), the whole
//! surface is here:
//!
//! - [`TerrainGenerator`] on the terrain root: graph path, seed, overrides.
//!   Edit it like any component; it is saved with the scene.
//! - [`BuildTerrainGraph`]: write one to build a terrain in the background.
//!   The editor crate (`renzora_terrain_graph_editor`) runs it; a game build
//!   has no builder and the message goes unread.
//! - [`TerrainGraphBuilds`]: progress and the last outcome per terrain, for
//!   a progress bar and a "built 2 min ago" line.
//! - [`TerrainGraphOutput`] on the terrain root after a build: the named
//!   masks, for paint layers and foliage.
//! - [`exposed_params`] lists what a graph lets a terrain override, with the
//!   value each currently has.

use std::sync::Arc;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::field::{Field, Grid};
use crate::format::TerrainGraph;

/// A per-terrain value for one of the graph's exposed parameters.
#[derive(Clone, Debug, Default, PartialEq, Reflect, Serialize, Deserialize)]
pub struct GeneratorOverride {
    pub name: String,
    pub value: f32,
}

/// Generate this terrain's base heights from a terrain graph.
///
/// Building writes the terrain's **generator layer**; hand sculpting sits on
/// a layer above it and survives every rebuild (see [`crate::apply`]).
#[derive(Component, Clone, Debug, Default, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default)]
pub struct TerrainGenerator {
    /// Project-relative path of the `.tgraph`.
    pub graph: String,
    /// The terrain's seed. One graph with two seeds is two different
    /// landscapes of the same kind.
    pub seed: u64,
    /// Values for the graph's exposed parameters, by name. A name the graph
    /// does not expose is kept (the graph may expose it again) and ignored.
    #[serde(default)]
    #[reflect(default)]
    pub overrides: Vec<GeneratorOverride>,
}

impl TerrainGenerator {
    pub fn new(graph: impl Into<String>, seed: u64) -> Self {
        Self { graph: graph.into(), seed, overrides: Vec::new() }
    }

    pub fn override_value(&self, name: &str) -> Option<f32> {
        self.overrides.iter().find(|o| o.name == name).map(|o| o.value)
    }

    /// Set an override, returning whether it changed.
    pub fn set_override(&mut self, name: &str, value: f32) -> bool {
        match self.overrides.iter_mut().find(|o| o.name == name) {
            Some(o) if o.value == value => false,
            Some(o) => {
                o.value = value;
                true
            }
            None => {
                self.overrides.push(GeneratorOverride { name: name.to_string(), value });
                true
            }
        }
    }

    pub fn clear_override(&mut self, name: &str) -> bool {
        let before = self.overrides.len();
        self.overrides.retain(|o| o.name != name);
        before != self.overrides.len()
    }

    /// The overrides in the evaluator's shape.
    pub fn override_pairs(&self) -> Vec<(String, f32)> {
        self.overrides.iter().map(|o| (o.name.clone(), o.value)).collect()
    }
}

/// One exposed parameter as a terrain sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct ExposedValue {
    pub name: String,
    pub label: String,
    pub min: f32,
    pub max: f32,
    /// The file's own value.
    pub default: f32,
    /// What this terrain builds with: its override, else the default.
    pub value: f32,
    pub overridden: bool,
}

/// What `graph` lets `generator` override, in the graph's order.
pub fn exposed_params(graph: &TerrainGraph, generator: Option<&TerrainGenerator>) -> Vec<ExposedValue> {
    graph
        .params
        .iter()
        .map(|p| {
            let default = graph.exposed_value(&p.name).unwrap_or(0.0);
            let own = generator.and_then(|g| g.override_value(&p.name));
            ExposedValue {
                name: p.name.clone(),
                label: p.label.clone(),
                min: p.min,
                max: p.max,
                default,
                value: own.unwrap_or(default),
                overridden: own.is_some(),
            }
        })
        .collect()
}

/// Build `terrain` (an entity with `TerrainData` and [`TerrainGenerator`])
/// at full resolution in the background. One undo step when it lands.
#[derive(Message, Clone, Copy, Debug)]
pub struct BuildTerrainGraph {
    pub terrain: Entity,
}

/// Where a terrain's build is.
#[derive(Clone, Debug, PartialEq)]
pub enum BuildState {
    /// Evaluating; `progress` is 0..1.
    Running { progress: f32 },
    /// Landed. The summary is one line for a status label.
    Done { summary: String },
    Failed { error: String },
    Cancelled,
}

#[derive(Clone, Debug)]
pub struct BuildStatus {
    pub terrain: Entity,
    pub state: BuildState,
    /// `Time<Real>` seconds the state was entered, for "built 2 min ago".
    pub since: f64,
}

/// Every terrain's latest build, running or finished.
#[derive(Resource, Default, Debug)]
pub struct TerrainGraphBuilds {
    pub builds: Vec<BuildStatus>,
}

impl TerrainGraphBuilds {
    pub fn get(&self, terrain: Entity) -> Option<&BuildStatus> {
        self.builds.iter().find(|b| b.terrain == terrain)
    }

    /// 0..1 while `terrain` is building, `None` otherwise.
    pub fn progress(&self, terrain: Entity) -> Option<f32> {
        match self.get(terrain)?.state {
            BuildState::Running { progress } => Some(progress),
            _ => None,
        }
    }

    pub fn set(&mut self, terrain: Entity, state: BuildState, now: f64) {
        match self.builds.iter_mut().find(|b| b.terrain == terrain) {
            Some(b) => {
                if b.state != state {
                    b.state = state;
                    b.since = now;
                }
            }
            None => self.builds.push(BuildStatus { terrain, state, since: now }),
        }
    }
}

/// What the last build of a terrain produced besides heights: the graph's
/// Mask Outputs at the build's full resolution, over the whole terrain.
///
/// Not saved: the masks are a pure function of the graph, the seed and the
/// overrides, so a rebuild recreates them, and storing megabytes of masks in
/// every scene to spare that would be the wrong trade. Paint layers and
/// foliage that want them bake from here when a build lands (watch for
/// `Changed<TerrainGraphOutput>`).
#[derive(Component, Clone, Debug)]
pub struct TerrainGraphOutput {
    /// The whole-terrain vertex grid the masks are sampled on.
    pub grid: Grid,
    pub masks: Vec<(String, Arc<Field>)>,
}

impl TerrainGraphOutput {
    pub fn mask(&self, name: &str) -> Option<&Field> {
        self.masks.iter().find(|(n, _)| n == name).map(|(_, f)| &**f)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.masks.iter().map(|(n, _)| n.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overrides_set_change_and_clear() {
        let mut g = TerrainGenerator::new("terrain/a.tgraph", 7);
        assert!(g.set_override("peak_height", 300.0));
        assert!(!g.set_override("peak_height", 300.0));
        assert_eq!(g.override_value("peak_height"), Some(300.0));
        assert!(g.clear_override("peak_height"));
        assert_eq!(g.override_value("peak_height"), None);
    }

    #[test]
    fn exposed_values_fall_back_to_the_file() {
        let graph = crate::presets::highlands();
        let mut gen = TerrainGenerator::new("x", 1);
        let before = exposed_params(&graph, Some(&gen));
        let peak = before.iter().find(|p| p.name == "peak_height").expect("highlands exposes its peak");
        assert!(!peak.overridden);
        gen.set_override("peak_height", 123.0);
        let after = exposed_params(&graph, Some(&gen));
        let peak = after.iter().find(|p| p.name == "peak_height").unwrap();
        assert!(peak.overridden);
        assert_eq!(peak.value, 123.0);
    }
}
