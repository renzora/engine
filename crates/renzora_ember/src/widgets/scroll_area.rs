//! Scroll area — a clipping viewport that scrolls its content with the wheel
//! (smoothly, with easing) and shows a draggable scrollbar that auto-hides when
//! the content fits.
//!
//! Layout: an `outer` (relative, clips) holds the scrolling `viewport` (the
//! [`EmberScroll`] node) plus an absolutely-positioned `track` → `thumb`. Sizes
//! are read from [`ComputedNode`] so the math works whether the viewport is
//! flex-filled or capped by `max_height`.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};
use bevy::window::SystemCursorIcon;

use crate::theme::{border, rgb, text_muted, text_primary};

use std::collections::HashMap;

const BAR_W: f32 = 9.0;
const MIN_THUMB: f32 = 28.0;
const WHEEL_STEP: f32 = 52.0;
const EASE: f32 = 16.0;

/// The scrolling viewport. Holds the smooth-scroll target + scrollbar handles.
#[derive(Component)]
pub struct EmberScroll {
    target: f32,
    /// Currently auto-following the bottom (for logs). Disabled when the user
    /// scrolls up; re-enabled when they return to the bottom — but only for
    /// `pinned` views.
    stick: bool,
    /// Whether this view is the auto-follow (pinned-to-bottom) kind. Normal
    /// scroll views are `false` and must never auto-stick, or they'd jump to the
    /// bottom (e.g. on the first frame before content height is measured).
    pinned: bool,
    /// Keep the scrollbar visible whenever content overflows (not only on hover).
    always_bar: bool,
    thumb: Entity,
    track: Entity,
    /// Horizontal smooth-scroll target (logical px). Only meaningful for both-axis
    /// views (`h_thumb`/`h_track` set); left at 0 for the vertical-only default.
    target_x: f32,
    /// Horizontal scrollbar thumb + track, present only for both-axis views
    /// (built by [`scroll_view_xy`]). `None` → vertical-only, unchanged behaviour.
    h_thumb: Option<Entity>,
    h_track: Option<Entity>,
    /// Whether the mouse wheel scrolls this view. `false` frees the wheel for a
    /// zoom handler (both-axis image viewers like the tilemap atlas zoom on
    /// wheel and pan with the scrollbars / right-drag instead).
    wheel_scroll: bool,
}

impl EmberScroll {
    /// Ease the view so vertical pixel offset `offset` (from the top of the
    /// content) becomes the new scroll position. `scroll_update` clamps it to
    /// the scrollable range each frame and eases `ScrollPosition` toward it, so
    /// callers can drive "scroll this row into view" by setting the offset.
    /// Cancels bottom-stick so a pinned log view doesn't snap back.
    pub fn scroll_to(&mut self, offset: f32) {
        self.target = offset;
        self.stick = false;
    }

    /// Nudge the smooth-scroll target by `delta` px (negative = up). `scroll_update`
    /// clamps it to the scrollable range each frame, so callers needn't clamp.
    /// Used for edge-autoscroll while drag-selecting. Cancels bottom-stick.
    pub fn nudge(&mut self, delta: f32) {
        self.target += delta;
        self.stick = false;
    }

    /// Set both scroll targets at once (both-axis views). Pair with writing
    /// `ScrollPosition` directly for an immediate, un-eased jump — used by
    /// zoom-to-cursor, which must re-anchor the content the same frame it scales.
    pub fn set_offset_xy(&mut self, x: f32, y: f32) {
        self.target_x = x;
        self.target = y;
        self.stick = false;
    }
}

/// The draggable scrollbar thumb; points back at its viewport.
#[derive(Component)]
pub struct ScrollThumb {
    viewport: Entity,
    /// `true` for the horizontal thumb of a both-axis view — drags map to the
    /// X axis (and drive `EmberScroll.target_x` / `ScrollPosition.x`).
    horizontal: bool,
}

/// The scrollbar thumb currently being dragged, latched on press so the grip
/// holds even when the cursor moves off the thumb, off the track, out of the
/// panel, or out of the window entirely — like a real scrollbar.
///
/// The old drag rode `Interaction::Pressed` on the thumb plus the track's
/// `RelativeCursorPosition`; both drop the moment the cursor leaves the node, so
/// a quick flick or a drag that strayed out of the panel would let go of the
/// thumb. Latching here and driving the offset from a [`GlobalCursor`] delta —
/// the same physical-space capture the dock's divider drag uses — keeps the
/// grip until the mouse button is released.
#[derive(Resource, Default)]
pub struct DraggedThumb(Option<ThumbDrag>);

