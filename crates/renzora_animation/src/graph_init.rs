//! Building an animator's graph once its clips have loaded.
//!
//! Most animators need nothing but their clips. Three features need more, and
//! wait for it (up to [`INIT_WAIT_LIMIT`] frames, then go ahead with what
//! arrived, so a missing sidecar file degrades the feature instead of the
//! whole animator):
//!
//! - **retargeting** needs each slot's `.skel` and the clip's raw keys (the
//!   `PropertyClip` half of the `.anim`), and the skeleton spawned, since the
//!   target rest pose is read off it;
//! - **additive** layers and blend tree overlays need the raw keys, to build
//!   the difference clips;
//! - **masks** and **root motion** need the skeleton spawned, to resolve a
//!   subtree or find the root bone.

use std::collections::{HashMap, HashSet};

use bevy::animation::transition::AnimationTransitions;
use bevy::asset::LoadState;
use bevy::prelude::*;

use crate::component::{AnimatorComponent, AnimatorState};
use crate::graph_builder::{build_animator_graph, GraphSpec, LayerSpec};
use crate::layers::{mask_includes, LayerBlendMode};
use crate::property_playback::PropertyClip;
use crate::retarget::{capture_skeleton, make_additive, map_bone_name, retarget_clip, SourceSkeleton};
use crate::state_machine::AnimationStateMachine;
use crate::systems::{add_animation_targets, find_animation_player};

/// How long (in frames) initialisation waits for a skeleton or a sidecar
/// asset before building without it. About five seconds at 60 Hz.
pub const INIT_WAIT_LIMIT: u32 = 300;

fn settled(asset_server: &AssetServer, id: impl Into<bevy::asset::UntypedAssetId>) -> bool {
    matches!(asset_server.get_load_state(id), Some(LoadState::Loaded) | Some(LoadState::Failed(_)) | None)
}

