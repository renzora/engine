//! Picking, dragging, box-select, scrolling and the keyboard: everything the
//! pointer and the keys do to the dopesheet.
//!
//! Keyframe picking is done by **math against the clip data** (cursor →
//! time/row), not by per-diamond `UiInteraction`: the timeline widget's scrub
//! overlay sits above the lanes and would swallow per-node hits. A drag is
//! likewise tracked against the raw mouse button, because the keyed list rebuilds
//! the dragged diamond the moment its time changes.
//!
//! A press on empty lane space no longer scrubs: dragging there draws a
//! selection box, and a plain click moves the playhead to where you clicked.
//! Scrubbing belongs to the ruler, where it always was in practice; letting the
//! whole lane area scrub made box-select impossible.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora::AnimMarker;
use renzora_animation::AnimatorComponent;
use renzora_editor_framework::EditorCommands;
use renzora_ember::font::EmberFonts;
use renzora_ember::widgets::{menu_item, screen_menu, EmberTextInput, TimelineView, LANE_INSET};
use renzora_ember::UiInteraction;

use crate::{AnimEditorAction, AnimEditorBridge, AnimationEditorState};

use super::clip::{clip_entity, Lane, SelKey, SelectedKey, SelectedKeys, TimelineClip};
use super::curves::TimelineMode;
use super::layout::{RowKind, TrackLayout};
use super::popover::PopoverState;
use super::props::{add_key_at, interp_label_tr, interp_menu_choices, set_key_interp, set_key_to_live, TimelineOps};
use super::runs::{run_of, KeyRuns, Run};
use super::snapshots::{key_drawn, lane_y, summary_diamonds, x_of};
use super::{
    AddMarkerBtn, AnimBtn, AnimPlayIcon, AnimTimeline, BoxRect, CollapseBtn, HeaderListOffset,
    KeyLane, MarkerNameField, NewClipBtn, NewClipNameField, PaneLayer, TrackRowBtn, ROW_DEFAULT,
    ROW_MAX, ROW_MIN, ZOOM_MAX, ZOOM_MIN,
};

pub(super) fn push(bridge: &AnimEditorBridge, action: AnimEditorAction) {
    if let Ok(mut p) = bridge.pending.lock() {
        p.push(action);
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn anim_btn_click(
    q: Query<(&UiInteraction, &AnimBtn), Changed<UiInteraction>>,
    state: Option<Res<AnimationEditorState>>,
    cache: Option<Res<TimelineClip>>,
    runs: Res<KeyRuns>,
    layout: Res<TrackLayout>,
    bridge: Option<Res<AnimEditorBridge>>,
    mut ops: ResMut<TimelineOps>,
) {
    let (Some(state), Some(bridge)) = (state, bridge) else { return };
    let dur = cache.and_then(|c| c.clip.as_ref().map(|c| c.duration)).unwrap_or(2.0);
    for (interaction, btn) in &q {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        let action = match btn {
            // To the loop region's start first, then (pressed again there) to
            // the clip's start, so a moved start handle never hides frame 0.
            AnimBtn::SkipBack => {
                let (start, _) = state.loop_range(dur);
                AnimEditorAction::SetScrubTime(if state.scrub_time > start + 1e-4 { start } else { 0.0 })
            }
            AnimBtn::PrevKey => AnimEditorAction::SetScrubTime(prev_key(&runs, state.scrub_time).unwrap_or(0.0)),
            AnimBtn::PlayPause => AnimEditorAction::TogglePreview,
            AnimBtn::NextKey => AnimEditorAction::SetScrubTime(next_key(&runs, state.scrub_time).unwrap_or(dur).min(dur)),
            // The mirror of SkipBack: loop end, then the clip's end.
            AnimBtn::SkipForward => {
                let (_, end) = state.loop_range(dur);
                AnimEditorAction::SetScrubTime(if state.scrub_time < end - 1e-4 { end } else { dur })
            }
            AnimBtn::Loop => AnimEditorAction::SetPreviewLooping(!state.preview_looping),
            AnimBtn::Snap => AnimEditorAction::SetSnapEnabled(!state.snap_enabled),
            AnimBtn::ZoomIn => AnimEditorAction::SetTimelineZoom((state.timeline_zoom * 1.25).min(ZOOM_MAX)),
            AnimBtn::ZoomOut => AnimEditorAction::SetTimelineZoom((state.timeline_zoom * 0.8).max(ZOOM_MIN)),
            AnimBtn::FrameAll => {
                push(&bridge, AnimEditorAction::SetTimelineScroll(0.0));
                AnimEditorAction::SetTimelineZoom(fit_zoom(dur, layout.view.x))
            }
            AnimBtn::Record => AnimEditorAction::SetRecordEnabled(!state.record_enabled),
            AnimBtn::AddProperty => {
                ops.add_empty_track = true;
                continue;
            }
            AnimBtn::AddKey => {
                ops.add_key = true;
                continue;
            }
        };
        push(&bridge, action);
    }
}

/// The zoom that spans the whole clip across a lane area `view_w` px wide.
pub(super) fn fit_zoom(dur: f32, view_w: f32) -> f32 {
    let width = (view_w - LANE_INSET * 2.0).max(100.0);
    (width / dur.max(0.1)).clamp(ZOOM_MIN, ZOOM_MAX)
}

/// Frame a freshly selected clip so it fills the lanes, once per selection
/// (`auto_fit_clip` is cleared whenever the clip or entity changes). Waits for
/// the clip to load and the lane area to be laid out.
pub(super) fn auto_fit_clip(
    state: Option<Res<AnimationEditorState>>,
    cache: Res<TimelineClip>,
    layout: Res<TrackLayout>,
    bridge: Option<Res<AnimEditorBridge>>,
) {
    let (Some(state), Some(bridge)) = (state, bridge) else { return };
    let (Some(clip), Some((entity, name))) = (cache.clip.as_ref(), cache.key.as_ref()) else { return };
    // Keyed by entity too, so the same clip name on another entity refits.
    let id = format!("{}#{name}", entity.to_bits());
    if state.auto_fit_clip.as_deref() == Some(id.as_str()) || layout.view.x <= LANE_INSET * 2.0 + 1.0 {
        return;
    }
    push(&bridge, AnimEditorAction::SetTimelineScroll(0.0));
    push(&bridge, AnimEditorAction::SetTimelineZoom(fit_zoom(clip.duration, layout.view.x)));
    push(&bridge, AnimEditorAction::AutoFitDone(id));
}

/// The nearest key time strictly before `t`.
pub(super) fn prev_key(runs: &KeyRuns, t: f32) -> Option<f32> {
    let i = runs.times.partition_point(|k| *k < t - 1e-4);
    i.checked_sub(1).map(|i| runs.times[i])
}

/// The nearest key time strictly after `t`.
pub(super) fn next_key(runs: &KeyRuns, t: f32) -> Option<f32> {
    let i = runs.times.partition_point(|k| *k <= t + 1e-4);
    runs.times.get(i).copied()
}

/// Push the panel's geometry into the shared timeline widget.
///
/// The widget's own scrub surface is drained and **discarded**: every scrub
/// here is started by [`ruler_drag`] (a press in the ruler or on the playhead
/// needle), which decides at the press what the gesture is. The surface could
/// not: it lies under the scrollbars, the handles and the lanes alike, and a
/// press passes through to it from all of them, so a scrollbar drag reported a
/// scrub every frame and only a frame-counted mute stood between that and the
/// playhead. Dragging a scrollbar dragged the playhead with it; ignoring the
/// surface closes that path outright instead of racing it.
pub(super) fn anim_sync(
    mut q: Query<&mut TimelineView, With<AnimTimeline>>,
    state: Option<Res<AnimationEditorState>>,
    cache: Option<Res<TimelineClip>>,
    layout: Res<TrackLayout>,
) {
    let Some(state) = state else { return };
    let clip = cache.as_ref().and_then(|c| c.clip.as_ref());
    let dur = clip.map(|c| c.duration).unwrap_or(2.0);
    for mut v in &mut q {
        v.set_geom(state.timeline_zoom, state.timeline_scroll, state.scrub_time, dur, state.track_height, layout.rows.len());
        if v.vscroll != layout.vscroll {
            v.vscroll = layout.vscroll;
        }
        let _ = v.take_scrub();
    }
}

pub(super) fn update_anim_play_icon(
    state: Option<Res<AnimationEditorState>>,
    mut q: Query<&mut Text, With<AnimPlayIcon>>,
) {
    let Some(state) = state else { return };
    let glyph = renzora_ember::font::icon_glyph(if state.is_previewing { "pause" } else { "play" });
    if let Some(g) = glyph {
        let s = g.to_string();
        for mut t in &mut q {
            if t.0 != s {
                t.0 = s.clone();
            }
        }
    }
}

// ── Picking ──────────────────────────────────────────────────────────────────

/// Cursor position in the lanes layer's pixel space, or `None` when outside.
pub(super) fn lane_cursor(lane: &Query<(&RelativeCursorPosition, &ComputedNode), With<KeyLane>>) -> Option<Vec2> {
    let (rcp, cn) = lane.iter().next()?;
    if !rcp.cursor_over {
        return None;
    }
    let n = rcp.normalized?;
    let size = cn.size() * cn.inverse_scale_factor();
    Some((n + Vec2::splat(0.5)) * size)
}

/// Like [`lane_cursor`] but unclamped to "over": the position keeps tracking
/// while a drag strays outside the lanes.
fn lane_cursor_any(lane: &Query<(&RelativeCursorPosition, &ComputedNode), With<KeyLane>>) -> Option<Vec2> {
    let (rcp, cn) = lane.iter().next()?;
    let n = rcp.normalized?;
    let size = cn.size() * cn.inverse_scale_factor();
    Some((n + Vec2::splat(0.5)) * size)
}

pub(super) struct PickedKey {
    pub(super) key: SelKey,
    pub(super) time: f32,
}

/// The runs of one lane.
pub(super) fn lane_runs(runs: &KeyRuns, lane: Lane) -> &[Run] {
    match lane {
        Lane::Bone { track, channel } => runs.bone.get(track).map(|c| c[channel as usize].as_slice()).unwrap_or(&[]),
        Lane::Prop { track } => runs.prop.get(track).map(|v| v.as_slice()).unwrap_or(&[]),
        Lane::Morph { track } => runs.morph.get(track).map(|v| v.as_slice()).unwrap_or(&[]),
        Lane::Fx { track } => runs.fx.get(track).map(|v| v.as_slice()).unwrap_or(&[]),
    }
}

/// The lanes a row holds, with their channel slot. An emitter row's lane is
/// its `Enabled` track, once it has one; the group head has none.
pub(super) fn row_lanes(kind: RowKind) -> Vec<(Lane, Option<u8>)> {
    match kind {
        RowKind::Summary | RowKind::FxGroup | RowKind::FxEmitter(_, None) => Vec::new(),
        RowKind::Bone(track) => (0..3u8).map(|ch| (Lane::Bone { track, channel: ch }, Some(ch))).collect(),
        RowKind::Prop(track) => vec![(Lane::Prop { track }, None)],
        RowKind::Morph(track) => vec![(Lane::Morph { track }, None)],
        RowKind::FxEmitter(_, Some(track)) | RowKind::FxTrack(track) => vec![(Lane::Fx { track }, None)],
    }
}

/// Find the drawn keyframe nearest to a lane-space point. A key hidden inside
/// a run bar is not pickable on its own (zoom in, or box-select it).
pub(super) fn pick_key(
    cache: &TimelineClip,
    runs: &KeyRuns,
    layout: &TrackLayout,
    zoom: f32,
    scroll: f32,
    th: f32,
    p: Vec2,
) -> Option<PickedKey> {
    let radius = (th * 0.30).clamp(5.0, 9.0);
    let zoom = zoom.max(1.0);
    let row = layout.row_at(p.y, th)?;
    let mut best: Option<(f32, PickedKey)> = None;
    for r in row.saturating_sub(1)..(row + 2).min(layout.rows.len()) {
        for (lane, ch) in row_lanes(layout.rows[r].kind) {
            let y = lane_y(layout, r, ch, th);
            let dy = (p.y - y).abs();
            if dy > radius {
                continue;
            }
            let times = cache.key_times(lane);
            let lruns = lane_runs(runs, lane);
            let t = (p.x - LANE_INSET) / zoom + scroll;
            let lo = times.partition_point(|k| *k < t - radius / zoom);
            let hi = times.partition_point(|k| *k <= t + radius / zoom);
            for (idx, &kt) in times.iter().enumerate().take(hi).skip(lo) {
                let drawn = run_of(lruns, idx).is_none_or(|run| key_drawn(run, idx, zoom));
                if !drawn {
                    continue;
                }
                let dx = (x_of(kt, zoom, scroll) - p.x).abs();
                let score = dx.max(dy);
                if best.as_ref().is_none_or(|(s, _)| score < *s) {
                    best = Some((score, PickedKey { key: SelKey { lane, index: idx }, time: kt }));
                }
            }
        }
    }
    best.map(|(_, k)| k)
}

/// The summary ("All tracks") diamond nearest a lane-space point, as the key
/// time it stands for. Picks only among the diamonds actually drawn (see
/// [`summary_diamonds`]), so what the cursor offers is what the eye sees.
pub(super) fn pick_summary(runs: &KeyRuns, layout: &TrackLayout, zoom: f32, scroll: f32, th: f32, p: Vec2) -> Option<f32> {
    let radius = (th * 0.30).clamp(5.0, 9.0);
    let zoom = zoom.max(1.0);
    let row = layout.row_at(p.y, th)?;
    if layout.rows[row].kind != RowKind::Summary || (p.y - lane_y(layout, row, None, th)).abs() > radius {
        return None;
    }
    summary_diamonds(&runs.times, zoom)
        .into_iter()
        .map(|t| ((x_of(t, zoom, scroll) - p.x).abs(), t))
        .filter(|(dx, _)| *dx <= radius)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, t)| t)
}

