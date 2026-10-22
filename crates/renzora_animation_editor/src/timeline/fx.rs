//! The Particles group: a particle effect's keyed parameters as timeline rows,
//! under the animation tracks, on the same clock.
//!
//! **Which effect.** [`resolve_target`] picks it once a frame: on a `.vfx`
//! document tab, the effect being edited (through the effect editor's preview
//! entity); otherwise a `ParticleSystem` on the selected entity, on the entity
//! the timeline resolved the selection to, or under it. With no animation clip
//! to show, the timeline still opens on a stand-in clip
//! ([`FX_CLIP`], see `clip::cache_clip`) whose length is the effect's.
//!
//! **The buffer.** [`FxDoc`] holds a copy of the effect in the clip buffer
//! ([`TimelineClip::fx`]), and every edit path the timeline already has (key
//! drag, box-select, delete) reaches it through [`Lane::Fx`]. [`sync_fx`]
//! reconciles the copy once a frame, in one direction at a time: a copy the
//! timeline changed is written into the effect editor's document
//! ([`renzora_particles_editor::timeline::edit_effect`]), which records the
//! undo step, pushes the asset live and marks the file dirty; a source that
//! changed under an untouched copy (an undo, an edit in the graph, a reload)
//! is adopted. Only the tracks and the length are written back, the two
//! things the timeline edits, so a graph edit landing on the same frame is
//! never overwritten by a stale copy of the rest.
//!
//! **The clock.** [`drive_fx_clock`] makes the playhead the effect's time once
//! the timeline is used: scrubbing seeks it (`simulate_to`, deterministic, so a
//! scrubbed frame is exactly the frame play reaches), and Play runs it beside
//! the animation preview, re-seeking whenever the two drift (a loop wrap, a
//! jump). Until then a selected effect keeps playing as it was, and it gets its
//! own play state back when the timeline moves on.

use std::hash::Hasher;

use bevy::prelude::*;

use renzora_editor_framework::{EditorContext, EditorSelection};
use renzora_ember::font::EmberFonts;
use renzora_ember::widgets::{menu_header, menu_item, menu_sep, menu_submenu, screen_menu};
use renzora_ember::UiInteraction;
use renzora_particles::asset::{PinValue, VfxAsset};
use renzora_particles::params::{self, KeyInterp, ParamTarget, ParamTrack};
use renzora_particles::{ParticlePlayback, ParticleRuntime, ParticleSystem};
use renzora_particles_editor::state::active_vfx_path;
use renzora_particles_editor::timeline::{self as link, PreviewEffect, TimelineLink};

use crate::AnimationEditorState;

use super::clip::{Lane, SelectedKey, SelectedKeys, TimelineClip};
use super::layout::{RowKind, TrackLayout};
use super::props::TimelineOps;

/// The clip name of the stand-in clip an effect gets when there is no
/// animation clip to show it under. A control character, so no `.anim` slot
/// (whose names come from a text field) can ever be called this.
pub(crate) const FX_CLIP: &str = "\u{1}effect";

/// How long a timeline edit waits before it is saved, when the effect is not
/// open on a document tab where the user saves it themselves. The clip
/// autosave's interval, so both halves of the panel behave alike.
const AUTOSAVE_AFTER: f64 = 1.5;

/// The entity whose effect the timeline shows.
#[derive(Resource, Default)]
pub(crate) struct FxTarget {
    pub(crate) entity: Option<Entity>,
    /// A `.vfx` is the active document: the target is its preview, and the
    /// timeline shows that effect alone, whatever the scene selection is.
    pub(crate) doc_mode: bool,
}

/// The effect the timeline shows, and its edit buffer. See the module docs.
pub(crate) struct FxDoc {
    pub(crate) entity: Entity,
    /// Project-relative `.vfx` path, as the `ParticleSystem` names it.
    pub(crate) path: String,
    /// The timeline's working copy: what it draws, and what its edits change.
    pub(crate) doc: VfxAsset,
    /// The source as it was when the copy last matched it.
    synced: VfxAsset,
    /// A project file. A built-in effect is shown, not edited.
    pub(crate) editable: bool,
    /// The stand-in clip length this module last set, to tell the ruler's
    /// length handle moving from the effect's own duration changing.
    dur_seen: Option<f32>,
    /// When the timeline last wrote an edit into the effect, for its autosave.
    edited_at: Option<f64>,
    /// The toolbar's Save was pressed.
    pub(super) save_requested: bool,
}

impl FxDoc {
    /// "Particles · Campfire": the group row's name.
    pub(crate) fn title(&self) -> String {
        let name = if self.doc.name.trim().is_empty() {
            self.path.rsplit(['/', '\\']).next().unwrap_or("").trim_end_matches(".vfx").to_string()
        } else {
            self.doc.name.clone()
        };
        format!("{} · {}", renzora::lang::t_or("timeline.particles", "Particles"), name)
    }

