//! Assembling a whole shader from a compiled graph, one builder per domain.
//!
//! None of these writes a `@fragment` that hand-builds a `PbrInput`. They emit
//! Bevy's *extension hook* instead: call
//! `pbr_input_from_standard_material` to get everything StandardMaterial would
//! have set, run the graph body, then overwrite only the fields whose pins the
//! user actually wired. That is what lets a graph override roughness alone and
//! keep the StandardMaterial's base-colour texture.
//!
//! The builders also own the line accounting. Each mutation line is attributed
//! to the node feeding its pin, so a bad expression reports at the node that
//! produced it rather than at the assembly line that consumed it.

use std::collections::HashMap;

use super::super::graph::{MaterialDomain, MaterialGraph, MaterialNode, NodeId};
use super::ctx::Ctx;
use super::wgsl::{
    emit_ext_shader_header, emit_module_prelude, fragment_input_aliases, noise_helpers,
    texture_bindings_wgsl, PARALLAX_FRAGMENT_WGSL,
};

/// Merge the per-line spans recorded during emission into per-node absolute
/// ranges. `prelude_base` / `body_base` are the 1-based absolute lines the
/// first prelude / body line lands on in the assembled shader; `extra`
/// covers builder-tail lines (the PbrInput mutation block) whose absolute
/// positions only the builder knows.
pub(crate) fn node_line_ranges(
    ctx: &Ctx,
    prelude_base: u32,
    body_base: u32,
    extra: &[(NodeId, u32, u32)],
) -> Vec<(u64, u32, u32)> {
    let mut ranges: HashMap<NodeId, (u32, u32)> = HashMap::new();
    let mut extend = |node: NodeId, start: u32, end: u32| {
        let r = ranges.entry(node).or_insert((start, end));
        r.0 = r.0.min(start);
        r.1 = r.1.max(end);
    };
    for &(node, idx) in &ctx.body_spans {
        extend(node, body_base + idx, body_base + idx);
    }
    for &(node, start, count) in &ctx.prelude_spans {
        extend(node, prelude_base + start, prelude_base + start + count - 1);
    }
    for &(node, start, end) in extra {
        extend(node, start, end);
    }
    let mut ranges: Vec<(u64, u32, u32)> = ranges
        .into_iter()
        .map(|(node, (start, end))| (node, start, end))
        .collect();
    ranges.sort_unstable();
    ranges
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
pub(crate) fn build_pbr_shader(
    ctx: &Ctx,
    resolved: &HashMap<String, String>,
    _domain: MaterialDomain,
) -> (String, Vec<(u64, u32, u32)>) {
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

    /// A pin's constant, as a WGSL literal, but **only when it is also wired**.
    ///
    /// glTF's factors multiply their texture: `roughness = roughnessFactor *
    /// mr.g`, `baseColor = baseColorFactor * baseColorTexture`, and so on.
    /// Treating the constant as a mere fallback — used only when nothing is
    /// connected — silently discards it on every imported material, which is
    /// exactly what the importer writes for all of them.
    ///
    /// The Porsche is the case that made this obvious. Its `coat` is a
    /// clear-coat over a separate `paint` material, authored with
    /// `baseColorFactor.a = 0.243`. Dropped, the coat rendered fully opaque and
    /// its metallic shell replaced the silver paint underneath entirely.
    ///
    /// Multiplying also keeps this path level with the other two. The trivial
    /// fast path writes the factor *and* the texture onto a `StandardMaterial`
    /// and lets Bevy's shader multiply them, and so does the base approximation
    /// every non-forward pass shades from. Codegen was the only one dropping it,
    /// so the same material shaded differently depending on which path it took.
    fn wired_factor(node: &MaterialNode, graph: &MaterialGraph, pin: &str) -> Option<String> {
        graph.connection_to(node.id, pin)?;
        node.input_values.get(pin).map(|v| v.to_wgsl())
    }

    // `expr`, scaled by the pin's constant when it has one. `dims` is how many
    // components the pin carries, so a scalar factor widens to match a vector
    // expression rather than failing to type-check.
    let scaled = |pin: &str, expr: &str, dims: usize| -> String {
        match wired_factor(output_node, ctx.graph, pin) {
            None => expr.to_string(),
            Some(f) => {
                // `to_wgsl` emits a Color/Vec4 as `vec4<f32>(..)` and a Float as
                // a bare literal; widen the scalar so `f32 * vec3` never appears.
                let f = if dims > 1 && !f.starts_with("vec") {
                    format!("vec{dims}<f32>({f})")
                } else {
                    f
                };
                format!("(({f}) * ({expr}))")
            }
        }
    };

    let mut shader = String::new();
    emit_ext_shader_header(ctx, &mut shader);
    shader.push_str(&texture_bindings_wgsl(ctx));
    shader.push_str(&noise_helpers(ctx));
    let prelude_base = shader.matches('\n').count() as u32 + 1;
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

    // Must land between the aliases and the body: it consumes `mat_uv` and
    // replaces it, and every sampler in the body reads it afterwards.
    if ctx.uses_parallax {
        shader.push_str(PARALLAX_FRAGMENT_WGSL);
    }

    // Graph body — runs between the StandardMaterial init and the mutations.
    let body_base = shader.matches('\n').count() as u32 + 1;
    for line in &ctx.lines {
        shader.push_str(line);
        shader.push('\n');
    }

    // Override pbr_input fields for each pin the user wired up. Disconnected
    // pins leave StandardMaterial's defaults in place, so authors can partially
    // override (e.g. only procedural roughness, keeping base_color from the
    // StandardMaterial's texture). Each mutation line attributes to the node
    // feeding the pin — a bad expression fails *here*, not where it was built.
    shader.push_str("\n    // Graph → PbrInput mutations\n");
    let feeder = |pin: &str| {
        ctx.graph
            .connection_to(output_id, pin)
            .map(|c| c.from_node)
            .unwrap_or(output_id)
    };
    let mut mutations: Vec<(NodeId, String)> = Vec::new();
    if is_connected("base_color") {
        let e = resolved.get("base_color").unwrap();
        let e = scaled("base_color", e, 4);
        mutations.push((feeder("base_color"), format!("    pbr_input.material.base_color = {e};")));
    }
    if is_connected("metallic") {
        let e = resolved.get("metallic").unwrap();
        let e = scaled("metallic", e, 1);
        mutations.push((feeder("metallic"), format!("    pbr_input.material.metallic = {e};")));
    }
    if is_connected("roughness") {
        let e = resolved.get("roughness").unwrap();
        let e = scaled("roughness", e, 1);
        mutations.push((feeder("roughness"), format!("    pbr_input.material.perceptual_roughness = {e};")));
    }
    if is_connected("emissive") {
        let e = resolved.get("emissive").unwrap();
        let e = scaled("emissive", e, 3);
        mutations.push((feeder("emissive"), format!("    pbr_input.material.emissive = vec4<f32>({e}, 1.0);")));
    }
    if is_connected("ao") {
        let e = resolved.get("ao").unwrap();
        mutations.push((feeder("ao"), format!("    pbr_input.diffuse_occlusion = vec3<f32>({e});")));
    }
    if is_connected("normal") {
        let e = resolved.get("normal").unwrap();
        mutations.push((feeder("normal"), format!("    pbr_input.N = normalize({e});\n    pbr_input.world_normal = pbr_input.N;")));
    }
    if is_connected("alpha") {
        let e = resolved.get("alpha").unwrap();
        // glTF folds opacity into `baseColorFactor.a`, and the importer puts it
        // on this pin. A clear-coat authored at 0.243 that ignores its factor
        // renders solid and hides whatever it was meant to sit over.
        let e = scaled("alpha", e, 1);
        mutations.push((feeder("alpha"), format!("    pbr_input.material.base_color.a = {e};")));
    }
    if is_connected("reflectance") {
        let e = resolved.get("reflectance").unwrap();
        mutations.push((feeder("reflectance"), format!("    pbr_input.material.reflectance = {e};")));
    }
    // ── Transmission (water, glass, ice) ──────────────────────────────
    // `specular_transmission > 0` on the CPU-side StandardMaterial is what
    // triggers Bevy to schedule its transmissive pass. The resolver takes
    // care of setting the CPU-side flag (see `requires_transmission`).
    if is_connected("specular_transmission") {
        let e = resolved.get("specular_transmission").unwrap();
        mutations.push((feeder("specular_transmission"), format!("    pbr_input.material.specular_transmission = {e};")));
    }
    if is_connected("diffuse_transmission") {
        let e = resolved.get("diffuse_transmission").unwrap();
        mutations.push((feeder("diffuse_transmission"), format!("    pbr_input.material.diffuse_transmission = {e};")));
    }
    if is_connected("thickness") {
        let e = resolved.get("thickness").unwrap();
        mutations.push((feeder("thickness"), format!("    pbr_input.material.thickness = {e};")));
    }
    if is_connected("ior") {
        let e = resolved.get("ior").unwrap();
        mutations.push((feeder("ior"), format!("    pbr_input.material.ior = {e};")));
    }
    if is_connected("attenuation_distance") {
        let e = resolved.get("attenuation_distance").unwrap();
        mutations.push((feeder("attenuation_distance"), format!("    pbr_input.material.attenuation_distance = {e};")));
    }
    // The colour light attenuates *toward* over that distance. Without it the
    // distance pin was half a control: thick glass got darker but never took
    // on a tint.
    if is_connected("attenuation_color") {
        let e = resolved.get("attenuation_color").unwrap();
        mutations.push((feeder("attenuation_color"), format!("    pbr_input.material.attenuation_color = vec4<f32>({e}, 1.0);")));
    }

    // ── Clearcoat (car paint, lacquer) ────────────────────────────────
    if is_connected("clearcoat") {
        let e = resolved.get("clearcoat").unwrap();
        mutations.push((feeder("clearcoat"), format!("    pbr_input.material.clearcoat = {e};")));
    }
    if is_connected("clearcoat_roughness") {
        let e = resolved.get("clearcoat_roughness").unwrap();
        mutations.push((feeder("clearcoat_roughness"), format!("    pbr_input.material.clearcoat_perceptual_roughness = {e};")));
    }

    // ── Anisotropy (brushed metal, hair) ──────────────────────────────
    // WGSL expects `anisotropy_rotation` as a vec2<cos, sin>. Our graph pin
    // takes the rotation angle as a scalar (radians), so we build the vec2.
    if is_connected("anisotropy_strength") {
        let e = resolved.get("anisotropy_strength").unwrap();
        mutations.push((feeder("anisotropy_strength"), format!("    pbr_input.material.anisotropy_strength = {e};")));
    }
    if is_connected("anisotropy_rotation") {
        let e = resolved.get("anisotropy_rotation").unwrap();
        mutations.push((feeder("anisotropy_rotation"), format!("    pbr_input.material.anisotropy_rotation = vec2<f32>(cos({e}), sin({e}));")));
    }

    let mut extra: Vec<(NodeId, u32, u32)> = Vec::new();
    for (node, text) in &mutations {
        let start = shader.matches('\n').count() as u32 + 1;
        shader.push_str(text);
        shader.push('\n');
        extra.push((*node, start, start + text.matches('\n').count() as u32));
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

    (shader, node_line_ranges(ctx, prelude_base, body_base, &extra))
}

pub(crate) fn build_terrain_layer_shader(
    ctx: &Ctx,
    resolved: &HashMap<String, String>,
) -> (String, Vec<(u64, u32, u32)>) {
    let base_color = resolved
        .get("base_color")
        .cloned()
        .unwrap_or("vec4<f32>(0.5, 0.5, 0.5, 1.0)".into());
    let metallic = resolved.get("metallic").cloned().unwrap_or("0.0".into());
    let roughness = resolved.get("roughness").cloned().unwrap_or("0.5".into());
    let _height = resolved.get("height").cloned().unwrap_or("0.5".into());

    let mut shader = String::new();
    shader.push_str("// Auto-generated terrain layer shader\n");
    shader.push_str("#import bevy_pbr::mesh_view_bindings::globals\n\n");
    shader.push_str(&texture_bindings_wgsl(ctx));
    shader.push_str(&noise_helpers(ctx));
    let prelude_base = shader.matches('\n').count() as u32 + 1;
    emit_module_prelude(ctx, &mut shader);

    // layer_main: returns base color
    shader.push_str("\nfn layer_main(uv: vec2<f32>, world_pos: vec3<f32>, world_normal: vec3<f32>, time: f32) -> vec4<f32> {\n");
    shader.push_str("    // Alias inputs for compatibility\n");
    shader.push_str("    struct FakeIn { uv: vec2<f32>, world_position: vec4<f32>, world_normal: vec3<f32> };\n");
    shader.push_str("    let in = FakeIn(uv, vec4<f32>(world_pos, 1.0), world_normal);\n");
    // Terrain has explicit UV; vertex_color isn't meaningful here so use white.
    shader.push_str("    let mat_uv = uv;\n");
    shader.push_str("    let mat_vertex_color = vec4<f32>(1.0, 1.0, 1.0, 1.0);\n");
    let body_base = shader.matches('\n').count() as u32 + 1;
    for line in &ctx.lines {
        shader.push_str(line);
        shader.push('\n');
    }
    let output_id = ctx.graph.output_node().unwrap().id;
    let feeder = |pin: &str| {
        ctx.graph
            .connection_to(output_id, pin)
            .map(|c| c.from_node)
            .unwrap_or(output_id)
    };
    let mut extra: Vec<(NodeId, u32, u32)> = Vec::new();
    let start = shader.matches('\n').count() as u32 + 1;
    shader.push_str(&format!("    return {base_color};\n"));
    extra.push((feeder("base_color"), start, start));
    shader.push_str("}\n\n");

    // layer_pbr: returns (metallic, roughness)
    shader.push_str("fn layer_pbr(uv: vec2<f32>, world_pos: vec3<f32>) -> vec2<f32> {\n");
    let start = shader.matches('\n').count() as u32 + 1;
    shader.push_str(&format!("    return vec2<f32>({metallic}, {roughness});\n"));
    extra.push((feeder("metallic"), start, start));
    shader.push_str("}\n");

    (shader, node_line_ranges(ctx, prelude_base, body_base, &extra))
}

/// Unlit domain uses the same extension-hook skeleton as Surface. The key
/// difference is the resolver flips `StandardMaterial.unlit = true` on the
/// base — that makes `apply_pbr_lighting` return `base_color` unchanged,
/// skipping diffuse / specular / IBL. The graph's "color" pin becomes the
/// material's base_color; "alpha" drives alpha.
pub(crate) fn build_unlit_shader(
    ctx: &Ctx,
    resolved: &HashMap<String, String>,
) -> (String, Vec<(u64, u32, u32)>) {
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
    let prelude_base = shader.matches('\n').count() as u32 + 1;
    emit_module_prelude(ctx, &mut shader);

    shader.push_str("\n@fragment\n");
    shader.push_str("fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {\n");
    shader.push_str("    var pbr_input = pbr_input_from_standard_material(in, is_front);\n");
    shader.push_str(&fragment_input_aliases());

    let body_base = shader.matches('\n').count() as u32 + 1;
    for line in &ctx.lines {
        shader.push_str(line);
        shader.push('\n');
    }

    // Unlit "color" pin drives the StandardMaterial base_color. Because the
    // base has `unlit = true`, `apply_pbr_lighting` returns this value
    // unmodified (no lighting math applied) — the fastest path for HUD /
    // debug viz / stylised materials.
    let feeder = |pin: &str| {
        ctx.graph
            .connection_to(output_id, pin)
            .map(|c| c.from_node)
            .unwrap_or(output_id)
    };
    let mut extra: Vec<(NodeId, u32, u32)> = Vec::new();
    if color_connected {
        let e = resolved.get("color").unwrap();
        let start = shader.matches('\n').count() as u32 + 1;
        shader.push_str(&format!("    pbr_input.material.base_color = {e};\n"));
        extra.push((feeder("color"), start, start));
    }
    if alpha_connected {
        let e = resolved.get("alpha").unwrap();
        let start = shader.matches('\n').count() as u32 + 1;
        shader.push_str(&format!("    pbr_input.material.base_color.a = {e};\n"));
        extra.push((feeder("alpha"), start, start));
    }

    // Match bevy_pbr::pbr.wgsl — alpha_discard handles OPAQUE/MASK before lighting.
    shader.push_str("    pbr_input.material.base_color = pbr_functions::alpha_discard(pbr_input.material, pbr_input.material.base_color);\n");

    shader.push_str("\n    var out: FragmentOutput;\n");
    shader.push_str("    out.color = pbr_functions::apply_pbr_lighting(pbr_input);\n");
    shader.push_str("    out.color = pbr_functions::main_pass_post_lighting_processing(pbr_input, out.color);\n");
    shader.push_str("    return out;\n");
    shader.push_str("}\n");

    (shader, node_line_ranges(ctx, prelude_base, body_base, &extra))
}

pub(crate) fn build_vegetation_vertex_shader(
    _ctx: &Ctx,
    resolved: &HashMap<String, String>,
) -> String {
    let vertex_offset = resolved
        .get("vertex_offset")
        .cloned()
        .unwrap_or("vec3<f32>(0.0, 0.0, 0.0)".into());

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
    shader.push_str(&format!(
        "    world_pos = vec4<f32>(world_pos.xyz + {vertex_offset}, world_pos.w);\n"
    ));

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
