//! Preview system — drives AnimationPlayer seek from the editor scrubber.

use bevy::prelude::*;

use renzora_animation::{AnimatorComponent, AnimatorState};

use crate::AnimationEditorState;

/// Advance scrub time when previewing and optionally seek the animation player.
pub fn update_animation_preview(
    time: Res<Time>,
    mut editor_state: ResMut<AnimationEditorState>,
    animators: Query<(&AnimatorComponent, &AnimatorState)>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let Some(entity) = editor_state.selected_entity else {
        return;
    };

    if !editor_state.is_previewing {
        return;
    }

    // Advance scrub time
    editor_state.scrub_time += time.delta_secs() * editor_state.preview_speed;

    // Get the animator to find duration bounds
    let Ok((animator, state)) = animators.get(entity) else {
        return;
    };

    if !state.initialized {
        return;
    }

    // Find current clip's node index and seek the player
    let clip_name = editor_state
        .selected_clip
        .as_deref()
        .or(state.current_clip.as_deref())
        .or(animator.default_clip.as_deref());

    let Some(clip_name) = clip_name else {
        return;
    };

    let Some(&node_idx) = state.node_indices.get(clip_name) else {
        return;
    };

    let Some(player_entity) = state.player_entity else {
        return;
    };

    let Ok(mut player) = players.get_mut(player_entity) else {
        return;
    };

    if let Some(anim) = player.animation_mut(node_idx) {
        anim.seek_to(editor_state.scrub_time);
    }
}