    /// The effect's name alone, for the clip picker of a stand-in clip.
    pub(crate) fn effect_name(&self) -> String {
        self.title().split_once(" · ").map(|(_, n)| n.to_string()).unwrap_or_default()
    }

    pub(crate) fn enabled_track(&self, emitter: usize) -> Option<usize> {
        params::find_track(&self.doc, emitter, &ParamTarget::Enabled)
    }
}

// ── Picking the effect ───────────────────────────────────────────────────────

/// The first `ParticleSystem` below `root`, breadth first, so the effect
/// nearest the animated entity wins.
fn first_descendant(root: Entity, children: &Query<&Children>, systems: &Query<(), With<ParticleSystem>>) -> Option<Entity> {
    let mut queue = std::collections::VecDeque::from([root]);
    let mut guard = 0usize;
    while let Some(e) = queue.pop_front() {
        guard += 1;
        if guard > 10_000 {
            break;
        }
        if e != root && systems.contains(e) {
            return Some(e);
        }
        if let Ok(kids) = children.get(e) {
            queue.extend(kids.iter());
        }
    }
    None
}

/// Decide which effect the timeline shows (see the module docs).
pub(super) fn resolve_target(
    state: Option<Res<AnimationEditorState>>,
    selection: Option<Res<EditorSelection>>,
    context: Option<Res<EditorContext>>,
    systems: Query<(), With<ParticleSystem>>,
    children: Query<&Children>,
    preview: Query<Entity, With<PreviewEffect>>,
    mut target: ResMut<FxTarget>,
) {
    let doc_mode = active_vfx_path(context.as_deref()).is_some();
    let entity = if doc_mode {
        preview.iter().next()
    } else {
        let raw = selection.and_then(|s| s.get());
        let resolved = state.and_then(|s| s.selected_entity);
        raw.filter(|e| systems.contains(*e))
            .or_else(|| resolved.filter(|e| systems.contains(*e)))
            .or_else(|| resolved.and_then(|e| first_descendant(e, &children, &systems)))
    };
    if target.entity != entity || target.doc_mode != doc_mode {
        target.entity = entity;
        target.doc_mode = doc_mode;
    }
}

// ── Keeping the copy in step ─────────────────────────────────────────────────

/// The effect as its owner has it: the effect editor's document when that is
/// this file, else the loaded asset.
fn source<'w>(world: &'w World, path: &str, handle: &Handle<VfxAsset>) -> Option<&'w VfxAsset> {
    link::open_doc(world, path).or_else(|| world.get_resource::<Assets<VfxAsset>>()?.get(handle))
}

/// Selections address effect keys by track index, which a reload or an undo
/// renumbers: drop them rather than let them point at a neighbour.
fn drop_fx_selection(world: &mut World) {
    let has_fx = |k: &super::clip::SelKey| matches!(k.lane, Lane::Fx { .. });
    let any = world.get_resource::<SelectedKeys>().is_some_and(|s| s.keys.iter().any(has_fx))
        || world.get_resource::<SelectedKey>().and_then(|s| s.0).is_some_and(|k| has_fx(&k));
    if any {
        if let Some(mut s) = world.get_resource_mut::<SelectedKeys>() {
            s.clear();
        }
        if let Some(mut s) = world.get_resource_mut::<SelectedKey>() {
            s.0 = None;
        }
    }
}

/// Load, reconcile, save and publish the timeline's copy of the effect. See
/// the module docs.
pub(super) fn sync_fx(world: &mut World) {
    let target = world.resource::<FxTarget>().entity;
    let has_clip = world.resource::<TimelineClip>().clip.is_some();
    let found = target
        .filter(|_| has_clip)
        .and_then(|e| world.get::<ParticleSystem>(e).map(|s| (e, s.path.clone(), s.asset.clone())));
    let Some((entity, path, handle)) = found else {
        if world.resource::<TimelineClip>().fx.is_some() {
            world.resource_mut::<TimelineClip>().fx = None;
            drop_fx_selection(world);
        }
        publish(world);
        return;
    };

    let reload = world
        .resource::<TimelineClip>()
        .fx
        .as_ref()
        .is_none_or(|f| f.entity != entity || !link::same_path(&f.path, &path));
    if reload {
        let Some(src) = source(world, &path, &handle).cloned() else {
            // Not loaded yet: try again next frame.
            if world.resource::<TimelineClip>().fx.is_some() {
                world.resource_mut::<TimelineClip>().fx = None;
            }
            publish(world);
            return;
        };
        let editable = link::is_editable_path(&path);
        world.resource_mut::<TimelineClip>().fx = Some(FxDoc {
            entity,
            path: path.clone(),
            doc: src.clone(),
            synced: src,
            editable,
            dur_seen: None,
            edited_at: None,
            save_requested: false,
        });
        drop_fx_selection(world);
    } else {
        reconcile(world, &path, &handle);
    }
    sync_duration(world);
    apply_focus(world);
    save(world, &path);
    publish(world);
}

