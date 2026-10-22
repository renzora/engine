//! Keyed effect parameters: values that change over the effect's own time.
//!
//! A [`ParamTrack`] keys one thing about one emitter: an inline pin value of
//! its graph (a spawn rate, a colour, a curl noise strength), whether the
//! emitter is enabled, or bursts. They live on the effect
//! ([`VfxAsset::tracks`]) rather than in an animation clip because they are
//! part of what the effect *is*: a `.vfx` dropped into another scene, or
//! spawned by a script, plays its keys with no animator anywhere.
//!
//! ## How a keyed pin reaches the simulation
//!
//! The compiler is told which pins are keyed ([`keyed_pins`]) and lowers each
//! one to an [`Op::Param`](crate::ir::Op::Param) reading a slot instead of a
//! constant. Every step, before any program runs, the emitter fills its slots
//! from the tracks ([`EmitterTracks::fill`]) at the step's cycle time. That is
//! the whole mechanism, and it was chosen over rebuilding constants per frame
//! because the compiled program stays one program: nothing is recompiled while
//! the effect plays, and a GPU backend reads the same slots as a small uniform
//! array.
//!
//! The time a slot is filled at is what each context needs without any extra
//! bookkeeping. Particles are born at the start of a step, so the init context
//! reads the values at the particle's birth (and keeps them, since init runs
//! once); the spawn and update contexts read the current time. Nothing depends
//! on frame timing, so `simulate_to(t)` replays tracks exactly like plain play.
//!
//! A track with no keys overrides nothing: the pin keeps its inline value.

use bevy::math::Vec4;
use serde::{Deserialize, Serialize};

use crate::asset::{NodeId, PinValue, VfxAsset, VfxNode};
use crate::ir::ValueType;
use crate::nodes::{node_def, PinDefault, PinKind, PropKind, OUTPUT_NODE, PARAMETER_NODE};

/// One keyed parameter of one emitter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParamTrack {
    /// Index into [`VfxAsset::emitters`]. A track whose emitter is gone is
    /// ignored.
    pub emitter: usize,
    pub target: ParamTarget,
    /// Sorted by time. Kept sorted by every editor; the sampler does not rely
    /// on it beyond picking neighbours, so an unsorted hand-written file still
    /// plays, just not the way it reads.
    #[serde(default)]
    pub keys: Vec<ParamKey>,
}

/// What a track drives.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParamTarget {
    /// An input pin's inline value on a graph node. A pin that is wired is not
    /// overridden (the wire wins, as it does for the inline value), and a node
    /// that no longer exists is skipped.
    Pin { node: NodeId, pin: String },
    /// Whether the emitter spawns. Keys hold `Bool` and always step: an
    /// emitter is on or off, never half on. Particles already alive keep
    /// living when it switches off.
    Enabled,
    /// Each key spawns `Float(count)` particles at its time, every cycle, on
    /// top of whatever the graph's burst pins schedule.
    Burst,
}

/// One key. `interp` is how the value travels from this key to the next.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParamKey {
    pub time: f32,
    pub value: PinValue,
    #[serde(default, skip_serializing_if = "is_linear")]
    pub interp: KeyInterp,
}

