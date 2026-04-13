pub mod buoyancy;
pub mod component;
pub mod material;
pub mod mesh;
pub mod systems;

use bevy::prelude::*;
use bevy::asset::embedded_asset;

pub use buoyancy::Buoyant;
pub use component::{GerstnerWave, WaterSurface, WaterInteractor, WaterPreset};
pub use material::WaterMaterial;

pub struct WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] WaterPlugin");

        embedded_asset!(app, "water.wgsl");

        app.add_plugins(material::WaterMaterialPlugin)
            .register_type::<component::WaterSurface>()
            .register_type::<component::WaterInteractor>()
            .register_type::<buoyancy::Buoyant>()
            .add_systems(Update, (
                systems::ensure_depth_prepass,
                systems::setup_water_entities,
                systems::update_water_uniforms,
                buoyancy::apply_buoyancy,
            ));

        #[cfg(feature = "editor")]
        {
            use renzora_editor_framework::AppEditorExt;
            app.register_inspector(component::water_inspector_entry());
            app.register_inspector(component::water_interactor_inspector_entry());
            app.register_inspector(buoyancy::buoyant_inspector_entry());
        }
    }
}
