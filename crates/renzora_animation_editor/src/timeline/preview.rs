//! Scrub preview, live-edit and record capture — the three systems that move
//! values between the clip and the live entity.
//!
//! The subtle one is **manual-pose detection**. The preview writes sampled
//! values onto the entity every frame the playhead moves; if it did that
//! unconditionally, posing the object with a gizmo would be overwritten before
//! it could be keyed, and every captured key would come out identical. So the
//! preview records what it last wrote, notices when the entity has diverged from
//! that, and stands down — adopting the user's pose as the new baseline.

use bevy::prelude::*;

use renzora::{PropertyTrack, TrackValue};
use renzora_animation::property_playback::{apply_property_tracks, read_track_value};

use crate::AnimationEditorState;

use super::clip::{clip_entity, Lane, SelKey, SelectedKey, TimelineClip};
use super::edit::KeyDragState;
use super::props::upsert_key;

/// Tracks the last (entity, time) the scrub preview wrote, so the preview only
/// drives the object when playing or when the playhead actually moved — leaving
/// a stationary playhead free to pose the object for new keyframes.
#[derive(Resource, Default)]
pub(crate) struct PreviewApplied {
    time: f32,
    entity: Option<Entity>,
    pub(super) valid: bool,
    /// Last time (seconds) diagnostics were logged, to throttle them.
    last_log: f32,
    /// What the preview last wrote per track (aligned with the clip's property
    /// tracks). Used to detect a manual pose (current values diverging from this)
    /// so the preview stops fighting the user's gizmo/inspector edits.
    written: Vec<Option<TrackValue>>,
}

/// Record session baselines: the last live value seen per property track,
/// used to detect a user edit (vs. the sampler's own writes) while recording.
#[derive(Resource, Default)]
pub(crate) struct RecordState {
    session: Option<(Entity, Option<String>)>,
    baselines: std::collections::HashMap<usize, TrackValue>,
}

fn track_value_close(a: &TrackValue, b: &TrackValue) -> bool {
    const EPS: f32 = 1e-4;
    let close = |x: &[f32], y: &[f32]| x.iter().zip(y).all(|(a, b)| (a - b).abs() < EPS);
    match (a, b) {
        (TrackValue::Float(x), TrackValue::Float(y)) => (x - y).abs() < EPS,
        (TrackValue::Vec3(x), TrackValue::Vec3(y)) => close(x, y),
        (TrackValue::Color(x), TrackValue::Color(y)) => close(x, y),
        (TrackValue::Quat(x), TrackValue::Quat(y)) => close(x, y),
        (TrackValue::Bool(x), TrackValue::Bool(y)) => x == y,
        _ => false,
    }
}

/// Scrub preview: sample the in-memory property tracks at the playhead and write
/// the values onto the selected entity so scrubbing (and Play) animates it live.
/// The playhead itself is advanced by `advance_preview_time` (lib.rs). Suppressed
/// while recording so the user's edits aren't overwritten.
pub(super) fn preview_property_animation(world: &mut World) {
    let (scrub, record, previewing) = {
        let Some(s) = world.get_resource::<AnimationEditorState>() else { return };
        (s.scrub_time, s.record_enabled, s.is_previewing)
    };
    let Some(entity) = clip_entity(world) else { return };
    if record {
        return;
    }
    // Only drive the entity while playing or when the playhead actually moved.
    // A stationary playhead leaves the object free to be posed for new keys —
    // otherwise the preview would overwrite the user's edit every frame and
    // every captured key would be identical.
    let moved = world.get_resource::<PreviewApplied>().is_none_or(|p| {
        !p.valid || p.entity != Some(entity) || (p.time - scrub).abs() > 1e-6
    });
    if !previewing && !moved {
        return;
    }
    let tracks: Vec<PropertyTrack> =
        match world.get_resource::<TimelineClip>().and_then(|c| c.clip.as_ref()) {
            Some(clip) if !clip.property_tracks.is_empty() => clip.property_tracks.clone(),
            _ => return,
        };

    // Manual-pose detection: if the entity's current values diverge from what
    // the preview last wrote, the user posed it (gizmo or inspector). Pause
    // playback, adopt the pose as the new baseline and DON'T overwrite it — so
    // the edit sticks and can be keyed (this is why captures were grabbing the
    // preview's value instead of the user's rotation).
    let (written, pa_entity, pa_valid) = world
        .get_resource::<PreviewApplied>()
        .map(|p| (p.written.clone(), p.entity, p.valid))
        .unwrap_or_default();
    if pa_valid && pa_entity == Some(entity) && written.len() == tracks.len() {
        let current: Vec<Option<TrackValue>> =
            tracks.iter().map(|t| read_track_value(world, entity, t)).collect();
        let manual = current.iter().zip(&written).any(|(c, w)| match (c, w) {
            (Some(cv), Some(wv)) => !track_value_close(cv, wv),
            _ => false,
        });
        if manual {
            for (i, (c, w)) in current.iter().zip(&written).enumerate() {
                if let (Some(cv), Some(wv)) = (c, w) {
                    if !track_value_close(cv, wv) {
                        info!(
                            "[prop-anim] pose changed on track {} -> {:?} (not yet keyed — Add Key / right-click 'Set to current pose', or select a key first to live-edit it)",
                            i, cv
                        );
                    }
                }
            }
            if previewing {
                if let Some(mut s) = world.get_resource_mut::<AnimationEditorState>() {
                    s.is_previewing = false;
                }
            }
            if let Some(mut pa) = world.get_resource_mut::<PreviewApplied>() {
                pa.time = scrub;
                pa.entity = Some(entity);
                pa.valid = true;
                pa.written = current;
            }
            return;
        }
    }

    let moved = world.get_resource::<PreviewApplied>().is_none_or(|p| {
        !p.valid || p.entity != Some(entity) || (p.time - scrub).abs() > 1e-6
    });
    if !previewing && !moved {
        return;
    }

    // Throttle diagnostics to ~2×/sec, and only while actually playing.
    let now = world.resource::<Time>().elapsed_secs();
    let mut verbose = false;
    if previewing {
        if let Some(pa) = world.get_resource::<PreviewApplied>() {
            if now - pa.last_log > 0.5 {
                verbose = true;
            }
        }
    }
    apply_property_tracks(world, entity, &tracks, scrub, verbose);
    // Record what actually LANDED on the entity (read-back), not the raw
    // sampled values: fields that quantize on write — `SpriteSheet.frame` is a
    // u32, so a sampled 2.37 lands as 2 — would otherwise diverge from the
    // next frame's read in the manual-pose check above, which reads as "the
    // user posed it" and pauses playback on the very first Play frame.
    let written_now: Vec<Option<TrackValue>> =
        tracks.iter().map(|t| read_track_value(world, entity, t)).collect();
    if let Some(mut pa) = world.get_resource_mut::<PreviewApplied>() {
        pa.time = scrub;
        pa.entity = Some(entity);
        pa.valid = true;
        pa.written = written_now;
        if verbose {
            pa.last_log = now;
        }
    }
}

