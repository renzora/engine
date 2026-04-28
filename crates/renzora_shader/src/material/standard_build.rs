//! Trivial-graph compiler: tries to express a [`MaterialGraph`] as a plain
//! [`StandardMaterial`].
//!
//! The graph editor produces graphs that range from "imported glTF — a few
//! texture/sample nodes wired to PBR pins" to "fully procedural with noise,
//! UV scrolling and view-dependent math". The trivial subset is what every
//! glTF importer naturally emits: each pin is either a constant or a single
//! texture sample whose channels feed StandardMaterial inputs the way the
//! glTF spec describes them.
//!
//! For trivial graphs we skip codegen entirely and produce a regular
//! `Handle<StandardMaterial>` that goes through Bevy's stock PBR pipeline.
//! The benefits:
//!
//! - The drag/load asymmetry collapses, because both paths land on the same
//!   StandardMaterial that bevy_gltf would have produced from the same data.
//! - Bistro's 131 materials share Bevy's one PBR pipeline instead of
//!   producing 131 specialized pipelines via `ExtendedMaterial<...>`.
//! - We don't have to keep the `ExtendedMaterial` codegen pixel-equivalent to
//!   `apply_pbr_lighting` — Bevy is the reference implementation for trivial
//!   graphs.
//!
//! Anything outside the trivial subset (procedural noise, math nodes,
//! animated UVs, blend, custom WGSL, vertex displacement) returns `None` and
//! the caller falls back to the full graph codegen.
//!
//! The function is deliberately *strict*: when in doubt, fail. Failing means
//! "compile via the codegen path" which is always correct, just slower. A
//! permissive classifier that misclassifies a procedural graph as trivial
//! would render it wrong with no fallback.

use bevy::pbr::StandardMaterial;
use bevy::prelude::*;

use super::graph::{
    AlphaMode as GraphAlphaMode, MaterialDomain, MaterialGraph, MaterialNode, NodeId, PinValue,
};

