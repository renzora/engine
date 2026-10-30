//! The CPU evaluator: a [`TerrainGraph`] at a requested grid to heightfields.
//!
//! # Determinism
//!
//! Same graph, same request, same result, bit for bit, whatever the thread
//! count. Every node seeds its randomness from the request's seed, its own id
//! and its own Seed parameter (see [`node_seed`]); parallel work is split by
//! rows and every row is computed independently ([`crate::field::par_fill`]);
//! the simulations that cannot be split (erosion's raindrops) run on one
//! thread.
//!
//! # Caching
//!
//! A node's result is keyed by a hash of its type, its resolved parameters,
//! the keys of what feeds it, the grid and its seed ([`EvalCache`]). Editing a
//! parameter therefore re-runs that node and everything downstream of it and
//! nothing else, and toggling a value back and forth hits the cache both ways.
//! The key never includes the node's id, so two identical nodes share an entry.
//!
//! # Cancelling
//!
//! The editor re-evaluates in the background on every edit and cancels the
//! run it replaces. [`EvalControl::cancel`] is checked between nodes and inside
//! the long simulations, and a cancelled run returns [`Cancelled`] rather than
//! a half-computed result.

mod data;
mod primitives;
mod shape;
mod simulate;

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use bevy::math::Vec2;

use crate::catalog::{node_def, NodeDef, ParamKind};
use crate::field::{Field, Grid};
use crate::format::{GraphNode, ParamValue, TerrainGraph};

pub use primitives::{noise_mode_key, parse_noise_mode};

/// What to evaluate the graph at.
#[derive(Clone, Debug)]
pub struct EvalRequest {
    pub grid: Grid,
    /// The terrain's seed. Each node mixes in its id and its own Seed.
    pub seed: u32,
    /// Exposed parameter values by name, over the file's own.
    pub overrides: Vec<(String, f32)>,
    /// What the Current Terrain node reads: the terrain's generator layer in
    /// metres over the same extent, at any resolution.
    pub current: Option<Arc<Field>>,
    /// Where relative file paths (Import Heightmap) resolve from: the project.
    pub root: Option<PathBuf>,
    pub scope: EvalScope,
}

impl EvalRequest {
    pub fn new(grid: Grid, seed: u32) -> Self {
        Self { grid, seed, overrides: Vec::new(), current: None, root: None, scope: EvalScope::All }
    }
}

/// Which nodes to evaluate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvalScope {
    /// Every node: what the editor's previews need.
    All,
    /// Only the outputs and what feeds them: a build skips dangling nodes.
    Outputs,
}

/// Cancel flag and progress, shared between a background run and whoever
/// started it.
#[derive(Clone, Default, Debug)]
pub struct EvalControl {
    cancel: Arc<AtomicBool>,
    /// `f32` bits, 0..1.
    progress: Arc<AtomicU32>,
}

impl EvalControl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub fn progress(&self) -> f32 {
        f32::from_bits(self.progress.load(Ordering::Relaxed))
    }

    fn set_progress(&self, p: f32) {
        self.progress.store(p.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }
}

/// The run was cancelled before it finished.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cancelled;

/// One node's result.
#[derive(Clone, Debug)]
pub struct NodeResult {
    /// One field per output pin, in catalog order. An output node has one: the
    /// height or mask it hands to the build, so it previews like any other.
    pub outputs: Arc<Vec<Field>>,
    pub error: Option<String>,
    /// The cache key it was computed under.
    pub key: u64,
}

/// A whole evaluation.
#[derive(Clone, Debug)]
pub struct EvalResult {
    pub grid: Grid,
    pub nodes: HashMap<u64, NodeResult>,
    /// The Terrain Output's heights, metres. `None` when the graph has none.
    pub height: Option<Field>,
    /// Every Mask Output, by name, in graph order.
    pub masks: Vec<(String, Field)>,
    /// Problems, each with the node it belongs to when there is one.
    pub errors: Vec<(Option<u64>, String)>,
}

