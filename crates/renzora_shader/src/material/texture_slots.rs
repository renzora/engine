//! PBR texture slots — the "drop a normal map here" view of a material graph.
//!
//! A material's textures live in the node graph, wired sampler → output pin.
//! That is the right representation for authoring and the wrong one for the
//! common case, which is an artist with six PNGs who wants them on the mesh.
//! This module is the bridge: each [`TextureSlot`] names an output-node pin,
//! the sampler node type that drives it, and which channel of that sampler
//! carries the data. Reading a slot traces the pin back one hop; writing one
//! creates (or reuses) the sampler and connects it.
//!
//! There is deliberately no parallel store of "the material's textures" — a
//! slot is a *view*, so a drop in the inspector and a wire dragged in the graph
//! editor cannot disagree, and a graph hand-authored into a shape the slots
//! can't express simply reads back as empty rather than as something wrong.
//!
//! ## Channel choices
//!
//! The channels below are not arbitrary: they match what `pbr_build` emits for
//! imported glTF and what [`standard_build`](super::standard_build) recognises
//! as a *trivial* graph (roughness ← `g`, metallic ← `b`, AO ← `r`, i.e. the
//! glTF ORM packing). Dropping one packed ORM map onto all three slots reuses a
//! single sampler node, which is exactly the shape that compiles to a plain
//! `StandardMaterial` instead of a specialized pipeline. A dedicated greyscale
//! map is unaffected — its channels are equal — so the packing-friendly choice
//! costs nothing in the single-map case.

use std::path::Path;

use super::graph::{MaterialGraph, MaterialNode, NodeId, PinValue};

/// One droppable texture channel of a surface material.
#[derive(Clone, Copy, Debug)]
pub struct TextureSlot {
    /// Stable identifier, used by UI code to name a slot across rebuilds.
    pub key: &'static str,
    /// Human label.
    pub label: &'static str,
    /// Phosphor icon name for the empty state.
    pub icon: &'static str,
    /// Input pin on the graph's output node this slot feeds.
    pub pin: &'static str,
    /// Sampler node type that drives the pin.
    pub node_type: &'static str,
    /// Output pin of the sampler that carries this channel's data.
    pub channel: &'static str,
}

/// The slots the material component exposes, in display order. Deliberately the
/// six channels a PBR texture set ships with — everything else is still
/// authorable in the graph editor, which this table never restricts.
pub const TEXTURE_SLOTS: &[TextureSlot] = &[
    TextureSlot {
        key: "base_color",
        label: "Base Color",
        icon: "palette",
        pin: "base_color",
        node_type: "texture/sample",
        channel: "color",
    },
    TextureSlot {
        key: "normal",
        label: "Normal",
        icon: "mountains",
        pin: "normal",
        node_type: "texture/sample_normal",
        channel: "normal",
    },
    TextureSlot {
        key: "roughness",
        label: "Roughness",
        icon: "circle-half",
        pin: "roughness",
        node_type: "texture/sample",
        channel: "g",
    },
    TextureSlot {
        key: "metallic",
        label: "Metallic",
        icon: "diamond",
        pin: "metallic",
        node_type: "texture/sample",
        channel: "b",
    },
    TextureSlot {
        key: "ao",
        label: "Ambient Occlusion",
        icon: "moon",
        pin: "ao",
        node_type: "texture/sample",
        channel: "r",
    },
    TextureSlot {
        key: "emissive",
        label: "Emissive",
        icon: "lightbulb",
        pin: "emissive",
        node_type: "texture/sample",
        channel: "rgb",
    },
];

/// Look a slot up by its [`TextureSlot::key`].
pub fn slot(key: &str) -> Option<&'static TextureSlot> {
    TEXTURE_SLOTS.iter().find(|s| s.key == key)
}

/// Position of a slot in [`TEXTURE_SLOTS`] — also its row in the canvas column
/// new sampler nodes are laid out in.
fn slot_index(slot: &TextureSlot) -> usize {
    TEXTURE_SLOTS
        .iter()
        .position(|s| s.key == slot.key)
        .unwrap_or(0)
}

/// The texture path currently feeding `slot`, if the pin traces back to a
/// sampler node with a texture set.
///
/// Only a direct sampler → pin connection reads back. A pin driven through math
/// or a subgraph is a graph the slots can't represent, and reporting the texture
/// found further upstream would invite a drop that silently rewires it.
pub fn slot_texture(graph: &MaterialGraph, slot: &TextureSlot) -> Option<String> {
    let output_id = graph.output_node()?.id;
    let conn = graph.connection_to(output_id, slot.pin)?;
    let node = graph.get_node(conn.from_node)?;
    if !node.node_type.starts_with("texture/") {
        return None;
    }
    node_texture(node)
}

