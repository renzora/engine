//! WGSL code generation from material graphs.
//!
//! Walks the graph from the output node backwards, generating WGSL code for
//! each node encountered. Produces a complete Bevy-compatible material shader.

use std::collections::{HashMap, HashSet};
use crate::graph::{MaterialGraph, MaterialNode, MaterialDomain, NodeId, PinValue};
use crate::nodes;

// ── Public result types ─────────────────────────────────────────────────────

pub struct CompileResult {
    /// Generated vertex shader (if domain needs custom vertex stage).
    pub vertex_shader: Option<String>,
    /// Generated fragment shader.
    pub fragment_shader: String,
    /// Texture assets needed by this material.
    pub texture_bindings: Vec<TextureBinding>,
    pub domain: MaterialDomain,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TextureBinding {
    pub name: String,
    pub binding: u32,
    pub asset_path: String,
}

// ── Codegen context ─────────────────────────────────────────────────────────

struct Ctx<'a> {
    graph: &'a MaterialGraph,
    /// Maps (node_id, pin_name) → WGSL variable expression.
    output_vars: HashMap<(NodeId, String), String>,
    var_counter: usize,
    processed: HashSet<NodeId>,
    texture_bindings: Vec<TextureBinding>,
    next_texture_binding: u32,
    lines: Vec<String>,
    uses_noise: bool,
    uses_voronoi: bool,
    uses_fbm: bool,
}

impl<'a> Ctx<'a> {
    fn new(graph: &'a MaterialGraph) -> Self {
        Self {
            graph,
            output_vars: HashMap::new(),
            var_counter: 0,
            processed: HashSet::new(),
            texture_bindings: Vec::new(),
            next_texture_binding: 0, // index into texture_0, texture_1, ...
            lines: Vec::new(),
            uses_noise: false,
            uses_voronoi: false,
            uses_fbm: false,
        }
    }

    fn next_var(&mut self, prefix: &str) -> String {
        let name = format!("{}_{}", prefix, self.var_counter);
        self.var_counter += 1;
        name
    }

    fn set_out(&mut self, node: NodeId, pin: &str, expr: String) {
        self.output_vars.insert((node, pin.to_string()), expr);
    }

    /// Look up the PinType for a given pin on a node definition.
    fn pin_type_for(node_type: &str, pin_name: &str, direction: crate::graph::PinDir) -> Option<crate::graph::PinType> {
        let def = nodes::node_def(node_type)?;
        let pins = (def.pins)();
        pins.iter()
            .find(|p| p.name == pin_name && p.direction == direction)
            .map(|p| p.pin_type)
    }

    /// Resolve an input pin value — follows connections or falls back to defaults.
    /// Applies automatic type coercion (e.g. Float → Vec4) when pin types differ.
    fn input(&mut self, node: &MaterialNode, pin_name: &str) -> String {
        // Determine expected type of destination pin
        let dest_type = Self::pin_type_for(&node.node_type, pin_name, crate::graph::PinDir::Input);

        // Check for connection
        if let Some(conn) = self.graph.connection_to(node.id, pin_name) {
            let from_node = conn.from_node;
            let from_pin = conn.from_pin.clone();
            // Generate source node if needed
            if !self.processed.contains(&from_node) {
                if let Some(src) = self.graph.get_node(from_node).cloned() {
                    self.gen_node(&src);
                }
            }
            if let Some(expr) = self.output_vars.get(&(from_node, from_pin.clone())).cloned() {
                // Apply type coercion if source and dest types differ
                if let (Some(dt), Some(src_node)) = (dest_type, self.graph.get_node(from_node)) {
                    if let Some(st) = Self::pin_type_for(&src_node.node_type, &from_pin, crate::graph::PinDir::Output) {
                        return crate::graph::PinType::cast_expr(st, dt, &expr);
                    }
                }
                return expr;
            }
        }

        // Check node-local override
        if let Some(val) = node.get_input_value(pin_name) {
            return val.to_wgsl();
        }

        // Check pin template default
        if let Some(def) = nodes::node_def(&node.node_type) {
            let pins = (def.pins)();
            if let Some(pin) = pins.iter().find(|p| p.name == pin_name) {
                return pin.default_value.to_wgsl();
            }
        }

        "0.0".to_string()
    }

    fn emit(&mut self, line: String) {
        self.lines.push(line);
    }