impl EvalResult {
    pub fn mask(&self, name: &str) -> Option<&Field> {
        self.masks.iter().find(|(n, _)| n == name).map(|(_, f)| f)
    }
}

struct CacheEntry {
    outputs: Arc<Vec<Field>>,
    error: Option<String>,
    used: u64,
}

/// Node results kept between evaluations, keyed as the module docs say.
pub struct EvalCache {
    entries: HashMap<u64, CacheEntry>,
    tick: u64,
    /// Entries kept after a run, least recently used dropped first. A preview
    /// field is small; a full build's fields are not, so a build uses its own
    /// cache with a small capacity.
    pub capacity: usize,
}

impl Default for EvalCache {
    fn default() -> Self {
        Self::with_capacity(256)
    }
}

impl EvalCache {
    pub fn with_capacity(capacity: usize) -> Self {
        Self { entries: HashMap::new(), tick: 0, capacity }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    fn trim(&mut self) {
        if self.entries.len() <= self.capacity {
            return;
        }
        let mut ages: Vec<(u64, u64)> = self.entries.iter().map(|(k, e)| (e.used, *k)).collect();
        ages.sort_unstable();
        let excess = self.entries.len() - self.capacity;
        for (_, k) in ages.into_iter().take(excess) {
            self.entries.remove(&k);
        }
    }
}

/// A node's parameters with defaults filled in, overrides applied and
/// numbers clamped to the catalog's range. What every node function reads.
pub struct Params {
    values: Vec<(&'static str, ParamValue)>,
}

impl Params {
    pub fn resolve(def: &'static NodeDef, node: &GraphNode, graph: &TerrainGraph, overrides: &[(String, f32)]) -> Self {
        let mut values = Vec::with_capacity(def.params.len());
        for p in def.params {
            let mut v = node.params.get(p.name).cloned().unwrap_or_else(|| p.default.value());
            for exposed in graph.params.iter().filter(|e| e.node == node.id && e.param == p.name) {
                if let Some((_, o)) = overrides.iter().find(|(n, _)| *n == exposed.name) {
                    v = ParamValue::Number(*o);
                }
            }
            v = match (&p.kind, v) {
                (ParamKind::Number { min, max, .. }, ParamValue::Number(x)) => ParamValue::Number(x.clamp(*min, *max)),
                (ParamKind::Int { min, max }, ParamValue::Number(x)) => ParamValue::Number(x.round().clamp(*min, *max)),
                // A value of the wrong shape (hand-edited file) falls back to
                // the default rather than reaching a node that cannot read it.
                (ParamKind::Number { .. } | ParamKind::Int { .. }, other) if other.as_number().is_none() => p.default.value(),
                (ParamKind::Bool, other) if other.as_bool().is_none() => p.default.value(),
                (_, other) => other,
            };
            values.push((p.name, v));
        }
        Self { values }
    }

    fn get(&self, name: &str) -> Option<&ParamValue> {
        self.values.iter().find(|(n, _)| *n == name).map(|(_, v)| v)
    }

    pub fn num(&self, name: &str) -> f32 {
        self.get(name).and_then(ParamValue::as_number).unwrap_or(0.0)
    }

    pub fn int(&self, name: &str) -> u32 {
        self.num(name).max(0.0).round() as u32
    }

    pub fn flag(&self, name: &str) -> bool {
        self.get(name).and_then(ParamValue::as_bool).unwrap_or(false)
    }

    pub fn text(&self, name: &str) -> &str {
        self.get(name).and_then(ParamValue::as_text).unwrap_or("")
    }

    pub fn points(&self, name: &str) -> &[[f32; 2]] {
        self.get(name).and_then(ParamValue::as_points).unwrap_or(&[])
    }

