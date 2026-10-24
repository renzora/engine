//! Posing bones by hand, and keying the pose into the clip.
//!
//! In the editor the clip's `AnimationPlayer` is seeked to the playhead every
//! frame and rewrites every bone it has a curve for, so a bone turned with the
//! gizmo would snap back the next frame. [`PoseLayer`] holds what you posed on
//! top of the clip: [`capture_pose`] notices a selected bone changing by any
//! hand (the gizmo, the inspector), and [`apply_pose`] puts the pose back after
//! the player has written. It is the same rule animators know from other
//! tools: a pose that is not keyed lasts until the playhead moves.
//!
//! Keying ([`key_pose_bones`], K over the viewport or the timeline) writes each
//! posed or selected bone's local transform into its bone track at the
//! playhead, making the track if the bone had none. The loaded clip is rebuilt
//! in memory straight away, so the key plays on the next frame rather than
//! once autosave has written the file and the watcher has reloaded it. With
//! record armed, every change is keyed as you make it.

use std::collections::HashMap;

use bevy::animation::AnimationTargetId;
use bevy::prelude::*;

use renzora::BoneTrack;
use renzora_animation::AnimatorComponent;

use crate::AnimationEditorState;

use super::clip::TimelineClip;
use super::props::TimelineOps;

/// Two keys closer than this (seconds) are the same key: keying again at the
/// playhead replaces the value instead of stacking a second key on it.
const SAME_KEY: f32 = 1e-3;

/// The hand-made pose sitting on top of the clip. See the module doc.
#[derive(Resource, Default)]
pub(crate) struct PoseLayer {
    /// Bones posed by hand at this playhead, with the local pose they hold.
    pub(crate) posed: HashMap<Entity, Transform>,
    /// What each selected bone held when last frame finished, so an edit made
    /// in between is noticed.
    last: HashMap<Entity, Transform>,
    /// The playhead and clip the pose was made at: it is dropped when either
    /// moves, exactly like an unkeyed pose elsewhere.
    at: Option<(u32, Option<(Entity, String)>)>,
    /// Bones changed this frame with record armed, keyed by [`auto_key_pose`].
    autokey: Vec<Entity>,
    /// Bones changed by hand this frame, for anything that reacts to a pose
    /// edit as it happens (IK, in the Rig panel).
    pub(crate) changed: Vec<Entity>,
}

impl PoseLayer {
    /// What `bone` held at the end of last frame, before this frame's edit.
    pub(crate) fn last_of(&self, bone: Entity) -> Option<Transform> {
        self.last.get(&bone).copied()
    }
}

fn close(a: &Transform, b: &Transform) -> bool {
    a.translation.abs_diff_eq(b.translation, 1e-5)
        && a.rotation.abs_diff_eq(b.rotation, 1e-5)
        && a.scale.abs_diff_eq(b.scale, 1e-5)
}

/// Notice selected bones that changed since last frame (in PostUpdate, before
/// the player writes the clip's pose over them), and drop the pose when the
/// playhead or clip moves or playback runs.
pub(crate) fn capture_pose(
    state: Option<Res<AnimationEditorState>>,
    cache: Res<TimelineClip>,
    selection: Res<renzora_editor_framework::EditorSelection>,
    bones: Query<&Transform, With<AnimationTargetId>>,
    mut layer: ResMut<PoseLayer>,
) {
    let Some(state) = state else { return };
    layer.changed.clear();
    // Frame-quantised, so float noise in the playhead does not read as a move.
    let at = Some(((state.scrub_time * 1000.0).round() as u32, cache.key.clone()));
    if state.is_previewing || layer.at != at {
        layer.posed.clear();
        layer.last.clear();
        layer.at = at;
        if state.is_previewing {
            return;
        }
    }
    for bone in selection.get_all() {
        let Ok(now) = bones.get(bone) else { continue };
        let Some(before) = layer.last.get(&bone) else { continue };
        if close(now, before) {
            continue;
        }
        layer.posed.insert(bone, *now);
        layer.changed.push(bone);
        if state.record_enabled && !layer.autokey.contains(&bone) {
            layer.autokey.push(bone);
        }
    }
}

