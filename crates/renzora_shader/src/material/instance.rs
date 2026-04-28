//! Material Instance file format (`.material_instance`).
//!
//! A material instance references a master `.material` file and supplies
//! per-instance overrides for the master's named parameter nodes
//! (`param/float`, `param/color`, etc.). Instances are how a project ships
//! many *visually distinct* materials that share a single compiled master
//! shader — same idea as Unreal's Material Instances.
//!
//! ## File shape
//!
//! ```json
//! {
//!   "master": "models/Wood/materials/Wood.material",
//!   "overrides": {
//!     "BaseColor": { "Color": [0.45, 0.22, 0.10, 1.0] },
//!     "Roughness": { "Float": 0.85 }
//!   }
//! }
//! ```
//!
//! - `master` is the asset-relative path to the master `.material` file.
//! - `overrides` keys are parameter names authored on the master's `param/*`
//!   nodes. Unknown keys are ignored at resolve time so renaming a master
//!   parameter doesn't hard-fail every instance.
//!
//! ## Override application
//!
//! Two paths in the resolver:
//!
//! - **Trivial master** — clone the master graph, splice override values
//!   into each matching `param/*` node's `default`, then run the
//!   standard-build classifier. The result is a fresh
//!   `Handle<StandardMaterial>` cached per instance file path. No shader
//!   compilation involved.
//! - **Procedural master** — compile the master once (cached under the
//!   master path). For each instance, clone the master's `GraphMaterial`
//!   and overwrite only the parameter UBO's slots with overridden values.
//!   The compiled shader UUID and texture bindings carry over, so wgpu
//!   reuses the same specialized pipeline across every instance.

use bevy::math::Vec4;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::codegen::{MaterialParam, ParamKind};
use super::graph::{MaterialGraph, PinValue};
use super::material_ref::ParamValue;
use super::surface_ext::SURFACE_GRAPH_PARAM_SLOTS;

/// On-disk representation of a `.material_instance` file.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MaterialInstance {
    /// Asset-relative path to the master `.material` graph this instance
    /// derives from.
    pub master: String,
    /// Per-instance parameter overrides keyed by the master's parameter
    /// name. `#[serde(default)]` lets a hand-edited file omit the field.
    #[serde(default)]
    pub overrides: HashMap<String, ParamValue>,
}

/// Translate a stored [`ParamValue`] (the on-disk override format) into the
/// [`PinValue`] the graph nodes use internally. The two types overlap
/// fully — this is just a coordinate change.
pub fn param_value_to_pin_value(v: &ParamValue) -> PinValue {
    match v {
        ParamValue::Float(f) => PinValue::Float(*f),
        ParamValue::Vec2(a) => PinValue::Vec2(*a),
        ParamValue::Vec3(a) => PinValue::Vec3(*a),
        ParamValue::Vec4(a) => PinValue::Vec4(*a),
        ParamValue::Color(c) => PinValue::Color(*c),
        ParamValue::Int(i) => PinValue::Int(*i),
        ParamValue::Bool(b) => PinValue::Bool(*b),
    }
}