/// Every key, on any track, sitting at `time`: what dragging a summary
/// diamond moves together.
fn keys_at_time(cache: &TimelineClip, time: f32) -> Vec<SelKey> {
    let mut out = Vec::new();
    let Some(clip) = cache.clip.as_ref() else { return out };
    let fx_tracks = cache.fx.as_ref().filter(|f| f.editable).map(|f| f.doc.tracks.len()).unwrap_or(0);
    let lanes = (0..clip.tracks.len())
        .flat_map(|track| (0..3u8).map(move |channel| Lane::Bone { track, channel }))
        .chain((0..clip.property_tracks.len()).map(|track| Lane::Prop { track }))
        .chain((0..clip.morph_tracks.len()).map(|track| Lane::Morph { track }))
        .chain((0..fx_tracks).map(|track| Lane::Fx { track }));
    for lane in lanes {
        for (index, t) in cache.key_times(lane).into_iter().enumerate() {
            if (t - time).abs() < 1e-4 {
                out.push(SelKey { lane, index });
            }
        }
    }
    out
}

/// Every key inside a lane-space rectangle: what a box-select picks. Keys
/// hidden in run bars are included, so a box over a baked stretch selects it.
fn keys_in_box(
    cache: &TimelineClip,
    layout: &TrackLayout,
    zoom: f32,
    scroll: f32,
    th: f32,
    a: Vec2,
    b: Vec2,
) -> Vec<SelKey> {
    let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
    let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
    let zoom = zoom.max(1.0);
    let (t0, t1) = ((x0 - LANE_INSET) / zoom + scroll, (x1 - LANE_INSET) / zoom + scroll);
    let mut out = Vec::new();
    let first = layout.row_at(y0.max(0.0), th).unwrap_or(0);
    let last = layout.row_at(y1, th).unwrap_or(layout.rows.len().saturating_sub(1));
    for r in first..=last.min(layout.rows.len().saturating_sub(1)) {
        for (lane, ch) in row_lanes(layout.rows[r].kind) {
            let y = lane_y(layout, r, ch, th);
            if y < y0 - 3.0 || y > y1 + 3.0 {
                continue;
            }
            let times = cache.key_times(lane);
            let lo = times.partition_point(|k| *k < t0);
            let hi = times.partition_point(|k| *k <= t1);
            out.extend((lo..hi).map(|index| SelKey { lane, index }));
        }
    }
    out
}

/// In-flight pointer gestures on the lanes and ruler. Survives the keyed-list
/// rebuilding the dragged node mid-drag (the rebuild drops `UiInteraction`
/// state, so gestures are tracked against the raw mouse button instead).
#[derive(Resource, Default)]
pub(crate) struct KeyDragState {
    pub(super) active: Option<KeyDrag>,
    /// A press on empty lane space: a click moves the playhead there, a drag
    /// draws the box in `boxed`.
    press: Option<Vec2>,
    pub(super) boxed: Option<(Vec2, Vec2)>,
    /// A gesture in the ruler (or on the playhead needle): see [`ruler_drag`].
    pub(super) ruler: Option<RulerGrab>,
    /// Pressed on (or dragging) one of the scrollbars.
    pub(super) scrollbar: bool,
    /// A middle- or right-drag panning the view; see [`timeline_pan`].
    pan: Option<Pan>,
    /// The last right-button gesture moved the view, so its release is not a
    /// click and must not open the context menu.
    pan_dragged: bool,
}

struct Pan {
    button: MouseButton,
    /// Lane-space cursor, time scroll and row scroll at the press.
    start: Vec2,
    scroll0: f32,
    vscroll0: f32,
    moved: bool,
}

