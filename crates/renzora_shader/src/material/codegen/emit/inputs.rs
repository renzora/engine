//! `input/*` and `param/*` node emitters.
//!
//! Input nodes read the fragment stage's own attributes. Several of them go
//! through a `mat_*` alias rather than `in.<field>` directly: a mesh without a
//! UV or colour attribute has no such field on `VertexOutput`, so the alias is
//! defined behind an `#ifdef` and referencing the field would fail to compile
//! on exactly the meshes that lack it.
//!
//! Parameter nodes read `material_params.slots[N]`, where `N` comes from
//! [`Ctx::intern_parameter`]. The master bakes its authored default into that
//! slot; an instance reuses the master's compiled shader and overwrites the
//! same slot, which is why the slot index — not the name — is what codegen
//! emits.

use super::super::super::graph::{MaterialNode, NodeId, PinValue};
use super::super::{param_name, ParamKind};
use super::super::ctx::Ctx;

impl Ctx<'_> {
    pub(crate) fn gen_input_node(&mut self, node: &MaterialNode, id: NodeId) {
        match node.node_type.as_str() {
            "input/uv" => {
                // `mat_uv` is aliased at fragment entry behind `#ifdef
                // VERTEX_UVS_A`. Meshes without a UV attribute (e.g. some
                // Bistro submeshes) don't get the field on `VertexOutput`,
                // so referencing `in.uv` directly fails to compile.
                self.set_out(id, "uv", "mat_uv".into());
                self.set_out(id, "u", "mat_uv.x".into());
                self.set_out(id, "v", "mat_uv.y".into());
            }
            "input/world_position" => {
                self.set_out(id, "position", "in.world_position.xyz".into());
                self.set_out(id, "x", "in.world_position.x".into());
                self.set_out(id, "y", "in.world_position.y".into());
                self.set_out(id, "z", "in.world_position.z".into());
            }
            "input/world_normal" => {
                self.set_out(id, "normal", "in.world_normal".into());
                self.set_out(id, "x", "in.world_normal.x".into());
                self.set_out(id, "y", "in.world_normal.y".into());
                self.set_out(id, "z", "in.world_normal.z".into());
            }
            "input/view_direction" => {
                let v = self.next_var("view_dir");
                self.emit(format!(
                    "    let {v} = normalize(view.world_position.xyz - in.world_position.xyz);"
                ));
                self.set_out(id, "direction", v);
            }
            "input/time" => {
                self.set_out(id, "time", "globals.time".into());
                let s = self.next_var("sin_t");
                let c = self.next_var("cos_t");
                self.emit(format!("    let {s} = sin(globals.time);"));
                self.emit(format!("    let {c} = cos(globals.time);"));
                self.set_out(id, "sin_time", s);
                self.set_out(id, "cos_time", c);
            }
            "input/uv_scale" => {
                let uv = if self.graph.connection_to(node.id, "uv").is_some() {
                    self.input(node, "uv")
                } else {
                    "mat_uv".to_string()
                };
                let scale = self.input(node, "scale");
                let offset = self.input(node, "offset");
                let v = self.next_var("uv_scaled");
                self.emit(format!("    let {v} = {uv} * {scale} + {offset};"));
                self.set_out(id, "uv", v);
            }
            "input/uv_polar" => {
                let uv = if self.graph.connection_to(node.id, "uv").is_some() {
                    self.input(node, "uv")
                } else {
                    "mat_uv".to_string()
                };
                let center = self.input(node, "center");
                let v = self.next_var("polar");
                self.emit(format!("    let {v}_d = {uv} - {center};"));
                self.emit(format!(
                    "    let {v}_angle = fract(atan2({v}_d.y, {v}_d.x) / 6.2831853 + 1.0);"
                ));
                self.emit(format!("    let {v}_radius = length({v}_d);"));
                self.emit(format!("    let {v} = vec2<f32>({v}_angle, {v}_radius);"));
                self.set_out(id, "uv", v.clone());
                self.set_out(id, "angle", format!("{v}_angle"));
                self.set_out(id, "radius", format!("{v}_radius"));
            }
            "input/uv_rotator" => {
                let uv = if self.graph.connection_to(node.id, "uv").is_some() {
                    self.input(node, "uv")
                } else {
                    "mat_uv".to_string()
                };
                let angle = self.input(node, "angle");
                let center = self.input(node, "center");
                let v = self.next_var("rot");
                self.emit(format!(
                    "    let {v}_cs = vec2<f32>(cos({angle}), sin({angle}));"
                ));
                self.emit(format!("    let {v}_d = {uv} - {center};"));
                self.emit(format!(
                    "    let {v} = {center} + vec2<f32>({v}_d.x * {v}_cs.x - {v}_d.y * {v}_cs.y, {v}_d.x * {v}_cs.y + {v}_d.y * {v}_cs.x);"
                ));
                self.set_out(id, "uv", v);
            }
            "input/uv_panner" => {
                let uv = if self.graph.connection_to(node.id, "uv").is_some() {
                    self.input(node, "uv")
                } else {
                    "mat_uv".to_string()
                };
                let speed = self.input(node, "speed");
                let toff = self.input(node, "time_offset");
                let v = self.next_var("pan");
                self.emit(format!(
                    "    let {v} = {uv} + {speed} * (globals.time + {toff});"
                ));
                self.set_out(id, "uv", v);
            }
            "input/vertex_color" => {
                // Aliased behind `#ifdef VERTEX_COLORS` — meshes without a
                // color attribute don't have the field on `VertexOutput`.
                self.set_out(id, "color", "mat_vertex_color".into());
                self.set_out(id, "r", "mat_vertex_color.r".into());
                self.set_out(id, "g", "mat_vertex_color.g".into());
                self.set_out(id, "b", "mat_vertex_color.b".into());
                self.set_out(id, "a", "mat_vertex_color.a".into());
            }
            "input/camera_position" => {
                self.set_out(id, "position", "view.world_position.xyz".into());
            }
            "input/object_position" => {
                // Column 3 of the model matrix is the object's world-space translation.
                self.set_out(
                    id,
                    "position",
                    "mesh_functions::get_world_from_local(in.instance_index)[3].xyz".into(),
                );
            }
            unknown => self.unknown_node(unknown),
        }
    }

    /// Each `param/*` node reads from `material_params.slots[N]`
    /// where `N` is the slot index allocated by `intern_parameter`.
    /// The resolver writes the master's authored default into that
    /// slot when building the master GraphMaterial; material
    /// instances reuse the master's compiled shader and overwrite
    /// the same slot with their per-instance value.
    pub(crate) fn gen_param_node(&mut self, node: &MaterialNode, id: NodeId) {
        match node.node_type.as_str() {
            "param/float" => {
                let name = param_name(node, "FloatParam");
                let default = match node.input_values.get("default") {
                    Some(PinValue::Float(f)) => *f,
                    _ => 0.0,
                };
                let slot = self.intern_parameter(&name, ParamKind::Float, PinValue::Float(default));
                let v = self.next_var("param_f");
                self.emit(format!("    let {v} = material_params.slots[{slot}].x;"));
                self.set_out(id, "value", v);
            }
            "param/color" => {
                let name = param_name(node, "ColorParam");
                let default = match node.input_values.get("default") {
                    Some(PinValue::Color(c)) => *c,
                    _ => [1.0, 1.0, 1.0, 1.0],
                };
                let slot = self.intern_parameter(&name, ParamKind::Color, PinValue::Color(default));
                let v = self.next_var("param_c");
                self.emit(format!("    let {v} = material_params.slots[{slot}];"));
                self.set_out(id, "value", v);
            }
            "param/vec2" => {
                let name = param_name(node, "Vec2Param");
                let default = match node.input_values.get("default") {
                    Some(PinValue::Vec2(v)) => *v,
                    _ => [0.0, 0.0],
                };
                let slot = self.intern_parameter(&name, ParamKind::Vec2, PinValue::Vec2(default));
                let v = self.next_var("param_v2");
                self.emit(format!("    let {v} = material_params.slots[{slot}].xy;"));
                self.set_out(id, "value", v);
            }
            "param/vec3" => {
                let name = param_name(node, "Vec3Param");
                let default = match node.input_values.get("default") {
                    Some(PinValue::Vec3(v)) => *v,
                    _ => [0.0, 0.0, 0.0],
                };
                let slot = self.intern_parameter(&name, ParamKind::Vec3, PinValue::Vec3(default));
                let v = self.next_var("param_v3");
                self.emit(format!("    let {v} = material_params.slots[{slot}].xyz;"));
                self.set_out(id, "value", v);
            }
            "param/vec4" => {
                let name = param_name(node, "Vec4Param");
                let default = match node.input_values.get("default") {
                    Some(PinValue::Vec4(v)) => *v,
                    _ => [0.0, 0.0, 0.0, 0.0],
                };
                let slot = self.intern_parameter(&name, ParamKind::Vec4, PinValue::Vec4(default));
                let v = self.next_var("param_v4");
                self.emit(format!("    let {v} = material_params.slots[{slot}];"));
                self.set_out(id, "value", v);
            }
            "param/bool" => {
                let name = param_name(node, "BoolParam");
                let default = match node.input_values.get("default") {
                    Some(PinValue::Bool(b)) => *b,
                    _ => false,
                };
                let slot = self.intern_parameter(&name, ParamKind::Bool, PinValue::Bool(default));
                let v = self.next_var("param_b");
                self.emit(format!(
                    "    let {v} = material_params.slots[{slot}].x > 0.5;"
                ));
                self.set_out(id, "value", v);
            }
            unknown => self.unknown_node(unknown),
        }
    }
}
