//! Runtime systems for the animation plugin.

use bevy::animation::AnimationTargetId;
use bevy::prelude::*;
use std::time::Duration;

use crate::component::{AnimatorComponent, AnimatorState};
use crate::state_machine::{AnimationStateMachine, StateMotion};

// ============================================================================
// Rehydration: build AnimationGraph when AnimatorComponent is added
// ============================================================================

/// Spawns `AnimatorState` for new `AnimatorComponent` entities and starts loading clips.
pub fn rehydrate_animators(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    new_animators: Query<(Entity, &AnimatorComponent), Without<AnimatorState>>,
) {
    for (entity, animator) in new_animators.iter() {
        if animator.clips.is_empty() {
            info!("[animation] Rehydrate: {:?} has empty clips, skipping", entity);
            continue;
        }

        info!(
            "[animation] Rehydrate: {:?} with {} clips, loading...",
            entity,
            animator.clips.len()
        );

        let mut state = AnimatorState::default();

        // Start loading all clip assets
        for slot in &animator.clips {
            if !slot.path.is_empty() {
                info!("[animation]   Loading clip '{}' from '{}'", slot.name, slot.path);
                let handle: Handle<AnimationClip> = asset_server.load(&slot.path);
                state.clip_handles.insert(slot.name.clone(), handle);
            }
        }

        // Start loading state machine if specified
        if let Some(ref sm_path) = animator.state_machine {
            if !sm_path.is_empty() {
                let handle: Handle<AnimationStateMachine> = asset_server.load(sm_path);
                state.sm_handle = Some(handle);
            }
        }

        commands.entity(entity).try_insert(state);
    }
}

/// Once all clips are loaded, build the AnimationGraph and wire up the player.
pub fn initialize_animation_graphs(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    clip_assets: Res<Assets<AnimationClip>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut animators: Query<(Entity, &AnimatorComponent, &mut AnimatorState)>,
    children_query: Query<&Children>,
    player_query: Query<Entity, With<AnimationPlayer>>,
    name_query: Query<&Name>,
) {
    for (entity, animator, mut state) in animators.iter_mut() {
        if state.initialized || state.clip_handles.is_empty() {
            continue;
        }

        // Check if all clips are loaded
        let mut loaded_count = 0;
        let mut failed_count = 0;
        for (name, handle) in state.clip_handles.iter() {
            let load_state = asset_server.get_load_state(handle.id());
            match load_state {
                Some(bevy::asset::LoadState::Loaded) => loaded_count += 1,
                Some(bevy::asset::LoadState::Failed(_)) => {
                    warn!("[animation] Clip '{}' FAILED to load", name);
                    failed_count += 1;
                }
                _ => {}
            }
        }

        let total = state.clip_handles.len();
        if loaded_count + failed_count < total {
            // Still loading
            continue;
        }

        if loaded_count == 0 {
            warn!("[animation] All {} clips failed to load for {:?}", total, entity);
            state.initialized = true; // Don't retry
            continue;
        }

        info!("[animation] {}/{} clips loaded for {:?}", loaded_count, total, entity);

        // Collect valid clip handles in slot order
        let mut ordered_handles: Vec<(String, Handle<AnimationClip>)> = Vec::new();
        for slot in &animator.clips {
            if let Some(handle) = state.clip_handles.get(&slot.name) {
                if clip_assets.get(handle).is_some() {
                    ordered_handles.push((slot.name.clone(), handle.clone()));
                }
            }
        }

        if ordered_handles.is_empty() {
            warn!("[animation] No valid clip assets found despite loaded status");
            continue;
        }

        // Build the animation graph
        let (graph, node_indices) =
            AnimationGraph::from_clips(ordered_handles.iter().map(|(_, h)| h.clone()));

        // Map names to node indices
        for (i, (name, _)) in ordered_handles.iter().enumerate() {
            state.node_indices.insert(name.clone(), node_indices[i]);
        }

        let graph_handle = graphs.add(graph);
        state.graph_handle = Some(graph_handle.clone());

        // Find the AnimationPlayer in children
        let Some(player_entity) =
            find_animation_player(entity, &children_query, &player_query)
        else {
            // Player might not be spawned yet (GLTF still loading scenes)
            warn!("[animation] AnimationPlayer not found in children of {:?}, will retry", entity);
            continue;
        };
        info!("[animation] Found AnimationPlayer at {:?}", player_entity);

        state.player_entity = Some(player_entity);

        // Add AnimationTarget components to skeleton bones
        add_animation_targets(&mut commands, entity, player_entity, &children_query, &name_query);

        // Assign graph and transitions to the player
        commands
            .entity(player_entity)
            .insert(AnimationGraphHandle(graph_handle))
            .try_insert(AnimationTransitions::new());

        state.initialized = true;

        info!(
            "AnimatorComponent: initialized {} clips on {:?}, player at {:?}",
            ordered_handles.len(),
            entity,
            player_entity
        );
    }
}