/// What a left press in the ruler grabbed.
///
/// The ruler is split in two bands so the loop region and scrubbing can both
/// be dragged without a modifier (the DAW convention): the **upper half** is
/// the loop strip, where the blue band and its two handles live, and the
/// **lower half** scrubs and carries the clip-length marker. The length used
/// to be the band's right-hand handle, which made the loop region and the
/// clip's real length one control that people kept mistaking for each other.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum RulerGrab {
    /// The loop region's start handle.
    LoopStart,
    /// The loop region's end handle (the band's right edge).
    LoopEnd,
    /// The band itself, moved whole. `grab` is the cursor time at the press
    /// and `start`, `end` the region then, so the band keeps its length.
    Band { grab: f32, start: f32, end: f32 },
    /// The clip-length marker in the lower half: changes the clip's length.
    Length,
    /// The playhead. `offset` is the playhead's time minus the cursor's at the
    /// press, so taking the badge by its edge does not first jump the needle
    /// under the cursor; a press on bare ruler has none, and lands there.
    /// `needle` says the press took the playhead itself.
    Scrub { offset: f32, needle: bool },
}

/// How close, px, a press must land to a ruler handle to take it.
const HANDLE_HIT: f32 = 6.0;
/// How close, px, a press must land to the playhead needle to take it.
const NEEDLE_HIT: f32 = 4.0;
/// Half the playhead badge's width and its vertical span in the ruler, px
/// (see `snapshots::ruler_snapshot`, which draws it).
const BADGE_HALF_W: f32 = 20.0;
const BADGE_Y: (f32, f32) = (5.0, 20.0);

/// Where the ruler's loop strip ends and its scrub half begins.
pub(super) fn loop_strip_h(ruler_h: f32) -> f32 {
    (ruler_h * 0.5).round()
}

impl KeyDragState {
    fn owns_pointer(&self) -> bool {
        self.active.is_some() || self.press.is_some() || self.ruler.is_some() || self.scrollbar || self.pan.is_some()
    }
}

/// What a press at pane-space `p` inside a ruler `ruler_h` tall takes (see
/// [`RulerGrab`] for the two halves). Handles win over the playhead where they
/// overlap: a loop handle and the playhead both sit at t=0 on a fresh clip,
/// and a handle the playhead covered could never be picked up. The badge is
/// wider than a handle's hit zone, so the playhead stays reachable by its
/// sides, and it wins over the band so it can always be dragged.
pub(super) fn ruler_hit(p: Vec2, state: &AnimationEditorState, dur: f32, ruler_h: f32) -> RulerGrab {
    let (zoom, scroll) = (state.timeline_zoom.max(1.0), state.timeline_scroll);
    let (start, end) = state.loop_range(dur);
    let cursor_t = (p.x - LANE_INSET) / zoom + scroll;
    let upper = p.y < loop_strip_h(ruler_h);
    if upper {
        let de = (x_of(end, zoom, scroll) - p.x).abs();
        let ds = (x_of(start, zoom, scroll) - p.x).abs();
        if de <= HANDLE_HIT && de <= ds {
            return RulerGrab::LoopEnd;
        }
        if ds <= HANDLE_HIT {
            return RulerGrab::LoopStart;
        }
    } else if (x_of(dur, zoom, scroll) - p.x).abs() <= HANDLE_HIT {
        return RulerGrab::Length;
    }
    let dp = (x_of(state.scrub_time, zoom, scroll) - p.x).abs();
    let on_badge = dp <= BADGE_HALF_W && p.y >= BADGE_Y.0 && p.y <= BADGE_Y.1;
    let needle = dp <= NEEDLE_HIT || on_badge;
    // A band spanning the whole clip has nowhere to move, so there the strip
    // scrubs like the rest of the ruler instead of swallowing the press.
    let movable = end - start < dur - 1e-4;
    if !needle && upper && movable && cursor_t >= start && cursor_t <= end {
        return RulerGrab::Band { grab: cursor_t, start, end };
    }
    RulerGrab::Scrub { offset: if needle { state.scrub_time - cursor_t } else { 0.0 }, needle }
}

/// Middle-drag anywhere over the lanes, or right-drag from empty lane space,
/// pans the view in time and through the rows, the way a node graph or a
/// viewport pans. Right-drag starts only off a key, so right-clicking a key
/// still opens its menu; a right press that never moves is still a click and
/// opens the empty-space menu on release (see [`key_context_menu`]).
#[allow(clippy::too_many_arguments)]
pub(super) fn timeline_pan(
    mut drag: ResMut<KeyDragState>,
    buttons: Res<ButtonInput<MouseButton>>,
    lane: Query<(&RelativeCursorPosition, &ComputedNode), With<KeyLane>>,
    state: Option<Res<AnimationEditorState>>,
    runs: Res<KeyRuns>,
    mut layout: ResMut<TrackLayout>,
    cache: Res<TimelineClip>,
    bridge: Option<Res<AnimEditorBridge>>,
    mode: Res<TimelineMode>,
) {
    let (Some(state), Some(bridge)) = (state, bridge) else { return };
    let (zoom, scroll, th) = (state.timeline_zoom.max(1.0), state.timeline_scroll, state.track_height.max(1.0));
    if buttons.just_pressed(MouseButton::Right) {
        drag.pan_dragged = false;
    }
    // The Curves view pans time and value (`graph::graph_pan`), not rows.
    if mode.curves {
        drag.pan = None;
        return;
    }

    if let Some(pan) = drag.pan.as_mut() {
        if !buttons.pressed(pan.button) {
            let moved = pan.moved;
            drag.pan = None;
            drag.pan_dragged = moved;
            return;
        }
        let Some(p) = lane_cursor_any(&lane) else { return };
        let d = p - pan.start;
        if !pan.moved && d.length() > 4.0 {
            pan.moved = true;
        }
        if pan.moved {
            let dur = cache.clip.as_ref().map(|c| c.duration).unwrap_or(2.0);
            let (scroll0, vscroll0) = (pan.scroll0, pan.vscroll0);
            let s = clamp_hscroll(&state, &layout, dur, scroll0 - d.x / zoom);
            if (s - scroll).abs() > 1e-5 {
                push(&bridge, AnimEditorAction::SetTimelineScroll(s));
            }
            let v = (vscroll0 - d.y).clamp(0.0, layout.max_vscroll(th));
            if (v - layout.vscroll).abs() > 0.01 {
                layout.vscroll = v;
            }
        }
        return;
    }

    let button = if buttons.just_pressed(MouseButton::Middle) {
        MouseButton::Middle
    } else if buttons.just_pressed(MouseButton::Right) {
        MouseButton::Right
    } else {
        return;
    };
    if drag.owns_pointer() {
        return;
    }
    let Some(p) = lane_cursor(&lane) else { return };
    if button == MouseButton::Right && cache.clip.is_some() && pick_key(&cache, &runs, &layout, zoom, scroll, th, p).is_some() {
        return;
    }
    drag.pan = Some(Pan { button, start: p, scroll0: scroll, vscroll0: layout.vscroll, moved: false });
}

/// The cursor over the lanes says what a press there would do: a grab hand
/// over a key or the playhead (it drags), a resize arrow over either ruler
/// handle, a closed hand while dragging, scrubbing or panning, and the
/// ordinary pointer over empty space.
#[allow(clippy::too_many_arguments)]
pub(super) fn timeline_cursor(
    drag: Res<KeyDragState>,
    lane: Query<(&RelativeCursorPosition, &ComputedNode), With<KeyLane>>,
    pane: Query<(&RelativeCursorPosition, &ComputedNode), With<PaneLayer>>,
    theme: Option<Res<renzora_ember::style::Theme>>,
    state: Option<Res<AnimationEditorState>>,
    runs: Res<KeyRuns>,
    layout: Res<TrackLayout>,
    cache: Res<TimelineClip>,
    mode: Res<TimelineMode>,
    mut scrub: Query<&mut renzora_ember::cursor_icon::HoverCursor, With<renzora_ember::widgets::TlScrub>>,
) {
    use bevy::window::SystemCursorIcon as C;
    let Some(state) = state else { return };
    let (zoom, scroll, th) = (state.timeline_zoom.max(1.0), state.timeline_scroll, state.track_height.max(1.0));
    let ruler_h = theme.map(|t| t.timeline.ruler_height).unwrap_or(28.0);
    let want = if drag.active.is_some() || drag.pan.as_ref().is_some_and(|p| p.moved) {
        C::Grabbing
    } else if let Some(grab) = drag.ruler {
        match grab {
            RulerGrab::LoopStart | RulerGrab::LoopEnd | RulerGrab::Length => C::EwResize,
            RulerGrab::Band { .. } | RulerGrab::Scrub { .. } => C::Grabbing,
        }
    } else {
        // Over the ruler: what a press there would take.
        let in_ruler = pane.iter().next().and_then(|(rcp, cn)| {
            let n = rcp.normalized.filter(|_| rcp.cursor_over)?;
            let p = (n + Vec2::splat(0.5)) * (cn.size() * cn.inverse_scale_factor());
            let dur = cache.clip.as_ref()?.duration;
            (p.y <= ruler_h).then(|| ruler_hit(p, &state, dur, ruler_h))
        });
        let on_key = !mode.curves
            && cache.clip.is_some()
            && lane_cursor(&lane).is_some_and(|p| {
                pick_key(&cache, &runs, &layout, zoom, scroll, th, p).is_some() || pick_summary(&runs, &layout, zoom, scroll, th, p).is_some()
            });
        let on_needle = !on_key
            && !mode.curves
            && lane_cursor(&lane).is_some_and(|p| (x_of(state.scrub_time, zoom, scroll) - p.x).abs() <= NEEDLE_HIT);
        match in_ruler {
            Some(RulerGrab::LoopStart | RulerGrab::LoopEnd | RulerGrab::Length) => C::EwResize,
            Some(RulerGrab::Band { .. } | RulerGrab::Scrub { needle: true, .. }) => C::Grab,
            _ if on_key || on_needle => C::Grab,
            _ => C::Default,
        }
    };
    for mut hc in &mut scrub {
        if hc.0 != want {
            hc.0 = want;
        }
    }
}