    fn gen_node(&mut self, node: &MaterialNode) {
        if self.processed.contains(&node.id) {
            return;
        }
        self.processed.insert(node.id);
        let id = node.id;

        match node.node_type.as_str() {
            // ── Inputs ──────────────────────────────────────────────
            "input/uv" => {
                self.set_out(id, "uv", "in.uv".into());
                self.set_out(id, "u", "in.uv.x".into());
                self.set_out(id, "v", "in.uv.y".into());
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
                self.emit(format!("    let {v} = normalize(view.world_position.xyz - in.world_position.xyz);"));
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
            "input/vertex_color" => {
                self.set_out(id, "color", "in.color".into());
                self.set_out(id, "r", "in.color.r".into());
                self.set_out(id, "g", "in.color.g".into());
                self.set_out(id, "b", "in.color.b".into());
                self.set_out(id, "a", "in.color.a".into());
            }
            "input/camera_position" => {
                self.set_out(id, "position", "view.world_position.xyz".into());
            }
            "input/object_position" => {
                // mesh_functions provides mesh[in.instance_index]
                self.set_out(id, "position", "mesh_functions::get_world_from_local(in.instance_index)[3].xyz".into());
            }

            // ── Textures ────────────────────────────────────────────
            "texture/sample" => {
                // Use in.uv when UV pin is unconnected (most common case)
                let uv = if self.graph.connection_to(node.id, "uv").is_some() {
                    self.input(node, "uv")
                } else {
                    "in.uv".to_string()
                };
                let path = node.input_values.get("texture")
                    .and_then(|v| if let PinValue::TexturePath(s) = v { Some(s.clone()) } else { None })
                    .unwrap_or_default();

                let slot = self.next_texture_binding;
                let tex_name = format!("texture_{slot}");
                let samp_name = format!("texture_{slot}_sampler");
                self.next_texture_binding += 1;

                self.texture_bindings.push(TextureBinding {
                    name: tex_name.clone(),
                    binding: slot,
                    asset_path: path,
                });

                let uv_expr = uv;
                let v = self.next_var("tex");
                self.emit(format!("    let {v} = textureSample({tex_name}, {samp_name}, {uv_expr});"));
                self.set_out(id, "color", v.clone());
                self.set_out(id, "rgb", format!("{v}.rgb"));
                self.set_out(id, "r", format!("{v}.r"));
                self.set_out(id, "g", format!("{v}.g"));
                self.set_out(id, "b", format!("{v}.b"));
                self.set_out(id, "a", format!("{v}.a"));
            }

            "texture/sample_normal" => {
                let uv = if self.graph.connection_to(node.id, "uv").is_some() {
                    self.input(node, "uv")
                } else {
                    "in.uv".to_string()
                };
                let strength = self.input(node, "strength");
                let path = node.input_values.get("texture")
                    .and_then(|v| if let PinValue::TexturePath(s) = v { Some(s.clone()) } else { None })
                    .unwrap_or_default();

                let slot = self.next_texture_binding;
                let tex_name = format!("texture_{slot}");
                let samp_name = format!("texture_{slot}_sampler");
                self.next_texture_binding += 1;

                self.texture_bindings.push(TextureBinding {
                    name: tex_name.clone(),
                    binding: slot,
                    asset_path: path,
                });

                let raw = self.next_var("nraw");
                let n = self.next_var("nmap");
                self.emit(format!("    let {raw} = textureSample({tex_name}, {samp_name}, {uv}).rgb * 2.0 - 1.0;"));
                self.emit(format!("    let {n} = normalize(vec3<f32>({raw}.xy * {strength}, {raw}.z));"));
                self.set_out(id, "normal", n);
            }

            "texture/triplanar" => {
                let scale = self.input(node, "scale");
                let sharpness = self.input(node, "sharpness");
                let path = node.input_values.get("texture")
                    .and_then(|v| if let PinValue::TexturePath(s) = v { Some(s.clone()) } else { None })
                    .unwrap_or_default();

                let slot = self.next_texture_binding;
                let tex_name = format!("texture_{slot}");
                let samp_name = format!("texture_{slot}_sampler");
                self.next_texture_binding += 1;

                self.texture_bindings.push(TextureBinding {
                    name: tex_name.clone(),
                    binding: slot,
                    asset_path: path,
                });

                let w = self.next_var("tri_w");
                let v = self.next_var("tri");
                self.emit(format!("    let {w} = pow(abs(in.world_normal), vec3<f32>({sharpness}));"));
                self.emit(format!("    let {w} = {w} / ({w}.x + {w}.y + {w}.z);"));
                let p = format!("in.world_position.xyz * {scale}");
                self.emit(format!("    let {v} = textureSample({tex_name}, {samp_name}, {p}.yz) * {w}.x + textureSample({tex_name}, {samp_name}, {p}.xz) * {w}.y + textureSample({tex_name}, {samp_name}, {p}.xy) * {w}.z;"));
                self.set_out(id, "color", v.clone());
                self.set_out(id, "rgb", format!("{v}.rgb"));
            }

            // ── Math ────────────────────────────────────────────────
            "math/add" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("add");
                self.emit(format!("    let {v} = {a} + {b};"));
                self.set_out(id, "result", v);
            }
            "math/subtract" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("sub");
                self.emit(format!("    let {v} = {a} - {b};"));
                self.set_out(id, "result", v);
            }
            "math/multiply" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("mul");
                self.emit(format!("    let {v} = {a} * {b};"));
                self.set_out(id, "result", v);
            }
            "math/divide" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("div");
                self.emit(format!("    let {v} = {a} / max({b}, 0.000001);"));
                self.set_out(id, "result", v);
            }
            "math/power" => {
                let base = self.input(node, "base");
                let exp = self.input(node, "exp");
                let v = self.next_var("pow");
                self.emit(format!("    let {v} = pow(abs({base}), {exp});"));
                self.set_out(id, "result", v);
            }
            "math/abs" => {
                let val = self.input(node, "value");
                let v = self.next_var("abs");
                self.emit(format!("    let {v} = abs({val});"));
                self.set_out(id, "result", v);
            }
            "math/negate" => {
                let val = self.input(node, "value");
                let v = self.next_var("neg");
                self.emit(format!("    let {v} = -{val};"));
                self.set_out(id, "result", v);
            }
            "math/one_minus" => {
                let val = self.input(node, "value");
                let v = self.next_var("om");
                self.emit(format!("    let {v} = 1.0 - {val};"));
                self.set_out(id, "result", v);
            }
            "math/fract" => {
                let val = self.input(node, "value");
                let v = self.next_var("frc");
                self.emit(format!("    let {v} = fract({val});"));
                self.set_out(id, "result", v);
            }
            "math/floor" => {
                let val = self.input(node, "value");
                let v = self.next_var("flr");
                self.emit(format!("    let {v} = floor({val});"));
                self.set_out(id, "result", v);
            }
            "math/ceil" => {
                let val = self.input(node, "value");
                let v = self.next_var("cel");
                self.emit(format!("    let {v} = ceil({val});"));
                self.set_out(id, "result", v);
            }
            "math/min" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("mn");
                self.emit(format!("    let {v} = min({a}, {b});"));
                self.set_out(id, "result", v);
            }
            "math/max" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("mx");
                self.emit(format!("    let {v} = max({a}, {b});"));
                self.set_out(id, "result", v);
            }
            "math/clamp" => {
                let val = self.input(node, "value");
                let lo = self.input(node, "min");
                let hi = self.input(node, "max");
                let v = self.next_var("cmp");
                self.emit(format!("    let {v} = clamp({val}, {lo}, {hi});"));
                self.set_out(id, "result", v);
            }
            "math/lerp" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let t = self.input(node, "t");
                let v = self.next_var("lrp");
                self.emit(format!("    let {v} = mix({a}, {b}, {t});"));
                self.set_out(id, "result", v);
            }
            "math/smoothstep" => {
                let e0 = self.input(node, "edge0");
                let e1 = self.input(node, "edge1");
                let val = self.input(node, "value");
                let v = self.next_var("ss");
                self.emit(format!("    let {v} = smoothstep({e0}, {e1}, {val});"));
                self.set_out(id, "result", v);
            }
            "math/step" => {
                let edge = self.input(node, "edge");
                let val = self.input(node, "value");
                let v = self.next_var("stp");
                self.emit(format!("    let {v} = step({edge}, {val});"));
                self.set_out(id, "result", v);
            }
            "math/remap" => {
                let val = self.input(node, "value");
                let in_min = self.input(node, "in_min");
                let in_max = self.input(node, "in_max");
                let out_min = self.input(node, "out_min");
                let out_max = self.input(node, "out_max");
                let v = self.next_var("remap");
                self.emit(format!("    let {v} = {out_min} + ({val} - {in_min}) / max({in_max} - {in_min}, 0.000001) * ({out_max} - {out_min});"));
                self.set_out(id, "result", v);
            }
            "math/sin" => {
                let val = self.input(node, "value");
                let v = self.next_var("sin");
                self.emit(format!("    let {v} = sin({val});"));
                self.set_out(id, "result", v);
            }
            "math/cos" => {
                let val = self.input(node, "value");
                let v = self.next_var("cos");
                self.emit(format!("    let {v} = cos({val});"));
                self.set_out(id, "result", v);
            }
            "math/saturate" => {
                let val = self.input(node, "value");
                let v = self.next_var("sat");
                self.emit(format!("    let {v} = saturate({val});"));
                self.set_out(id, "result", v);
            }

            // ── Vector ──────────────────────────────────────────────
            "vector/split_vec2" => {
                let vec = self.input(node, "vector");
                self.set_out(id, "x", format!("{vec}.x"));
                self.set_out(id, "y", format!("{vec}.y"));
            }
            "vector/split_vec3" => {
                let vec = self.input(node, "vector");
                self.set_out(id, "x", format!("{vec}.x"));
                self.set_out(id, "y", format!("{vec}.y"));
                self.set_out(id, "z", format!("{vec}.z"));
            }
            "vector/combine_vec2" => {
                let x = self.input(node, "x");
                let y = self.input(node, "y");
                let v = self.next_var("v2");
                self.emit(format!("    let {v} = vec2<f32>({x}, {y});"));
                self.set_out(id, "vector", v);
            }
            "vector/combine_vec3" => {
                let x = self.input(node, "x");
                let y = self.input(node, "y");
                let z = self.input(node, "z");
                let v = self.next_var("v3");
                self.emit(format!("    let {v} = vec3<f32>({x}, {y}, {z});"));
                self.set_out(id, "vector", v);
            }
            "vector/combine_vec4" => {
                let x = self.input(node, "x");
                let y = self.input(node, "y");
                let z = self.input(node, "z");
                let w = self.input(node, "w");
                let v = self.next_var("v4");
                self.emit(format!("    let {v} = vec4<f32>({x}, {y}, {z}, {w});"));
                self.set_out(id, "vector", v);
            }
            "vector/dot" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("dot");
                self.emit(format!("    let {v} = dot({a}, {b});"));
                self.set_out(id, "result", v);
            }
            "vector/cross" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("cross");
                self.emit(format!("    let {v} = cross({a}, {b});"));
                self.set_out(id, "result", v);
            }
            "vector/normalize" => {
                let vec = self.input(node, "vector");
                let v = self.next_var("norm");
                self.emit(format!("    let {v} = normalize({vec});"));
                self.set_out(id, "result", v);
            }
            "vector/distance" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("dist");
                self.emit(format!("    let {v} = distance({a}, {b});"));
                self.set_out(id, "result", v);
            }
            "vector/length" => {
                let vec = self.input(node, "vector");
                let v = self.next_var("len");
                self.emit(format!("    let {v} = length({vec});"));
                self.set_out(id, "result", v);
            }
            "vector/reflect" => {
                let inc = self.input(node, "incident");
                let n = self.input(node, "normal");
                let v = self.next_var("refl");
                self.emit(format!("    let {v} = reflect({inc}, {n});"));
                self.set_out(id, "result", v);
            }

            // ── Color ───────────────────────────────────────────────
            "color/constant" => {
                let val = node.input_values.get("color")
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
                let val = node.input_values.get("value")
                    .map(|v| v.to_wgsl())
                    .unwrap_or_else(|| "0.0".to_string());
                self.set_out(id, "value", val);
            }
            "color/vec2" => {
                let val = node.input_values.get("value")
                    .map(|v| v.to_wgsl())
                    .unwrap_or_else(|| "vec2<f32>(0.0, 0.0)".to_string());
                self.set_out(id, "value", val);
            }
            "color/vec3" => {
                let val = node.input_values.get("value")
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
                self.emit(format!("    let {v} = {a} + {b} * cos(6.2831853 * ({c} * {t} + {d}));"));
                self.set_out(id, "color", v);
            }
            "color/fresnel" => {
                let power = self.input(node, "power");
                let v = self.next_var("fres");
                self.emit(format!("    let {v} = pow(1.0 - max(dot(normalize(view.world_position.xyz - in.world_position.xyz), in.world_normal), 0.0), {power});"));
                self.set_out(id, "result", v);
            }

            // ── Procedural ──────────────────────────────────────────
            "procedural/noise_perlin" | "procedural/noise_simplex" => {
                self.uses_noise = true;
                let uv = self.input(node, "uv");
                let scale = self.input(node, "scale");
                let v = self.next_var("noise");
                self.emit(format!("    let {v} = mat_noise({uv} * {scale});"));
                self.set_out(id, "value", v);
            }
            "procedural/noise_voronoi" => {
                self.uses_voronoi = true;
                let uv = self.input(node, "uv");
                let scale = self.input(node, "scale");
                let v = self.next_var("vor");
                self.emit(format!("    let {v} = mat_voronoi({uv} * {scale});"));
                self.set_out(id, "distance", format!("{v}.x"));
                self.set_out(id, "cell_id", format!("{v}.y"));
            }
            "procedural/noise_fbm" => {
                self.uses_noise = true;
                self.uses_fbm = true;
                let uv = self.input(node, "uv");
                let scale = self.input(node, "scale");
                let octaves = self.input(node, "octaves");
                let lac = self.input(node, "lacunarity");
                let pers = self.input(node, "persistence");
                let v = self.next_var("fbm");
                self.emit(format!("    let {v} = mat_fbm({uv} * {scale}, i32({octaves}), {lac}, {pers});"));
                self.set_out(id, "value", v);
            }
            "procedural/checkerboard" => {
                let uv = self.input(node, "uv");
                let scale = self.input(node, "scale");
                let v = self.next_var("check");
                self.emit(format!("    let {v} = step(0.5, fract(floor({uv}.x * {scale}) * 0.5 + floor({uv}.y * {scale}) * 0.5 + 0.25));"));
                self.set_out(id, "value", v);
            }
            "procedural/gradient" => {
                let uv = self.input(node, "uv");
                self.set_out(id, "u", format!("{uv}.x"));
                self.set_out(id, "v", format!("{uv}.y"));
            }
            "procedural/brick" => {
                let uv = self.input(node, "uv");
                let scale = self.input(node, "scale");
                let mortar = self.input(node, "mortar");
                let v = self.next_var("brick");
                let buv = self.next_var("buv");
                self.emit(format!("    var {buv} = {uv} * {scale};"));
                self.emit(format!("    {buv}.x = {buv}.x + step(1.0, fract({buv}.y * 0.5)) * 0.5;"));
                self.emit(format!("    let {v} = step({mortar}, fract({buv}.x)) * step({mortar}, fract({buv}.y));"));
                self.set_out(id, "value", v);
            }
            "procedural/normal_from_height" => {
                let height = self.input(node, "height");
                let strength = self.input(node, "strength");
                let v = self.next_var("nfh");
                self.emit(format!("    let {v} = normalize(vec3<f32>(dpdx({height}) * {strength}, dpdy({height}) * {strength}, 1.0));"));
                self.set_out(id, "normal", v);
            }

            // ── Animation ───────────────────────────────────────────
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
                self.emit(format!("    let {v1} = {uv} + {flow} * {strength} * {phase};"));
                self.emit(format!("    let {v2} = {uv} + {flow} * {strength} * fract({phase} + 0.5);"));
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
                self.emit(format!("    let {v} = sin(globals.time * {freq} + {offset}) * {amp};"));
                self.set_out(id, "value", v);
            }
            "animation/ping_pong" => {
                let speed = self.input(node, "speed");
                let v = self.next_var("pp");
                self.emit(format!("    let {v} = abs(fract(globals.time * {speed}) * 2.0 - 1.0);"));
                self.set_out(id, "value", v);
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

            // ── Utility ─────────────────────────────────────────────
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
                self.emit(format!("    let {v} = saturate(in.world_position.y / max({dist}, 0.001));"));
                self.set_out(id, "fade", v);
            }

            // Output nodes are handled in compile(), not here
            t if t.starts_with("output/") => {}

            unknown => {
                self.lines.push(format!("    // Unknown node type: {unknown}"));
            }
        }
    }
}