fn is_linear(i: &KeyInterp) -> bool {
    *i == KeyInterp::Linear
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyInterp {
    #[default]
    Linear,
    /// Smoothstep: eases out of this key and into the next.
    Smooth,
    /// Holds this key's value until the next key.
    Step,
}

impl ParamTrack {
    pub fn pin(emitter: usize, node: NodeId, pin: &str) -> Self {
        Self {
            emitter,
            target: ParamTarget::Pin { node, pin: pin.to_string() },
            keys: Vec::new(),
        }
    }

    /// Insert a key at `time`, or replace the value of one already there, and
    /// keep the keys sorted. Returns the key's index.
    pub fn upsert(&mut self, time: f32, value: PinValue) -> usize {
        const EPS: f32 = 1e-4;
        if let Some(i) = self.keys.iter().position(|k| (k.time - time).abs() < EPS) {
            self.keys[i].value = value;
            return i;
        }
        let i = self.keys.partition_point(|k| k.time < time);
        self.keys.insert(i, ParamKey { time, value, interp: KeyInterp::Linear });
        i
    }

    /// The track's value at `t` as four lanes; see [`sample_keys`].
    pub fn sample(&self, t: f32) -> Option<[f32; 4]> {
        sample_keys(&self.keys, t)
    }
}

/// A value's four lanes, laid out the way the compiler lays out an inline
/// value of the same shape (a `Float` fills every lane). A `Vec3` gets an alpha
/// of 1 so a Vec3 key on a colour pin reads as an opaque colour, as an inline
/// Vec3 does; the interpreter masks the lane away on a Vec3 pin.
pub fn pin_lanes(value: &PinValue) -> Option<[f32; 4]> {
    Some(match value {
        PinValue::Float(v) => [*v; 4],
        PinValue::Vec2([x, y]) => [*x, *y, 0.0, 0.0],
        PinValue::Vec3([x, y, z]) => [*x, *y, *z, 1.0],
        PinValue::Color(c) => *c,
        PinValue::Bool(b) => [if *b { 1.0 } else { 0.0 }; 4],
        PinValue::Curve(_) | PinValue::Gradient(_) | PinValue::Text(_) => return None,
    })
}

/// The keys' value at `t`: the first key's before it, the last key's after it,
/// and between two keys the earlier key's interpolation. `None` with no keys
/// (or only keys that are not numbers).
pub fn sample_keys(keys: &[ParamKey], t: f32) -> Option<[f32; 4]> {
    let first = keys.first()?;
    let t = if t.is_finite() { t } else { 0.0 };
    if t <= first.time || keys.len() == 1 {
        return pin_lanes(&first.value);
    }
    let i = keys.partition_point(|k| k.time <= t);
    if i >= keys.len() {
        return pin_lanes(&keys[keys.len() - 1].value);
    }
    let (a, b) = (&keys[i - 1], &keys[i]);
    let va = pin_lanes(&a.value)?;
    let vb = pin_lanes(&b.value).unwrap_or(va);
    let span = b.time - a.time;
    let u = if span > 1e-6 { ((t - a.time) / span).clamp(0.0, 1.0) } else { 1.0 };
    let f = match a.interp {
        KeyInterp::Linear => u,
        KeyInterp::Smooth => u * u * (3.0 - 2.0 * u),
        KeyInterp::Step => 0.0,
    };
    Some(std::array::from_fn(|l| va[l] + (vb[l] - va[l]) * f))
}

/// Whether an `Enabled` track's keys say the emitter is on at `t`. Steps,
/// whatever the keys' interpolation says.
pub fn enabled_at(keys: &[ParamKey], t: f32, fallback: bool) -> bool {
    let Some(first) = keys.first() else { return fallback };
    let i = keys.partition_point(|k| k.time <= t);
    let key = if i == 0 { first } else { &keys[i - 1] };
    match &key.value {
        PinValue::Bool(b) => *b,
        other => pin_lanes(other).is_some_and(|l| l[0] > 0.5),
    }
}

/// The pins of `emitter` the compiler must treat as parameters, in slot
/// order: every `Pin` track of that emitter with at least one key, once each,
/// then the `value` pin of every Parameter node not already among them.
///
/// A Parameter node is a slot for the same reason a keyed pin is: a script
/// changes its value while the effect plays, and a slot is how a value
/// changes without recompiling (or, on the GPU, without a new pipeline). It
/// rides the keyed pins' machinery whole, so a Parameter keyed in the
/// timeline still plays its keys until a script takes it over.
pub fn keyed_pins(asset: &VfxAsset, emitter: usize) -> Vec<(NodeId, String)> {
    let mut out: Vec<(NodeId, String)> = Vec::new();
    for t in &asset.tracks {
        if t.emitter != emitter || t.keys.is_empty() {
            continue;
        }
        if let ParamTarget::Pin { node, pin } = &t.target {
            if !out.iter().any(|(n, p)| n == node && p == pin) {
                out.push((*node, pin.clone()));
            }
        }
    }
    if let Some(e) = asset.emitters.get(emitter) {
        for n in e.graph.nodes.iter().filter(|n| n.node_type == PARAMETER_NODE) {
            if !out.iter().any(|(id, p)| *id == n.id && p == PARAMETER_PIN) {
                out.push((n.id, PARAMETER_PIN.to_string()));
            }
        }
    }
    out
}

/// The Parameter node's pin a script's value replaces.
pub const PARAMETER_PIN: &str = "value";

/// The name a script sets `node` by, if it is a Parameter node with one.
/// Trimmed, so a stray space typed in the editor does not make a name no
/// script can match; empty is no name. A node saved without the property
/// has the catalog's default.
pub fn parameter_name(node: &VfxNode) -> Option<&str> {
    if node.node_type != PARAMETER_NODE {
        return None;
    }
    let name = match node.input_values.get(PARAMETER_NAME_PROP) {
        Some(PinValue::Text(s)) => s.as_str(),
        _ => match node_def(PARAMETER_NODE).and_then(|d| d.prop(PARAMETER_NAME_PROP)).map(|p| p.kind) {
            Some(PropKind::Text(default)) => default,
            _ => "",
        },
    };
    let name = name.trim();
    (!name.is_empty()).then_some(name)
}

/// The Parameter node's name property.
pub const PARAMETER_NAME_PROP: &str = "name";

/// Every parameter name the effect exposes, sorted, once each: what a script
/// can pass to `vfx_set_param`.
pub fn parameter_names(asset: &VfxAsset) -> Vec<String> {
    let mut names: Vec<String> = asset
        .emitters
        .iter()
        .flat_map(|e| e.graph.nodes.iter())
        .filter_map(parameter_name)
        .map(str::to_string)
        .collect();
    names.sort();
    names.dedup();
    names
}

/// A script's value as a slot's four lanes, laid out as [`pin_lanes`] lays out
/// an inline value of the same shape, so a script setting a parameter and the
/// file setting it mean the same.
pub fn script_lanes(value: &PinValue) -> Option<Vec4> {
    pin_lanes(value).map(Vec4::from_array)
}

/// Whether the asset gives `emitter` an `Enabled` track with keys, which makes
/// the emitter worth simulating even when its `enabled` flag is off.
pub fn has_enabled_track(asset: &VfxAsset, emitter: usize) -> bool {
    asset
        .tracks
        .iter()
        .any(|t| t.emitter == emitter && t.target == ParamTarget::Enabled && !t.keys.is_empty())
}

/// One emitter's tracks, resolved for the simulation: the keys of each
/// parameter slot, the enabled keys and the burst keys. Owned, so a running
/// effect does not hold the asset.
#[derive(Clone, Debug, Default)]
pub struct EmitterTracks {
    /// Index-aligned with [`keyed_pins`]: slot `i` reads `slots[i]`.
    pub slots: Vec<Vec<ParamKey>>,
    pub enabled: Option<Vec<ParamKey>>,
    pub bursts: Vec<Vec<ParamKey>>,
    /// The emitter's own `enabled` flag, for when no track says otherwise.
    pub enabled_default: bool,
    /// Index-aligned with `slots`: the parameter name a script sets the slot
    /// by, for a Parameter node's slot.
    pub names: Vec<Option<String>>,
    /// Index-aligned with `slots`: what a slot reads with no key and no
    /// script value, the pin's inline value. Only a Parameter's slot can be
    /// in that state (a keyed pin's track has keys), and it must read its
    /// inline value rather than zero.
    pub defaults: Vec<Vec4>,
    /// Index-aligned with `slots`: a script's value, which wins over the keys
    /// until it is cleared. Runtime state, set from `ParticleParams` every
    /// frame, so it survives the effect being rebuilt on a hot reload.
    pub overrides: Vec<Option<Vec4>>,
}

impl EmitterTracks {
    pub fn from_asset(asset: &VfxAsset, emitter: usize) -> Self {
        let enabled_default = asset.emitters.get(emitter).is_some_and(|e| e.enabled);
        let pins = keyed_pins(asset, emitter);
        let mine = || asset.tracks.iter().filter(move |t| t.emitter == emitter && !t.keys.is_empty());
        let slots = pins
            .iter()
            .map(|(node, pin)| {
                mine()
                    .find(|t| matches!(&t.target, ParamTarget::Pin { node: n, pin: p } if n == node && p == pin))
                    .map(|t| t.keys.clone())
                    .unwrap_or_default()
            })
            .collect();
        let graph_node = |id: NodeId| asset.emitters.get(emitter).and_then(|e| e.graph.nodes.iter().find(|n| n.id == id));
        let names = pins
            .iter()
            .map(|(node, pin)| {
                (pin == PARAMETER_PIN).then(|| graph_node(*node).and_then(parameter_name).map(str::to_string)).flatten()
            })
            .collect();
        let defaults = pins
            .iter()
            .map(|(node, pin)| {
                pin_value(asset, emitter, *node, pin).as_ref().and_then(pin_lanes).map(Vec4::from_array).unwrap_or(Vec4::ZERO)
            })
            .collect();
        let overrides = vec![None; pins.len()];
        let enabled = mine().find(|t| t.target == ParamTarget::Enabled).map(|t| t.keys.clone());
        let bursts = mine().filter(|t| t.target == ParamTarget::Burst).map(|t| t.keys.clone()).collect();
        Self { slots, enabled, bursts, enabled_default, names, defaults, overrides }
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty() && self.enabled.is_none() && self.bursts.is_empty()
    }

    /// Write every slot's value at cycle time `t` into `out`: a script's
    /// value, else the keys', else the inline value.
    pub fn fill(&self, t: f32, out: &mut Vec<Vec4>) {
        out.clear();
        out.extend(self.slots.iter().enumerate().map(|(i, keys)| {
            if let Some(Some(v)) = self.overrides.get(i) {
                return *v;
            }
            sample_keys(keys, t)
                .map(Vec4::from_array)
                .unwrap_or_else(|| self.defaults.get(i).copied().unwrap_or(Vec4::ZERO))
        }));
    }

    /// Set (or with `None` clear) the script value of every slot named
    /// `name`. Whether the emitter has one.
    pub fn set_parameter(&mut self, name: &str, value: Option<Vec4>) -> bool {
        let mut found = false;
        for (slot, n) in self.names.iter().enumerate() {
            if n.as_deref() == Some(name) {
                if let Some(o) = self.overrides.get_mut(slot) {
                    *o = value.filter(|v| v.is_finite());
                }
                found = true;
            }
        }
        found
    }

    /// Make the script values exactly `values`, by parameter name: a slot
    /// whose name is missing from it goes back to its keys or inline value.
    pub fn apply_parameters(&mut self, values: &std::collections::BTreeMap<String, Vec4>) {
        for (o, name) in self.overrides.iter_mut().zip(&self.names) {
            *o = name.as_ref().and_then(|n| values.get(n)).copied().filter(|v| v.is_finite());
        }
    }

    pub fn enabled_at(&self, t: f32) -> bool {
        match &self.enabled {
            Some(keys) => enabled_at(keys, t, self.enabled_default),
            None => self.enabled_default,
        }
    }

    /// Particles the burst keys spawn in the cycle-time spans `segments`
    /// (half open, `[a, b)`, so a key on a step boundary fires exactly once).
    pub fn burst_count(&self, segments: &[(f32, f32)]) -> u64 {
        let mut n: u64 = 0;
        for keys in &self.bursts {
            for k in keys {
                if segments.iter().any(|&(a, b)| a <= k.time && k.time < b) {
                    let count = pin_lanes(&k.value).map(|l| l[0]).unwrap_or(0.0);
                    if count.is_finite() && count > 0.0 {
                        n = n.saturating_add(count.round() as u64);
                    }
                }
            }
        }
        n
    }
}

// ── What can be keyed ───────────────────────────────────────────────────────

/// A pin a track could key: an unwired number input with a value editor.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyablePin {
    pub node: NodeId,
    pub pin: &'static str,
    /// "Spawn Rate", or "Curl Noise · Strength" off the output node.
    pub label: String,
    /// The value it has now: the inline value, else the catalog default.
    pub value: PinValue,
}

