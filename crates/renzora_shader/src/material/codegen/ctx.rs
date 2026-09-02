//! The codegen context — the mutable state a graph walk accumulates, and the
//! primitives every node emitter is written against.
//!
//! `Ctx` is deliberately one big struct rather than several: a node emitter
//! needs to allocate a variable, resolve an upstream pin, flag a WGSL helper
//! as used, and record which node authored which line, all in one call. The
//! `uses_*` flags are how the prelude stays minimal — a helper is emitted only
//! if some node actually reached for it.
//!
//! Its fields are `pub(crate)` because the node emitters live in
//! [`super::nodes`]; they are not a public API.

use super::super::graph::{
    self, MaterialFunction, MaterialGraph, MaterialNode, NodeId,
};
use super::super::nodes;
use super::{safe_fn_ident, FunctionRegistry, MaterialParam, ParamKind, TextureBinding,
    MAX_PARAMETER_SLOTS};
use std::collections::{HashMap, HashSet};

pub(crate) struct Ctx<'a> {
    pub(crate) graph: &'a MaterialGraph,
    /// Maps (node_id, pin_name) → WGSL variable expression.
    pub(crate) output_vars: HashMap<(NodeId, String), String>,
    pub(crate) var_counter: usize,
    pub(crate) processed: HashSet<NodeId>,
    pub(crate) texture_bindings: Vec<TextureBinding>,
    pub(crate) next_texture_binding: u32,
    pub(crate) lines: Vec<String>,
    /// WGSL declarations emitted at module scope (structs, helper fns).
    pub(crate) module_prelude: Vec<String>,
    /// Registry of available material functions (subgraphs).
    pub(crate) functions: Option<&'a FunctionRegistry>,
    /// Names of functions whose WGSL has already been emitted into module_prelude
    /// so multiple calls to the same function share a single definition.
    pub(crate) emitted_functions: HashSet<String>,
    /// Names of functions currently being compiled — for cycle detection.
    pub(crate) compiling_functions: HashSet<String>,
    pub(crate) uses_noise: bool,
    pub(crate) uses_voronoi: bool,
    pub(crate) uses_voronoi_full: bool,
    pub(crate) uses_fbm: bool,
    pub(crate) uses_fbm_ridged: bool,
    pub(crate) uses_fbm_turbulence: bool,
    pub(crate) uses_fbm_billow: bool,
    pub(crate) uses_curl: bool,
    pub(crate) uses_hash: bool,
    pub(crate) uses_hsv: bool,
    pub(crate) uses_srgb: bool,
    pub(crate) uses_blend: bool,
    pub(crate) uses_scene_depth: bool,
    pub(crate) uses_scene_normal: bool,
    pub(crate) uses_motion_vector: bool,
    pub(crate) uses_transmission: bool,
    pub(crate) uses_env_map: bool,
    pub(crate) uses_hex_tile: bool,
    pub(crate) uses_cube_0: bool,
    pub(crate) uses_array_0: bool,
    pub(crate) uses_volume_0: bool,
    /// Set once the graph's `displacement` pin has been compiled into a
    /// `graph_displacement` helper — the signal for `build_pbr_shader` to
    /// emit the parallax march that rebinds `mat_uv`.
    pub(crate) uses_parallax: bool,
    /// True while emitting the body of `graph_displacement`. Texture reads
    /// switch to `textureSampleLevel` in that window: the helper is called
    /// from a variable-length loop, and the wgpu DX12 (FXC) backend refuses
    /// to compile gradient instructions inside one — the same reason Bevy's
    /// own `sample_depth_map` is level-sampled.
    pub(crate) in_displacement_fn: bool,
    /// Named parameters discovered while walking the graph. The `Vec`'s
    /// position is the slot index in `material_params.slots[N]` — codegen
    /// emits reads keyed on that index, and the resolver writes the
    /// corresponding default (or instance override) into the same slot.
    /// Names are deduped: two `param/float` nodes with the same name share
    /// one slot so changing one override updates every reader.
    pub(crate) parameters: Vec<MaterialParam>,
    /// Name → slot index, mirroring `parameters` ordering. Lets the codegen
    /// look up an already-allocated slot in O(1) when the same parameter
    /// name appears on multiple nodes.
    pub(crate) parameter_slots: HashMap<String, usize>,
    pub(crate) warnings: Vec<String>,
    /// Node whose lines are currently being emitted. Save/restored around
    /// `gen_node` — `input()` recurses upstream and would otherwise leave a
    /// node's own lines attributed to whatever it last pulled from.
    pub(crate) current_node: NodeId,
    /// `(node, 0-based index into `lines`)` for every emitted body line.
    pub(crate) body_spans: Vec<(NodeId, u32)>,
    /// `(node, 0-based start line in the concatenated prelude, line count)`.
    pub(crate) prelude_spans: Vec<(NodeId, u32, u32)>,
    pub(crate) prelude_line_count: u32,
    /// Latched vectorization rank per `math/*` node (see
    /// [`graph::resolve_math_ranks`]). Swapped alongside `graph` when a
    /// material function's internal graph is compiled.
    pub(crate) math_ranks: HashMap<NodeId, graph::PinType>,
}