    fn hash_into(&self, h: &mut impl Hasher) {
        for (n, v) in &self.values {
            n.hash(h);
            v.hash_into(h);
        }
    }
}

/// What a node function sees.
pub struct NodeCtx<'a> {
    pub grid: Grid,
    /// This node's seed (see [`node_seed`]).
    pub seed: u32,
    pub params: &'a Params,
    pub control: &'a EvalControl,
    pub current: Option<&'a Field>,
    pub root: Option<&'a Path>,
}

impl NodeCtx<'_> {
    pub fn cancelled(&self) -> bool {
        self.control.is_cancelled()
    }

    /// Terrain-local metres of sample `(x, z)`.
    pub fn pos(&self, x: usize, z: usize) -> Vec2 {
        self.grid.pos(x, z)
    }
}

/// A node's inputs, one per catalog input pin.
pub struct Inputs {
    fields: Vec<(Arc<Vec<Field>>, usize)>,
    connected: Vec<bool>,
}

impl Inputs {
    pub fn get(&self, i: usize) -> &Field {
        let (outs, pin) = &self.fields[i];
        &outs[*pin]
    }

    pub fn connected(&self, i: usize) -> bool {
        self.connected.get(i).copied().unwrap_or(false)
    }
}

type NodeFn = fn(&NodeCtx, &Inputs) -> Result<Vec<Field>, String>;

fn node_fn(node_type: &str) -> Option<NodeFn> {
    Some(match node_type {
        "noise" => primitives::noise,
        "mountain_range" => primitives::mountain_range,
        "plateau" => primitives::plateau,
        "crater" => primitives::crater,
        "volcano" => primitives::volcano,
        "dunes" => primitives::dunes,
        "cells" => primitives::cells,
        "gradient" => primitives::gradient,
        "constant" => primitives::constant,
        "import_heightmap" => primitives::import_heightmap,
        "current_terrain" => primitives::current_terrain,
        "combine" => shape::combine,
        "curve" => shape::curve,
        "terrace" => shape::terrace,
        "warp" => shape::warp,
        "blur" => shape::blur,
        "sharpen" => shape::sharpen,
        "transform" => shape::transform,
        "clamp" => shape::clamp,
        "invert" => shape::invert,
        "height_range" => shape::height_range,
        "flatten" => shape::flatten,
        "erosion" => simulate::erosion,
        "thermal" => simulate::thermal,
        "rivers" => simulate::rivers,
        "lakes" => simulate::lakes,
        "snow" => simulate::snow,
        "slope" => data::slope,
        "height_mask" => data::height_mask,
        "curvature" => data::curvature,
        "flow" => data::flow,
        "wetness" => data::wetness,
        "aspect" => data::aspect,
        "select_range" => data::select_range,
        "terrain_output" => shape::terrain_output,
        "mask_output" => shape::mask_output,
        _ => return None,
    })
}

/// Whether the evaluator implements `node_type`. Every catalog node does; the
/// test below holds it to that.
pub fn is_implemented(node_type: &str) -> bool {
    node_fn(node_type).is_some()
}

/// A node's seed: the terrain's, the node's id and its own Seed parameter,
/// mixed. The id is in it so two Noise nodes with default settings are two
/// different noises rather than one drawn twice.
pub fn node_seed(seed: u32, node: u64, own: u32) -> u32 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (seed, node, own).hash(&mut h);
    let v = h.finish();
    (v ^ (v >> 32)) as u32
}

/// The terrain's 64-bit seed as the evaluator's 32.
pub fn fold_seed(seed: u64) -> u32 {
    (seed ^ (seed >> 32)) as u32
}