/// Push the copy's edits, or adopt the source's.
fn reconcile(world: &mut World, path: &str, handle: &Handle<VfxAsset>) {
    let pending = world
        .resource::<TimelineClip>()
        .fx
        .as_ref()
        .filter(|f| f.doc != f.synced)
        .map(|f| (f.doc.tracks.clone(), f.doc.duration, f.editable));
    if let Some((tracks, duration, editable)) = pending {
        let written = editable
            && link::edit_effect(world, path, |d| {
                d.tracks = tracks;
                d.duration = duration;
            });
        let now = world.resource::<Time<Real>>().elapsed_secs_f64();
        let fresh = source(world, path, handle).cloned();
        let mut cache = world.resource_mut::<TimelineClip>();
        let Some(fx) = cache.fx.as_mut() else { return };
        match (written, fresh) {
            (true, Some(src)) => {
                fx.doc = src.clone();
                fx.synced = src;
                fx.edited_at = Some(now);
            }
            // Refused (a built-in, a file that will not open): put the copy
            // back, so the timeline never shows an edit that did not happen.
            _ => fx.doc = fx.synced.clone(),
        }
        return;
    }
    let changed = {
        let cache = world.resource::<TimelineClip>();
        let Some(fx) = cache.fx.as_ref() else { return };
        source(world, path, handle).is_some_and(|s| *s != fx.synced)
    };
    if changed {
        let Some(src) = source(world, path, handle).cloned() else { return };
        let tracks_changed = world.resource::<TimelineClip>().fx.as_ref().is_some_and(|f| f.synced.tracks.len() != src.tracks.len());
        if let Some(fx) = world.resource_mut::<TimelineClip>().fx.as_mut() {
            fx.doc = src.clone();
            fx.synced = src;
        }
        if tracks_changed {
            drop_fx_selection(world);
        }
    }
}

/// The ruler length of a stand-in clip for an effect `duration` long. An
/// effect with no end (`0`) still needs a ruler; two seconds is the default
/// effect length.
fn timeline_len(duration: f32) -> f32 {
    if duration.is_finite() && duration > 0.0 {
        duration.clamp(0.2, 600.0)
    } else {
        2.0
    }
}

/// On a stand-in clip the ruler's length is the effect's duration, both ways:
/// dragging the length handle sets the effect's, and a duration changed in
/// the effect editor moves the handle.
fn sync_duration(world: &mut World) {
    let mut cache = world.resource_mut::<TimelineClip>();
    if !cache.is_effect_only() {
        return;
    }
    let c = &mut *cache;
    let (Some(clip), Some(fx)) = (c.clip.as_mut(), c.fx.as_mut()) else { return };
    let handle_moved = fx.dur_seen.is_some_and(|seen| (clip.duration - seen).abs() > 1e-5);
    if handle_moved && fx.editable {
        fx.doc.duration = clip.duration;
        fx.dur_seen = Some(clip.duration);
        return;
    }
    let want = timeline_len(fx.doc.duration);
    if (clip.duration - want).abs() > 1e-5 {
        clip.duration = want;
    }
    fx.dur_seen = Some(want);
}

/// Select the track "Animate" (or "+ Track") asked for, once its row exists.
fn apply_focus(world: &mut World) {
    let Some((path, track)) = world.get_resource::<TimelineLink>().and_then(|l| l.focus.clone()) else { return };
    let shown = world.resource::<TimelineClip>().fx.as_ref().map(|f| f.path.clone());
    if !shown.is_some_and(|p| link::same_path(&p, &path)) {
        return;
    }
    let th = world.get_resource::<AnimationEditorState>().map(|s| s.track_height).unwrap_or(super::ROW_DEFAULT).max(1.0);
    let Some(mut layout) = world.get_resource_mut::<TrackLayout>() else { return };
    let row = layout.rows.iter().position(|r| matches!(r.kind, RowKind::FxTrack(t) | RowKind::FxEmitter(_, Some(t)) if t == track));
    let Some(row) = row else { return };
    let kind = layout.rows[row].kind;
    layout.selected = Some(kind);
    let top = layout.row_top(row, th);
    if top < 0.0 || top + th > layout.view.y {
        layout.vscroll = (row as f32 * th - th).clamp(0.0, layout.max_vscroll(th));
    }
    if let Some(mut l) = world.get_resource_mut::<TimelineLink>() {
        l.focus = None;
    }
}