/// Point `slot` at `texture` (an asset-relative path), creating or reusing the
/// sampler node as needed. Returns `false` only when the graph has no output
/// node to wire into.
pub fn set_slot_texture(graph: &mut MaterialGraph, slot: &TextureSlot, texture: &str) -> bool {
    let Some(output_id) = graph.output_node().map(|n| n.id) else {
        return false;
    };
    let previous = graph.connection_to(output_id, slot.pin).map(|c| c.from_node);
    // Does the pin's current source also drive base color's alpha? Answered
    // before any rewiring, because the answer decides whether alpha follows the
    // new texture or belongs to a separate opacity map that must be left alone.
    let alpha_follows = graph
        .connection_to(output_id, "alpha")
        .map(|c| c.from_node)
        .is_some_and(|n| Some(n) == previous);

    let node_id = match sampler_for(graph, slot, texture, previous, output_id) {
        Some(id) => id,
        None => {
            let pos = free_position(graph, slot_index(slot));
            graph.add_node(slot.node_type, pos)
        }
    };
    if let Some(node) = graph.get_node_mut(node_id) {
        node.input_values.insert(
            "texture".to_string(),
            PinValue::TexturePath(texture.to_string()),
        );
    }
    graph.connect(node_id, slot.channel, output_id, slot.pin);

    // Base color carries opacity: wire the sampler's alpha channel through so a
    // cutout texture punches holes without a second trip to the graph editor.
    // Skipped when a standalone opacity map owns the pin — that one is the
    // artist's explicit choice and outranks the convenience.
    if slot.key == "base_color"
        && (alpha_follows || graph.connection_to(output_id, "alpha").is_none())
    {
        graph.connect(node_id, "a", output_id, "alpha");
    }

    if let Some(prev) = previous.filter(|p| *p != node_id) {
        prune_orphan_sampler(graph, prev);
    }
    true
}

/// Disconnect `slot` and drop the sampler if nothing else used it. Returns
/// `false` when the slot was already empty.
pub fn clear_slot(graph: &mut MaterialGraph, slot: &TextureSlot) -> bool {
    let Some(output_id) = graph.output_node().map(|n| n.id) else {
        return false;
    };
    let Some(previous) = graph.connection_to(output_id, slot.pin).map(|c| c.from_node) else {
        return false;
    };
    disconnect_input(graph, output_id, slot.pin);
    // The alpha wire is base color's passenger (see `set_slot_texture`); it
    // rides along on clear too, or it would keep a removed texture alive.
    if slot.key == "base_color"
        && graph
            .connection_to(output_id, "alpha")
            .is_some_and(|c| c.from_node == previous)
    {
        disconnect_input(graph, output_id, "alpha");
    }
    prune_orphan_sampler(graph, previous);
    true
}

/// Which slots a texture file's name suggests, in the order they should be
/// filled. Empty when nothing matches — callers decide whether to fall back to
/// base color or to refuse the drop.
///
/// A packed map returns several slots, so dropping `rock_ORM.png` on the
/// material fills occlusion, roughness and metallic from one sampler.
pub fn guess_slots(path: &Path) -> Vec<&'static TextureSlot> {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    let tokens = tokenize(stem);
    let has = |set: &[&str]| tokens.iter().any(|t| set.contains(&t.as_str()));

    const NORMAL: &[&str] = &["n", "nrm", "nrml", "nor", "norm", "normal", "normalmap", "bump"];
    const ROUGHNESS: &[&str] = &["r", "rgh", "rough", "roughness"];
    const METALLIC: &[&str] = &["m", "mtl", "metal", "metallic", "metalness"];
    const AO: &[&str] = &["ao", "occ", "occlusion", "ambientocclusion"];
    const EMISSIVE: &[&str] = &["e", "emit", "emissive", "emission", "glow"];
    const BASE_COLOR: &[&str] = &[
        "d", "bc", "col", "color", "colour", "albedo", "diffuse", "basecolor", "base",
    ];

    let by_key = |keys: &[&str]| -> Vec<&'static TextureSlot> {
        keys.iter().filter_map(|k| slot(k)).collect()
    };

    // Packed maps first. `rock_metallicRoughness` tokenizes to two channel words
    // and `rock_ORM` to one initialism; either way the single-channel tests
    // below would claim it for one channel and silently drop the rest.
    const PACKED_ORM: &[&str] = &["orm", "arm", "rma", "mra", "occlusionroughnessmetallic"];
    if has(PACKED_ORM) || (has(ROUGHNESS) && has(METALLIC) && has(AO)) {
        return by_key(&["ao", "roughness", "metallic"]);
    }
    if has(&["mr", "metallicroughness", "roughnessmetallic"])
        || (has(ROUGHNESS) && has(METALLIC))
    {
        return by_key(&["roughness", "metallic"]);
    }

    for (set, key) in [
        (NORMAL, "normal"),
        (AO, "ao"),
        (ROUGHNESS, "roughness"),
        (METALLIC, "metallic"),
        (EMISSIVE, "emissive"),
        (BASE_COLOR, "base_color"),
    ] {
        if has(set) {
            return by_key(&[key]);
        }
    }
    Vec::new()
}