/// The furthest the view may scroll in time: the clip's end plus half a view
/// of runout, so the end handle can be dragged out past it.
pub(super) fn max_hscroll(dur: f32, zoom: f32, view_w: f32) -> f32 {
    let vis = (view_w - LANE_INSET).max(1.0) / zoom.max(1.0);
    (dur + vis * 0.5 - vis).max(0.0)
}

pub(super) fn clamp_hscroll(state: &AnimationEditorState, layout: &TrackLayout, dur: f32, s: f32) -> f32 {
    s.clamp(0.0, max_hscroll(dur, state.timeline_zoom, layout.view.x))
}

pub(super) struct KeyDrag {
    /// Every key moving, with its time when the drag began. The pressed key is
    /// `keys[0]`.
    keys: Vec<(SelKey, f32)>,
    start_cursor_x: f32,
    moved: bool,
    /// Started on a summary diamond: a click keeps the whole time's keys
    /// selected instead of narrowing to one key and opening its popover.
    summary: bool,
}

/// Drag keys to retime them (snap-aware), box-select on empty space, and open
/// the key popover on a click.
#[allow(clippy::too_many_arguments)]
pub(super) fn key_drag(
    mut drag: ResMut<KeyDragState>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    lane: Query<(&RelativeCursorPosition, &ComputedNode), With<KeyLane>>,
    windows: Query<&Window>,
    state: Option<Res<AnimationEditorState>>,
    runs: Res<KeyRuns>,
    mut layout: ResMut<TrackLayout>,
    mut cache: ResMut<TimelineClip>,
    mut selected: ResMut<SelectedKey>,
    mut all: ResMut<SelectedKeys>,
    mut popover: ResMut<PopoverState>,
    mode: Res<TimelineMode>,
    bridge: Option<Res<AnimEditorBridge>>,
) {
    let Some(state) = state else { return };
    // A scrollbar owns this press (it runs earlier in the chain).
    if drag.scrollbar {
        return;
    }
    let (zoom, scroll, th) = (state.timeline_zoom.max(1.0), state.timeline_scroll, state.track_height.max(1.0));
    let cursor_win = windows.iter().find_map(|w| w.cursor_position());

    // The Curves view has its own pointer (`graph::graph_input`, next in the
    // chain): its keys sit at values, not on rows.
    if mode.curves {
        drag.active = None;
        drag.press = None;
        return;
    }

    // ── Key drag in flight ──
    if drag.active.is_some() {
        if !buttons.pressed(MouseButton::Left) {
            let Some(d) = drag.active.take() else { return };
            if d.moved {
                finish_move(&mut cache, &mut selected, &mut all, &d);
            } else if d.summary {
                // Already selected at the press; nothing narrows.
            } else if let (Some((k, _)), Some(c)) = (d.keys.first(), cursor_win) {
                // A click, not a drag: just this key, and its popover (an
                // effect's key card is the same card; see `popover`).
                selected.0 = Some(*k);
                all.set(vec![*k]);
                popover.request = Some(c);
            }
            return;
        }
        let Some(p) = lane_cursor_any(&lane) else { return };
        let Some(d) = drag.active.as_mut() else { return };
        let dt_raw = (p.x - d.start_cursor_x) / zoom;
        if !d.moved && (p.x - d.start_cursor_x).abs() < 3.0 {
            return;
        }
        d.moved = true;
        // Snap the pressed key, then move the rest by the same amount so a
        // group keeps its spacing.
        let (_, t0) = d.keys[0];
        let mut t = (t0 + dt_raw).max(0.0);
        if state.snap_enabled && state.snap_interval > 0.0 {
            t = (t / state.snap_interval).round() * state.snap_interval;
        }
        let dur = cache.clip.as_ref().map(|c| c.duration).unwrap_or(f32::MAX);
        let delta = t.min(dur) - t0;
        let moves: Vec<(SelKey, f32)> = d.keys.iter().map(|(k, orig)| (*k, (orig + delta).clamp(0.0, dur))).collect();
        let mut changed = false;
        for (k, nt) in moves {
            if let Some(mut chan) = cache.lane_times(k.lane) {
                if chan.time(k.index).is_some_and(|cur| (cur - nt).abs() > 1e-6) {
                    chan.set_time(k.index, nt);
                    changed = true;
                }
            }
        }
        if changed {
            cache.dirty = true;
        }
        return;
    }

    // ── Box-select / click on empty lane space in flight ──
    if let Some(start) = drag.press {
        let cur = lane_cursor_any(&lane).unwrap_or(start);
        if buttons.pressed(MouseButton::Left) {
            if drag.boxed.is_some() || cur.distance(start) > 4.0 {
                drag.boxed = Some((start, cur));
            }
            return;
        }
        drag.press = None;
        if let Some((a, b)) = drag.boxed.take() {
            if cache.clip.is_some() {
                let mut found = keys_in_box(&cache, &layout, zoom, scroll, th, a, b);
                let additive = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
                if additive {
                    for k in &all.keys {
                        if !found.contains(k) {
                            found.push(*k);
                        }
                    }
                }
                selected.0 = found.first().copied();
                all.set(found);
            }
        } else {
            // A plain click: move the playhead there, drop the selection.
            selected.0 = None;
            all.clear();
            let mut t = ((start.x - LANE_INSET) / zoom + scroll).max(0.0);
            if state.snap_enabled && state.snap_interval > 0.0 {
                t = (t / state.snap_interval).round() * state.snap_interval;
            }
            let dur = cache.clip.as_ref().map(|c| c.duration).unwrap_or(t);
            if let Some(bridge) = &bridge {
                push(bridge, AnimEditorAction::SetScrubTime(t.min(dur)));
            }
            if let Some(row) = layout.row_at(start.y, th) {
                layout.selected = Some(layout.rows[row].kind);
            }
        }
        return;
    }

    // ── A new press ──
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(p) = lane_cursor(&lane) else { return };
    if cache.clip.is_none() {
        return;
    }
    let Some(pick) = pick_key(&cache, &runs, &layout, zoom, scroll, th, p) else {
        // A summary diamond: every key on every track at that time moves as
        // one, through the same drag (and so the same snap, sort and undo) as
        // a hand-made selection.
        if let Some(t) = pick_summary(&runs, &layout, zoom, scroll, th, p) {
            let group = keys_at_time(&cache, t);
            if !group.is_empty() {
                selected.0 = group.first().copied();
                all.set(group.clone());
                if let Some(bridge) = &bridge {
                    push(bridge, AnimEditorAction::SetScrubTime(t));
                }
                drag.active = Some(KeyDrag { keys: group.into_iter().map(|k| (k, t)).collect(), start_cursor_x: p.x, moved: false, summary: true });
                return;
            }
        }
        // The playhead's needle runs the full height of the lanes, so it can
        // be taken there as well as in the ruler. `ruler_drag` (next in the
        // chain) carries the scrub on from this same frame. A key under the
        // needle still wins, above, so keys at the playhead stay draggable.
        let xp = x_of(state.scrub_time, zoom, scroll);
        if (xp - p.x).abs() <= NEEDLE_HIT {
            drag.ruler = Some(RulerGrab::Scrub { offset: state.scrub_time - ((p.x - LANE_INSET) / zoom + scroll), needle: true });
            return;
        }
        drag.press = Some(p);
        return;
    };
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    if shift {
        // Shift-click toggles the key in the selection, without dragging.
        let mut set = all.keys.clone();
        if let Some(i) = set.iter().position(|k| *k == pick.key) {
            set.remove(i);
        } else {
            set.push(pick.key);
        }
        selected.0 = set.first().copied();
        all.set(set);
        return;
    }
    let group: Vec<SelKey> = if all.contains(&pick.key) && all.keys.len() > 1 {
        let mut g = vec![pick.key];
        g.extend(all.keys.iter().copied().filter(|k| *k != pick.key));
        g
    } else {
        all.set(vec![pick.key]);
        vec![pick.key]
    };
    selected.0 = Some(pick.key);
    // Found by the lane it holds: an emitter's on/off keys sit on its
    // emitter row, which a track index alone does not name.
    if let Some(row) = layout.rows.iter().position(|r| row_lanes(r.kind).iter().any(|(l, _)| *l == pick.key.lane)) {
        layout.selected = Some(layout.rows[row].kind);
    }
    // Jump the playhead to the key so its pose shows and live-edit can capture
    // changes back into it.
    if let Some(bridge) = &bridge {
        push(bridge, AnimEditorAction::SetScrubTime(pick.time));
    }
    let keys_with_t: Vec<(SelKey, f32)> = group
        .into_iter()
        .filter_map(|k| {
            let t = cache.key_times(k.lane).get(k.index).copied()?;
            Some((k, t))
        })
        .collect();
    drag.active = Some(KeyDrag { keys: keys_with_t, start_cursor_x: p.x, moved: false, summary: false });
}

