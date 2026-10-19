//! WGSL code generation from material graphs.
//!
//! Walks the graph from the output node backwards, generating WGSL code for
//! each node encountered. Produces a complete Bevy-compatible material shader.
//!
//! - [`ctx`] holds the walk's accumulated state and the primitives every node
//!   emitter is written against
//! - [`emit`] holds the emitters themselves, one module per node category
//! - [`wgsl`] is the library of helper functions and binding declarations they
//!   call by name
//! - [`shaders`] assembles the finished shader, one builder per material domain
//!
//! This file keeps the public seam: the result types, the two `compile`
//! entry points, and the line→node lookup that turns a shader compile error
//! into a pointer at the node that authored it.

use super::graph::{
    self, MaterialDomain, MaterialFunction, MaterialGraph, MaterialNode, PinValue,
};
use super::nodes;
use std::collections::HashMap;

mod ctx;
mod emit;
mod shaders;
mod wgsl;

use ctx::Ctx;
use shaders::{
    build_pbr_shader, build_sprite_shader, build_terrain_layer_shader, build_unlit_shader,
    build_vegetation_vertex_shader,
};
use wgsl::parallax_helper_wgsl;

/// Registry of loaded material functions, keyed by function name.
/// Populated from disk by the resolver and passed to `compile()`.
pub type FunctionRegistry = HashMap<String, MaterialFunction>;

/// How many distinct named parameters one master graph can declare. Must
/// match the array size in the WGSL `SurfaceGraphParams` declaration and
/// `surface_ext::SURFACE_GRAPH_PARAM_SLOTS`. Bumping this requires updating
/// all three locations together.
pub const MAX_PARAMETER_SLOTS: usize = 32;

/// Sanitize a function name into a WGSL-identifier-safe string.
pub(crate) fn safe_fn_ident(name: &str) -> String {
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

/// Pull the parameter identifier off a `param/*` node, falling back to a
/// type-appropriate default if the user hasn't authored one.
pub(crate) fn param_name(node: &MaterialNode, fallback: &str) -> String {
    match node.input_values.get("name") {
        Some(PinValue::String(s)) if !s.trim().is_empty() => s.clone(),
        _ => fallback.to_string(),
    }
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
    /// Named parameters declared by `param/*` nodes in the graph. Each entry
    /// is the master's authored default value; material instances supply
    /// per-instance overrides keyed by name.
    pub parameters: Vec<MaterialParam>,
    /// Which node authored which lines of `fragment_shader`:
    /// `(node id, first line, last line)`, 1-based, sorted by node id.
    /// Header and helper lines appear in no range — an error there is an
    /// engine bug, not a user bug. This is what lets compile errors point
    /// at a node instead of a line nobody can see.
    pub node_lines: Vec<(u64, u32, u32)>,
}

/// One named graph-boundary parameter discovered by the compiler.
///
/// Master shaders bake the default value into the WGSL; downstream tooling
/// (material instances, the inspector) consults this list to know what
/// overrides are valid and what defaults they replace.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MaterialParam {
    /// Identifier the user authored on the parameter node (e.g. "BaseColor").
    pub name: String,
    /// What kind of value the parameter holds.
    pub kind: ParamKind,
    /// Authored default — what the master shader uses absent an override.
    pub default: graph::PinValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ParamKind {
    Float,
    Color,
    Vec2,
    Vec3,
    Vec4,
    Bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TextureBinding {
    pub name: String,
    pub binding: u32,
    pub asset_path: String,
    pub kind: TextureKind,
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
                parameters: Vec::new(),
                node_lines: Vec::new(),
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
        errors.push(format!(
            "Unknown output node type: {}",
            output_node.node_type
        ));
        Vec::new()
    };

    // Parallax first, and out of band: it rewrites `mat_uv`, so its subgraph
    // has to be compiled before any pin that samples with it. Only a wired
    // `displacement` counts — a constant height describes a flat surface, and
    // marching one would burn a loop to return the UV it started with.
    if graph.domain == MaterialDomain::Surface
        && graph
            .connection_to(output_node.id, "displacement")
            .is_some()
    {
        let scale = match output_node.input_values.get("displacement_scale") {
            Some(PinValue::Float(v)) => *v,
            _ => 0.05,
        };
        let disp_fn = ctx.compile_displacement_fn(&output_node);
        ctx.module_prelude.push(disp_fn);
        ctx.module_prelude.push(parallax_helper_wgsl(scale));
        ctx.uses_parallax = true;
    }

    // Resolve each output pin's input (triggers recursive codegen).
    //
    // The two displacement pins are skipped: they produce no `pbr_input`
    // mutation, the march above is their only consumer, and resolving them
    // again out here would emit the height subgraph a second time — a second
    // texture *binding* for the same image, out of the six 2D slots the
    // extension has, plus a sample nothing reads.
    let mut resolved: HashMap<String, String> = HashMap::new();
    for pin_name in &output_pins {
        if matches!(pin_name.as_str(), "displacement" | "displacement_scale") {
            continue;
        }
        let expr = ctx.input(&output_node, pin_name);
        resolved.insert(pin_name.clone(), expr);
    }

    // Build the full shader
    let (fragment_shader, node_lines) = match graph.domain {
        MaterialDomain::Surface | MaterialDomain::Vegetation => {
            build_pbr_shader(&ctx, &resolved, graph.domain)
        }
        MaterialDomain::TerrainLayer => build_terrain_layer_shader(&ctx, &resolved),
        MaterialDomain::Unlit => build_unlit_shader(&ctx, &resolved),
        MaterialDomain::Sprite => build_sprite_shader(&ctx, &resolved),
    };
    errors.extend(ctx.errors.iter().cloned());

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
        warnings: ctx.warnings,
        requires_transmission,
        parameters: ctx.parameters,
        node_lines,
    }
}

