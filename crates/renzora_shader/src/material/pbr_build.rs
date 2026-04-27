//! Build a [`MaterialGraph`] from plain PBR factors + texture paths and
//! serialize it to the on-disk `.material` JSON format.
//!
//! Importers (renzora_import_ui dialog, viewport drop pipeline) emit
//! `renzora::PbrMaterialExtracted` events with the per-material PBR data
//! they pulled out of the source file. The observer below converts each
//! event into a graph file the rest of the editor can load and edit.

use crate::material::graph::{
    AlphaMode, Connection, MaterialDomain, MaterialGraph, PinValue,
};

/// Plain PBR inputs, deliberately a mirror of `renzora_import::ExtractedPbrMaterial`
/// so importers don't need a crate-specific conversion struct. Covers the
/// full glTF 2.0 metallic-roughness model plus alpha behavior.
pub struct PbrInputs {
    pub name: String,
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    /// glTF `emissiveFactor` (RGB linear). Multiplied with `emissive_texture`
    /// when present; used directly when not.
    pub emissive: [f32; 3],
    pub base_color_texture: Option<String>,
    pub normal_texture: Option<String>,
    /// glTF metallic-roughness map. Channels: G = roughness, B = metallic.
    pub metallic_roughness_texture: Option<String>,
    pub emissive_texture: Option<String>,
    /// Ambient occlusion (R channel only).
    pub occlusion_texture: Option<String>,
    /// glTF spec-gloss `specularGlossinessTexture` (alpha = per-pixel
    /// glossiness). The graph builder routes `1 - alpha` into the
    /// `roughness` pin so wet stones / glass survive the spec-gloss →
    /// metal-rough conversion with their per-pixel reflectivity intact.
    pub specular_glossiness_texture: Option<String>,
    pub alpha_mode: AlphaMode,
    pub double_sided: bool,
}

