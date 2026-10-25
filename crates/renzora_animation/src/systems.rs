//! Runtime systems for the animation plugin.
//!
//! Playback goes through the animator's own controller
//! ([`crate::controller`]): commands and the state machine say *what* the base
//! should play, and `controller::drive_animators` turns that into clip weights
//! on the player each frame.

use bevy::animation::AnimationTargetId;
use bevy::prelude::*;

use crate::component::{AnimatorComponent, AnimatorState};
use crate::controller::Motion;
use crate::state_machine::{AnimState, AnimationStateMachine, StateMotion};

// ============================================================================
// Rehydration: build AnimationGraph when AnimatorComponent is added
// ============================================================================

/// Spawns `AnimatorState` for new `AnimatorComponent` entities and keeps
/// loaded clips in sync when slots are added, removed, or reordered in the
/// editor. The previous implementation ran once per entity — adding a second
/// clip after initialization wouldn't load or register it.
pub fn rehydrate_animators(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    new_animators: Query<(Entity, &AnimatorComponent), Without<AnimatorState>>,
    mut changed_animators: Query<
        (Entity, &AnimatorComponent, &mut AnimatorState),
        Changed<AnimatorComponent>,
    >,
) {
    for (entity, animator) in new_animators.iter() {
        if animator.clips.is_empty() {
            info!(
                "[animation] Rehydrate: {:?} has empty clips, skipping",
                entity
            );
            continue;
        }

        info!(
            "[animation] Rehydrate: {:?} with {} clips, loading...",
            entity,
            animator.clips.len()
        );

        let mut state = AnimatorState::default();

        for slot in &animator.clips {
            if !slot.path.is_empty() {
                info!(
                    "[animation]   Loading clip '{}' from '{}'",
                    slot.name, slot.path
                );
                let handle: Handle<AnimationClip> = asset_server.load(&slot.path);
                state.clip_handles.insert(slot.name.clone(), handle);
                let prop_handle = asset_server
                    .load::<crate::property_playback::PropertyClip>(&slot.path);
                state.prop_clip_handles.insert(slot.name.clone(), prop_handle);
            }
            if let Some(skel) = slot.skeleton_path() {
                state.skeleton_handles.insert(slot.name.clone(), asset_server.load(skel.to_string()));
            }
        }

        if let Some(ref sm_path) = animator.state_machine {
            if !sm_path.is_empty() {
                let handle: Handle<AnimationStateMachine> = asset_server.load(sm_path);
                state.sm_handle = Some(handle);
            }
        }

        commands.entity(entity).try_insert(state);
    }

    // Already-rehydrated animators: reload clip handles on slot changes and
    // flag the graph for rebuild. `current_clip` is cleared so auto-play
    // picks a valid clip once re-init finishes.
    for (entity, animator, mut state) in changed_animators.iter_mut() {
        let current_slot_names: std::collections::HashSet<String> =
            animator.clips.iter().map(|s| s.name.clone()).collect();

        state
            .clip_handles
            .retain(|name, _| current_slot_names.contains(name));
        state
            .prop_clip_handles
            .retain(|name, _| current_slot_names.contains(name));

        // A slot's source skeleton changing (or appearing) is a graph change,
        // caught by the signature below. Loading an already loaded path hands
        // back the same handle, so this only loads what is new.
        state.skeleton_handles = animator
            .clips
            .iter()
            .filter_map(|slot| Some((slot.name.clone(), asset_server.load(slot.skeleton_path()?.to_string()))))
            .collect();

        let mut added_any = false;
        for slot in &animator.clips {
            if slot.path.is_empty() {
                continue;
            }
            if !state.clip_handles.contains_key(&slot.name) {
                info!(
                    "[animation]   Loading new clip '{}' from '{}' on {:?}",
                    slot.name, slot.path, entity
                );
                let handle: Handle<AnimationClip> = asset_server.load(&slot.path);
                state.clip_handles.insert(slot.name.clone(), handle);
                added_any = true;
            }
            if !state.prop_clip_handles.contains_key(&slot.name) {
                let prop_handle = asset_server
                    .load::<crate::property_playback::PropertyClip>(&slot.path);
                state.prop_clip_handles.insert(slot.name.clone(), prop_handle);
            }
        }

        let reshaped = state.initialized && state.graph_signature != animator.graph_signature();
        if added_any || state.node_indices.len() != animator.clips.len() || reshaped {
            state.reset_graph();
        }
    }
}

