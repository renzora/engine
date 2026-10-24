//! What the pointer does in the Curves view: pick and box-select keys, drag
//! them in time and value, drag a smooth key's tangent handles, pan and zoom
//! the value axis, frame, and the right-click menu.
//!
//! The drawing is [`super::curves`]; everything here edits the clip buffer
//! directly, like the dopesheet's own drag does, so every gesture lands on
//! the undo stack through the clip's snapshot observer
//! ([`super::undo::anim_undo_observer`]), which also rebuilds the playing
//! clip, so a curve plays the moment it is reshaped.
//!
//! Picking is by math against the clip, not by per-node interaction, for the
//! same reason as the dopesheet's (see [`super::edit`]): the keyed list
//! rebuilds the node under the cursor the moment its key moves.
//!
//! A drag freezes the value window first ([`CurveView`]): framed to its keys,
//! the view would slide under the cursor as the key it holds moves.

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora::{euler_deg_to_quat, euler_slope_of_quat_tangent, quat_tangent_of_euler_slope, quat_to_euler_deg, AnimClip, KeyMode, TrackValue};
use renzora_ember::font::EmberFonts;
use renzora_ember::widgets::{menu_item, screen_menu, LANE_INSET};
use renzora_ember::UiInteraction;

use crate::{AnimEditorAction, AnimEditorBridge, AnimationEditorState};

use super::clip::{Lane, SelKey, SelectedKey, SelectedKeys, TimelineClip};
use super::curves::{current_set, frame_of, handle_points, span_of, CurvePick, CurveSel, CurveSet, CurveView, Frame, FrameSelBtn, TimelineMode};
use super::edit::{fit_zoom, push, KeyDragState};
use super::layout::TrackLayout;
use super::popover::PopoverState;
use super::snapshots::x_of;
use super::{AnimBtn, AnimTimeline, KeyLane};

/// How close, px, a press must land to a key or a handle to take it.
const PICK_PX: f32 = 7.0;

/// The Curves view's gesture in flight, and the frame requests its toolbar
/// buttons leave for [`graph_sync`].
#[derive(Resource, Default)]
pub(crate) struct GraphDrag {
    gesture: Option<Gesture>,
    pan: Option<Pan>,
    /// The last right-button gesture moved the view: its release is not a
    /// click, and opens no menu.
    pan_dragged: bool,
    frame_request: Option<FrameRequest>,
    /// Wheel lines gathered by [`graph_wheel_read`] this frame.
    wheel: f32,
}

#[derive(Clone, Copy, PartialEq)]
enum FrameRequest {
    All,
    Selected,
}

struct Pan {
    button: MouseButton,
    start: Vec2,
    scroll0: f32,
    range0: (f32, f32),
    moved: bool,
}

enum Gesture {
    /// A press on empty space: a click moves the playhead, a drag boxes.
    Press { at: Vec2, shift: bool },
    /// A box being drawn from `a` (drawn by `edit::sync_box_rect`).
    Box { a: Vec2, shift: bool },
    /// Keys moving: each with its time and value when the drag began.
    Keys { start: Vec2, moved: bool, clicked: CurvePick, keys: Vec<Moving> },
    /// A tangent handle: `out` is the right-hand one.
    Handle { pick: CurvePick, out: bool },
}

struct Moving {
    key: SelKey,
    time0: f32,
    value0: Raw,
    comps: Vec<usize>,
}

/// A key's value as stored, read at the start of a drag.
#[derive(Clone, Copy)]
enum Raw {
    Bone(Vec4),
    Prop(TrackValue),
}

// ── Reading and writing keys ─────────────────────────────────────────────────

fn raw_value(clip: &AnimClip, k: SelKey) -> Option<Raw> {
    match k.lane {
        Lane::Bone { track, channel } => clip.tracks.get(track)?.channel_keys(channel).get(k.index).map(|v| Raw::Bone(v.1)),
        Lane::Prop { track } => clip.property_tracks.get(track)?.keys.get(k.index).map(|key| Raw::Prop(key.value)),
        // A weight rides in `x`, as in its spline.
        Lane::Morph { track } => clip.morph_tracks.get(track)?.keys.get(k.index).map(|key| Raw::Bone(Vec4::new(key.1, 0.0, 0.0, 0.0))),
        Lane::Fx { .. } => None,
    }
}

/// A rotation `q0` turned by `deltas` (component, degrees) about its Euler
/// angles, kept in `q0`'s sign so a hand-set tangent stored against that sign
/// still means what it did.
fn turned(q0: Vec4, deltas: &[(usize, f32)]) -> Vec4 {
    let mut e = quat_to_euler_deg(Quat::from_vec4(q0).normalize());
    for (c, dv) in deltas {
        e[(*c).min(2)] += dv;
    }
    let q = Vec4::from(euler_deg_to_quat(e));
    if q.dot(q0) < 0.0 {
        -q
    } else {
        q
    }
}

