//! Preview system: drives the model's AnimationPlayer from the editor's
//! playhead.
//!
//! Only the real model is played. The stage's copy of it (the studio preview)
//! takes the real model's bones each frame (`stage::clone`), so it shows the
//! clip and everything posed over it. It used to play the clip itself, from a
//! copy of this player's graph, and showed the clip and nothing else.

use std::time::Duration;

use bevy::prelude::*;

use renzora_animation::AnimatorState;

use crate::AnimationEditorState;

/// Seek the selected entity's AnimationPlayer to the playhead.
pub fn update_animation_preview(
    editor_state: Res<AnimationEditorState>,
    animators: Query<&AnimatorState>,
    mut players: Query<(&mut AnimationPlayer, Option<&mut AnimationTransitions>)>,
) {
    let Some(entity) = editor_state.selected_entity else {
        return;
    };

    let Ok(state) = animators.get(entity) else {
        return;
    };

    if !state.initialized {
        return;
    }

    // Resolve which clip to play
    let clip_name = editor_state
        .selected_clip
        .clone()
        .or_else(|| state.current_clip.clone());

    let Some(clip_name) = clip_name else {
        return;
    };

    let Some(&node_idx) = state.node_indices.get(&clip_name) else {
        return;
    };

    // The playhead (scrub_time) is advanced globally by `advance_preview_time`
    // (lib.rs); here we only seek the player to it.
    let scrub = editor_state.scrub_time;

    if let Some(player_entity) = state.player_entity {
        if let Ok((mut player, transitions)) = players.get_mut(player_entity) {
            show_only(&mut player, transitions, node_idx, scrub);
        }
    }
}

/// Make `node` the one animation on `player`, frozen (speed 0) at `time`.
///
/// Switching clips used to start the new node only through
/// `AnimationTransitions`, and only when that component was present; with it
/// missing, or with an older node still active (a Play from before the editor
/// gated them, or a node left over from a graph rebuild), the old clip kept
/// playing and picking another in the dropdown looked like it did nothing.
/// So every other active node is stopped here, and the new one is started
/// directly on the player when there are no transitions to go through.
fn show_only(
    player: &mut AnimationPlayer,
    transitions: Option<Mut<AnimationTransitions>>,
    node: AnimationNodeIndex,
    time: f32,
) {
    let stale: Vec<AnimationNodeIndex> = player
        .playing_animations()
        .map(|(idx, _)| *idx)
        .filter(|idx| *idx != node)
        .collect();
    for idx in stale {
        player.stop(idx);
    }
    if player.animation(node).is_none() {
        match transitions {
            Some(mut transitions) => {
                transitions.play(player, node, Duration::ZERO).set_speed(0.0).repeat();
            }
            None => {
                player.play(node).set_speed(0.0).repeat();
            }
        }
    }
    if let Some(anim) = player.animation_mut(node) {
        // Zeroed every frame, not only on start: a node something else started
        // runs at full speed, and seeking alone only drags it back once a frame.
        anim.set_speed(0.0);
        anim.seek_to(time);
    }
}

/// Update the studio preview camera clear color to match the theme.
pub fn sync_preview_clear_color(
    theme_manager: Option<Res<renzora_theme::ThemeManager>>,
    mut cameras: Query<&mut Camera, With<crate::studio_preview::StudioPreviewCamera>>,
) {
    let Some(tm) = theme_manager else { return };

    let [r, g, b, _] = tm.active_theme.surfaces.panel.to_array();
    let clear = Color::srgba(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        1.0,
    );

    for mut camera in cameras.iter_mut() {
        camera.clear_color = ClearColorConfig::Custom(clear);
    }
}