// ============================================================================
// Auto-play default clip
// ============================================================================

/// After initialization, auto-play the default clip if specified.
///
/// Uses a simple flag check rather than `Changed<>` because deferred commands
/// from `initialize_animation_graphs` (which inserts AnimationTransitions)
/// won't be applied until after the current system chain finishes.
pub fn auto_play_default(
    mut animators: Query<(&AnimatorComponent, &mut AnimatorState)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    graph_query: Query<(), With<AnimationGraphHandle>>,
) {
    for (animator, mut state) in animators.iter_mut() {
        if !state.initialized {
            continue;
        }

        // Check if the animation graph was removed from the player (e.g. by scene reload)
        if let Some(player_entity) = state.player_entity {
            if graph_query.get(player_entity).is_err() {
                if state.current_clip.is_some() {
                    warn!("[animation] AnimationGraphHandle disappeared from player {:?}, re-initializing", player_entity);
                    state.initialized = false;
                    state.current_clip = None;
                    continue;
                }
            }
        }

        if state.current_clip.is_some() {
            continue;
        }

        let Some(ref default_name) = animator.default_clip else {
            continue;
        };

        let Some(player_entity) = state.player_entity else {
            continue;
        };

        let Some(&node_idx) = state.node_indices.get(default_name.as_str()) else {
            warn!(
                "[animation] default_clip '{}' not found in node_indices",
                default_name
            );
            continue;
        };

        let Ok((mut player, mut transitions)) = players.get_mut(player_entity) else {
            continue;
        };

        let slot = animator.get_slot(default_name);
        let looping = slot.map_or(true, |s| s.looping);
        let speed = slot.map_or(1.0, |s| s.speed);

        let blend = Duration::from_secs_f32(animator.blend_duration.max(0.0));
        let playing = transitions.play(&mut player, node_idx, blend);
        if looping {
            playing.repeat();
        }
        playing.set_speed(speed);

        state.current_clip = Some(default_name.clone());
        info!("[animation] Auto-playing default clip '{}' on {:?}", default_name, player_entity);
    }
}

// ============================================================================
// Command processing: play/stop/pause/resume from scripts & blueprints
// ============================================================================

/// Animation command to be processed this frame.
#[derive(Debug)]
pub enum AnimationCommand {
    Play {
        entity: Entity,
        name: String,
        looping: bool,
        speed: f32,
    },
    Stop {
        entity: Entity,
    },
    Pause {
        entity: Entity,
    },
    Resume {
        entity: Entity,
    },
    SetSpeed {
        entity: Entity,
        speed: f32,
    },
    /// Crossfade to a new clip with explicit duration.
    Crossfade {
        entity: Entity,
        name: String,
        duration: f32,
        looping: bool,
    },
    /// Set a float parameter on the state machine.
    SetParam {
        entity: Entity,
        name: String,
        value: f32,
    },
    /// Set a bool parameter on the state machine.
    SetBoolParam {
        entity: Entity,
        name: String,
        value: bool,
    },
    /// Fire a trigger parameter on the state machine.
    Trigger {
        entity: Entity,
        name: String,
    },
    /// Set a layer's weight.
    SetLayerWeight {
        entity: Entity,
        layer_name: String,
        weight: f32,
    },
}

