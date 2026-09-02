//! `animation/*`, `utility/*` and `custom/*` node emitters.
//!
//! The animation nodes are all functions of `globals.time`; the utility ones
//! are thin wrappers over WGSL derivative builtins plus two world-space masks.
//!
//! `custom/code` is the odd one: WGSL has no value-returning blocks, so a
//! user's snippet is wrapped in a generated helper named after the node id and
//! pushed into the module prelude. That is also why custom-code lines are the
//! ones most likely to appear in a compile error — [`Ctx::emit_prelude`]
//! records the span so the error can point at the node.

use super::super::super::graph::{MaterialNode, NodeId, PinValue};
use super::super::ctx::Ctx;

impl Ctx<'_> {
    pub(crate) fn gen_animation_node(&mut self, node: &MaterialNode, id: NodeId) {
        match node.node_type.as_str() {
            "animation/uv_scroll" => {
                let uv = self.input(node, "uv");
                let speed = self.input(node, "speed");
                let v = self.next_var("scroll");
                self.emit(format!("    let {v} = {uv} + {speed} * globals.time;"));
                self.set_out(id, "uv", v);
            }
            "animation/flow_map" => {
                let uv = self.input(node, "uv");
                let flow = self.input(node, "flow");
                let speed = self.input(node, "speed");
                let strength = self.input(node, "strength");
                let phase = self.next_var("phase");
                let v1 = self.next_var("flow_uv1");
                let v2 = self.next_var("flow_uv2");
                let blend = self.next_var("flow_blend");
                self.emit(format!("    let {phase} = fract(globals.time * {speed});"));
                self.emit(format!(
                    "    let {v1} = {uv} + {flow} * {strength} * {phase};"
                ));
                self.emit(format!(
                    "    let {v2} = {uv} + {flow} * {strength} * fract({phase} + 0.5);"
                ));
                self.emit(format!("    let {blend} = abs(2.0 * {phase} - 1.0);"));
                self.set_out(id, "uv1", v1);
                self.set_out(id, "uv2", v2);
                self.set_out(id, "blend", blend);
            }
            "animation/sine_wave" => {
                let freq = self.input(node, "frequency");
                let amp = self.input(node, "amplitude");
                let offset = self.input(node, "offset");
                let v = self.next_var("swave");
                self.emit(format!(
                    "    let {v} = sin(globals.time * {freq} + {offset}) * {amp};"
                ));
                self.set_out(id, "value", v);
            }
            "animation/ping_pong" => {
                let speed = self.input(node, "speed");
                let v = self.next_var("pp");
                self.emit(format!(
                    "    let {v} = abs(fract(globals.time * {speed}) * 2.0 - 1.0);"
                ));
                self.set_out(id, "value", v);
            }
            "animation/flipbook_uv" => {
                let uv = if self.graph.connection_to(node.id, "uv").is_some() {
                    self.input(node, "uv")
                } else {
                    "mat_uv".to_string()
                };
                let frame = self.input(node, "frame");
                let cols = self.input(node, "cols");
                let rows = self.input(node, "rows");
                let v = self.next_var("flip");
                self.emit(format!("    let {v}_cols = max({cols}, 1.0);"));
                self.emit(format!("    let {v}_rows = max({rows}, 1.0);"));
                self.emit(format!("    let {v}_total = {v}_cols * {v}_rows;"));
                self.emit(format!("    let {v}_idx = floor(({frame}) - floor(({frame}) / {v}_total) * {v}_total);"));
                self.emit(format!(
                    "    let {v}_col = floor({v}_idx - floor({v}_idx / {v}_cols) * {v}_cols);"
                ));
                self.emit(format!("    let {v}_row = floor({v}_idx / {v}_cols);"));
                self.emit(format!(
                    "    let {v}_tile = vec2<f32>(1.0 / {v}_cols, 1.0 / {v}_rows);"
                ));
                self.emit(format!("    let {v} = fract({uv}) * {v}_tile + vec2<f32>({v}_col, {v}_row) * {v}_tile;"));
                self.set_out(id, "uv", v);
            }
            "animation/wind" => {
                let strength = self.input(node, "strength");
                let speed = self.input(node, "speed");
                let dir = self.input(node, "direction");
                let turb = self.input(node, "turbulence");
                let mask = self.input(node, "mask");
                let v = self.next_var("wind");
                // Wind uses world position for phase variation + time
                self.emit(format!("    let {v} = vec3<f32>({dir}.x, 0.0, {dir}.y) * sin(globals.time * {speed} + dot(in.world_position.xz, vec2<f32>(0.7, 0.3)) * 3.0 + sin(globals.time * {speed} * 2.3) * {turb}) * {strength} * {mask};"));
                self.set_out(id, "displacement", v);
            }
            unknown => self.unknown_node(unknown),
        }
    }

    pub(crate) fn gen_utility_node(&mut self, node: &MaterialNode, id: NodeId) {
        match node.node_type.as_str() {
            "utility/world_pos_mask" => {
                let height = self.input(node, "height");
                let falloff = self.input(node, "falloff");
                let v = self.next_var("hmask");
                self.emit(format!("    let {v} = saturate((in.world_position.y - {height}) / max({falloff}, 0.001));"));
                self.set_out(id, "mask", v);
            }
            "utility/slope_mask" => {
                let threshold = self.input(node, "threshold");
                let falloff = self.input(node, "falloff");
                let v = self.next_var("slope");
                self.emit(format!("    let {v} = smoothstep({threshold} - {falloff}, {threshold} + {falloff}, in.world_normal.y);"));
                self.set_out(id, "mask", v);
            }
            "utility/depth_fade" => {
                let dist = self.input(node, "distance");
                let v = self.next_var("dfade");
                // Simplified — actual depth fade needs scene depth texture
                self.emit(format!(
                    "    let {v} = saturate(in.world_position.y / max({dist}, 0.001));"
                ));
                self.set_out(id, "fade", v);
            }
            "utility/dpdx" => {
                let val = self.input(node, "value");
                let v = self.next_var("ddx");
                self.emit(format!("    let {v} = dpdx({val});"));
                self.set_out(id, "result", v);
            }
            "utility/dpdy" => {
                let val = self.input(node, "value");
                let v = self.next_var("ddy");
                self.emit(format!("    let {v} = dpdy({val});"));
                self.set_out(id, "result", v);
            }
            "utility/fwidth" => {
                let val = self.input(node, "value");
                let v = self.next_var("fw");
                self.emit(format!("    let {v} = fwidth({val});"));
                self.set_out(id, "result", v);
            }
            "utility/dither" => {
                // 4x4 Bayer dither based on fragment coord
                let v = self.next_var("dith");
                self.emit(format!(
                    "    let {v}_xy = vec2<i32>(i32(in.position.x) & 3, i32(in.position.y) & 3);"
                ));
                self.emit(format!(
                    "    let {v}_bayer = array<f32, 16>(0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);"
                ));
                self.emit(format!(
                    "    let {v} = {v}_bayer[{v}_xy.y * 4 + {v}_xy.x] / 16.0;"
                ));
                self.set_out(id, "value", v);
            }
            "utility/hash" => {
                self.uses_hash = true;
                let val = self.input(node, "value");
                let v = self.next_var("hash");
                self.emit(format!("    let {v} = mat_hash({val});"));
                self.set_out(id, "result", v);
            }
            unknown => self.unknown_node(unknown),
        }
    }

    pub(crate) fn gen_custom_node(&mut self, node: &MaterialNode, id: NodeId) {
        match node.node_type.as_str() {
            "custom/code" => {
                // Resolve inputs first (triggers upstream codegen) — each is
                // coerced to vec4 by `input()` since the pins are declared Vec4.
                let in_a = self.input(node, "a");
                let in_b = self.input(node, "b");
                let in_c = self.input(node, "c");
                let in_d = self.input(node, "d");
                let code = match node.input_values.get("code") {
                    Some(PinValue::String(s)) if !s.trim().is_empty() => s.clone(),
                    _ => "result = a;".to_string(),
                };
                // WGSL has no value-returning blocks, so the snippet runs in a
                // generated helper. Each node id is unique and generated once,
                // so the helper is emitted exactly once with no dedup needed.
                let fn_name = format!("mat_custom_{id}");
                self.emit_prelude(format!(
                    "fn {fn_name}(a: vec4<f32>, b: vec4<f32>, c: vec4<f32>, d: vec4<f32>) -> vec4<f32> {{\n    var result: vec4<f32> = vec4<f32>(0.0, 0.0, 0.0, 1.0);\n    {code}\n    return result;\n}}"
                ));
                let res = self.next_var("custom");
                self.emit(format!(
                    "    let {res} = {fn_name}({in_a}, {in_b}, {in_c}, {in_d});"
                ));
                self.set_out(id, "result", res.clone());
                self.set_out(id, "rgb", format!("{res}.xyz"));
                self.set_out(id, "x", format!("{res}.x"));
                self.set_out(id, "y", format!("{res}.y"));
                self.set_out(id, "z", format!("{res}.z"));
                self.set_out(id, "w", format!("{res}.w"));
            }
            unknown => self.unknown_node(unknown),
        }
    }
}
