//! The loaded clip — which is also the edit buffer — plus the lane and selection
//! vocabulary every other module in this panel speaks.

use bevy::prelude::*;

use renzora::PropertyKey;
use renzora_animation::{AnimClip, AnimatorComponent};
use renzora_ember::reactive::Rx;

use crate::AnimationEditorState;

use super::preview::PreviewApplied;

/// Disk-loaded copy of the currently selected clip, reloaded when the
/// `(entity, clip)` selection changes. Drives the header + keyframe snapshots,
/// and doubles as the edit buffer: keyframe drags/deletes mutate `clip` in
/// place and set `dirty` until the Save button flushes it back to `path`.
#[derive(Resource, Default)]
pub(crate) struct TimelineClip {
    pub(super) key: Option<(Entity, String)>,
    pub(crate) clip: Option<AnimClip>,
    /// Absolute path of the loaded `.anim` (save target).
    pub(super) path: Option<std::path::PathBuf>,
    /// Unsaved keyframe edits pending.
    pub(super) dirty: bool,
}

impl TimelineClip {
    /// Mutable access to one lane's `(time, …)` key vector, erased over the
    /// channel value types via the times-only view the editor needs. A lane is
    /// either a skeletal bone channel (T/R/S) or a single property track.
    pub(super) fn lane_times(&mut self, lane: Lane) -> Option<ChannelTimes<'_>> {
        let clip = self.clip.as_mut()?;
        match lane {
            Lane::Bone { track, channel } => {
                let track = clip.tracks.get_mut(track)?;
                Some(match channel {
                    0 => ChannelTimes::T(&mut track.translations),
                    1 => ChannelTimes::R(&mut track.rotations),
                    _ => ChannelTimes::S(&mut track.scales),
                })
            }
            Lane::Prop { track } => {
                let track = clip.property_tracks.get_mut(track)?;
                Some(ChannelTimes::P(&mut track.keys))
            }
        }
    }
}

/// Identifies an editable lane: a skeletal bone channel or a property track.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Lane {
    Bone { track: usize, channel: u8 },
    Prop { track: usize },
}

/// The currently selected keyframe (lane + index), highlighted on the dopesheet
/// with its value shown in the toolbar. Cleared when the clip selection changes.
#[derive(Resource, Default)]
pub(crate) struct SelectedKey(pub(super) Option<SelKey>);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct SelKey {
    pub(super) lane: Lane,
    pub(super) index: usize,
}

