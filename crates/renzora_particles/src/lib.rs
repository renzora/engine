//! Renzora's own particle system, for 2D and 3D.
//!
//! An effect is a `.vfx` file ([`asset`]): a list of emitters, each with a node
//! graph for its behaviour and a few render settings. The graph is compiled
//! ([`compile`]) into a small backend-neutral IR ([`ir`]): one program for
//! the spawn context, one run once per particle at birth, one run every step.
//! This crate runs that IR on the CPU ([`sim`]), which works on every platform
//! including WebGL2, and draws each emitter as one dynamic mesh whose vertex
//! shader turns quads toward the camera ([`render`]). Large emitters on a
//! device with compute shaders run on the GPU instead ([`gpu`]): the same IR
//! generated into compute kernels, drawn straight from the particle buffer by
//! the same shader. Each emitter's `backend` setting says which, and the CPU
//! is always the fallback.
//!
//! The world wind (`renzora::wind::WindState`) reaches the simulation as a
//! per-frame input that the `force/wind` node turns into drag through moving
//! air; see [`sim`]. The fragment shader can draw procedural flames and smoke
//! instead of a sprite ([`asset::Shading`]), coloured through a blackbody ramp
//! ([`blackbody`]).
//!
//! An emitter can light the scene with real point lights that follow its
//! particles ([`light`]), draw its particles as ribbons ([`render::ribbon`]),
//! bounce them off collision nodes and spawn other emitters' particles from
//! them ([`sim`]). A Collide Scene node bounces them off the scene's real
//! physics colliders instead, through the contract crate's scene query, so no
//! physics engine is linked here ([`scene`]).
//!
//! It replaced the `bevy_hanabi` integration, which has been removed. Effects
//! made for that one still play: [`convert`] turns its `.particle` format into
//! a [`VfxAsset`], and [`legacy`] loads `.particle` files through that and
//! turns a scene's old `HanabiEffect` component into a [`ParticleSystem`] as
//! the scene loads.
//!
//! Scene integration is [`component::ParticleSystem`] (persisted by path) and
//! [`component::ParticlePlayback`]; scripts drive it with the `vfx_*`
//! functions in [`script`]. The editor half (Add Entity preset, inspector,
//! effect editor) is the nested `renzora_particles_editor` crate.

pub mod asset;
pub mod blackbody;
pub mod compile;
pub mod component;
pub mod convert;
pub mod curve;
pub mod gpu;
pub mod ir;
pub mod legacy;
pub mod light;
pub mod noise;
pub mod nodes;
pub mod params;
pub mod presets;
pub mod random;
pub mod render;
pub mod scene;
pub mod script;
pub mod sim;
pub mod systems;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod feature_tests;

#[cfg(test)]
mod parity_tests;

#[cfg(test)]
mod emission_and_order_tests;

pub use asset::{
    Backend, BlendMode, Dimension, DrawOrder, Emitter, Facing, LightMode, Lighting, MeshOrient, ParticleLight, ParticleMesh,
    PinValue, RenderMethod, RenderSettings, Ribbon, RibbonKind, RibbonShape,
    Shading, SimulationSpace, SubEmitter, SubEmitterTrigger, VfxAsset, VfxAssetLoader, VfxGraph, VfxNode,
};
pub use component::{OneShot, ParticleParams, ParticlePlayback, ParticleStats, ParticleSystem, PlayState, DEFAULT_EFFECT};
pub use systems::ParticleRuntime;

use bevy::camera::visibility::VisibilitySystems;
use bevy::prelude::*;
use bevy::transform::TransformSystems;

/// Ordering handle for anything that wants to run around the particle update.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParticleSystems;

#[derive(Default)]
pub struct ParticlesPlugin;