/// The value type an unwired pin edits as: its inline value's shape on a
/// generic pin, else the pin's own type. The same rule the effect editor's
/// value fields follow, so a track keys what the field shows.
fn pin_value_type(current: Option<&PinValue>, kind: PinKind) -> Option<ValueType> {
    match kind {
        PinKind::Fixed(ty) => Some(ty),
        PinKind::Generic => Some(match current {
            Some(PinValue::Vec2(_)) => ValueType::Vec2,
            Some(PinValue::Vec3(_)) => ValueType::Vec3,
            Some(PinValue::Color(_)) => ValueType::Color,
            Some(PinValue::Float(_)) | None => ValueType::Float,
            Some(_) => return None,
        }),
    }
}

fn value_of(ty: ValueType, lanes: [f32; 4]) -> PinValue {
    match ty {
        ValueType::Float => PinValue::Float(lanes[0]),
        ValueType::Vec2 => PinValue::Vec2([lanes[0], lanes[1]]),
        ValueType::Vec3 => PinValue::Vec3([lanes[0], lanes[1], lanes[2]]),
        ValueType::Color => PinValue::Color(lanes),
    }
}

/// The current value of a pin, as a key would store it. `None` for a pin that
/// cannot be keyed (wired, an attribute pin, a property, a missing node).
pub fn pin_value(asset: &VfxAsset, emitter: usize, node: NodeId, pin: &str) -> Option<PinValue> {
    let graph = &asset.emitters.get(emitter)?.graph;
    let n = graph.nodes.iter().find(|n| n.id == node)?;
    let def = node_def(&n.node_type)?;
    let p = def.input(pin)?;
    if graph.connections.iter().any(|c| c.to_node == node && c.to_pin == pin) {
        return None;
    }
    let PinDefault::Value(default) = p.default else { return None };
    let current = n.input_values.get(pin);
    let ty = pin_value_type(current, p.kind)?;
    let lanes = current.and_then(pin_lanes).unwrap_or(default);
    // A Vec3 stored on a colour pin is an opaque colour.
    let lanes = match (ty, current) {
        (ValueType::Color, Some(PinValue::Vec3(_))) => [lanes[0], lanes[1], lanes[2], 1.0],
        _ => lanes,
    };
    Some(value_of(ty, lanes))
}