struct ThumbDrag {
    /// The thumb entity being dragged (its [`ScrollThumb`] resolves the viewport,
    /// track, and axis).
    thumb: Entity,
    /// Cursor position in physical **screen** space at grab time (from
    /// [`GlobalCursor`]); the drag is a pure delta from here, so the cursor can
    /// wander anywhere and the thumb still tracks it 1:1.
    start_cursor: Vec2,
    /// Scroll offset (logical px, along the dragged axis) at grab time.
    start_offset: f32,
}

/// Marks a scrollbar track (vertical or horizontal). Its rect covers the thumb
/// too, so a press anywhere on it is a scroll interaction — [`scrollbar_busy`]
/// uses this to tell panels "the pointer is on the scrollbar, not your content".
#[derive(Component)]
pub struct ScrollTrack;

/// True while the pointer is actively working a scrollbar — pressing on a visible
/// track (thumb or the bare track band) or mid-drag on a thumb.
///
/// The scrollbar sits *inside* each panel's content area, so without this a press
/// on the scrollbar (to scroll) also reads as a press on "empty content" and
/// starts a marquee selection / deselect / drag in the panel beneath. Panels
/// consult this flag on left-press and skip their press action when it's set, so
/// grabbing the scrollbar never triggers a selection or drag. Set in `PreUpdate`
/// (after `UiSystems::Focus`, so cursor state is fresh) and read by panel systems
/// in `Update`.
#[derive(Resource, Default)]
pub struct ScrollbarBusy(pub bool);

impl ScrollbarBusy {
    /// Whether the pointer is currently engaging a scrollbar.
    pub fn active(&self) -> bool {
        self.0
    }
}

/// Refresh [`ScrollbarBusy`]: busy when a thumb is mid-drag, or when the left
/// button is held with the cursor over any *visible* scroll track.
pub(crate) fn scrollbar_busy(
    dragged: Res<DraggedThumb>,
    mouse: Res<ButtonInput<MouseButton>>,
    tracks: Query<(&RelativeCursorPosition, &Node), With<ScrollTrack>>,
    mut busy: ResMut<ScrollbarBusy>,
) {
    let over_track = mouse.pressed(MouseButton::Left)
        && tracks
            .iter()
            .any(|(rcp, n)| n.display != Display::None && rcp.cursor_over);
    let next = dragged.0.is_some() || over_track;
    if busy.0 != next {
        busy.0 = next;
    }
}

/// Last scroll offset (logical px from the top) of each *keyed* scroll area,
/// surviving the entity's despawn. A scroll view spawned with a [`ScrollKey`]
/// (see [`scroll_area_keyed`] / [`scroll_view_keyed`]) saves its offset here and
/// restores it when an identically-keyed view is spawned again — so panels and
/// dropdowns that are torn down and rebuilt (e.g. the whole editor chrome
/// re-spawning on a theme switch) keep their scroll position instead of jumping
/// back to the top.
#[derive(Resource, Default)]
pub struct ScrollMemory(pub HashMap<String, f32>);

/// Tags a scroll viewport whose offset is remembered in [`ScrollMemory`] under
/// `key`. `restored` guards the one-shot restore: the saved offset is re-applied
/// once, only after the content's height is measured (so the clamp in
/// [`scroll_update`] can't collapse it to 0 on the first, unmeasured frame).
#[derive(Component)]
pub struct ScrollKey {
    key: String,
    restored: bool,
}

