//! Blend shapes (morph targets) set by hand, and keyed into the clip.
//!
//! The same rule as bones ([`super::pose`]): in the editor the clip's player
//! is seeked to the playhead every frame and rewrites every weight it has a
//! curve for, so a blend shape slider dragged in the Face panel would snap
//! back the next frame. [`MorphLayer`] holds the hand-set weights on top of
//! the clip, [`apply_morph_pose`] puts them back after the player has written,
//! and they last until the playhead (or the clip) moves, exactly like an
//! unkeyed bone pose.
//!
//! Keying ([`key_morphs`]) writes weights into the clip's morph tracks, one
//! track per target, making the track if the target had none, and rebuilds
//! the playing clip so the key plays on the next frame. K, Key Face and the
//! Face panel's Key Blend Shapes all key the hand-set weights at the playhead
//! ([`key_posed_morphs`]); with record armed a weight is keyed as it is set.

use std::collections::HashMap;

use bevy::animation::AnimationTargetId;
use bevy::mesh::morph::MorphWeights;
use bevy::prelude::*;

use renzora::{KeyCurve, MorphTrack};

use crate::AnimationEditorState;

use super::clip::TimelineClip;

/// Two keys closer than this (seconds) are the same key. See `pose::SAME_KEY`.
const SAME_KEY: f32 = 1e-3;

/// The hand-set blend shape weights sitting on top of the clip.
#[derive(Resource, Default)]
pub(crate) struct MorphLayer {
    /// `(entity carrying the weights, target index) -> weight`.
    pub(crate) posed: HashMap<(Entity, usize), f32>,
    /// The playhead and clip the weights were set at; moving either drops
    /// them.
    at: Option<(u32, Option<(Entity, String)>)>,
}

/// Where the editor is, frame-quantised like `pose::capture_pose` so float
/// noise in the playhead does not read as a move.
fn playhead(world: &World) -> Option<(u32, Option<(Entity, String)>)> {
    let time = world.get_resource::<AnimationEditorState>()?.scrub_time;
    let key = world.get_resource::<TimelineClip>().and_then(|c| c.key.clone());
    Some(((time * 1000.0).round() as u32, key))
}

/// Set one target's weight by hand: on the mesh now, and held over the clip
/// until the playhead moves. With record armed it is keyed straight away.
pub(crate) fn set_morph(world: &mut World, entity: Entity, index: usize, weight: f32) {
    if let Some(mut m) = world.get_mut::<MorphWeights>(entity) {
        if let Some(w) = m.weights_mut().get_mut(index) {
            *w = weight;
        }
    }
    let at = playhead(world);
    if let Some(mut layer) = world.get_resource_mut::<MorphLayer>() {
        if layer.at != at {
            layer.posed.clear();
            layer.at = at;
        }
        layer.posed.insert((entity, index), weight);
    }
    let record = world.get_resource::<AnimationEditorState>().is_some_and(|s| s.record_enabled);
    if record {
        let time = world.get_resource::<AnimationEditorState>().map(|s| s.scrub_time).unwrap_or(0.0);
        key_morphs(world, &[(entity, index, time, weight)]);
    }
}

/// Drop the hand-set weights when the playhead or clip moved or playback runs,
/// else put them back over what the player just wrote. PostUpdate, after
/// the animation systems and before anything renders the weights.
pub(crate) fn apply_morph_pose(world: &mut World) {
    let previewing = world.get_resource::<AnimationEditorState>().is_some_and(|s| s.is_previewing);
    let at = playhead(world);
    let posed: Vec<((Entity, usize), f32)> = {
        let Some(mut layer) = world.get_resource_mut::<MorphLayer>() else { return };
        if layer.posed.is_empty() {
            return;
        }
        if previewing || layer.at != at {
            layer.posed.clear();
            layer.at = at;
            return;
        }
        layer.posed.iter().map(|(k, v)| (*k, *v)).collect()
    };
    for ((entity, index), weight) in posed {
        let Some(mut m) = world.get_mut::<MorphWeights>(entity) else { continue };
        // Only written when the player moved it, so an untouched weight does
        // not trip change detection every frame.
        if m.weights().get(index).is_some_and(|w| *w != weight) {
            if let Some(w) = m.weights_mut().get_mut(index) {
                *w = weight;
            }
        }
    }
}

/// The name a mesh gives target `index`, empty when it names none.
pub(crate) fn target_name(world: &World, entity: Entity, index: usize) -> String {
    let meshes = world.get_resource::<Assets<Mesh>>();
    world
        .get::<MorphWeights>(entity)
        .and_then(|w| w.first_mesh())
        .and_then(|h| meshes?.get(h))
        .and_then(|m| m.morph_target_names().and_then(|n| n.get(index).cloned()))
        .unwrap_or_default()
}

/// Key every hand-set weight at the playhead. Returns whether anything was
/// keyed.
pub(crate) fn key_posed_morphs(world: &mut World) -> bool {
    let Some(time) = world.get_resource::<AnimationEditorState>().map(|s| s.scrub_time) else { return false };
    let mut keys: Vec<(Entity, usize, f32, f32)> = world
        .get_resource::<MorphLayer>()
        .map(|l| l.posed.iter().map(|((e, i), w)| (*e, *i, time, *w)).collect())
        .unwrap_or_default();
    if keys.is_empty() {
        return false;
    }
    // Stable order, so new tracks are made in target order.
    keys.sort_by_key(|(e, i, ..)| (*e, *i));
    key_morphs(world, &keys)
}

