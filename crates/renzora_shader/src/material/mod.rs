pub mod graph;
pub mod nodes;
pub mod codegen;
pub mod surface_ext;
pub mod runtime;
pub mod material_ref;
pub mod resolver;
pub mod pbr_build;
pub mod standard_build;
pub mod instance;

// Re-export the public asset type at module root so downstream code can write
// `material::GraphMaterial` the same way it did before this rewrite.
pub use surface_ext::GraphMaterial;

use bevy::prelude::*;

pub struct MaterialPlugin;

impl Plugin for MaterialPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] MaterialPlugin");
        app.add_plugins(runtime::GraphMaterialPlugin);
        app.add_plugins(resolver::MaterialResolverPlugin);
        // Importers emit `PbrMaterialExtracted` per material pulled out of
        // a source file. Turn each event into a `.material` graph file on
        // disk so the resolver can later load it and the material editor
        // can open it as a node graph.
        app.add_observer(on_pbr_material_extracted);
    }
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
        emissive_texture: ev.emissive_texture.clone(),
        occlusion_texture: ev.occlusion_texture.clone(),
        specular_glossiness_texture: ev.specular_glossiness_texture.clone(),
        alpha_mode,
        double_sided: ev.double_sided,
    };

    match pbr_build::pbr_to_json(&inputs) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                warn!("[material] write '{}': {}", path.display(), e);
            } else {
                info!("[material] wrote {}", path.display());
            }
        }
        Err(e) => warn!("[material] serialize '{}': {}", ev.name, e),
    }
}
