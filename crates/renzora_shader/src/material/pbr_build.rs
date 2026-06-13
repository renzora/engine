//! Build a [`MaterialGraph`] from plain PBR factors + texture paths and
//! serialize it to the on-disk `.material` JSON format.
//!
//! Importers (renzora_import_ui dialog, viewport drop pipeline) emit
//! `renzora::PbrMaterialExtracted` events with the per-material PBR data
//! they pulled out of the source file. The observer below converts each
//! event into a graph file the rest of the editor can load and edit.

use crate::material::graph::{AlphaMode, Connection, MaterialDomain, MaterialGraph, NodeId, PinValue};

/// Add a `texture/sample` node bound to `tex_path` and wire its `channel`
/// output pin into `to_pin` on the output node. Shared by the advanced-channel
/// branches (clearcoat, transmission, thickness, anisotropy) which all follow
/// the same one-channel-into-one-scalar-pin shape.
fn sample_into(
    graph: &mut MaterialGraph,
    tex_path: &str,
    channel: &str,
    output_id: NodeId,
    to_pin: &str,
    pos: [f32; 2],
) {
    let tex_id = graph.add_node("texture/sample", pos);
    if let Some(node) = graph.nodes.iter_mut().find(|n| n.id == tex_id) {
        node.input_values
            .insert("texture".into(), PinValue::TexturePath(tex_path.to_string()));
    }
    graph.connect(tex_id, channel, output_id, to_pin);
}