// ============================================================================
// Auto-play default clip
// ============================================================================

/// After initialization, auto-play the default clip if specified.
///
/// Uses a simple flag check rather than `Changed<>` because deferred commands
/// from `initialize_animation_graphs` won't be applied until after the
/// current system chain finishes.
///
/// An animator whose state machine loaded leaves the base to the machine,
/// which enters its default state itself; both starting at once made the
/// first frame a crossfade between the default clip and the entry state.
pub fn auto_play_default(
    mut animators: Query<(&AnimatorComponent, &mut AnimatorState)>,
    graph_query: Query<(), With<AnimationGraphHandle>>,
    sm_assets: Res<Assets<AnimationStateMachine>>,
) {
    for (animator, mut state) in animators.iter_mut() {
        if !state.initialized {
            continue;
        }

        // Check if the animation graph was removed from the player (e.g. by scene reload)
        if let Some(player_entity) = state.player_entity {
            if graph_query.get(player_entity).is_err() && state.current_clip.is_some() {
                warn!("[animation] AnimationGraphHandle disappeared from player {:?}, re-initializing", player_entity);
                state.reset_graph();
                continue;
            }
        }

        if state.current_clip.is_some() || !state.base.is_idle() {
            continue;
        }
        if state.sm_handle.as_ref().is_some_and(|h| sm_assets.contains(h)) {
            continue;
        }

        let Some(ref default_name) = animator.default_clip else {
            continue;
        };

        if !state.layout.base.contains_key(default_name.as_str()) {
            warn!("[animation] default_clip '{}' not found in node_indices", default_name);
            continue;
        }

        let slot = animator.get_slot(default_name);
        let looping = slot.is_none_or(|s| s.looping);
        let speed = slot.map_or(1.0, |s| s.speed);

        state.base.play(Motion::Clip(default_name.clone()), animator.blend_duration.max(0.0), looping, speed);
        state.current_clip = Some(default_name.clone());
        info!("[animation] Auto-playing default clip '{}'", default_name);
    }
}

// ============================================================================
// Command processing: play/stop/pause/resume from scripts
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
    /// Seek playback to an absolute time (seconds).
    Seek {
        entity: Entity,
        time: f32,
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
    /// Play a clip on a layer, crossfading from whatever the layer played.
    PlayOnLayer {
        entity: Entity,
        layer_name: String,
        name: String,
    },
    /// Fade a layer's clip out, leaving the layers below it.
    StopLayer {
        entity: Entity,
        layer_name: String,
    },
}

/// Resource that collects animation commands each frame.
#[derive(Resource, Default)]
pub struct AnimationCommandQueue {
    pub commands: Vec<AnimationCommand>,
}

impl AnimationCommandQueue {
    pub fn push(&mut self, command: AnimationCommand) {
        self.commands.push(command);
    }
}

/// The current base motion's single clip, if it is one.
fn current_single_clip(state: &AnimatorState) -> Option<String> {
    match &state.base.current.as_ref()?.motion {
        Motion::Clip(name) => Some(name.clone()),
        Motion::Tree(_) => None,
    }
}

