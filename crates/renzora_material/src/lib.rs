pub mod graph;
pub mod nodes;
pub mod codegen;
pub mod runtime;
pub mod material_ref;
pub mod resolver;
pub mod pbr_build;

use bevy::prelude::*;

pub struct MaterialPlugin;

impl Plugin for MaterialPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] MaterialPlugin");
        app.add_plugins(runtime::GraphMaterialPlugin);
        app.add_plugins(resolver::MaterialResolverPlugin);
        // Observe the shared extraction event defined in renzora. Any
        // importer that fires `PbrMaterialExtracted` gets a `.material` file
        // written — without the importer knowing this crate exists.
        app.add_observer(on_pbr_material_extracted);
    }
}

/// Turn a [`renzora::PbrMaterialExtracted`] event into a `.material`
/// graph file on disk. Failures are logged; the observer never panics.
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

    let inputs = pbr_build::PbrInputs {
        name: ev.name.clone(),
        base_color: ev.base_color,
        metallic: ev.metallic,
        roughness: ev.roughness,
        base_color_texture: ev.base_color_texture.clone(),
        normal_texture: ev.normal_texture.clone(),
    };

    match pbr_build::pbr_to_ron(&inputs) {
        Ok(ron) => {
            if let Err(e) = std::fs::write(&path, ron) {
                warn!("[material] write '{}': {}", path.display(), e);
            } else {
                info!("[material] wrote {}", path.display());
            }
        }
        Err(e) => warn!("[material] serialize '{}': {}", ev.name, e),
    }
}
