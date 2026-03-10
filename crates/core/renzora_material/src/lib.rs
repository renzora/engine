pub mod graph;
pub mod nodes;
pub mod codegen;
pub mod runtime;

use bevy::prelude::*;

pub struct MaterialPlugin;

impl Plugin for MaterialPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] MaterialPlugin");
        app.add_plugins(runtime::GraphMaterialPlugin);
    }
}