/// Resource that collects animation commands each frame.
#[derive(Resource, Default)]
pub struct AnimationCommandQueue {
    pub commands: Vec<AnimationCommand>,
}

/// System that processes animation commands.
pub fn process_animation_commands(
    mut cmd_queue: ResMut<AnimationCommandQueue>,
    mut animators: Query<(&mut AnimatorComponent, &mut AnimatorState)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    if !cmd_queue.commands.is_empty() {
        info!("[animation] Processing {} command(s): {:?}", cmd_queue.commands.len(), cmd_queue.commands);
    }
    for cmd in cmd_queue.commands.drain(..) {
        match cmd {
            AnimationCommand::Play {
                entity,
                name,
                looping,
                speed,
            } => {
                let Ok((animator, mut state)) = animators.get_mut(entity) else {
                    continue;
                };
                if !state.initialized {
                    continue;
                }

                let Some(&node_idx) = state.node_indices.get(&name) else {
                    warn!("AnimationCommand::Play: clip '{}' not found", name);
                    continue;
                };

                let Some(player_entity) = state.player_entity else {
                    continue;
                };

                let Ok((mut player, mut transitions)) = players.get_mut(player_entity) else {
                    continue;
                };

                let blend =
                    Duration::from_secs_f32(animator.blend_duration.max(0.0));
                let playing = transitions.play(&mut player, node_idx, blend);
                if looping {
                    playing.repeat();
                }
                playing.set_speed(speed);

                state.current_clip = Some(name);
                state.is_paused = false;
            }

            AnimationCommand::Stop { entity } => {
                let Ok((_, mut state)) = animators.get_mut(entity) else {
                    continue;
                };
                if !state.initialized {
                    continue;
                }

                let Some(player_entity) = state.player_entity else {
                    continue;
                };

                let Ok((mut player, _)) = players.get_mut(player_entity) else {
                    continue;
                };

                // Stop all playing animations
                for &node_idx in state.node_indices.values() {
                    if let Some(anim) = player.animation_mut(node_idx) {
                        anim.set_speed(0.0);
                        anim.seek_to(0.0);
                    }
                }

                state.current_clip = None;
                state.is_paused = false;
            }

            AnimationCommand::Pause { entity } => {
                let Ok((_, mut state)) = animators.get_mut(entity) else {
                    continue;
                };
                if !state.initialized {
                    continue;
                }

                let Some(player_entity) = state.player_entity else {
                    continue;
                };

                let Ok((mut player, _)) = players.get_mut(player_entity) else {
                    continue;
                };

                player.pause_all();
                state.is_paused = true;
            }

            AnimationCommand::Resume { entity } => {
                let Ok((_, mut state)) = animators.get_mut(entity) else {
                    continue;
                };
                if !state.initialized {
                    continue;
                }

                let Some(player_entity) = state.player_entity else {
                    continue;
                };

                let Ok((mut player, _)) = players.get_mut(player_entity) else {
                    continue;
                };

                player.resume_all();
                state.is_paused = false;
            }

            AnimationCommand::SetSpeed { entity, speed } => {
                let Ok((_, state)) = animators.get_mut(entity) else {
                    continue;
                };
                if !state.initialized {
                    continue;
                }

                let Some(ref current) = state.current_clip else {
                    continue;
                };

                let Some(&node_idx) = state.node_indices.get(current.as_str()) else {
                    continue;
                };

                let Some(player_entity) = state.player_entity else {
                    continue;
                };

                let Ok((mut player, _)) = players.get_mut(player_entity) else {
                    continue;
                };

                if let Some(anim) = player.animation_mut(node_idx) {
                    anim.set_speed(speed);
                }
            }

            AnimationCommand::Crossfade {
                entity,
                name,
                duration,
                looping,
            } => {
                let Ok((_, mut state)) = animators.get_mut(entity) else {
                    continue;
                };
                if !state.initialized {
                    continue;
                }

                let Some(&node_idx) = state.node_indices.get(&name) else {
                    warn!("AnimationCommand::Crossfade: clip '{}' not found", name);
                    continue;
                };

                let Some(player_entity) = state.player_entity else {
                    continue;
                };

                let Ok((mut player, mut transitions)) = players.get_mut(player_entity) else {
                    continue;
                };

                let blend = Duration::from_secs_f32(duration.max(0.0));
                let playing = transitions.play(&mut player, node_idx, blend);
                if looping {
                    playing.repeat();
                }

                state.current_clip = Some(name);
                state.is_paused = false;
            }

            AnimationCommand::SetParam { entity, name, value } => {
                let Ok((_, mut state)) = animators.get_mut(entity) else {
                    continue;
                };
                state.params.set_float(name, value);
            }

            AnimationCommand::SetBoolParam { entity, name, value } => {
                let Ok((_, mut state)) = animators.get_mut(entity) else {
                    continue;
                };
                state.params.set_bool(name, value);
            }

            AnimationCommand::Trigger { entity, name } => {
                let Ok((_, mut state)) = animators.get_mut(entity) else {
                    continue;
                };
                state.params.fire_trigger(name);
            }

            AnimationCommand::SetLayerWeight {
                entity,
                layer_name,
                weight,
            } => {
                let Ok((mut animator, _)) = animators.get_mut(entity) else {
                    continue;
                };
                if let Some(layer) = animator
                    .layers
                    .iter_mut()
                    .find(|l| l.name == layer_name)
                {
                    layer.weight = weight.clamp(0.0, 1.0);
                }
            }
        }
    }
}