/// System that processes animation commands.
///
/// `Play` and `Crossfade` are dropped while editing, for the same reason
/// [`auto_play_default`] is gated: in the editor the timeline preview owns the
/// pose. Several editor controls (picking a clip in the timeline's clip menu or
/// the Animation panel, the inspector's default-clip dropdown) pushed a `Play`
/// as a side effect of *selecting* a clip, which started the player at full
/// speed, looping forever, with the timeline's own transport still reading
/// stopped and nothing in edit mode that would ever stop it. So adding a clip
/// to an animator and choosing it to edit set the model running in the
/// viewport. A shipped game has no `PlayModeState`, so nothing changes there.
pub fn process_animation_commands(
    mut cmd_queue: ResMut<AnimationCommandQueue>,
    mut animators: Query<(&mut AnimatorComponent, &mut AnimatorState)>,
    mut players: Query<&mut AnimationPlayer>,
    play_mode: Option<Res<renzora::PlayModeState>>,
) {
    let running = renzora::core::game_running(play_mode);
    if !cmd_queue.commands.is_empty() {
        debug!(
            "[animation] Processing {} command(s): {:?}",
            cmd_queue.commands.len(),
            cmd_queue.commands
        );
    }
    for cmd in cmd_queue.commands.drain(..) {
        match cmd {
            AnimationCommand::Play {
                entity,
                name,
                looping,
                speed,
            } => {
                if !running {
                    continue;
                }
                let Ok((animator, mut state)) = animators.get_mut(entity) else {
                    continue;
                };

                // Idempotent: scripts re-call `play_animation` every frame
                // because the Lua state doesn't persist locals. Skip restarting
                // if the clip is already the current one.
                let already =
                    state.base.current_is_clip(&name) && !state.is_paused && !state.prop_stopped;
                let prev_clip = current_single_clip(&state);

                // Property-animation state — applied even without a skeletal
                // player (so a sun / property-only entity responds to scripts).
                state.prop_speed = speed;
                state.prop_stopped = false;
                state.is_paused = false;
                if already {
                    continue;
                }
                state.current_clip = Some(name.clone());
                state.prop_time = 0.0;
                state.prop_started = false;

                if animator.get_slot(&name).is_none() {
                    warn!("AnimationCommand::Play: clip '{}' not found", name);
                    continue;
                }

                // Per-clip blend: prefer target.blend_in, else source.blend_out,
                // else the animator's global blend_duration.
                let target_blend_in = animator.get_slot(&name).and_then(|s| s.blend_in);
                let source_blend_out = prev_clip
                    .as_ref()
                    .and_then(|n| animator.get_slot(n))
                    .and_then(|s| s.blend_out);
                let blend_secs = target_blend_in
                    .or(source_blend_out)
                    .unwrap_or(animator.blend_duration)
                    .max(0.0);
                // Recorded even before the graph exists: the controller starts
                // it once the animator initialises, so a Play issued on the
                // first frame is not lost behind the idempotency check above.
                state.base.play(Motion::Clip(name), blend_secs, looping, speed);
            }

            AnimationCommand::Stop { entity } => {
                let Ok((_, mut state)) = animators.get_mut(entity) else {
                    continue;
                };
                state.prop_stopped = true;
                state.prop_time = 0.0;
                state.is_paused = false;
                state.base.clear();
                state.current_clip = None;
                if !state.initialized {
                    continue;
                }

                let Some(player_entity) = state.player_entity else {
                    continue;
                };

                let Ok(mut player) = players.get_mut(player_entity) else {
                    continue;
                };

                // Freeze every base clip on its first frame.
                for &node_idx in state.layout.base.values() {
                    if let Some(anim) = player.animation_mut(node_idx) {
                        anim.set_speed(0.0);
                        anim.seek_to(0.0);
                    }
                }
            }

            AnimationCommand::Pause { entity } => {
                let Ok((_, mut state)) = animators.get_mut(entity) else {
                    continue;
                };
                state.is_paused = true;
                if !state.initialized {
                    continue;
                }

                let Some(player_entity) = state.player_entity else {
                    continue;
                };

                let Ok(mut player) = players.get_mut(player_entity) else {
                    continue;
                };

                player.pause_all();
            }

            AnimationCommand::Resume { entity } => {
                let Ok((_, mut state)) = animators.get_mut(entity) else {
                    continue;
                };
                state.is_paused = false;
                state.prop_stopped = false;
                if !state.initialized {
                    continue;
                }

                let Some(player_entity) = state.player_entity else {
                    continue;
                };

                let Ok(mut player) = players.get_mut(player_entity) else {
                    continue;
                };

                player.resume_all();
            }

            AnimationCommand::Seek { entity, time } => {
                let Ok((_, mut state)) = animators.get_mut(entity) else {
                    continue;
                };
                let time = time.max(0.0);
                state.prop_time = time;
                if !state.initialized {
                    continue;
                }
                let Some(current) = state.base.current.as_ref() else {
                    continue;
                };
                let Some(player_entity) = state.player_entity else {
                    continue;
                };
                let Ok(mut player) = players.get_mut(player_entity) else {
                    continue;
                };
                // A tree's clips seek together, which keeps them in step.
                for clip in current.motion.clips() {
                    if let Some(anim) = state.layout.base.get(&clip).and_then(|n| player.animation_mut(*n)) {
                        anim.seek_to(time);
                    }
                }
            }

            AnimationCommand::SetSpeed { entity, speed } => {
                let Ok((_, mut state)) = animators.get_mut(entity) else {
                    continue;
                };
                state.prop_speed = speed;
                state.base.set_speed(speed);
                if !state.initialized {
                    continue;
                }

                let Some(current) = current_single_clip(&state) else {
                    continue;
                };

                let Some(&node_idx) = state.layout.base.get(current.as_str()) else {
                    continue;
                };

                let Some(player_entity) = state.player_entity else {
                    continue;
                };

                let Ok(mut player) = players.get_mut(player_entity) else {
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
                if !running {
                    continue;
                }
                let Ok((animator, mut state)) = animators.get_mut(entity) else {
                    continue;
                };
                state.prop_stopped = false;
                state.is_paused = false;
                if state.current_clip.as_deref() != Some(name.as_str()) {
                    state.prop_time = 0.0;
                    state.prop_started = false;
                }
                state.current_clip = Some(name.clone());

                let Some(slot) = animator.get_slot(&name) else {
                    warn!("AnimationCommand::Crossfade: clip '{}' not found", name);
                    continue;
                };
                let speed = slot.speed;
                state.base.play(Motion::Clip(name), duration.max(0.0), looping, speed);
            }

            AnimationCommand::SetParam {
                entity,
                name,
                value,
            } => {
                let Ok((_, mut state)) = animators.get_mut(entity) else {
                    continue;
                };
                state.params.set_float(name, value);
            }

            AnimationCommand::SetBoolParam {
                entity,
                name,
                value,
            } => {
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
                if let Some(layer) = animator.layers.iter_mut().find(|l| l.name == layer_name) {
                    layer.weight = weight.clamp(0.0, 1.0);
                }
            }

            AnimationCommand::PlayOnLayer { entity, layer_name, name } => {
                let Ok((mut animator, _)) = animators.get_mut(entity) else {
                    continue;
                };
                if animator.get_slot(&name).is_none() {
                    warn!("AnimationCommand::PlayOnLayer: clip '{}' not found", name);
                    continue;
                }
                match animator.layers.iter_mut().find(|l| l.name == layer_name) {
                    // Written only when it changes: scripts call this every
                    // frame, and each write marks the component changed.
                    Some(layer) if layer.current_clip.as_deref() != Some(name.as_str()) => {
                        layer.current_clip = Some(name);
                    }
                    Some(_) => {}
                    None => warn!("AnimationCommand::PlayOnLayer: layer '{}' not found", layer_name),
                }
            }

            AnimationCommand::StopLayer { entity, layer_name } => {
                let Ok((mut animator, _)) = animators.get_mut(entity) else {
                    continue;
                };
                if let Some(layer) = animator.layers.iter_mut().find(|l| l.name == layer_name) {
                    if layer.current_clip.is_some() {
                        layer.current_clip = None;
                    }
                }
            }
        }
    }
}

// ============================================================================
// State Machine
// ============================================================================

/// The base motion a state plays.
fn state_motion(state: &AnimState) -> Motion {
    match &state.motion {
        StateMotion::Clip(name) | StateMotion::BlendTree(name) => Motion::Clip(name.clone()),
        StateMotion::Blend(tree) => Motion::Tree(tree.clone()),
    }
}

/// Enter `target`. While editing only the current clip follows the machine
/// (the timeline preview shows it, which is how the Parameters panel previews
/// a transition); the base controller, which would drive the player, is left
/// to gameplay.
fn enter_state(animator_state: &mut AnimatorState, target: &AnimState, blend: f32, running: bool) {
    let motion = state_motion(target);
    if let Motion::Clip(name) = &motion {
        animator_state.current_clip = Some(name.clone());
    }
    if running {
        animator_state.base.play(motion, blend.max(0.0), target.looping, target.speed);
        animator_state.sm_playing = true;
    }
}

/// Evaluate state machine transitions and update the active animation state.
///
/// Entering a state (the default one included, which used to be selected and
/// then never played) hands its clip or blend tree to the base controller.
pub fn update_state_machines(
    time: Res<Time>,
    sm_assets: Res<Assets<AnimationStateMachine>>,
    mut animators: Query<&mut AnimatorState>,
    play_mode: Option<Res<renzora::PlayModeState>>,
) {
    let running = renzora::core::game_running(play_mode);
    for mut state in animators.iter_mut() {
        if !state.initialized || state.is_paused {
            continue;
        }

        let Some(sm_handle) = state.sm_handle.clone() else {
            continue;
        };

        let Some(sm) = sm_assets.get(&sm_handle) else {
            continue;
        };

        if state.current_state.is_none() {
            state.current_state = Some(sm.default_state.clone());
            state.state_time = 0.0;
            if let Some(entry) = sm.get_state(&sm.default_state) {
                enter_state(&mut state, entry, 0.0, running);
            }
        }

        let Some(current_state_name) = state.current_state.clone() else {
            continue;
        };

        // Entered while editing, so nothing is playing it yet: play starting
        // in the same world picks the state up where it is.
        if running && !state.sm_playing {
            if let Some(entry) = sm.get_state(&current_state_name) {
                enter_state(&mut state, entry, 0.0, running);
            }
        }

        state.state_time += time.delta_secs();

        // Evaluate transitions
        if let Some(transition) =
            sm.evaluate_transitions(&current_state_name, &state.params, state.state_time)
        {
            let to_state = transition.to.clone();
            let blend_duration = transition.blend_duration;

            // Consume trigger if the condition was a trigger
            if let crate::state_machine::AnimCondition::Trigger(ref trigger_name) =
                transition.condition
            {
                state.params.consume_trigger(trigger_name);
            }

            if let Some(target_state) = sm.get_state(&to_state) {
                enter_state(&mut state, target_state, blend_duration, running);
                state.current_state = Some(to_state);
                state.state_time = 0.0;
            }
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Recursively search for an `AnimationPlayer` entity in the hierarchy.
pub(crate) fn find_animation_player(
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
pub(crate) fn add_animation_targets(
    commands: &mut Commands,
    root: Entity,
    player_entity: Entity,
    children_query: &Query<&Children>,
    name_query: &Query<(&Name, Option<&renzora::BindName>)>,
) {
    visit_skeleton(root, player_entity, commands, children_query, name_query);
}

fn visit_skeleton(
    entity: Entity,
    player_entity: Entity,
    commands: &mut Commands,
    children_query: &Query<&Children>,
    name_query: &Query<(&Name, Option<&renzora::BindName>)>,
) {
    if let Ok((name, bind)) = name_query.get(entity) {
        // Bevy 0.18 requires two components on an animated entity:
        // `AnimationTargetId` identifies which curves in the clip apply here,
        // and `AnimatedBy` points back at the AnimationPlayer that drives it.
        // Missing `AnimatedBy` means clips silently do nothing.
        //
        // Through `bone_target` rather than `from_name` directly, so this agrees
        // with the clip loader no matter whether `enforce_entity_ids` has
        // renamed the bone yet. See [`crate::bone_target`]. A name made
        // unique with a suffix binds by the name it had (`renzora::BindName`).
        let target_id = crate::bone_target(renzora::BindName::of(name, bind));
        commands
            .entity(entity)
            .try_insert((target_id, bevy::animation::AnimatedBy(player_entity)));
    }

    if let Ok(children) = children_query.get(entity) {
        for child in children.iter() {
            visit_skeleton(child, player_entity, commands, children_query, name_query);
        }
    }
}

/// Keeps `AnimationTargetId` + `AnimatedBy` in sync for named descendants of
/// every initialized animator. Bones can spawn on any frame — on scene reload
/// the GLB may still be instantiating when `initialize_animation_graphs` runs,
/// which leaves the first pass with no bones to tag. Running this each frame
/// is cheap (`try_insert` is a no-op when components already exist) and
/// guarantees targets exist once the skeleton is populated.
/// How long after initialization we keep re-walking the skeleton looking for
/// newly-spawned bones. 120 frames @ 60 Hz = 2 seconds, which is plenty for
/// any GLB to finish instantiating on modern hardware. After that we stop
/// until new named entities appear, so the per-frame cost drops to
/// O(animators), not O(bones).
const TARGET_WALK_FRAMES: u32 = 120;

pub fn ensure_animation_targets(
    mut commands: Commands,
    mut animators: Query<(Entity, &mut AnimatorState)>,
    children_query: Query<&Children>,
    name_query: Query<(&Name, Option<&renzora::BindName>)>,
    target_query: Query<(), With<AnimationTargetId>>,
    fresh_names: Query<Entity, (Added<Name>, Without<AnimationTargetId>)>,
    parents: Query<&ChildOf>,
) {
    // A model respawned in place (rigging it, removing its rig, reloading its
    // file) brings a whole new skeleton under an animator that finished its
    // warm-up long ago. Its old bones were tagged; the new ones never were, so
    // the clip kept its keys and moved nothing until the editor restarted.
    // A named entity arriving untagged under an animator opens that animator's
    // window again. Under that animator only: names arrive all the time
    // elsewhere (UI rows, a game's spawns), and each would otherwise restart
    // every animator's walk.
    let mut respawned: Vec<Entity> = Vec::new();
    for fresh in &fresh_names {
        let mut at = fresh;
        while let Ok(up) = parents.get(at) {
            at = up.parent();
            if animators.contains(at) {
                if !respawned.contains(&at) {
                    respawned.push(at);
                }
                break;
            }
        }
    }
    for (entity, mut state) in animators.iter_mut() {
        if !state.initialized {
            continue;
        }
        let Some(player_entity) = state.player_entity else {
            continue;
        };
        if respawned.contains(&entity) {
            state.frames_since_init = 0;
            // A new skeleton is new bone entities: the cached root bone is gone.
            state.root_bone = None;
        }

        // Warm-up is bounded — 2 seconds of walking is more than enough for
        // any GLB to finish spawning. After that we assume the skeleton is
        // stable until something new spawns (above).
        if state.frames_since_init > TARGET_WALK_FRAMES {
            continue;
        }
        state.frames_since_init = state.frames_since_init.saturating_add(1);

        let mut stack = vec![entity];
        while let Some(current) = stack.pop() {
            if let Ok((name, bind)) = name_query.get(current) {
                if target_query.get(current).is_err() {
                    let target_id = crate::bone_target(renzora::BindName::of(name, bind));
                    commands
                        .entity(current)
                        .try_insert((target_id, bevy::animation::AnimatedBy(player_entity)));
                }
            }
            if let Ok(children) = children_query.get(current) {
                stack.extend(children.iter());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// The rig-rebuild case: an animator long past its warm-up gets a fresh
    /// skeleton, and the new bones must still be bound to its player.
    #[test]
    fn a_skeleton_spawned_after_warm_up_is_still_bound() {
        let mut world = World::new();
        let animator = world.spawn_empty().id();
        world.entity_mut(animator).insert(AnimatorState {
            initialized: true,
            player_entity: Some(animator),
            frames_since_init: TARGET_WALK_FRAMES * 10,
            ..Default::default()
        });
        let bone = world.spawn((Name::new("face_jaw"), ChildOf(animator))).id();
        world.run_system_once(ensure_animation_targets).unwrap();
        assert_eq!(world.get::<AnimationTargetId>(bone), Some(&crate::bone_target("face_jaw")));
        assert_eq!(world.get::<bevy::animation::AnimatedBy>(bone).map(|b| b.0), Some(animator));
    }

    /// A name spawning outside every animator (a UI row, a game's projectile)
    /// leaves a settled animator settled.
    #[test]
    fn a_name_elsewhere_does_not_wake_an_animator() {
        let mut world = World::new();
        let animator = world.spawn_empty().id();
        world.entity_mut(animator).insert(AnimatorState {
            initialized: true,
            player_entity: Some(animator),
            frames_since_init: TARGET_WALK_FRAMES * 10,
            ..Default::default()
        });
        world.spawn(Name::new("bullet"));
        world.run_system_once(ensure_animation_targets).unwrap();
        assert_eq!(world.get::<AnimatorState>(animator).map(|s| s.frames_since_init), Some(TARGET_WALK_FRAMES * 10));
    }

    /// Entering a state hands its motion to the base controller: a blend tree
    /// as a tree, a clip as a clip that also becomes the current clip.
    #[test]
    fn entering_a_blend_tree_state_plays_the_tree() {
        use crate::blend_tree::{Blend1DEntry, BlendTree};
        let tree = BlendTree::Blend1D {
            param: "speed".into(),
            entries: vec![Blend1DEntry { clip: "walk".into(), threshold: 0.0 }],
        };
        let target = AnimState { name: "move".into(), motion: StateMotion::Blend(tree.clone()), speed: 1.0, looping: true };
        let mut state = AnimatorState::default();
        enter_state(&mut state, &target, 0.2, false);
        assert!(state.base.is_idle(), "editing never drives the player");
        enter_state(&mut state, &target, 0.2, true);
        assert!(matches!(&state.base.current, Some(c) if c.motion == Motion::Tree(tree.clone()) && c.needs_start));
        let clip = AnimState { name: "idle".into(), motion: StateMotion::Clip("idle".into()), speed: 1.0, looping: true };
        enter_state(&mut state, &clip, 0.2, true);
        assert_eq!(state.current_clip.as_deref(), Some("idle"));
        assert!(state.base.current_is_clip("idle"));
    }
}