/// Write key `k` as `value0` moved by `deltas` (component, shown units).
/// Always from the value the drag began with, never the current one, so a
/// rotation's Euler angles do not drift as they are re-read every frame.
fn write_value(clip: &mut AnimClip, k: SelKey, value0: Raw, deltas: &[(usize, f32)]) {
    match (k.lane, value0) {
        (Lane::Bone { track, channel }, Raw::Bone(v0)) => {
            let Some(t) = clip.tracks.get_mut(track) else { return };
            if channel == 1 {
                if let Some(key) = t.rotations.get_mut(k.index) {
                    key.1 = turned(v0, deltas).to_array();
                }
            } else {
                let mut v = v0;
                for (c, dv) in deltas {
                    v[(*c).min(2)] += dv;
                }
                let keys = if channel == 0 { &mut t.translations } else { &mut t.scales };
                if let Some(key) = keys.get_mut(k.index) {
                    key.1 = v.truncate().to_array();
                }
            }
        }
        (Lane::Prop { track }, Raw::Prop(v0)) => {
            let Some(key) = clip.property_tracks.get_mut(track).and_then(|p| p.keys.get_mut(k.index)) else { return };
            let sum = |c: usize| deltas.iter().filter(|d| d.0 == c).map(|d| d.1).sum::<f32>();
            key.value = match v0 {
                TrackValue::Float(x) => TrackValue::Float(x + sum(0)),
                TrackValue::Vec3(a) => TrackValue::Vec3([a[0] + sum(0), a[1] + sum(1), a[2] + sum(2)]),
                TrackValue::Color(c) => TrackValue::Color([c[0] + sum(0), c[1] + sum(1), c[2] + sum(2), c[3] + sum(3)]),
                TrackValue::Quat(q) => TrackValue::Quat(turned(Vec4::from_array(q), deltas).to_array()),
                TrackValue::Bool(b) => TrackValue::Bool(f32::from(u8::from(b)) + sum(0) >= 0.5),
            };
        }
        (Lane::Morph { track }, Raw::Bone(v0)) => {
            let Some(key) = clip.morph_tracks.get_mut(track).and_then(|m| m.keys.get_mut(k.index)) else { return };
            key.1 = v0.x + deltas.iter().filter(|d| d.0 == 0).map(|d| d.1).sum::<f32>();
        }
        _ => {}
    }
}

/// Set one side of a bone key's tangent from a handle dragged to `slope`
/// (shown units per second, on component `comp`).
///
/// A key that was not hand-set becomes Free (Broken with Alt held), seeded
/// with the tangents it had, so only the dragged component moves. A rotation's
/// handle is dragged on an Euler curve: its tangent is converted to Euler
/// slopes, the one component replaced, and converted back.
fn set_handle(clip: &mut AnimClip, pick: CurvePick, out: bool, slope: f32, broken: bool) {
    let want = if broken { KeyMode::Broken } else { KeyMode::Free };
    // A blend shape key: one component, no rotation to convert through.
    if let Lane::Morph { track } = pick.key.lane {
        let current = clip.morph_tracks.get(track).map(|t| t.key_curve(pick.key.index).mode);
        if current.is_some_and(|m| !m.is_hand_set() || (broken && m == KeyMode::Free)) {
            super::keymode::set_bone_mode(clip, pick.key, want);
        }
        let Some(t) = clip.morph_tracks.get_mut(track) else { return };
        let mut c = t.key_curve(pick.key.index);
        let mut tan = if out || c.mode == KeyMode::Free { c.out_tan } else { c.in_tan };
        tan[0] = slope;
        match c.mode {
            KeyMode::Broken if out => c.out_tan = tan,
            KeyMode::Broken => c.in_tan = tan,
            _ => {
                c.out_tan = tan;
                c.in_tan = tan;
            }
        }
        t.set_key_curve(pick.key.index, c);
        return;
    }
    let Lane::Bone { track, channel } = pick.key.lane else { return };
    let current = clip.tracks.get(track).map(|t| t.key_curve(channel, pick.key.index).mode);
    if current.is_some_and(|m| !m.is_hand_set() || (broken && m == KeyMode::Free)) {
        super::keymode::set_bone_mode(clip, pick.key, want);
    }
    let Some(t) = clip.tracks.get_mut(track) else { return };
    let mut c = t.key_curve(channel, pick.key.index);
    let side = if out || c.mode == KeyMode::Free { c.out_tan } else { c.in_tan };
    let mut tan = Vec4::from_array(side);
    if channel == 1 {
        let Some(q) = t.channel_keys(1).get(pick.key.index).map(|k| k.1) else { return };
        let mut e = euler_slope_of_quat_tangent(q, tan);
        e[pick.comp.min(2)] = slope;
        tan = quat_tangent_of_euler_slope(q, e);
    } else {
        tan[pick.comp.min(2)] = slope;
    }
    match c.mode {
        KeyMode::Broken if out => c.out_tan = tan.to_array(),
        KeyMode::Broken => c.in_tan = tan.to_array(),
        _ => {
            c.out_tan = tan.to_array();
            c.in_tan = tan.to_array();
        }
    }
    t.set_key_curve(channel, pick.key.index, c);
}

