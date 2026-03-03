//! Systems for AnimatorComponent: graph initialization and playback.

use bevy::prelude::*;
use bevy::animation::{AnimatedBy, AnimationTargetId};
use std::time::Duration;

use super::component::AnimatorComponent;

// ============================================================================
// Graph initialization
// ============================================================================

/// Runs on Changed<AnimatorComponent>; loads handles and builds the graph when all clips are ready.
pub fn setup_animator_graphs(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    clip_assets: Res<Assets<AnimationClip>>,
    mut animators: Query<(Entity, &mut AnimatorComponent), Changed<AnimatorComponent>>,
    children_query: Query<&Children>,
    player_query: Query<Entity, With<AnimationPlayer>>,
    name_query: Query<&Name>,
) {
    for (entity, mut animator) in animators.iter_mut() {
        if animator.initialized || animator.clips.is_empty() {
            continue;
        }

        // Step 1: Load handles for any unloaded clips
        let mut all_loaded = true;
        for slot in animator.clips.iter_mut() {
            if slot.handle.is_none() && !slot.path.is_empty() {
                slot.handle = Some(asset_server.load(&slot.path));
            }
            if let Some(ref handle) = slot.handle {
                if clip_assets.get(handle).is_none() {
                    all_loaded = false;
                }
            } else {
                all_loaded = false;
            }
        }

        if !all_loaded {
            continue;
        }

        // Step 2: Collect valid handles
        let clip_handles: Vec<Handle<AnimationClip>> = animator.clips.iter()
            .filter_map(|s| s.handle.clone())
            .collect();

        if clip_handles.is_empty() {
            continue;
        }

        // Step 3: Build animation graph
        let (graph, node_indices) = AnimationGraph::from_clips(clip_handles.iter().cloned());
        let graph_handle = graphs.add(graph);

        // Assign node indices back to slots
        for (slot, node_idx) in animator.clips.iter_mut().zip(node_indices.iter()) {
            slot.node_index = Some(*node_idx);
        }

        // Step 4: Find the AnimationPlayer entity in children
        let Some(player_entity) = find_animation_player_in_children(entity, &children_query, &player_query) else {
            warn!("AnimatorComponent on {:?}: no AnimationPlayer found in children", entity);
            continue;
        };

        animator.player_entity = Some(player_entity);

        // Step 5: Add AnimationTarget components to all skeleton bones
        // This is needed so the animation clip can target the correct bone entities
        add_animation_targets_to_skeleton(
            &mut commands,
            entity,
            player_entity,
            &children_query,
            &name_query,
        );

        // Step 6: Assign graph and transitions to the player entity
        commands.entity(player_entity)
            .insert(AnimationGraphHandle(graph_handle))
            .try_insert(AnimationTransitions::new());

        animator.initialized = true;

        info!("AnimatorComponent: initialized {} clips on {:?}, player at {:?}",
            animator.clips.len(), entity, player_entity);
    }
}

/// Add AnimationTarget components to all named entities in the skeleton hierarchy.
fn add_animation_targets_to_skeleton(
    commands: &mut Commands,
    root_entity: Entity,
    player_entity: Entity,
    children_query: &Query<&Children>,
    name_query: &Query<&Name>,
) {
    visit_skeleton_entities(
        root_entity,
        player_entity,
        commands,
        children_query,
        name_query,
    );
}

fn visit_skeleton_entities(
    entity: Entity,
    player_entity: Entity,
    commands: &mut Commands,
    children_query: &Query<&Children>,
    name_query: &Query<&Name>,
) {
    // Add AnimationTargetId + AnimatedBy for every named entity in the hierarchy
    if let Ok(name) = name_query.get(entity) {
        let target_id = AnimationTargetId::from_name(name);
        commands.entity(entity).try_insert((
            target_id,
            AnimatedBy(player_entity),
        ));
    }

    if let Ok(children) = children_query.get(entity) {
        for child in children.iter() {
            visit_skeleton_entities(child, player_entity, commands, children_query, name_query);
        }
    }
}

// ============================================================================
// Playback
// ============================================================================

/// Drive AnimationPlayer from AnimatorComponent::current_clip.
pub fn update_animator_playback(
    animators: Query<&AnimatorComponent>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    for animator in animators.iter() {
        if !animator.initialized {
            continue;
        }
        let Some(player_entity) = animator.player_entity else { continue };
        let Ok((mut player, mut transitions)) = players.get_mut(player_entity) else { continue };
        let Some(ref clip_name) = animator.current_clip else {
            player.pause_all();
            continue;
        };

        // Find the slot
        let Some(slot) = animator.clips.iter().find(|s| &s.name == clip_name) else { continue };
        let Some(node_idx) = slot.node_index else { continue };

        // Start playing if not already
        if !player.is_playing_animation(node_idx) {
            let blend = Duration::from_secs_f32(animator.blend_duration.max(0.0));
            let playing = transitions.play(&mut player, node_idx, blend);
            if slot.looping {
                playing.repeat();
            }
        }

        // Update speed
        if let Some(anim) = player.animation_mut(node_idx) {
            anim.set_speed(slot.speed);
        }
    }
}

// ============================================================================
// Helper
// ============================================================================

fn find_animation_player_in_children(
    entity: Entity,
    children_query: &Query<&Children>,
    player_query: &Query<Entity, With<AnimationPlayer>>,
) -> Option<Entity> {
    if player_query.get(entity).is_ok() {
        return Some(entity);
    }
    if let Ok(children) = children_query.get(entity) {
        for child in children.iter() {
            if let Some(found) = find_animation_player_in_children(child, children_query, player_query) {
                return Some(found);
            }
        }
    }
    None
}
