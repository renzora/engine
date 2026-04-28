//! WGSL code generation from material graphs.
//!
//! Walks the graph from the output node backwards, generating WGSL code for
//! each node encountered. Produces a complete Bevy-compatible material shader.

use std::collections::{HashMap, HashSet};
use super::graph::{self, MaterialFunction, MaterialGraph, MaterialNode, MaterialDomain, NodeId, PinValue};
use super::nodes;

/// Registry of loaded material functions, keyed by function name.
/// Populated from disk by the resolver and passed to `compile()`.
pub type FunctionRegistry = HashMap<String, MaterialFunction>;

/// Sanitize a function name into a WGSL-identifier-safe string.
fn safe_fn_ident(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    out
}

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
    /// True when the graph connects `specular_transmission` or
    /// `diffuse_transmission` on its Surface output. The resolver must flip
    /// the CPU-side `StandardMaterial.specular_transmission > 0` on the base,
    /// because that's what tells Bevy to schedule the transmissive pass and
    /// populate `view_transmission_texture` — a runtime-only shader mutation
    /// isn't enough to trigger the pipeline decision.
    pub requires_transmission: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureKind {
    /// Standard 2D sampler (bindings 100/102/104/106 + paired samplers).
    D2,
    /// User cubemap (binding 108). One per material.
    Cube,
    /// 2D array (binding 110). One per material.
    D2Array,
    /// 3D volume (binding 112). One per material.
    D3,
}

#[derive(Debug, Clone)]
pub struct TextureBinding {
    pub name: String,
    pub binding: u32,
    pub asset_path: String,
    pub kind: TextureKind,
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
    /// WGSL declarations emitted at module scope (structs, helper fns).
    module_prelude: Vec<String>,
    /// Registry of available material functions (subgraphs).
    functions: Option<&'a FunctionRegistry>,
    /// Names of functions whose WGSL has already been emitted into module_prelude
    /// so multiple calls to the same function share a single definition.
    emitted_functions: HashSet<String>,
    /// Names of functions currently being compiled — for cycle detection.
    compiling_functions: HashSet<String>,
    uses_noise: bool,
    uses_voronoi: bool,
    uses_voronoi_full: bool,
    uses_fbm: bool,
    uses_fbm_ridged: bool,
    uses_fbm_turbulence: bool,
    uses_fbm_billow: bool,
    uses_curl: bool,
    uses_hash: bool,
    uses_hsv: bool,
    uses_srgb: bool,
    uses_blend: bool,
    uses_scene_depth: bool,
    uses_scene_normal: bool,
    uses_motion_vector: bool,
    uses_transmission: bool,
    uses_env_map: bool,
    uses_hex_tile: bool,
    uses_cube_0: bool,
    uses_array_0: bool,
    uses_volume_0: bool,
}

impl<'a> Ctx<'a> {
    fn new(graph: &'a MaterialGraph) -> Self {
        Self::new_with_functions(graph, None)
    }

    fn new_with_functions(graph: &'a MaterialGraph, functions: Option<&'a FunctionRegistry>) -> Self {
        Self {
            graph,
            output_vars: HashMap::new(),
            var_counter: 0,
            processed: HashSet::new(),
            texture_bindings: Vec::new(),
            next_texture_binding: 0,
            lines: Vec::new(),
            module_prelude: Vec::new(),
            functions,
            emitted_functions: HashSet::new(),
            compiling_functions: HashSet::new(),
            uses_noise: false,
            uses_voronoi: false,
            uses_voronoi_full: false,
            uses_fbm: false,
            uses_fbm_ridged: false,
            uses_fbm_turbulence: false,
            uses_fbm_billow: false,
            uses_curl: false,
            uses_hash: false,
            uses_hsv: false,
            uses_srgb: false,
            uses_blend: false,
            uses_scene_depth: false,
            uses_scene_normal: false,
            uses_motion_vector: false,
            uses_transmission: false,
            uses_env_map: false,
            uses_hex_tile: false,
            uses_cube_0: false,
            uses_array_0: false,
            uses_volume_0: false,
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
    fn pin_type_for(node_type: &str, pin_name: &str, direction: graph::PinDir) -> Option<graph::PinType> {
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
        let dest_type = Self::pin_type_for(&node.node_type, pin_name, graph::PinDir::Input);

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
                    if let Some(st) = Self::pin_type_for(&src_node.node_type, &from_pin, graph::PinDir::Output) {
                        return graph::PinType::cast_expr(st, dt, &expr);
                    }
                }
                return expr;
            }
        }

        // Check node-local override. If the user set a PinValue whose type
        // doesn't match the declared pin type (e.g. Vec3 value on a Float pin),
        // coerce it so downstream code — which assumes the declared type —
        // sees a matching-typed expression. Without this the shader ends up
        // composing things like `vec4(vec3, vec3, vec3, 1.0)` (10 components).
        if let Some(val) = node.get_input_value(pin_name) {
            let expr = val.to_wgsl();
            if let Some(dt) = dest_type {
                let vt = val.pin_type();
                if vt != dt {
                    return graph::PinType::cast_expr(vt, dt, &expr);
                }
            }
            return expr;
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

    /// Emit a triplanar-sampled FBM-family noise. The shared shape:
    ///   - multiply world_position by `scale`
    ///   - power(|world_normal|, sharpness) → blend weights
    ///   - call `fbm_fn(plane_uv, i32(octaves), lacunarity, persistence)` on yz/xz/xy
    ///   - weighted sum → output "value"
    /// `fbm_fn` is the helper name (mat_fbm, mat_fbm_ridged, ...).
    /// `_arity` kept for future variants with different param counts.
    fn emit_triplanar_noise(
        &mut self,
        node: &MaterialNode,
        id: NodeId,
        fbm_fn: &str,
        prefix: &str,
        _arity: usize,
    ) {
        let scale = self.input(node, "scale");
        let octaves = self.input(node, "octaves");
        let lac = self.input(node, "lacunarity");
        let pers = self.input(node, "persistence");
        let sharp = self.input(node, "sharpness");
        let v = self.next_var(prefix);
        self.emit(format!("    let {v}_p = in.world_position.xyz * {scale};"));
        self.emit(format!("    let {v}_wa = pow(abs(in.world_normal), vec3<f32>({sharp}));"));
        self.emit(format!("    let {v}_w = {v}_wa / ({v}_wa.x + {v}_wa.y + {v}_wa.z + 0.000001);"));
        self.emit(format!("    let {v}_x = {fbm_fn}({v}_p.yz, i32({octaves}), {lac}, {pers});"));
        self.emit(format!("    let {v}_y = {fbm_fn}({v}_p.xz, i32({octaves}), {lac}, {pers});"));
        self.emit(format!("    let {v}_z = {fbm_fn}({v}_p.xy, i32({octaves}), {lac}, {pers});"));
        self.emit(format!("    let {v} = {v}_x * {v}_w.x + {v}_y * {v}_w.y + {v}_z * {v}_w.z;"));
        self.set_out(id, "value", v);
    }

    /// Compile a MaterialFunction's internal graph into a standalone WGSL fn
    /// (signature `fn mfunc_<name>(in_0..in_3: vec4<f32>) -> MFuncOut_<name>`).
    /// The function body runs against `mat_fn.graph`, but var_counter,
    /// module_prelude, texture_bindings and uses_* flags remain shared with
    /// the outer Ctx — so helpers, textures and var names stay unique across
    /// the whole shader. Requires `mat_fn: &'a MaterialFunction` so the
    /// function's graph lifetime matches the Ctx's graph lifetime parameter.
    fn compile_function_body(&mut self, mat_fn: &'a MaterialFunction) -> String {
        let ident = safe_fn_ident(&mat_fn.name);

        // Swap outer graph state for the function's local state.
        let saved_graph = std::mem::replace(&mut self.graph, &mat_fn.graph);
        let saved_lines = std::mem::take(&mut self.lines);
        let saved_output_vars = std::mem::take(&mut self.output_vars);
        let saved_processed = std::mem::take(&mut self.processed);

        // Resolve the function's return values by walking the output_point's inputs.
        let (out_0, out_1, out_2, out_3) = match mat_fn.output_point() {
            Some(out_node) => {
                let o = out_node.clone();
                (
                    self.input(&o, "out_0"),
                    self.input(&o, "out_1"),
                    self.input(&o, "out_2"),
                    self.input(&o, "out_3"),
                )
            }
            None => (
                "vec4<f32>(0.0)".to_string(),
                "vec4<f32>(0.0)".to_string(),
                "vec4<f32>(0.0)".to_string(),
                "vec4<f32>(0.0)".to_string(),
            ),
        };

        let body_lines = std::mem::replace(&mut self.lines, saved_lines);

        // Restore outer graph state.
        self.output_vars = saved_output_vars;
        self.processed = saved_processed;
        self.graph = saved_graph;

        // Stitch into a WGSL function.
        let mut s = String::new();
        s.push_str(&format!(
            "\nstruct MFuncOut_{ident} {{\n    out_0: vec4<f32>,\n    out_1: vec4<f32>,\n    out_2: vec4<f32>,\n    out_3: vec4<f32>,\n}};\n\n"
        ));
        s.push_str(&format!(
            "fn mfunc_{ident}(in_0: vec4<f32>, in_1: vec4<f32>, in_2: vec4<f32>, in_3: vec4<f32>) -> MFuncOut_{ident} {{\n"
        ));
        for line in &body_lines {
            s.push_str(line);
            s.push('\n');
        }
        s.push_str(&format!(
            "    return MFuncOut_{ident}({out_0}, {out_1}, {out_2}, {out_3});\n"
        ));
        s.push_str("}\n");
        s
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
                self.emit(format!(
                    "    let {v}_d = {uv} - {center};"
                ));
                self.emit(format!(
                    "    let {v}_angle = fract(atan2({v}_d.y, {v}_d.x) / 6.2831853 + 1.0);"
                ));
                self.emit(format!(
                    "    let {v}_radius = length({v}_d);"
                ));
                self.emit(format!(
                    "    let {v} = vec2<f32>({v}_angle, {v}_radius);"
                ));
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
                self.emit(format!(
                    "    let {v}_d = {uv} - {center};"
                ));
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
                // mesh_functions provides mesh[in.instance_index]
                self.set_out(id, "position", "mesh_functions::get_world_from_local(in.instance_index)[3].xyz".into());
            }

            // ── Textures ────────────────────────────────────────────
            "texture/sample" => {
                // Use in.uv when UV pin is unconnected (most common case)
                let uv = if self.graph.connection_to(node.id, "uv").is_some() {
                    self.input(node, "uv")
                } else {
                    "mat_uv".to_string()
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
                    kind: TextureKind::D2,
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
                    "mat_uv".to_string()
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
                    kind: TextureKind::D2,
                });

                let raw = self.next_var("nraw");
                let n = self.next_var("nmap");
                self.emit(format!("    let {raw} = textureSample({tex_name}, {samp_name}, {uv}).rgb * 2.0 - 1.0;"));
                self.emit(format!("    let {n} = normalize(vec3<f32>({raw}.xy * {strength}, {raw}.z));"));
                self.set_out(id, "normal", n);
            }