// ── Picking ──────────────────────────────────────────────────────────────────

fn cursor(world: &mut World, over_only: bool) -> Option<Vec2> {
    let mut q = world.query_filtered::<(&RelativeCursorPosition, &ComputedNode), With<KeyLane>>();
    let (rcp, cn) = q.iter(world).next()?;
    if over_only && !rcp.cursor_over {
        return None;
    }
    Some((rcp.normalized? + Vec2::splat(0.5)) * (cn.size() * cn.inverse_scale_factor()))
}

/// The key point of `ch`'s key `i`, lane space.
fn key_point(set: &CurveSet, comp: usize, lane: Lane, i: usize, frame: &Frame, zoom: f32, scroll: f32) -> Option<Vec2> {
    let ch = set.channels.iter().find(|c| c.comp == comp && c.lane == lane)?;
    let (t, v) = *ch.keys.get(i)?;
    Some(Vec2::new(x_of(t, zoom, scroll), frame.y_of(v)))
}

/// The curve key nearest `p`, within [`PICK_PX`].
pub(super) fn pick_key(set: &CurveSet, frame: &Frame, zoom: f32, scroll: f32, p: Vec2) -> Option<CurvePick> {
    let mut best: Option<(f32, CurvePick)> = None;
    for ch in &set.channels {
        for (i, (t, v)) in ch.keys.iter().enumerate() {
            let d = Vec2::new(x_of(*t, zoom, scroll), frame.y_of(*v)).distance(p);
            if d <= PICK_PX && best.is_none_or(|b| d < b.0) {
                best = Some((d, CurvePick { key: SelKey { lane: ch.lane, index: i }, comp: ch.comp }));
            }
        }
    }
    best.map(|b| b.1)
}

/// A selected key's tangent handle under `p`, and which side it is.
fn pick_handle(set: &CurveSet, sel: &CurveSel, frame: &Frame, zoom: f32, scroll: f32, p: Vec2) -> Option<(CurvePick, bool)> {
    for pick in &sel.picks {
        if !set.has_handles(pick.key.index) {
            continue;
        }
        let Some(k) = key_point(set, pick.comp, pick.key.lane, pick.key.index, frame, zoom, scroll) else { continue };
        let Some(slopes) = set.slopes(pick.comp, pick.key.index) else { continue };
        let n = set.channels.iter().find(|c| c.comp == pick.comp).map(|c| c.keys.len()).unwrap_or(0);
        let (a, b) = handle_points(k, slopes, zoom, frame);
        if pick.key.index + 1 < n && b.distance(p) <= PICK_PX {
            return Some((*pick, true));
        }
        if pick.key.index > 0 && a.distance(p) <= PICK_PX {
            return Some((*pick, false));
        }
    }
    None
}

fn picks_in_box(set: &CurveSet, frame: &Frame, zoom: f32, scroll: f32, a: Vec2, b: Vec2) -> Vec<CurvePick> {
    let (lo, hi) = (a.min(b), a.max(b));
    let mut out = Vec::new();
    for ch in &set.channels {
        for (i, (t, v)) in ch.keys.iter().enumerate() {
            let q = Vec2::new(x_of(*t, zoom, scroll), frame.y_of(*v));
            if q.x >= lo.x && q.x <= hi.x && q.y >= lo.y && q.y <= hi.y {
                out.push(CurvePick { key: SelKey { lane: ch.lane, index: i }, comp: ch.comp });
            }
        }
    }
    out
}

/// Select `picks`, mirrored into the dopesheet's selection (by key).
fn select(world: &mut World, picks: Vec<CurvePick>, primary: Option<SelKey>) {
    let mut keys: Vec<SelKey> = Vec::new();
    for p in &picks {
        if !keys.contains(&p.key) {
            keys.push(p.key);
        }
    }
    let primary = primary.or_else(|| keys.first().copied());
    world.resource_mut::<SelectedKeys>().set(keys);
    world.resource_mut::<SelectedKey>().0 = primary;
    world.resource_mut::<CurveSel>().set(picks);
}

/// Pin the value window where it is, so a drag does not move it.
fn freeze_view(world: &mut World, set: &CurveSet, frame: &Frame) {
    let mut v = world.resource_mut::<CurveView>();
    if v.subject != Some(set.subject) || v.range.is_none() {
        v.subject = Some(set.subject);
        v.range = Some((frame.vmin, frame.vmax));
    }
}

// ── Systems ──────────────────────────────────────────────────────────────────

