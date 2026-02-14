//! GLTF Animation playback system
//!
//! Handles playing animations from imported GLTF files using Bevy's
//! built-in animation system.

use bevy::prelude::*;

use crate::component_system::data::components::animation::{GltfAnimations, GltfAnimationStorage};

/// System to initialize animation graphs for entities with GltfAnimations
pub fn setup_gltf_animation_graphs(
    mut commands: Commands,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut gltf_anims: Query<(Entity, &mut GltfAnimations), Changed<GltfAnimations>>,
    mut animation_storage: ResMut<GltfAnimationStorage>,
    children_query: Query<&Children>,
    player_query: Query<Entity, With<AnimationPlayer>>,
) {
    for (entity, mut anims) in gltf_anims.iter_mut() {
        // Skip if already initialized or no clip names
        if anims.initialized || anims.clip_names.is_empty() {
            continue;
        }

        // Get the handles from storage
        let Some(handles) = animation_storage.handles.get_mut(&entity) else {
            warn!("No animation handles found in storage for {:?}", entity);
            continue;
        };

        if handles.clips.is_empty() {
            continue;
        }

        // Create animation graph with all clips
        let (graph, node_indices) = AnimationGraph::from_clips(handles.clips.iter().cloned());
        let graph_handle = graphs.add(graph);

        handles.graph = graph_handle.clone();
        handles.node_indices = node_indices;
        anims.initialized = true;

        // Find AnimationPlayer in children (GLTF skeletons have it on armature entity)
        if let Some(player_entity) = find_animation_player_in_children(entity, &children_query, &player_query) {
            anims.player_entity = Some(player_entity);

            // Set the animation graph on the player entity
            commands.entity(player_entity)
                .insert(AnimationGraphHandle(graph_handle));

            // Add transitions component if not present
            commands.entity(player_entity)
                .try_insert(AnimationTransitions::new());

            info!("Initialized animation graph for {:?} with {} clips, player on {:?}",
                entity, anims.clip_names.len(), player_entity);
        } else {
            warn!("No AnimationPlayer found in children of {:?}", entity);
        }
    }
}

/// System to handle animation playback based on GltfAnimations state
pub fn update_gltf_animation_playback(
    gltf_anims: Query<(Entity, &GltfAnimations)>,
    animation_storage: Res<GltfAnimationStorage>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    for (entity, anims) in gltf_anims.iter() {
        let Some(player_entity) = anims.player_entity else {
            continue;
        };

        let Ok((mut player, mut transitions)) = players.get_mut(player_entity) else {
            continue;
        };

        let Some(active_idx) = anims.active_clip else {
            continue;
        };

        // Get node indices from storage
        let Some(handles) = animation_storage.handles.get(&entity) else {
            continue;
        };

        let Some(&node_index) = handles.node_indices.get(active_idx) else {
            continue;
        };

        if anims.is_playing {
            // Check if we need to start or the clip changed
            if !player.is_playing_animation(node_index) {
                transitions
                    .play(&mut player, node_index, std::time::Duration::from_millis(250))
                    .repeat();
            }

            // Update speed
            if let Some(active_anim) = player.animation_mut(node_index) {
                active_anim.set_speed(anims.speed);
            }
        } else {
            // Pause all animations
            player.pause_all();
        }
    }
}

/// Recursively find an entity with AnimationPlayer in children
fn find_animation_player_in_children(
    entity: Entity,
    children_query: &Query<&Children>,
    player_query: &Query<Entity, With<AnimationPlayer>>,
) -> Option<Entity> {
    // Check if this entity has a player
    if player_query.get(entity).is_ok() {
        return Some(entity);
    }

    // Check children recursively
    if let Ok(children) = children_query.get(entity) {
        for child in children.iter() {
            if let Some(found) = find_animation_player_in_children(child, children_query, player_query) {
                return Some(found);
            }
        }
    }

    None
}

/// Plugin to add GLTF animation systems
pub struct GltfAnimationPlugin;

impl Plugin for GltfAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GltfAnimationStorage>();
        app.add_systems(
            Update,
            (
                setup_gltf_animation_graphs,
                update_gltf_animation_playback,
            )
                .chain(),
        );
    }
}