            "texture/sample_lod" => {
                let uv = if self.graph.connection_to(node.id, "uv").is_some() {
                    self.input(node, "uv")
                } else {
                    "mat_uv".to_string()
                };
                let lod = self.input(node, "lod");
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
                    kind: TextureKind::D2,
                });

                let v = self.next_var("texl");
                self.emit(format!("    let {v} = textureSampleLevel({tex_name}, {samp_name}, {uv}, {lod});"));
                self.set_out(id, "color", v.clone());
                self.set_out(id, "rgb", format!("{v}.rgb"));
                self.set_out(id, "r", format!("{v}.r"));
                self.set_out(id, "g", format!("{v}.g"));
                self.set_out(id, "b", format!("{v}.b"));
                self.set_out(id, "a", format!("{v}.a"));
            }

            "texture/sample_grad" => {
                let uv = if self.graph.connection_to(node.id, "uv").is_some() {
                    self.input(node, "uv")
                } else {
                    "mat_uv".to_string()
                };
                let ddx = self.input(node, "ddx");
                let ddy = self.input(node, "ddy");
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
                    kind: TextureKind::D2,
                });

                let v = self.next_var("texg");
                self.emit(format!("    let {v} = textureSampleGrad({tex_name}, {samp_name}, {uv}, {ddx}, {ddy});"));
                self.set_out(id, "color", v.clone());
                self.set_out(id, "rgb", format!("{v}.rgb"));
                self.set_out(id, "r", format!("{v}.r"));
                self.set_out(id, "g", format!("{v}.g"));
                self.set_out(id, "b", format!("{v}.b"));
                self.set_out(id, "a", format!("{v}.a"));
            }

            "texture/sample_cubemap" => {
                self.uses_cube_0 = true;
                let dir = self.input(node, "direction");
                let lod = self.input(node, "lod");
                let path = node.input_values.get("texture")
                    .and_then(|v| if let PinValue::TexturePath(s) = v { Some(s.clone()) } else { None })
                    .unwrap_or_default();
                if !path.is_empty() {
                    self.texture_bindings.push(TextureBinding {
                        name: "cube_0".to_string(),
                        binding: 0,
                        asset_path: path,
                        kind: TextureKind::Cube,
                    });
                }
                let v = self.next_var("cubes");
                self.emit(format!("    let {v} = textureSampleLevel(cube_0, cube_0_sampler, normalize({dir}), {lod});"));
                self.set_out(id, "color", v.clone());
                self.set_out(id, "rgb", format!("{v}.rgb"));
                self.set_out(id, "a", format!("{v}.a"));
            }

            "texture/sample_2d_array" => {
                self.uses_array_0 = true;
                let uv = if self.graph.connection_to(node.id, "uv").is_some() {
                    self.input(node, "uv")
                } else {
                    "mat_uv".to_string()
                };
                let layer = self.input(node, "layer");
                let path = node.input_values.get("texture")
                    .and_then(|v| if let PinValue::TexturePath(s) = v { Some(s.clone()) } else { None })
                    .unwrap_or_default();
                if !path.is_empty() {
                    self.texture_bindings.push(TextureBinding {
                        name: "array_0".to_string(),
                        binding: 0,
                        asset_path: path,
                        kind: TextureKind::D2Array,
                    });
                }
                let v = self.next_var("tarr");
                self.emit(format!("    let {v} = textureSample(array_0, array_0_sampler, {uv}, i32(round({layer})));"));
                self.set_out(id, "color", v.clone());
                self.set_out(id, "rgb", format!("{v}.rgb"));
                self.set_out(id, "r", format!("{v}.r"));
                self.set_out(id, "g", format!("{v}.g"));
                self.set_out(id, "b", format!("{v}.b"));
                self.set_out(id, "a", format!("{v}.a"));
            }

            "texture/sample_3d" => {
                self.uses_volume_0 = true;
                let uvw = self.input(node, "uvw");
                let path = node.input_values.get("texture")
                    .and_then(|v| if let PinValue::TexturePath(s) = v { Some(s.clone()) } else { None })
                    .unwrap_or_default();
                if !path.is_empty() {
                    self.texture_bindings.push(TextureBinding {
                        name: "volume_0".to_string(),
                        binding: 0,
                        asset_path: path,
                        kind: TextureKind::D3,
                    });
                }
                let v = self.next_var("t3d");
                self.emit(format!("    let {v} = textureSample(volume_0, volume_0_sampler, {uvw});"));
                self.set_out(id, "color", v.clone());
                self.set_out(id, "rgb", format!("{v}.rgb"));
                self.set_out(id, "r", format!("{v}.r"));
                self.set_out(id, "g", format!("{v}.g"));
                self.set_out(id, "b", format!("{v}.b"));
                self.set_out(id, "a", format!("{v}.a"));
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
                    kind: TextureKind::D2,
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
            "math/modulo" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("mod");
                self.emit(format!("    let {v} = {a} - {b} * floor({a} / max({b}, 0.000001));"));
                self.set_out(id, "result", v);
            }
            "math/sign" => {
                let val = self.input(node, "value");
                let v = self.next_var("sgn");
                self.emit(format!("    let {v} = sign({val});"));
                self.set_out(id, "result", v);
            }
            "math/atan2" => {
                let y = self.input(node, "y");
                let x = self.input(node, "x");
                let v = self.next_var("atan2");
                self.emit(format!("    let {v} = atan2({y}, {x});"));
                self.set_out(id, "result", v);
            }
            "math/trunc" => {
                let val = self.input(node, "value");
                let v = self.next_var("trn");
                self.emit(format!("    let {v} = trunc({val});"));
                self.set_out(id, "result", v);
            }
            "math/round" => {
                let val = self.input(node, "value");
                let v = self.next_var("rnd");
                self.emit(format!("    let {v} = round({val});"));
                self.set_out(id, "result", v);
            }
            "math/exp" => {
                let val = self.input(node, "value");
                let v = self.next_var("exp");
                self.emit(format!("    let {v} = exp({val});"));
                self.set_out(id, "result", v);
            }
            "math/log" => {
                let val = self.input(node, "value");
                let v = self.next_var("log");
                self.emit(format!("    let {v} = log(max({val}, 0.000001));"));
                self.set_out(id, "result", v);
            }
            "math/sqrt" => {
                let val = self.input(node, "value");
                let v = self.next_var("sqrt");
                self.emit(format!("    let {v} = sqrt(max({val}, 0.0));"));
                self.set_out(id, "result", v);
            }
            "math/reciprocal" => {
                let val = self.input(node, "value");
                let v = self.next_var("rcp");
                self.emit(format!("    let {v} = 1.0 / max({val}, 0.000001);"));
                self.set_out(id, "result", v);
            }
            "math/tan" => {
                let val = self.input(node, "value");
                let v = self.next_var("tan");
                self.emit(format!("    let {v} = tan({val});"));
                self.set_out(id, "result", v);
            }
            "math/asin" => {
                let val = self.input(node, "value");
                let v = self.next_var("asin");
                self.emit(format!("    let {v} = asin(clamp({val}, -1.0, 1.0));"));
                self.set_out(id, "result", v);
            }
            "math/acos" => {
                let val = self.input(node, "value");
                let v = self.next_var("acos");
                self.emit(format!("    let {v} = acos(clamp({val}, -1.0, 1.0));"));
                self.set_out(id, "result", v);
            }
            "math/radians" => {
                let val = self.input(node, "value");
                let v = self.next_var("rad");
                self.emit(format!("    let {v} = radians({val});"));
                self.set_out(id, "result", v);
            }
            "math/degrees" => {
                let val = self.input(node, "value");
                let v = self.next_var("deg");
                self.emit(format!("    let {v} = degrees({val});"));
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
            "vector/refract" => {
                let inc = self.input(node, "incident");
                let n = self.input(node, "normal");
                let eta = self.input(node, "eta");
                let v = self.next_var("refr");
                self.emit(format!("    let {v} = refract({inc}, {n}, {eta});"));
                self.set_out(id, "result", v);
            }
            "vector/swizzle" => {
                let vec = self.input(node, "vector");
                let choices = ["out_x", "out_y", "out_z", "out_w"];
                let mut parts = Vec::with_capacity(4);
                for pin in &choices {
                    let sel = node.input_values.get(*pin)
                        .and_then(|v| if let PinValue::Int(i) = v { Some(*i) } else { None })
                        .unwrap_or(match *pin {
                            "out_x" => 0,
                            "out_y" => 1,
                            "out_z" => 2,
                            _ => 3,
                        });
                    parts.push(match sel {
                        0 => format!("({vec}).x"),
                        1 => format!("({vec}).y"),
                        2 => format!("({vec}).z"),
                        3 => format!("({vec}).w"),
                        4 => "0.0".to_string(),
                        5 => "1.0".to_string(),
                        _ => format!("({vec}).x"),
                    });
                }
                let v = self.next_var("swz");
                self.emit(format!(
                    "    let {v} = vec4<f32>({}, {}, {}, {});",
                    parts[0], parts[1], parts[2], parts[3]
                ));
                self.set_out(id, "vector", v);
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
            "color/srgb_to_linear" => {
                self.uses_srgb = true;
                let c = self.input(node, "color");
                let v = self.next_var("s2l");
                self.emit(format!("    let {v} = vec4<f32>(mat_srgb_to_linear(({c}).rgb), ({c}).a);"));
                self.set_out(id, "result", v);
            }
            "color/linear_to_srgb" => {
                self.uses_srgb = true;
                let c = self.input(node, "color");
                let v = self.next_var("l2s");
                self.emit(format!("    let {v} = vec4<f32>(mat_linear_to_srgb(({c}).rgb), ({c}).a);"));
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
                self.emit(format!("    let {v} = dot({rgb}, vec3<f32>(0.2126, 0.7152, 0.0722));"));
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
                let mode = node.input_values.get("mode")
                    .and_then(|v| if let PinValue::Int(i) = v { Some(*i) } else { None })
                    .unwrap_or(0);
                let v = self.next_var("blend");
                self.emit(format!(
                    "    let {v} = mat_blend({base}, {blnd}, {op}, {mode});"
                ));
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
                self.uses_voronoi_full = true;
                self.uses_hash = true;
                let uv = self.input(node, "uv");
                let scale = self.input(node, "scale");
                let v = self.next_var("vor");
                self.emit(format!("    let {v} = mat_voronoi_full({uv} * {scale});"));
                self.set_out(id, "distance", format!("{v}.x"));
                self.set_out(id, "f2",       format!("{v}.y"));
                self.set_out(id, "edge",     format!("{v}.z"));
                self.set_out(id, "cell_id",  format!("{v}.w"));
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
            "procedural/world_normal_from_height" => {
                // Reconstruct a world-space tangent frame per-fragment from the
                // screen-space derivatives of `world_position`, then perturb the
                // world normal by the height gradient in that frame.
                //
                // Based on Christian Schüler's "Normal Mapping Without Precomputed
                // Tangents" trick — works on any surface orientation without
                // requiring mesh-supplied tangents.
                let height = self.input(node, "height");
                let strength = self.input(node, "strength");
                let v = self.next_var("wnfh");
                self.emit(format!("    let {v}_dpdx = dpdx(in.world_position.xyz);"));
                self.emit(format!("    let {v}_dpdy = dpdy(in.world_position.xyz);"));
                self.emit(format!("    let {v}_dhdx = dpdx({height});"));
                self.emit(format!("    let {v}_dhdy = dpdy({height});"));
                self.emit(format!("    let {v}_n0 = normalize(in.world_normal);"));
                self.emit(format!("    let {v}_r1 = cross({v}_dpdy, {v}_n0);"));
                self.emit(format!("    let {v}_r2 = cross({v}_n0, {v}_dpdx);"));
                self.emit(format!(
                    "    let {v}_det = max(dot({v}_dpdx, {v}_r1), 0.0000001);"
                ));
                self.emit(format!(
                    "    let {v}_grad = ({v}_dhdx * {v}_r1 + {v}_dhdy * {v}_r2) / {v}_det;"
                ));
                self.emit(format!(
                    "    let {v} = normalize({v}_n0 - {v}_grad * {strength});"
                ));
                self.set_out(id, "normal", v);
            }
            "procedural/domain_warp" => {
                self.uses_noise = true;
                self.uses_fbm = true;
                let uv = self.input(node, "uv");
                let scale = self.input(node, "scale");
                let strength = self.input(node, "strength");
                let offset = self.input(node, "offset");
                let v = self.next_var("warp");
                self.emit(format!(
                    "    let {v} = {uv} + vec2<f32>(mat_fbm({uv} * {scale}, 3, 2.0, 0.5), mat_fbm(({uv} + {offset}) * {scale}, 3, 2.0, 0.5)) * {strength};"
                ));
                self.set_out(id, "uv", v);
            }
            "procedural/noise_ridged" => {
                self.uses_noise = true;
                self.uses_fbm_ridged = true;
                let uv = self.input(node, "uv");
                let scale = self.input(node, "scale");
                let octaves = self.input(node, "octaves");
                let lac = self.input(node, "lacunarity");
                let pers = self.input(node, "persistence");
                let v = self.next_var("ridged");
                self.emit(format!("    let {v} = mat_fbm_ridged({uv} * {scale}, i32({octaves}), {lac}, {pers});"));
                self.set_out(id, "value", v);
            }
            "procedural/noise_turbulence" => {
                self.uses_noise = true;
                self.uses_fbm_turbulence = true;
                let uv = self.input(node, "uv");
                let scale = self.input(node, "scale");
                let octaves = self.input(node, "octaves");
                let lac = self.input(node, "lacunarity");
                let pers = self.input(node, "persistence");
                let v = self.next_var("turb");
                self.emit(format!("    let {v} = mat_fbm_turbulence({uv} * {scale}, i32({octaves}), {lac}, {pers});"));
                self.set_out(id, "value", v);
            }
            "procedural/noise_billow" => {
                self.uses_noise = true;
                self.uses_fbm_billow = true;
                let uv = self.input(node, "uv");
                let scale = self.input(node, "scale");
                let octaves = self.input(node, "octaves");
                let lac = self.input(node, "lacunarity");
                let pers = self.input(node, "persistence");
                let v = self.next_var("billow");
                self.emit(format!("    let {v} = mat_fbm_billow({uv} * {scale}, i32({octaves}), {lac}, {pers});"));
                self.set_out(id, "value", v);
            }
            "procedural/noise_white" => {
                self.uses_hash = true;
                let uv = self.input(node, "uv");
                let scale = self.input(node, "scale");
                let v = self.next_var("wh");
                self.emit(format!("    let {v} = mat_hash(floor({uv} * {scale}));"));
                self.set_out(id, "value", v);
            }
            "procedural/noise_curl" => {
                self.uses_noise = true;
                self.uses_curl = true;
                let uv = self.input(node, "uv");
                let scale = self.input(node, "scale");
                let eps = self.input(node, "epsilon");
                let v = self.next_var("curl");
                self.emit(format!("    let {v} = mat_curl_noise({uv} * {scale}, {eps});"));
                self.set_out(id, "flow", v);
            }
            "procedural/gradient_radial" => {
                let uv = self.input(node, "uv");
                let center = self.input(node, "center");
                let radius = self.input(node, "radius");
                let soft = self.input(node, "softness");
                let v = self.next_var("grad_r");
                self.emit(format!(
                    "    let {v} = 1.0 - smoothstep({radius} - {soft}, {radius}, length({uv} - {center}));"
                ));
                self.set_out(id, "value", v);
            }
            "procedural/gradient_linear" => {
                let uv = self.input(node, "uv");
                let angle = self.input(node, "angle");
                let center = self.input(node, "center");
                let v = self.next_var("grad_l");
                self.emit(format!(
                    "    let {v} = saturate(dot({uv} - {center}, vec2<f32>(cos({angle}), sin({angle}))) + 0.5);"
                ));
                self.set_out(id, "value", v);
            }
            "procedural/gradient_angular" => {
                let uv = self.input(node, "uv");
                let center = self.input(node, "center");
                let off = self.input(node, "offset");
                let v = self.next_var("grad_a");
                self.emit(format!(
                    "    let {v} = fract((atan2(({uv} - {center}).y, ({uv} - {center}).x) / 6.2831853) + {off});"
                ));
                self.set_out(id, "value", v);
            }
            "procedural/gradient_diamond" => {
                let uv = self.input(node, "uv");
                let center = self.input(node, "center");
                let size = self.input(node, "size");
                let v = self.next_var("grad_d");
                self.emit(format!(
                    "    let {v} = 1.0 - saturate((abs(({uv} - {center}).x) + abs(({uv} - {center}).y)) / max({size}, 0.0001));"
                ));
                self.set_out(id, "value", v);
            }
            "procedural/bump_offset" => {
                let uv = self.input(node, "uv");
                let height = self.input(node, "height");
                let reference = self.input(node, "reference");
                let strength = self.input(node, "strength");
                let v = self.next_var("bump");
                // Simplified: approximate view as tangent-space (0,0,1) and offset by (height-ref)*strength
                // toward fake view direction using UV derivatives.
                self.emit(format!(
                    "    let {v} = {uv} + normalize(vec2<f32>(dpdx({height}), dpdy({height})) + vec2<f32>(0.0001)) * (({height} - {reference}) * {strength});"
                ));
                self.set_out(id, "uv", v);
            }
            "procedural/noise_triplanar_fbm" => {
                self.emit_triplanar_noise(node, id, "mat_fbm", "tri_fbm", /*extra_arg_arity=*/3);
                self.uses_noise = true;
                self.uses_fbm = true;
            }
            "procedural/noise_triplanar_ridged" => {
                self.emit_triplanar_noise(node, id, "mat_fbm_ridged", "tri_ridged", 3);
                self.uses_noise = true;
                self.uses_fbm_ridged = true;
            }
            "procedural/noise_triplanar_turbulence" => {
                self.emit_triplanar_noise(node, id, "mat_fbm_turbulence", "tri_turb", 3);
                self.uses_noise = true;
                self.uses_fbm_turbulence = true;
            }
            "procedural/noise_triplanar_billow" => {
                self.emit_triplanar_noise(node, id, "mat_fbm_billow", "tri_billow", 3);
                self.uses_noise = true;
                self.uses_fbm_billow = true;
            }
            "procedural/noise_triplanar_voronoi" => {
                // Voronoi's full helper returns vec4 (f1, f2, edge, cell_id).
                // We project onto 3 world planes and blend by world normal.
                self.uses_voronoi_full = true;
                self.uses_hash = true;
                let scale = self.input(node, "scale");
                let sharp = self.input(node, "sharpness");
                let v = self.next_var("tri_vor");
                self.emit(format!("    let {v}_p = in.world_position.xyz * {scale};"));
                self.emit(format!("    let {v}_wa = pow(abs(in.world_normal), vec3<f32>({sharp}));"));
                self.emit(format!("    let {v}_w = {v}_wa / ({v}_wa.x + {v}_wa.y + {v}_wa.z + 0.000001);"));
                self.emit(format!("    let {v}_x = mat_voronoi_full({v}_p.yz);"));
                self.emit(format!("    let {v}_y = mat_voronoi_full({v}_p.xz);"));
                self.emit(format!("    let {v}_z = mat_voronoi_full({v}_p.xy);"));
                self.emit(format!("    let {v} = {v}_x * {v}_w.x + {v}_y * {v}_w.y + {v}_z * {v}_w.z;"));
                self.set_out(id, "distance", format!("{v}.x"));
                self.set_out(id, "cell_id", format!("{v}.w"));
            }

            "procedural/hex_tile" => {
                self.uses_hex_tile = true;
                self.uses_hash = true;
                let uv = if self.graph.connection_to(node.id, "uv").is_some() {
                    self.input(node, "uv")
                } else {
                    "mat_uv".to_string()
                };
                let scale = self.input(node, "scale");
                let variation = self.input(node, "variation");
                let v = self.next_var("hex");
                self.emit(format!("    let {v} = mat_hex_tile({uv} * {scale}, {variation});"));
                self.set_out(id, "uv1", format!("{v}.uv_a"));
                self.set_out(id, "uv2", format!("{v}.uv_b"));
                self.set_out(id, "uv3", format!("{v}.uv_c"));
                self.set_out(id, "w1", format!("{v}.w.x"));
                self.set_out(id, "w2", format!("{v}.w.y"));
                self.set_out(id, "w3", format!("{v}.w.z"));
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
                self.emit(format!("    let {v}_col = floor({v}_idx - floor({v}_idx / {v}_cols) * {v}_cols);"));
                self.emit(format!("    let {v}_row = floor({v}_idx / {v}_cols);"));
                self.emit(format!("    let {v}_tile = vec2<f32>(1.0 / {v}_cols, 1.0 / {v}_rows);"));
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

            // ── Control ─────────────────────────────────────────────
            "control/if" => {
                let cond = self.input(node, "condition");
                let thresh = self.input(node, "threshold");
                let a = self.input(node, "if_true");
                let b = self.input(node, "if_false");
                let v = self.next_var("ifn");
                self.emit(format!(
                    "    let {v} = select({b}, {a}, {cond} > {thresh});"
                ));
                self.set_out(id, "result", v);
            }
            "control/static_switch" => {
                // Compile-time: only emit the selected branch. `input()` on the
                // unselected pin is never called, so its upstream subgraph is not
                // walked — that chain stays out of the shader entirely.
                let use_a = node.input_values.get("use_a")
                    .and_then(|v| if let PinValue::Bool(b) = v { Some(*b) } else { None })
                    .unwrap_or(true);
                let selected = if use_a {
                    self.input(node, "a")
                } else {
                    self.input(node, "b")
                };
                self.set_out(id, "result", selected);
            }
            "control/component_mask" => {
                let vec = self.input(node, "vector");
                let get_bool = |name: &str, default: bool| {
                    node.input_values.get(name)
                        .and_then(|v| if let PinValue::Bool(b) = v { Some(*b) } else { None })
                        .unwrap_or(default)
                };
                let kr = if get_bool("keep_r", true)  { format!("({vec}).x") } else { "0.0".to_string() };
                let kg = if get_bool("keep_g", true)  { format!("({vec}).y") } else { "0.0".to_string() };
                let kb = if get_bool("keep_b", true)  { format!("({vec}).z") } else { "0.0".to_string() };
                let ka = if get_bool("keep_a", false) { format!("({vec}).w") } else { "0.0".to_string() };
                let v = self.next_var("mask");
                self.emit(format!(
                    "    let {v} = vec4<f32>({kr}, {kg}, {kb}, {ka});"
                ));
                self.set_out(id, "vector", v);
            }
            "control/greater_than" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("gt");
                self.emit(format!("    let {v} = select(0.0, 1.0, {a} > {b});"));
                self.set_out(id, "result", v);
            }
            "control/less_than" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("lt");
                self.emit(format!("    let {v} = select(0.0, 1.0, {a} < {b});"));
                self.set_out(id, "result", v);
            }
            "control/equal" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let eps = self.input(node, "epsilon");
                let v = self.next_var("eq");
                self.emit(format!(
                    "    let {v} = select(0.0, 1.0, abs({a} - {b}) < {eps});"
                ));
                self.set_out(id, "result", v);
            }
            "control/not_equal" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let eps = self.input(node, "epsilon");
                let v = self.next_var("neq");
                self.emit(format!(
                    "    let {v} = select(0.0, 1.0, abs({a} - {b}) >= {eps});"
                ));
                self.set_out(id, "result", v);
            }
            "control/and" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("and");
                self.emit(format!("    let {v} = min({a}, {b});"));
                self.set_out(id, "result", v);
            }
            "control/or" => {
                let a = self.input(node, "a");
                let b = self.input(node, "b");
                let v = self.next_var("or");
                self.emit(format!("    let {v} = max({a}, {b});"));
                self.set_out(id, "result", v);
            }
            "control/not" => {
                let val = self.input(node, "value");
                let v = self.next_var("not");
                self.emit(format!("    let {v} = 1.0 - {val};"));
                self.set_out(id, "result", v);
            }

            // ── Scene ────────────────────────────────────────────────
            "scene/pixel_depth" => {
                self.uses_scene_depth = true;
                let v = self.next_var("pxdepth");
                self.emit(format!("    let {v} = mat_linearize_depth(in.position.z);"));
                self.set_out(id, "depth", v);
            }
            "scene/scene_depth" => {
                self.uses_scene_depth = true;
                let v = self.next_var("scdepth");
                // Guard the prepass sample — if no DepthPrepass is active, the
                // shader still compiles but returns a "far away" sentinel.
                self.emit(format!("#ifdef DEPTH_PREPASS"));
                self.emit(format!(
                    "    let {v} = mat_linearize_depth(bevy_pbr::prepass_utils::prepass_depth(in.position, 0u));"
                ));
                self.emit(format!("#else"));
                self.emit(format!("    let {v} = 1.0e6;"));
                self.emit(format!("#endif"));
                self.set_out(id, "depth", v);
            }
            "scene/depth_fade" => {
                self.uses_scene_depth = true;
                let distance = self.input(node, "distance");
                let v = self.next_var("sdfade");
                self.emit(format!("#ifdef DEPTH_PREPASS"));
                self.emit(format!(
                    "    let {v}_scene = mat_linearize_depth(bevy_pbr::prepass_utils::prepass_depth(in.position, 0u));"
                ));
                self.emit(format!(
                    "    let {v}_pixel = mat_linearize_depth(in.position.z);"
                ));
                self.emit(format!(
                    "    let {v} = saturate(({v}_scene - {v}_pixel) / max({distance}, 0.0001));"
                ));
                self.emit(format!("#else"));
                self.emit(format!("    let {v} = 1.0;"));
                self.emit(format!("#endif"));
                self.set_out(id, "fade", v);
            }
            "scene/scene_normal" => {
                self.uses_scene_normal = true;
                let v = self.next_var("snrm");
                self.emit(format!("#ifdef NORMAL_PREPASS"));
                self.emit(format!(
                    "    let {v} = bevy_pbr::prepass_utils::prepass_normal(in.position, 0u);"
                ));
                self.emit(format!("#else"));
                self.emit(format!("    let {v} = vec3<f32>(0.0, 1.0, 0.0);"));
                self.emit(format!("#endif"));
                self.set_out(id, "normal", v);
            }
            "scene/motion_vector" => {
                self.uses_motion_vector = true;
                let vel = self.next_var("mv");
                self.emit(format!("#ifdef MOTION_VECTOR_PREPASS"));
                self.emit(format!(
                    "    let {vel} = bevy_pbr::prepass_utils::prepass_motion_vector(in.position, 0u);"
                ));
                self.emit(format!("#else"));
                self.emit(format!("    let {vel} = vec2<f32>(0.0, 0.0);"));
                self.emit(format!("#endif"));
                self.set_out(id, "velocity", vel.clone());
                self.set_out(id, "speed", format!("length({vel})"));
            }
            "scene/refraction_uv_offset" => {
                let n = self.input(node, "normal");
                let s = self.input(node, "strength");
                let v = self.next_var("refuv");
                self.emit(format!(
                    "    let {v} = ({n}).xy * {s};"
                ));
                self.set_out(id, "offset", v);
            }
            "scene/screen_uv" => {
                let v = self.next_var("suv");
                // view.viewport = (x, y, width, height) in physical pixels
                self.emit(format!(
                    "    let {v} = (in.position.xy - view.viewport.xy) / view.viewport.zw;"
                ));
                self.set_out(id, "uv", v);
            }
            "scene/scene_color" => {
                // Samples Bevy's built-in `view_transmission_texture` — the
                // scene color grab that Bevy populates between opaque and
                // transparent phases for its transmission pipeline.
                //
                // IMPORTANT: this texture is only populated when Bevy actually
                // runs a transmissive pass, which it does when there are
                // materials with PBR transmission > 0 in the scene. If none
                // exists, this returns black (or stale previous content).
                // For reliable "sky-in-refraction", use `scene/env_map_sample`
                // instead — that samples the env cubemap directly and works
                // regardless of transmission pipeline state.
                self.uses_transmission = true;
                let uv = self.input(node, "uv");
                let v = self.next_var("scenec");
                self.emit(format!(
                    "    let {v} = textureSample(view_transmission_texture, view_transmission_sampler, {uv});"
                ));
                self.set_out(id, "color", v.clone());
                self.set_out(id, "rgb", format!("{v}.rgb"));
            }
            "scene/env_map_sample" => {
                self.uses_env_map = true;
                let dir = self.input(node, "direction");
                let mip = self.input(node, "mip_level");
                let v = self.next_var("env");
                // Guarded for Bevy's two env-map binding variants. We only
                // emit the sampling code itself; the bindings are imported
                // from bevy_pbr::mesh_view_bindings (which Bevy already
                // binds for every camera with a view bind group).
                self.emit(format!("#ifdef MULTIPLE_LIGHT_PROBES_IN_ARRAY"));
                self.emit(format!(
                    "    let {v} = textureSampleLevel(specular_environment_maps[0], environment_map_sampler, normalize({dir}), {mip});"
                ));
                self.emit(format!("#else"));
                self.emit(format!(
                    "    let {v} = textureSampleLevel(specular_environment_map, environment_map_sampler, normalize({dir}), {mip});"
                ));
                self.emit(format!("#endif"));
                self.set_out(id, "color", v.clone());
                self.set_out(id, "rgb", format!("{v}.rgb"));
            }
            "scene/env_map_reflect" => {
                self.uses_env_map = true;
                let n = self.input(node, "normal");
                let mip = self.input(node, "mip_level");
                let v = self.next_var("envr");
                // view_dir points FROM fragment TO camera; reflect incoming
                // (negated view_dir) around the surface normal to get the
                // outgoing reflection direction.
                self.emit(format!(
                    "    let {v}_vd = normalize(view.world_position.xyz - in.world_position.xyz);"
                ));
                self.emit(format!(
                    "    let {v}_rd = reflect(-{v}_vd, normalize({n}));"
                ));
                self.emit(format!("#ifdef MULTIPLE_LIGHT_PROBES_IN_ARRAY"));
                self.emit(format!(
                    "    let {v} = textureSampleLevel(specular_environment_maps[0], environment_map_sampler, {v}_rd, {mip});"
                ));
                self.emit(format!("#else"));
                self.emit(format!(
                    "    let {v} = textureSampleLevel(specular_environment_map, environment_map_sampler, {v}_rd, {mip});"
                ));
                self.emit(format!("#endif"));
                self.set_out(id, "color", v.clone());
                self.set_out(id, "rgb", format!("{v}.rgb"));
            }

            // ── Functions (subgraphs) ────────────────────────────────
            "function/input_point" => {
                // Only meaningful inside a function compilation — its outputs
                // resolve to the function's parameter names. When encountered
                // in a top-level graph we still set them (harmless) so users
                // can preview a function graph directly.
                self.set_out(id, "in_0", "in_0".to_string());
                self.set_out(id, "in_1", "in_1".to_string());
                self.set_out(id, "in_2", "in_2".to_string());
                self.set_out(id, "in_3", "in_3".to_string());
            }
            "function/output_point" => {
                // Handled specially by compile_function_body.
                // In a top-level graph it's inert (no outputs pins to wire).
            }
            "function/call" => {
                let fn_name = node.input_values.get("function")
                    .and_then(|v| if let PinValue::String(s) = v { Some(s.clone()) } else { None })
                    .unwrap_or_default();

                if fn_name.is_empty() || self.functions.is_none() {
                    // No registry or empty reference — degrade to pass-through.
                    self.lines.push(format!("    // function/call: empty reference (id={id})"));
                    self.set_out(id, "out_0", "vec4<f32>(0.0)".to_string());
                    self.set_out(id, "out_1", "vec4<f32>(0.0)".to_string());
                    self.set_out(id, "out_2", "vec4<f32>(0.0)".to_string());
                    self.set_out(id, "out_3", "vec4<f32>(0.0)".to_string());
                    return;
                }

                // Resolve input expressions up-front (triggers upstream codegen).
                let in0 = self.input(node, "in_0");
                let in1 = self.input(node, "in_1");
                let in2 = self.input(node, "in_2");
                let in3 = self.input(node, "in_3");

                // Emit the function body into module_prelude exactly once.
                if !self.emitted_functions.contains(&fn_name) {
                    if self.compiling_functions.contains(&fn_name) {
                        self.lines.push(format!(
                            "    // function/call: recursive cycle detected for '{fn_name}'"
                        ));
                    } else {
                        let registry = self.functions.unwrap();
                        match registry.get(&fn_name) {
                            Some(mat_fn) => {
                                self.compiling_functions.insert(fn_name.clone());
                                let fn_wgsl = self.compile_function_body(mat_fn);
                                self.compiling_functions.remove(&fn_name);
                                self.module_prelude.push(fn_wgsl);
                                self.emitted_functions.insert(fn_name.clone());
                            }
                            None => {
                                self.lines.push(format!(
                                    "    // function/call: unknown function '{fn_name}'"
                                ));
                                self.set_out(id, "out_0", "vec4<f32>(0.0)".to_string());
                                self.set_out(id, "out_1", "vec4<f32>(0.0)".to_string());
                                self.set_out(id, "out_2", "vec4<f32>(0.0)".to_string());
                                self.set_out(id, "out_3", "vec4<f32>(0.0)".to_string());
                                return;
                            }
                        }
                    }
                }

                let ident = safe_fn_ident(&fn_name);
                let v = self.next_var("fcall");
                self.emit(format!(
                    "    let {v} = mfunc_{ident}({in0}, {in1}, {in2}, {in3});"
                ));
                self.set_out(id, "out_0", format!("{v}.out_0"));
                self.set_out(id, "out_1", format!("{v}.out_1"));
                self.set_out(id, "out_2", format!("{v}.out_2"));
                self.set_out(id, "out_3", format!("{v}.out_3"));
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
    compile_with_functions(graph, None)
}