/// Split a file stem into lowercase words on separators, case changes and
/// letter/digit boundaries, so `Rock_ORM`, `rockBaseColor` and `rock-normal-2k`
/// all reduce to comparable tokens.
///
/// Matching whole tokens rather than substrings is the point: `rock_normal`
/// contains the letters `rma`, and a substring test read that as a packed ORM
/// map.
fn tokenize(stem: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut prev: Option<char> = None;
    let chars: Vec<char> = stem.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_ascii_alphanumeric() {
            if !cur.is_empty() {
                tokens.push(std::mem::take(&mut cur));
            }
            prev = None;
            continue;
        }
        // Break `baseColor` between the case change, `ORMMap` before the final
        // capital of a run, and `wood2k` at the letter/digit boundary.
        let camel = prev.is_some_and(|p| p.is_ascii_lowercase() && c.is_ascii_uppercase());
        let acronym_end = prev.is_some_and(|p| p.is_ascii_uppercase())
            && c.is_ascii_uppercase()
            && chars.get(i + 1).is_some_and(|n| n.is_ascii_lowercase());
        let digit_edge = prev.is_some_and(|p| p.is_ascii_digit() != c.is_ascii_digit());
        if !cur.is_empty() && (camel || acronym_end || digit_edge) {
            tokens.push(std::mem::take(&mut cur));
        }
        cur.push(c.to_ascii_lowercase());
        prev = Some(c);
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    tokens
}

// ── Internals ───────────────────────────────────────────────────────────────

/// The texture path set on a sampler node, if any.
fn node_texture(node: &MaterialNode) -> Option<String> {
    match node.input_values.get("texture")? {
        PinValue::TexturePath(p) if !p.is_empty() => Some(p.clone()),
        _ => None,
    }
}

/// Pick the sampler node a `set_slot_texture` should wire from, or `None` to
/// build a fresh one.
fn sampler_for(
    graph: &MaterialGraph,
    slot: &TextureSlot,
    texture: &str,
    previous: Option<NodeId>,
    output_id: NodeId,
) -> Option<NodeId> {
    // A node already sampling this exact file wins — that is what collapses a
    // packed ORM dropped on three slots into one sampler and one texture
    // binding, and it keeps the graph free of duplicate reads of one image.
    if let Some(shared) = graph
        .nodes
        .iter()
        .find(|n| n.node_type == slot.node_type && node_texture(n).as_deref() == Some(texture))
    {
        return Some(shared.id);
    }
    // Otherwise retarget the sampler already on this pin, so replacing a
    // texture keeps the node's canvas position and any UV wiring feeding it —
    // but only when this pin is the sole thing it drives, or the swap would
    // silently change the other pins too.
    let prev = previous?;
    let node = graph.get_node(prev)?;
    if node.node_type != slot.node_type {
        return None;
    }
    let drives_only_this = graph.connections.iter().filter(|c| c.from_node == prev).all(|c| {
        c.to_node == output_id
            && (c.to_pin == slot.pin || (slot.key == "base_color" && c.to_pin == "alpha"))
    });
    drives_only_this.then_some(prev)
}

/// Canvas position for a newly created sampler: a column to the left of the
/// output node, one row per slot, nudged down if something already sits there
/// so a fresh node never lands exactly on top of an existing one.
fn free_position(graph: &MaterialGraph, row: usize) -> [f32; 2] {
    const X: f32 = -160.0;
    const Y0: f32 = -240.0;
    const DY: f32 = 140.0;
    let mut y = Y0 + row as f32 * DY;
    while graph
        .nodes
        .iter()
        .any(|n| (n.position[0] - X).abs() < 20.0 && (n.position[1] - y).abs() < 20.0)
    {
        y += 60.0;
    }
    [X, y]
}

/// Drop every connection into `(node, pin)`. [`MaterialGraph::disconnect`]
/// matches a pin name on *either* end, which would also cut same-named wires
/// leaving the node.
fn disconnect_input(graph: &mut MaterialGraph, node: NodeId, pin: &str) {
    graph
        .connections
        .retain(|c| !(c.to_node == node && c.to_pin == pin));
}