fn build_scroll(
    commands: &mut Commands,
    content: Entity,
    max_height: Option<f32>,
    stick: bool,
    always_bar: bool,
    key: Option<String>,
    both: bool,
) -> Entity {
    let viewport = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: if max_height.is_some() {
                    Val::Auto
                } else {
                    Val::Percent(100.0)
                },
                max_height: max_height.map(Val::Px).unwrap_or(Val::Auto),
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                // Both-axis views also scroll horizontally; the extra horizontal
                // track/thumb are built below.
                overflow: if both {
                    Overflow::scroll()
                } else {
                    Overflow::scroll_y()
                },
                ..default()
            },
            RelativeCursorPosition::default(),
            ScrollPosition::default(),
            Name::new("scroll-viewport"),
        ))
        .id();
    if let Some(key) = key {
        commands.entity(viewport).insert(ScrollKey { key, restored: false });
    }
    commands.entity(viewport).add_child(content);

    let track = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(2.0),
                bottom: Val::Px(2.0),
                right: Val::Px(2.0),
                width: Val::Px(BAR_W),
                display: Display::None,
                border_radius: BorderRadius::all(Val::Px(BAR_W / 2.0)),
                ..default()
            },
            BackgroundColor(rgb(border()).with_alpha(0.5)),
            RelativeCursorPosition::default(),
            ScrollTrack,
            // Local ZIndex (not Global) so the bar sits above the content but
            // stays within its modal/panel stacking context — a GlobalZIndex
            // would render it *behind* a higher-z modal (e.g. the settings
            // overlay), which is why it looked missing there.
            ZIndex(50),
            Name::new("scroll-track"),
        ))
        .id();
    let thumb = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(0.0),
                border_radius: BorderRadius::all(Val::Px(BAR_W / 2.0)),
                ..default()
            },
            BackgroundColor(rgb(text_muted())),
            Interaction::default(),
            ScrollThumb {
                viewport,
                horizontal: false,
            },
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("scroll-thumb"),
        ))
        .id();
    commands.entity(track).add_child(thumb);

    // Both-axis views get a horizontal track + thumb along the bottom edge,
    // mirroring the vertical pair. `scroll_update` sizes/hides it and
    // `scroll_thumb_drag` drives it via the `horizontal` flag.
    let (h_track, h_thumb) = if both {
        let h_track = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(2.0),
                    // Leave the bottom-right corner clear for the vertical bar.
                    right: Val::Px(BAR_W + 2.0),
                    bottom: Val::Px(2.0),
                    height: Val::Px(BAR_W),
                    display: Display::None,
                    border_radius: BorderRadius::all(Val::Px(BAR_W / 2.0)),
                    ..default()
                },
                BackgroundColor(rgb(border()).with_alpha(0.5)),
                RelativeCursorPosition::default(),
                ScrollTrack,
                ZIndex(50),
                Name::new("scroll-track-h"),
            ))
            .id();
        let h_thumb = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    height: Val::Percent(100.0),
                    width: Val::Px(0.0),
                    border_radius: BorderRadius::all(Val::Px(BAR_W / 2.0)),
                    ..default()
                },
                BackgroundColor(rgb(text_muted())),
                Interaction::default(),
                ScrollThumb {
                    viewport,
                    horizontal: true,
                },
                crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
                Name::new("scroll-thumb-h"),
            ))
            .id();
        commands.entity(h_track).add_child(h_thumb);
        (Some(h_track), Some(h_thumb))
    } else {
        (None, None)
    };

    commands.entity(viewport).insert(EmberScroll {
        target: 0.0,
        stick,
        pinned: stick,
        always_bar,
        thumb,
        track,
        target_x: 0.0,
        h_thumb,
        h_track,
        // Both-axis views are image viewers that zoom on the wheel; vertical
        // views keep the wheel for scrolling.
        wheel_scroll: !both,
    });

    let outer = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                flex_grow: if max_height.is_some() { 0.0 } else { 1.0 },
                flex_basis: if max_height.is_some() {
                    Val::Auto
                } else {
                    Val::Px(0.0)
                },
                ..default()
            },
            Name::new("scroll"),
        ))
        .id();
    let mut kids = vec![viewport, track];
    if let Some(h) = h_track {
        kids.push(h);
    }
    commands.entity(outer).add_children(&kids);
    outer
}

/// Wraps `content` in a flex-filling scrollable viewport (grows to fill its
/// parent; scrolls when content overflows).
pub fn scroll_view(commands: &mut Commands, content: Entity) -> Entity {
    build_scroll(commands, content, None, false, false, None, false)
}

/// Like [`scroll_view`] but scrolls **both** axes, with a horizontal scrollbar in
/// addition to the vertical one. Use for content wider *and* taller than its
/// viewport (e.g. a large tileset atlas). The content should size itself to its
/// natural extent (not `100%`), so both axes can overflow.
pub fn scroll_view_xy(commands: &mut Commands, content: Entity) -> Entity {
    build_scroll(commands, content, None, false, true, None, true)
}