impl Plugin for ParticlesPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] ParticlesPlugin");

        // The asset and the persisted components are registered everywhere,
        // including a dedicated server, so a scene that carries an effect
        // deserializes the same on every build.
        app.init_asset::<VfxAsset>()
            .init_asset_loader::<VfxAssetLoader>()
            .register_type::<ParticleSystem>()
            .register_type::<ParticlePlayback>()
            .register_type::<PlayState>()
            .register_type::<OneShot>();
        presets::embed(app);
        // Old Hanabi effects: the `.particle` loader and the scene component's
        // migration. Before the server early-out for the same reason as the
        // registrations above: a server loads the same scenes.
        legacy::plugin(app);

        {
            let mut extensions = app.world_mut().get_resource_or_insert_with(
                renzora::script_extension::ScriptExtensions::default,
            );
            extensions.register(script::ParticleScriptExtension);
        }
        app.add_observer(script::handle_script_actions);
        // Shared with the render world, which fills it once it has a device;
        // see `gpu::GpuSupport`.
        app.init_resource::<gpu::GpuSupport>();

        // Particles are purely visual: a dedicated server has no render world
        // and nothing to draw them for, so it neither simulates nor renders.
        if app.world().contains_resource::<renzora::DedicatedServer>() {
            return;
        }

        #[cfg(feature = "render_3d")]
        {
            bevy::asset::embedded_asset!(app, "shaders/particle3d.wesl");
            app.add_plugins(bevy::pbr::MaterialPlugin::<render::ParticleMaterial3d>::default());
            app.add_systems(
                PostUpdate,
                (
                    render::sync_motion_rate::<render::ParticleMaterial3d>
                        .after(systems::step_effects)
                        .in_set(ParticleSystems),
                    // After every system that creates a material, so a
                    // deferred request is settled before the render world
                    // extracts it (see `render::material::RenderRoute`).
                    render::material::resolve_render_routes::<render::ParticleMaterial3d>.after(ParticleSystems),
                ),
            );
        }
        // Project materials on particles (`render::surface`), with the
        // engine's material system in the build.
        #[cfg(feature = "material_graph")]
        {
            bevy::asset::embedded_asset!(app, "shaders/particle_surface.wesl");
            render::surface::plugin(app);
        }
        #[cfg(feature = "render_2d")]
        {
            bevy::asset::embedded_asset!(app, "shaders/particle2d.wesl");
            app.add_plugins(
                bevy::sprite_render::Material2dPlugin::<render::ParticleMaterial2d>::default(),
            );
        }

        app.configure_sets(
            PostUpdate,
            ParticleSystems
                .after(TransformSystems::Propagate)
                .before(VisibilitySystems::CheckVisibility),
        );
        app.add_systems(
            PostUpdate,
            (
                systems::resolve_handles,
                systems::sync_instances,
                systems::step_effects,
                systems::write_meshes,
            )
                .chain()
                .in_set(ParticleSystems),
        );
        // Collide Scene: the frame's moves against the scene's colliders, in
        // one batch once every effect has stepped (see `scene`), before the
        // lights and meshes read where the particles ended up.
        app.add_systems(
            PostUpdate,
            scene::collide_with_scene
                .after(systems::step_effects)
                .before(systems::write_meshes)
                .in_set(ParticleSystems),
        );
        #[cfg(feature = "render_3d")]
        app.add_systems(
            PostUpdate,
            systems::update_lights
                .after(scene::collide_with_scene)
                .after(systems::step_effects)
                .before(systems::write_meshes)
                .in_set(ParticleSystems),
        );
        // Mesh particles (3D only): templates and GPU pool meshes, filled in
        // once a model has loaded, before the meshes are written.
        #[cfg(feature = "render_3d")]
        app.init_resource::<render::MeshTemplates>().add_systems(
            PostUpdate,
            render::mesh_particles::resolve_mesh_particles
                .after(systems::step_effects)
                .before(systems::write_meshes)
                .in_set(ParticleSystems),
        );
        // The GPU backend, where it can exist at all: never on wasm, which
        // draws with WebGL2 and has no compute, and only with a renderer.
        #[cfg(all(not(target_arch = "wasm32"), any(feature = "render_3d", feature = "render_2d")))]
        gpu::runtime::plugin(app);
    }
}

renzora::add!(ParticlesPlugin, Runtime);
