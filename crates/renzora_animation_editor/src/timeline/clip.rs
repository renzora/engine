//! The loaded clip — which is also the edit buffer — plus the lane and selection
//! vocabulary every other module in this panel speaks.

use bevy::prelude::*;

use renzora::PropertyKey;
use renzora_animation::{AnimClip, AnimatorComponent};
use renzora_particles::params::ParamKey;
use renzora_ember::reactive::Rx;
use renzora_ember::UiInteraction;

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
    /// The particle effect shown under the clip's tracks (the Particles
    /// group), and its edit buffer. Lives here rather than in a resource of
    /// its own so every edit path that already takes the clip buffer (drag,
    /// delete, box-select) reaches the effect's keys through [`Lane::Fx`]
    /// without a second parameter. See [`super::fx`].
    pub(crate) fx: Option<super::fx::FxDoc>,
}

impl TimelineClip {
    /// Mutable access to one lane's `(time, …)` key vector, erased over the
    /// channel value types via the times-only view the editor needs. A lane is
    /// a skeletal bone channel (T/R/S), a single property track, or one of the
    /// effect's keyed parameters.
    pub(super) fn lane_times(&mut self, lane: Lane) -> Option<ChannelTimes<'_>> {
        if let Lane::Fx { track } = lane {
            let fx = self.fx.as_mut().filter(|f| f.editable)?;
            return Some(ChannelTimes::F(&mut fx.doc.tracks.get_mut(track)?.keys));
        }
        let clip = self.clip.as_mut()?;
        match lane {
            Lane::Bone { track, channel } => Some(ChannelTimes::B(clip.tracks.get_mut(track)?, channel)),
            Lane::Prop { track } => {
                let track = clip.property_tracks.get_mut(track)?;
                Some(ChannelTimes::P(&mut track.keys))
            }
            Lane::Morph { track } => Some(ChannelTimes::M(clip.morph_tracks.get_mut(track)?)),
            Lane::Fx { .. } => None,
        }
    }

    /// Times of one lane, in key order: [`super::snapshots::lane_key_times`]
    /// for the clip's own lanes, the effect's keys for a [`Lane::Fx`].
    pub(super) fn key_times(&self, lane: Lane) -> Vec<f32> {
        match lane {
            Lane::Fx { track } => self
                .fx
                .as_ref()
                .and_then(|f| f.doc.tracks.get(track))
                .map(|t| t.keys.iter().map(|k| k.time).collect())
                .unwrap_or_default(),
            _ => self.clip.as_ref().map(|c| super::snapshots::lane_key_times(c, lane)).unwrap_or_default(),
        }
    }

    /// The clip is a stand-in for an effect shown on its own (an entity with
    /// a particle system and no animation clip), not a `.anim` on disk.
    pub(crate) fn is_effect_only(&self) -> bool {
        self.key.as_ref().is_some_and(|(_, name)| name == super::fx::FX_CLIP)
    }

    /// The stand-in clip's length, which is the effect's.
    pub(crate) fn effect_only_duration(&self) -> Option<f32> {
        self.is_effect_only().then(|| self.clip.as_ref().map(|c| c.duration)).flatten()
    }
}

/// Identifies an editable lane: a skeletal bone channel, a property track, a
/// blend shape's weight (an index into `AnimClip::morph_tracks`), or one of the
/// effect's keyed parameters (an index into `VfxAsset::tracks`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Lane {
    Bone { track: usize, channel: u8 },
    Prop { track: usize },
    Morph { track: usize },
    Fx { track: usize },
}

/// The *primary* selected keyframe (lane + index): the one the key popover
/// edits and live-edit writes into. Cleared when the clip selection changes.
#[derive(Resource, Default)]
pub(crate) struct SelectedKey(pub(super) Option<SelKey>);

/// Every selected keyframe, the primary included: what a box-select picked, and
/// what a drag, Delete or Copy acts on. `gen` bumps on every change so the
/// draw tokens notice without hashing the list.
#[derive(Resource, Default)]
pub(crate) struct SelectedKeys {
    pub(super) keys: Vec<SelKey>,
    pub(super) gen: u64,
}