/// Like [`scroll_view`] but the scrollbar stays visible whenever the content
/// overflows (not only while hovered).
pub fn scroll_view_bar(commands: &mut Commands, content: Entity) -> Entity {
    build_scroll(commands, content, None, false, true, None, false)
}

/// Like [`scroll_view`] but auto-follows the bottom as content grows (for logs /
/// chat); releases when the user scrolls up, re-follows at the bottom.
pub fn scroll_view_pinned(commands: &mut Commands, content: Entity) -> Entity {
    build_scroll(commands, content, None, true, false, None, false)
}

/// Wraps `content` in a scrollable viewport capped at `max_height` px.
pub fn scroll_area(commands: &mut Commands, content: Entity, max_height: f32) -> Entity {
    build_scroll(commands, content, Some(max_height), false, false, None, false)
}

/// Like [`scroll_view`] but its offset persists across despawn/rebuild, keyed by
/// `key` in [`ScrollMemory`]. Use a stable, unique key per logical view so two
/// rebuilt instances of the *same* list line up (e.g. `"hierarchy"` or
/// `"status-theme-menu"`); distinct lists must use distinct keys or they'd share
/// (and fight over) one saved offset.
pub fn scroll_view_keyed(
    commands: &mut Commands,
    content: Entity,
    key: impl Into<String>,
) -> Entity {
    build_scroll(commands, content, None, false, false, Some(key.into()), false)
}

/// Like [`scroll_view_bar`] (always-visible bar) but its offset is remembered
/// across rebuilds under `key` (via [`ScrollMemory`]) — so a panel that
/// re-spawns its content doesn't snap the scroll back to the top.
pub fn scroll_view_bar_keyed(
    commands: &mut Commands,
    content: Entity,
    key: impl Into<String>,
) -> Entity {
    build_scroll(commands, content, None, false, true, Some(key.into()), false)
}

/// Like [`scroll_area`] (capped at `max_height`) but its offset persists across
/// despawn/rebuild under `key` — see [`scroll_view_keyed`] for keying rules.
pub fn scroll_area_keyed(
    commands: &mut Commands,
    content: Entity,
    max_height: f32,
    key: impl Into<String>,
) -> Entity {
    build_scroll(commands, content, Some(max_height), false, false, Some(key.into()), false)
}

/// Content height (logical px) of a viewport = its single content child's size.
fn content_h(kids: &Children, computed: &Query<&ComputedNode>, inv: f32) -> f32 {
    kids.iter()
        .next()
        .and_then(|c| computed.get(c).ok())
        .map(|cn| cn.size().y * inv)
        .unwrap_or(0.0)
}

/// Content width (logical px) of a viewport = its single content child's size.
fn content_w(kids: &Children, computed: &Query<&ComputedNode>, inv: f32) -> f32 {
    kids.iter()
        .next()
        .and_then(|c| computed.get(c).ok())
        .map(|cn| cn.size().x * inv)
        .unwrap_or(0.0)
}

