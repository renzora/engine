pub mod codegen;
pub mod graph;
pub mod instance;
pub mod material_ref;
pub mod nodes;
pub mod pbr_build;
// Moved to the contract crate so the Material Resolver panel could become a
// native plugin: a plugin links `bevy`, `renzora` and `renzora_ember` and could
// never have named a type in here. Collection still happens in `resolver.rs`;
// only the vocabulary moved. Re-exported under the old path so every existing
// `material::perf::…` still resolves.
pub use renzora::diagnostics::material as perf;
pub mod precompiled;
pub mod resolver;
pub mod runtime;
pub mod standard_build;
pub mod surface_ext;
pub mod texture_slots;
// Problems-panel validation: a naga_oil composer over all of bevy_pbr's
// modules plus a naga pass per material. A shipped game has no Problems
// panel, so it never pays for this.
#[cfg(feature = "editor")]
pub mod validate;

// Re-export the public asset type at module root so downstream code can write
// `material::GraphMaterial` the same way it did before this rewrite.
pub use surface_ext::GraphMaterial;

use bevy::prelude::*;

#[derive(Default)]
pub struct MaterialPlugin;

impl Plugin for MaterialPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] MaterialPlugin");
        app.add_plugins(runtime::GraphMaterialPlugin);
        app.add_plugins(resolver::MaterialResolverPlugin);
        // Authoring-only, so it follows `validate.rs` behind the same feature:
        // nothing in a shipped game asks what nodes exist, and the catalogue is
        // 159 entries of strings built at startup to answer that question.
        // Unlike the validator this needs no runtime `EditorSession` gate,
        // because being wrong costs a little memory rather than a wrong result.
        #[cfg(feature = "editor")]
        publish_node_catalogue(app);
        // Importers emit `PbrMaterialExtracted` per material pulled out of
        // a source file. Turn each event into a `.material` graph file on
        // disk so the resolver can later load it and the material editor
        // can open it as a node graph.
        app.add_observer(on_pbr_material_extracted);
    }
}

/// Copy [`nodes::ALL_NODES`] into the contract crate's catalogue, as data.
///
/// The definitions cannot leave this crate: a `PinTemplate` carries a
/// `PinValue`, which is the compiler's own vocabulary, and moving that chain
/// into `renzora` would end with the shader compiler in the contract crate. But
/// the *description* is answerable without any of it, and something has to
/// answer it: a native plugin links `bevy`, `renzora` and `renzora_ember`, so
/// without this there is no way for one to find out that
/// `procedural/noise_fbm` exists, let alone what its pins are called. The
/// alternative was every such consumer keeping its own copy of a 159-entry list
/// and discovering it had drifted when a graph failed to compile.
///
/// Once, at startup. The list is `&'static` and nothing adds to it at runtime.
#[cfg(feature = "editor")]
fn publish_node_catalogue(app: &mut App) {
    use renzora::core::material_nodes::{
        MaterialNodeCatalog, MaterialNodeDefinition, MaterialNodePin,
    };

    let entries: Vec<MaterialNodeDefinition> = nodes::ALL_NODES
        .iter()
        .map(|def| MaterialNodeDefinition {
            node_type: def.node_type.to_string(),
            display_name: def.display_name.to_string(),
            category: def.category.to_string(),
            description: def.description.to_string(),
            pins: (def.pins)()
                .into_iter()
                .map(|pin| MaterialNodePin {
                    name: pin.name,
                    label: pin.label,
                    pin_type: format!("{:?}", pin.pin_type),
                    input: matches!(pin.direction, graph::PinDir::Input),
                })
                .collect(),
        })
        .collect();

    app.init_resource::<MaterialNodeCatalog>();
    app.world_mut()
        .resource_mut::<MaterialNodeCatalog>()
        .set(entries);
}

/// Observer: write a `.material` JSON file for each emitted
/// [`renzora::PbrMaterialExtracted`] event. Format must match what
/// `resolver::resolve_graph_material` parses (`serde_json::from_str::<MaterialGraph>`).
/// Failures are logged; the observer never panics.
fn on_pbr_material_extracted(trigger: On<renzora::PbrMaterialExtracted>) {
    let ev = trigger.event();
    if let Err(e) = std::fs::create_dir_all(&ev.output_dir) {
        warn!(
            "[material] failed to create materials dir '{}': {}",
            ev.output_dir.display(),
            e
        );
        return;
    }

    let safe_name: String = ev
        .name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let file_name = if safe_name.is_empty() {
        "material".to_string()
    } else {
        safe_name
    };
    let path = ev.output_dir.join(format!("{}.material", file_name));

    use crate::material::graph::AlphaMode as GraphAlpha;
    let alpha_mode = match ev.alpha_mode {
        renzora::core::PbrAlphaMode::Opaque => GraphAlpha::Opaque,
        renzora::core::PbrAlphaMode::Mask => GraphAlpha::Mask {
            cutoff: ev.alpha_cutoff,
        },
        renzora::core::PbrAlphaMode::Blend => GraphAlpha::Blend,
    };

    let inputs = pbr_build::PbrInputs {
        name: ev.name.clone(),
        base_color: ev.base_color,
        metallic: ev.metallic,
        roughness: ev.roughness,
        emissive: ev.emissive,
        base_color_texture: ev.base_color_texture.clone(),
        normal_texture: ev.normal_texture.clone(),
        metallic_roughness_texture: ev.metallic_roughness_texture.clone(),
        roughness_texture: ev.roughness_texture.clone(),
        metallic_texture: ev.metallic_texture.clone(),
        emissive_texture: ev.emissive_texture.clone(),
        occlusion_texture: ev.occlusion_texture.clone(),
        specular_glossiness_texture: ev.specular_glossiness_texture.clone(),
        opacity_texture: ev.opacity_texture.clone(),
        specular_texture: ev.specular_texture.clone(),
        advanced: ev.advanced.clone(),
        alpha_mode,
        double_sided: ev.double_sided,
    };

    let mut graph = pbr_build::pbr_to_graph(&inputs);
    match precompiled::save_compiled_and_serialize(&mut graph, &path) {
        Ok((json, report)) => {
            for err in &report.errors {
                warn!("[material] codegen '{}': {}", ev.name, err);
            }
            for warning in &report.warnings {
                warn!("[material] codegen warning '{}': {}", ev.name, warning);
            }
            if let Err(e) = std::fs::write(&path, json) {
                warn!("[material] write '{}': {}", path.display(), e);
            } else {
                info!("[material] wrote {}", path.display());
            }
        }
        Err(e) => warn!("[material] save '{}': {}", ev.name, e),
    }
}

renzora::add!(MaterialPlugin);