/// Every pin of `emitter` a track could key, output node first, in graph
/// order after it.
pub fn keyable_pins(asset: &VfxAsset, emitter: usize) -> Vec<KeyablePin> {
    let Some(e) = asset.emitters.get(emitter) else { return Vec::new() };
    let mut nodes: Vec<_> = e.graph.nodes.iter().collect();
    nodes.sort_by_key(|n| n.node_type != OUTPUT_NODE);
    let mut out = Vec::new();
    for n in nodes {
        let Some(def) = node_def(&n.node_type) else { continue };
        for p in def.inputs {
            let Some(value) = pin_value(asset, emitter, n.id, p.name) else { continue };
            let label = if def.node_type == OUTPUT_NODE {
                p.label.to_string()
            } else {
                format!("{} · {}", def.display_name, p.label)
            };
            out.push(KeyablePin { node: n.id, pin: p.name, label, value });
        }
    }
    out
}

/// A new track on a pin, with one key at `time` holding the pin's current
/// value, so creating the track changes nothing on screen.
pub fn new_pin_track(asset: &VfxAsset, emitter: usize, node: NodeId, pin: &str, time: f32) -> Option<ParamTrack> {
    let value = pin_value(asset, emitter, node, pin)?;
    let mut track = ParamTrack::pin(emitter, node, pin);
    track.upsert(time.max(0.0), value);
    Some(track)
}

