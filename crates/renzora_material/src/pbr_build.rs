//! Build a [`MaterialGraph`] from plain PBR parameters and serialize it to
//! RON for writing to a `.material` file.
//!
//! This is the bridge between importers (who only know about PBR factors +
//! texture paths) and the material graph system (which owns the graph
//! format). Keeping the conversion here means importers don't have to know
//! anything about node types, pin names, or graph connections.

use crate::graph::{Connection, MaterialDomain, MaterialGraph, PinValue};

/// Plain PBR inputs, deliberately a mirror of `renzora_import::ExtractedPbrMaterial`
/// so callers don't need to convert between crate-specific structs.
pub struct PbrInputs {
    pub name: String,
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    /// Asset-relative path to the base color texture, e.g.
    /// `"models/character/textures/diffuse.png"`.
    pub base_color_texture: Option<String>,
    pub normal_texture: Option<String>,
}

/// Build a surface-domain [`MaterialGraph`] from the given PBR inputs. The
/// graph always contains an `output/surface` sink; texture-sampling nodes
/// are added only when the corresponding path is set, with connections from
/// the sampled color to the output pin.
pub fn build_surface_graph(inputs: &PbrInputs) -> MaterialGraph {
    let mut graph = MaterialGraph::new(&inputs.name, MaterialDomain::Surface);
    // `MaterialGraph::new` always inserts the output node at id=1.
    let output_id = graph.nodes.first().map(|n| n.id).unwrap_or(1);

    // Seed the output pin defaults with the factors. These stick even if the
    // user later disconnects a texture node.
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

/// Serialize a `MaterialGraph` to the on-disk `.material` RON format. Uses
/// pretty-printing so the file is human-readable and diff-friendly.
pub fn graph_to_ron(graph: &MaterialGraph) -> Result<String, ron::Error> {
    ron::ser::to_string_pretty(graph, ron::ser::PrettyConfig::default().struct_names(false))
}

/// One-shot helper: PBR inputs → serialized `.material` RON.
pub fn pbr_to_ron(inputs: &PbrInputs) -> Result<String, ron::Error> {
    let graph = build_surface_graph(inputs);
    graph_to_ron(&graph)
}