/// A moved group is unsorted until the drag ends: sort each touched lane for
/// playback, then find where each moved key landed so the selection follows it.
fn finish_move(cache: &mut TimelineClip, selected: &mut SelectedKey, all: &mut SelectedKeys, d: &KeyDrag) {
    let mut lanes: Vec<Lane> = d.keys.iter().map(|(k, _)| k.lane).collect();
    lanes.dedup();
    // New time of every moved key, read before sorting.
    let moved: Vec<(SelKey, f32)> = d
        .keys
        .iter()
        .filter_map(|(k, _)| {
            let chan = cache.lane_times(k.lane)?;
            Some((*k, chan.time(k.index)?))
        })
        .collect();
    for lane in &lanes {
        if let Some(mut chan) = cache.lane_times(*lane) {
            chan.sort();
        }
    }
    let mut remapped: Vec<SelKey> = Vec::new();
    for (k, t) in &moved {
        let times = cache.key_times(k.lane);
        let found = times
            .iter()
            .enumerate()
            .find(|(i, kt)| (**kt - t).abs() < 1e-6 && !remapped.contains(&SelKey { lane: k.lane, index: *i }))
            .map(|(i, _)| i);
        if let Some(index) = found {
            remapped.push(SelKey { lane: k.lane, index });
        }
    }
    selected.0 = remapped.first().copied();
    all.set(remapped);
}

/// Every left-button gesture in the ruler, decided once at the press by
/// [`ruler_hit`]: drag a loop handle or the whole loop band (upper half), the
/// clip-length marker (lower half), or scrub. The playhead needle in the
/// lanes hands its press here too (see [`key_drag`]).
///
/// The loop region is editor state only; the length marker is the one
/// gesture here that edits the clip.
///
/// Scrubbing lives here rather than in the timeline widget's scrub surface
/// (see [`anim_sync`]) so it only ever starts from a press this system saw
/// land in the ruler with nothing else claiming it: a scrollbar press raises
/// `drag.scrollbar` earlier in the chain and is turned away below.
pub(super) fn ruler_drag(
    mut drag: ResMut<KeyDragState>,
    buttons: Res<ButtonInput<MouseButton>>,
    pane: Query<(&RelativeCursorPosition, &ComputedNode), With<PaneLayer>>,
    theme: Option<Res<renzora_ember::style::Theme>>,
    state: Option<ResMut<AnimationEditorState>>,
    mut cache: ResMut<TimelineClip>,
) {
    let Some(mut state) = state else { return };
    let Some((rcp, cn)) = pane.iter().next() else { return };
    let Some(n) = rcp.normalized else { return };
    let p = (n + Vec2::splat(0.5)) * (cn.size() * cn.inverse_scale_factor());
    let ruler_h = theme.map(|t| t.timeline.ruler_height).unwrap_or(28.0);
    let (zoom, scroll) = (state.timeline_zoom.max(1.0), state.timeline_scroll);
    let Some(dur) = cache.clip.as_ref().map(|c| c.duration) else { return };
    let snap_on = state.snap_enabled && state.snap_interval > 0.0;
    let interval = state.snap_interval;
    let snap = |t: f32| if snap_on { (t / interval).round() * interval } else { t };
    let cursor_t = (p.x - LANE_INSET) / zoom + scroll;

    if let Some(grab) = drag.ruler {
        if !buttons.pressed(MouseButton::Left) {
            drag.ruler = None;
            return;
        }
        match grab {
            RulerGrab::Length => {
                let t = snap(cursor_t.max(0.2)).clamp(0.2, 600.0);
                if (t - dur).abs() > 1e-5 {
                    if let Some(clip) = cache.clip.as_mut() {
                        clip.duration = t;
                    }
                    cache.dirty = true;
                    state.clip_duration = Some(t);
                }
                // The stored region is clamped into the new length on every
                // read (`loop_range`), so it needs no fix-up here.
            }
            RulerGrab::LoopStart => {
                let (_, end) = state.loop_range(dur);
                let t = snap(cursor_t).clamp(0.0, (end - crate::LOOP_MIN_SPAN).max(0.0));
                if (t - state.loop_start).abs() > 1e-5 {
                    state.loop_start = t;
                }
            }
            RulerGrab::LoopEnd => {
                let (start, _) = state.loop_range(dur);
                let t = snap(cursor_t).clamp((start + crate::LOOP_MIN_SPAN).min(dur), dur);
                set_loop_end(&mut state, t, dur);
            }
            RulerGrab::Band { grab, start, end } => {
                // Whole frames when snapping, and clamped so the band keeps
                // its length against either end of the clip.
                let span = end - start;
                let s = snap(start + (cursor_t - grab)).clamp(0.0, (dur - span).max(0.0));
                if (s - state.loop_start).abs() > 1e-5 {
                    state.loop_start = s;
                    set_loop_end(&mut state, s + span, dur);
                }
            }
            RulerGrab::Scrub { offset, .. } => {
                let t = snap(cursor_t + offset).clamp(0.0, dur);
                if (t - state.scrub_time).abs() > 1e-6 {
                    state.scrub_time = t;
                }
            }
        }
        return;
    }
    if !buttons.just_pressed(MouseButton::Left) || !rcp.cursor_over || p.y > ruler_h || drag.scrollbar || drag.owns_pointer() {
        return;
    }
    let grab = ruler_hit(p, &state, dur, ruler_h);
    drag.ruler = Some(grab);
    // A press on bare ruler moves the playhead there at once, as a click.
    if let RulerGrab::Scrub { offset, .. } = grab {
        state.scrub_time = snap(cursor_t + offset).clamp(0.0, dur);
    }
}

/// Store a loop end, as `None` when it sits on the clip's end so the region
/// keeps following the clip if its length changes later.
pub(super) fn set_loop_end(state: &mut AnimationEditorState, t: f32, dur: f32) {
    let want = ((dur - t).abs() > 1e-4).then_some(t);
    if state.loop_end != want {
        state.loop_end = want;
    }
}

/// Offset the track rows so they scroll with the lanes.
pub(super) fn sync_header_offset(
    layout: Res<TrackLayout>,
    state: Option<Res<AnimationEditorState>>,
    mut q: Query<&mut Node, With<HeaderListOffset>>,
) {
    let th = state.map(|s| s.track_height).unwrap_or(ROW_DEFAULT).max(1.0);
    let (first, _) = layout.visible_range(th);
    let top = Val::Px(first as f32 * th - layout.vscroll);
    for mut n in &mut q {
        if n.top != top {
            n.top = top;
        }
    }
}

/// Show the box-select rectangle while one is being dragged.
pub(super) fn sync_box_rect(drag: Res<KeyDragState>, mut q: Query<&mut Node, With<BoxRect>>) {
    for mut n in &mut q {
        match drag.boxed {
            Some((a, b)) => {
                let (min, max) = (a.min(b), a.max(b));
                let want = (Val::Px(min.x), Val::Px(min.y), Val::Px(max.x - min.x), Val::Px(max.y - min.y));
                if n.display != Display::Flex {
                    n.display = Display::Flex;
                }
                if (n.left, n.top, n.width, n.height) != want {
                    (n.left, n.top, n.width, n.height) = want;
                }
            }
            None => {
                if n.display != Display::None {
                    n.display = Display::None;
                }
            }
        }
    }
}

