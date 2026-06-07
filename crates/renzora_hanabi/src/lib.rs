//! Bevy Hanabi particle effects integration
//!
//! Provides data structures, effect builder, runtime sync systems,
//! and asset loader for .particle files.

pub mod builder;
pub mod data;
pub mod node_graph;
pub mod systems;

pub use data::*;
pub use systems::{HanabiEffectSynced, ParticleCommand, ParticleCommandQueue};

use bevy::prelude::*;
use bevy_hanabi::HanabiPlugin;

#[derive(Default)]
pub struct HanabiParticlePlugin;

impl Plugin for HanabiParticlePlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] HanabiParticlePlugin");

        // `bevy_hanabi` is GPU-only and unwraps the `RenderApp` in its
        // `finish()`, so it panics on a dedicated server (no render world).
        // Particles are purely visual — skip the GPU plugin and the runtime
        // sync systems on the server. Type registration still runs below so
        // scenes carrying particle components deserialize consistently.
        let headless = app
            .world()
            .contains_resource::<renzora::DedicatedServer>();

        if !headless {
            app.add_plugins(HanabiPlugin);
            app.init_resource::<ParticleCommandQueue>();
        }

        app.init_resource::<ParticleEditorState>();
        app.init_resource::<ParticlePreviewState>();

        app.register_type::<HanabiEffect>()
            .register_type::<HanabiEffectDefinition>()
            .register_type::<EffectSource>()
            .register_type::<HanabiEmitShape>()
            .register_type::<ShapeDimension>()
            .register_type::<SpawnMode>()
            .register_type::<VelocityMode>()
            .register_type::<BlendMode>()
            .register_type::<BillboardMode>()
            .register_type::<SimulationSpace>()
            .register_type::<SimulationCondition>()
            .register_type::<GradientStop>()
            .register_type::<CurvePoint>()
            .register_type::<EffectVariable>()
            .register_type::<ParticleAlphaMode>()
            .register_type::<ParticleOrientMode>()
            .register_type::<MotionIntegrationMode>()
            .register_type::<ParticleColorBlendMode>()
            .register_type::<KillZone>()
            .register_type::<ConformToSphere>()
            .register_type::<OrbitSettings>()
            .register_type::<FlipbookSettings>()
            .register_type::<node_graph::PinType>()
            .register_type::<node_graph::PinDir>()
            .register_type::<node_graph::PinValue>()
            .register_type::<node_graph::ParticleNodeType>()
            .register_type::<node_graph::ParticleNode>()
            .register_type::<node_graph::NodeConnection>()
            .register_type::<node_graph::ParticleNodeGraph>();

        // Sync systems drive `bevy_hanabi` assets/components, which only exist
        // when the GPU plugin is present.
        if !headless {
            app.add_systems(
                PostUpdate,
                (
                    systems::hot_reload_saved_effects,
                    systems::sync_hanabi_effects,
                    systems::apply_runtime_overrides,
                    systems::process_particle_commands,
                )
                    .chain(),
            );
        }
    }
}

renzora::add!(HanabiParticlePlugin);
