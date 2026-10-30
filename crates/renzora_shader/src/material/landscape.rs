//! What a landscape material says about its layers, read off the graph.
//!
//! A terrain paints weights for named layers, and the names come from the
//! material: every connected layer of a Layer Blend node, then any name a Layer
//! Sample node asks for that no blend uses. This module is the one place that
//! list is worked out, so codegen (which weight a layer reads) and the terrain
//! (which layers its Paint tab offers) can never disagree about it.
//!
//! The order is the order layers first appear, Layer Blend nodes by id and
//! their slots in order, then Layer Sample nodes by id. A layer's position in
//! the list is its *material layer index*, which is what the generated shader
//! indexes its weights by. A terrain maps those indices onto its own weight
//! channels by name (`renzora_terrain::landscape`), so reordering or adding
//! layers in the graph never scrambles what was painted.

use super::graph::{MaterialGraph, MaterialNode, PinValue};
use super::nodes::LANDSCAPE_MAX_LAYERS;

/// How a layer of a Layer Blend node mixes with the others.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LandscapeBlend {
    /// An even mix by weight.
    #[default]
    Weight,
    /// Where it meets another height-blended layer, the taller one (its
    /// weight plus its height input) wins, so the edge follows the textures.
    Height,
    /// Laid over everything else by its weight, after the rest are mixed.
    Alpha,
}

impl LandscapeBlend {
    /// Read the `blend_N` pin's value: a name (any case) or 0 / 1 / 2.
    /// Anything else is Weight, the cheapest and least surprising reading.
    pub fn from_pin(value: Option<&PinValue>) -> Self {
        match value {
            Some(PinValue::String(s)) => match s.trim().to_ascii_lowercase().as_str() {
                "height" => Self::Height,
                "alpha" => Self::Alpha,
                _ => Self::Weight,
            },
            Some(PinValue::Int(1)) => Self::Height,
            Some(PinValue::Int(2)) => Self::Alpha,
            Some(PinValue::Float(f)) if f.round() == 1.0 => Self::Height,
            Some(PinValue::Float(f)) if f.round() == 2.0 => Self::Alpha,
            _ => Self::Weight,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Weight => "Weight",
            Self::Height => "Height",
            Self::Alpha => "Alpha",
        }
    }
}

/// One layer a landscape material declares, with how its first Layer Blend
/// mixes it (Weight for a layer only a Layer Sample names).
#[derive(Clone, Debug, PartialEq)]
pub struct LandscapeLayerInfo {
    pub name: String,
    pub blend: LandscapeBlend,
}

/// The name in slot `slot` of a Layer Blend node, as the editor shows it: the
/// `name_N` pin's value, or "Layer N+1" when it has none.
pub fn blend_slot_name(node: &MaterialNode, slot: usize) -> String {
    match node.input_values.get(&format!("name_{slot}")) {
        Some(PinValue::String(s)) if !s.trim().is_empty() => s.trim().to_string(),
        _ => format!("Layer {}", slot + 1),
    }
}

/// The name a Layer Sample node reads.
pub fn sample_name(node: &MaterialNode) -> String {
    match node.input_values.get("name") {
        Some(PinValue::String(s)) if !s.trim().is_empty() => s.trim().to_string(),
        _ => "Layer 1".to_string(),
    }
}

/// Whether slot `slot` of a Layer Blend node takes part: something is wired
/// into its `layer_N` input.
pub fn blend_slot_active(graph: &MaterialGraph, node: &MaterialNode, slot: usize) -> bool {
    graph.connection_to(node.id, &format!("layer_{slot}")).is_some()
}