// ── Public compile function ─────────────────────────────────────────────────

pub fn compile(graph: &MaterialGraph) -> CompileResult {
    let mut errors = Vec::new();

    let output_node = match graph.output_node() {
        Some(n) => n.clone(),
        None => {
            errors.push("No output node found in graph".to_string());
            return CompileResult {
                vertex_shader: None,
                fragment_shader: String::new(),
                texture_bindings: Vec::new(),
                domain: graph.domain,
                errors,
                warnings: Vec::new(),
            };
        }
    };

    let mut ctx = Ctx::new(graph);

    // Generate code for all inputs connected to the output node
    let output_pins: Vec<String> = if let Some(def) = nodes::node_def(&output_node.node_type) {
        (def.pins)()
            .iter()
            .filter(|p| p.direction == crate::graph::PinDir::Input)
            .map(|p| p.name.clone())
            .collect()
    } else {
        errors.push(format!("Unknown output node type: {}", output_node.node_type));
        Vec::new()
    };

    // Resolve each output pin's input (triggers recursive codegen)
    let mut resolved: HashMap<String, String> = HashMap::new();
    for pin_name in &output_pins {
        let expr = ctx.input(&output_node, pin_name);
        resolved.insert(pin_name.clone(), expr);
    }

    // Build the full shader
    let fragment_shader = match graph.domain {
        MaterialDomain::Surface | MaterialDomain::Vegetation => {
            build_pbr_shader(&ctx, &resolved, graph.domain)
        }
        MaterialDomain::TerrainLayer => {
            build_terrain_layer_shader(&ctx, &resolved)
        }
        MaterialDomain::Unlit => {
            build_unlit_shader(&ctx, &resolved)
        }
    };

    let vertex_shader = if graph.domain == MaterialDomain::Vegetation {
        if resolved.contains_key("vertex_offset") {
            Some(build_vegetation_vertex_shader(&ctx, &resolved))
        } else {
            None
        }
    } else {
        None
    };

    CompileResult {
        vertex_shader,
        fragment_shader,
        texture_bindings: ctx.texture_bindings,
        domain: graph.domain,
        errors,
        warnings: Vec::new(),
    }
}