/// Which node authored `line` (1-based) of the generated shader — the
/// innermost range containing it, since a node nested inside another's
/// lines is the more specific answer. `None` for header/helper lines, which
/// no node authored; an error there is an engine bug, not a user bug.
pub fn node_for_line(node_lines: &[(u64, u32, u32)], line: u32) -> Option<u64> {
    node_lines
        .iter()
        .filter(|(_, start, end)| *start <= line && line <= *end)
        .max_by_key(|(_, start, _)| *start)
        .map(|(node, _, _)| *node)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use graph::*;

    /// One graph per node that reads something a pipeline may not provide
    /// (a prepass, an environment map, tangents, vertex colours), plus the
    /// vegetation and terrain domains: every place the generator emits a gate.
    fn gated_graphs() -> Vec<(String, MaterialGraph)> {
        let mut graphs = Vec::new();
        for (domain, ty) in [
            (MaterialDomain::Surface, "scene/scene_depth"),
            (MaterialDomain::Surface, "scene/depth_fade"),
            (MaterialDomain::Surface, "scene/scene_normal"),
            (MaterialDomain::Surface, "scene/motion_vector"),
            (MaterialDomain::Surface, "scene/scene_color"),
            (MaterialDomain::Surface, "scene/env_map_sample"),
            (MaterialDomain::Surface, "scene/env_map_reflect"),
            (MaterialDomain::Surface, "input/vertex_color"),
            (MaterialDomain::Surface, "texture/sample_normal"),
            (MaterialDomain::Vegetation, "input/uv"),
            (MaterialDomain::TerrainLayer, "input/uv"),
        ] {
            let mut graph = MaterialGraph::new("Dump", domain);
            let n = graph.add_node(ty, [0.0, 0.0]);
            let pin = (nodes::node_def(ty).unwrap().pins)()
                .into_iter()
                .find(|p| p.direction == PinDir::Output)
                .unwrap()
                .name;
            let output_id = graph.output_node().unwrap().id;
            graph.connect(n, &pin, output_id, "base_color");
            if ty == "texture/sample_normal" {
                // With displacement too, for the parallax block.
                let h = graph.add_node("texture/sample", [0.0, 0.0]);
                graph.connect(h, "r", output_id, "displacement");
            }
            graphs.push((format!("{domain:?} {ty}"), graph));
        }
        graphs
    }

    /// Codegen writes WESL, so a generated shader never goes through the
    /// legacy translator. Proven equivalent when this changed: compiled
    /// against Bevy 0.20's shader library under every combination of the flags
    /// these shaders test, the new output produced the same modules as the
    /// translated old output.
    #[test]
    fn generated_shaders_are_wesl() {
        use renzora::shader_source::{dialect, Dialect};
        for (name, graph) in gated_graphs() {
            let result = compile(&graph);
            assert!(result.errors.is_empty(), "{name}: {:?}", result.errors);
            for src in std::iter::once(&result.fragment_shader).chain(result.vertex_shader.as_ref()) {
                assert_eq!(dialect(src, "material/x.wesl"), Dialect::Wesl, "{name}:\n{src}");
                for legacy in ["#import", "#ifdef", "#ifndef", "#else", "#endif", "#{"] {
                    assert!(!src.contains(legacy), "{name} still writes `{legacy}`:\n{src}");
                }
            }
        }
    }

    /// Sprite graphs: the empty one (the sprite as it is), and ones that
    /// distort the sprite's own texture, tint it from a parameter, mix in a
    /// texture and noise, and read time and the object's position.
    fn sprite_graphs() -> Vec<(String, MaterialGraph)> {
        let mut graphs = vec![("empty".to_string(), MaterialGraph::new("Sprite", MaterialDomain::Sprite))];

        // The sprite scrolled through a panner, blended towards a parameter
        // colour by animated noise.
        let mut g = MaterialGraph::new("Wave", MaterialDomain::Sprite);
        let uv = g.add_node("input/uv_panner", [0.0, 0.0]);
        let sprite = g.add_node("input/sprite_color", [0.0, 0.0]);
        let tint = g.add_node("param/color", [0.0, 0.0]);
        let noise = g.add_node("procedural/noise_perlin", [0.0, 0.0]);
        let lerp = g.add_node("color/lerp", [0.0, 0.0]);
        let out = g.output_node().unwrap().id;
        g.connect(uv, "uv", sprite, "uv");
        g.connect(uv, "uv", noise, "uv");
        g.connect(sprite, "color", lerp, "a");
        g.connect(tint, "value", lerp, "b");
        g.connect(noise, "value", lerp, "t");
        g.connect(lerp, "color", out, "color");
        graphs.push(("wave".to_string(), g));

        // A texture in place of the sprite's, faded by noise seeded from the
        // object's position (which reaches for `mesh_functions`).
        let mut g = MaterialGraph::new("Mix", MaterialDomain::Sprite);
        let tex = g.add_node("texture/sample", [0.0, 0.0]);
        let pos = g.add_node("input/object_position", [0.0, 0.0]);
        let noise = g.add_node("procedural/noise_perlin", [0.0, 0.0]);
        let out = g.output_node().unwrap().id;
        g.connect(tex, "color", out, "color");
        g.connect(pos, "position", noise, "uv");
        g.connect(noise, "value", out, "alpha");
        graphs.push(("mix".to_string(), g));

        graphs.push(("dissolve".to_string(), dissolve_sprite_graph()));
        graphs
    }

    /// A looping burn-away: noise is cut at a threshold that sweeps with time,
    /// and the band just above the cut glows a parameter colour. Exercises
    /// step, sine, parameters and the sprite's own alpha together.
    fn dissolve_sprite_graph() -> MaterialGraph {
        fn set(g: &mut MaterialGraph, id: NodeId, pin: &str, v: PinValue) {
            g.get_node_mut(id).unwrap().input_values.insert(pin.to_string(), v);
        }
        let mut g = MaterialGraph::new("Dissolve", MaterialDomain::Sprite);
        let out = g.output_node().unwrap().id;
        g.get_node_mut(out).unwrap().position = [900.0, 0.0];

        let uv = g.add_node("input/uv", [0.0, -120.0]);
        let noise = g.add_node("procedural/noise_perlin", [200.0, -120.0]);
        set(&mut g, noise, "scale", PinValue::Float(8.0));

        let speed = g.add_node("param/float", [0.0, 80.0]);
        set(&mut g, speed, "name", PinValue::String("speed".into()));
        set(&mut g, speed, "default", PinValue::Float(1.0));
        let wave = g.add_node("animation/sine_wave", [200.0, 80.0]);
        set(&mut g, wave, "amplitude", PinValue::Float(0.55));
        let amount = g.add_node("math/add", [400.0, 80.0]);
        set(&mut g, amount, "b", PinValue::Float(0.5));

        let width = g.add_node("param/float", [400.0, 220.0]);
        set(&mut g, width, "name", PinValue::String("edge_width".into()));
        set(&mut g, width, "default", PinValue::Float(0.06));
        let edge_at = g.add_node("math/add", [600.0, 200.0]);

        let visible = g.add_node("math/step", [600.0, -60.0]);
        let core = g.add_node("math/step", [600.0, 60.0]);
        let band = g.add_node("math/subtract", [750.0, 0.0]);

        let sprite = g.add_node("input/sprite_color", [400.0, -260.0]);
        let glow = g.add_node("param/color", [400.0, -400.0]);
        set(&mut g, glow, "name", PinValue::String("edge_color".into()));
        set(&mut g, glow, "default", PinValue::Color([1.0, 0.45, 0.1, 1.0]));
        let tint = g.add_node("color/lerp", [750.0, -260.0]);
        let alpha = g.add_node("math/multiply", [750.0, 120.0]);

        g.connect(uv, "uv", noise, "uv");
        g.connect(speed, "value", wave, "frequency");
        g.connect(wave, "value", amount, "a");
        g.connect(amount, "result", edge_at, "a");
        g.connect(width, "value", edge_at, "b");
        g.connect(amount, "result", visible, "edge");
        g.connect(noise, "value", visible, "value");
        g.connect(edge_at, "result", core, "edge");
        g.connect(noise, "value", core, "value");
        g.connect(visible, "result", band, "a");
        g.connect(core, "result", band, "b");
        g.connect(sprite, "color", tint, "a");
        g.connect(glow, "value", tint, "b");
        g.connect(band, "result", tint, "t");
        g.connect(visible, "result", alpha, "a");
        g.connect(sprite, "a", alpha, "b");
        g.connect(tint, "color", out, "color");
        g.connect(alpha, "result", out, "alpha");
        g
    }

    #[test]
    fn sprite_shaders_compile_from_the_sprite_pipeline() {
        for (name, graph) in sprite_graphs() {
            let result = compile(&graph);
            assert!(result.errors.is_empty(), "{name}: {:?}", result.errors);
            let s = &result.fragment_shader;
            assert!(!s.contains("Unknown node type"), "{name} names a node that does not exist:\n{s}");
            assert!(s.contains("import bevy_sprite_render::sprite_mesh::functions as sprite_fn;"), "{s}");
            assert!(s.contains("sprite_fn::get_final_color("), "{s}");
            assert!(!s.contains("bevy_pbr"), "{name} reaches for the 3D pipeline:\n{s}");
            // Past the sprite's own bindings (0..2, bindless at 10).
            assert!(s.contains("@group(constants::MATERIAL_BIND_GROUP) @binding(20) var texture_0"), "{s}");
        }
    }

    #[test]
    fn an_empty_sprite_material_draws_the_sprite() {
        let s = compile(&MaterialGraph::new("Sprite", MaterialDomain::Sprite)).fragment_shader;
        assert!(
            s.contains("var sprite_color = sprite_fn::sample_sprite_texture(mat_uv, in.instance_index);"),
            "{s}"
        );
    }

    /// The one rule the add-node menu, the wiring check and codegen share.
    #[test]
    fn each_domain_refuses_the_other_kinds_nodes() {
        assert!(MaterialDomain::Sprite.refuses_node("scene/scene_depth").is_some());
        assert!(MaterialDomain::Sprite.refuses_node("scene/env_map_sample").is_some());
        assert!(MaterialDomain::Sprite.refuses_node("input/sprite_color").is_none());
        assert!(MaterialDomain::Sprite.refuses_node("math/add").is_none());
        assert!(MaterialDomain::Surface.refuses_node("input/sprite_color").is_some());
        assert!(MaterialDomain::Surface.refuses_node("scene/scene_depth").is_none());
        // Screen UV is plain arithmetic, so a sprite may use it.
        assert!(MaterialDomain::Sprite.refuses_node("scene/screen_uv").is_none());
    }

    #[test]
    fn a_sprite_material_refuses_nodes_that_need_a_3d_view() {
        let mut g = MaterialGraph::new("Fog", MaterialDomain::Sprite);
        let depth = g.add_node("scene/scene_depth", [0.0, 0.0]);
        let out = g.output_node().unwrap().id;
        g.connect(depth, "depth", out, "alpha");
        let result = compile(&g);
        assert_eq!(result.errors.len(), 1, "{:?}", result.errors);
        assert!(result.errors[0].contains("3D view"), "{:?}", result.errors);
    }

    /// Bevy 0.20 dropped `mesh_functions::mesh_position_world_to_clip`, so a
    /// vegetation vertex shader that called it could not compile.
    #[test]
    fn vegetation_projects_through_view_transformations() {
        let graph = MaterialGraph::new("Grass", MaterialDomain::Vegetation);
        let vertex = compile(&graph).vertex_shader.expect("vegetation has a vertex stage");
        assert!(!vertex.contains("mesh_position_world_to_clip"), "{vertex}");
        assert!(vertex.contains("position_world_to_clip(world_pos.xyz)"), "{vertex}");
    }

    /// WGSL has no struct declarations inside a function.
    #[test]
    fn terrain_layer_declares_its_input_struct_at_module_scope() {
        let graph = MaterialGraph::new("Grass", MaterialDomain::TerrainLayer);
        let shader = compile(&graph).fragment_shader;
        let decl = shader.find("struct FakeIn").expect("FakeIn");
        let main = shader.find("fn layer_main").expect("layer_main");
        assert!(decl < main, "{shader}");
    }

    /// A Custom node may still hold old-dialect code (`#ifdef`), which makes
    /// the whole shader legacy and sends it through the translator, WESL and
    /// all. The translator has to leave the WESL it is handed working.
    #[test]
    fn a_legacy_custom_node_still_translates_cleanly() {
        let mut graph = MaterialGraph::new("Mixed", MaterialDomain::Surface);
        let depth = graph.add_node("scene/scene_depth", [0.0, 0.0]);
        let code = graph.add_node("custom/code", [100.0, 0.0]);
        graph.get_node_mut(code).unwrap().input_values.insert(
            "code".to_string(),
            PinValue::String(
                "#ifdef VERTEX_COLORS\nresult = vec4<f32>(a.xyz, 1.0);\n#else\nresult = vec4<f32>(1.0);\n#endif"
                    .to_string(),
            ),
        );
        let output_id = graph.output_node().unwrap().id;
        graph.connect(depth, "depth", code, "a");
        graph.connect(code, "result", output_id, "base_color");

        let src = compile(&graph).fragment_shader;
        let t = renzora::shader_source::to_wesl(&src);
        let errors: Vec<_> = t
            .diagnostics
            .iter()
            .filter(|d| d.severity == renzora::shader_source::Severity::Error)
            .collect();
        assert!(errors.is_empty(), "{errors:?}\n{}", t.source);
        assert!(t.source.contains("import bevy_pbr::render::pbr_functions;"), "{}", t.source);
        assert!(t.source.contains("@if(DEPTH_PREPASS)"), "{}", t.source);
        assert!(!t.source.contains("#ifdef"), "{}", t.source);
    }

    #[test]
    fn compile_default_surface() {
        let graph = MaterialGraph::new("Test", MaterialDomain::Surface);
        let result = compile(&graph);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(result
            .fragment_shader
            .contains("pbr_input_from_standard_material"));
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
        // Unlit uses the same extension-hook skeleton as Surface — the
        // resolver's `unlit = true` on the base material is what makes
        // `apply_pbr_lighting` pass base_color through unlit.
        assert!(result
            .fragment_shader
            .contains("pbr_input_from_standard_material"));
        assert!(result.fragment_shader.contains("apply_pbr_lighting"));
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

    #[test]
    fn displacement_emits_a_parallax_march_before_the_body() {
        let mut graph = MaterialGraph::new("Parallax", MaterialDomain::Surface);
        let height = graph.add_node("texture/sample", [-200.0, 0.0]);
        let output_id = graph.output_node().unwrap().id;
        graph.connect(height, "r", output_id, "displacement");

        let result = compile(&graph);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        let s = &result.fragment_shader;
        assert!(s.contains("fn graph_displacement("), "shader:\n{s}");
        assert!(s.contains("fn graph_parallax_uv("), "shader:\n{s}");
        // `mat_uv` has to be assignable, and the march has to run before the
        // graph body that samples through it.
        assert!(s.contains("var mat_uv = in.uv;"), "shader:\n{s}");
        let march = s.find("graph_parallax_uv(in, mat_uv").expect("march call");
        let aliases = s.find("var mat_uv = in.uv;").expect("alias");
        assert!(march > aliases, "march must follow the alias:\n{s}");
        // Height reads sit inside a variable-length loop, so they must not
        // carry gradient instructions.
        assert!(
            s.contains("textureSampleLevel(texture_0, texture_sampler, mat_uv, 0.0)"),
            "shader:\n{s}"
        );
        // The height texture must claim exactly one of the six 2D slots — the
        // subgraph is compiled standalone, so a second resolve out in the main
        // body would silently bind the same image twice.
        assert_eq!(result.texture_bindings.len(), 1, "shader:\n{s}");
    }

    #[test]
    fn a_constant_displacement_emits_no_march() {
        // No relief to walk through — marching would spend a loop arriving
        // back at the UV it started from.
        let mut graph = MaterialGraph::new("Flat", MaterialDomain::Surface);
        let output_id = graph.output_node().unwrap().id;
        graph
            .get_node_mut(output_id)
            .unwrap()
            .input_values
            .insert("displacement".into(), PinValue::Float(0.7));

        let result = compile(&graph);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert!(!result.fragment_shader.contains("graph_parallax_uv"));
    }

    #[test]
    fn flip_green_negates_the_normal_maps_y() {
        let mut graph = MaterialGraph::new("DxNormal", MaterialDomain::Surface);
        let n = graph.add_node("texture/sample_normal", [-200.0, 0.0]);
        let output_id = graph.output_node().unwrap().id;
        graph.connect(n, "normal", output_id, "normal");

        let plain = compile(&graph);
        assert!(plain.fragment_shader.contains("select(1.0, -1.0, false)"));

        graph
            .get_node_mut(n)
            .unwrap()
            .input_values
            .insert("flip_green".into(), PinValue::Bool(true));
        let flipped = compile(&graph);
        assert!(flipped.fragment_shader.contains("select(1.0, -1.0, true)"));
    }

    /// glTF factors multiply their texture; treating them as fallbacks
    /// discarded them on every imported material. Values are the Porsche's
    /// `coat`: a clear-coat over a separate `paint` material, authored at
    /// `baseColorFactor.a = 0.243`. Dropping that rendered the coat opaque and
    /// its metallic shell replaced the silver paint underneath.
    #[test]
    fn a_wired_pin_still_multiplies_by_its_factor() {
        let mut graph = MaterialGraph::new("Coat", MaterialDomain::Surface);
        let output_id = graph.output_node().unwrap().id;

        let tex = graph.add_node("texture/sample", [-200.0, 0.0]);
        graph.connect(tex, "color", output_id, "base_color");
        graph.connect(tex, "a", output_id, "alpha");
        graph.connect(tex, "g", output_id, "roughness");

        if let Some(out) = graph.get_node_mut(output_id) {
            out.input_values
                .insert("alpha".into(), PinValue::Float(0.242617));
            out.input_values
                .insert("roughness".into(), PinValue::Float(0.716311));
        }

        let shader = compile(&graph).fragment_shader;
        assert!(
            shader.contains("0.242617"),
            "the opacity factor must reach the shader:\n{shader}"
        );
        assert!(shader.contains("0.716311"), "roughness factor dropped");
    }

    /// A pin with a constant and *no* wire keeps writing the constant straight
    /// through — multiplying is only for the both-present case, or every
    /// unwired slider would square itself.
    #[test]
    fn an_unwired_factor_is_not_multiplied_by_itself() {
        let mut graph = MaterialGraph::new("Plain", MaterialDomain::Surface);
        let output_id = graph.output_node().unwrap().id;
        if let Some(out) = graph.get_node_mut(output_id) {
            out.input_values
                .insert("roughness".into(), PinValue::Float(0.25));
        }

        let shader = compile(&graph).fragment_shader;
        // `to_wgsl` wraps floats in `f32(..)` so naga never faces a bare
        // abstract literal; the point is the assignment carries no `*` factor.
        assert!(
            shader.contains("perceptual_roughness = f32(0.250000);"),
            "unwired constant must pass straight through:\n{shader}"
        );
    }

    /// A normal map decodes to tangent space, but the Surface Output `normal`
    /// pin feeds `pbr_input.N`, which `apply_pbr_lighting` reads as world
    /// space. Handing the tangent-space vector over unmapped makes a flat map
    /// region — (0,0,1) — claim the surface faces world +Z, so a floor loses
    /// half of every light to `N·L` clamping: a spot light renders as a
    /// half-disc with a hard straight edge, and rotating it in X or Z moves
    /// that edge instead of moving the pool. Reproduce with a spot light over
    /// any surface that has a normal map wired.
    #[test]
    fn a_sampled_normal_map_reaches_the_normal_pin_in_world_space() {
        let mut graph = MaterialGraph::new("WorldNormal", MaterialDomain::Surface);
        let n = graph.add_node("texture/sample_normal", [-200.0, 0.0]);
        let output_id = graph.output_node().unwrap().id;
        graph.connect(n, "normal", output_id, "normal");

        let shader = compile(&graph).fragment_shader;
        assert!(
            shader.contains("calculate_tbn_mikktspace(in.world_normal, in.world_tangent)"),
            "expected a mikktspace TBN, got:\n{shader}"
        );
        // Tangent-less meshes have no frame to map through; StandardMaterial
        // falls back to the vertex normal and so must we.
        assert!(shader.contains("normalize(in.world_normal)"));
    }

    /// The import pipeline bakes normal maps to `Bc5RgUnorm`, which has no blue
    /// channel. Reading `.b` off one of those gives 0 → z = -1, a normal facing
    /// into the surface, and the model lights inside-out. Z must be derived from
    /// XY, which is also exact for an ordinary three-channel map.
    #[test]
    fn a_normal_map_derives_z_instead_of_sampling_blue() {
        let mut graph = MaterialGraph::new("Bc5Normal", MaterialDomain::Surface);
        let n = graph.add_node("texture/sample_normal", [-200.0, 0.0]);
        let output_id = graph.output_node().unwrap().id;
        graph.connect(n, "normal", output_id, "normal");

        let shader = compile(&graph).fragment_shader;
        assert!(
            shader.contains("sqrt(max(0.0, 1.0 -"),
            "expected a derived Z, got:\n{shader}"
        );
        // `.rgb` would drag in the absent blue channel.
        assert!(!shader.contains(".rgb * 2.0 - 1.0"));
    }

    /// Terrain compiles to `layer_main()`, whose `FakeIn` has no
    /// `world_tangent` and which never imports `pbr_functions` — emitting the
    /// TBN conversion there would simply fail to compile. That domain consumes
    /// the tangent-space value directly, so it must stay untouched.
    #[test]
    fn a_terrain_layer_normal_map_stays_in_tangent_space() {
        let mut graph = MaterialGraph::new("TerrainNormal", MaterialDomain::TerrainLayer);
        let n = graph.add_node("texture/sample_normal", [-200.0, 0.0]);
        let output_id = graph.output_node().unwrap().id;
        graph.connect(n, "normal", output_id, "normal");

        let shader = compile(&graph).fragment_shader;
        assert!(!shader.contains("calculate_tbn_mikktspace"));
        assert!(!shader.contains("in.world_tangent"));
    }

    /// 1-based line of the first line containing `needle`.
    fn line_of(source: &str, needle: &str) -> u32 {
        source
            .lines()
            .position(|l| l.contains(needle))
            .map(|i| i as u32 + 1)
            .unwrap_or_else(|| panic!("'{needle}' not in shader:\n{source}"))
    }

    #[test]
    fn custom_code_nodes_map_their_lines_back() {
        let mut graph = MaterialGraph::new("Map", MaterialDomain::Surface);
        let first = graph.add_node("custom/code", [0.0, 0.0]);
        graph
            .get_node_mut(first)
            .unwrap()
            .input_values
            .insert(
                "code".to_string(),
                PinValue::String("result = vec4<f32>(1.0, 0.0, 0.0, 1.0);".to_string()),
            );
        let second = graph.add_node("custom/code", [100.0, 0.0]);
        graph
            .get_node_mut(second)
            .unwrap()
            .input_values
            .insert("code".to_string(), PinValue::String("result = a;".to_string()));
        let output_id = graph.output_node().unwrap().id;
        graph.connect(first, "result", second, "a");
        graph.connect(second, "result", output_id, "base_color");

        let result = compile(&graph);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);

        // Two helpers, each starting on its own line — a missing separator
        // would glue the second `fn` onto the first's closing brace.
        assert!(result.fragment_shader.contains(&format!("fn mat_custom_{first}(")));
        assert!(result.fragment_shader.contains(&format!("\nfn mat_custom_{second}(")));

        let first_line = line_of(&result.fragment_shader, "result = vec4<f32>(1.0, 0.0, 0.0, 1.0);");
        let second_line = line_of(&result.fragment_shader, "result = a;");
        assert_eq!(node_for_line(&result.node_lines, first_line), Some(first));
        assert_eq!(node_for_line(&result.node_lines, second_line), Some(second));

        // The PbrInput mutation attributes to the node wired into the pin.
        let mutation_line = line_of(&result.fragment_shader, "pbr_input.material.base_color = ");
        assert_eq!(node_for_line(&result.node_lines, mutation_line), Some(second));
    }
}