/// Try to compile `graph` to a plain [`StandardMaterial`]. Returns `None` if
/// any reachable node falls outside the trivial subset, in which case the
/// caller should fall back to full codegen (`ExtendedMaterial` path).
pub fn try_build_standard_material(
    graph: &MaterialGraph,
    asset_server: &AssetServer,
) -> Option<StandardMaterial> {
    // StandardMaterial only models the Surface domain. Vegetation needs a
    // custom vertex stage; TerrainLayer compiles to layer_main/layer_pbr and
    // is consumed by a different shader entirely.
    if graph.domain != MaterialDomain::Surface {
        return None;
    }

    let output = graph.output_node()?;

    let mut mat = StandardMaterial::default();

    // Apply graph-level flags first; nothing below should override these.
    mat.alpha_mode = match graph.alpha_mode {
        GraphAlphaMode::Opaque => AlphaMode::Opaque,
        GraphAlphaMode::Mask { cutoff } => AlphaMode::Mask(cutoff),
        GraphAlphaMode::Blend => AlphaMode::Blend,
    };
    mat.cull_mode = if graph.double_sided {
        None
    } else {
        Some(bevy::render::render_resource::Face::Back)
    };

    // ── base_color (factor + texture) ───────────────────────────────────
    if let Some(PinValue::Color(c)) = output.input_values.get("base_color") {
        mat.base_color = Color::linear_rgba(c[0], c[1], c[2], c[3]);
    }
    if let Some(conn) = graph.connection_to(output.id, "base_color") {
        let src = graph.get_node(conn.from_node)?;
        // Accept the whole-RGBA pin or the rgb-only pin. Other channels (.r,
        // .g, .b, .a alone) on a base_color route mean the user is doing
        // something non-PBR and we let codegen handle it.
        if !is_texture_sample(src) {
            return None;
        }
        match conn.from_pin.as_str() {
            "color" | "rgb" => {
                mat.base_color_texture = Some(asset_server.load(texture_path(src)?));
            }
            _ => return None,
        }
    }

    // The "alpha" pin lands on `base_color.a`. Two common shapes coexist:
    //
    // - `texture/sample.a → output.alpha` *and* `texture/sample.color →
    //   output.base_color`, both from the SAME node. This is what every
    //   imported glTF emits — alpha is the texture's alpha channel,
    //   already covered by `base_color_texture`'s alpha. We accept and
    //   ignore.
    // - A constant `Float` on the alpha pin → `mat.base_color.a` factor.
    //
    // Anything else (alpha sourced from a different texture, or a
    // computed alpha) isn't expressible by StandardMaterial alone.
    if let Some(conn) = graph.connection_to(output.id, "alpha") {
        let src = graph.get_node(conn.from_node)?;
        if !is_texture_sample(src) || conn.from_pin != "a" {
            return None;
        }
        // Must be the same texture as base_color.
        let bc_conn = graph.connection_to(output.id, "base_color");
        match bc_conn {
            Some(bc) if bc.from_node == conn.from_node => { /* ok */ }
            _ => return None,
        }
    }
    if let Some(PinValue::Float(a)) = output.input_values.get("alpha") {
        let bc = mat.base_color.to_linear();
        mat.base_color = Color::linear_rgba(bc.red, bc.green, bc.blue, *a);
    }

    // ── metallic + roughness (factors and the shared MR texture) ────────
    if let Some(PinValue::Float(m)) = output.input_values.get("metallic") {
        mat.metallic = *m;
    }
    if let Some(PinValue::Float(r)) = output.input_values.get("roughness") {
        mat.perceptual_roughness = *r;
    }
    let metallic_src = pin_texture_source(graph, output.id, "metallic", "b")?;
    let roughness_src = pin_texture_source(graph, output.id, "roughness", "g")?;
    match (metallic_src, roughness_src) {
        (PinSource::Constant, PinSource::Constant) => {}
        (PinSource::Texture(m_node), PinSource::Texture(r_node)) if m_node == r_node => {
            // glTF MR layout: G=roughness, B=metallic on a single texture.
            let src = graph.get_node(m_node)?;
            let path = texture_path(src)?;
            mat.metallic_roughness_texture = Some(asset_server.load(path));
        }
        // Texture wired to one but not the other, or two different
        // textures — non-trivial.
        _ => return None,
    }

    // ── normal map ──────────────────────────────────────────────────────
    if let Some(conn) = graph.connection_to(output.id, "normal") {
        let src = graph.get_node(conn.from_node)?;
        if src.node_type != "texture/sample_normal" || conn.from_pin != "normal" {
            return None;
        }
        mat.normal_map_texture = Some(asset_server.load(texture_path(src)?));
    }

    // ── emissive (factor + texture) ─────────────────────────────────────
    if let Some(PinValue::Vec3(e)) = output.input_values.get("emissive") {
        // glTF / KHR_materials_emissive_strength HDR values land here. The
        // factor multiplies with the texture in Bevy's PBR shader, so a
        // factor of `[8, 0, 0]` with a white texture gives 8x red emissive.
        mat.emissive = LinearRgba::new(e[0], e[1], e[2], 1.0);
    }
    if let Some(conn) = graph.connection_to(output.id, "emissive") {
        let src = graph.get_node(conn.from_node)?;
        if !is_texture_sample(src) {
            return None;
        }
        match conn.from_pin.as_str() {
            "rgb" | "color" => {
                mat.emissive_texture = Some(asset_server.load(texture_path(src)?));
                // If no factor was set, default to white so the texture
                // shows through unmodulated.
                if !output.input_values.contains_key("emissive") {
                    mat.emissive = LinearRgba::WHITE;
                }
            }
            _ => return None,
        }
    }

    // ── ambient occlusion (R-channel sample) ────────────────────────────
    if let Some(conn) = graph.connection_to(output.id, "ao") {
        let src = graph.get_node(conn.from_node)?;
        if !is_texture_sample(src) || conn.from_pin != "r" {
            return None;
        }
        mat.occlusion_texture = Some(asset_server.load(texture_path(src)?));
    }

    // ── extension fields (transmission, ior, clearcoat, anisotropy) ─────
    //
    // Pure factors map directly. Texture connections on these pins would
    // require routing into the matching `*_texture` field; for now we treat
    // a connection as non-trivial and let codegen handle it. Most imported
    // materials don't use these anyway.
    if let Some(PinValue::Float(v)) = output.input_values.get("specular_transmission") {
        mat.specular_transmission = *v;
    }
    if let Some(PinValue::Float(v)) = output.input_values.get("diffuse_transmission") {
        mat.diffuse_transmission = *v;
    }
    if let Some(PinValue::Float(v)) = output.input_values.get("thickness") {
        mat.thickness = *v;
    }
    if let Some(PinValue::Float(v)) = output.input_values.get("ior") {
        mat.ior = *v;
    }
    if let Some(PinValue::Float(v)) = output.input_values.get("attenuation_distance") {
        mat.attenuation_distance = *v;
    }
    if let Some(PinValue::Float(v)) = output.input_values.get("clearcoat") {
        mat.clearcoat = *v;
    }
    if let Some(PinValue::Float(v)) = output.input_values.get("clearcoat_roughness") {
        mat.clearcoat_perceptual_roughness = *v;
    }
    if let Some(PinValue::Float(v)) = output.input_values.get("anisotropy_strength") {
        mat.anisotropy_strength = *v;
    }
    if let Some(PinValue::Float(v)) = output.input_values.get("anisotropy_rotation") {
        mat.anisotropy_rotation = *v;
    }
    if let Some(PinValue::Float(v)) = output.input_values.get("reflectance") {
        mat.reflectance = *v;
    }

    // Any of these pins being *connected* (not just constant-overridden)
    // means the user is computing the value, which is the non-trivial case.
    for pin in [
        "specular_transmission",
        "diffuse_transmission",
        "thickness",
        "ior",
        "attenuation_distance",
        "clearcoat",
        "clearcoat_roughness",
        "anisotropy_strength",
        "anisotropy_rotation",
        "reflectance",
    ] {
        if graph.connection_to(output.id, pin).is_some() {
            return None;
        }
    }

    // Vegetation-only output pins: a non-empty connection to vertex_offset
    // means custom vertex displacement, which StandardMaterial doesn't
    // expose. Belt-and-braces — we already rejected non-Surface domains
    // above, but the editor may set vertex_offset on a Surface graph by
    // accident.
    if graph.connection_to(output.id, "vertex_offset").is_some() {
        return None;
    }

    Some(mat)
}