/// Key `(entity, target, time, weight)` into the open clip's morph tracks,
/// then rebuild the playing clip from the edit buffer.
///
/// A track is found by the key playback binds it by (the node's
/// [`AnimationTargetId`]) plus the target index, never by the node's
/// spelling: an imported clip says `Face:Mesh` while the entity is named
/// `face_mesh`, as with bone tracks (`pose::key_transforms`).
pub(crate) fn key_morphs(world: &mut World, keys: &[(Entity, usize, f32, f32)]) -> bool {
    let resolved: Vec<(AnimationTargetId, String, usize, String, f32, f32)> = keys
        .iter()
        .filter_map(|&(e, index, time, weight)| {
            let name = renzora::BindName::of(world.get::<Name>(e)?, world.get::<renzora::BindName>(e)).to_string();
            let target = world.get::<AnimationTargetId>(e).copied().unwrap_or_else(|| renzora_animation::bone_target(&name));
            Some((target, name, index, target_name(world, e, index), time, weight))
        })
        .collect();
    if resolved.is_empty() {
        return false;
    }
    {
        let Some(mut cache) = world.get_resource_mut::<TimelineClip>() else { return false };
        if cache.is_effect_only() {
            return false;
        }
        let Some(clip) = cache.clip.as_mut() else { return false };
        for (target, node, index, label, time, weight) in resolved {
            let at = clip
                .morph_tracks
                .iter()
                .position(|t| t.index == index && renzora_animation::bone_target(&t.target) == target);
            let track = match at {
                Some(i) => &mut clip.morph_tracks[i],
                None => {
                    clip.morph_tracks.push(MorphTrack::new(node, index, label));
                    clip.morph_tracks.last_mut().expect("just pushed")
                }
            };
            // Auto, like a keyed bone: shapes ease into one another.
            track.upsert_key(time, weight, KeyCurve::AUTO, SAME_KEY);
        }
        cache.dirty = true;
    }
    super::pose::rebuild_loaded_clip(world);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world_with_clip() -> (World, Entity) {
        let mut world = World::new();
        world.init_resource::<MorphLayer>();
        world.insert_resource(AnimationEditorState { scrub_time: 0.5, ..Default::default() });
        let face = world.spawn((Name::new("Face"), MorphWeights::new(vec![0.0, 0.0, 0.0], None).unwrap())).id();
        let mut cache = TimelineClip::default();
        cache.clip = Some(renzora_animation::extract::create_empty_clip("talk", 1.0));
        world.insert_resource(cache);
        (world, face)
    }

    #[test]
    fn a_hand_set_weight_keys_into_a_track_per_target() {
        let (mut world, face) = world_with_clip();
        set_morph(&mut world, face, 2, 0.75);
        assert_eq!(world.get::<MorphWeights>(face).unwrap().weights()[2], 0.75);
        assert!(key_posed_morphs(&mut world));
        let clip = world.resource::<TimelineClip>().clip.clone().unwrap();
        assert_eq!(clip.morph_tracks.len(), 1);
        let t = &clip.morph_tracks[0];
        assert_eq!((t.target.as_str(), t.index), ("Face", 2));
        assert_eq!(t.keys, vec![(0.5, 0.75)]);
        assert!(world.resource::<TimelineClip>().dirty);

        // Keying the same target again later adds a key to the same track,
        // and the node's other spelling still finds it.
        world.resource_mut::<AnimationEditorState>().scrub_time = 1.0;
        world.entity_mut(face).insert(Name::new("face"));
        set_morph(&mut world, face, 2, 0.25);
        key_posed_morphs(&mut world);
        let clip = world.resource::<TimelineClip>().clip.clone().unwrap();
        assert_eq!(clip.morph_tracks.len(), 1);
        assert_eq!(clip.morph_tracks[0].keys, vec![(0.5, 0.75), (1.0, 0.25)]);
    }

    /// A hand-set weight holds over the clip until the playhead moves.
    #[test]
    fn a_hand_set_weight_lasts_until_the_playhead_moves() {
        let (mut world, face) = world_with_clip();
        set_morph(&mut world, face, 0, 0.6);
        // The player writes the clip's weight over it...
        world.get_mut::<MorphWeights>(face).unwrap().weights_mut()[0] = 0.0;
        apply_morph_pose(&mut world);
        // ...and the hand-set one is put back.
        assert_eq!(world.get::<MorphWeights>(face).unwrap().weights()[0], 0.6);
        world.resource_mut::<AnimationEditorState>().scrub_time = 0.9;
        world.get_mut::<MorphWeights>(face).unwrap().weights_mut()[0] = 0.1;
        apply_morph_pose(&mut world);
        assert_eq!(world.get::<MorphWeights>(face).unwrap().weights()[0], 0.1);
        assert!(world.resource::<MorphLayer>().posed.is_empty());
        assert!(!key_posed_morphs(&mut world));
    }
}
