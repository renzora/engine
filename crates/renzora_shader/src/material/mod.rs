pub mod graph;
pub mod nodes;
pub mod codegen;
pub mod surface_ext;
pub mod runtime;
pub mod material_ref;
pub mod resolver;

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
    }
}