/// Save on the toolbar's Save, and autosave edits the timeline made to an
/// effect that is not on a document tab (nobody would press its Save).
fn save(world: &mut World, path: &str) {
    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    let (requested, due) = {
        let cache = world.resource::<TimelineClip>();
        let Some(fx) = cache.fx.as_ref() else { return };
        (fx.save_requested, fx.edited_at.is_some_and(|at| now - at > AUTOSAVE_AFTER))
    };
    if !requested && !due {
        return;
    }
    if requested || !link::is_active_document(world, path) {
        link::save_effect(world, path);
    }
    if let Some(fx) = world.resource_mut::<TimelineClip>().fx.as_mut() {
        fx.save_requested = false;
        fx.edited_at = None;
    }
}

/// Tell the effect editor which effect the timeline shows, and where its
/// playhead is (its "Animate" keys there).
fn publish(world: &mut World) {
    let shown = world
        .resource::<TimelineClip>()
        .fx
        .as_ref()
        .map(|f| (f.path.clone(), world.get_resource::<AnimationEditorState>().map(|s| s.scrub_time).unwrap_or(0.0)));
    if let Some(mut l) = world.get_resource_mut::<TimelineLink>() {
        if l.shown != shown {
            l.shown = shown;
        }
    }
}

// ── The clock ────────────────────────────────────────────────────────────────

/// What [`drive_fx_clock`] has taken over, so it can hand it back.
#[derive(Default)]
pub(super) struct FxClock {
    entity: Option<Entity>,
    /// The timeline has moved the playhead or played since this effect was
    /// picked, so its time is the playhead's.
    engaged: bool,
    last_scrub: f32,
    last_previewing: bool,
}

/// How far an effect playing beside the preview may drift from the playhead
/// before it is re-seeked, seconds. Above a frame so ordinary play never
/// seeks, well under anything visible.
const DRIFT: f32 = 0.1;

/// Drive the shown effect from the timeline's clock (see the module docs).
///
/// It holds the clock through `ParticlePlayback::editor_clock`, which is
/// never saved: pausing the effect through its play state instead would have
/// been written into the scene by a save made while scrubbing, and the game
/// would have started with the effect frozen.
pub(super) fn drive_fx_clock(
    state: Option<Res<AnimationEditorState>>,
    cache: Res<TimelineClip>,
    mut effects: Query<(&mut ParticlePlayback, Option<&ParticleRuntime>)>,
    mut link: Option<ResMut<TimelineLink>>,
    mut clock: Local<FxClock>,
) {
    let Some(state) = state else { return };
    let target = cache.fx.as_ref().map(|f| f.entity);
    if target != clock.entity {
        if let Some(old) = clock.entity.filter(|_| clock.engaged) {
            if let Ok((mut pb, _)) = effects.get_mut(old) {
                pb.bypass_change_detection().editor_clock = None;
            }
        }
        if let Some(l) = link.as_mut().filter(|l| l.driving.is_some()) {
            l.driving = None;
        }
        *clock = FxClock { entity: target, engaged: false, last_scrub: state.scrub_time, last_previewing: state.is_previewing };
        return;
    }
    let Some(entity) = target else { return };
    let t = state.scrub_time;
    let moved = (t - clock.last_scrub).abs() > 1e-6;
    let toggled = state.is_previewing != clock.last_previewing;
    clock.last_scrub = t;
    clock.last_previewing = state.is_previewing;
    if !clock.engaged && !moved && !toggled {
        return;
    }
    let Ok((mut pb, runtime)) = effects.get_mut(entity) else { return };
    if !clock.engaged {
        clock.engaged = true;
        if let Some(l) = link.as_mut() {
            l.driving = Some(entity);
        }
    }
    let sim_time = runtime.map(|r| r.sim.time);
    let pb = pb.bypass_change_detection();
    // Playing, the effect runs at the preview's speed beside the playhead and
    // is re-seeked when they drift (a loop wrap, a jump); held, it sits on the
    // playhead, where a seek (a deterministic replay) puts exactly the frame
    // play reaches at `t`. A rebuild after an edit resumes at the same time,
    // so neither re-seeks every frame.
    let rate = if state.is_previewing { state.preview_speed } else { 0.0 };
    if pb.editor_clock != Some(rate) {
        pb.editor_clock = Some(rate);
    }
    let tolerance = if state.is_previewing { DRIFT } else { 1e-3 };
    if moved && !state.is_previewing || toggled || sim_time.is_none_or(|s| (s - t).abs() > tolerance) {
        pb.seek(t);
    }
}

// ── Rows and their keys ──────────────────────────────────────────────────────

/// Where emitter `emitter` is on, within `0..dur`: the bar its row draws.
pub(crate) fn enabled_spans(doc: &VfxAsset, emitter: usize, dur: f32) -> Vec<(f32, f32)> {
    let Some(e) = doc.emitters.get(emitter) else { return Vec::new() };
    let keys = params::find_track(doc, emitter, &ParamTarget::Enabled).map(|i| doc.tracks[i].keys.as_slice()).unwrap_or(&[]);
    let mut bounds: Vec<f32> = vec![0.0];
    bounds.extend(keys.iter().map(|k| k.time).filter(|t| *t > 0.0 && *t < dur));
    bounds.push(dur.max(0.0));
    let mut out: Vec<(f32, f32)> = Vec::new();
    for w in bounds.windows(2) {
        let (a, b) = (w[0], w[1]);
        if b <= a || !params::enabled_at(keys, a, e.enabled) {
            continue;
        }
        match out.last_mut() {
            Some(last) if (last.1 - a).abs() < 1e-6 => last.1 = b,
            _ => out.push((a, b)),
        }
    }
    out
}