/// Internal helper: how a particular output pin is sourced.
enum PinSource {
    /// Pin has no connection, only (optional) constant input_values.
    Constant,
    /// Pin is connected to the named texture/sample node.
    Texture(NodeId),
}

/// Resolve the source of `pin_name` on the output node, requiring that any
/// connection comes from a `texture/sample` node via `expected_channel`.
/// Returns `None` if the connection routes through anything else (a math
/// node, a different texture pin, etc.) — i.e. non-trivial.
fn pin_texture_source(
    graph: &MaterialGraph,
    output_id: NodeId,
    pin_name: &str,
    expected_channel: &str,
) -> Option<PinSource> {
    match graph.connection_to(output_id, pin_name) {
        None => Some(PinSource::Constant),
        Some(conn) => {
            let src = graph.get_node(conn.from_node)?;
            if !is_texture_sample(src) || conn.from_pin != expected_channel {
                return None;
            }
            Some(PinSource::Texture(conn.from_node))
        }
    }
}

/// Returns true if `node` is the basic 2D texture sample node — *not* lod /
/// grad / cubemap / 2d-array / triplanar variants, which require
/// non-StandardMaterial sampling logic.
fn is_texture_sample(node: &MaterialNode) -> bool {
    node.node_type == "texture/sample"
}

