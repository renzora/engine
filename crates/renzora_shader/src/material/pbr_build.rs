//! Build a [`MaterialGraph`] from plain PBR factors + texture paths and
//! serialize it to the on-disk `.material` JSON format.
//!
//! Importers (renzora_import_ui dialog, viewport drop pipeline) emit
//! `renzora::PbrMaterialExtracted` events with the per-material PBR data
//! they pulled out of the source file. The observer below converts each
//! event into a graph file the rest of the editor can load and edit.

use crate::material::graph::{Connection, MaterialDomain, MaterialGraph, PinValue};

/// Plain PBR inputs, deliberately a mirror of `renzora_import::ExtractedPbrMaterial`
/// so importers don't need a crate-specific conversion struct.
pub struct PbrInputs {
    pub name: String,
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    /// Asset-relative path to the base-color texture, e.g.
    /// `"models/character/textures/diffuse.png"`.
    pub base_color_texture: Option<String>,
    pub normal_texture: Option<String>,
}

/// Build a surface-domain [`MaterialGraph`] from the given PBR inputs.
///
/// The graph always contains the `output/surface` sink (added by
/// [`MaterialGraph::new`]). Texture-sampling nodes are appended only when
/// the corresponding path is set, with connections from the sampled output
/// pin to the corresponding output input pin. Factor values seed the output
/// pin defaults so they remain in effect even if the user later disconnects
/// a texture.
pub fn build_surface_graph(inputs: &PbrInputs) -> MaterialGraph {
    let mut graph = MaterialGraph::new(&inputs.name, MaterialDomain::Surface);
    let output_id = graph.nodes.first().map(|n| n.id).unwrap_or(1);

    if let Some(out) = graph.nodes.iter_mut().find(|n| n.id == output_id) {
        out.input_values
            .insert("base_color".into(), PinValue::Color(inputs.base_color));
        out.input_values
            .insert("metallic".into(), PinValue::Float(inputs.metallic));
        out.input_values
            .insert("roughness".into(), PinValue::Float(inputs.roughness));
    }

    if let Some(ref tex_path) = inputs.base_color_texture {
        let tex_id = graph.add_node("texture/sample", [-80.0, -60.0]);
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
    }

    if let Some(ref tex_path) = inputs.normal_texture {
        let tex_id = graph.add_node("texture/sample_normal", [-80.0, 80.0]);
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
