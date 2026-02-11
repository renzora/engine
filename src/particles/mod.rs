//! Bevy Hanabi particle effects integration
//!
//! This module provides:
//! - Data structures for effect definitions
//! - Effect builder to convert definitions to bevy_hanabi assets
//! - Runtime sync systems
//! - Asset loader for .effect files
//!
//! **Note:** Particles render in the transparent pass, which runs after Solari's lighting
//! pass thanks to a patched render graph edge in `crates/bevy_solari/`. Particles will
//! render correctly in both standard PBR mode and when `SolariLighting` is active.

mod builder;
mod data;
mod systems;

pub use builder::*;
pub use data::*;
pub use systems::*;

use bevy::prelude::*;
use bevy_hanabi::HanabiPlugin;

use crate::core::AppState;

/// Plugin for the particle effects system
pub struct ParticlesPlugin;

impl Plugin for ParticlesPlugin {
    fn build(&self, app: &mut App) {
        // Add bevy_hanabi plugin
        app.add_plugins(HanabiPlugin);

        // Initialize resources
        app.init_resource::<ParticleEditorState>();
        app.init_resource::<ParticlePreviewState>();

        // Register types for reflection/serialization
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
            .register_type::<FlipbookSettings>();

        // Systems that run in editor state
        app.add_systems(
            Update,
            (
                hot_reload_saved_effects,
                sync_hanabi_effects,
                apply_runtime_overrides,
            )
                .chain()
                .run_if(in_state(AppState::Editor)),
        );
    }
}