/// Pull the texture asset path off a `texture/sample` or
/// `texture/sample_normal` node's `texture` input value.
fn texture_path(node: &MaterialNode) -> Option<String> {
    match node.input_values.get("texture")? {
        PinValue::TexturePath(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::graph::*;

    #[test]
    fn empty_graph_yields_default_material() {
        let graph = MaterialGraph::new("Empty", MaterialDomain::Surface);
        let app = App::new();
        let asset_server = app.world().resource::<AssetServer>();
        let mat = try_build_standard_material(&graph, asset_server);
        assert!(mat.is_some());
    }

    #[test]
    fn rejects_non_surface_domains() {
        let graph = MaterialGraph::new("Veg", MaterialDomain::Vegetation);
        let app = App::new();
        let asset_server = app.world().resource::<AssetServer>();
        assert!(try_build_standard_material(&graph, asset_server).is_none());
    }

    #[test]
    fn rejects_procedural_node() {
        let mut graph = MaterialGraph::new("Procedural", MaterialDomain::Surface);
        let output_id = graph.output_node().unwrap().id;
        let noise = graph.add_node("procedural/noise_perlin", [-200.0, 0.0]);
        graph.connect(noise, "value", output_id, "base_color");
        let app = App::new();
        let asset_server = app.world().resource::<AssetServer>();
        assert!(try_build_standard_material(&graph, asset_server).is_none());
    }

    #[test]
    fn rejects_math_node_on_pbr_pin() {
        let mut graph = MaterialGraph::new("Math", MaterialDomain::Surface);
        let output_id = graph.output_node().unwrap().id;
        let m = graph.add_node("math/multiply", [-200.0, 0.0]);
        graph.connect(m, "result", output_id, "roughness");
        let app = App::new();
        let asset_server = app.world().resource::<AssetServer>();
        assert!(try_build_standard_material(&graph, asset_server).is_none());
    }

    #[test]
    fn rejects_split_metallic_roughness_textures() {
        let mut graph = MaterialGraph::new("SplitMR", MaterialDomain::Surface);
        let output_id = graph.output_node().unwrap().id;
        let tex_m = graph.add_node("texture/sample", [-200.0, -100.0]);
        let tex_r = graph.add_node("texture/sample", [-200.0, 100.0]);
        if let Some(n) = graph.get_node_mut(tex_m) {
            n.input_values.insert("texture".into(), PinValue::TexturePath("a.rmip".into()));
        }
        if let Some(n) = graph.get_node_mut(tex_r) {
            n.input_values.insert("texture".into(), PinValue::TexturePath("b.rmip".into()));
        }
        graph.connect(tex_m, "b", output_id, "metallic");
        graph.connect(tex_r, "g", output_id, "roughness");
        let app = App::new();
        let asset_server = app.world().resource::<AssetServer>();
        assert!(try_build_standard_material(&graph, asset_server).is_none());
    }

    #[test]
    fn accepts_imported_glb_shape() {
        // Mirrors what `pbr_build::build_surface_graph` produces: one MR
        // texture, one normal texture, one base color texture all wired
        // through a single `output/surface` node, no math.
        let mut graph = MaterialGraph::new("Imported", MaterialDomain::Surface);
        let output_id = graph.output_node().unwrap().id;

        let bc = graph.add_node("texture/sample", [-200.0, -200.0]);
        if let Some(n) = graph.get_node_mut(bc) {
            n.input_values.insert("texture".into(), PinValue::TexturePath("bc.rmip".into()));
        }
        graph.connect(bc, "color", output_id, "base_color");
        graph.connect(bc, "a", output_id, "alpha");

        let mr = graph.add_node("texture/sample", [-200.0, -50.0]);
        if let Some(n) = graph.get_node_mut(mr) {
            n.input_values.insert("texture".into(), PinValue::TexturePath("mr.rmip".into()));
        }
        graph.connect(mr, "g", output_id, "roughness");
        graph.connect(mr, "b", output_id, "metallic");

        let n = graph.add_node("texture/sample_normal", [-200.0, 100.0]);
        if let Some(node) = graph.get_node_mut(n) {
            node.input_values.insert("texture".into(), PinValue::TexturePath("n.rmip".into()));
        }
        graph.connect(n, "normal", output_id, "normal");

        // Constants on the factor pins, as importer emits.
        if let Some(out) = graph.get_node_mut(output_id) {
            out.input_values.insert("base_color".into(), PinValue::Color([1.0, 1.0, 1.0, 1.0]));
            out.input_values.insert("metallic".into(), PinValue::Float(1.0));
            out.input_values.insert("roughness".into(), PinValue::Float(1.0));
            out.input_values.insert("emissive".into(), PinValue::Vec3([8.0, 0.0, 0.0]));
            out.input_values.insert("alpha".into(), PinValue::Float(1.0));
        }

        let app = App::new();
        let asset_server = app.world().resource::<AssetServer>();
        let mat = try_build_standard_material(&graph, asset_server).expect("trivial");
        assert!(mat.base_color_texture.is_some());
        assert!(mat.metallic_roughness_texture.is_some());
        assert!(mat.normal_map_texture.is_some());
        // HDR emissive lands as a LinearRgba > 1.0 — what bloom needs.
        assert_eq!(mat.emissive.red, 8.0);
    }
}