// ============================================================================
// State Machine
// ============================================================================

/// Evaluate state machine transitions and update the active animation state.
pub fn update_state_machines(
    time: Res<Time>,
    sm_assets: Res<Assets<AnimationStateMachine>>,
    mut animators: Query<(Entity, &AnimatorComponent, &mut AnimatorState)>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    for (_entity, _animator, mut state) in animators.iter_mut() {
        if !state.initialized || state.is_paused {
            continue;
        }

        let Some(ref sm_handle) = state.sm_handle else {
            continue;
        };

        let Some(sm) = sm_assets.get(sm_handle) else {
            continue;
        };

        // Initialize current state if not set
        if state.current_state.is_none() {
            state.current_state = Some(sm.default_state.clone());
            state.state_time = 0.0;
        }

        let Some(ref current_state_name) = state.current_state.clone() else {
            continue;
        };

        state.state_time += time.delta_secs();

        // Evaluate transitions
        if let Some(transition) = sm.evaluate_transitions(
            current_state_name,
            &state.params,
            state.state_time,
        ) {
            let to_state = transition.to.clone();
            let blend_duration = transition.blend_duration;

            // Consume trigger if the condition was a trigger
            if let crate::state_machine::AnimCondition::Trigger(ref trigger_name) = transition.condition {
                state.params.consume_trigger(trigger_name);
            }

            // Transition to the new state
            if let Some(target_state) = sm.get_state(&to_state) {
                let clip_name = match &target_state.motion {
                    StateMotion::Clip(name) => Some(name.clone()),
                    StateMotion::BlendTree(_) => {
                        // Blend trees are handled by update_blend_weights
                        None
                    }
                };

                if let Some(clip_name) = clip_name {
                    if let Some(&node_idx) = state.node_indices.get(&clip_name) {
                        if let Some(player_entity) = state.player_entity {
                            if let Ok((mut player, mut transitions)) = players.get_mut(player_entity) {
                                let blend = Duration::from_secs_f32(blend_duration.max(0.0));
                                let playing = transitions.play(&mut player, node_idx, blend);
                                if target_state.looping {
                                    playing.repeat();
                                }
                                playing.set_speed(target_state.speed);
                                state.current_clip = Some(clip_name);
                            }
                        }
                    }
                }

                state.current_state = Some(to_state);
                state.state_time = 0.0;
            }
        }
    }
}

