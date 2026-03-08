//! Bevy Hanabi particle effects integration
//!
//! Provides data structures, effect builder, runtime sync systems,
//! and asset loader for .particle files.

pub mod builder;
pub mod data;
pub mod node_graph;
pub mod systems;

#[cfg(feature = "editor")]
mod inspector;

pub use data::*;
pub use systems::{
    HanabiEffectSynced, ParticleCommand, ParticleCommandQueue,
};

use bevy::prelude::*;
use bevy_hanabi::HanabiPlugin;

pub struct HanabiParticlePlugin;

impl Plugin for HanabiParticlePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HanabiPlugin);

        app.init_resource::<ParticleEditorState>();
        app.init_resource::<ParticlePreviewState>();
        app.init_resource::<ParticleCommandQueue>();

        app.register_type::<HanabiEffectData>()
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

        #[cfg(feature = "editor")]
        inspector::register_inspector(app);
    }
}