/// Wheel over a viewport → nudge its smooth-scroll target. Only the *topmost*
/// scroll area under the cursor scrolls (by `ComputedNode.stack_index`), so the
/// wheel never bleeds through to panels behind it or to a panel beneath an open
/// overlay. While a [`super::overlay::ModalSurface`] is open, only scroll areas
/// inside that modal respond; and any visible [`super::popup::OverlaySurface`]
/// (dropdown / menu / popup) stacked above the candidate swallows the wheel.
pub(crate) fn scroll_wheel(
    mut wheel: MessageReader<MouseWheel>,
    capture: Res<super::drag_value::WheelOverDragValue>,
    // 0.19: the UI stack index moved off `ComputedNode` into its own
    // `ComputedStackIndex(u32)` component.
    mut areas: Query<(Entity, &RelativeCursorPosition, &bevy::ui::ComputedStackIndex, &mut EmberScroll)>,
    overlays: Query<(Entity, &RelativeCursorPosition, &bevy::ui::ComputedStackIndex, &Node), With<super::popup::OverlaySurface>>,
    modals: Query<Entity, With<super::overlay::ModalSurface>>,
    parents: Query<&ChildOf>,
) {
    let mut dy = 0.0;
    for ev in wheel.read() {
        dy += ev.y;
    }
    if dy == 0.0 {
        return;
    }
    // A value field under the cursor claims the wheel (scrubs its value); don't
    // also scroll the panel beneath it.
    if capture.0 {
        return;
    }
    let modal_open = !modals.is_empty();

    // The topmost floating overlay (dropdown / menu / popup) under the cursor, if
    // any. It confines the wheel exactly like a modal: only scroll areas *inside*
    // that overlay may scroll, so an open overlay fully isolates the wheel from
    // the panel behind it — and when the overlay has no scroll area of its own,
    // the wheel is swallowed rather than leaking through. Ancestry, not stack
    // index, decides this, so it's correct even when the panel behind sits higher
    // in the UI tree's stacking order than the floating overlay.
    let top_overlay: Option<Entity> = overlays
        .iter()
        .filter(|(_, rcp, _, node)| rcp.cursor_over && node.display != Display::None)
        .max_by_key(|(_, _, si, _)| si.0)
        .map(|(e, _, _, _)| e);

    // The frontmost scroll area under the cursor (highest stack index) that's
    // allowed to scroll given any open modal / overlay confinement.
    let mut best: Option<(Entity, u32)> = None;
    for (e, rcp, cn, es) in &areas {
        if !rcp.cursor_over {
            continue;
        }
        // Views that opt out of wheel scrolling (image viewers that zoom on the
        // wheel) never claim it.
        if !es.wheel_scroll {
            continue;
        }
        if modal_open && !under_overlay(e, &parents, &modals) {
            continue;
        }
        if let Some(ov) = top_overlay {
            if e != ov && !is_descendant_of(e, ov, &parents) {
                continue;
            }
        }
        let si = cn.0;
        if best.is_none_or(|(_, b)| si >= b) {
            best = Some((e, si));
        }
    }
    // No eligible scroll area — if an overlay is under the cursor it swallows the
    // wheel (returning here leaves the panel behind untouched).
    let Some((target, _)) = best else {
        return;
    };

    if let Ok((_, _, _, mut s)) = areas.get_mut(target) {
        s.target -= dy * WHEEL_STEP;
        s.stick = false; // user took control; scroll_update re-sticks at bottom
    }
}

/// Is `e` a descendant of `ancestor` in the UI tree?
fn is_descendant_of(mut e: Entity, ancestor: Entity, parents: &Query<&ChildOf>) -> bool {
    while let Ok(c) = parents.get(e) {
        let p = c.parent();
        if p == ancestor {
            return true;
        }
        e = p;
    }
    false
}

/// Is `e` itself or any ancestor a [`super::overlay::ModalSurface`]?
fn under_overlay(
    mut e: Entity,
    parents: &Query<&ChildOf>,
    modals: &Query<Entity, With<super::overlay::ModalSurface>>,
) -> bool {
    loop {
        if modals.get(e).is_ok() {
            return true;
        }
        match parents.get(e) {
            Ok(c) => e = c.parent(),
            Err(_) => return false,
        }
    }
}

