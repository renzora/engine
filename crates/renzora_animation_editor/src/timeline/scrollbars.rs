//! The dopesheet's two scrollbars: time along the bottom, rows down the right.
//!
//! The wheel already scrolled both ways (Shift for time), but nothing on
//! screen said so, how far the clip ran past the edge, or where in it you
//! were. These draw that and can be dragged, or clicked off the thumb to jump
//! there.
//!
//! They sit in the pane's marker overlay, above the scrub surface, so a press
//! on one is theirs: [`scrollbar_drag`] runs ahead of the key and ruler
//! gestures in the chain and raises `KeyDragState::scrollbar`, which those
//! check before starting anything of their own. Like the other gestures it
//! tracks the raw mouse button, so a drag keeps going off the end of the bar.
//! The playhead cannot follow a scrollbar drag: the only scrub is the one
//! `edit::ruler_drag` starts, and it turns a scrollbar's press away.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::interaction::{BLOCK, PASS_THROUGH};
use renzora_ember::style::Theme;
use renzora_ember::theme::{accent, border, rgb, tree_line};
use renzora_ember::UiInteraction;

use crate::{AnimEditorAction, AnimEditorBridge, AnimationEditorState};

use super::clip::TimelineClip;
use super::edit::{max_hscroll, push, KeyDragState};
use super::layout::TrackLayout;

/// Thickness of a bar, px (the thumb is inset inside it).
pub(crate) const BAR: f32 = 10.0;
const THUMB_INSET: f32 = 2.0;
/// The thumb never shrinks below this, so a long clip still has a grip.
const THUMB_MIN: f32 = 28.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Axis {
    /// Time, along the bottom.
    H,
    /// Rows, down the right.
    V,
}

#[derive(Component)]
pub(super) struct ScrollTrack(Axis);

#[derive(Component)]
pub(super) struct ScrollThumb(Axis);

/// The bar being dragged and where along the thumb it was grabbed, px.
#[derive(Resource, Default)]
pub(super) struct ScrollbarGrab {
    axis: Option<Axis>,
    grab: f32,
}