// ── Shader builders ─────────────────────────────────────────────────────────

fn noise_helpers(ctx: &Ctx) -> String {
    let mut s = String::new();
    if ctx.uses_noise {
        s.push_str(r#"
fn mat_hash(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

fn mat_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(mat_hash(i + vec2<f32>(0.0, 0.0)), mat_hash(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(mat_hash(i + vec2<f32>(0.0, 1.0)), mat_hash(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y
    );
}
"#);
    }
    if ctx.uses_fbm {
        s.push_str(r#"
fn mat_fbm(uv: vec2<f32>, octaves: i32, lacunarity: f32, persistence: f32) -> f32 {
    var p = uv;
    var value = 0.0;
    var amplitude = 0.5;
    for (var i = 0; i < octaves; i = i + 1) {
        value = value + mat_noise(p) * amplitude;
        p = p * lacunarity;
        amplitude = amplitude * persistence;
    }
    return value;
}
"#);
    }
    if ctx.uses_voronoi {
        s.push_str(r#"
fn mat_voronoi(p: vec2<f32>) -> vec2<f32> {
    let n = floor(p);
    let f = fract(p);
    var min_dist = 8.0;
    var cell = 0.0;
    for (var j = -1; j <= 1; j = j + 1) {
        for (var i = -1; i <= 1; i = i + 1) {
            let g = vec2<f32>(f32(i), f32(j));
            let o = vec2<f32>(mat_hash(n + g), mat_hash(n + g + vec2<f32>(57.0, 113.0)));
            let d = length(g + o - f);
            if (d < min_dist) {
                min_dist = d;
                cell = mat_hash(n + g + vec2<f32>(234.0, 567.0));
            }
        }
    }
    return vec2<f32>(min_dist, cell);
}
"#);
    }
    s
}

fn texture_bindings_wgsl(_ctx: &Ctx) -> String {
    // Always declare all 4 texture slots so the pipeline layout is stable.
    // Unused slots are bound to a 1x1 white fallback on the Rust side.
    // Bevy 0.18: MATERIAL_BIND_GROUP_INDEX = 3 (not 2).
    let mut s = String::new();
    for slot in 0..4u32 {
        let tex_binding = 1 + slot * 2;
        let samp_binding = tex_binding + 1;
        s.push_str(&format!(
            "@group(3) @binding({tex_binding}) var texture_{slot}: texture_2d<f32>;\n",
        ));
        s.push_str(&format!(
            "@group(3) @binding({samp_binding}) var texture_{slot}_sampler: sampler;\n",
        ));
    }
    s
}

fn build_pbr_shader(ctx: &Ctx, resolved: &HashMap<String, String>, _domain: MaterialDomain) -> String {
    let base_color = resolved.get("base_color").cloned().unwrap_or("vec4<f32>(0.8, 0.8, 0.8, 1.0)".into());
    let metallic = resolved.get("metallic").cloned().unwrap_or("0.0".into());
    let roughness = resolved.get("roughness").cloned().unwrap_or("0.5".into());
    let normal_connected = ctx.graph.connection_to(ctx.graph.output_node().unwrap().id, "normal").is_some();
    let normal = if normal_connected { resolved.get("normal") } else { None };
    let emissive = resolved.get("emissive").cloned().unwrap_or("vec3<f32>(0.0, 0.0, 0.0)".into());
    let ao = resolved.get("ao").cloned().unwrap_or("1.0".into());
    let alpha = resolved.get("alpha").cloned().unwrap_or("1.0".into());

    let mut shader = String::new();

    shader.push_str("#import bevy_pbr::{\n");
    shader.push_str("    pbr_functions,\n");
    shader.push_str("    pbr_types::PbrInput,\n");
    shader.push_str("    pbr_types::pbr_input_new,\n");
    shader.push_str("    mesh_view_bindings::view,\n");
    shader.push_str("    mesh_view_bindings::globals,\n");
    shader.push_str("    forward_io::VertexOutput,\n");
    shader.push_str("}\n\n");

    // Texture bindings
    shader.push_str(&texture_bindings_wgsl(ctx));

    // Helper functions
    shader.push_str(&noise_helpers(ctx));

    // Fragment entry point
    shader.push_str("\n@fragment\n");
    shader.push_str("fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {\n");

    // Generated code lines
    for line in &ctx.lines {
        shader.push_str(line);
        shader.push('\n');
    }

    // PBR assembly — matches legacy BlueprintMaterial pattern
    shader.push_str("\n    // PBR Output\n");
    shader.push_str("    var pbr_input: PbrInput = pbr_input_new();\n");
    shader.push_str(&format!("    pbr_input.material.base_color = {base_color};\n"));
    shader.push_str(&format!("    pbr_input.material.metallic = {metallic};\n"));
    shader.push_str(&format!("    pbr_input.material.perceptual_roughness = {roughness};\n"));
    shader.push_str(&format!("    pbr_input.material.emissive = vec4<f32>({emissive}, 1.0);\n"));
    shader.push_str(&format!("    pbr_input.diffuse_occlusion = vec3<f32>({ao});\n"));

    if let Some(n) = normal {
        shader.push_str(&format!("    pbr_input.world_normal = normalize({n});\n"));
        shader.push_str(&format!("    pbr_input.N = normalize({n});\n"));
    } else {
        shader.push_str("    pbr_input.world_normal = in.world_normal;\n");
        shader.push_str("    pbr_input.N = normalize(in.world_normal);\n");
    }
    shader.push_str("    pbr_input.world_position = in.world_position;\n");
    shader.push_str("    pbr_input.V = pbr_functions::calculate_view(in.world_position, pbr_input.is_orthographic);\n");
    shader.push_str("    pbr_input.frag_coord = in.position;\n");

    shader.push_str("\n    var color = pbr_functions::apply_pbr_lighting(pbr_input);\n");
    shader.push_str("    color = pbr_functions::main_pass_post_lighting_processing(pbr_input, color);\n");
    shader.push_str(&format!("    color.a = {alpha};\n"));
    shader.push_str("    return color;\n");
    shader.push_str("}\n");

    shader
}

fn build_terrain_layer_shader(ctx: &Ctx, resolved: &HashMap<String, String>) -> String {
    let base_color = resolved.get("base_color").cloned().unwrap_or("vec4<f32>(0.5, 0.5, 0.5, 1.0)".into());
    let metallic = resolved.get("metallic").cloned().unwrap_or("0.0".into());
    let roughness = resolved.get("roughness").cloned().unwrap_or("0.5".into());
    let _height = resolved.get("height").cloned().unwrap_or("0.5".into());

    let mut shader = String::new();
    shader.push_str("// Auto-generated terrain layer shader\n");
    shader.push_str("#import bevy_pbr::{\n");
    shader.push_str("    mesh_view_bindings::globals,\n");
    shader.push_str("}\n\n");
    shader.push_str(&texture_bindings_wgsl(ctx));
    shader.push_str(&noise_helpers(ctx));

    // layer_main: returns base color
    shader.push_str("\nfn layer_main(uv: vec2<f32>, world_pos: vec3<f32>, world_normal: vec3<f32>, time: f32) -> vec4<f32> {\n");
    shader.push_str("    // Alias inputs for compatibility\n");
    shader.push_str("    struct FakeIn { uv: vec2<f32>, world_position: vec4<f32>, world_normal: vec3<f32> };\n");
    shader.push_str("    let in = FakeIn(uv, vec4<f32>(world_pos, 1.0), world_normal);\n");
    for line in &ctx.lines {
        shader.push_str(line);
        shader.push('\n');
    }
    shader.push_str(&format!("    return {base_color};\n"));
    shader.push_str("}\n\n");

    // layer_pbr: returns (metallic, roughness)
    shader.push_str("fn layer_pbr(uv: vec2<f32>, world_pos: vec3<f32>) -> vec2<f32> {\n");
    shader.push_str(&format!("    return vec2<f32>({metallic}, {roughness});\n"));
    shader.push_str("}\n");

    shader
}

fn build_unlit_shader(ctx: &Ctx, resolved: &HashMap<String, String>) -> String {
    let color = resolved.get("color").cloned().unwrap_or("vec4<f32>(1.0, 1.0, 1.0, 1.0)".into());
    let alpha = resolved.get("alpha").cloned().unwrap_or("1.0".into());

    let mut shader = String::new();
    shader.push_str("#import bevy_pbr::{\n");
    shader.push_str("    mesh_view_bindings::globals,\n");
    shader.push_str("    forward_io::VertexOutput,\n");
    shader.push_str("}\n\n");
    shader.push_str(&texture_bindings_wgsl(ctx));
    shader.push_str(&noise_helpers(ctx));

    shader.push_str("\n@fragment\n");
    shader.push_str("fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {\n");
    for line in &ctx.lines {
        shader.push_str(line);
        shader.push('\n');
    }
    shader.push_str(&format!("    var out_color = {color};\n"));
    shader.push_str(&format!("    out_color.a = {alpha};\n"));
    shader.push_str("    return out_color;\n");
    shader.push_str("}\n");

    shader
}

fn build_vegetation_vertex_shader(_ctx: &Ctx, resolved: &HashMap<String, String>) -> String {
    let vertex_offset = resolved.get("vertex_offset").cloned().unwrap_or("vec3<f32>(0.0, 0.0, 0.0)".into());

    let mut shader = String::new();
    shader.push_str("#import bevy_pbr::mesh_functions\n");
    shader.push_str("#import bevy_pbr::forward_io::{Vertex, VertexOutput}\n");
    shader.push_str("#import bevy_pbr::mesh_view_bindings::globals\n\n");

    shader.push_str("@vertex\n");
    shader.push_str("fn vertex(in: Vertex) -> VertexOutput {\n");
    shader.push_str("    var out: VertexOutput;\n");
    shader.push_str("    var world_pos = mesh_functions::mesh_position_local_to_world(\n");
    shader.push_str("        mesh_functions::get_world_from_local(in.instance_index),\n");
    shader.push_str("        vec4<f32>(in.position, 1.0)\n");
    shader.push_str("    );\n");

    // Wind vertex displacement — the resolved expression references globals.time
    // which is available since we imported Globals
    shader.push_str(&format!("    world_pos = vec4<f32>(world_pos.xyz + {vertex_offset}, world_pos.w);\n"));

    shader.push_str("    out.world_position = world_pos;\n");
    shader.push_str("    out.position = mesh_functions::mesh_position_world_to_clip(world_pos);\n");
    shader.push_str("    out.world_normal = mesh_functions::mesh_normal_local_to_world(in.normal, in.instance_index);\n");
    shader.push_str("    out.uv = in.uv;\n");
    shader.push_str("    return out;\n");
    shader.push_str("}\n");

    shader
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::*;

    #[test]
    fn compile_default_surface() {
        let graph = MaterialGraph::new("Test", MaterialDomain::Surface);
        let result = compile(&graph);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(result.fragment_shader.contains("pbr_input_new"));
        assert!(result.fragment_shader.contains("apply_pbr_lighting"));
    }

    #[test]
    fn compile_checkerboard() {
        let mut graph = MaterialGraph::new("Checker", MaterialDomain::Surface);
        let uv_id = graph.add_node("input/uv", [-200.0, 0.0]);
        let check_id = graph.add_node("procedural/checkerboard", [0.0, 0.0]);
        let lerp_id = graph.add_node("color/lerp", [200.0, 0.0]);

        // UV → checkerboard
        graph.connect(uv_id, "uv", check_id, "uv");
        // checkerboard → lerp T
        graph.connect(check_id, "value", lerp_id, "t");

        // Set colors on the lerp
        let output_id = graph.output_node().unwrap().id;
        graph.connect(lerp_id, "color", output_id, "base_color");

        let result = compile(&graph);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(result.fragment_shader.contains("fract(floor("));
    }

    #[test]
    fn compile_terrain_layer() {
        let graph = MaterialGraph::new("Grass", MaterialDomain::TerrainLayer);
        let result = compile(&graph);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(result.fragment_shader.contains("fn layer_main"));
        assert!(result.fragment_shader.contains("fn layer_pbr"));
    }

    #[test]
    fn compile_unlit() {
        let graph = MaterialGraph::new("Glow", MaterialDomain::Unlit);
        let result = compile(&graph);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(!result.fragment_shader.contains("pbr_input_new"));
    }

    #[test]
    fn compile_checkerboard_direct_to_base_color() {
        // Float output → Color input (should auto-widen to vec4)
        let mut graph = MaterialGraph::new("CheckDirect", MaterialDomain::Surface);
        let uv_id = graph.add_node("input/uv", [-200.0, 0.0]);
        let check_id = graph.add_node("procedural/checkerboard", [0.0, 0.0]);
        let output_id = graph.output_node().unwrap().id;

        graph.connect(uv_id, "uv", check_id, "uv");
        graph.connect(check_id, "value", output_id, "base_color");

        let result = compile(&graph);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        // The float must be widened to vec4 for base_color
        assert!(
            result.fragment_shader.contains("vec4<f32>(check_"),
            "Expected float→vec4 coercion in shader:\n{}",
            result.fragment_shader
        );
    }
}