/// Compile a material graph that may reference subgraph functions.
pub fn compile_with_functions(
    graph: &MaterialGraph,
    functions: Option<&FunctionRegistry>,
) -> CompileResult {
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
                requires_transmission: false,
            };
        }
    };

    // Probe the output node for any transmission usage. This runs BEFORE the
    // codegen walk so the flag is ready in time for the resolver.
    //
    // A transmission pin counts as "used" when either:
    //   * it has a graph connection (the user drives it at runtime), OR
    //   * it has a non-zero input_values override (the user set a constant).
    //
    // Checking for non-zero matters: a user who explicitly sets transmission=0
    // to disable refraction shouldn't pay the transmissive-pass cost.
    let requires_transmission = {
        let oid = output_node.id;
        let positive_override = |pin: &str| -> bool {
            match output_node.input_values.get(pin) {
                Some(PinValue::Float(v)) => *v > 0.0,
                _ => false,
            }
        };
        graph.connection_to(oid, "specular_transmission").is_some()
            || graph.connection_to(oid, "diffuse_transmission").is_some()
            || positive_override("specular_transmission")
            || positive_override("diffuse_transmission")
    };

    let mut ctx = Ctx::new_with_functions(graph, functions);

    // Generate code for all inputs connected to the output node
    let output_pins: Vec<String> = if let Some(def) = nodes::node_def(&output_node.node_type) {
        (def.pins)()
            .iter()
            .filter(|p| p.direction == graph::PinDir::Input)
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
        requires_transmission,
    }
}

