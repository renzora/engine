// The `Buoyant` marker is a plain component (queried by the water-interaction
// shader uniforms), so the module stays compiled. Only `buoyancy::apply_buoyancy`
// applies avian forces — that system (and the avian import) is gated behind
// `physics`, so a no-physics lean export drops `avian3d`.
pub mod buoyancy;
pub mod component;
pub mod material;
pub mod mesh;
pub mod systems;

use bevy::asset::embedded_asset;
use bevy::prelude::*;

pub use buoyancy::Buoyant;
pub use component::{GerstnerWave, WaterInteractor, WaterPreset, WaterSurface};
pub use material::WaterMaterial;

#[derive(Default)]
pub struct WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] WaterPlugin");

        embedded_asset!(app, "water.wgsl");

        app.add_plugins(material::WaterMaterialPlugin)
            .register_type::<component::WaterSurface>()
            .register_type::<component::WaterInteractor>()
            .register_type::<buoyancy::Buoyant>()
            .add_systems(
                Update,
                (
                    systems::ensure_depth_prepass,
                    systems::setup_water_entities,
                    systems::update_water_uniforms,
                ),
            );

        // Applying buoyancy forces needs avian — only when `physics` is built.
        #[cfg(feature = "physics")]
        app.add_systems(Update, buoyancy::apply_buoyancy);
    }
}

renzora::add!(WaterPlugin);