/// `lanes` in the shape of `like`, so a key added between two keys keeps the
/// track's value type.
fn value_like(like: &PinValue, lanes: [f32; 4]) -> PinValue {
    match like {
        PinValue::Float(_) => PinValue::Float(lanes[0]),
        PinValue::Vec2(_) => PinValue::Vec2([lanes[0], lanes[1]]),
        PinValue::Vec3(_) => PinValue::Vec3([lanes[0], lanes[1], lanes[2]]),
        PinValue::Color(_) => PinValue::Color(lanes),
        PinValue::Bool(_) => PinValue::Bool(lanes[0] > 0.5),
        other => other.clone(),
    }
}

/// Key `kind`'s row at `t` without changing what plays there: a parameter
/// keys its current value (its pin's, on a track with no keys yet), a burst
/// repeats the count before it, and an emitter row toggles the emitter, which
/// is what a key on an on/off bar is for. Returns the lane and key index.
pub(super) fn add_key(cache: &mut TimelineClip, kind: RowKind, t: f32) -> Option<(Lane, usize)> {
    let fx = cache.fx.as_mut().filter(|f| f.editable)?;
    let t = t.max(0.0);
    match kind {
        RowKind::FxTrack(i) => {
            let doc = &mut fx.doc;
            let track = doc.tracks.get(i)?;
            let value = match &track.target {
                ParamTarget::Pin { node, pin } => match (track.keys.first(), track.sample(t)) {
                    (Some(first), Some(lanes)) => value_like(&first.value, lanes),
                    _ => params::pin_value(doc, track.emitter, *node, pin)?,
                },
                ParamTarget::Burst => {
                    let before = track.keys.iter().rev().find(|k| k.time <= t).or(track.keys.first());
                    before.map(|k| k.value.clone()).unwrap_or(PinValue::Float(10.0))
                }
                ParamTarget::Enabled => PinValue::Bool(!params::enabled_at(&track.keys, t, true)),
            };
            let idx = doc.tracks[i].upsert(t, value);
            Some((Lane::Fx { track: i }, idx))
        }
        RowKind::FxEmitter(emitter, _) => {
            let doc = &mut fx.doc;
            let default = doc.emitters.get(emitter)?.enabled;
            let i = match params::find_track(doc, emitter, &ParamTarget::Enabled) {
                Some(i) => i,
                None => {
                    // The bar before the first key is the emitter's own flag:
                    // pin it at 0 so the new key is a switch, not a reset.
                    let mut track = ParamTrack { emitter, target: ParamTarget::Enabled, keys: Vec::new() };
                    if t > 1e-4 {
                        track.upsert(0.0, PinValue::Bool(default));
                    }
                    doc.tracks.push(track);
                    doc.tracks.len() - 1
                }
            };
            let on = params::enabled_at(&doc.tracks[i].keys, t, default);
            let idx = doc.tracks[i].upsert(t, PinValue::Bool(!on));
            Some((Lane::Fx { track: i }, idx))
        }
        _ => None,
    }
}

/// Key every parameter track at `t` (the K shortcut, beside the property
/// tracks it already keys). Bursts and on/off keys are events, not values, so
/// they are left alone.
pub(super) fn key_all(cache: &mut TimelineClip, t: f32) {
    let n = cache.fx.as_ref().map(|f| f.doc.tracks.len()).unwrap_or(0);
    for i in 0..n {
        let is_pin = cache.fx.as_ref().is_some_and(|f| matches!(f.doc.tracks[i].target, ParamTarget::Pin { .. }));
        if is_pin {
            add_key(cache, RowKind::FxTrack(i), t);
        }
    }
}

pub(super) fn set_key_interp(cache: &mut TimelineClip, track: usize, idx: usize, interp: KeyInterp) {
    let Some(fx) = cache.fx.as_mut().filter(|f| f.editable) else { return };
    if let Some(k) = fx.doc.tracks.get_mut(track).and_then(|t| t.keys.get_mut(idx)) {
        k.interp = interp;
    }
}

/// Remove an effect track. Selections go with it (see [`drop_fx_selection`]).
pub(super) fn delete_track(world: &mut World, track: usize) {
    let removed = {
        let mut cache = world.resource_mut::<TimelineClip>();
        match cache.fx.as_mut().filter(|f| f.editable) {
            Some(fx) if track < fx.doc.tracks.len() => {
                fx.doc.tracks.remove(track);
                true
            }
            _ => false,
        }
    };
    if removed {
        drop_fx_selection(world);
        if let Some(mut layout) = world.get_resource_mut::<TrackLayout>() {
            if layout.selected.is_some_and(|k| matches!(k, RowKind::FxTrack(_) | RowKind::FxEmitter(..))) {
                layout.selected = None;
            }
        }
    }
}

