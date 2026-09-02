//! Undo for clip edits.
//!
//! Keyframe edits mutate the in-memory clip buffer ([`TimelineClip::clip`]). A
//! change-observer records a coarse snapshot of that buffer whenever it changes,
//! covering drags, deletes, interp changes, live-record capture and marker edits
//! from one place. Full RON serialization is the change signal so no field is
//! missed. The clip is scene-attached content, so edits land on the active
//! (Scene) stack; per-frame drag spam collapses via the merge key, and the
//! global gesture seal splits gestures.

use bevy::prelude::*;

use renzora_animation::AnimClip;

use super::clip::TimelineClip;

/// Shadow of the clip the observer last saw, its serialized form (the diff key),
/// and the `(entity, clip)` selection it belongs to. Changing selection reseeds.
#[derive(Resource, Default)]
pub(crate) struct AnimUndoShadow {
    key: Option<(Entity, String)>,
    serialized: Option<String>,
    clip: Option<AnimClip>,
}

/// Restore a snapshotted clip — the `restore` fn for the animation `SnapshotCmd`.
/// Writes the buffer and marks it dirty; the timeline + dopesheet rebuild
/// reactively from the buffer, and Save flushes it to disk.
fn restore_anim_clip(world: &mut World, clip: &AnimClip) {
    if let Some(mut c) = world.get_resource_mut::<TimelineClip>() {
        c.clip = Some(clip.clone());
        c.dirty = true;
    }
    if let Some(mut sh) = world.get_resource_mut::<AnimUndoShadow>() {
        sh.serialized = ron::to_string(clip).ok();
        sh.clip = Some(clip.clone());
    }
}

pub(super) fn anim_undo_observer(world: &mut World) {
    let (cur, key) = {
        let Some(c) = world.get_resource::<TimelineClip>() else {
            return;
        };
        let Some(clip) = c.clip.clone() else {
            return;
        };
        (clip, c.key.clone())
    };
    let serialized = match ron::to_string(&cur) {
        Ok(s) => s,
        Err(_) => return,
    };
    let (prev_key, prev_serialized, prev_clip) = {
        let sh = world.resource::<AnimUndoShadow>();
        (sh.key.clone(), sh.serialized.clone(), sh.clip.clone())
    };
    if prev_key != key || prev_clip.is_none() {
        let mut sh = world.resource_mut::<AnimUndoShadow>();
        sh.key = key;
        sh.serialized = Some(serialized);
        sh.clip = Some(cur);
        return;
    }
    if prev_serialized.as_deref() == Some(serialized.as_str()) {
        return;
    }
    let before = prev_clip.unwrap();
    let ctx = renzora_undo::active_context(world);
    renzora_undo::record(
        world,
        ctx,
        Box::new(renzora_undo::SnapshotCmd {
            label: "Animation".to_string(),
            before,
            after: cur.clone(),
            restore: restore_anim_clip,
            merge_key: Some("anim-clip".to_string()),
        }),
    );
    let mut sh = world.resource_mut::<AnimUndoShadow>();
    sh.serialized = Some(serialized);
    sh.clip = Some(cur);
}