/// The order to evaluate in: every node after everything feeding it. Nodes in
/// a cycle (only a hand-edited file can hold one; `connect` refuses them) are
/// left out and reported.
fn order(graph: &TerrainGraph, scope: EvalScope, errors: &mut Vec<(Option<u64>, String)>) -> Vec<u64> {
    let roots: Vec<u64> = match scope {
        EvalScope::All => graph.nodes.iter().map(|n| n.id).collect(),
        EvalScope::Outputs => graph
            .nodes
            .iter()
            .filter(|n| node_def(&n.node_type).is_some_and(NodeDef::is_output))
            .map(|n| n.id)
            .collect(),
    };
    let mut out = Vec::new();
    let mut done = HashSet::new();
    let mut active = HashSet::new();
    fn visit(
        graph: &TerrainGraph,
        id: u64,
        out: &mut Vec<u64>,
        done: &mut HashSet<u64>,
        active: &mut HashSet<u64>,
        errors: &mut Vec<(Option<u64>, String)>,
    ) {
        if done.contains(&id) {
            return;
        }
        if !active.insert(id) {
            errors.push((Some(id), "this node is part of a loop".into()));
            return;
        }
        for c in graph.connections.iter().filter(|c| c.to_node == id) {
            if graph.node(c.from_node).is_some() {
                visit(graph, c.from_node, out, done, active, errors);
            }
        }
        active.remove(&id);
        done.insert(id);
        out.push(id);
    }
    let mut loops = Vec::new();
    for id in roots {
        visit(graph, id, &mut out, &mut done, &mut active, &mut loops);
    }
    // A loop is met once per way into it; report each node in one once.
    let mut seen = HashSet::new();
    for (node, msg) in loops {
        if node.is_some_and(|n| seen.insert(n)) {
            errors.push((node, msg));
        }
    }
    out
}

