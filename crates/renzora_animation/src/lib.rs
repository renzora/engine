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
pub mod discovery;
pub mod extract;
pub mod graph_builder;
pub mod layers;
pub mod loader;
pub mod plugin_bridge;
pub mod property_playback;
pub mod read_state;
#[cfg(feature = "scripting")]
pub mod script_extension;
pub mod sm_loader;
pub mod state_machine;
pub mod systems;
pub mod tween;

pub use blend_tree::BlendTree;
pub use clip::{AnimClip, BoneTrack};
pub use component::{AnimClipSlot, AnimatorComponent, AnimatorState};
pub use discovery::discover_animation_clips;
pub use layers::{AnimationLayer, LayerBlendMode};
pub use loader::AnimClipLoader;
pub use property_playback::{PropertyClip, PropertyClipLoader};
pub use read_state::AnimatorReadState;
pub use state_machine::{
    AnimCondition, AnimParams, AnimState, AnimTransition, AnimationStateMachine, StateMotion,
};
pub use systems::{AnimationCommand, AnimationCommandQueue};
pub use tween::{EasingFunction, ProceduralTween, TweenProperty};

use bevy::prelude::*;

/// The retarget key for one bone, from either side of the binding.
///
/// # Why a single name, and not Bevy's path
///
/// Bevy addresses a curve with [`AnimationTargetId::from_names`] over the whole
/// chain of names from the animation root, which is right for a clip embedded in
/// the glTF that also holds the skeleton. A `.anim` is not that. The workflow
/// this format exists for is a character downloaded with no animations and its
/// animations downloaded with no character — separate files, imported
/// separately — and a path hash from the animation's file could never match a
/// skeleton spawned from the model's. Matching on the bone name alone *is* the
/// retargeting, which is why one clip can drive any rig that names its bones the
/// same way.
///
/// # Why it is sanitized
///
/// Because the name the importer wrote and the name the entity ends up with are
/// not the same string. Every importer records the source's spelling
/// (`mixamorig:Hips`, from the FBX or glTF node), while
/// `renzora_editor_framework`'s `enforce_entity_ids` runs [`renzora::sanitize_id`]
/// over every `Name` in the world and turns that into `mixamorig_hips`.
///
/// Hashing the raw string on one side and the live `Name` on the other made
/// binding a race between the tagging systems below and that rename, unordered
/// with respect to each other: tag first and every curve bound, rename first and
/// none did, and `ensure_animation_targets` froze whichever won because it only
/// ever filled in *absent* ids. The visible symptom was a character in its bind
/// pose with a clip that was genuinely playing, at full weight, into nothing —
/// the animator, the current clip and the timeline all reporting success,
/// because none of them can see whether a curve found a bone.
///
/// `sanitize_id` is idempotent, so putting both sides through it collapses the
/// two spellings onto one key and the rename stops mattering. Retargeting is
/// unaffected: the key is still derived from the bone name and nothing else.
///
/// The one case this does not save is a rig whose bones collide *after*
/// sanitizing — `Bone:L` and `Bone-L` both become `bone_l`, and
/// `enforce_entity_ids` renames the second to `bone_l_1`, which no clip names.
/// Mixamo rigs have no such collisions.
pub fn bone_target(bone_name: &str) -> bevy::animation::AnimationTargetId {
    bevy::animation::AnimationTargetId::from_name(&Name::new(renzora::sanitize_id(bone_name)))
}

#[derive(Default)]
pub struct AnimationPlugin;

impl Plugin for AnimationPlugin {
    fn build(&self, app: &mut App) {
        info!("[animation] AnimationPlugin");

        app.register_type::<AnimatorComponent>()
            .register_type::<AnimClipSlot>()
            .register_type::<AnimatorReadState>()
            .register_type::<AnimParams>()
            .register_type::<AnimationLayer>()
            .register_type::<LayerBlendMode>()
            .init_asset::<AnimationStateMachine>()
            .init_asset::<PropertyClip>()
            .init_asset_loader::<AnimClipLoader>()
            .init_asset_loader::<PropertyClipLoader>()
            .init_asset_loader::<sm_loader::AnimSmLoader>()
            .init_resource::<AnimationCommandQueue>()
            .init_resource::<property_playback::PropAnimDebug>()
            .init_resource::<renzora::ScriptAnimEventInbox>();

        // Script animation commands (decoupled via ScriptAction observer)
        app.add_observer(bridge::handle_animation_script_actions);

        // Register script functions owned by the animation crate.
        #[cfg(feature = "scripting")]
        {
            let mut extensions = app.world_mut().get_resource_or_insert_with(
                renzora_scripting::extension::ScriptExtensions::default,
            );
            extensions.register(script_extension::AnimationScriptExtension);
        }

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

        // Property animation runs in the exported runtime / editor play mode.
        // While editing, the animation-editor scrub preview drives sampling.
        app.add_systems(
            Update,
            property_playback::apply_runtime_property_animation
                .after(systems::process_animation_commands)
                .run_if(property_playback::property_animation_active),
        );

        app.add_systems(
            Update,
            (
                read_state::auto_init_animator_read_state,
                read_state::update_animator_read_state,
            )
                .chain(),
        );

        // The C-ABI surface: standalone plugins driving and reading animators.
        plugin_bridge::install(app);

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
                state.prop_clip_handles.clear();
            }
        }
    }
}

renzora::add!(AnimationPlugin);

#[cfg(test)]
mod tests {
    use super::*;

    /// The property the whole binding rests on: an importer's spelling and the
    /// spelling `enforce_entity_ids` leaves on the entity must reach the same
    /// key. Without it, whether a clip animates depends on which of two
    /// unordered systems ran first.
    #[test]
    fn both_spellings_of_a_bone_reach_one_key() {
        assert_eq!(bone_target("mixamorig:Hips"), bone_target("mixamorig_hips"));
        assert_eq!(
            bone_target("mixamorig:LeftHandIndex1"),
            bone_target("mixamorig_lefthandindex1")
        );
        // Blender and Maya rigs, which use the same characters differently.
        assert_eq!(bone_target("Armature|Bone.001"), bone_target("armature_bone_001"));
    }

    /// Sanitizing must not merge bones that are genuinely different, or one
    /// clip would drive two limbs.
    #[test]
    fn distinct_bones_keep_distinct_keys() {
        assert_ne!(bone_target("mixamorig:LeftArm"), bone_target("mixamorig:RightArm"));
        assert_ne!(bone_target("Spine"), bone_target("Spine1"));
    }
}