/// The index of the track keying `target` on `emitter`, if there is one.
pub fn find_track(asset: &VfxAsset, emitter: usize, target: &ParamTarget) -> Option<usize> {
    asset.tracks.iter().position(|t| t.emitter == emitter && &t.target == target)
}

/// The name a track goes by in the timeline: "Flames · Spawn Rate".
pub fn track_label(asset: &VfxAsset, track: &ParamTrack) -> String {
    let emitter = asset.emitters.get(track.emitter).map(|e| e.name.as_str()).unwrap_or("?");
    let what = match &track.target {
        ParamTarget::Enabled => "Enabled".to_string(),
        ParamTarget::Burst => "Burst".to_string(),
        ParamTarget::Pin { node, pin } => {
            let n = asset.emitters.get(track.emitter).and_then(|e| e.graph.nodes.iter().find(|n| n.id == *node));
            match n.and_then(|n| node_def(&n.node_type)) {
                Some(def) => {
                    let label = def.input(pin).map(|p| p.label).unwrap_or(pin.as_str());
                    if def.node_type == OUTPUT_NODE {
                        label.to_string()
                    } else {
                        format!("{} · {}", def.display_name, label)
                    }
                }
                None => format!("{pin} (missing node)"),
            }
        }
    };
    format!("{emitter} · {what}")
}