/// Click a track row to select it; click a bone's caret to fold its children.
pub(super) fn track_row_click(
    rows: Query<(&UiInteraction, &TrackRowBtn), Changed<UiInteraction>>,
    carets: Query<(&UiInteraction, &CollapseBtn), Changed<UiInteraction>>,
    cache: Res<TimelineClip>,
    mut layout: ResMut<TrackLayout>,
) {
    for (i, r) in &rows {
        if *i == UiInteraction::Pressed && layout.selected != Some(r.0) {
            layout.selected = Some(r.0);
        }
    }
    let Some(clip) = cache.clip.as_ref() else { return };
    for (i, c) in &carets {
        if *i != UiInteraction::Pressed {
            continue;
        }
        let Some(name) = clip.tracks.get(c.0).map(|t| t.bone_name.clone()) else { continue };
        if !layout.collapsed.remove(&name) {
            layout.collapsed.insert(name);
        }
    }
}

/// Right-click a row in the track list: delete the track, or disable it so
/// the clip plays without it (and enable it again). The summary row merges
/// every track and is not one itself, so it has no menu.
///
/// On release, like [`key_context_menu`]. The row is found by hover, which
/// ember keeps on a row while the pointer is over any of its children (the
/// property dropdown, the chips), so a right-click anywhere on the row counts.
pub(super) fn track_context_menu(
    buttons: Res<ButtonInput<MouseButton>>,
    rows: Query<(&UiInteraction, &TrackRowBtn)>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    cache: Res<TimelineClip>,
    mut layout: ResMut<TrackLayout>,
    mut commands: Commands,
) {
    if !buttons.just_released(MouseButton::Right) {
        return;
    }
    let Some(fonts) = fonts else { return };
    let Some(kind) = rows
        .iter()
        .find(|(i, _)| matches!(i, UiInteraction::Hovered | UiInteraction::Pressed))
        .map(|(_, r)| r.0)
    else {
        return;
    };
    let Some(clip) = cache.clip.as_ref() else { return };
    // An effect's rows: delete the track (or an emitter's on/off keys).
    // There is no disable for them; switching an emitter off is what its own
    // on/off keys are for.
    if matches!(kind, RowKind::FxEmitter(..) | RowKind::FxTrack(_)) {
        let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else { return };
        if !cache.fx.as_ref().is_some_and(|f| f.editable) {
            return;
        }
        let items = super::fx::row_menu_items(&mut commands, &fonts, kind);
        if items.is_empty() {
            return;
        }
        if layout.selected != Some(kind) {
            layout.selected = Some(kind);
        }
        let menu = screen_menu(&mut commands, cursor.x, cursor.y);
        commands.entity(menu).add_children(&items);
        return;
    }
    let muted = match kind {
        RowKind::Bone(i) => clip.tracks.get(i).map(|t| t.muted),
        RowKind::Prop(i) => clip.property_tracks.get(i).map(|p| p.muted),
        RowKind::Morph(i) => clip.morph_tracks.get(i).map(|m| m.muted),
        RowKind::Summary | RowKind::FxGroup | RowKind::FxEmitter(..) | RowKind::FxTrack(_) => None,
    };
    let Some(muted) = muted else { return };
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else { return };
    // Select the row too, so it is plain which track the menu acts on.
    if layout.selected != Some(kind) {
        layout.selected = Some(kind);
    }
    let menu = screen_menu(&mut commands, cursor.x, cursor.y);
    let (icon, label) = if muted {
        ("eye", renzora::lang::t_or("timeline.enable_track", "Enable track"))
    } else {
        ("eye-slash", renzora::lang::t_or("timeline.disable_track", "Disable track"))
    };
    let toggle = menu_item(&mut commands, &fonts, icon, &label, move |w| {
        super::props::set_track_muted(w, kind, !muted);
    });
    let del = menu_item(&mut commands, &fonts, "trash", &renzora::lang::t_or("timeline.delete_track", "Delete track"), move |w| {
        super::props::delete_track(w, kind);
    });
    commands.entity(menu).add_children(&[toggle, del]);
}

/// Right-click a key → its menu (delete, set to pose, interpolation); on a
/// marker flag → delete it; on empty property lane → add a key there.
#[allow(clippy::too_many_arguments)]
pub(super) fn key_context_menu(
    buttons: Res<ButtonInput<MouseButton>>,
    lane: Query<(&RelativeCursorPosition, &ComputedNode), With<KeyLane>>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    state: Option<Res<AnimationEditorState>>,
    runs: Res<KeyRuns>,
    layout: Res<TrackLayout>,
    cache: Res<TimelineClip>,
    drag: Res<KeyDragState>,
    mode: Res<TimelineMode>,
    selection: (Res<SelectedKey>, Res<SelectedKeys>),
    mut commands: Commands,
) {
    // On release, not press: a right press on empty space may become a pan
    // (`timeline_pan`), and only once it is let go is it known to be a click.
    // The Curves view has its own menu (`graph::graph_context_menu`).
    if !buttons.just_released(MouseButton::Right) || drag.pan_dragged || mode.curves {
        return;
    }
    let (Some(fonts), Some(state)) = (fonts, state) else { return };
    let Some(p) = lane_cursor(&lane) else { return };
    let Some(clip) = cache.clip.as_ref() else { return };
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else {
        return;
    };
    let (zoom, scroll, th) = (state.timeline_zoom, state.timeline_scroll, state.track_height);

    // Top strip: right-click near a marker line → delete that marker.
    if p.y < 16.0 {
        if let Some(mi) = clip.markers.iter().position(|m| (x_of(m.time, zoom, scroll) - p.x).abs() < 7.0) {
            let menu = screen_menu(&mut commands, cursor.x, cursor.y);
            let label = format!("{} '{}'", renzora::lang::t("timeline.delete_marker"), clip.markers[mi].name);
            let del = menu_item(&mut commands, &fonts, "trash", &label, move |w| {
                if let Some(mut cache) = w.get_resource_mut::<TimelineClip>() {
                    if let Some(clip) = cache.clip.as_mut() {
                        if mi < clip.markers.len() {
                            clip.markers.remove(mi);
                        }
                    }
                    cache.dirty = true;
                }
            });
            commands.entity(menu).add_children(&[del]);
            return;
        }
    }

    let fx_editable = cache.fx.as_ref().is_some_and(|f| f.editable);
    if let Some(pick) = pick_key(&cache, &runs, &layout, zoom, scroll, th, p) {
        if let Lane::Fx { track } = pick.key.lane {
            if fx_editable {
                let kids = super::fx::key_menu_items(&mut commands, &fonts, &cache, track, pick.key.index);
                let menu = screen_menu(&mut commands, cursor.x, cursor.y);
                commands.entity(menu).add_children(&kids);
            }
            return;
        }
        let menu = screen_menu(&mut commands, cursor.x, cursor.y);
        let (lane_ref, idx) = (pick.key.lane, pick.key.index);
        let del = menu_item(&mut commands, &fonts, "trash", &renzora::lang::t("timeline.delete_keyframe"), move |w| {
            if let Some(mut cache) = w.get_resource_mut::<TimelineClip>() {
                if let Some(mut chan) = cache.lane_times(lane_ref) {
                    chan.remove(idx);
                }
                cache.dirty = true;
            }
            if let Some(mut s) = w.get_resource_mut::<SelectedKeys>() {
                s.clear();
            }
            if let Some(mut s) = w.get_resource_mut::<SelectedKey>() {
                s.0 = None;
            }
        });
        let mut kids = vec![del];
        if let Lane::Bone { .. } | Lane::Morph { .. } = lane_ref {
            // How the key moves on: the whole selection when the key is in
            // it, else just this key. A blend shape key eases with the same
            // modes as a bone key.
            let (primary, all) = (&selection.0, &selection.1);
            let mut sel = all.keys.clone();
            if let Some(p) = primary.0.filter(|p| !sel.contains(p)) {
                sel.push(p);
            }
            let targets = if sel.contains(&pick.key) { sel } else { vec![pick.key] };
            let current = super::keymode::key_mode(clip, pick.key);
            kids.extend(super::keymode::mode_menu_items(&mut commands, &fonts, targets, current));
        }
        if let Lane::Prop { track } = lane_ref {
            let set_item = menu_item(&mut commands, &fonts, "crosshair", &renzora::lang::t("timeline.set_to_current_pose"), move |w| {
                let entity = clip_entity(w);
                set_key_to_live(w, entity, track, idx);
            });
            kids.push(set_item);
            let current = clip.property_tracks.get(track).and_then(|pt| pt.keys.get(idx)).map(|k| k.interp);
            for (icon, label, interp) in interp_menu_choices() {
                let shown_icon = if current == Some(interp) { "check" } else { icon };
                let item = menu_item(&mut commands, &fonts, shown_icon, &interp_label_tr(label), move |w| {
                    set_key_interp(w, track, idx, interp);
                });
                kids.push(item);
            }
        }
        commands.entity(menu).add_children(&kids);
        return;
    }

    // Right-click on empty space over a bound property lane → add a key there.
    let Some(row) = layout.row_at(p.y, th) else { return };
    let mut time = ((p.x - LANE_INSET) / zoom.max(1.0) + scroll).max(0.0);
    if state.snap_enabled && state.snap_interval > 0.0 {
        time = (time / state.snap_interval).round() * state.snap_interval;
    }
    // The same on an effect's row: a parameter keys its value there, an
    // emitter row switches the emitter (see `fx::add_key`).
    let kind = layout.rows[row].kind;
    if matches!(kind, RowKind::FxEmitter(..) | RowKind::FxTrack(_)) {
        if fx_editable {
            let menu = screen_menu(&mut commands, cursor.x, cursor.y);
            let add = menu_item(&mut commands, &fonts, "plus", &renzora::lang::t("timeline.add_keyframe_here"), move |w| {
                if let Some(mut cache) = w.get_resource_mut::<TimelineClip>() {
                    super::fx::add_key(&mut cache, kind, time);
                }
            });
            commands.entity(menu).add_children(&[add]);
        }
        return;
    }
    let RowKind::Prop(pi) = kind else { return };
    let Some(pt) = clip.property_tracks.get(pi) else { return };
    if pt.component.is_empty() {
        return;
    }
    let entity = cache.key.as_ref().map(|(e, _)| *e);
    let menu = screen_menu(&mut commands, cursor.x, cursor.y);
    let add = menu_item(&mut commands, &fonts, "plus", &renzora::lang::t("timeline.add_keyframe_here"), move |w| {
        add_key_at(w, entity, pi, time);
    });
    commands.entity(menu).add_children(&[add]);
}

