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
            use renzora_editor_framework::AppEditorExt;
            app.register_inspector(inspector::animator_inspector_entry());
        }

        // Script animation commands (decoupled via ScriptAction observer)
        app.add_observer(bridge::handle_animation_script_actions);

        app.add_systems(
                Update,
                (
                    systems::rehydrate_animators,
                    systems::initialize_animation_graphs,
                    systems::ensure_animation_targets,
                    systems::auto_play_default,
                    systems::process_animation_commands,
                    systems::update_state_machines,
                    systems::update_layer_weights,
                    systems::detect_animation_finished,
                    tween::update_procedural_tweens,
                )
                    .chain(),
            );

        app.add_observer(apply_asset_path_changes_to_animators);
    }
}

/// Patch `AnimatorComponent` clip / state-machine paths when an asset is
/// renamed or moved. Keeps scene references valid without forcing the user
/// to manually re-point every animator.
fn apply_asset_path_changes_to_animators(
    trigger: On<renzora::AssetPathChanged>,
    mut animators: Query<(&mut AnimatorComponent, Option<&mut AnimatorState>)>,
) {
    let ev = trigger.event();
    for (mut animator, state) in animators.iter_mut() {
        let mut touched = false;
        for slot in animator.clips.iter_mut() {
            if let Some(new_path) = ev.rewrite(&slot.path) {
                info!(
                    "[asset-move] rewriting AnimClipSlot '{}' → '{}'",
                    slot.path, new_path
                );
                slot.path = new_path;
                touched = true;
            }
        }
        if let Some(ref sm) = animator.state_machine.clone() {
            if let Some(new_path) = ev.rewrite(sm) {
                info!(
                    "[asset-move] rewriting state_machine '{}' → '{}'",
                    sm, new_path
                );
                animator.state_machine = Some(new_path);
                touched = true;
            }
        }
        // Force re-initialization so the new paths get loaded.
        if touched {
            if let Some(mut state) = state {
                state.initialized = false;
                state.frames_since_init = 0;
                state.node_indices.clear();
                state.graph_handle = None;
                state.current_clip = None;
                state.clip_handles.clear();
            }
        }
    }
}
