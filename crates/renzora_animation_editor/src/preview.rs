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

/// The pose the preview last had Bevy apply.
///
/// Bevy writes every target of an active clip every frame, at any speed, so a
/// clip frozen at the playhead marked the model's whole tree changed on every
/// frame of an idle editor: the viewport never stopped rendering (it renders on
/// change) and transform propagation walked the model each frame. Once a pose
/// has been applied it is held by setting the clip's weight to zero, which Bevy
/// skips without touching the targets, and applied again only when something
/// that decides it changes: the player, the clip node, the playhead, or the clip
/// or graph assets (a key added in the timeline rebuilds the clip in place).
#[derive(Default)]
pub struct AppliedPreview {
    /// The player the preview is driving, kept apart from `at` so a player it
    /// moves off is always held, whatever else reset `at`.
    player: Option<Entity>,
    /// The clip node and playhead last applied to `player`.
    at: Option<(BlendNodeIndex, f32)>,
}

/// Seek the selected entity's AnimationPlayer to the playhead.
pub fn update_animation_preview(
    editor_state: Res<AnimationEditorState>,
    animators: Query<&AnimatorState>,
    mut players: Query<(&mut AnimationPlayer, Option<&mut AnimationTransitions>)>,
    mut clip_events: MessageReader<AssetEvent<AnimationClip>>,
    mut graph_events: MessageReader<AssetEvent<BlendGraph>>,
    mut applied: Local<AppliedPreview>,
) {
    // Read both every run so a stale backlog never forces a re-apply later.
    let assets_changed = clip_events.read().count() > 0 || graph_events.read().count() > 0;
    if assets_changed {
        applied.at = None;
    }
    let state = editor_state
        .selected_entity
        .and_then(|e| animators.get(e).ok())
        .filter(|s| s.initialized);
    // A player the preview moved off keeps its clip active, so it would go on
    // being written every frame: hold it where it is. Checked before any early
    // return, so picking an entity with no animator holds it too.
    let current = state.and_then(|s| s.player_entity);
    if let Some(previous) = applied.player {
        if Some(previous) != current {
            if let Ok((mut old, _)) = players.get_mut(previous) {
                hold_all(&mut old);
            }
            applied.player = None;
            applied.at = None;
        }
    }
    let Some(state) = state else {
        return;
    };

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

    let Some(player_entity) = state.player_entity else {
        return;
    };
    if let Ok((mut player, transitions)) = players.get_mut(player_entity) {
        let key = (node_idx, scrub);
        let started = show_only(&mut player, transitions, node_idx, scrub);
        if started || applied.player != Some(player_entity) || applied.at != Some(key) {
            // Applied this frame by Bevy's animation pass; held from the next.
            applied.player = Some(player_entity);
            applied.at = Some(key);
            if let Some(anim) = player.animation_mut(node_idx) {
                anim.set_weight(1.0);
            }
        } else if player.animation(node_idx).is_some_and(|a| a.weight() != 0.0) {
            if let Some(anim) = player.animation_mut(node_idx) {
                anim.set_weight(0.0);
            }
        }
    }
}

/// Stop Bevy re-applying any of `player`'s clips, leaving the pose as it is.
fn hold_all(player: &mut AnimationPlayer) {
    let active: Vec<BlendNodeIndex> = player.playing_animations().map(|(idx, _)| *idx).collect();
    for idx in active {
        if let Some(anim) = player.animation_mut(idx) {
            anim.set_weight(0.0);
        }
    }
}

/// Make `node` the one animation on `player`, frozen (speed 0) at `time`.
/// Returns whether it had to be started.
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
    node: BlendNodeIndex,
    time: f32,
) -> bool {
    let stale: Vec<BlendNodeIndex> = player
        .playing_animations()
        .map(|(idx, _)| *idx)
        .filter(|idx| *idx != node)
        .collect();
    for idx in stale {
        player.stop(idx);
    }
    let started = player.animation(node).is_none();
    if started {
        match transitions {
            Some(mut transitions) => {
                transitions.play(player, node, Duration::ZERO).set_speed(0.0).repeat();
            }
            None => {
                player.play(node).set_speed(0.0).repeat();
            }
        }
    }
    // Zeroed every frame, not only on start: a node something else started
    // runs at full speed, and seeking alone only drags it back once a frame.
    // Compared first, so a held pose is not re-sought for nothing.
    let settled = player
        .animation(node)
        .is_some_and(|a| a.speed() == 0.0 && a.seek_time() == time);
    if !settled {
        if let Some(anim) = player.animation_mut(node) {
            anim.set_speed(0.0);
            anim.seek_to(time);
        }
    }
    started
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