// ── Toolbar actions and shortcuts ────────────────────────────────────────────

/// "Add event" (overflow menu) → an event marker at the playhead, named from
/// the field beside it (default "event").
pub(super) fn add_marker_click(
    q: Query<&UiInteraction, (With<AddMarkerBtn>, Changed<UiInteraction>)>,
    field: Query<&EmberTextInput, With<MarkerNameField>>,
    state: Option<Res<AnimationEditorState>>,
    mut cache: ResMut<TimelineClip>,
) {
    if !q.iter().any(|i| *i == UiInteraction::Pressed) {
        return;
    }
    let Some(state) = state else { return };
    let name = field
        .iter()
        .next()
        .map(|f| f.value.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "event".to_string());
    let time = state.scrub_time;
    if let Some(clip) = cache.clip.as_mut() {
        clip.markers.push(AnimMarker { time, name });
        cache.dirty = true;
    }
}

/// "+" beside the clip picker, or Create in the "New clip" card → a new clip on
/// the selected entity's animator, then select it. With no name typed it picks
/// the first unused `clip`, `clip_2`, ... name. Defers the world mutation
/// through [`EditorCommands`] and reuses `setup::create_clip_on_entity`, so
/// this and the empty-state cards build clips identically.
pub(super) fn new_clip_click(
    q: Query<&UiInteraction, (With<NewClipBtn>, Changed<UiInteraction>)>,
    field: Query<&EmberTextInput, With<NewClipNameField>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    if !q.iter().any(|i| *i == UiInteraction::Pressed) {
        return;
    }
    let raw = field.iter().next().map(|f| f.value.trim().to_string()).unwrap_or_default();
    cmds.push(move |world: &mut World| {
        let Some(entity) = world.get_resource::<AnimationEditorState>().and_then(|s| s.selected_entity) else {
            return;
        };
        let name = if raw.is_empty() { unused_clip_name(world, entity, "clip") } else { crate::setup::sanitize_clip_name(&raw) };
        crate::setup::create_clip_on_entity(world, entity, &name);
        // Close the "New clip" card if it was open.
        let cards: Vec<Entity> = world
            .query_filtered::<Entity, With<super::menus::NewClipCard>>()
            .iter(world)
            .collect();
        for e in cards {
            if let Ok(em) = world.get_entity_mut(e) {
                em.despawn();
            }
        }
    });
}

/// `base`, or `base_2`, `base_3`, ... : the first name with neither a slot on
/// the entity's animator nor an `animations/<name>.anim` file on disk.
pub(super) fn unused_clip_name(world: &World, entity: Entity, base: &str) -> String {
    let slots: Vec<String> = world
        .get::<AnimatorComponent>(entity)
        .map(|a| a.clips.iter().map(|c| c.name.clone()).collect())
        .unwrap_or_default();
    let root = world.get_resource::<renzora::core::CurrentProject>().map(|p| p.path.clone());
    let taken = |n: &str| {
        slots.iter().any(|s| s == n)
            || root.as_ref().is_some_and(|r| r.join(format!("animations/{n}.anim")).exists())
    };
    let base = crate::setup::sanitize_clip_name(base);
    if !taken(&base) {
        return base;
    }
    (2..1000).map(|i| format!("{base}_{i}")).find(|n| !taken(n)).unwrap_or(base)
}

/// Keyboard shortcuts, active only while the cursor is over the timeline panel
/// (so they don't clash with global editor shortcuts):
/// Space = play/pause · Home / End = loop start / loop end · Shift+Home /
/// Shift+End = clip start / clip end · I / O = set the loop start / end at
/// the playhead · ←/→ or ,/. = step frame · K = add keyframe (all tracks) ·
/// N = new track · Delete = delete selected keys · Ctrl+C / Ctrl+V = copy /
/// paste keys · Escape = clear the selection.
///
/// Home and End land on the loop region rather than the clip because with the
/// region untouched the two are the same, and once it is set the region is
/// what is being worked on; Shift reaches past it.
///
/// Held off entirely while a UI text field has keyboard focus
/// (`ui_wants_keyboard`) or a drag-value is being typed into, otherwise typing
/// a clip/marker/filter name or a number would leak into these actions.
#[allow(clippy::too_many_arguments)]
pub(super) fn timeline_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    root: Query<&RelativeCursorPosition, With<AnimTimeline>>,
    state: Option<ResMut<AnimationEditorState>>,
    cache: Option<Res<TimelineClip>>,
    bridge: Option<Res<AnimEditorBridge>>,
    mut selected: ResMut<SelectedKey>,
    mut all: ResMut<SelectedKeys>,
    focus: Option<Res<renzora::InputFocusState>>,
    drag_editing: Option<Res<renzora_ember::widgets::AnyDragValueEditing>>,
    popover: Res<PopoverState>,
    mut ops: ResMut<TimelineOps>,
) {
    if focus.is_some_and(|f| f.ui_wants_keyboard) || drag_editing.is_some_and(|d| d.0) {
        return;
    }
    if !root.iter().any(|r| r.cursor_over) {
        return;
    }
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::SuperLeft, KeyCode::SuperRight]);
    let has_sel = selected.0.is_some() || !all.keys.is_empty();
    if (keys.just_pressed(KeyCode::Delete) || keys.just_pressed(KeyCode::Backspace)) && has_sel {
        ops.delete_selected_key = true;
    }
    if ctrl && keys.just_pressed(KeyCode::KeyC) && has_sel {
        ops.copy_keys = true;
    }
    if ctrl && keys.just_pressed(KeyCode::KeyV) {
        ops.paste_keys = true;
    }
    if keys.just_pressed(KeyCode::Escape) && !popover.open && has_sel {
        all.clear();
        selected.0 = None;
    }
    let (Some(mut state), Some(bridge)) = (state, bridge) else { return };
    let dur = cache.and_then(|c| c.clip.as_ref().map(|c| c.duration)).unwrap_or(2.0);
    let frame = 1.0 / 30.0;
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let (loop_s, loop_e) = state.loop_range(dur);
    if keys.just_pressed(KeyCode::Space) {
        push(&bridge, AnimEditorAction::TogglePreview);
    }
    if keys.just_pressed(KeyCode::Home) {
        push(&bridge, AnimEditorAction::SetScrubTime(if shift { 0.0 } else { loop_s }));
    }
    if keys.just_pressed(KeyCode::End) {
        push(&bridge, AnimEditorAction::SetScrubTime(if shift { dur } else { loop_e }));
    }
    // I / O: the loop region's start / end at the playhead (the in / out
    // points of a video editor). Each keeps the region at least
    // `LOOP_MIN_SPAN` long by holding against the other end.
    let t = state.scrub_time.clamp(0.0, dur);
    if !ctrl && keys.just_pressed(KeyCode::KeyI) {
        let s = t.min((loop_e - crate::LOOP_MIN_SPAN).max(0.0));
        if (s - state.loop_start).abs() > 1e-5 {
            state.loop_start = s;
        }
    }
    if !ctrl && keys.just_pressed(KeyCode::KeyO) {
        let e = t.max((loop_s + crate::LOOP_MIN_SPAN).min(dur));
        set_loop_end(&mut state, e, dur);
    }
    if keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::Period) {
        push(&bridge, AnimEditorAction::SetScrubTime((state.scrub_time + frame).min(dur)));
    }
    if keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::Comma) {
        push(&bridge, AnimEditorAction::SetScrubTime((state.scrub_time - frame).max(0.0)));
    }
    if !ctrl && keys.just_pressed(KeyCode::KeyK) {
        ops.add_key = true;
    }
    if !ctrl && keys.just_pressed(KeyCode::KeyN) {
        ops.add_empty_track = true;
    }
}