/// The three interpolations an effect key offers, as `(icon, label, interp)`:
/// the ones the runtime samples. The clip's eased curves have no meaning on
/// an effect parameter.
pub(super) fn interp_choices() -> [(&'static str, String, KeyInterp); 3] {
    [
        ("minus", renzora::lang::t("animation.interp_linear"), KeyInterp::Linear),
        ("chart-line", renzora::lang::t("animation.interp_smooth"), KeyInterp::Smooth),
        ("stairs", renzora::lang::t("animation.interp_stepped"), KeyInterp::Step),
    ]
}

/// The interpolation a clip-style choice stands for on an effect key, for the
/// key card and the Curves toolbar, which speak [`renzora::Interp`].
pub(super) fn interp_from_clip(interp: renzora::Interp) -> Option<KeyInterp> {
    use bevy::curve::EaseFunction as E;
    match interp {
        renzora::Interp::Linear => Some(KeyInterp::Linear),
        renzora::Interp::Stepped => Some(KeyInterp::Step),
        renzora::Interp::Eased(E::SmoothStep) => Some(KeyInterp::Smooth),
        _ => None,
    }
}

pub(super) fn interp_to_clip(interp: KeyInterp) -> renzora::Interp {
    match interp {
        KeyInterp::Linear => renzora::Interp::Linear,
        KeyInterp::Step => renzora::Interp::Stepped,
        KeyInterp::Smooth => renzora::Interp::Eased(bevy::curve::EaseFunction::SmoothStep),
    }
}

/// Hash what the track list shows of the effect: names, flags and which
/// tracks exist. Part of the layout signature and the header token.
pub(crate) fn structure_hash(fx: &FxDoc, h: &mut impl Hasher) {
    use std::hash::Hash;
    (fx.entity, &fx.path, &fx.doc.name, fx.editable).hash(h);
    for e in &fx.doc.emitters {
        (&e.name, e.enabled).hash(h);
    }
    for t in &fx.doc.tracks {
        (t.emitter, &t.target, t.keys.is_empty()).hash(h);
    }
}

/// Fold every effect key's time and value into the run fingerprint: an
/// emitter's on/off bar changes with a key's value, not only its time.
pub(crate) fn mix_keys(fx: &FxDoc, mix: &mut impl FnMut(u64, f32)) {
    for (i, t) in fx.doc.tracks.iter().enumerate() {
        let lane = 2_000_000 + i as u64 * 2;
        for k in &t.keys {
            mix(lane, k.time);
            let v = params::pin_lanes(&k.value).unwrap_or([0.0; 4]);
            mix(lane + 1, v[0] + v[1] * 3.0 + v[2] * 7.0 + v[3] * 11.0 + k.interp as u8 as f32 * 13.0);
        }
    }
}

// ── Menus and buttons ────────────────────────────────────────────────────────

/// A parameter row's "key at the playhead" button (carries the track index).
#[derive(Component)]
pub(super) struct FxAddKeyBtn(pub(super) usize);
/// A parameter row's delete button (carries the track index).
#[derive(Component)]
pub(super) struct FxDeleteTrackBtn(pub(super) usize);

/// The parameter rows' buttons: key at the playhead, delete the track.
pub(super) fn fx_header_click(
    addks: Query<(&UiInteraction, &FxAddKeyBtn), Changed<UiInteraction>>,
    dels: Query<(&UiInteraction, &FxDeleteTrackBtn), Changed<UiInteraction>>,
    state: Option<Res<AnimationEditorState>>,
    mut cache: ResMut<TimelineClip>,
    mut commands: Commands,
) {
    let t = state.map(|s| s.scrub_time).unwrap_or(0.0);
    for (i, b) in &addks {
        if *i == UiInteraction::Pressed {
            add_key(&mut cache, RowKind::FxTrack(b.0), t);
        }
    }
    for (i, b) in &dels {
        if *i == UiInteraction::Pressed {
            let track = b.0;
            commands.queue(move |w: &mut World| delete_track(w, track));
        }
    }
}

/// Add `track` to the effect and select its row.
fn add_track(world: &mut World, track: ParamTrack) {
    let added = {
        let mut cache = world.resource_mut::<TimelineClip>();
        cache.fx.as_mut().filter(|f| f.editable).map(|fx| {
            fx.doc.tracks.push(track);
            (fx.path.clone(), fx.doc.tracks.len() - 1)
        })
    };
    if let (Some(focus), Some(mut l)) = (added, world.get_resource_mut::<TimelineLink>()) {
        l.focus = Some(focus);
    }
}

/// "+ Track" when an effect is shown: a property track (on a real clip), and
/// a Particle parameter submenu with each emitter's on/off, bursts and the
/// graph's keyable pins.
pub(super) fn open_add_track_menu(world: &mut World, pos: Vec2) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
    let scrub = world.get_resource::<AnimationEditorState>().map(|s| s.scrub_time).unwrap_or(0.0);
    let (effect_only, fx) = {
        let cache = world.resource::<TimelineClip>();
        (cache.is_effect_only(), cache.fx.as_ref().map(|f| (f.doc.clone(), f.editable)))
    };
    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        let menu = screen_menu(&mut commands, pos.x, pos.y);
        let mut kids = Vec::new();
        if !effect_only {
            kids.push(menu_item(&mut commands, &fonts, "sliders-horizontal", &renzora::lang::t_or("timeline.property_track", "Property track"), |w| {
                if let Some(mut ops) = w.get_resource_mut::<TimelineOps>() {
                    ops.add_empty_track = true;
                }
            }));
        }
        match fx {
            Some((doc, true)) => {
                let (row, content) = menu_submenu(&mut commands, &fonts, "sparkle", &renzora::lang::t_or("timeline.particle_parameter", "Particle parameter"));
                let several = doc.emitters.len() > 1;
                let mut sub_kids = Vec::new();
                for (e, emitter) in doc.emitters.iter().enumerate() {
                    let items = emitter_items(&mut commands, &fonts, &doc, e, scrub);
                    if several {
                        let (erow, econtent) = menu_submenu(&mut commands, &fonts, "sparkle", &emitter.name);
                        commands.entity(econtent).add_children(&items);
                        sub_kids.push(erow);
                    } else {
                        sub_kids.extend(items);
                    }
                }
                commands.entity(content).add_children(&sub_kids);
                kids.push(row);
            }
            Some((_, false)) => {
                kids.push(menu_header(&mut commands, &fonts, &renzora::lang::t_or("timeline.builtin_effect", "Built-in effect: copy it into the project to key it")));
            }
            None => {}
        }
        commands.entity(menu).add_children(&kids);
    }
    queue.apply(world);
}