/// Build a surface-domain [`MaterialGraph`] from the given PBR inputs.
///
/// The graph always contains the `output/surface` sink (added by
/// [`MaterialGraph::new`]). Texture-sampling nodes are appended only when
/// the corresponding path is set, with connections from the relevant output
/// pin to the corresponding output input pin. Factor values seed the output
/// pin defaults so they remain in effect even if the user later disconnects
/// a texture.
///
/// Texture nodes share a single column on the canvas (all at `x = -160`)
/// stacked vertically so a freshly-imported graph reads cleanly when opened
/// in the material editor.
pub fn build_surface_graph(inputs: &PbrInputs) -> MaterialGraph {
    let mut graph = MaterialGraph::new(&inputs.name, MaterialDomain::Surface);
    graph.alpha_mode = inputs.alpha_mode;
    graph.double_sided = inputs.double_sided;
    let output_id = graph.nodes.first().map(|n| n.id).unwrap_or(1);

    // Seed output-pin defaults from the material's scalar factors. The
    // graph keeps these values whenever the corresponding texture is not
    // wired in, so a material with `metallic=1` `roughness=0.2` and no MR
    // texture still renders shiny.
    if let Some(out) = graph.nodes.iter_mut().find(|n| n.id == output_id) {
        out.input_values
            .insert("base_color".into(), PinValue::Color(inputs.base_color));
        out.input_values
            .insert("metallic".into(), PinValue::Float(inputs.metallic));
        out.input_values
            .insert("roughness".into(), PinValue::Float(inputs.roughness));
        out.input_values
            .insert("emissive".into(), PinValue::Vec3(inputs.emissive));
        // Carry baseColorFactor.alpha onto the alpha pin so transparent
        // materials surface their authored alpha even when the texture
        // sample isn't wired into it.
        out.input_values
            .insert("alpha".into(), PinValue::Float(inputs.base_color[3]));
    }

    // Lay texture nodes out in a column to the left of the output. The
    // y-cursor advances per node so they don't overlap.
    let mut tex_y: f32 = -240.0;
    let tex_x: f32 = -160.0;
    let tex_dy: f32 = 140.0;

    if let Some(ref tex_path) = inputs.base_color_texture {
        let tex_id = graph.add_node("texture/sample", [tex_x, tex_y]);
        if let Some(node) = graph.nodes.iter_mut().find(|n| n.id == tex_id) {
            node.input_values
                .insert("texture".into(), PinValue::TexturePath(tex_path.clone()));
        }
        graph.connections.push(Connection {
            from_node: tex_id,
            from_pin: "color".into(),
            to_node: output_id,
            to_pin: "base_color".into(),
        });
        // Wire the texture's alpha to the output alpha so transparent
        // textures (glass, foliage cutouts) actually punch through.
        graph.connections.push(Connection {
            from_node: tex_id,
            from_pin: "a".into(),
            to_node: output_id,
            to_pin: "alpha".into(),
        });
        tex_y += tex_dy;
    }

    // glTF packs metallic and roughness into one texture: G = roughness,
    // B = metallic. We emit a single sampler and route the two channels to
    // their respective output pins.
    if let Some(ref tex_path) = inputs.metallic_roughness_texture {
        let tex_id = graph.add_node("texture/sample", [tex_x, tex_y]);
        if let Some(node) = graph.nodes.iter_mut().find(|n| n.id == tex_id) {
            node.input_values
                .insert("texture".into(), PinValue::TexturePath(tex_path.clone()));
        }
        graph.connections.push(Connection {
            from_node: tex_id,
            from_pin: "g".into(),
            to_node: output_id,
            to_pin: "roughness".into(),
        });
        graph.connections.push(Connection {
            from_node: tex_id,
            from_pin: "b".into(),
            to_node: output_id,
            to_pin: "metallic".into(),
        });
        tex_y += tex_dy;
    }

    if let Some(ref tex_path) = inputs.normal_texture {
        let tex_id = graph.add_node("texture/sample_normal", [tex_x, tex_y]);
        if let Some(node) = graph.nodes.iter_mut().find(|n| n.id == tex_id) {
            node.input_values
                .insert("texture".into(), PinValue::TexturePath(tex_path.clone()));
        }
        graph.connections.push(Connection {
            from_node: tex_id,
            from_pin: "normal".into(),
            to_node: output_id,
            to_pin: "normal".into(),
        });
        tex_y += tex_dy;
    }

    if let Some(ref tex_path) = inputs.emissive_texture {
        let tex_id = graph.add_node("texture/sample", [tex_x, tex_y]);
        if let Some(node) = graph.nodes.iter_mut().find(|n| n.id == tex_id) {
            node.input_values
                .insert("texture".into(), PinValue::TexturePath(tex_path.clone()));
        }
        // Output `rgb` is Vec3 — the type that the emissive pin expects.
        graph.connections.push(Connection {
            from_node: tex_id,
            from_pin: "rgb".into(),
            to_node: output_id,
            to_pin: "emissive".into(),
        });
        tex_y += tex_dy;
    }

    if let Some(ref tex_path) = inputs.occlusion_texture {
        let tex_id = graph.add_node("texture/sample", [tex_x, tex_y]);
        if let Some(node) = graph.nodes.iter_mut().find(|n| n.id == tex_id) {
            node.input_values
                .insert("texture".into(), PinValue::TexturePath(tex_path.clone()));
        }
        // glTF spec: occlusion lives in R, never modulates the other channels.
        graph.connections.push(Connection {
            from_node: tex_id,
            from_pin: "r".into(),
            to_node: output_id,
            to_pin: "ao".into(),
        });
        tex_y += tex_dy;
    }

    // Spec-gloss → roughness routing. We sample the spec-gloss texture and
    // feed its alpha channel through a `1 - x` node into the roughness pin,
    // so wet cobblestone / glass / polished metal recover their per-pixel
    // reflectivity instead of getting one uniform roughness.
    if let Some(ref tex_path) = inputs.specular_glossiness_texture {
        let tex_id = graph.add_node("texture/sample", [tex_x, tex_y]);
        if let Some(node) = graph.nodes.iter_mut().find(|n| n.id == tex_id) {
            node.input_values
                .insert("texture".into(), PinValue::TexturePath(tex_path.clone()));
        }
        let inv_id = graph.add_node("math/one_minus", [tex_x + 90.0, tex_y]);
        graph.connections.push(Connection {
            from_node: tex_id,
            from_pin: "a".into(),
            to_node: inv_id,
            to_pin: "value".into(),
        });
        graph.connections.push(Connection {
            from_node: inv_id,
            from_pin: "result".into(),
            to_node: output_id,
            to_pin: "roughness".into(),
        });
    }

    graph
}

/// One-shot helper: PBR inputs → serialized `.material` JSON. The resolver
/// reads JSON via `serde_json::from_str::<MaterialGraph>`, so any change to
/// the on-disk format must agree with that call site.
pub fn pbr_to_json(inputs: &PbrInputs) -> Result<String, serde_json::Error> {
    let graph = build_surface_graph(inputs);
    serde_json::to_string_pretty(&graph)
}