/// Put the hand-made pose back over the clip's (in PostUpdate, after the
/// player and before transforms propagate), and remember what each selected
/// bone ended the frame holding.
pub(crate) fn apply_pose(
    selection: Res<renzora_editor_framework::EditorSelection>,
    mut bones: Query<&mut Transform, With<AnimationTargetId>>,
    mut layer: ResMut<PoseLayer>,
) {
    let PoseLayer { posed, last, .. } = &mut *layer;
    posed.retain(|bone, pose| match bones.get_mut(*bone) {
        Ok(mut t) => {
            if !close(&t, pose) {
                *t = *pose;
            }
            true
        }
        Err(_) => false,
    });
    last.clear();
    for bone in selection.get_all() {
        if let Ok(t) = bones.get(bone) {
            last.insert(bone, *t);
        }
    }
}

/// K over the viewport or the stage: key the pose. The timeline's own K
/// already does this (and keys its property tracks too); this is the same key
/// where you pose.
#[allow(clippy::too_many_arguments)]
pub(crate) fn viewport_key_shortcut(
    keys: Res<ButtonInput<KeyCode>>,
    viewport: Option<Res<renzora::core::viewport_types::ViewportState>>,
    stage: Option<Res<crate::stage::view::StageCursor>>,
    focus: Option<Res<renzora::InputFocusState>>,
    layer: Res<PoseLayer>,
    selection: Res<renzora_editor_framework::EditorSelection>,
    bones: Query<(), With<AnimationTargetId>>,
    mut ops: ResMut<TimelineOps>,
) {
    if !keys.just_pressed(KeyCode::KeyK)
        || keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::SuperLeft, KeyCode::SuperRight])
        || focus.is_some_and(|f| f.ui_wants_keyboard)
        || !(viewport.is_some_and(|v| v.hovered) || stage.is_some_and(|s| s.over))
    {
        return;
    }
    let has_bones = !layer.posed.is_empty() || selection.get_all().into_iter().any(|e| bones.contains(e));
    if has_bones {
        ops.add_key = true;
    }
}

/// Record armed: key each bone changed this frame.
pub(crate) fn auto_key_pose(world: &mut World) {
    let bones = match world.get_resource_mut::<PoseLayer>() {
        Some(mut l) if !l.autokey.is_empty() => std::mem::take(&mut l.autokey),
        _ => return,
    };
    key_bones(world, &bones);
}

/// Key every posed bone and every selected bone at the playhead. Returns
/// whether anything was keyed.
pub(crate) fn key_pose_bones(world: &mut World) -> bool {
    let mut bones: Vec<Entity> = world
        .get_resource::<PoseLayer>()
        .map(|l| l.posed.keys().copied().collect())
        .unwrap_or_default();
    let selected = world
        .get_resource::<renzora_editor_framework::EditorSelection>()
        .map(|s| s.get_all())
        .unwrap_or_default();
    for e in selected {
        if world.get::<AnimationTargetId>(e).is_some() && !bones.contains(&e) {
            bones.push(e);
        }
    }
    if bones.is_empty() {
        return false;
    }
    key_bones(world, &bones)
}

/// Write each bone's local transform into its track at the playhead, then
/// rebuild the playing clip from the edit buffer.
pub(crate) fn key_bones(world: &mut World, bones: &[Entity]) -> bool {
    let Some(time) = world.get_resource::<AnimationEditorState>().map(|s| s.scrub_time) else {
        return false;
    };
    let keys: Vec<(Entity, f32, Transform)> =
        bones.iter().filter_map(|&b| Some((b, time, *world.get::<Transform>(b)?))).collect();
    key_transforms(world, &keys)
}

