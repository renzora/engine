//! `color/*` node emitters — constants, colour-space conversion, and grading.
//!
//! The conversion and blend arms set a `uses_*` flag so their WGSL helper is
//! emitted into the module prelude; a graph that never touches HSV never pays
//! for `mat_rgb_to_hsv`.

use super::super::super::graph::{MaterialNode, NodeId, PinValue};
use super::super::ctx::Ctx;

impl Ctx<'_> {
    pub(crate) fn gen_color_node(&mut self, node: &MaterialNode, id: NodeId) {
        match node.node_type.as_str() {
            "color/constant" => {
                let val = node
                    .input_values
                    .get("color")
                    .map(|v| v.to_wgsl())
                    .unwrap_or_else(|| "vec4<f32>(1.0, 1.0, 1.0, 1.0)".to_string());
                self.set_out(id, "color", val.clone());
                self.set_out(id, "rgb", format!("{val}.rgb"));
                self.set_out(id, "r", format!("{val}.r"));
                self.set_out(id, "g", format!("{val}.g"));
                self.set_out(id, "b", format!("{val}.b"));
                self.set_out(id, "a", format!("{val}.a"));
            }
            "color/float" => {
                let val = node
                    .input_values
                    .get("value")
                    .map(|v| v.to_wgsl())
                    .unwrap_or_else(|| "0.0".to_string());
                self.set_out(id, "value", val);
            }
            "color/vec2" => {
                let val = node
                    .input_values
                    .get("value")
                    .map(|v| v.to_wgsl())
                    .unwrap_or_else(|| "vec2<f32>(0.0, 0.0)".to_string());
                self.set_out(id, "value", val);
            }
            "color/vec3" => {
                let val = node
                    .input_values
                    .get("value")
                    .map(|v| v.to_wgsl())
                    .unwrap_or_else(|| "vec3<f32>(0.0, 0.0, 0.0)".to_string());
                self.set_out(id, "value", val);
            }
            "color/lerp" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let t = self.input(node, "t");
                let v = self.next_var("clrp");
                self.emit(format!("    let {v} = mix({a}, {b}, vec4<f32>({t}));"));
                self.set_out(id, "color", v);
            }
            "color/cosine_palette" => {
                let t = self.input(node, "t");
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let c = self.input(node, "c");
                let d = self.input(node, "d");
                let v = self.next_var("pal");
                self.emit(format!(
                    "    let {v} = {a} + {b} * cos(6.2831853 * ({c} * {t} + {d}));"
                ));
                self.set_out(id, "color", v);
            }
            "color/fresnel" => {
                let power = self.input(node, "power");
                let v = self.next_var("fres");
                self.emit(format!("    let {v} = pow(1.0 - max(dot(normalize(view.world_position.xyz - in.world_position.xyz), in.world_normal), 0.0), {power});"));
                self.set_out(id, "result", v);
            }
            "color/srgb_to_linear" => {
                self.uses_srgb = true;
                let c = self.input(node, "color");
                let v = self.next_var("s2l");
                self.emit(format!(
                    "    let {v} = vec4<f32>(mat_srgb_to_linear(({c}).rgb), ({c}).a);"
                ));
                self.set_out(id, "result", v);
            }
            "color/linear_to_srgb" => {
                self.uses_srgb = true;
                let c = self.input(node, "color");
                let v = self.next_var("l2s");
                self.emit(format!(
                    "    let {v} = vec4<f32>(mat_linear_to_srgb(({c}).rgb), ({c}).a);"
                ));
                self.set_out(id, "result", v);
            }
            "color/rgb_to_hsv" => {
                self.uses_hsv = true;
                let rgb = self.input(node, "rgb");
                let v = self.next_var("hsv");
                self.emit(format!("    let {v} = mat_rgb_to_hsv({rgb});"));
                self.set_out(id, "hsv", v.clone());
                self.set_out(id, "h", format!("{v}.x"));
                self.set_out(id, "s", format!("{v}.y"));
                self.set_out(id, "v", format!("{v}.z"));
            }
            "color/hsv_to_rgb" => {
                self.uses_hsv = true;
                let hsv = self.input(node, "hsv");
                let v = self.next_var("rgb");
                self.emit(format!("    let {v} = mat_hsv_to_rgb({hsv});"));
                self.set_out(id, "rgb", v);
            }
            "color/hue_shift" => {
                self.uses_hsv = true;
                let rgb = self.input(node, "rgb");
                let shift = self.input(node, "shift");
                let v = self.next_var("hshift");
                self.emit(format!("    var {v}_hsv = mat_rgb_to_hsv({rgb});"));
                self.emit(format!("    {v}_hsv.x = fract({v}_hsv.x + {shift});"));
                self.emit(format!("    let {v} = mat_hsv_to_rgb({v}_hsv);"));
                self.set_out(id, "rgb", v);
            }
            "color/luminance" => {
                let rgb = self.input(node, "rgb");
                let v = self.next_var("lum");
                self.emit(format!(
                    "    let {v} = dot({rgb}, vec3<f32>(0.2126, 0.7152, 0.0722));"
                ));
                self.set_out(id, "value", v);
            }
            "color/gamma" => {
                let c = self.input(node, "color");
                let g = self.input(node, "gamma");
                let v = self.next_var("gam");
                self.emit(format!("    let {v} = vec4<f32>(pow(max(({c}).rgb, vec3<f32>(0.0)), vec3<f32>({g})), ({c}).a);"));
                self.set_out(id, "result", v);
            }
            "color/brightness_contrast" => {
                let c = self.input(node, "color");
                let b = self.input(node, "brightness");
                let con = self.input(node, "contrast");
                let v = self.next_var("bc");
                self.emit(format!(
                    "    let {v} = vec4<f32>((({c}).rgb - vec3<f32>(0.5)) * {con} + vec3<f32>(0.5 + {b}), ({c}).a);"
                ));
                self.set_out(id, "result", v);
            }
            "color/saturation" => {
                let c = self.input(node, "color");
                let s = self.input(node, "saturation");
                let v = self.next_var("sat_c");
                self.emit(format!(
                    "    let {v}_l = dot(({c}).rgb, vec3<f32>(0.2126, 0.7152, 0.0722));"
                ));
                self.emit(format!(
                    "    let {v} = vec4<f32>(mix(vec3<f32>({v}_l), ({c}).rgb, {s}), ({c}).a);"
                ));
                self.set_out(id, "result", v);
            }
            "color/blend" => {
                self.uses_blend = true;
                let base = self.input(node, "base");
                let blnd = self.input(node, "blend");
                let op = self.input(node, "opacity");
                let mode = node
                    .input_values
                    .get("mode")
                    .and_then(|v| {
                        if let PinValue::Int(i) = v {
                            Some(*i)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                let v = self.next_var("blend");
                self.emit(format!(
                    "    let {v} = mat_blend({base}, {blnd}, {op}, {mode});"
                ));
                self.set_out(id, "result", v);
            }
            unknown => self.unknown_node(unknown),
        }
    }
}