/// One emitter's entries in the Particle parameter submenu.
fn emitter_items(commands: &mut Commands, fonts: &EmberFonts, doc: &VfxAsset, e: usize, scrub: f32) -> Vec<Entity> {
    let mut items = Vec::new();
    let enabled = doc.emitters.get(e).is_some_and(|em| em.enabled);
    if params::find_track(doc, e, &ParamTarget::Enabled).is_none() {
        items.push(menu_item(commands, fonts, "toggle-right", &renzora::lang::t_or("timeline.fx_enabled", "Enabled"), move |w| {
            let keys = vec![params::ParamKey { time: 0.0, value: PinValue::Bool(enabled), interp: KeyInterp::Step }];
            add_track(w, ParamTrack { emitter: e, target: ParamTarget::Enabled, keys });
        }));
    }
    items.push(menu_item(commands, fonts, "asterisk", &renzora::lang::t_or("timeline.fx_burst", "Burst"), move |w| {
        let mut track = ParamTrack { emitter: e, target: ParamTarget::Burst, keys: Vec::new() };
        track.upsert(scrub.max(0.0), PinValue::Float(10.0));
        add_track(w, track);
    }));
    let pins: Vec<_> = params::keyable_pins(doc, e)
        .into_iter()
        .filter(|p| params::find_track(doc, e, &ParamTarget::Pin { node: p.node, pin: p.pin.to_string() }).is_none())
        .collect();
    if !pins.is_empty() {
        items.push(menu_sep(commands));
    }
    for p in pins {
        let (node, pin) = (p.node, p.pin);
        items.push(menu_item(commands, fonts, "diamond", &p.label, move |w| {
            let track = {
                let cache = w.resource::<TimelineClip>();
                cache.fx.as_ref().and_then(|f| params::new_pin_track(&f.doc, e, node, pin, scrub))
            };
            if let Some(track) = track {
                add_track(w, track);
            }
        }));
    }
    items
}

/// The right-click menu of an effect key: delete, and the interpolations a
/// parameter key has.
pub(super) fn key_menu_items(commands: &mut Commands, fonts: &EmberFonts, cache: &TimelineClip, track: usize, idx: usize) -> Vec<Entity> {
    let mut kids = vec![menu_item(commands, fonts, "trash", &renzora::lang::t("timeline.delete_keyframe"), move |w| {
        if let Some(mut cache) = w.get_resource_mut::<TimelineClip>() {
            if let Some(mut chan) = cache.lane_times(Lane::Fx { track }) {
                chan.remove(idx);
            }
        }
        drop_fx_selection(w);
    })];
    let Some(t) = cache.fx.as_ref().and_then(|f| f.doc.tracks.get(track)) else { return kids };
    if matches!(t.target, ParamTarget::Pin { .. }) {
        let current = t.keys.get(idx).map(|k| k.interp);
        for (icon, label, interp) in interp_choices() {
            let icon = if current == Some(interp) { "check" } else { icon };
            kids.push(menu_item(commands, fonts, icon, &label, move |w| {
                if let Some(mut cache) = w.get_resource_mut::<TimelineClip>() {
                    set_key_interp(&mut cache, track, idx, interp);
                }
            }));
        }
    }
    kids
}