/// Update layer weights on the animation player.
pub fn update_layer_weights(
    animators: Query<(&AnimatorComponent, &AnimatorState), Changed<AnimatorComponent>>,
    mut players: Query<&mut AnimationPlayer>,
) {
    for (animator, state) in animators.iter() {
        if !state.initialized || animator.layers.is_empty() {
            continue;
        }

        let Some(player_entity) = state.player_entity else {
            continue;
        };

        let Ok(mut player) = players.get_mut(player_entity) else {
            continue;
        };

        // Apply layer weights to active animations
        for layer in &animator.layers {
            if let Some(ref clip_name) = layer.current_clip {
                if let Some(&node_idx) = state.node_indices.get(clip_name.as_str()) {
                    if let Some(anim) = player.animation_mut(node_idx) {
                        anim.set_weight(layer.weight);
                    }
                }
            }
        }
    }
}

// ============================================================================
// Animation Finished Detection
// ============================================================================

/// Detect when a non-looping animation finishes and signal blueprint event nodes.
pub fn detect_animation_finished(
    animators: Query<(Entity, &AnimatorComponent, &AnimatorState)>,
    players: Query<&AnimationPlayer>,
    mut runtime_states: Query<&mut renzora_blueprint::interpreter::BlueprintRuntimeState>,
) {
    for (entity, animator, state) in animators.iter() {
        if !state.initialized || state.is_paused {
            continue;
        }

        let Some(ref current_clip) = state.current_clip else {
            continue;
        };

        // Check if the current clip is non-looping
        let slot = animator.get_slot(current_clip);
        let is_looping = slot.map_or(true, |s| s.looping);
        if is_looping {
            continue;
        }

        // Check if the animation has finished playing
        let Some(&node_idx) = state.node_indices.get(current_clip.as_str()) else {
            continue;
        };

        let Some(player_entity) = state.player_entity else {
            continue;
        };

        let Ok(player) = players.get(player_entity) else {
            continue;
        };

        if let Some(anim) = player.animation(node_idx) {
            if anim.is_finished() {
                // Signal the blueprint runtime state
                if let Ok(mut bp_runtime) = runtime_states.get_mut(entity) {
                    bp_runtime.anim_finished_clip = Some(current_clip.clone());
                }
            }
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Recursively search for an `AnimationPlayer` entity in the hierarchy.
fn find_animation_player(
    entity: Entity,
    children_query: &Query<&Children>,
    player_query: &Query<Entity, With<AnimationPlayer>>,
) -> Option<Entity> {
    if player_query.get(entity).is_ok() {
        return Some(entity);
    }
    if let Ok(children) = children_query.get(entity) {
        for child in children.iter() {
            if let Some(found) = find_animation_player(child, children_query, player_query) {
                return Some(found);
            }
        }
    }
    None
}

/// Add `AnimationTarget` components to all named entities in the skeleton hierarchy.
fn add_animation_targets(
    commands: &mut Commands,
    root: Entity,
    player_entity: Entity,
    children_query: &Query<&Children>,
    name_query: &Query<&Name>,
) {
    visit_skeleton(root, player_entity, commands, children_query, name_query);
}

fn visit_skeleton(
    entity: Entity,
    player_entity: Entity,
    commands: &mut Commands,
    children_query: &Query<&Children>,
    name_query: &Query<&Name>,
) {
    if let Ok(name) = name_query.get(entity) {
        let target_id = AnimationTargetId::from_name(name);
        commands.entity(entity).try_insert(target_id);
    }

    if let Ok(children) = children_query.get(entity) {
        for child in children.iter() {
            visit_skeleton(child, player_entity, commands, children_query, name_query);
        }
    }
}
