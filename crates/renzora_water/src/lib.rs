//! FFT ocean waves.
//!
//! The surface is a sum of **wave cascades**, each an independent simulation of
//! a JONSWAP/TMA ocean spectrum over its own tile. Each frame the GPU
//! propagates the spectrum in time and inverse-Fourier-transforms it into a
//! displacement map and a normal/foam map ([`sim`]); the water material
//! displaces its vertices by those maps and shades them ([`material`],
//! `water.wgsl`). Foam comes from the Jacobian of the displacement — it appears
//! where the surface folds over itself, i.e. where waves actually break.
//!
//! Ported from [GodotOceanWaves](https://github.com/2Retr0/GodotOceanWaves)
//! (MIT). The per-file headers record where each kernel deviates from the
//! original and why.
//!
//! Buoyancy does not read the GPU maps back. [`heightfield`] recomputes the
//! same sea at low resolution on the CPU, which is both cheaper than a readback
//! and the only option on a headless server.
//!
//! Only the `Buoyant` marker is a plain component; `buoyancy::apply_buoyancy`
//! is gated behind `physics` so a no-physics lean export drops `avian3d`.
pub mod buoyancy;
pub mod component;
pub mod heightfield;
pub mod material;
pub mod mesh;
pub mod sim;
pub mod systems;
pub mod world_wind;

use bevy::asset::embedded_asset;
use bevy::prelude::*;

pub use buoyancy::Buoyant;
pub use component::{
    WaterMeshMode, WaterMeshQuality, WaterPreset, WaterSurface, WaveCascade, MAX_CASCADES,
};
pub use heightfield::WaterHeightField;
pub use material::WaterMaterial;

#[derive(Default)]
pub struct WaterPlugin;

impl Plugin for WaterPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] WaterPlugin");

        embedded_asset!(app, "water.wgsl");
        embedded_asset!(app, "shaders/water_spectrum.wgsl");
        embedded_asset!(app, "shaders/water_modulate.wgsl");
        embedded_asset!(app, "shaders/water_butterfly.wgsl");
        embedded_asset!(app, "shaders/water_fft.wgsl");
        embedded_asset!(app, "shaders/water_transpose.wgsl");
        embedded_asset!(app, "shaders/water_unpack.wgsl");

        app.add_plugins((material::WaterMaterialPlugin, sim::WaterSimPlugin))
            .init_resource::<systems::WaterSimState>()
            .init_resource::<heightfield::WaterHeightField>()
            .register_type::<component::WaterSurface>()
            // Nested types have to be registered too, or reflection-based
            // scene save/load can't walk into the cascade list.
            .register_type::<component::WaveCascade>()
            .register_type::<component::WaterMeshMode>()
            .register_type::<component::WaterMeshQuality>()
            .register_type::<buoyancy::Buoyant>()
            .register_type::<world_wind::WaterWindBaseline>()
            .add_systems(
                Update,
                (
                    // Chained: the textures must exist before a material can
                    // point at them, and the simulation clock must advance
                    // before the height field samples it.
                    // Before everything else: this rewrites the cascades, and
                    // the spectrum signature downstream is what decides
                    // whether they need re-baking.
                    world_wind::apply_world_wind,
                    systems::ensure_cascade_textures,
                    systems::setup_water_entities,
                    systems::drive_water_simulation,
                    systems::follow_camera_with_clipmap,
                    systems::update_water_uniforms,
                    systems::update_water_heightfield,
                )
                    .chain(),
            );

        // Applying buoyancy forces needs avian — only when `physics` is built.
        #[cfg(feature = "physics")]
        app.add_systems(
            Update,
            buoyancy::apply_buoyancy.after(systems::update_water_heightfield),
        );
    }
}

renzora::add!(WaterPlugin);