/// The right-click menu of an effect row in the track list.
pub(super) fn row_menu_items(commands: &mut Commands, fonts: &EmberFonts, kind: RowKind) -> Vec<Entity> {
    let track = match kind {
        RowKind::FxTrack(t) | RowKind::FxEmitter(_, Some(t)) => t,
        _ => return Vec::new(),
    };
    let label = match kind {
        RowKind::FxEmitter(..) => renzora::lang::t_or("timeline.remove_on_off_keys", "Remove on/off keys"),
        _ => renzora::lang::t_or("timeline.delete_track", "Delete track"),
    };
    vec![menu_item(commands, fonts, "trash", &label, move |w| delete_track(w, track))]
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_particles::asset::{Emitter, VfxGraph, VfxNode};
    use renzora_particles::nodes::OUTPUT_NODE;

    fn doc() -> VfxAsset {
        VfxAsset {
            name: "Fire".into(),
            emitters: vec![Emitter {
                name: "Flames".into(),
                graph: VfxGraph { nodes: vec![VfxNode::new(1, OUTPUT_NODE).with("spawn_rate", PinValue::Float(20.0))], ..Default::default() },
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn cache_with(doc: VfxAsset) -> TimelineClip {
        TimelineClip {
            fx: Some(FxDoc {
                entity: Entity::PLACEHOLDER,
                path: "effects/fire.vfx".into(),
                synced: doc.clone(),
                doc,
                editable: true,
                dur_seen: None,
                edited_at: None,
                save_requested: false,
            }),
            ..Default::default()
        }
    }

    #[test]
    fn an_emitter_bar_covers_where_it_is_on() {
        let mut d = doc();
        assert_eq!(enabled_spans(&d, 0, 3.0), vec![(0.0, 3.0)]);
        let mut t = ParamTrack { emitter: 0, target: ParamTarget::Enabled, keys: Vec::new() };
        t.upsert(0.0, PinValue::Bool(true));
        t.upsert(1.0, PinValue::Bool(false));
        t.upsert(2.0, PinValue::Bool(true));
        d.tracks.push(t);
        assert_eq!(enabled_spans(&d, 0, 3.0), vec![(0.0, 1.0), (2.0, 3.0)]);
        d.emitters[0].enabled = false;
        d.tracks.clear();
        assert!(enabled_spans(&d, 0, 3.0).is_empty());
    }

    #[test]
    fn a_key_on_an_emitter_row_switches_it() {
        let mut cache = cache_with(doc());
        let (lane, _) = add_key(&mut cache, RowKind::FxEmitter(0, None), 1.5).unwrap();
        let fx = cache.fx.as_ref().unwrap();
        let Lane::Fx { track } = lane else { panic!("an effect lane") };
        let keys = &fx.doc.tracks[track].keys;
        // The emitter's own state pinned at 0, then the switch.
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].value, PinValue::Bool(true));
        assert_eq!(keys[1].value, PinValue::Bool(false));
        assert_eq!(enabled_spans(&fx.doc, 0, 3.0), vec![(0.0, 1.5)]);
    }

    #[test]
    fn a_key_on_a_parameter_keeps_its_value() {
        let mut d = doc();
        let mut t = params::new_pin_track(&d, 0, 1, "spawn_rate", 0.0).unwrap();
        t.upsert(2.0, PinValue::Float(60.0));
        d.tracks.push(t);
        let mut cache = cache_with(d);
        let (_, idx) = add_key(&mut cache, RowKind::FxTrack(0), 1.0).unwrap();
        let key = &cache.fx.as_ref().unwrap().doc.tracks[0].keys[idx];
        assert_eq!(key.value, PinValue::Float(40.0), "halfway between 20 and 60");
    }

    #[test]
    fn a_read_only_effect_takes_no_keys() {
        let mut cache = cache_with(doc());
        cache.fx.as_mut().unwrap().editable = false;
        assert!(add_key(&mut cache, RowKind::FxEmitter(0, None), 1.0).is_none());
        assert!(cache.lane_times(Lane::Fx { track: 0 }).is_none());
    }

    #[test]
    fn an_endless_effect_still_gets_a_ruler() {
        assert_eq!(timeline_len(0.0), 2.0);
        assert_eq!(timeline_len(5.0), 5.0);
        assert_eq!(timeline_len(f32::NAN), 2.0);
    }
}