/// Keep tracks pointing at the right emitter after one was removed at
/// `removed`: its own tracks go, the ones after it move down one.
pub fn on_emitter_removed(asset: &mut VfxAsset, removed: usize) {
    asset.tracks.retain(|t| t.emitter != removed);
    for t in &mut asset.tracks {
        if t.emitter > removed {
            t.emitter -= 1;
        }
    }
    // Sub-emitters name their target by index too: one that spawned the
    // removed emitter has nothing left to spawn.
    for e in &mut asset.emitters {
        e.sub_emitters.retain(|s| s.target != removed);
        for s in &mut e.sub_emitters {
            if s.target > removed {
                s.target -= 1;
            }
        }
    }
}

/// Keep tracks and sub-emitter targets pointing at the right emitter after
/// one was inserted at `inserted` (a duplicate goes in after its source).
pub fn on_emitter_inserted(asset: &mut VfxAsset, inserted: usize) {
    for t in &mut asset.tracks {
        if t.emitter >= inserted {
            t.emitter += 1;
        }
    }
    // The inserted emitter's own rules too: a copy's targets were numbered
    // before it existed.
    for e in &mut asset.emitters {
        for s in &mut e.sub_emitters {
            if s.target >= inserted {
                s.target += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::{parse_vfx, write_vfx, Emitter, VfxGraph, VfxNode};
    use crate::compile::compile_graph_with_params;
    use crate::ir::Op;
    use crate::sim::{EffectSim, FrameInput};

    fn key(time: f32, v: f32, interp: KeyInterp) -> ParamKey {
        ParamKey { time, value: PinValue::Float(v), interp }
    }

    /// An effect whose output node is id 1, with `values` inline on it.
    fn effect(values: &[(&str, PinValue)]) -> VfxAsset {
        let mut out = VfxNode::new(1, OUTPUT_NODE);
        for (k, v) in values {
            out.input_values.insert(k.to_string(), v.clone());
        }
        VfxAsset {
            duration: 4.0,
            looping: false,
            emitters: vec![Emitter {
                name: "Flames".into(),
                capacity: 10_000,
                graph: VfxGraph { nodes: vec![out], ..Default::default() },
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn run(asset: &VfxAsset, t: f32) -> EffectSim {
        let (mut sim, errors) = EffectSim::new(asset);
        assert!(errors.is_empty(), "compile errors");
        sim.simulate_to(t, &FrameInput::default());
        sim
    }

    #[test]
    fn keys_interpolate_linear_smooth_and_step() {
        let lin = [key(0.0, 0.0, KeyInterp::Linear), key(2.0, 10.0, KeyInterp::Linear)];
        assert_eq!(sample_keys(&lin, 1.0).unwrap()[0], 5.0);
        // Held flat outside the keys.
        assert_eq!(sample_keys(&lin, -1.0).unwrap()[0], 0.0);
        assert_eq!(sample_keys(&lin, 9.0).unwrap()[0], 10.0);

        let smooth = [key(0.0, 0.0, KeyInterp::Smooth), key(2.0, 10.0, KeyInterp::Linear)];
        let quarter = sample_keys(&smooth, 0.5).unwrap()[0];
        assert!(quarter < 2.5 && quarter > 0.0, "smooth starts slow: {quarter}");
        assert!((sample_keys(&smooth, 1.0).unwrap()[0] - 5.0).abs() < 1e-5);

        let step = [key(0.0, 3.0, KeyInterp::Step), key(2.0, 10.0, KeyInterp::Linear)];
        assert_eq!(sample_keys(&step, 1.99).unwrap()[0], 3.0);
        assert_eq!(sample_keys(&step, 2.0).unwrap()[0], 10.0);
        assert!(sample_keys(&[], 1.0).is_none());
    }

    #[test]
    fn a_keyed_pin_compiles_to_a_parameter_read() {
        let asset = effect(&[("spawn_rate", PinValue::Float(5.0))]);
        let compiled = compile_graph_with_params(&asset.emitters[0].graph, &[(1, "spawn_rate".into())]).unwrap();
        let reg = compiled.spawn.output(crate::ir::SinkPin::SpawnRate).unwrap();
        assert_eq!(compiled.spawn.instrs[reg.0 as usize].op, Op::Param(0));
        // Unkeyed, the same pin is the constant it always was.
        let plain = compile_graph_with_params(&asset.emitters[0].graph, &[]).unwrap();
        let reg = plain.spawn.output(crate::ir::SinkPin::SpawnRate).unwrap();
        assert!(matches!(plain.spawn.instrs[reg.0 as usize].op, Op::Const(_)));
    }

    #[test]
    fn a_track_overrides_the_inline_value() {
        // Inline says no particles at all; the track says 100 per second.
        let mut asset = effect(&[("spawn_rate", PinValue::Float(0.0)), ("lifetime", PinValue::Float(10.0))]);
        assert_eq!(run(&asset, 1.0).alive(), 0);
        let mut track = ParamTrack::pin(0, 1, "spawn_rate");
        track.upsert(0.0, PinValue::Float(100.0));
        asset.tracks.push(track);
        let alive = run(&asset, 1.0).alive();
        assert!((95..=101).contains(&alive), "{alive}");
    }

    #[test]
    fn init_parameters_are_read_at_birth() {
        // Velocity keyed from 0 to 10 over a second: the first particle was
        // born at rest and stays so; the newest was born moving.
        let mut asset = effect(&[("spawn_rate", PinValue::Float(30.0)), ("lifetime", PinValue::Float(10.0))]);
        let mut track = ParamTrack::pin(0, 1, "velocity");
        track.upsert(0.0, PinValue::Vec3([0.0, 0.0, 0.0]));
        track.upsert(1.0, PinValue::Vec3([0.0, 10.0, 0.0]));
        asset.tracks.push(track);
        let sim = run(&asset, 1.0);
        let p = &sim.emitters[0].particles;
        let first = p.id.iter().position(|&id| id == 0).unwrap();
        let newest = (0..p.len()).max_by_key(|&i| p.id[i]).unwrap();
        assert!(p.velocity[first].y.abs() < 0.5, "{:?}", p.velocity[first]);
        assert!(p.velocity[newest].y > 8.0, "{:?}", p.velocity[newest]);
    }

    #[test]
    fn tracks_replay_deterministically() {
        let mut asset = effect(&[("lifetime", PinValue::Float(1.0))]);
        let mut rate = ParamTrack::pin(0, 1, "spawn_rate");
        rate.upsert(0.0, PinValue::Float(5.0));
        rate.upsert(2.0, PinValue::Float(80.0));
        rate.keys[0].interp = KeyInterp::Smooth;
        asset.tracks.push(rate);
        let mut burst = ParamTrack { emitter: 0, target: ParamTarget::Burst, keys: Vec::new() };
        burst.upsert(0.75, PinValue::Float(12.0));
        asset.tracks.push(burst);

        let a = run(&asset, 2.5);
        let b = run(&asset, 2.5);
        assert_eq!(a.emitters[0].particles, b.emitters[0].particles);
        // Stepping by the seek step lands on the same particles as a seek.
        let (mut stepped, _) = EffectSim::new(&asset);
        for _ in 0..150 {
            stepped.step(crate::sim::SEEK_STEP, &FrameInput::default());
        }
        assert_eq!(stepped.emitters[0].particles, a.emitters[0].particles);
    }

    #[test]
    fn burst_keys_spawn_at_their_time() {
        let mut asset = effect(&[("spawn_rate", PinValue::Float(0.0)), ("lifetime", PinValue::Float(10.0))]);
        let mut burst = ParamTrack { emitter: 0, target: ParamTarget::Burst, keys: Vec::new() };
        burst.upsert(0.5, PinValue::Float(7.0));
        burst.upsert(1.5, PinValue::Float(3.0));
        asset.tracks.push(burst);
        assert_eq!(run(&asset, 0.4).alive(), 0);
        assert_eq!(run(&asset, 0.6).alive(), 7);
        assert_eq!(run(&asset, 2.0).alive(), 10);
    }

    #[test]
    fn an_enabled_track_switches_spawning() {
        let mut asset = effect(&[("spawn_rate", PinValue::Float(50.0)), ("lifetime", PinValue::Float(10.0))]);
        let mut enabled = ParamTrack { emitter: 0, target: ParamTarget::Enabled, keys: Vec::new() };
        enabled.upsert(0.0, PinValue::Bool(false));
        enabled.upsert(1.0, PinValue::Bool(true));
        asset.tracks.push(enabled);
        assert_eq!(run(&asset, 0.9).alive(), 0);
        assert!(run(&asset, 1.5).alive() > 20);
        // Keyed on, an emitter switched off in its settings still runs.
        asset.emitters[0].enabled = false;
        assert!(run(&asset, 1.5).alive() > 20);
    }

    #[test]
    fn tracks_round_trip_through_ron() {
        let mut asset = effect(&[("spawn_rate", PinValue::Float(5.0))]);
        let mut rate = ParamTrack::pin(0, 1, "spawn_rate");
        rate.upsert(0.0, PinValue::Float(5.0));
        rate.upsert(1.0, PinValue::Float(40.0));
        rate.keys[0].interp = KeyInterp::Step;
        asset.tracks.push(rate);
        asset.tracks.push(ParamTrack {
            emitter: 0,
            target: ParamTarget::Enabled,
            keys: vec![ParamKey { time: 0.5, value: PinValue::Bool(false), interp: KeyInterp::Linear }],
        });
        let text = write_vfx(&asset).unwrap();
        assert_eq!(parse_vfx(&text).unwrap(), asset);
        // An effect with no tracks writes no `tracks` field, so files saved
        // before keyed parameters re-save unchanged; and one without the field
        // loads with none.
        let plain = effect(&[]);
        let text = write_vfx(&plain).unwrap();
        assert!(!text.contains("tracks"), "{text}");
        assert!(parse_vfx(&text).unwrap().tracks.is_empty());
    }

    #[test]
    fn keyable_pins_skip_wired_and_attribute_pins() {
        let mut asset = effect(&[]);
        asset.emitters[0].graph.nodes.push(VfxNode::new(2, "force/curl_noise"));
        let pins = keyable_pins(&asset, 0);
        assert_eq!(pins[0].node, 1, "the output node comes first");
        assert!(pins.iter().any(|p| p.node == 1 && p.pin == "spawn_rate" && p.value == PinValue::Float(10.0)));
        assert!(pins.iter().any(|p| p.node == 2 && p.pin == "strength"));
        // The curl noise position means "this particle" while unwired.
        assert!(!pins.iter().any(|p| p.node == 2 && p.pin == "position"));
        let track = new_pin_track(&asset, 0, 1, "lifetime", 0.0).unwrap();
        assert_eq!(track_label(&asset, &track), "Flames · Lifetime");
    }
}