// ── Shader builders ─────────────────────────────────────────────────────────

fn noise_helpers(ctx: &Ctx) -> String {
    let mut s = String::new();
    if ctx.uses_noise || ctx.uses_hash {
        s.push_str(r#"
fn mat_hash(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}
"#);
    }
    if ctx.uses_noise {
        s.push_str(r#"
// Random gradient for Perlin-style noise
fn mat_hash_grad(p: vec2<f32>) -> vec2<f32> {
    let k = vec2<f32>(
        dot(p, vec2<f32>(127.1, 311.7)),
        dot(p, vec2<f32>(269.5, 183.3)),
    );
    return fract(sin(k) * 43758.5453) * 2.0 - 1.0;
}

// Gradient (Perlin) noise with C2-continuous quintic interpolation.
// Returns [0, 1]. Much less grid-aligned artifact than value noise.
fn mat_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);

    let g00 = dot(mat_hash_grad(i + vec2<f32>(0.0, 0.0)), f - vec2<f32>(0.0, 0.0));
    let g10 = dot(mat_hash_grad(i + vec2<f32>(1.0, 0.0)), f - vec2<f32>(1.0, 0.0));
    let g01 = dot(mat_hash_grad(i + vec2<f32>(0.0, 1.0)), f - vec2<f32>(0.0, 1.0));
    let g11 = dot(mat_hash_grad(i + vec2<f32>(1.0, 1.0)), f - vec2<f32>(1.0, 1.0));

    return mix(mix(g00, g10, u.x), mix(g01, g11, u.x), u.y) * 0.5 + 0.5;
}
"#);
    }
    if ctx.uses_fbm {
        s.push_str(r#"
// FBM with inter-octave rotation — breaks grid-aligned artifacts of basic noise
fn mat_fbm(uv: vec2<f32>, octaves: i32, lacunarity: f32, persistence: f32) -> f32 {
    var p = uv;
    var value = 0.0;
    var amplitude = 0.5;
    let c = cos(0.77); let sn = sin(0.77);
    let r = mat2x2<f32>(c, sn, -sn, c);
    for (var i = 0; i < octaves; i = i + 1) {
        value = value + mat_noise(p) * amplitude;
        p = r * p * lacunarity + vec2<f32>(37.1, 17.3);
        amplitude = amplitude * persistence;
    }
    return value;
}
"#);
    }
    if ctx.uses_fbm_ridged {
        s.push_str(r#"
fn mat_fbm_ridged(uv: vec2<f32>, octaves: i32, lacunarity: f32, persistence: f32) -> f32 {
    var p = uv;
    var value = 0.0;
    var amplitude = 0.5;
    var total = 0.0;
    let c = cos(1.13); let sn = sin(1.13);
    let r = mat2x2<f32>(c, sn, -sn, c);
    for (var i = 0; i < octaves; i = i + 1) {
        let n = mat_noise(p);
        value = value + (1.0 - abs(n * 2.0 - 1.0)) * amplitude;
        total = total + amplitude;
        p = r * p * lacunarity + vec2<f32>(21.7, 43.9);
        amplitude = amplitude * persistence;
    }
    return value / max(total, 0.000001);
}
"#);
    }
    if ctx.uses_fbm_turbulence {
        s.push_str(r#"
fn mat_fbm_turbulence(uv: vec2<f32>, octaves: i32, lacunarity: f32, persistence: f32) -> f32 {
    var p = uv;
    var value = 0.0;
    var amplitude = 0.5;
    var total = 0.0;
    let c = cos(0.63); let sn = sin(0.63);
    let r = mat2x2<f32>(c, sn, -sn, c);
    for (var i = 0; i < octaves; i = i + 1) {
        value = value + abs(mat_noise(p) * 2.0 - 1.0) * amplitude;
        total = total + amplitude;
        p = r * p * lacunarity + vec2<f32>(53.1, 29.7);
        amplitude = amplitude * persistence;
    }
    return value / max(total, 0.000001);
}
"#);
    }
    if ctx.uses_fbm_billow {
        s.push_str(r#"
fn mat_fbm_billow(uv: vec2<f32>, octaves: i32, lacunarity: f32, persistence: f32) -> f32 {
    var p = uv;
    var value = 0.0;
    var amplitude = 0.5;
    var total = 0.0;
    let c = cos(0.91); let sn = sin(0.91);
    let r = mat2x2<f32>(c, sn, -sn, c);
    for (var i = 0; i < octaves; i = i + 1) {
        let n = abs(mat_noise(p) * 2.0 - 1.0);
        value = value + n * n * amplitude;
        total = total + amplitude;
        p = r * p * lacunarity + vec2<f32>(13.7, 61.1);
        amplitude = amplitude * persistence;
    }
    return value / max(total, 0.000001);
}
"#);
    }
    if ctx.uses_curl {
        s.push_str(r#"
fn mat_curl_noise(uv: vec2<f32>, eps: f32) -> vec2<f32> {
    let e = max(eps, 0.0001);
    let n1 = mat_noise(uv + vec2<f32>(0.0, e));
    let n2 = mat_noise(uv - vec2<f32>(0.0, e));
    let n3 = mat_noise(uv + vec2<f32>(e, 0.0));
    let n4 = mat_noise(uv - vec2<f32>(e, 0.0));
    // 2D curl: (∂n/∂y, -∂n/∂x)
    return vec2<f32>((n1 - n2) / (2.0 * e), -(n3 - n4) / (2.0 * e));
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
    if ctx.uses_voronoi_full {
        s.push_str(r#"
// Extended Voronoi — returns (F1, F2, edge_dist, cell_id).
// Edge distance uses a second pass that compares F1 neighbors as in IQ's article.
fn mat_voronoi_full(p: vec2<f32>) -> vec4<f32> {
    let n = floor(p);
    let f = fract(p);

    // Pass 1: find nearest cell F1
    var f1 = 8.0;
    var f2 = 8.0;
    var nearest = vec2<f32>(0.0);
    var cell = 0.0;
    for (var j = -1; j <= 1; j = j + 1) {
        for (var i = -1; i <= 1; i = i + 1) {
            let g = vec2<f32>(f32(i), f32(j));
            let o = vec2<f32>(mat_hash(n + g), mat_hash(n + g + vec2<f32>(57.0, 113.0)));
            let r = g + o - f;
            let d = dot(r, r);
            if (d < f1) {
                f2 = f1;
                f1 = d;
                nearest = r;
                cell = mat_hash(n + g + vec2<f32>(234.0, 567.0));
            } else if (d < f2) {
                f2 = d;
            }
        }
    }

    // Pass 2: edge distance — minimum of dot((r_i + nearest)/2, normalize(r_i - nearest))
    var edge = 8.0;
    for (var j = -2; j <= 2; j = j + 1) {
        for (var i = -2; i <= 2; i = i + 1) {
            let g = vec2<f32>(f32(i), f32(j));
            let o = vec2<f32>(mat_hash(n + g), mat_hash(n + g + vec2<f32>(57.0, 113.0)));
            let r = g + o - f;
            let diff = r - nearest;
            if (dot(diff, diff) > 0.00001) {
                let e = dot(0.5 * (nearest + r), normalize(diff));
                if (e < edge) { edge = e; }
            }
        }
    }

    return vec4<f32>(sqrt(f1), sqrt(f2), edge, cell);
}
"#);
    }
    if ctx.uses_srgb {
        s.push_str(r#"
fn mat_srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let cutoff = vec3<f32>(0.04045);
    let lo = c / 12.92;
    let hi = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= cutoff);
}

fn mat_linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let cutoff = vec3<f32>(0.0031308);
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return select(hi, lo, c <= cutoff);
}
"#);
    }
    if ctx.uses_hsv {
        s.push_str(r#"
fn mat_rgb_to_hsv(c: vec3<f32>) -> vec3<f32> {
    let K = vec4<f32>(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    let p = select(vec4<f32>(c.bg, K.wz), vec4<f32>(c.gb, K.xy), c.g >= c.b);
    let q = select(vec4<f32>(p.xyw, c.r), vec4<f32>(c.r, p.yzx), c.r >= p.x);
    let d = q.x - min(q.w, q.y);
    let e = 1.0e-10;
    return vec3<f32>(
        abs(q.z + (q.w - q.y) / (6.0 * d + e)),
        d / (q.x + e),
        q.x
    );
}

fn mat_hsv_to_rgb(c: vec3<f32>) -> vec3<f32> {
    let K = vec4<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    let p = abs(fract(vec3<f32>(c.x) + K.xyz) * 6.0 - vec3<f32>(K.w));
    return c.z * mix(vec3<f32>(K.x), clamp(p - vec3<f32>(K.x), vec3<f32>(0.0), vec3<f32>(1.0)), c.y);
}
"#);
    }
    if ctx.uses_scene_depth {
        s.push_str(r#"
fn mat_linearize_depth(ndc_depth: f32) -> f32 {
    let near = view.clip_from_view[3][2];
    let far_factor = view.clip_from_view[2][2];
    return near / (far_factor + ndc_depth);
}
"#);
    }
    if ctx.uses_hex_tile {
        // Hex anti-tiling (Heitz & Neyret 2018 / "Hex Tiling" by Jasper Flick).
        // Decomposes UV space into hex cells, rotates UV inside each cell by a
        // pseudo-random angle keyed to the cell's integer position, and returns
        // three overlapping samples with barycentric weights for the triangle
        // formed by the three nearest hex centers. A consumer samples its
        // texture three times at uv_a/uv_b/uv_c and combines by w.x/w.y/w.z.
        s.push_str(r#"
struct HexTile {
    uv_a: vec2<f32>,
    uv_b: vec2<f32>,
    uv_c: vec2<f32>,
    w: vec3<f32>,
};

fn mat_hex_cell_uv(cell: vec2<f32>, local: vec2<f32>, variation: f32) -> vec2<f32> {
    let ang = mat_hash(cell) * 6.2831853 * variation;
    let cs = vec2<f32>(cos(ang), sin(ang));
    let off = vec2<f32>(mat_hash(cell + vec2<f32>(17.0, 83.0)), mat_hash(cell + vec2<f32>(47.0, 29.0)));
    let r = vec2<f32>(local.x * cs.x - local.y * cs.y, local.x * cs.y + local.y * cs.x);
    return r + off;
}

fn mat_hex_tile(uv: vec2<f32>, variation: f32) -> HexTile {
    // Skew UV into hex-grid axes (flat-topped hex basis).
    let skew = mat2x2<f32>(1.0, 0.0, 0.5, 0.8660254);
    let inv_skew = mat2x2<f32>(1.0, 0.0, -0.5773503, 1.1547005);
    let hex_uv = inv_skew * uv;
    let base = floor(hex_uv);
    let f = fract(hex_uv);

    // Three corners of the unit quad whose barycentric triangle our sample falls into.
    var c1: vec2<f32>;
    var c2: vec2<f32>;
    var c3: vec2<f32>;
    var w: vec3<f32>;
    if (f.x + f.y < 1.0) {
        c1 = base + vec2<f32>(0.0, 0.0);
        c2 = base + vec2<f32>(1.0, 0.0);
        c3 = base + vec2<f32>(0.0, 1.0);
        w = vec3<f32>(1.0 - f.x - f.y, f.x, f.y);
    } else {
        c1 = base + vec2<f32>(1.0, 1.0);
        c2 = base + vec2<f32>(0.0, 1.0);
        c3 = base + vec2<f32>(1.0, 0.0);
        w = vec3<f32>(f.x + f.y - 1.0, 1.0 - f.x, 1.0 - f.y);
    }

    // Local offset of the input point from each hex center, back in world UV space.
    let p = uv;
    let p1 = p - skew * c1;
    let p2 = p - skew * c2;
    let p3 = p - skew * c3;

    var out: HexTile;
    out.uv_a = mat_hex_cell_uv(c1, p1, variation);
    out.uv_b = mat_hex_cell_uv(c2, p2, variation);
    out.uv_c = mat_hex_cell_uv(c3, p3, variation);
    // Gain-corrected weights preserve variance after blending three rotated samples.
    let w2 = w * w;
    let s = w2.x + w2.y + w2.z;
    out.w = w2 / max(s, 0.00001);
    return out;
}
"#);
    }
    if ctx.uses_blend {
        s.push_str(r#"
fn mat_blend(base: vec4<f32>, blnd: vec4<f32>, opacity: f32, mode: i32) -> vec4<f32> {
    let b = base.rgb;
    let t = blnd.rgb;
    var r: vec3<f32>;
    switch mode {
        case 1: { r = b * t; }                                              // multiply
        case 2: { r = vec3<f32>(1.0) - (vec3<f32>(1.0) - b) * (vec3<f32>(1.0) - t); } // screen
        case 3: {                                                            // overlay
            let lt = 2.0 * b * t;
            let gt = vec3<f32>(1.0) - 2.0 * (vec3<f32>(1.0) - b) * (vec3<f32>(1.0) - t);
            r = select(gt, lt, b < vec3<f32>(0.5));
        }
        case 4: { r = b + t; }                                              // add
        case 5: { r = b - t; }                                              // subtract
        case 6: {                                                            // soft-light
            r = (vec3<f32>(1.0) - 2.0 * t) * b * b + 2.0 * t * b;
        }
        case 7: {                                                            // hard-light
            let lt = 2.0 * b * t;
            let gt = vec3<f32>(1.0) - 2.0 * (vec3<f32>(1.0) - b) * (vec3<f32>(1.0) - t);
            r = select(gt, lt, t < vec3<f32>(0.5));
        }
        case 8: { r = abs(b - t); }                                         // difference
        case 9: { r = b / max(t, vec3<f32>(0.000001)); }                    // divide
        default: { r = t; }                                                 // normal
    }
    return vec4<f32>(mix(b, r, opacity), base.a);
}
"#);
    }
    s
}

fn emit_module_prelude(ctx: &Ctx, s: &mut String) {
    for chunk in &ctx.module_prelude {
        s.push_str(chunk);
    }
}

/// WGSL snippet that aliases mesh-conditional VertexOutput fields. Generated
/// graph code references `mat_uv` / `mat_vertex_color` instead of `in.uv` /
/// `in.color` so a mesh without those attributes still compiles — the
/// `#ifdef` falls back to a sane default (zeroed UV, white vertex color).
fn fragment_input_aliases() -> String {
    r#"#ifdef VERTEX_UVS_A
    let mat_uv = in.uv;
#else
    let mat_uv = vec2<f32>(0.0, 0.0);
#endif
#ifdef VERTEX_COLORS
    let mat_vertex_color = in.color;
#else
    let mat_vertex_color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
#endif
"#.to_string()
}

fn texture_bindings_wgsl(ctx: &Ctx) -> String {
    // Extension-material texture slots live at bindings 100..113 in group 3,
    // alongside StandardMaterial's own bindings (which occupy 0..~30). The
    // extension's AsBindGroup (see `SurfaceGraphExt`) declares the same
    // offsets, so the shader and the CPU-side bind group match.
    //
    // Bevy 0.18 merges base-material + extension bindings into a single bind
    // group 3 (`MATERIAL_BIND_GROUP_INDEX`), filtering duplicates. As long as
    // our bindings don't collide with StandardMaterial's, they coexist fine.
    let mut s = String::new();
    // Slots 0..3 live on bindings 100/101..106/107. Slots 4..5 live on 114/115
    // and 116/117 — the cubemap/array/3D slots sit between them at 108-113, so
    // we can't keep the linear `100 + slot*2` formula past slot 3.
    const D2_BINDINGS: [(u32, u32); 6] = [
        (100, 101),
        (102, 103),
        (104, 105),
        (106, 107),
        (114, 115),
        (116, 117),
    ];
    for (slot, (tex_binding, samp_binding)) in D2_BINDINGS.iter().enumerate() {
        s.push_str(&format!(
            "@group(3) @binding({tex_binding}) var texture_{slot}: texture_2d<f32>;\n",
        ));
        s.push_str(&format!(
            "@group(3) @binding({samp_binding}) var texture_{slot}_sampler: sampler;\n",
        ));
    }
    // Cubemap / 2D-array / 3D bindings are only declared when the graph
    // actually samples them — their @binding slots exist on the layout either
    // way (Bevy's fallback image handles that) but emitting unused `var`s
    // would add harmless-but-noisy lines to every shader.
    if ctx.uses_cube_0 {
        s.push_str("@group(3) @binding(108) var cube_0: texture_cube<f32>;\n");
        s.push_str("@group(3) @binding(109) var cube_0_sampler: sampler;\n");
    }
    if ctx.uses_array_0 {
        s.push_str("@group(3) @binding(110) var array_0: texture_2d_array<f32>;\n");
        s.push_str("@group(3) @binding(111) var array_0_sampler: sampler;\n");
    }
    if ctx.uses_volume_0 {
        s.push_str("@group(3) @binding(112) var volume_0: texture_3d<f32>;\n");
        s.push_str("@group(3) @binding(113) var volume_0_sampler: sampler;\n");
    }
    s
}

/// Emit the common import block shared by the Surface and Unlit shader
/// templates. Kept as a helper so both code paths stay in sync when we add /
/// remove imports based on which nodes the graph uses.
fn emit_ext_shader_header(ctx: &Ctx, shader: &mut String) {
    // The extension-hook pattern: import StandardMaterial's PbrInput builder +
    // the full lighting pipeline. This is the seam documented in the Bevy PBR
    // source (`extended_material.rs` + example usage).
    shader.push_str("#import bevy_pbr::pbr_fragment::pbr_input_from_standard_material\n");
    shader.push_str("#import bevy_pbr::pbr_functions\n");
    shader.push_str("#import bevy_pbr::forward_io::{VertexOutput, FragmentOutput}\n");
    shader.push_str("#import bevy_pbr::mesh_view_bindings::{view, globals}\n");

    if ctx.uses_scene_depth || ctx.uses_scene_normal || ctx.uses_motion_vector {
        shader.push_str("#import bevy_pbr::prepass_utils\n");
    }
    if ctx.uses_transmission {
        shader.push_str("#import bevy_pbr::mesh_view_bindings::{view_transmission_texture, view_transmission_sampler}\n");
    }
    if ctx.uses_env_map {
        shader.push_str("#ifdef MULTIPLE_LIGHT_PROBES_IN_ARRAY\n");
        shader.push_str("#import bevy_pbr::mesh_view_bindings::{specular_environment_maps, environment_map_sampler}\n");
        shader.push_str("#else\n");
        shader.push_str("#import bevy_pbr::mesh_view_bindings::{specular_environment_map, environment_map_sampler}\n");
        shader.push_str("#endif\n");
    }
    shader.push_str("\n");
}

/// Emit a Surface-domain PBR shader as a StandardMaterial extension hook.
///
/// The compiler no longer builds a full `@fragment` that manually assembles a
/// `PbrInput`. Instead it emits the extension pattern:
///
///   1. `pbr_input_from_standard_material(in, is_front)` — initialises the
///      PbrInput identically to how StandardMaterial would have, inheriting
///      every feature StandardMaterial supports (clearcoat, anisotropy,
///      transmission, IBL, fog, shadows, tonemapping).
///   2. The graph's compiled body runs next and overrides specific fields of
///      `pbr_input.material` / `pbr_input.N` / etc. based on which output pins
///      the user has either connected OR overridden via input_values.
///   3. `apply_pbr_lighting` + `main_pass_post_lighting_processing` do the rest.
fn build_pbr_shader(ctx: &Ctx, resolved: &HashMap<String, String>, _domain: MaterialDomain) -> String {
    let output_node = ctx.graph.output_node().unwrap();
    let output_id = output_node.id;
    // A pin is considered "set" when the user either connected a graph to it
    // OR explicitly set an input_values override in the node's serialized
    // data. Disconnected + un-overridden pins let StandardMaterial's own
    // defaults flow through unchanged.
    let is_connected = |pin: &str| {
        ctx.graph.connection_to(output_id, pin).is_some()
            || output_node.input_values.contains_key(pin)
    };

    let mut shader = String::new();
    emit_ext_shader_header(ctx, &mut shader);
    shader.push_str(&texture_bindings_wgsl(ctx));
    shader.push_str(&noise_helpers(ctx));
    emit_module_prelude(ctx, &mut shader);

    shader.push_str("\n@fragment\n");
    shader.push_str("fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {\n");
    shader.push_str("    var pbr_input = pbr_input_from_standard_material(in, is_front);\n");
    // Alias mesh-conditional VertexOutput fields so generated graph code can
    // reference them unconditionally. Bevy's pipeline specialization defines
    // `VERTEX_UVS_A` / `VERTEX_COLORS` based on the actual mesh attributes;
    // meshes without those attributes don't get the corresponding fields, so
    // referencing `in.uv` directly would fail to compile for them.
    shader.push_str(&fragment_input_aliases());

    // Graph body — runs between the StandardMaterial init and the mutations.
    for line in &ctx.lines {
        shader.push_str(line);
        shader.push('\n');
    }

    // Override pbr_input fields for each pin the user wired up. Disconnected
    // pins leave StandardMaterial's defaults in place, so authors can partially
    // override (e.g. only procedural roughness, keeping base_color from the
    // StandardMaterial's texture).
    shader.push_str("\n    // Graph → PbrInput mutations\n");
    if is_connected("base_color") {
        let e = resolved.get("base_color").unwrap();
        shader.push_str(&format!("    pbr_input.material.base_color = {e};\n"));
    }
    if is_connected("metallic") {
        let e = resolved.get("metallic").unwrap();
        shader.push_str(&format!("    pbr_input.material.metallic = {e};\n"));
    }
    if is_connected("roughness") {
        let e = resolved.get("roughness").unwrap();
        shader.push_str(&format!("    pbr_input.material.perceptual_roughness = {e};\n"));
    }
    if is_connected("emissive") {
        let e = resolved.get("emissive").unwrap();
        shader.push_str(&format!("    pbr_input.material.emissive = vec4<f32>({e}, 1.0);\n"));
    }
    if is_connected("ao") {
        let e = resolved.get("ao").unwrap();
        shader.push_str(&format!("    pbr_input.diffuse_occlusion = vec3<f32>({e});\n"));
    }
    if is_connected("normal") {
        let e = resolved.get("normal").unwrap();
        shader.push_str(&format!("    pbr_input.N = normalize({e});\n"));
        shader.push_str(&format!("    pbr_input.world_normal = pbr_input.N;\n"));
    }
    if is_connected("alpha") {
        let e = resolved.get("alpha").unwrap();
        shader.push_str(&format!("    pbr_input.material.base_color.a = {e};\n"));
    }
    if is_connected("reflectance") {
        let e = resolved.get("reflectance").unwrap();
        shader.push_str(&format!("    pbr_input.material.reflectance = {e};\n"));
    }

    // ── Transmission (water, glass, ice) ──────────────────────────────
    // `specular_transmission > 0` on the CPU-side StandardMaterial is what
    // triggers Bevy to schedule its transmissive pass. The resolver takes
    // care of setting the CPU-side flag (see `requires_transmission`).
    if is_connected("specular_transmission") {
        let e = resolved.get("specular_transmission").unwrap();
        shader.push_str(&format!("    pbr_input.material.specular_transmission = {e};\n"));
    }
    if is_connected("diffuse_transmission") {
        let e = resolved.get("diffuse_transmission").unwrap();
        shader.push_str(&format!("    pbr_input.material.diffuse_transmission = {e};\n"));
    }
    if is_connected("thickness") {
        let e = resolved.get("thickness").unwrap();
        shader.push_str(&format!("    pbr_input.material.thickness = {e};\n"));
    }
    if is_connected("ior") {
        let e = resolved.get("ior").unwrap();
        shader.push_str(&format!("    pbr_input.material.ior = {e};\n"));
    }
    if is_connected("attenuation_distance") {
        let e = resolved.get("attenuation_distance").unwrap();
        shader.push_str(&format!("    pbr_input.material.attenuation_distance = {e};\n"));
    }

    // ── Clearcoat (car paint, lacquer) ────────────────────────────────
    if is_connected("clearcoat") {
        let e = resolved.get("clearcoat").unwrap();
        shader.push_str(&format!("    pbr_input.material.clearcoat = {e};\n"));
    }
    if is_connected("clearcoat_roughness") {
        let e = resolved.get("clearcoat_roughness").unwrap();
        shader.push_str(&format!("    pbr_input.material.clearcoat_perceptual_roughness = {e};\n"));
    }

    // ── Anisotropy (brushed metal, hair) ──────────────────────────────
    // WGSL expects `anisotropy_rotation` as a vec2<cos, sin>. Our graph pin
    // takes the rotation angle as a scalar (radians), so we build the vec2.
    if is_connected("anisotropy_strength") {
        let e = resolved.get("anisotropy_strength").unwrap();
        shader.push_str(&format!("    pbr_input.material.anisotropy_strength = {e};\n"));
    }
    if is_connected("anisotropy_rotation") {
        let e = resolved.get("anisotropy_rotation").unwrap();
        shader.push_str(&format!(
            "    pbr_input.material.anisotropy_rotation = vec2<f32>(cos({e}), sin({e}));\n"
        ));
    }

    // Run alpha_discard before lighting — this is what bevy_pbr::pbr.wgsl
    // does. For OPAQUE materials it forces base_color.a = 1.0; for MASK it
    // either clamps to 1.0 or `discard`s. Skipping it leaves emissive
    // unscaled by alpha — `apply_pbr_lighting` does `emissive_light =
    // emissive.rgb * output_color.a`, so a glTF material authored with
    // baseColorFactor.a = 0 (common for emissive-only string lights) would
    // otherwise render with no glow.
    shader.push_str("    pbr_input.material.base_color = pbr_functions::alpha_discard(pbr_input.material, pbr_input.material.base_color);\n");

    shader.push_str("\n    var out: FragmentOutput;\n");
    shader.push_str("    out.color = pbr_functions::apply_pbr_lighting(pbr_input);\n");
    shader.push_str("    out.color = pbr_functions::main_pass_post_lighting_processing(pbr_input, out.color);\n");
    shader.push_str("    return out;\n");
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
    shader.push_str("#import bevy_pbr::mesh_view_bindings::globals\n\n");
    shader.push_str(&texture_bindings_wgsl(ctx));
    shader.push_str(&noise_helpers(ctx));
    emit_module_prelude(ctx, &mut shader);

    // layer_main: returns base color
    shader.push_str("\nfn layer_main(uv: vec2<f32>, world_pos: vec3<f32>, world_normal: vec3<f32>, time: f32) -> vec4<f32> {\n");
    shader.push_str("    // Alias inputs for compatibility\n");
    shader.push_str("    struct FakeIn { uv: vec2<f32>, world_position: vec4<f32>, world_normal: vec3<f32> };\n");
    shader.push_str("    let in = FakeIn(uv, vec4<f32>(world_pos, 1.0), world_normal);\n");
    // Terrain has explicit UV; vertex_color isn't meaningful here so use white.
    shader.push_str("    let mat_uv = uv;\n");
    shader.push_str("    let mat_vertex_color = vec4<f32>(1.0, 1.0, 1.0, 1.0);\n");
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

/// Unlit domain uses the same extension-hook skeleton as Surface. The key
/// difference is the resolver flips `StandardMaterial.unlit = true` on the
/// base — that makes `apply_pbr_lighting` return `base_color` unchanged,
/// skipping diffuse / specular / IBL. The graph's "color" pin becomes the
/// material's base_color; "alpha" drives alpha.
fn build_unlit_shader(ctx: &Ctx, resolved: &HashMap<String, String>) -> String {
    let output_node = ctx.graph.output_node().unwrap();
    let output_id = output_node.id;
    let pin_set = |pin: &str| {
        ctx.graph.connection_to(output_id, pin).is_some()
            || output_node.input_values.contains_key(pin)
    };
    let color_connected = pin_set("color");
    let alpha_connected = pin_set("alpha");

    let mut shader = String::new();
    emit_ext_shader_header(ctx, &mut shader);
    shader.push_str(&texture_bindings_wgsl(ctx));
    shader.push_str(&noise_helpers(ctx));
    emit_module_prelude(ctx, &mut shader);

    shader.push_str("\n@fragment\n");
    shader.push_str("fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {\n");
    shader.push_str("    var pbr_input = pbr_input_from_standard_material(in, is_front);\n");
    shader.push_str(&fragment_input_aliases());

    for line in &ctx.lines {
        shader.push_str(line);
        shader.push('\n');
    }

    // Unlit "color" pin drives the StandardMaterial base_color. Because the
    // base has `unlit = true`, `apply_pbr_lighting` returns this value
    // unmodified (no lighting math applied) — the fastest path for HUD /
    // debug viz / stylised materials.
    if color_connected {
        let e = resolved.get("color").unwrap();
        shader.push_str(&format!("    pbr_input.material.base_color = {e};\n"));
    }
    if alpha_connected {
        let e = resolved.get("alpha").unwrap();
        shader.push_str(&format!("    pbr_input.material.base_color.a = {e};\n"));
    }

    // Match bevy_pbr::pbr.wgsl — alpha_discard handles OPAQUE/MASK before lighting.
    shader.push_str("    pbr_input.material.base_color = pbr_functions::alpha_discard(pbr_input.material, pbr_input.material.base_color);\n");

    shader.push_str("\n    var out: FragmentOutput;\n");
    shader.push_str("    out.color = pbr_functions::apply_pbr_lighting(pbr_input);\n");
    shader.push_str("    out.color = pbr_functions::main_pass_post_lighting_processing(pbr_input, out.color);\n");
    shader.push_str("    return out;\n");
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
    // Both `Vertex.uv` and `VertexOutput.uv` are gated on `VERTEX_UVS_A` —
    // omit the assignment when the mesh has no UV attribute.
    shader.push_str("#ifdef VERTEX_UVS_A\n");
    shader.push_str("    out.uv = in.uv;\n");
    shader.push_str("#endif\n");
    shader.push_str("    return out;\n");
    shader.push_str("}\n");

    shader
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use graph::*;

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
