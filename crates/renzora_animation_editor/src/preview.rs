//! Preview system — drives AnimationPlayer seek from the editor scrubber,
//! both on the source entity and the studio preview clone.

use std::time::Duration;

use bevy::prelude::*;
use bevy::animation::AnimationTargetId;

use renzora_animation::AnimatorState;

use crate::studio_preview::StudioPreviewModel;
use crate::AnimationEditorState;

/// Resource tracking whether the preview animation has been started on the
/// studio preview model's player. Reset when the selected entity/clip changes.
#[derive(Resource, Default)]
pub struct PreviewPlaybackState {
    /// Entity whose graph was last copied to the preview.
    pub synced_source: Option<Entity>,
    /// Whether the animation node has been activated on the preview player.
    pub preview_started: bool,
    /// The clip name that was started.
    pub started_clip: Option<String>,
}

/// Advance scrub time when previewing, clamp/wrap at clip duration,
/// and seek both the source entity's and the preview model's AnimationPlayer.
pub fn update_animation_preview(
    time: Res<Time>,
    mut editor_state: ResMut<AnimationEditorState>,
    animators: Query<&AnimatorState>,
    mut players: Query<(&mut AnimationPlayer, Option<&mut AnimationTransitions>)>,
    preview_roots: Query<Entity, With<StudioPreviewModel>>,
    children_query: Query<&Children>,
    player_query: Query<Entity, With<AnimationPlayer>>,
    mut playback: ResMut<PreviewPlaybackState>,
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

    // Advance scrub time if previewing
    if editor_state.is_previewing {
        editor_state.scrub_time += time.delta_secs() * editor_state.preview_speed;

        // Wrap or stop at clip end
        if let Some(duration) = editor_state.clip_duration {
            if duration > 0.0 && editor_state.scrub_time >= duration {
                if editor_state.preview_looping {
                    editor_state.scrub_time = 0.0;
                } else {
                    editor_state.scrub_time = 0.0;
                    editor_state.is_previewing = false;
                }
            }
        }
    }

    let scrub = editor_state.scrub_time;

    // Seek the source entity's AnimationPlayer
    if let Some(player_entity) = state.player_entity {
        if let Ok((mut player, transitions)) = players.get_mut(player_entity) {
            // Ensure the animation node is active
            if let Some(anim) = player.animation_mut(node_idx) {
                anim.seek_to(scrub);
            } else if let Some(mut transitions) = transitions {
                // Node not active — start it
                let playing = transitions.play(&mut player, node_idx, Duration::ZERO);
                playing.set_speed(0.0); // We control time via seek
                playing.repeat();
                if let Some(anim) = player.animation_mut(node_idx) {
                    anim.seek_to(scrub);
                }
            }
        }
    }

    // Seek the studio preview model's AnimationPlayer
    // Check if we need to start the clip on the preview player
    let need_start = !playback.preview_started
        || playback.started_clip.as_deref() != Some(&clip_name);

    for root in preview_roots.iter() {
        if let Some(preview_player_entity) = find_player_in_children(root, &children_query, &player_query) {
            if let Ok((mut player, transitions)) = players.get_mut(preview_player_entity) {
                if need_start {
                    // Start the animation node on the preview player
                    if let Some(mut transitions) = transitions {
                        let playing = transitions.play(&mut player, node_idx, Duration::ZERO);
                        playing.set_speed(0.0); // We drive time via seek_to
                        playing.repeat();
                        playback.preview_started = true;
                        playback.started_clip = Some(clip_name.clone());
                    }
                }

                if let Some(anim) = player.animation_mut(node_idx) {
                    anim.seek_to(scrub);
                }
            }
        }
    }
}

/// Copy the AnimationGraph from the source entity's player to the studio preview
/// model's player, and add AnimationTarget components to the preview skeleton.
pub fn sync_preview_animation_graph(
    editor_state: Res<AnimationEditorState>,
    animators: Query<&AnimatorState>,
    preview_roots: Query<Entity, With<StudioPreviewModel>>,
    children_query: Query<&Children>,
    player_query: Query<Entity, With<AnimationPlayer>>,
    graph_query: Query<&AnimationGraphHandle>,
    name_query: Query<&Name>,
    mut commands: Commands,
    mut playback: ResMut<PreviewPlaybackState>,
) {
    let Some(entity) = editor_state.selected_entity else {
        playback.synced_source = None;
        playback.preview_started = false;
        return;
    };

    // Reset if source entity changed
    if playback.synced_source != Some(entity) {
        playback.preview_started = false;
        playback.started_clip = None;
    }

    let Ok(state) = animators.get(entity) else {
        return;
    };

    if !state.initialized {
        return;
    }

    let Some(player_entity) = state.player_entity else {
        return;
    };

    let Ok(source_graph) = graph_query.get(player_entity) else {
        return;
    };

    let graph_handle = source_graph.0.clone();

    for root in preview_roots.iter() {
        if let Some(preview_player) = find_player_in_children(root, &children_query, &player_query) {
            if graph_query.get(preview_player).is_err() {
                // Copy graph
                commands
                    .entity(preview_player)
                    .insert(AnimationGraphHandle(graph_handle.clone()))
                    .try_insert(AnimationTransitions::new());

                // Add AnimationTarget components to all named bones in the preview
                add_targets_recursive(root, &children_query, &name_query, &mut commands);

                playback.synced_source = Some(entity);
                playback.preview_started = false;
                info!("[studio_preview] Copied AnimationGraph + targets to preview player {:?}", preview_player);
            }
        }
    }
}

/// Update the studio preview camera clear color to match the theme.
pub fn sync_preview_clear_color(
    theme_manager: Option<Res<renzora::theme::ThemeManager>>,
    mut cameras: Query<&mut Camera, With<crate::studio_preview::StudioPreviewCamera>>,
) {
    let Some(tm) = theme_manager else { return };

    let c = tm.active_theme.surfaces.panel.to_color32();
    let clear = Color::srgba(
        c.r() as f32 / 255.0,
        c.g() as f32 / 255.0,
        c.b() as f32 / 255.0,
        1.0,
    );

    for mut camera in cameras.iter_mut() {
        camera.clear_color = ClearColorConfig::Custom(clear);
    }
}

/// Walk children recursively to find an AnimationPlayer entity.
fn find_player_in_children(
    root: Entity,
    children_query: &Query<&Children>,
    player_query: &Query<Entity, With<AnimationPlayer>>,
) -> Option<Entity> {
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if player_query.get(entity).is_ok() {
            return Some(entity);
        }
        if let Ok(children) = children_query.get(entity) {
            stack.extend(children.iter());
        }
    }
    None
}

/// Add `AnimationTargetId` to all named descendants so the animation system
/// can drive their transforms.
fn add_targets_recursive(
    entity: Entity,
    children_query: &Query<&Children>,
    name_query: &Query<&Name>,
    commands: &mut Commands,
) {
    let mut stack = vec![entity];
    while let Some(e) = stack.pop() {
        if let Ok(name) = name_query.get(e) {
            let target_id = AnimationTargetId::from_name(name);
            commands.entity(e).try_insert(target_id);
        }
        if let Ok(children) = children_query.get(e) {
            stack.extend(children.iter());
        }
    }
}