/// Left button in the Curves view: press, drag and release of every gesture
/// above. Runs after the dopesheet's own drag (which stands aside in this
/// view) and before the ruler's, which owns presses in the ruler.
pub(super) fn graph_input(world: &mut World) {
    if !world.get_resource::<TimelineMode>().is_some_and(|m| m.curves) {
        return;
    }
    if world.get_resource::<KeyDragState>().is_some_and(|d| d.scrollbar || d.ruler.is_some()) {
        return;
    }
    let Some(state) = world.get_resource::<AnimationEditorState>() else { return };
    let (zoom, scroll) = (state.timeline_zoom.max(1.0), state.timeline_scroll);
    let (snap_on, interval) = (state.snap_enabled && state.snap_interval > 0.0, state.snap_interval);
    let buttons = world.resource::<ButtonInput<MouseButton>>();
    let (pressed, just_pressed) = (buttons.pressed(MouseButton::Left), buttons.just_pressed(MouseButton::Left));
    let keys = world.resource::<ButtonInput<KeyCode>>();
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let alt = keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);
    let Some(set) = current_set(world) else {
        world.resource_mut::<GraphDrag>().gesture = None;
        return;
    };
    let h = world.get_resource::<TrackLayout>().map(|l| l.view.y).unwrap_or(60.0);
    let frame = frame_of(world, &set, h);
    let dur = world.get_resource::<TimelineClip>().and_then(|c| c.clip.as_ref()).map(|c| c.duration).unwrap_or(f32::MAX);
    let cursor_win = {
        let mut q = world.query::<&Window>();
        q.iter(world).find_map(|w| w.cursor_position())
    };

    let gesture = world.resource_mut::<GraphDrag>().gesture.take();
    match gesture {
        None => {}
        Some(Gesture::Press { at, shift }) => {
            let cur = cursor(world, false).unwrap_or(at);
            if pressed {
                let g = if cur.distance(at) > 4.0 { Gesture::Box { a: at, shift } } else { Gesture::Press { at, shift } };
                world.resource_mut::<GraphDrag>().gesture = Some(g);
                return;
            }
            // A click on empty graph: playhead there, selection dropped.
            select(world, Vec::new(), None);
            let mut t = ((at.x - LANE_INSET) / zoom + scroll).max(0.0);
            if snap_on {
                t = (t / interval).round() * interval;
            }
            if let Some(bridge) = world.get_resource::<AnimEditorBridge>() {
                push(bridge, AnimEditorAction::SetScrubTime(t.min(dur)));
            }
            return;
        }
        Some(Gesture::Box { a, shift }) => {
            let b = cursor(world, false).unwrap_or(a);
            if pressed {
                world.resource_mut::<KeyDragState>().boxed = Some((a, b));
                world.resource_mut::<GraphDrag>().gesture = Some(Gesture::Box { a, shift });
                return;
            }
            world.resource_mut::<KeyDragState>().boxed = None;
            let mut found = picks_in_box(&set, &frame, zoom, scroll, a, b);
            if shift {
                for p in world.resource::<CurveSel>().picks.clone() {
                    if !found.contains(&p) {
                        found.push(p);
                    }
                }
            }
            select(world, found, None);
            return;
        }
        Some(Gesture::Handle { pick, out }) => {
            if !pressed {
                return;
            }
            let (Some(p), Some(k)) = (cursor(world, false), key_point(&set, pick.comp, pick.key.lane, pick.key.index, &frame, zoom, scroll)) else {
                world.resource_mut::<GraphDrag>().gesture = Some(Gesture::Handle { pick, out });
                return;
            };
            let d = p - k;
            // Never vertical, and never round to the other side of the key.
            let dx = if out { d.x.max(2.0) } else { d.x.min(-2.0) };
            let slope = -(d.y / frame.py()) / (dx / zoom);
            let mut cache = world.resource_mut::<TimelineClip>();
            if let Some(clip) = cache.clip.as_mut() {
                set_handle(clip, pick, out, slope, alt);
            }
            cache.dirty = true;
            world.resource_mut::<GraphDrag>().gesture = Some(Gesture::Handle { pick, out });
            return;
        }
        Some(Gesture::Keys { start, mut moved, clicked, keys }) => {
            if !pressed {
                if moved {
                    finish_move(world, &keys);
                } else if let Some(c) = cursor_win {
                    // A click, not a drag: the key's card.
                    world.resource_mut::<SelectedKey>().0 = Some(clicked.key);
                    world.resource_mut::<PopoverState>().request = Some(c);
                }
                return;
            }
            let p = cursor(world, false).unwrap_or(start);
            let d = p - start;
            if !moved && d.length() < 3.0 {
                world.resource_mut::<GraphDrag>().gesture = Some(Gesture::Keys { start, moved, clicked, keys });
                return;
            }
            moved = true;
            // Snap the pressed key's time, and move the rest with it.
            let t0 = keys.iter().find(|m| m.key == clicked.key).map(|m| m.time0).unwrap_or(0.0);
            let mut t = (t0 + d.x / zoom).max(0.0);
            if snap_on {
                t = (t / interval).round() * interval;
            }
            let dt = t.min(dur) - t0;
            let dv = -d.y / frame.py();
            let mut cache = world.resource_mut::<TimelineClip>();
            for m in &keys {
                if let Some(clip) = cache.clip.as_mut() {
                    let deltas: Vec<(usize, f32)> = m.comps.iter().map(|c| (*c, dv)).collect();
                    write_value(clip, m.key, m.value0, &deltas);
                }
                if let Some(mut chan) = cache.lane_times(m.key.lane) {
                    chan.set_time(m.key.index, (m.time0 + dt).clamp(0.0, dur));
                }
            }
            cache.dirty = true;
            world.resource_mut::<GraphDrag>().gesture = Some(Gesture::Keys { start, moved, clicked, keys });
            return;
        }
    }

    // ── A new press ──
    if !just_pressed {
        return;
    }
    let Some(p) = cursor(world, true) else { return };
    if world.get_resource::<GraphDrag>().is_some_and(|g| g.pan.is_some()) {
        return;
    }
    let sel = world.resource::<CurveSel>();
    if let Some((pick, out)) = pick_handle(&set, sel, &frame, zoom, scroll, p) {
        freeze_view(world, &set, &frame);
        world.resource_mut::<GraphDrag>().gesture = Some(Gesture::Handle { pick, out });
        return;
    }
    let Some(pick) = pick_key(&set, &frame, zoom, scroll, p) else {
        world.resource_mut::<GraphDrag>().gesture = Some(Gesture::Press { at: p, shift });
        return;
    };
    let mut picks = world.resource::<CurveSel>().picks.clone();
    if shift {
        // Shift-click toggles the key in the selection, without dragging.
        match picks.iter().position(|q| *q == pick) {
            Some(i) => {
                picks.remove(i);
            }
            None => picks.push(pick),
        }
        select(world, picks, None);
        return;
    }
    if !picks.contains(&pick) {
        picks = vec![pick];
    }
    select(world, picks.clone(), Some(pick.key));
    freeze_view(world, &set, &frame);
    // The playhead to the key, so its pose shows while it is reshaped.
    let time = set.channels.iter().find(|c| c.lane == pick.key.lane && c.comp == pick.comp).and_then(|c| c.keys.get(pick.key.index)).map(|k| k.0);
    if let (Some(t), Some(bridge)) = (time, world.get_resource::<AnimEditorBridge>()) {
        push(bridge, AnimEditorAction::SetScrubTime(t));
    }
    let Some(clip) = world.get_resource::<TimelineClip>().and_then(|c| c.clip.as_ref()) else { return };
    let mut moving: Vec<Moving> = Vec::new();
    for p in &picks {
        if let Some(m) = moving.iter_mut().find(|m| m.key == p.key) {
            m.comps.push(p.comp);
            continue;
        }
        let time0 = super::snapshots::lane_key_times(clip, p.key.lane).get(p.key.index).copied();
        if let (Some(time0), Some(value0)) = (time0, raw_value(clip, p.key)) {
            moving.push(Moving { key: p.key, time0, value0, comps: vec![p.comp] });
        }
    }
    world.resource_mut::<GraphDrag>().gesture = Some(Gesture::Keys { start: p, moved: false, clicked: pick, keys: moving });
}