/// Every layer the graph declares, in material layer index order, at most
/// [`LANDSCAPE_MAX_LAYERS`]. Empty for a graph with no Landscape nodes.
pub fn landscape_layer_infos(graph: &MaterialGraph) -> Vec<LandscapeLayerInfo> {
    let mut by_id: Vec<&MaterialNode> = graph
        .nodes
        .iter()
        .filter(|n| n.node_type == "landscape/layer_blend" || n.node_type == "landscape/layer_sample")
        .collect();
    by_id.sort_by_key(|n| (n.node_type != "landscape/layer_blend", n.id));

    let mut out: Vec<LandscapeLayerInfo> = Vec::new();
    for node in by_id {
        if node.node_type == "landscape/layer_blend" {
            for slot in 0..LANDSCAPE_MAX_LAYERS {
                if !blend_slot_active(graph, node, slot) {
                    continue;
                }
                let name = blend_slot_name(node, slot);
                if !out.iter().any(|l| l.name == name) {
                    out.push(LandscapeLayerInfo {
                        name,
                        blend: LandscapeBlend::from_pin(node.input_values.get(&format!("blend_{slot}"))),
                    });
                }
            }
        } else {
            let name = sample_name(node);
            if !out.iter().any(|l| l.name == name) {
                out.push(LandscapeLayerInfo { name, blend: LandscapeBlend::Weight });
            }
        }
    }
    out.truncate(LANDSCAPE_MAX_LAYERS);
    out
}

/// Just the names of [`landscape_layer_infos`]: what a terrain's Paint tab
/// lists for this material.
pub fn landscape_layers(graph: &MaterialGraph) -> Vec<String> {
    landscape_layer_infos(graph).into_iter().map(|l| l.name).collect()
}

/// Whether the graph uses any Landscape node, and so wants a terrain's weight
/// maps bound.
pub fn is_landscape_graph(graph: &MaterialGraph) -> bool {
    graph.nodes.iter().any(|n| n.node_type.starts_with("landscape/"))
}

#[cfg(test)]
mod tests {
    use super::super::graph::MaterialDomain;
    use super::*;

    fn named(g: &mut MaterialGraph, id: u64, pin: &str, value: &str) {
        g.get_node_mut(id)
            .unwrap()
            .input_values
            .insert(pin.to_string(), PinValue::String(value.to_string()));
    }

    #[test]
    fn layers_list_connected_blend_slots_then_samples_without_repeats() {
        let mut g = MaterialGraph::new("Land", MaterialDomain::Surface);
        let colour = g.add_node("color/constant", [0.0, 0.0]);
        let blend = g.add_node("landscape/layer_blend", [0.0, 0.0]);
        let second = g.add_node("landscape/layer_blend", [0.0, 0.0]);
        let sample = g.add_node("landscape/layer_sample", [0.0, 0.0]);
        let again = g.add_node("landscape/layer_sample", [0.0, 0.0]);
        named(&mut g, blend, "name_0", "Grass");
        named(&mut g, blend, "name_1", "Unwired");
        named(&mut g, blend, "name_2", "Rock");
        named(&mut g, blend, "blend_2", "height");
        named(&mut g, second, "name_0", "Rock");
        named(&mut g, second, "name_3", "Snow");
        named(&mut g, sample, "name", "Wet");
        named(&mut g, again, "name", "Grass");
        g.connect(colour, "color", blend, "layer_0");
        g.connect(colour, "color", blend, "layer_2");
        g.connect(colour, "color", second, "layer_0");
        g.connect(colour, "color", second, "layer_3");

        let infos = landscape_layer_infos(&g);
        let names: Vec<&str> = infos.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["Grass", "Rock", "Snow", "Wet"]);
        assert_eq!(infos[1].blend, LandscapeBlend::Height);
        assert!(is_landscape_graph(&g));
        assert!(landscape_layers(&MaterialGraph::new("Plain", MaterialDomain::Surface)).is_empty());
    }

    #[test]
    fn blend_types_read_names_and_numbers() {
        assert_eq!(LandscapeBlend::from_pin(Some(&PinValue::String("Alpha".into()))), LandscapeBlend::Alpha);
        assert_eq!(LandscapeBlend::from_pin(Some(&PinValue::Int(1))), LandscapeBlend::Height);
        assert_eq!(LandscapeBlend::from_pin(None), LandscapeBlend::Weight);
        assert_eq!(LandscapeBlend::from_pin(Some(&PinValue::String("nonsense".into()))), LandscapeBlend::Weight);
    }
}