/// Remove a sampler that a rewire left driving nothing. Scoped to `texture/*`
/// nodes so a slot edit can never delete math the user parked in the graph, and
/// skipped whenever anything still reads from the node.
fn prune_orphan_sampler(graph: &mut MaterialGraph, node_id: NodeId) {
    let is_sampler = graph
        .get_node(node_id)
        .is_some_and(|n| n.node_type.starts_with("texture/"));
    if !is_sampler {
        return;
    }
    if graph.connections.iter().any(|c| c.from_node == node_id) {
        return;
    }
    graph.remove_node(node_id);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::graph::MaterialDomain;

    fn graph() -> MaterialGraph {
        MaterialGraph::new("test", MaterialDomain::Surface)
    }

    fn keys(slots: Vec<&TextureSlot>) -> Vec<&str> {
        slots.iter().map(|s| s.key).collect()
    }

    #[test]
    fn set_then_read_round_trips() {
        let mut g = graph();
        let normal = slot("normal").unwrap();
        assert!(set_slot_texture(&mut g, normal, "tex/rock_n.png"));
        assert_eq!(slot_texture(&g, normal).as_deref(), Some("tex/rock_n.png"));
        assert_eq!(
            g.nodes.iter().filter(|n| n.node_type == "texture/sample_normal").count(),
            1
        );
    }

    #[test]
    fn base_color_carries_alpha_and_releases_it_on_clear() {
        let mut g = graph();
        let bc = slot("base_color").unwrap();
        set_slot_texture(&mut g, bc, "tex/wood.png");
        let out = g.output_node().unwrap().id;
        assert!(g.connection_to(out, "alpha").is_some());
        assert!(clear_slot(&mut g, bc));
        assert!(g.connection_to(out, "alpha").is_none());
        assert!(g.nodes.iter().all(|n| n.node_type != "texture/sample"));
    }

    #[test]
    fn one_packed_map_feeds_three_slots_from_one_sampler() {
        let mut g = graph();
        for key in ["ao", "roughness", "metallic"] {
            set_slot_texture(&mut g, slot(key).unwrap(), "tex/rock_orm.png");
        }
        assert_eq!(
            g.nodes.iter().filter(|n| n.node_type == "texture/sample").count(),
            1
        );
        for key in ["ao", "roughness", "metallic"] {
            assert_eq!(
                slot_texture(&g, slot(key).unwrap()).as_deref(),
                Some("tex/rock_orm.png")
            );
        }
    }

    #[test]
    fn replacing_a_texture_leaves_no_orphan_sampler() {
        let mut g = graph();
        let rough = slot("roughness").unwrap();
        set_slot_texture(&mut g, rough, "tex/a.png");
        set_slot_texture(&mut g, rough, "tex/b.png");
        assert_eq!(
            g.nodes.iter().filter(|n| n.node_type == "texture/sample").count(),
            1
        );
        assert_eq!(slot_texture(&g, rough).as_deref(), Some("tex/b.png"));
    }

    #[test]
    fn clearing_one_slot_keeps_a_sampler_another_slot_still_uses() {
        let mut g = graph();
        set_slot_texture(&mut g, slot("roughness").unwrap(), "tex/orm.png");
        set_slot_texture(&mut g, slot("metallic").unwrap(), "tex/orm.png");
        clear_slot(&mut g, slot("roughness").unwrap());
        assert_eq!(
            slot_texture(&g, slot("metallic").unwrap()).as_deref(),
            Some("tex/orm.png")
        );
    }

    #[test]
    fn names_map_to_slots() {
        assert_eq!(keys(guess_slots(Path::new("rock_normal.png"))), ["normal"]);
        assert_eq!(keys(guess_slots(Path::new("rock_nrm.png"))), ["normal"]);
        assert_eq!(keys(guess_slots(Path::new("Rock_N.png"))), ["normal"]);
        assert_eq!(keys(guess_slots(Path::new("wood_Roughness.png"))), ["roughness"]);
        assert_eq!(keys(guess_slots(Path::new("wood_AO.png"))), ["ao"]);
        assert_eq!(keys(guess_slots(Path::new("lava_emissive.png"))), ["emissive"]);
        assert_eq!(keys(guess_slots(Path::new("wood_BaseColor.png"))), ["base_color"]);
        assert_eq!(keys(guess_slots(Path::new("wood_albedo.png"))), ["base_color"]);
        assert_eq!(
            keys(guess_slots(Path::new("rock_ORM.png"))),
            ["ao", "roughness", "metallic"]
        );
        assert_eq!(
            keys(guess_slots(Path::new("rock_metallicRoughness.png"))),
            ["roughness", "metallic"]
        );
        assert!(guess_slots(Path::new("untitled.png")).is_empty());
    }

    #[test]
    fn tokenizer_splits_the_shapes_names_actually_come_in() {
        assert_eq!(tokenize("rock_ORM"), ["rock", "orm"]);
        assert_eq!(tokenize("woodBaseColor"), ["wood", "base", "color"]);
        assert_eq!(tokenize("rock-normal-2k"), ["rock", "normal", "2", "k"]);
        assert_eq!(tokenize("T_Wood_D"), ["t", "wood", "d"]);
        assert_eq!(tokenize("ORMMap"), ["orm", "map"]);
    }
}