/// A moved group is unsorted until the drag ends: sort each touched lane,
/// then follow every moved key (and its picked curves) to its new index.
fn finish_move(world: &mut World, keys: &[Moving]) {
    let landed: Vec<(SelKey, f32)> = {
        let mut cache = world.resource_mut::<TimelineClip>();
        let landed: Vec<(SelKey, f32)> = keys.iter().filter_map(|m| Some((m.key, cache.lane_times(m.key.lane)?.time(m.key.index)?))).collect();
        let mut lanes: Vec<Lane> = keys.iter().map(|m| m.key.lane).collect();
        lanes.dedup();
        for lane in lanes {
            if let Some(mut chan) = cache.lane_times(lane) {
                chan.sort();
            }
        }
        landed
    };
    let cache = world.resource::<TimelineClip>();
    let mut remap: Vec<(SelKey, SelKey)> = Vec::new();
    for (k, t) in &landed {
        let times = cache.key_times(k.lane);
        let found = times.iter().enumerate().find(|(i, kt)| (**kt - t).abs() < 1e-6 && !remap.iter().any(|(_, n)| *n == SelKey { lane: k.lane, index: *i }));
        if let Some((index, _)) = found {
            remap.push((*k, SelKey { lane: k.lane, index }));
        }
    }
    let picks: Vec<CurvePick> = world
        .resource::<CurveSel>()
        .picks
        .iter()
        .map(|p| CurvePick { key: remap.iter().find(|(o, _)| *o == p.key).map(|(_, n)| *n).unwrap_or(p.key), comp: p.comp })
        .collect();
    let primary = world.resource::<SelectedKey>().0.map(|k| remap.iter().find(|(o, _)| *o == k).map(|(_, n)| *n).unwrap_or(k));
    select(world, picks, primary);
}