/// Borrowed view over a single keyframe lane — lets the drag/delete systems
/// edit times without caring about the per-channel value payload type.
pub(super) enum ChannelTimes<'a> {
    T(&'a mut Vec<(f32, [f32; 3])>),
    R(&'a mut Vec<(f32, [f32; 4])>),
    S(&'a mut Vec<(f32, [f32; 3])>),
    P(&'a mut Vec<PropertyKey>),
}

impl ChannelTimes<'_> {
    pub(super) fn time(&self, idx: usize) -> Option<f32> {
        match self {
            ChannelTimes::T(v) | ChannelTimes::S(v) => v.get(idx).map(|k| k.0),
            ChannelTimes::R(v) => v.get(idx).map(|k| k.0),
            ChannelTimes::P(v) => v.get(idx).map(|k| k.time),
        }
    }
    pub(super) fn set_time(&mut self, idx: usize, t: f32) {
        match self {
            ChannelTimes::T(v) | ChannelTimes::S(v) => {
                if let Some(k) = v.get_mut(idx) {
                    k.0 = t;
                }
            }
            ChannelTimes::R(v) => {
                if let Some(k) = v.get_mut(idx) {
                    k.0 = t;
                }
            }
            ChannelTimes::P(v) => {
                if let Some(k) = v.get_mut(idx) {
                    k.time = t;
                }
            }
        }
    }
    pub(super) fn remove(&mut self, idx: usize) {
        match self {
            ChannelTimes::T(v) | ChannelTimes::S(v) => {
                if idx < v.len() {
                    v.remove(idx);
                }
            }
            ChannelTimes::R(v) => {
                if idx < v.len() {
                    v.remove(idx);
                }
            }
            ChannelTimes::P(v) => {
                if idx < v.len() {
                    v.remove(idx);
                }
            }
        }
    }
    pub(super) fn sort(&mut self) {
        match self {
            ChannelTimes::T(v) | ChannelTimes::S(v) => {
                v.sort_by(|a, b| a.0.total_cmp(&b.0));
            }
            ChannelTimes::R(v) => v.sort_by(|a, b| a.0.total_cmp(&b.0)),
            ChannelTimes::P(v) => v.sort_by(|a, b| a.time.total_cmp(&b.time)),
        }
    }
}

// ── Accessors ────────────────────────────────────────────────────────────────

pub(super) fn state(w: &World) -> Option<&AnimationEditorState> {
    w.get_resource::<AnimationEditorState>()
}

pub(super) fn cur_clip(w: &World) -> Option<&AnimClip> {
    w.get_resource::<TimelineClip>().and_then(|c| c.clip.as_ref())
}

/// The entity the currently-loaded clip belongs to (the cache's key entity).
/// Apply/capture must target THIS, not the bare `selected_entity` — on a
/// selection change the two differ for a frame, which would otherwise write the
/// old clip's pose onto the newly-selected entity.
pub(super) fn clip_entity(w: &World) -> Option<Entity> {
    w.get_resource::<TimelineClip>()
        .and_then(|c| c.key.as_ref().map(|(e, _)| *e))
}

/// Whether the timeline has a clip to show (vs an empty-state message).
pub(super) fn ready(w: &Rx) -> bool {
    cur_clip(w.untracked()).is_some()
}

pub(super) fn empty_msg(w: &Rx) -> String {
    let Some(s) = state(w.untracked()) else { return String::new() };
    if s.selected_entity.is_none() {
        renzora::lang::t("animation.select_entity_to_animate")
    } else if s
        .selected_entity
        .and_then(|e| w.get::<AnimatorComponent>(e))
        .is_none_or(|a| a.clips.is_empty())
    {
        renzora::lang::t("animation.no_animation_create_below")
    } else if s.selected_clip.is_none() {
        renzora::lang::t("animation.choose_clip_above")
    } else {
        renzora::lang::t("animation.loading_clip")
    }
}

// ── Load / save ──────────────────────────────────────────────────────────────

pub(super) fn cache_clip(
    mut cache: ResMut<TimelineClip>,
    state: Res<AnimationEditorState>,
    animators: Query<&AnimatorComponent>,
    project: Option<Res<renzora::core::CurrentProject>>,
    mut selected: ResMut<SelectedKey>,
    mut preview: ResMut<PreviewApplied>,
) {
    let key = match (state.selected_entity, state.selected_clip.as_deref()) {
        (Some(e), Some(c)) => Some((e, c.to_string())),
        _ => None,
    };
    if key == cache.key {
        return;
    }
    selected.0 = None;
    // The preview's stored baseline belongs to the old clip — invalidate it so
    // the manual-pose detector doesn't compare against stale data (which would
    // mis-fire and pause playback when switching/clicking away and back).
    preview.valid = false;
    // Auto-save pending edits before switching away, instead of discarding them
    // — clicking off an entity must not lose unsaved keyframes.
    if cache.dirty {
        if let (Some(clip), Some(path)) = (cache.clip.as_ref(), cache.path.as_ref()) {
            match renzora::core::write_anim_file(clip, path) {
                Ok(()) => info!("[timeline] auto-saved keyframe edits before switching"),
                Err(e) => warn!("[timeline] auto-save failed: {}", e),
            }
        }
    }
    cache.key = key.clone();
    cache.clip = None;
    cache.path = None;
    cache.dirty = false;
    let (Some((entity, clip_name)), Some(project)) = (key, project) else { return };
    let Ok(animator) = animators.get(entity) else { return };
    let Some(slot) = animator.clips.iter().find(|s| s.name == clip_name) else { return };
    let path = project.path.join(&slot.path);
    if let Ok(content) = std::fs::read_to_string(&path) {
        cache.clip = ron::from_str::<AnimClip>(&content).ok();
        cache.path = Some(path);
    }
}

/// Set the clip duration (toolbar Length field), syncing the editor's cached
/// duration so the ruler/playhead range + loop point update immediately.
pub(super) fn set_clip_duration(world: &mut World, dur: f32) {
    let dur = dur.clamp(0.2, 600.0);
    if let Some(mut cache) = world.get_resource_mut::<TimelineClip>() {
        let changed = match cache.clip.as_mut() {
            Some(clip) if (clip.duration - dur).abs() > 1e-5 => {
                clip.duration = dur;
                true
            }
            _ => false,
        };
        if changed {
            cache.dirty = true;
        }
    }
    if let Some(mut s) = world.get_resource_mut::<AnimationEditorState>() {
        s.clip_duration = Some(dur);
    }
}

/// Throttle for periodic auto-save.
#[derive(Resource, Default)]
pub(crate) struct AutoSaveTimer {
    last: f32,
}

/// Auto-save the edit buffer back to disk while dirty (at most ~1×/1.5s), so
/// edits are never lost and Play mode (which reads the `.anim` from disk) picks
/// them up without a manual save.
pub(super) fn auto_save_clip(
    time: Res<Time>,
    mut cache: ResMut<TimelineClip>,
    mut timer: ResMut<AutoSaveTimer>,
) {
    if !cache.dirty {
        return;
    }
    let now = time.elapsed_secs();
    if now - timer.last < 1.5 {
        return;
    }
    timer.last = now;
    let result = match (cache.clip.as_ref(), cache.path.as_ref()) {
        (Some(clip), Some(path)) => Some(renzora::core::write_anim_file(clip, path)),
        _ => None,
    };
    match result {
        Some(Ok(())) => {
            cache.dirty = false;
            info!("[timeline] auto-saved keyframe edits");
        }
        Some(Err(e)) => warn!("[timeline] auto-save failed: {}", e),
        None => {}
    }
}

/// Save button → flush the edit buffer back to the `.anim` file on disk.
pub(super) fn save_clip_click(
    q: Query<&Interaction, (With<super::SaveClipBtn>, Changed<Interaction>)>,
    mut cache: ResMut<TimelineClip>,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    if !cache.dirty {
        return;
    }
    let result = match (cache.clip.as_ref(), cache.path.as_ref()) {
        (Some(clip), Some(path)) => renzora::core::write_anim_file(clip, path),
        _ => return,
    };
    match result {
        Ok(()) => {
            cache.dirty = false;
            info!("[timeline] saved keyframe edits");
        }
        Err(e) => warn!("[timeline] save failed: {}", e),
    }
}