impl SelectedKeys {
    pub(super) fn set(&mut self, keys: Vec<SelKey>) {
        self.keys = keys;
        self.gen = self.gen.wrapping_add(1);
    }
    pub(super) fn clear(&mut self) {
        if !self.keys.is_empty() {
            self.set(Vec::new());
        }
    }
    pub(super) fn contains(&self, k: &SelKey) -> bool {
        self.keys.contains(k)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct SelKey {
    pub(super) lane: Lane,
    pub(super) index: usize,
}

/// Borrowed view over a single keyframe lane — lets the drag/delete systems
/// edit times without caring about the per-channel value payload type.
///
/// A bone lane borrows its whole track, not just the key vector, because each
/// bone key has a curve beside it ([`renzora::BoneKeyCurves`]) that has to
/// move, sort and delete with it.
pub(super) enum ChannelTimes<'a> {
    B(&'a mut renzora::BoneTrack, u8),
    P(&'a mut Vec<PropertyKey>),
    /// A blend shape track, whole for the same reason as a bone's: its keys'
    /// curves sit beside them.
    M(&'a mut renzora::MorphTrack),
    F(&'a mut Vec<ParamKey>),
}

impl ChannelTimes<'_> {
    pub(super) fn time(&self, idx: usize) -> Option<f32> {
        match self {
            ChannelTimes::B(t, ch) => t.key_time(*ch, idx),
            ChannelTimes::P(v) => v.get(idx).map(|k| k.time),
            ChannelTimes::M(t) => t.key_time(idx),
            ChannelTimes::F(v) => v.get(idx).map(|k| k.time),
        }
    }
    pub(super) fn set_time(&mut self, idx: usize, t: f32) {
        match self {
            ChannelTimes::B(track, ch) => track.set_key_time(*ch, idx, t),
            ChannelTimes::M(track) => track.set_key_time(idx, t),
            ChannelTimes::P(v) => {
                if let Some(k) = v.get_mut(idx) {
                    k.time = t;
                }
            }
            ChannelTimes::F(v) => {
                if let Some(k) = v.get_mut(idx) {
                    k.time = t;
                }
            }
        }
    }
    pub(super) fn remove(&mut self, idx: usize) {
        match self {
            ChannelTimes::B(track, ch) => track.remove_key(*ch, idx),
            ChannelTimes::M(track) => track.remove_key(idx),
            ChannelTimes::P(v) => {
                if idx < v.len() {
                    v.remove(idx);
                }
            }
            ChannelTimes::F(v) => {
                if idx < v.len() {
                    v.remove(idx);
                }
            }
        }
    }
    pub(super) fn sort(&mut self) {
        match self {
            ChannelTimes::B(track, ch) => track.sort_channel(*ch),
            ChannelTimes::M(track) => track.sort(),
            ChannelTimes::P(v) => v.sort_by(|a, b| a.time.total_cmp(&b.time)),
            ChannelTimes::F(v) => v.sort_by(|a, b| a.time.total_cmp(&b.time)),
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
    mut state: ResMut<AnimationEditorState>,
    animators: Query<&AnimatorComponent>,
    project: Option<Res<renzora::core::CurrentProject>>,
    mut selected: ResMut<SelectedKey>,
    mut all_selected: ResMut<SelectedKeys>,
    mut preview: ResMut<PreviewApplied>,
    target: Res<super::fx::FxTarget>,
) {
    // An effect with no clip to sit under still gets the timeline, on a
    // stand-in clip keyed `FX_CLIP`: on a `.vfx` document tab (the effect being
    // edited), or when the selection has a particle system and no clip.
    let effect_only = || target.entity.map(|e| (e, super::fx::FX_CLIP.to_string()));
    let key = if target.doc_mode {
        effect_only()
    } else {
        match (state.selected_entity, state.selected_clip.as_deref()) {
            (Some(e), Some(c)) => Some((e, c.to_string())),
            _ => effect_only(),
        }
    };
    if key == cache.key {
        return;
    }
    // The playhead's range came from the clip being left; the next one sets
    // its own (`cache_clip_duration`, or the effect for a stand-in).
    state.clip_duration = None;
    // The loop region belongs to the clip it was set on. Reset here as well as
    // in the selection paths, since a clip can also arrive by being created or
    // renamed, and a region past the new clip's end would loop nothing.
    if state.loop_start != 0.0 || state.loop_end.is_some() {
        state.reset_loop();
    }
    selected.0 = None;
    all_selected.clear();
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
    if cache.is_effect_only() {
        // No `.anim` behind it and no path, so nothing ever saves it: it only
        // carries the effect's length for the ruler (`fx::sync_fx` keeps the
        // two in step) and gives the panel its rows.
        cache.clip = Some(renzora_animation::extract::create_empty_clip("", 2.0));
        return;
    }
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
pub(crate) fn set_clip_duration(world: &mut World, dur: f32) {
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
    q: Query<&UiInteraction, (With<super::SaveClipBtn>, Changed<UiInteraction>)>,
    mut cache: ResMut<TimelineClip>,
) {
    if !q.iter().any(|i| *i == UiInteraction::Pressed) {
        return;
    }
    // Save covers the whole panel: the effect's tracks too (written by
    // `fx::sync_fx` through the effect editor's save).
    if let Some(fx) = cache.fx.as_mut() {
        fx.save_requested = true;
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