/// Middle-drag anywhere, or right-drag from empty graph, pans the view in time
/// and value; the wheel zooms the value axis about the cursor (Ctrl+wheel
/// still zooms time, Shift+wheel still pans it: [`super::edit::timeline_wheel`]).
pub(super) fn graph_pan(world: &mut World) {
    if !world.get_resource::<TimelineMode>().is_some_and(|m| m.curves) {
        world.resource_mut::<GraphDrag>().pan = None;
        return;
    }
    let buttons = world.resource::<ButtonInput<MouseButton>>();
    let right_press = buttons.just_pressed(MouseButton::Right);
    let button = if buttons.just_pressed(MouseButton::Middle) {
        Some(MouseButton::Middle)
    } else if right_press {
        Some(MouseButton::Right)
    } else {
        None
    };
    let held = [MouseButton::Middle, MouseButton::Right].map(|b| (b, buttons.pressed(b)));
    let held = move |b: MouseButton| held.iter().any(|(x, on)| *x == b && *on);
    if right_press {
        world.resource_mut::<GraphDrag>().pan_dragged = false;
    }
    let Some(set) = current_set(world) else { return };
    let h = world.get_resource::<TrackLayout>().map(|l| l.view.y).unwrap_or(60.0);
    let frame = frame_of(world, &set, h);
    let Some(state) = world.get_resource::<AnimationEditorState>() else { return };
    let (zoom, scroll) = (state.timeline_zoom.max(1.0), state.timeline_scroll);

    let pan = world.resource_mut::<GraphDrag>().pan.take();
    if let Some(pan) = pan {
        if !held(pan.button) {
            world.resource_mut::<GraphDrag>().pan_dragged = pan.moved;
            return;
        }
        let p = cursor(world, false).unwrap_or(pan.start);
        let d = p - pan.start;
        let moved = pan.moved || d.length() > 4.0;
        if moved {
            if let Some(bridge) = world.get_resource::<AnimEditorBridge>() {
                push(bridge, AnimEditorAction::SetTimelineScroll((pan.scroll0 - d.x / zoom).max(0.0)));
            }
            let dv = d.y / frame.py();
            let mut v = world.resource_mut::<CurveView>();
            v.subject = Some(set.subject);
            v.range = Some((pan.range0.0 + dv, pan.range0.1 + dv));
        }
        world.resource_mut::<GraphDrag>().pan = Some(Pan { moved, ..pan });
        return;
    }
    let Some(button) = button else { return };
    let Some(p) = cursor(world, true) else { return };
    // A right press on a key is its menu, not a pan.
    if button == MouseButton::Right && pick_key(&set, &frame, zoom, scroll, p).is_some() {
        return;
    }
    world.resource_mut::<GraphDrag>().pan = Some(Pan { button, start: p, scroll0: scroll, range0: (frame.vmin, frame.vmax), moved: false });
}

/// Gather this frame's wheel for [`graph_wheel`], which needs the whole world
/// to frame the curves and so cannot read messages itself.
pub(super) fn graph_wheel_read(mut wheel: MessageReader<MouseWheel>, mut drag: ResMut<GraphDrag>) {
    let dy: f32 = wheel
        .read()
        .map(|ev| match ev.unit {
            MouseScrollUnit::Line => ev.y,
            MouseScrollUnit::Pixel => ev.y / 40.0,
        })
        .sum();
    if dy != 0.0 {
        drag.wheel += dy;
    }
}

/// The wheel over the Curves view, without a modifier: zoom the value axis
/// about the cursor.
pub(super) fn graph_wheel(world: &mut World) {
    let dy = std::mem::take(&mut world.resource_mut::<GraphDrag>().wheel);
    if dy == 0.0 || !world.get_resource::<TimelineMode>().is_some_and(|m| m.curves) {
        return;
    }
    let keys = world.resource::<ButtonInput<KeyCode>>();
    if keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::SuperLeft, KeyCode::SuperRight, KeyCode::ShiftLeft, KeyCode::ShiftRight, KeyCode::AltLeft, KeyCode::AltRight]) {
        return;
    }
    let Some(p) = cursor(world, true) else { return };
    let Some(set) = current_set(world) else { return };
    let h = world.get_resource::<TrackLayout>().map(|l| l.view.y).unwrap_or(60.0);
    let frame = frame_of(world, &set, h);
    let at = frame.v_of(p.y);
    let f = 0.87f32.powf(dy);
    let (lo, hi) = (at - (at - frame.vmin) * f, at + (frame.vmax - at) * f);
    if hi - lo < 1e-4 || hi - lo > 1e7 {
        return;
    }
    let mut v = world.resource_mut::<CurveView>();
    v.subject = Some(set.subject);
    v.range = Some((lo, hi));
}

/// The Curves toolbar's Frame selected and Frame all, left as requests for
/// [`graph_sync`].
pub(super) fn graph_buttons(
    sel: Query<&UiInteraction, (With<FrameSelBtn>, Changed<UiInteraction>)>,
    all: Query<(&UiInteraction, &AnimBtn), Changed<UiInteraction>>,
    mut drag: ResMut<GraphDrag>,
) {
    if sel.iter().any(|i| *i == UiInteraction::Pressed) {
        drag.frame_request = Some(FrameRequest::Selected);
    }
    if all.iter().any(|(i, b)| *i == UiInteraction::Pressed && matches!(b, AnimBtn::FrameAll)) {
        drag.frame_request = Some(FrameRequest::All);
    }
}