/// Key `(bone, time, local transform)` triples into the open clip, then
/// rebuild the playing clip from the edit buffer. For keying many times at
/// once (generated blinks, lip sync) as well as the pose at the playhead.
pub(crate) fn key_transforms(world: &mut World, keys: &[(Entity, f32, Transform)]) -> bool {
    // Each bone with the key playback binds it by. A track is found by that
    // key, never by its spelling: an imported clip says `mixamorig:LeftUpLeg`
    // while the entity it plays on is named `mixamorig_leftupleg`, and matching
    // the strings would have added a second track for the same bone. A bone
    // with no target yet (a rig just built, before an animator has walked it)
    // is bound by its name the same way playback will bind it.
    let poses: Vec<(AnimationTargetId, String, f32, Transform)> = keys
        .iter()
        .filter_map(|&(b, time, pose)| {
            let name = renzora::BindName::of(world.get::<Name>(b)?, world.get::<renzora::BindName>(b)).to_string();
            let target = world.get::<AnimationTargetId>(b).copied().unwrap_or_else(|| renzora_animation::bone_target(&name));
            Some((target, name, time, pose))
        })
        .collect();
    if poses.is_empty() {
        return false;
    }
    let rebuilt = {
        let Some(mut cache) = world.get_resource_mut::<TimelineClip>() else { return false };
        if cache.is_effect_only() {
            return false;
        }
        let Some(clip) = cache.clip.as_mut() else { return false };
        for (target, name, time, pose) in &poses {
            let time = *time;
            let track = match clip.tracks.iter().position(|t| renzora_animation::bone_target(&t.bone_name) == *target) {
                Some(i) => &mut clip.tracks[i],
                None => {
                    clip.tracks.push(BoneTrack {
                        bone_name: name.clone(),
                        translations: Vec::new(),
                        rotations: Vec::new(),
                        scales: Vec::new(),
                        muted: false,
                    });
                    clip.tracks.last_mut().expect("just pushed")
                }
            };
            upsert(&mut track.translations, time, pose.translation.to_array());
            upsert(&mut track.rotations, time, pose.rotation.normalize().to_array());
            upsert(&mut track.scales, time, pose.scale.to_array());
        }
        let rebuilt = renzora_animation::to_animation_clip(clip);
        cache.dirty = true;
        rebuilt
    };
    replace_loaded_clip(world, rebuilt);
    true
}

/// Set `value` at `time`, replacing a key already there, keeping time order.
fn upsert<V>(keys: &mut Vec<(f32, V)>, time: f32, value: V) {
    match keys.iter().position(|(t, _)| (t - time).abs() < SAME_KEY) {
        Some(i) => keys[i].1 = value,
        None => {
            let at = keys.iter().position(|(t, _)| *t > time).unwrap_or(keys.len());
            keys.insert(at, (time, value));
        }
    }
}

/// Swap the clip the player is using for one built from the edit buffer. The
/// handle stays the same, so the animation graph keeps pointing at it.
fn replace_loaded_clip(world: &mut World, clip: AnimationClip) {
    let path = (|| {
        let (entity, name) = world.get_resource::<TimelineClip>()?.key.clone()?;
        let animator = world.get::<AnimatorComponent>(entity)?;
        Some(animator.clips.iter().find(|s| s.name == name)?.path.clone())
    })();
    let Some(path) = path else { return };
    let Some(handle) = world.resource::<AssetServer>().get_handle::<AnimationClip>(&path) else { return };
    let _ = world.resource_mut::<Assets<AnimationClip>>().insert(handle.id(), clip);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keying_at_an_existing_time_replaces_it() {
        let mut keys = vec![(0.0, 1.0), (0.5, 2.0)];
        upsert(&mut keys, 0.5, 9.0);
        assert_eq!(keys, vec![(0.0, 1.0), (0.5, 9.0)]);
    }

    #[test]
    fn a_new_key_lands_in_time_order() {
        let mut keys = vec![(0.0, 1.0), (1.0, 2.0)];
        upsert(&mut keys, 0.25, 5.0);
        upsert(&mut keys, 2.0, 7.0);
        assert_eq!(keys, vec![(0.0, 1.0), (0.25, 5.0), (1.0, 2.0), (2.0, 7.0)]);
    }
}