/// Evaluate `graph` for `req`. See the module docs.
pub fn evaluate(
    graph: &TerrainGraph,
    req: &EvalRequest,
    cache: &mut EvalCache,
    control: &EvalControl,
) -> Result<EvalResult, Cancelled> {
    cache.tick += 1;
    let tick = cache.tick;
    let mut errors = Vec::new();
    let ids = order(graph, req.scope, &mut errors);
    let total: f32 = ids
        .iter()
        .filter_map(|id| graph.node(*id))
        .map(|n| node_def(&n.node_type).map_or(1.0, NodeDef::cost))
        .sum::<f32>()
        .max(1.0);
    let mut done_cost = 0.0;
    control.set_progress(0.0);

    let grid = req.grid;
    let current = req.current.as_ref().map(|f| {
        if f.w == grid.w && f.h == grid.h {
            (**f).clone()
        } else {
            f.resampled(grid.w, grid.h)
        }
    });
    let mut results: HashMap<u64, NodeResult> = HashMap::new();

    for id in ids {
        if control.is_cancelled() {
            return Err(Cancelled);
        }
        let Some(node) = graph.node(id) else { continue };
        let Some(def) = node_def(&node.node_type) else {
            let msg = format!("unknown node type `{}`", node.node_type);
            errors.push((Some(id), msg.clone()));
            results.insert(id, NodeResult { outputs: Arc::new(Vec::new()), error: Some(msg), key: 0 });
            continue;
        };
        let params = Params::resolve(def, node, graph, &req.overrides);
        let own_seed = params.int("seed");
        let seed = node_seed(req.seed, id, own_seed);

        // Inputs, and the problems with them.
        let mut fields = Vec::with_capacity(def.inputs.len());
        let mut connected = Vec::with_capacity(def.inputs.len());
        let mut missing: Vec<&str> = Vec::new();
        let mut key = std::collections::hash_map::DefaultHasher::new();
        (def.node_type, grid.w, grid.h, grid.extent.x.to_bits(), grid.extent.y.to_bits(), seed).hash(&mut key);
        params.hash_into(&mut key);
        if def.node_type == "current_terrain" {
            if let Some(c) = &current {
                // The layer's own content keys this node: a rebuild that
                // changed the terrain must not reuse the old read.
                c.data.len().hash(&mut key);
                for v in c.data.iter().step_by((c.data.len() / 4096).max(1)) {
                    v.to_bits().hash(&mut key);
                }
            }
        }
        for pin in def.inputs {
            let upstream = graph.input_of(id, pin.name).and_then(|c| {
                let r = results.get(&c.from_node)?;
                let from_def = graph.node(c.from_node).and_then(|n| node_def(&n.node_type))?;
                let index = from_def.outputs.iter().position(|o| o.name == c.from_pin);
                // An output node's one field is its result (see `NodeResult`).
                let index = index.or_else(|| from_def.is_output().then_some(0))?;
                (index < r.outputs.len()).then(|| (r.outputs.clone(), index, r.key))
            });
            match upstream {
                Some((outs, index, k)) => {
                    (1u8, k, index).hash(&mut key);
                    fields.push((outs, index));
                    connected.push(true);
                }
                None => {
                    if pin.required {
                        missing.push(pin.label);
                    }
                    (0u8, pin.unconnected.to_bits()).hash(&mut key);
                    fields.push((Arc::new(vec![Field::of(&grid, pin.unconnected)]), 0));
                    connected.push(false);
                }
            }
        }
        let key = key.finish();
        let flat_outputs = || Arc::new(vec![Field::of(&grid, 0.0); def.outputs.len().max(usize::from(def.is_output()))]);

        let result = if !missing.is_empty() {
            let msg = format!("needs an input: {}", missing.join(", "));
            NodeResult { outputs: flat_outputs(), error: Some(msg), key }
        } else if let Some(hit) = cache.entries.get_mut(&key) {
            hit.used = tick;
            NodeResult { outputs: hit.outputs.clone(), error: hit.error.clone(), key }
        } else {
            let Some(run) = node_fn(def.node_type) else {
                let msg = format!("`{}` is not implemented", def.node_type);
                errors.push((Some(id), msg.clone()));
                results.insert(id, NodeResult { outputs: flat_outputs(), error: Some(msg), key });
                continue;
            };
            let ctx = NodeCtx {
                grid,
                seed,
                params: &params,
                control,
                current: current.as_ref(),
                root: req.root.as_deref(),
            };
            let inputs = Inputs { fields, connected };
            let (outputs, error) = match run(&ctx, &inputs) {
                Ok(mut outs) => {
                    for f in &mut outs {
                        f.sanitize();
                    }
                    (Arc::new(outs), None)
                }
                Err(e) => (flat_outputs(), Some(e)),
            };
            if control.is_cancelled() {
                return Err(Cancelled);
            }
            cache.entries.insert(key, CacheEntry { outputs: outputs.clone(), error: error.clone(), used: tick });
            NodeResult { outputs, error, key }
        };
        if let Some(e) = &result.error {
            errors.push((Some(id), e.clone()));
        }
        results.insert(id, result);
        done_cost += def.cost();
        control.set_progress(done_cost / total);
    }
    cache.trim();

    // Gather the outputs.
    let mut height = None;
    let mut masks: Vec<(String, Field)> = Vec::new();
    for node in &graph.nodes {
        let Some(r) = results.get(&node.id) else { continue };
        let Some(field) = r.outputs.first() else { continue };
        match node.node_type.as_str() {
            "terrain_output" => {
                if height.is_none() {
                    height = Some(field.clone());
                } else {
                    errors.push((Some(node.id), "only the first Terrain Output is built".into()));
                }
            }
            "mask_output" => {
                let def = node_def("mask_output").expect("in the catalog");
                let name = Params::resolve(def, node, graph, &req.overrides).text("name").trim().to_string();
                if masks.iter().any(|(n, _)| *n == name) {
                    errors.push((Some(node.id), format!("two Mask Outputs are named `{name}`; the first wins")));
                } else {
                    masks.push((name, field.clone()));
                }
            }
            _ => {}
        }
    }
    if height.is_none() {
        errors.push((None, "the graph has no Terrain Output".into()));
    }
    control.set_progress(1.0);
    Ok(EvalResult { grid, nodes: results, height, masks, errors })
}

#[cfg(test)]
mod tests;