/// Keep the Curves view's state in step: reframe on a new subject or a frame
/// request (F frames the selection), and keep [`CurveSel`] in step with the
/// dopesheet's selection, which the key card, Delete and a box in the Keys
/// view also change.
pub(super) fn graph_sync(world: &mut World) {
    if !world.get_resource::<TimelineMode>().is_some_and(|m| m.curves) {
        return;
    }
    let Some(set) = current_set(world) else { return };
    // F over the timeline frames the selection, as in a viewport.
    let f_key = world.resource::<ButtonInput<KeyCode>>().just_pressed(KeyCode::KeyF)
        && !world.get_resource::<renzora::InputFocusState>().is_some_and(|f| f.ui_wants_keyboard)
        && {
            let mut q = world.query_filtered::<&RelativeCursorPosition, With<AnimTimeline>>();
            q.iter(world).any(|r| r.cursor_over)
        };
    let mut request = world.resource_mut::<GraphDrag>().frame_request.take();
    if f_key {
        request = Some(FrameRequest::Selected);
    }
    if world.resource::<CurveView>().subject != Some(set.subject) {
        let mut v = world.resource_mut::<CurveView>();
        v.subject = Some(set.subject);
        v.range = None;
    }

    // The selection: drop picks whose key is no longer selected, and give a
    // key selected elsewhere every curve it shows.
    let keys = super::keymode::selection(world);
    let mut picks: Vec<CurvePick> = world.resource::<CurveSel>().picks.iter().copied().filter(|p| keys.contains(&p.key)).collect();
    for k in &keys {
        if !picks.iter().any(|p| p.key == *k) {
            picks.extend(set.channels.iter().filter(|c| c.lane == k.lane && k.index < c.keys.len()).map(|c| CurvePick { key: *k, comp: c.comp }));
        }
    }
    world.resource_mut::<CurveSel>().set(picks.clone());

    let Some(request) = request else { return };
    let selected: Vec<(f32, f32)> = picks
        .iter()
        .filter_map(|p| set.channels.iter().find(|c| c.lane == p.key.lane && c.comp == p.comp)?.keys.get(p.key.index).copied())
        .collect();
    let view_w = world.get_resource::<TrackLayout>().map(|l| l.view.x).unwrap_or(400.0);
    let dur = world.get_resource::<TimelineClip>().and_then(|c| c.clip.as_ref()).map(|c| c.duration).unwrap_or(2.0);
    let (range, zoom, scroll) = if request == FrameRequest::Selected && !selected.is_empty() {
        let (mut lo, mut hi) = span_of(selected.iter().map(|k| k.1));
        // One key, or keys at one value: a sliver of the whole curve's
        // height around them rather than a window a degree tall.
        let min_h = (set.vmax - set.vmin) * 0.25;
        if hi - lo < min_h {
            let mid = (lo + hi) * 0.5;
            (lo, hi) = (mid - min_h * 0.5, mid + min_h * 0.5);
        }
        let t0 = selected.iter().map(|k| k.0).fold(f32::INFINITY, f32::min);
        let t1 = selected.iter().map(|k| k.0).fold(f32::NEG_INFINITY, f32::max);
        let span = (t1 - t0).max(0.25);
        let zoom = fit_zoom(span * 1.3, view_w);
        let scroll = ((t0 + t1) * 0.5 - (view_w - LANE_INSET * 2.0) * 0.5 / zoom).max(0.0);
        (Some((lo, hi)), zoom, scroll)
    } else {
        (None, fit_zoom(dur, view_w), 0.0)
    };
    world.resource_mut::<CurveView>().range = range;
    if let Some(bridge) = world.get_resource::<AnimEditorBridge>() {
        push(bridge, AnimEditorAction::SetTimelineZoom(zoom));
        push(bridge, AnimEditorAction::SetTimelineScroll(scroll));
    }
}