impl<'a> Ctx<'a> {
    // kept for API symmetry with new_with_functions / future callers
    #[allow(dead_code)]
    pub(crate) fn new(graph: &'a MaterialGraph) -> Self {
        Self::new_with_functions(graph, None)
    }

    pub(crate) fn new_with_functions(
        graph: &'a MaterialGraph,
        functions: Option<&'a FunctionRegistry>,
    ) -> Self {
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
            uses_parallax: false,
            in_displacement_fn: false,
            parameters: Vec::new(),
            parameter_slots: HashMap::new(),
            warnings: Vec::new(),
            current_node: 0,
            body_spans: Vec::new(),
            prelude_spans: Vec::new(),
            prelude_line_count: 0,
            math_ranks: graph::resolve_math_ranks(graph),
        }
    }

    /// Allocate (or reuse) a parameter slot by name. The first call for a
    /// given name appends to `parameters`; subsequent calls return the
    /// existing slot. Saturates at the last slot if a graph exceeds the
    /// uniform buffer's capacity — every read past the cap collides on
    /// slot N-1, which is wrong but won't UB-trap the GPU. The compile
    /// emits a warning so the user can split the master.
    pub(crate) fn intern_parameter(
        &mut self,
        name: &str,
        kind: ParamKind,
        default: graph::PinValue,
    ) -> usize {
        if let Some(&slot) = self.parameter_slots.get(name) {
            return slot;
        }
        let slot = self.parameters.len();
        if slot >= MAX_PARAMETER_SLOTS {
            // Once we hit the cap, every subsequent unique name aliases the
            // last slot. We still record the parameter so tooling can list
            // it, but the actual reads will collide.
            self.warnings.push(format!(
                "parameter '{name}' exceeds the {MAX_PARAMETER_SLOTS}-slot parameter buffer; \
                 it aliases slot {} and will read as '{}'. Split the material or reuse names.",
                MAX_PARAMETER_SLOTS - 1,
                self.parameters
                    .last()
                    .map(|p| p.name.as_str())
                    .unwrap_or("?"),
            ));
            return MAX_PARAMETER_SLOTS - 1;
        }
        self.parameters.push(MaterialParam {
            name: name.to_string(),
            kind,
            default,
        });
        self.parameter_slots.insert(name.to_string(), slot);
        slot
    }

    pub(crate) fn next_var(&mut self, prefix: &str) -> String {
        let name = format!("{}_{}", prefix, self.var_counter);
        self.var_counter += 1;
        name
    }

    pub(crate) fn set_out(&mut self, node: NodeId, pin: &str, expr: String) {
        self.output_vars.insert((node, pin.to_string()), expr);
    }

    /// Look up the PinType a pin behaves as — the latched vectorization rank
    /// for a dynamic math pin, the declared template type for the rest.
    pub(crate) fn pin_type_for(
        &self,
        node: &MaterialNode,
        pin_name: &str,
        direction: graph::PinDir,
    ) -> Option<graph::PinType> {
        graph::resolved_pin_type(&self.math_ranks, node, pin_name, direction)
    }

    /// The math node's latched rank (Float when nothing is wired to it).
    /// Every `math/*` node names its output pin `result`.
    pub(crate) fn math_rank(&self, node: &MaterialNode) -> graph::PinType {
        self.pin_type_for(node, "result", graph::PinDir::Output)
            .unwrap_or(graph::PinType::Float)
    }

    /// Resolve an input pin value — follows connections or falls back to defaults.
    /// Applies automatic type coercion (e.g. Float → Vec4) when pin types differ.
    pub(crate) fn input(&mut self, node: &MaterialNode, pin_name: &str) -> String {
        // Determine expected type of destination pin
        let dest_type = self.pin_type_for(node, pin_name, graph::PinDir::Input);

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
            if let Some(expr) = self
                .output_vars
                .get(&(from_node, from_pin.clone()))
                .cloned()
            {
                // Apply type coercion if source and dest types differ
                if let (Some(dt), Some(src_node)) = (dest_type, self.graph.get_node(from_node)) {
                    if let Some(st) = self.pin_type_for(src_node, &from_pin, graph::PinDir::Output)
                    {
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
                let expr = pin.default_value.to_wgsl();
                // A pin with no default falls back to a plain `0.0`, so a Vec3
                // pin gets a float unless we widen it. The cast target is the
                // *resolved* type, not the template's: a dynamic math pin
                // declares Float even after the node latched wider, and an
                // unwired `b` on a Vec4 Add would otherwise compose
                // `vec4 + f32`.
                let vt = pin.default_value.pin_type();
                let dt = dest_type.unwrap_or(pin.pin_type);
                if vt != dt {
                    return graph::PinType::cast_expr(vt, dt, &expr);
                }
                return expr;
            }
        }

        "0.0".to_string()
    }

    /// A scalar guard literal at a math node's latched rank: plain for Float,
    /// a splat `vecN<f32>(lit)` constructor for vectors. Deliberately not
    /// `cast_expr(Float, rank, …)` — its Vec4 widening fills w with `1.0`,
    /// which would clamp the guard's last component against 1.0 instead of
    /// the epsilon and change what the guard protects.
    pub(crate) fn guard_lit(t: graph::PinType, lit: &str) -> String {
        match t {
            graph::PinType::Float => lit.to_string(),
            other => format!("{}({lit})", other.wgsl_type()),
        }
    }

    pub(crate) fn emit(&mut self, line: String) {
        self.body_spans.push((self.current_node, self.lines.len() as u32));
        self.lines.push(line);
    }

    /// Push a module-scope chunk, recording which node authored it and how
    /// many lines it adds. `module_prelude` bypasses `emit`, and it is where
    /// `custom/code` snippets live — the lines users most often break.
    pub(crate) fn emit_prelude(&mut self, chunk: String) {
        let lines =
            chunk.matches('\n').count() as u32 + u32::from(!chunk.ends_with('\n'));
        self.prelude_spans
            .push((self.current_node, self.prelude_line_count, lines));
        self.prelude_line_count += lines;
        self.module_prelude.push(chunk);
    }

    /// Emit a triplanar-sampled FBM-family noise. The shared shape:
    ///   - multiply world_position by `scale`
    ///   - power(|world_normal|, sharpness) → blend weights
    ///   - call `fbm_fn(plane_uv, i32(octaves), lacunarity, persistence)` on yz/xz/xy
    ///   - weighted sum → output "value"
    ///
    /// `fbm_fn` is the helper name (mat_fbm, mat_fbm_ridged, ...).
    /// `_arity` kept for future variants with different param counts.
    pub(crate) fn emit_triplanar_noise(
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
        self.emit(format!(
            "    let {v}_wa = pow(abs(in.world_normal), vec3<f32>({sharp}));"
        ));
        self.emit(format!(
            "    let {v}_w = {v}_wa / ({v}_wa.x + {v}_wa.y + {v}_wa.z + 0.000001);"
        ));
        self.emit(format!(
            "    let {v}_x = {fbm_fn}({v}_p.yz, i32({octaves}), {lac}, {pers});"
        ));
        self.emit(format!(
            "    let {v}_y = {fbm_fn}({v}_p.xz, i32({octaves}), {lac}, {pers});"
        ));
        self.emit(format!(
            "    let {v}_z = {fbm_fn}({v}_p.xy, i32({octaves}), {lac}, {pers});"
        ));
        self.emit(format!(
            "    let {v} = {v}_x * {v}_w.x + {v}_y * {v}_w.y + {v}_z * {v}_w.z;"
        ));
        self.set_out(id, "value", v);
    }

    /// A 2D texture read, level-sampled while compiling `graph_displacement`.
    ///
    /// See [`Ctx::in_displacement_fn`] — inside the parallax loop the mip
    /// level has to be supplied rather than derived, or DX12 fails to build
    /// the pipeline. Everywhere else the derivative-based sample is what we
    /// want, so this is a single call site's worth of branching rather than
    /// a global change of sampling strategy.
    pub(crate) fn sample_call(&self, tex_name: &str, uv: &str) -> String {
        if self.in_displacement_fn {
            format!("textureSampleLevel({tex_name}, texture_sampler, {uv}, 0.0)")
        } else {
            format!("textureSample({tex_name}, texture_sampler, {uv})")
        }
    }

    /// Compile the output node's `displacement` subgraph into
    /// `fn graph_displacement(in: VertexOutput, mat_uv: vec2<f32>) -> f32`.
    ///
    /// Parallax needs a *function* of UV, not a value: the march evaluates the
    /// height at a dozen different UVs along the view ray. Resolving the pin
    /// inline the way every other pin is resolved would give one height at one
    /// UV, which is no relief at all. Compiling it standalone also breaks what
    /// would otherwise be a cycle — every sampler wants the parallaxed UV, and
    /// the parallaxed UV wants a height sample.
    ///
    /// `mat_uv` is the function's parameter, so any node that reads it (texture
    /// samples, UV-driven noise) is re-evaluated at the marched UV for free.
    /// `in` is passed through so world-position and vertex-attribute nodes keep
    /// working inside the helper.
    ///
    /// Line/var state is swapped exactly the way `compile_function_body` does
    /// it, so nothing memoized in here leaks into the main body — a texture
    /// node shared with `base_color` must be emitted again out there, against
    /// the parallaxed UV rather than the raw one.
    pub(crate) fn compile_displacement_fn(&mut self, output_node: &MaterialNode) -> String {
        let saved_lines = std::mem::take(&mut self.lines);
        let saved_output_vars = std::mem::take(&mut self.output_vars);
        let saved_processed = std::mem::take(&mut self.processed);
        self.in_displacement_fn = true;

        let height = self.input(output_node, "displacement");

        self.in_displacement_fn = false;
        let body_lines = std::mem::replace(&mut self.lines, saved_lines);
        self.output_vars = saved_output_vars;
        self.processed = saved_processed;

        let mut s = String::new();
        s.push_str(
            "\nfn graph_displacement(in: VertexOutput, mat_uv: vec2<f32>) -> f32 {\n\
             #ifdef VERTEX_COLORS\n    let mat_vertex_color = in.color;\n\
             #else\n    let mat_vertex_color = vec4<f32>(1.0, 1.0, 1.0, 1.0);\n#endif\n",
        );
        for line in &body_lines {
            s.push_str(line);
            s.push('\n');
        }
        s.push_str(&format!("    return {height};\n}}\n"));
        s
    }

    /// Compile a MaterialFunction's internal graph into a standalone WGSL fn
    /// (signature `fn mfunc_<name>(in_0..in_3: vec4<f32>) -> MFuncOut_<name>`).
    /// The function body runs against `mat_fn.graph`, but var_counter,
    /// module_prelude, texture_bindings and uses_* flags remain shared with
    /// the outer Ctx — so helpers, textures and var names stay unique across
    /// the whole shader. Requires `mat_fn: &'a MaterialFunction` so the
    /// function's graph lifetime matches the Ctx's graph lifetime parameter.
    pub(crate) fn compile_function_body(&mut self, mat_fn: &'a MaterialFunction) -> String {
        let ident = safe_fn_ident(&mat_fn.name);

        // Swap outer graph state for the function's local state.
        let saved_graph = std::mem::replace(&mut self.graph, &mat_fn.graph);
        let saved_ranks =
            std::mem::replace(&mut self.math_ranks, graph::resolve_math_ranks(&mat_fn.graph));
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
        self.math_ranks = saved_ranks;

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

    pub(crate) fn gen_node(&mut self, node: &MaterialNode) {
        if self.processed.contains(&node.id) {
            return;
        }
        self.processed.insert(node.id);
        let prev_node = std::mem::replace(&mut self.current_node, node.id);
        self.gen_node_body(node);
        self.current_node = prev_node;
    }

    /// Dispatch a node to the emitter for its category.
    ///
    /// Node types are `"<category>/<name>"` and the categories are disjoint,
    /// so splitting on the first `/` picks exactly one emitter — this used to
    /// be a single ~1,970-line `match` over every node type in the language.
    /// Each emitter keeps its own unknown-type arm, so a bad name inside a
    /// known category still produces the same `// Unknown node type` comment
    /// it always did.
    fn gen_node_body(&mut self, node: &MaterialNode) {
        let id = node.id;
        let ty = node.node_type.as_str();
        match ty.split('/').next().unwrap_or("") {
            "input" => self.gen_input_node(node, id),
            "param" => self.gen_param_node(node, id),
            "texture" => self.gen_texture_node(node, id),
            "math" => self.gen_math_node(node, id),
            "vector" => self.gen_vector_node(node, id),
            "color" => self.gen_color_node(node, id),
            "procedural" => self.gen_procedural_node(node, id),
            "animation" => self.gen_animation_node(node, id),
            "utility" => self.gen_utility_node(node, id),
            "custom" => self.gen_custom_node(node, id),
            "control" => self.gen_control_node(node, id),
            "scene" => self.gen_scene_node(node, id),
            "function" => self.gen_function_node(node, id),
            // Output nodes are handled in compile(), not here
            "output" => {}
            _ => self.unknown_node(ty),
        }
    }

    /// The fallthrough every category emitter shares.
    pub(crate) fn unknown_node(&mut self, ty: &str) {
        self.lines.push(format!("    // Unknown node type: {ty}"));
    }
}