/// Each frame: clamp the target to the scrollable range, ease the actual scroll
/// toward it, and size/place (or hide) the scrollbar thumb.
pub(crate) fn scroll_update(
    time: Res<Time>,
    dragged: Res<DraggedThumb>,
    mut viewports: Query<(
        &mut EmberScroll,
        &mut ScrollPosition,
        &ComputedNode,
        &Children,
        &RelativeCursorPosition,
    )>,
    computed: Query<&ComputedNode>,
    mut nodes: Query<&mut Node>,
) {
    let lerp = 1.0 - (-time.delta_secs() * EASE).exp();
    for (mut s, mut sp, vcn, kids, rcp) in &mut viewports {
        let inv = vcn.inverse_scale_factor();
        let vh = vcn.size().y * inv;
        let ch = content_h(kids, &computed, inv);
        let max = (ch - vh).max(0.0);

        s.target = s.target.clamp(0.0, max);
        // Auto-follow the bottom (logs only); re-engage once scrolled back near
        // it. Normal scroll views never auto-stick, or they'd jump to the bottom
        // (e.g. on the first frame before content height is measured → max 0).
        if s.stick {
            s.target = max;
        } else if s.pinned && max - s.target < 6.0 {
            s.stick = true;
        }
        let next = if (s.target - sp.y).abs() < 0.5 {
            s.target
        } else {
            sp.y + (s.target - sp.y) * lerp
        };
        if (sp.y - next).abs() > 0.01 {
            sp.y = next;
        }

        // Scrollbar — overlay style: show only while the cursor is over the
        // panel (or the thumb is being dragged), and only if content overflows.
        let dragging = dragged.0.as_ref().is_some_and(|d| d.thumb == s.thumb);
        let show = max > 0.5 && (rcp.cursor_over || dragging || s.always_bar);
        if let Ok(mut track) = nodes.get_mut(s.track) {
            let d = if show { Display::Flex } else { Display::None };
            if track.display != d {
                track.display = d;
            }
        }
        if show {
            if let Ok(mut thumb) = nodes.get_mut(s.thumb) {
                let ratio = (vh / ch).clamp(0.0, 1.0);
                let thumb_h = (vh * ratio).max(MIN_THUMB).min(vh);
                let top = (next / max) * (vh - thumb_h);
                let h = Val::Px(thumb_h);
                let t = Val::Px(top);
                if thumb.height != h {
                    thumb.height = h;
                }
                if thumb.top != t {
                    thumb.top = t;
                }
            }
        }

        // Horizontal scrollbar (both-axis views only) — mirror of the vertical
        // logic above on the X axis.
        if let (Some(h_track), Some(h_thumb)) = (s.h_track, s.h_thumb) {
            let vw = vcn.size().x * inv;
            let cw = content_w(kids, &computed, inv);
            let max_x = (cw - vw).max(0.0);
            s.target_x = s.target_x.clamp(0.0, max_x);
            let next_x = if (s.target_x - sp.x).abs() < 0.5 {
                s.target_x
            } else {
                sp.x + (s.target_x - sp.x) * lerp
            };
            if (sp.x - next_x).abs() > 0.01 {
                sp.x = next_x;
            }
            let h_dragging = dragged.0.as_ref().is_some_and(|d| d.thumb == h_thumb);
            let show_h = max_x > 0.5 && (rcp.cursor_over || h_dragging || s.always_bar);
            if let Ok(mut track) = nodes.get_mut(h_track) {
                let d = if show_h { Display::Flex } else { Display::None };
                if track.display != d {
                    track.display = d;
                }
            }
            if show_h {
                if let Ok(mut thumb) = nodes.get_mut(h_thumb) {
                    let ratio = (vw / cw).clamp(0.0, 1.0);
                    let thumb_w = (vw * ratio).max(MIN_THUMB).min(vw);
                    let left = (next_x / max_x) * (vw - thumb_w);
                    let w = Val::Px(thumb_w);
                    let l = Val::Px(left);
                    if thumb.width != w {
                        thumb.width = w;
                    }
                    if thumb.left != l {
                        thumb.left = l;
                    }
                }
            }
        }
    }
}