/// Live-edit: when a property keyframe is selected and the playhead sits on it,
/// posing the entity updates that keyframe's value. This makes the intuitive
/// "click a key, then move the object to set it" workflow work.
pub(super) fn live_edit_selected_key(world: &mut World) {
    let (scrub, record, previewing, sel, dragging) = {
        let Some(s) = world.get_resource::<AnimationEditorState>() else { return };
        let sel = world.get_resource::<SelectedKey>().and_then(|x| x.0);
        let dragging = world.get_resource::<KeyDragState>().is_some_and(|d| d.active.is_some());
        (s.scrub_time, s.record_enabled, s.is_previewing, sel, dragging)
    };
    let Some(entity) = clip_entity(world) else { return };
    if record || previewing || dragging {
        return;
    }
    let Some(SelKey { lane: Lane::Prop { track }, index }) = sel else { return };
    let track_data = world
        .get_resource::<TimelineClip>()
        .and_then(|c| c.clip.as_ref())
        .and_then(|c| c.property_tracks.get(track))
        .cloned();
    let Some(track_data) = track_data else { return };
    let Some(stored) = track_data.keys.get(index).map(|k| k.value) else { return };
    let key_time = track_data.keys[index].time;
    // Only when the playhead is on the key (so the preview is showing it); a
    // little slack covers snap/float drift after the select-jump.
    if (key_time - scrub).abs() > 0.05 {
        return;
    }
    let Some(live) = read_track_value(world, entity, &track_data) else { return };
    if track_value_close(&stored, &live) {
        return;
    }
    info!(
        "[prop-anim] live-edit selected key {} of {}.{}: {:?} -> {:?}",
        index, track_data.component, track_data.field, stored, live
    );
    if let Some(mut cache) = world.get_resource_mut::<TimelineClip>() {
        if let Some(key) = cache
            .clip
            .as_mut()
            .and_then(|c| c.property_tracks.get_mut(track))
            .and_then(|pt| pt.keys.get_mut(index))
        {
            key.value = live;
        }
        cache.dirty = true;
    }
}

/// Record capture: while armed, detect user edits to tracked fields (live value
/// diverging from the per-track baseline) and auto-key them at the playhead.
pub(super) fn record_capture(world: &mut World) {
    let Some(state) = world.get_resource::<AnimationEditorState>() else { return };
    if !state.record_enabled {
        return;
    }
    let scrub = state.scrub_time;
    let clip_name = state.selected_clip.clone();
    let Some(entity) = clip_entity(world) else { return };
    let tracks: Vec<PropertyTrack> =
        match world.get_resource::<TimelineClip>().and_then(|c| c.clip.as_ref()) {
            Some(clip) if !clip.property_tracks.is_empty() => clip.property_tracks.clone(),
            _ => return,
        };

    // Reset baselines when the record target changes.
    {
        let mut rec = world.get_resource_or_insert_with(RecordState::default);
        if rec.session.as_ref() != Some(&(entity, clip_name.clone())) {
            rec.session = Some((entity, clip_name.clone()));
            rec.baselines.clear();
        }
    }

    let mut captures: Vec<(usize, TrackValue)> = Vec::new();
    for (pi, track) in tracks.iter().enumerate() {
        let Some(live) = read_track_value(world, entity, track) else { continue };
        let mut rec = world.get_resource_mut::<RecordState>().unwrap();
        match rec.baselines.get(&pi).copied() {
            None => {
                rec.baselines.insert(pi, live);
            }
            Some(base) => {
                if !track_value_close(&base, &live) {
                    rec.baselines.insert(pi, live);
                    captures.push((pi, live));
                }
            }
        }
    }
    if captures.is_empty() {
        return;
    }
    if let Some(mut cache) = world.get_resource_mut::<TimelineClip>() {
        if let Some(clip) = cache.clip.as_mut() {
            for (pi, val) in captures {
                if let Some(pt) = clip.property_tracks.get_mut(pi) {
                    info!("[prop-anim] Record: keyed track {} @ t={:.3} -> {:?}", pi, scrub, val);
                    upsert_key(pt, scrub, val);
                }
            }
            cache.dirty = true;
        }
    }
}