/// Spawn both bars; the caller parents them into the pane overlay.
pub(super) fn build_scrollbars(commands: &mut Commands) -> [Entity; 2] {
    [Axis::H, Axis::V].map(|axis| {
        let track = commands
            .spawn((
                Node { position_type: PositionType::Absolute, display: Display::None, ..default() },
                BackgroundColor(Color::NONE),
                UiInteraction::default(),
                // Blocks, so a press on the bar never reaches the timeline
                // widget's scrub surface underneath it.
                BLOCK,
                RelativeCursorPosition::default(),
                HoverCursor(bevy::window::SystemCursorIcon::Default),
                ScrollTrack(axis),
                Name::new("timeline-scrollbar"),
            ))
            .id();
        let thumb = commands
            .spawn((
                Node { position_type: PositionType::Absolute, border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
                BackgroundColor(rgb(border())),
                PASS_THROUGH,
                ScrollThumb(axis),
            ))
            .id();
        commands.entity(track).add_child(thumb);
        track
    })
}

/// One bar's numbers: how far it scrolls, how far it is scrolled, and how much
/// of the content the view shows (all in the axis' own unit).
struct Extent {
    max: f32,
    pos: f32,
    /// Fraction of the content visible, 0..1.
    shown: f32,
}

fn extent(axis: Axis, state: &AnimationEditorState, layout: &TrackLayout, dur: f32) -> Extent {
    match axis {
        Axis::H => {
            let zoom = state.timeline_zoom.max(1.0);
            let max = max_hscroll(dur, zoom, layout.view.x).max(state.timeline_scroll);
            let vis = (layout.view.x - renzora_ember::widgets::LANE_INSET).max(1.0) / zoom;
            Extent { max, pos: state.timeline_scroll, shown: vis / (max + vis) }
        }
        Axis::V => {
            let th = state.track_height.max(1.0);
            let max = layout.max_vscroll(th);
            Extent { max, pos: layout.vscroll, shown: layout.view.y / layout.content_h(th).max(1.0) }
        }
    }
}

/// Thumb start and length along a track `len` px long.
fn thumb_span(e: &Extent, len: f32) -> (f32, f32) {
    let size = (e.shown.clamp(0.0, 1.0) * len).max(THUMB_MIN).min(len);
    let free = (len - size).max(0.0);
    let start = if e.max > 0.0 { (e.pos / e.max).clamp(0.0, 1.0) * free } else { 0.0 };
    (start, size)
}

fn along(axis: Axis, v: Vec2) -> f32 {
    match axis {
        Axis::H => v.x,
        Axis::V => v.y,
    }
}

/// Place the bars, hide the vertical one when every row fits, and colour the
/// thumbs for hover and drag. Writes only what changed.
#[allow(clippy::too_many_arguments)]
pub(super) fn scrollbar_layout(
    state: Option<Res<AnimationEditorState>>,
    layout: Res<TrackLayout>,
    cache: Res<TimelineClip>,
    grab: Res<ScrollbarGrab>,
    theme: Option<Res<Theme>>,
    mut tracks: Query<(&ScrollTrack, &mut Node, &ComputedNode, &UiInteraction), Without<ScrollThumb>>,
    mut thumbs: Query<(&ScrollThumb, &mut Node, &mut BackgroundColor), Without<ScrollTrack>>,
) {
    let Some(state) = state else { return };
    let dur = cache.clip.as_ref().map(|c| c.duration).unwrap_or(2.0);
    let ruler_h = theme.map(|t| t.timeline.ruler_height).unwrap_or(28.0);
    let th = state.track_height.max(1.0);
    let v_on = cache.clip.is_some() && layout.max_vscroll(th) > 0.5;
    let h_on = cache.clip.is_some();
    let px = Val::Px;

    let mut spans = [(0.0, 0.0, false); 2];
    for (track, mut node, cn, ui) in &mut tracks {
        let axis = track.0;
        let on = match axis {
            Axis::H => h_on,
            Axis::V => v_on,
        };
        let want = if on { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
        let (left, right, top, bottom, width, height) = match axis {
            Axis::H => (px(0.0), px(if v_on { BAR } else { 0.0 }), Val::Auto, px(0.0), Val::Auto, px(BAR)),
            Axis::V => (Val::Auto, px(0.0), px(ruler_h), px(if h_on { BAR } else { 0.0 }), px(BAR), Val::Auto),
        };
        if node.left != left || node.right != right || node.top != top || node.bottom != bottom || node.width != width || node.height != height {
            node.left = left;
            node.right = right;
            node.top = top;
            node.bottom = bottom;
            node.width = width;
            node.height = height;
        }
        let len = along(axis, cn.size() * cn.inverse_scale_factor()) - THUMB_INSET * 2.0;
        let (start, size) = thumb_span(&extent(axis, &state, &layout, dur), len.max(0.0));
        let i = axis as usize;
        spans[i] = (start, size, matches!(ui, UiInteraction::Hovered | UiInteraction::Pressed));
    }

    for (thumb, mut node, mut bg) in &mut thumbs {
        let axis = thumb.0;
        let (start, size, hot) = spans[axis as usize];
        let (a, b) = (px(THUMB_INSET + start), px(size));
        let (left, top, width, height) = match axis {
            Axis::H => (a, px(THUMB_INSET), b, px(BAR - THUMB_INSET * 2.0)),
            Axis::V => (px(THUMB_INSET), a, px(BAR - THUMB_INSET * 2.0), b),
        };
        if node.left != left || node.top != top || node.width != width || node.height != height {
            node.left = left;
            node.top = top;
            node.width = width;
            node.height = height;
        }
        // Theme colours rather than white at an alpha, which reads as a flat
        // grey: the border tone at rest, the lighter line tone when hot, the
        // accent while dragged.
        let c = if grab.axis == Some(axis) {
            rgb(accent()).with_alpha(0.7)
        } else if hot {
            rgb(tree_line())
        } else {
            rgb(border())
        };
        if bg.0 != c {
            bg.0 = c;
        }
    }
}

/// Drag a thumb to scroll; press the bar off the thumb to jump the thumb's
/// middle to the cursor and keep dragging from there.
#[allow(clippy::too_many_arguments)]
pub(super) fn scrollbar_drag(
    buttons: Res<ButtonInput<MouseButton>>,
    tracks: Query<(&ScrollTrack, &RelativeCursorPosition, &ComputedNode, &Node)>,
    state: Option<Res<AnimationEditorState>>,
    mut layout: ResMut<TrackLayout>,
    cache: Res<TimelineClip>,
    mut grab: ResMut<ScrollbarGrab>,
    mut drag: ResMut<KeyDragState>,
    bridge: Option<Res<AnimEditorBridge>>,
) {
    let (Some(state), Some(bridge)) = (state, bridge) else { return };
    let dur = cache.clip.as_ref().map(|c| c.duration).unwrap_or(2.0);

    if grab.axis.is_some() && !buttons.pressed(MouseButton::Left) {
        grab.axis = None;
        drag.scrollbar = false;
        return;
    }

    let pressed = buttons.just_pressed(MouseButton::Left);
    for (track, rcp, cn, node) in &tracks {
        let axis = track.0;
        if node.display == Display::None {
            continue;
        }
        if grab.axis.is_some_and(|a| a != axis) {
            continue;
        }
        if grab.axis.is_none() && !(pressed && rcp.cursor_over) {
            continue;
        }
        let Some(n) = rcp.normalized else { continue };
        let len = (along(axis, cn.size() * cn.inverse_scale_factor()) - THUMB_INSET * 2.0).max(1.0);
        let cursor = along(axis, (n + Vec2::splat(0.5)) * (cn.size() * cn.inverse_scale_factor())) - THUMB_INSET;
        let e = extent(axis, &state, &layout, dur);
        let (start, size) = thumb_span(&e, len);

        if grab.axis.is_none() {
            grab.axis = Some(axis);
            drag.scrollbar = true;
            grab.grab = if cursor >= start && cursor <= start + size { cursor - start } else { size * 0.5 };
        }
        let free = (len - size).max(1.0);
        let pos = ((cursor - grab.grab) / free).clamp(0.0, 1.0) * e.max;
        match axis {
            Axis::H => {
                if (pos - state.timeline_scroll).abs() > 1e-5 {
                    push(&bridge, AnimEditorAction::SetTimelineScroll(pos));
                }
            }
            Axis::V => {
                if (pos - layout.vscroll).abs() > 0.01 {
                    layout.vscroll = pos;
                }
            }
        }
    }
}