/// Once all clips are loaded, build the AnimationGraph and wire up the player.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn initialize_animation_graphs(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut clip_assets: ResMut<Assets<AnimationClip>>,
    prop_clips: Res<Assets<PropertyClip>>,
    skeletons: Res<Assets<SourceSkeleton>>,
    state_machines: Res<Assets<AnimationStateMachine>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut animators: Query<(Entity, &AnimatorComponent, &mut AnimatorState)>,
    children_query: Query<&Children>,
    player_query: Query<Entity, With<AnimationPlayer>>,
    name_query: Query<(&Name, Option<&renzora::BindName>)>,
    transforms: Query<&Transform>,
) {
    for (entity, animator, mut state) in animators.iter_mut() {
        if state.initialized || state.clip_handles.is_empty() {
            continue;
        }

        // Check if all clips are loaded
        let mut loaded_count = 0;
        let mut failed_count = 0;
        for (name, handle) in state.clip_handles.iter() {
            match asset_server.get_load_state(handle.id()) {
                Some(LoadState::Loaded) => loaded_count += 1,
                Some(LoadState::Failed(_)) => {
                    warn!("[animation] Clip '{}' FAILED to load", name);
                    failed_count += 1;
                }
                _ => {}
            }
        }
        let total = state.clip_handles.len();
        if loaded_count + failed_count < total {
            continue;
        }
        if loaded_count == 0 {
            warn!("[animation] All {} clips failed to load for {:?}", total, entity);
            state.initialized = true; // Don't retry
            continue;
        }

        // What this animator needs beyond its clips.
        let retarget = animator.clips.iter().any(|s| s.skeleton_path().is_some()) || !animator.bone_map.is_empty();
        let sm = state.sm_handle.as_ref().and_then(|h| state_machines.get(h));
        let sm_pending = sm.is_none() && state.sm_handle.as_ref().is_some_and(|h| !settled(&asset_server, h.id()));
        let overlay_slots: Vec<String> = sm.map(|sm| sm.additive_clips()).unwrap_or_default();
        let additive_layer = animator.layers.iter().any(|l| l.blend_mode == LayerBlendMode::Additive);
        let masked = animator.layers.iter().any(|l| l.active_mask().is_some());
        let need_source = retarget || additive_layer || !overlay_slots.is_empty();
        let need_skeleton = retarget || masked || animator.root_motion.enabled();

        let source_pending = need_source && state.prop_clip_handles.values().any(|h| !settled(&asset_server, h.id()));
        let skel_pending = retarget && state.skeleton_handles.values().any(|h| !settled(&asset_server, h.id()));
        let bone_names: HashSet<String> = state
            .prop_clip_handles
            .values()
            .filter_map(|h| prop_clips.get(h))
            .filter_map(|c| c.skeletal.as_ref())
            .flat_map(|c| c.tracks.iter().map(|t| renzora::sanitize_id(&map_bone_name(&animator.bone_map, &t.bone_name))))
            .collect();
        let skeleton_missing = need_skeleton && !has_bones(entity, &bone_names, &children_query, &name_query);
        if (sm_pending || source_pending || skel_pending || skeleton_missing) && state.init_wait_frames < INIT_WAIT_LIMIT {
            state.init_wait_frames += 1;
            continue;
        }
        if state.init_wait_frames >= INIT_WAIT_LIMIT {
            warn!(
                "[animation] {:?}: building without (state machine {}, raw clips {}, skeleton files {}, skeleton {})",
                entity, sm_pending, source_pending, skel_pending, skeleton_missing
            );
        }

        // Captured on the first initialisation whatever this animator needs
        // now, while the model is fresh from spawning and so in its rest pose:
        // a rebuild later (root motion or a mask switched on mid-play) would
        // otherwise capture whatever pose the character was in.
        if state.target_rest.is_none() {
            let rest = capture_skeleton(
                entity,
                |e| children_query.get(e).map(|c| c.iter().collect()).unwrap_or_default(),
                |e| {
                    let (name, bind) = name_query.get(e).ok()?;
                    let t = transforms.get(e).ok()?;
                    Some((renzora::BindName::of(name, bind).to_string(), *t))
                },
            );
            if !rest.is_empty() {
                state.target_rest = Some(rest);
            }
        }

        // The clip each slot plays, and its difference clip where one is needed.
        let state = &mut *state;
        state.derived_clips.clear();
        let mut base: Vec<(String, Handle<AnimationClip>)> = Vec::new();
        let mut additive: HashMap<String, Handle<AnimationClip>> = HashMap::new();
        for slot in &animator.clips {
            let Some(handle) = state.clip_handles.get(&slot.name).cloned() else { continue };
            if clip_assets.get(&handle).is_none() {
                continue;
            }
            let source = state.prop_clip_handles.get(&slot.name).and_then(|h| prop_clips.get(h)).and_then(|c| c.skeletal.clone());
            let mut played = handle;
            let mut moved = None;
            if retarget {
                let src = state.skeleton_handles.get(&slot.name).and_then(|h| skeletons.get(h)).map(|s| s.skeleton.clone());
                if let (Some(raw), Some(dst)) = (source.as_deref(), state.target_rest.as_ref()) {
                    if src.is_some() || !animator.bone_map.is_empty() {
                        let clip = retarget_clip(raw, src.as_deref(), dst, &animator.bone_map, animator.root_motion.bone.as_deref());
                        played = clip_assets.add(crate::loader::to_animation_clip(&clip));
                        state.derived_clips.push(played.clone());
                        moved = Some(clip);
                    }
                }
            }
            if additive_layer || overlay_slots.contains(&slot.name) {
                if let Some(keys) = moved.as_ref().or(source.as_deref()) {
                    let delta = clip_assets.add(crate::loader::to_animation_clip(&make_additive(keys)));
                    state.derived_clips.push(delta.clone());
                    additive.insert(slot.name.clone(), delta);
                }
            }
            base.push((slot.name.clone(), played));
        }
        if base.is_empty() {
            warn!("[animation] No valid clip assets found despite loaded status");
            continue;
        }

        let layers: Vec<LayerSpec> = animator
            .layers
            .iter()
            .map(|layer| LayerSpec {
                mode: layer.blend_mode,
                masked_out: layer.active_mask().map(|mask| {
                    let mut universe: HashSet<String> = bone_names.clone();
                    if let Some(rest) = &state.target_rest {
                        universe.extend(rest.keys().map(str::to_string));
                    }
                    universe
                        .iter()
                        .filter(|bone| {
                            let path = state.target_rest.as_ref().map(|r| r.path(bone)).filter(|p| !p.is_empty());
                            let path = path.unwrap_or_else(|| vec![bone.as_str()]);
                            !mask_includes(mask, &path)
                        })
                        .map(|bone| crate::bone_target(bone))
                        .collect()
                }),
            })
            .collect();

        let (graph, layout) = build_animator_graph(&GraphSpec {
            base: &base,
            additive: &additive,
            overlay_slots: &overlay_slots,
            layers: &layers,
        });
        state.node_indices = layout.base.clone();
        state.layout = layout;
        let graph_handle = graphs.add(graph);
        state.graph_handle = Some(graph_handle.clone());

        // Find an AnimationPlayer already in the hierarchy (bevy_gltf inserts
        // one when a GLB carries embedded animations). If the model was
        // imported without embedded animations (a Mixamo character with side-
        // loaded `.anim` clips, say) there's no player yet, so insert one on the
        // AnimatorComponent entity itself. It's the natural animation root
        // because all skinned descendants hang beneath it.
        let player_entity = match find_animation_player(entity, &children_query, &player_query) {
            Some(e) => e,
            None => {
                info!("[animation] No AnimationPlayer found for {:?}; inserting one (side-loaded clips)", entity);
                commands.entity(entity).insert(AnimationPlayer::default());
                entity
            }
        };
        state.player_entity = Some(player_entity);

        add_animation_targets(&mut commands, entity, player_entity, &children_query, &name_query);

        // The animator sets every weight itself (`crate::controller`).
        // `AnimationTransitions` would overwrite the weight of whatever it
        // thinks is main each frame, so it must not be on the player.
        commands
            .entity(player_entity)
            .insert(AnimationGraphHandle(graph_handle))
            .remove::<AnimationTransitions>();
        // A rebuild (a clip added or removed) renumbers the graph's nodes, but
        // the player keeps whatever it was already running under the OLD
        // numbers, which now name different clips or none. Those leftovers
        // kept playing beside whatever was picked next, so after re-adding an
        // animator and its clips one by one, choosing another clip appeared
        // to do nothing. Start the new graph from a clean player.
        commands.entity(player_entity).entry::<AnimationPlayer>().and_modify(|mut player| {
            player.stop_all();
        });

        state.graph_signature = animator.graph_signature();
        state.initialized = true;
        info!(
            "AnimatorComponent: initialized {} clips ({} derived) on {:?}, player at {:?}",
            base.len(),
            state.derived_clips.len(),
            entity,
            player_entity
        );
    }
}

/// Whether the skeleton has spawned under `root`: any descendant named like
/// one of the clips' bones, or, with no bone names known, any named
/// descendant at all.
fn has_bones(
    root: Entity,
    bone_names: &HashSet<String>,
    children: &Query<&Children>,
    names: &Query<(&Name, Option<&renzora::BindName>)>,
) -> bool {
    let mut stack: Vec<Entity> = children.get(root).map(|c| c.iter().collect()).unwrap_or_default();
    while let Some(e) = stack.pop() {
        if let Ok((name, bind)) = names.get(e) {
            if bone_names.is_empty() || bone_names.contains(&renzora::sanitize_id(renzora::BindName::of(name, bind))) {
                return true;
            }
        }
        if let Ok(c) = children.get(e) {
            stack.extend(c.iter());
        }
    }
    false
}
