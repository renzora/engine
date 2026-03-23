//! Renzora Animation — runtime animation system built on Bevy's AnimationGraph.
//!
//! Provides:
//! - `.anim` file format (RON) and asset loader
//! - `.animsm` state machine format and asset loader
//! - `AnimatorComponent` for scene-serializable animation clip management
//! - State machines, blend trees, and animation layers
//! - Procedural tweens with easing functions
//! - GLTF animation extraction pipeline
//! - Runtime systems: graph building, playback, script/blueprint command processing
//! - Bridge from ScriptCommandQueue to AnimationCommandQueue

pub mod blend_tree;
pub mod bridge;
pub mod clip;
pub mod component;
pub mod extract;
pub mod graph_builder;
#[cfg(feature = "editor")]
pub mod inspector;
pub mod layers;
pub mod loader;
pub mod sm_loader;
pub mod state_machine;
pub mod systems;
pub mod tween;

pub use blend_tree::BlendTree;
pub use clip::{AnimClip, BoneTrack};
pub use component::{AnimClipSlot, AnimatorComponent, AnimatorState};
pub use layers::{AnimationLayer, LayerBlendMode};
pub use loader::AnimClipLoader;
pub use state_machine::{AnimCondition, AnimParams, AnimState, AnimTransition, AnimationStateMachine, StateMotion};
pub use systems::{AnimationCommand, AnimationCommandQueue};
pub use tween::{EasingFunction, ProceduralTween, TweenProperty};

use bevy::prelude::*;

pub struct AnimationPlugin;

impl Plugin for AnimationPlugin {
    fn build(&self, app: &mut App) {
        info!("[animation] AnimationPlugin");

        app.register_type::<AnimatorComponent>()
            .register_type::<AnimClipSlot>()
            .register_type::<AnimParams>()
            .register_type::<AnimationLayer>()
            .register_type::<LayerBlendMode>()
            .init_asset::<AnimationStateMachine>()
            .init_asset_loader::<AnimClipLoader>()
            .init_asset_loader::<sm_loader::AnimSmLoader>()
            .init_resource::<AnimationCommandQueue>()
            ;

        #[cfg(feature = "editor")]
        {
            use renzora_editor::AppEditorExt;
            app.register_inspector(inspector::animator_inspector_entry());
        }

        app.add_systems(
                Update,
                (
                    bridge::route_script_animation_commands,
                    systems::rehydrate_animators,
                    systems::initialize_animation_graphs,
                    systems::auto_play_default,
                    systems::process_animation_commands,
                    systems::update_state_machines,
                    systems::update_layer_weights,
                    systems::detect_animation_finished,
                    tween::update_procedural_tweens,
                )
                    .chain(),
            );
    }
}