/// Right-click in the Curves view (on release, so a right-drag can pan): on a
/// key, its interpolation and Delete; on empty graph, the frame commands.
pub(super) fn graph_context_menu(world: &mut World) {
    if !world.get_resource::<TimelineMode>().is_some_and(|m| m.curves) {
        return;
    }
    if !world.resource::<ButtonInput<MouseButton>>().just_released(MouseButton::Right) || world.resource::<GraphDrag>().pan_dragged {
        return;
    }
    let Some(p) = cursor(world, true) else { return };
    let Some(at) = ({
        let mut q = world.query::<&Window>();
        q.iter(world).find_map(|w| w.cursor_position())
    }) else {
        return;
    };
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
    let Some(set) = current_set(world) else { return };
    let h = world.get_resource::<TrackLayout>().map(|l| l.view.y).unwrap_or(60.0);
    let frame = frame_of(world, &set, h);
    let Some(state) = world.get_resource::<AnimationEditorState>() else { return };
    let (zoom, scroll) = (state.timeline_zoom.max(1.0), state.timeline_scroll);
    let picked = pick_key(&set, &frame, zoom, scroll, p);
    let targets = picked.map(|k| super::keymode::menu_targets(world, k.key));
    let current = picked.and_then(|k| world.get_resource::<TimelineClip>()?.clip.as_ref().and_then(|c| super::keymode::key_mode(c, k.key)));

    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        let menu = screen_menu(&mut commands, at.x, at.y);
        let mut kids = Vec::new();
        if let Some(targets) = targets {
            kids.extend(super::keymode::mode_menu_items(&mut commands, &fonts, targets.clone(), current));
            kids.push(menu_item(&mut commands, &fonts, "trash", &renzora::lang::t("timeline.delete_keyframe"), move |w| {
                w.resource_mut::<SelectedKeys>().set(targets.clone());
                w.resource_mut::<SelectedKey>().0 = None;
                super::props::delete_selected_keys(w);
            }));
        } else {
            kids.push(menu_item(&mut commands, &fonts, "frame-corners", &renzora::lang::t_or("timeline.frame_selected", "Frame selected"), |w| {
                w.resource_mut::<GraphDrag>().frame_request = Some(FrameRequest::Selected);
            }));
            kids.push(menu_item(&mut commands, &fonts, "arrows-out", &renzora::lang::t_or("timeline.frame_all", "Frame all"), |w| {
                w.resource_mut::<GraphDrag>().frame_request = Some(FrameRequest::All);
            }));
        }
        commands.entity(menu).add_children(&kids);
    }
    queue.apply(world);
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora::{BoneTrack, KeyCurve};

    fn clip() -> AnimClip {
        let mut c = renzora_animation::extract::create_empty_clip("c", 2.0);
        let mut t = BoneTrack::new("arm");
        t.translations = vec![(0.0, [0.0; 3]), (1.0, [1.0, 0.0, 0.0]), (2.0, [0.0; 3])];
        t.rotations = vec![(0.0, Quat::IDENTITY.to_array()), (1.0, Quat::from_rotation_y(0.5).to_array())];
        for i in 0..3 {
            t.set_key_curve(0, i, KeyCurve::AUTO);
        }
        t.set_key_curve(1, 0, KeyCurve::AUTO);
        t.set_key_curve(1, 1, KeyCurve::AUTO);
        c.tracks.push(t);
        c
    }

    fn pick(channel: u8, index: usize, comp: usize) -> CurvePick {
        CurvePick { key: SelKey { lane: Lane::Bone { track: 0, channel }, index }, comp }
    }

    #[test]
    fn dragging_a_handle_makes_the_key_free_and_sets_its_slope() {
        let mut c = clip();
        set_handle(&mut c, pick(0, 1, 0), true, 3.0, false);
        let k = c.tracks[0].key_curve(0, 1);
        assert_eq!(k.mode, KeyMode::Free);
        assert_eq!((k.in_tan[0], k.out_tan[0]), (3.0, 3.0));
        // Alt breaks it: only the dragged side moves.
        set_handle(&mut c, pick(0, 1, 0), false, -2.0, true);
        let k = c.tracks[0].key_curve(0, 1);
        assert_eq!(k.mode, KeyMode::Broken);
        assert_eq!((k.in_tan[0], k.out_tan[0]), (-2.0, 3.0));
    }

    #[test]
    fn a_rotation_handle_is_set_in_euler_degrees() {
        let mut c = clip();
        set_handle(&mut c, pick(1, 0, 1), true, 90.0, false);
        let t = &c.tracks[0];
        let q = t.channel_keys(1)[0].1;
        let e = euler_slope_of_quat_tangent(q, Vec4::from_array(t.key_curve(1, 0).out_tan));
        assert!((e.y - 90.0).abs() < 0.5, "{e:?}");
        assert!(e.x.abs() < 0.5 && e.z.abs() < 0.5, "{e:?}");
    }

    #[test]
    fn a_rotation_value_drag_turns_about_one_euler_axis() {
        let mut c = clip();
        let k = SelKey { lane: Lane::Bone { track: 0, channel: 1 }, index: 0 };
        let v0 = raw_value(&c, k).unwrap();
        write_value(&mut c, k, v0, &[(1, 30.0)]);
        let e = quat_to_euler_deg(Quat::from_array(c.tracks[0].rotations[0].1));
        assert!((e.y - 30.0).abs() < 1e-3 && e.x.abs() < 1e-3, "{e:?}");
    }

    #[test]
    fn a_position_value_drag_moves_only_its_component() {
        let mut c = clip();
        let k = SelKey { lane: Lane::Bone { track: 0, channel: 0 }, index: 1 };
        let v0 = raw_value(&c, k).unwrap();
        write_value(&mut c, k, v0, &[(2, -0.5)]);
        assert_eq!(c.tracks[0].translations[1].1, [1.0, 0.0, -0.5]);
    }
}