/// Raise `InputFocusState::suppress_entity_delete` while keys are selected and
/// the cursor is over the timeline, so pressing Delete removes the keys
/// instead of the selected entity (the global entity-delete shortcut honors it).
pub(super) fn timeline_delete_guard(
    root: Query<&RelativeCursorPosition, With<AnimTimeline>>,
    selected: Res<SelectedKey>,
    all: Res<SelectedKeys>,
    focus: Option<ResMut<renzora::InputFocusState>>,
) {
    let active = (selected.0.is_some() || !all.keys.is_empty()) && root.iter().any(|r| r.cursor_over);
    if let Some(mut focus) = focus {
        if focus.suppress_entity_delete != active {
            focus.suppress_entity_delete = active;
        }
    }
}

/// The wheel over the timeline: scrolls the rows; Shift+wheel (or a sideways
/// trackpad swipe / tilt wheel) pans in time; Ctrl+wheel zooms about the
/// cursor; Alt+wheel makes the rows taller or shorter. The scrollbars do the
/// same two scrolls by dragging.
#[allow(clippy::too_many_arguments)]
pub(super) fn timeline_wheel(
    mut wheel: MessageReader<MouseWheel>,
    keys: Res<ButtonInput<KeyCode>>,
    root: Query<&RelativeCursorPosition, With<AnimTimeline>>,
    lane: Query<(&RelativeCursorPosition, &ComputedNode), With<KeyLane>>,
    state: Option<Res<AnimationEditorState>>,
    mut layout: ResMut<TrackLayout>,
    cache: Res<TimelineClip>,
    bridge: Option<Res<AnimEditorBridge>>,
    mode: Res<TimelineMode>,
) {
    let dur = cache.clip.as_ref().map(|c| c.duration).unwrap_or(2.0);
    let (mut dx, mut dy) = (0.0, 0.0);
    for ev in wheel.read() {
        let div = match ev.unit {
            bevy::input::mouse::MouseScrollUnit::Line => 1.0,
            bevy::input::mouse::MouseScrollUnit::Pixel => 40.0,
        };
        dx += ev.x / div;
        dy += ev.y / div;
    }
    if (dx == 0.0 && dy == 0.0) || !root.iter().any(|r| r.cursor_over) {
        return;
    }
    let (Some(state), Some(bridge)) = (state, bridge) else { return };
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::SuperLeft, KeyCode::SuperRight]);
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let alt = keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);
    let zoom = state.timeline_zoom.max(1.0);
    if dx != 0.0 && !ctrl && !alt {
        let step = 60.0 / zoom;
        push(&bridge, AnimEditorAction::SetTimelineScroll(clamp_hscroll(&state, &layout, dur, state.timeline_scroll - dx * step)));
        if dy == 0.0 {
            return;
        }
    }
    if alt {
        let th = (state.track_height + dy.signum() * 2.0).clamp(ROW_MIN, ROW_MAX);
        push(&bridge, AnimEditorAction::SetTrackHeight(th));
    } else if ctrl {
        let new_zoom = (zoom * 1.15f32.powf(dy)).clamp(ZOOM_MIN, ZOOM_MAX);
        // Keep the time under the cursor where it is.
        if let Some(p) = lane_cursor_any(&lane) {
            let t = (p.x - LANE_INSET) / zoom + state.timeline_scroll;
            let new_scroll = (t - (p.x - LANE_INSET) / new_zoom).max(0.0);
            push(&bridge, AnimEditorAction::SetTimelineScroll(new_scroll));
        }
        push(&bridge, AnimEditorAction::SetTimelineZoom(new_zoom));
    } else if shift {
        let step = 60.0 / zoom;
        push(&bridge, AnimEditorAction::SetTimelineScroll(clamp_hscroll(&state, &layout, dur, state.timeline_scroll - dy * step)));
    } else if mode.curves {
        // The Curves view has no rows to scroll: there the plain wheel zooms
        // the value axis (`graph::graph_wheel`).
    } else {
        let th = state.track_height.max(1.0);
        let v = (layout.vscroll - dy * th * 2.0).clamp(0.0, layout.max_vscroll(th));
        if (v - layout.vscroll).abs() > 0.01 {
            layout.vscroll = v;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 100 px/s, unscrolled: t seconds sits at `24 + 100 t` px.
    fn state() -> AnimationEditorState {
        AnimationEditorState { timeline_zoom: 100.0, timeline_scroll: 0.0, ..Default::default() }
    }

    /// A 28 px ruler: the loop strip is y < 14, the scrub half below.
    const RH: f32 = 28.0;

    #[test]
    fn start_handle_wins_over_a_playhead_parked_on_it() {
        // A fresh clip: loop start, playhead and t=0 all at one x. The start
        // handle has to be the one taken, or it could never be picked up.
        let s = state();
        assert_eq!(ruler_hit(Vec2::new(LANE_INSET, 8.0), &s, 2.0, RH), RulerGrab::LoopStart);
    }

    #[test]
    fn loop_end_and_clip_length_share_an_x_but_not_a_half() {
        // With the region untouched the loop end sits on the clip end: the
        // upper half takes the loop end, the lower half the clip's length.
        let s = state();
        let x = LANE_INSET + 200.0 + 3.0;
        assert_eq!(ruler_hit(Vec2::new(x, 8.0), &s, 2.0, RH), RulerGrab::LoopEnd);
        assert_eq!(ruler_hit(Vec2::new(x, 24.0), &s, 2.0, RH), RulerGrab::Length);
    }

    #[test]
    fn the_band_is_grabbed_in_the_strip_once_it_can_move() {
        let mut s = state();
        // Whole-clip band: nothing to move, so the strip scrubs.
        assert!(matches!(ruler_hit(Vec2::new(LANE_INSET + 120.0, 8.0), &s, 2.0, RH), RulerGrab::Scrub { .. }));
        s.loop_start = 0.5;
        s.loop_end = Some(1.5);
        assert_eq!(
            ruler_hit(Vec2::new(LANE_INSET + 120.0, 8.0), &s, 2.0, RH),
            RulerGrab::Band { grab: 1.2, start: 0.5, end: 1.5 }
        );
        // The scrub half under the band still scrubs.
        assert!(matches!(ruler_hit(Vec2::new(LANE_INSET + 120.0, 24.0), &s, 2.0, RH), RulerGrab::Scrub { .. }));
    }

    #[test]
    fn the_badge_takes_the_playhead_without_jumping_it() {
        let mut s = state();
        s.scrub_time = 1.0;
        // 15 px right of the needle, inside the 40 px badge.
        match ruler_hit(Vec2::new(LANE_INSET + 115.0, 12.0), &s, 2.0, RH) {
            RulerGrab::Scrub { offset, needle } => {
                assert!(needle);
                assert!((offset + 0.15).abs() < 1e-4, "offset {offset}");
            }
            other => panic!("expected a scrub, got {other:?}"),
        }
    }

    #[test]
    fn bare_ruler_scrubs_to_the_cursor() {
        let mut s = state();
        s.scrub_time = 1.0;
        assert_eq!(ruler_hit(Vec2::new(LANE_INSET + 50.0, 24.0), &s, 2.0, RH), RulerGrab::Scrub { offset: 0.0, needle: false });
    }

    #[test]
    fn loop_range_stays_inside_the_clip() {
        assert_eq!(crate::loop_range(-1.0, None, 2.0), (0.0, 2.0));
        let (s, e) = crate::loop_range(5.0, None, 2.0);
        assert!((s - (2.0 - crate::LOOP_MIN_SPAN)).abs() < 1e-6 && e == 2.0);
        // A stored end past a shortened clip is clamped to it.
        assert_eq!(crate::loop_range(0.0, Some(3.0), 2.0), (0.0, 2.0));
        assert_eq!(crate::loop_range(1.0, None, 0.01), (0.0, 0.01));
    }

    #[test]
    fn a_loop_end_on_the_clip_end_is_stored_as_none() {
        let mut s = state();
        set_loop_end(&mut s, 1.0, 2.0);
        assert_eq!(s.loop_end, Some(1.0));
        set_loop_end(&mut s, 2.0, 2.0);
        assert_eq!(s.loop_end, None);
    }
}