/// Pack a [`PinValue`] into the [`Vec4`] slot the codegen reads from. The
/// component layout matches what each `param/*` codegen branch emits:
///   - Float / Bool / Int → `.x` (other channels zero)
///   - Vec2 → `.xy` (z=0, w=0)
///   - Vec3 → `.xyz` (w=0)
///   - Vec4 / Color → all four channels
pub fn pin_value_to_vec4(value: &PinValue) -> Vec4 {
    match value {
        PinValue::Float(f) => Vec4::new(*f, 0.0, 0.0, 0.0),
        PinValue::Vec2([x, y]) => Vec4::new(*x, *y, 0.0, 0.0),
        PinValue::Vec3([x, y, z]) => Vec4::new(*x, *y, *z, 0.0),
        PinValue::Vec4([x, y, z, w]) | PinValue::Color([x, y, z, w]) => {
            Vec4::new(*x, *y, *z, *w)
        }
        PinValue::Bool(b) => Vec4::new(if *b { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0),
        PinValue::Int(i) => Vec4::new(*i as f32, 0.0, 0.0, 0.0),
        PinValue::TexturePath(_) | PinValue::String(_) | PinValue::None => Vec4::ZERO,
    }
}

/// Pack a [`ParamValue`] (override coming from a `.material_instance` file)
/// into the same [`Vec4`] slot layout as `pin_value_to_vec4`. Used when the
/// resolver writes instance overrides into a cloned master's uniform buffer.
pub fn param_value_to_vec4(value: &ParamValue) -> Vec4 {
    match value {
        ParamValue::Float(f) => Vec4::new(*f, 0.0, 0.0, 0.0),
        ParamValue::Vec2([x, y]) => Vec4::new(*x, *y, 0.0, 0.0),
        ParamValue::Vec3([x, y, z]) => Vec4::new(*x, *y, *z, 0.0),
        ParamValue::Vec4([x, y, z, w]) | ParamValue::Color([x, y, z, w]) => {
            Vec4::new(*x, *y, *z, *w)
        }
        ParamValue::Bool(b) => Vec4::new(if *b { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0),
        ParamValue::Int(i) => Vec4::new(*i as f32, 0.0, 0.0, 0.0),
    }
}

/// Build the master's default parameter buffer from the codegen-produced
/// parameter list. Each parameter writes its authored default into the slot
/// matching its position in the list — same indexing the WGSL uses for
/// `material_params.slots[i]`.
pub fn build_default_param_slots(parameters: &[MaterialParam]) -> [Vec4; SURFACE_GRAPH_PARAM_SLOTS] {
    let mut slots = [Vec4::ZERO; SURFACE_GRAPH_PARAM_SLOTS];
    for (i, p) in parameters.iter().enumerate() {
        if i >= SURFACE_GRAPH_PARAM_SLOTS {
            break;
        }
        slots[i] = pin_value_to_vec4(&p.default);
    }
    slots
}

/// Apply instance overrides on top of an existing parameter buffer. Looks
/// each override key up in the master's parameter list to find the slot
/// index, then writes the override into that slot. Unknown keys are ignored
/// — same tolerance as `graph_with_overrides_applied`. Returns the number of
/// overrides actually applied (purely for diagnostic use).
pub fn apply_overrides_to_param_slots(
    slots: &mut [Vec4; SURFACE_GRAPH_PARAM_SLOTS],
    parameters: &[MaterialParam],
    overrides: &HashMap<String, ParamValue>,
) -> usize {
    let mut applied = 0;
    for (name, value) in overrides {
        let Some(idx) = parameters.iter().position(|p| &p.name == name) else {
            continue;
        };
        if idx >= SURFACE_GRAPH_PARAM_SLOTS {
            continue;
        }
        let mut packed = param_value_to_vec4(value);
        // Float overrides on Color/Vec4 slots only fill .x; the alpha/etc
        // channels were already initialised from the master default. To
        // preserve the master's defaults for unset components, copy them
        // back in for narrow-typed overrides.
        if let Some(p) = parameters.get(idx) {
            match p.kind {
                ParamKind::Color | ParamKind::Vec4 => { /* full Vec4 from override */ }
                ParamKind::Vec3 => packed.w = slots[idx].w,
                ParamKind::Vec2 => {
                    packed.z = slots[idx].z;
                    packed.w = slots[idx].w;
                }
                ParamKind::Float | ParamKind::Bool => {
                    packed.y = slots[idx].y;
                    packed.z = slots[idx].z;
                    packed.w = slots[idx].w;
                }
            }
        }
        slots[idx] = packed;
        applied += 1;
    }
    applied
}

/// Produce a clone of `graph` with each `param/*` node's `default` pin
/// rewritten from `overrides` (when a matching name is present). Nodes whose
/// authored name isn't in the override map are left alone.
///
/// Re-running the standard-build classifier on the result yields a
/// `StandardMaterial` whose factors/textures reflect the overrides — no
/// changes needed inside the classifier itself.
pub fn graph_with_overrides_applied(
    graph: &MaterialGraph,
    overrides: &HashMap<String, ParamValue>,
) -> MaterialGraph {
    if overrides.is_empty() {
        return graph.clone();
    }
    let mut out = graph.clone();
    for node in out.nodes.iter_mut() {
        if !node.node_type.starts_with("param/") {
            continue;
        }
        let name = match node.input_values.get("name").cloned() {
            Some(PinValue::String(s)) => s,
            _ => continue,
        };
        let Some(ov) = overrides.get(&name) else {
            continue;
        };
        node.input_values
            .insert("default".to_string(), param_value_to_pin_value(ov));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::graph::*;

    #[test]
    fn override_replaces_param_default() {
        let mut graph = MaterialGraph::new("Test", MaterialDomain::Surface);
        let p = graph.add_node("param/color", [0.0, 0.0]);
        if let Some(node) = graph.get_node_mut(p) {
            node.input_values.insert("name".into(), PinValue::String("BaseColor".into()));
            node.input_values
                .insert("default".into(), PinValue::Color([1.0, 1.0, 1.0, 1.0]));
        }

        let mut overrides = HashMap::new();
        overrides.insert(
            "BaseColor".to_string(),
            ParamValue::Color([0.5, 0.25, 0.1, 1.0]),
        );

        let patched = graph_with_overrides_applied(&graph, &overrides);
        let patched_node = patched.get_node(p).unwrap();
        match patched_node.input_values.get("default") {
            Some(PinValue::Color(c)) => {
                assert!((c[0] - 0.5).abs() < 1e-4);
                assert!((c[1] - 0.25).abs() < 1e-4);
                assert!((c[2] - 0.1).abs() < 1e-4);
            }
            other => panic!("expected Color override, got {:?}", other),
        }
    }

    #[test]
    fn unknown_override_keys_are_ignored() {
        let mut graph = MaterialGraph::new("Test", MaterialDomain::Surface);
        let p = graph.add_node("param/float", [0.0, 0.0]);
        if let Some(node) = graph.get_node_mut(p) {
            node.input_values.insert("name".into(), PinValue::String("Metallic".into()));
            node.input_values.insert("default".into(), PinValue::Float(0.0));
        }

        let mut overrides = HashMap::new();
        overrides.insert("Roughness".to_string(), ParamValue::Float(0.9));

        let patched = graph_with_overrides_applied(&graph, &overrides);
        // Metallic param keeps its 0.0 default; the unknown "Roughness" key
        // is silently dropped rather than failing the whole resolve.
        match patched.get_node(p).unwrap().input_values.get("default") {
            Some(PinValue::Float(v)) => assert!((v - 0.0).abs() < 1e-4),
            other => panic!("expected Float, got {:?}", other),
        }
    }
}