/// Build an `output/unlit` graph (KHR_materials_unlit / FBX "constant" shading).
/// Unlit bypasses lighting, so only `color` + `alpha` matter.
fn build_unlit_graph(inputs: &PbrInputs) -> MaterialGraph {
    let mut graph = MaterialGraph::new(&inputs.name, MaterialDomain::Unlit);
    graph.alpha_mode = inputs.alpha_mode;
    graph.double_sided = inputs.double_sided;
    let output_id = graph.nodes.first().map(|n| n.id).unwrap_or(1);

    if let Some(out) = graph.nodes.iter_mut().find(|n| n.id == output_id) {
        out.input_values
            .insert("color".into(), PinValue::Color(inputs.base_color));
        out.input_values
            .insert("alpha".into(), PinValue::Float(inputs.base_color[3]));
    }

    if let Some(ref tex_path) = inputs.base_color_texture {
        let tex_id = graph.add_node("texture/sample", [-160.0, -120.0]);
        if let Some(node) = graph.nodes.iter_mut().find(|n| n.id == tex_id) {
            node.input_values
                .insert("texture".into(), PinValue::TexturePath(tex_path.clone()));
        }
        graph.connect(tex_id, "color", output_id, "color");
        if inputs.opacity_texture.is_none() {
            graph.connect(tex_id, "a", output_id, "alpha");
        }
    }
    if let Some(ref tex_path) = inputs.opacity_texture {
        sample_into(&mut graph, tex_path, "r", output_id, "alpha", [-160.0, 40.0]);
    }
    graph
}

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
    /// Standalone roughness map (`r` → roughness); used only when
    /// `metallic_roughness_texture` is absent.
    pub roughness_texture: Option<String>,
    /// Standalone metallic map (`r` → metallic).
    pub metallic_texture: Option<String>,
    pub emissive_texture: Option<String>,
    /// Ambient occlusion (R channel only).
    pub occlusion_texture: Option<String>,
    /// glTF spec-gloss `specularGlossinessTexture` (alpha = per-pixel
    /// glossiness). The graph builder routes `1 - alpha` into the
    /// `roughness` pin so wet stones / glass survive the spec-gloss →
    /// metal-rough conversion with their per-pixel reflectivity intact.
    pub specular_glossiness_texture: Option<String>,
    /// Standalone opacity mask → `alpha` pin. When set, the base-color
    /// texture's own alpha channel is NOT wired to alpha (this mask wins).
    pub opacity_texture: Option<String>,
    /// Standalone specular/reflectivity mask → `metallic` pin plus an
    /// inverted copy into `roughness`.
    pub specular_texture: Option<String>,
    /// Extended PBR channels (clearcoat, transmission, anisotropy, ior, …).
    pub advanced: renzora::core::PbrAdvanced,
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
    // Unlit materials (KHR_materials_unlit) use a different output node and
    // skip every lit channel, so branch off before building the surface graph.
    if inputs.advanced.unlit {
        return build_unlit_graph(inputs);
    }

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

        // Seed the extended-PBR pins from their factors. Sources that don't
        // author a channel pass `PbrAdvanced::default()`, whose values mirror
        // the glTF spec defaults, so this is a no-op for plain metal-rough
        // materials (e.g. ior stays 1.5, clearcoat stays 0).
        let a = &inputs.advanced;
        out.input_values
            .insert("clearcoat".into(), PinValue::Float(a.clearcoat));
        out.input_values.insert(
            "clearcoat_roughness".into(),
            PinValue::Float(a.clearcoat_roughness),
        );
        out.input_values.insert(
            "specular_transmission".into(),
            PinValue::Float(a.specular_transmission),
        );
        out.input_values.insert(
            "diffuse_transmission".into(),
            PinValue::Float(a.diffuse_transmission),
        );
        out.input_values
            .insert("thickness".into(), PinValue::Float(a.thickness));
        out.input_values.insert("ior".into(), PinValue::Float(a.ior));
        out.input_values.insert(
            "attenuation_distance".into(),
            PinValue::Float(a.attenuation_distance),
        );
        out.input_values.insert(
            "anisotropy_strength".into(),
            PinValue::Float(a.anisotropy_strength),
        );
        out.input_values.insert(
            "anisotropy_rotation".into(),
            PinValue::Float(a.anisotropy_rotation),
        );
        // `reflectance` is a Vec3 pin; broadcast the scalar specular factor.
        out.input_values.insert(
            "reflectance".into(),
            PinValue::Vec3([a.reflectance, a.reflectance, a.reflectance]),
        );
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
        // textures (glass, foliage cutouts) actually punch through — unless a
        // standalone opacity mask is present, in which case that mask drives
        // alpha instead (see below) and would otherwise conflict on the pin.
        if inputs.opacity_texture.is_none() {
            graph.connections.push(Connection {
                from_node: tex_id,
                from_pin: "a".into(),
                to_node: output_id,
                to_pin: "alpha".into(),
            });
        }
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

    // Standalone roughness / metallic maps (OBJ map_Pr/map_Pm, USD). Only used
    // when the packed metallic-roughness texture isn't present, so they never
    // double-drive a pin.
    if inputs.metallic_roughness_texture.is_none() {
        if let Some(ref tex_path) = inputs.roughness_texture {
            sample_into(&mut graph, tex_path, "r", output_id, "roughness", [tex_x, tex_y]);
            tex_y += tex_dy;
        }
        if let Some(ref tex_path) = inputs.metallic_texture {
            sample_into(&mut graph, tex_path, "r", output_id, "metallic", [tex_x, tex_y]);
            tex_y += tex_dy;
        }
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

    // Standalone opacity/alpha mask → `alpha`. Legacy FBX transparency maps
    // (cloud shells, decals) carry the cutout in a dedicated grayscale image
    // rather than the base-color alpha channel, so sample its `r` into the
    // alpha pin. The base-color block above already skipped its own alpha
    // wiring when this is present, so there's no conflict on the pin.
    if let Some(ref tex_path) = inputs.opacity_texture {
        let tex_id = graph.add_node("texture/sample", [tex_x, tex_y]);
        if let Some(node) = graph.nodes.iter_mut().find(|n| n.id == tex_id) {
            node.input_values
                .insert("texture".into(), PinValue::TexturePath(tex_path.clone()));
        }
        graph.connections.push(Connection {
            from_node: tex_id,
            from_pin: "r".into(),
            to_node: output_id,
            to_pin: "alpha".into(),
        });
        tex_y += tex_dy;
    }

    // Standalone specular/reflectivity mask → `metallic` + inverse `roughness`.
    // Pre-PBR specular/reflection maps (ocean masks, polished trims) have no
    // metal-rough equivalent; the closest physical reading is "bright = smooth
    // and reflective, dark = rough and matte". We route `r` straight into
    // metallic and `1 - r` into roughness so water reads as reflective and land
    // as diffuse from a single grayscale mask.
    if let Some(ref tex_path) = inputs.specular_texture {
        let tex_id = graph.add_node("texture/sample", [tex_x, tex_y]);
        if let Some(node) = graph.nodes.iter_mut().find(|n| n.id == tex_id) {
            node.input_values
                .insert("texture".into(), PinValue::TexturePath(tex_path.clone()));
        }
        graph.connections.push(Connection {
            from_node: tex_id,
            from_pin: "r".into(),
            to_node: output_id,
            to_pin: "metallic".into(),
        });
        let inv_id = graph.add_node("math/one_minus", [tex_x + 90.0, tex_y]);
        graph.connections.push(Connection {
            from_node: tex_id,
            from_pin: "r".into(),
            to_node: inv_id,
            to_pin: "value".into(),
        });
        graph.connections.push(Connection {
            from_node: inv_id,
            from_pin: "result".into(),
            to_node: output_id,
            to_pin: "roughness".into(),
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

    // Extended-PBR texture maps. Each samples one channel into its scalar pin,
    // matching the glTF KHR_materials_* channel conventions. The factor pins
    // were already seeded above; a connected texture overrides them.
    let adv = &inputs.advanced;
    if let Some(ref tex_path) = adv.clearcoat_texture {
        sample_into(&mut graph, tex_path, "r", output_id, "clearcoat", [tex_x, tex_y]);
        tex_y += tex_dy;
    }
    if let Some(ref tex_path) = adv.clearcoat_roughness_texture {
        // glTF packs clearcoat roughness in the G channel.
        sample_into(
            &mut graph,
            tex_path,
            "g",
            output_id,
            "clearcoat_roughness",
            [tex_x, tex_y],
        );
        tex_y += tex_dy;
    }
    if let Some(ref tex_path) = adv.transmission_texture {
        // KHR_materials_transmission: transmission in the R channel.
        sample_into(
            &mut graph,
            tex_path,
            "r",
            output_id,
            "specular_transmission",
            [tex_x, tex_y],
        );
        tex_y += tex_dy;
    }
    if let Some(ref tex_path) = adv.thickness_texture {
        // KHR_materials_volume: thickness in the G channel.
        sample_into(&mut graph, tex_path, "g", output_id, "thickness", [tex_x, tex_y]);
        tex_y += tex_dy;
    }
    if let Some(ref tex_path) = adv.anisotropy_texture {
        // KHR_materials_anisotropy packs direction in RG and strength in B.
        // We only have a scalar strength pin, so route B; the directional
        // component would need a dedicated tangent-rotation node.
        sample_into(
            &mut graph,
            tex_path,
            "b",
            output_id,
            "anisotropy_strength",
            [tex_x, tex_y],
        );
    }

    graph
}

/// PBR inputs → constructed `MaterialGraph`. Callers serialize via
/// [`super::precompiled::to_json_with_compile`] so the saved file carries
/// an embedded WGSL artifact alongside the graph.
pub fn pbr_to_graph(inputs: &PbrInputs) -> MaterialGraph {
    build_surface_graph(inputs)
}