/// Drag the thumb to scroll; hover tint. The drag latches on press into
/// [`DraggedThumb`] and runs off [`GlobalCursor`] deltas, so the grip survives
/// the cursor leaving the thumb / track / panel / window until mouse-up.
pub(crate) fn scroll_thumb_drag(
    mut dragged: ResMut<DraggedThumb>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Res<crate::dock::GlobalCursor>,
    thumbs: Query<(Entity, &Interaction, &ScrollThumb)>,
    mut viewports: Query<(&mut EmberScroll, &mut ScrollPosition, &ComputedNode, &Children)>,
    computed: Query<&ComputedNode>,
    mut tints: Query<(Entity, &Interaction, &mut BackgroundColor), With<ScrollThumb>>,
) {
    if mouse.just_released(MouseButton::Left) {
        dragged.0 = None;
    }

    // Latch a thumb on fresh press. Record the physical cursor and the scroll
    // offset now, so from here the drag is a pure delta — the cursor can move off
    // the thumb, off the panel, even out of the window, and the thumb follows.
    if dragged.0.is_none() && mouse.just_pressed(MouseButton::Left) {
        if let Some(pos) = cursor.pos {
            for (entity, interaction, thumb) in &thumbs {
                if *interaction != Interaction::Pressed {
                    continue;
                }
                if let Ok((s, _, _, _)) = viewports.get(thumb.viewport) {
                    let start_offset = if thumb.horizontal { s.target_x } else { s.target };
                    dragged.0 = Some(ThumbDrag { thumb: entity, start_cursor: pos, start_offset });
                }
                break;
            }
        }
    }

    // Apply the active drag: map the physical cursor delta into a scroll offset
    // via the thumb's travel (view minus thumb length), snapping the position so
    // the thumb tracks the cursor 1:1 without easing.
    if let Some(drag) = dragged.0.as_ref() {
        if let (Ok((_, _, thumb)), Some(cur)) = (thumbs.get(drag.thumb), cursor.pos) {
            if let Ok((mut s, mut sp, vcn, kids)) = viewports.get_mut(thumb.viewport) {
                let inv = vcn.inverse_scale_factor();
                if thumb.horizontal {
                    let vw = vcn.size().x * inv;
                    let cw = content_w(kids, &computed, inv);
                    let max = (cw - vw).max(0.0);
                    let ratio = (vw / cw).clamp(0.0, 1.0);
                    let thumb_w = (vw * ratio).max(MIN_THUMB).min(vw);
                    let travel = (vw - thumb_w).max(1.0);
                    let moved = (cur.x - drag.start_cursor.x) * inv;
                    let pos = (drag.start_offset + moved * (max / travel)).clamp(0.0, max);
                    s.target_x = pos;
                    sp.x = pos;
                } else {
                    let vh = vcn.size().y * inv;
                    let ch = content_h(kids, &computed, inv);
                    let max = (ch - vh).max(0.0);
                    let ratio = (vh / ch).clamp(0.0, 1.0);
                    let thumb_h = (vh * ratio).max(MIN_THUMB).min(vh);
                    let travel = (vh - thumb_h).max(1.0);
                    let moved = (cur.y - drag.start_cursor.y) * inv;
                    let pos = (drag.start_offset + moved * (max / travel)).clamp(0.0, max);
                    s.target = pos;
                    s.stick = false;
                    sp.y = pos;
                }
            }
        } else {
            // Thumb despawned mid-drag (e.g. panel rebuilt) — drop the latch.
            dragged.0 = None;
        }
    }

    // Keep the dragged thumb lit even once the cursor leaves it, since its
    // `Interaction` falls back to `None` off-node while the grip still holds.
    for (entity, interaction, mut bg) in &mut tints {
        let active = matches!(interaction, Interaction::Hovered | Interaction::Pressed)
            || dragged.0.as_ref().is_some_and(|d| d.thumb == entity);
        let target = if active { rgb(text_primary()) } else { rgb(text_muted()) };
        if bg.0 != target {
            bg.0 = target;
        }
    }
}

/// Restore a keyed view's saved offset once, after its content has been measured.
/// Runs *before* [`scroll_update`]: while the content height is still 0 (the
/// freshly-spawned, not-yet-laid-out frames) it just holds `target` at the saved
/// value so `scroll_update`'s clamp-to-range can't discard it; the moment the
/// content has real height it snaps both the target and the live position to the
/// (clamped) saved offset and marks itself done.
pub(crate) fn scroll_restore(
    memory: Res<ScrollMemory>,
    mut viewports: Query<(
        &mut EmberScroll,
        &mut ScrollPosition,
        &ComputedNode,
        &Children,
        &mut ScrollKey,
    )>,
    computed: Query<&ComputedNode>,
) {
    for (mut s, mut sp, vcn, kids, mut key) in &mut viewports {
        if key.restored {
            continue;
        }
        let Some(&saved) = memory.0.get(&key.key) else {
            // Nothing remembered for this key — nothing to restore.
            key.restored = true;
            continue;
        };
        let inv = vcn.inverse_scale_factor();
        let ch = content_h(kids, &computed, inv);
        if ch <= 0.0 {
            // Not laid out yet: keep the intent alive against the clamp, retry.
            s.target = saved;
            continue;
        }
        let max = (ch - vcn.size().y * inv).max(0.0);
        let pos = saved.clamp(0.0, max);
        s.target = pos;
        sp.y = pos; // snap (no ease) so there's no visible scroll-from-top
        key.restored = true;
    }
}

/// Persist each keyed view's intended offset into [`ScrollMemory`] so a later
/// rebuild can restore it. Saves the smooth-scroll `target` (the user's intent),
/// not the mid-ease position, and only once the one-shot restore has run so the
/// pre-layout 0 never clobbers a real saved value.
pub(crate) fn scroll_persist(
    mut memory: ResMut<ScrollMemory>,
    viewports: Query<(&EmberScroll, &ScrollKey)>,
) {
    for (s, key) in &viewports {
        if !key.restored {
            continue;
        }
        if memory.0.get(&key.key).copied() != Some(s.target) {
            memory.0.insert(key.key.clone(), s.target);
        }
    }
}
