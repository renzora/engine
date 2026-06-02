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

use crate::theme::rgb;

const BAR_W: f32 = 9.0;
const THUMB: (u8, u8, u8) = (96, 96, 112);
const THUMB_HOVER: (u8, u8, u8) = (128, 128, 148);
const TRACK: (u8, u8, u8) = (26, 26, 32);
const MIN_THUMB: f32 = 28.0;
const WHEEL_STEP: f32 = 52.0;
const EASE: f32 = 16.0;

/// The scrolling viewport. Holds the smooth-scroll target + scrollbar handles.
#[derive(Component)]
pub struct EmberScroll {
    target: f32,
    /// Auto-follow the bottom (for logs). Disabled when the user scrolls up;
    /// re-enabled when they return to the bottom.
    stick: bool,
    thumb: Entity,
    track: Entity,
}

/// The draggable scrollbar thumb; points back at its viewport + track.
#[derive(Component)]
pub struct ScrollThumb {
    viewport: Entity,
    track: Entity,
}

fn build_scroll(
    commands: &mut Commands,
    content: Entity,
    max_height: Option<f32>,
    stick: bool,
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
                overflow: Overflow::scroll_y(),
                ..default()
            },
            RelativeCursorPosition::default(),
            ScrollPosition::default(),
            // Pressing empty/label areas lands on the viewport (interactive
            // widgets capture their own press), enabling grab-to-scroll.
            Interaction::default(),
            Name::new("scroll-viewport"),
        ))
        .id();
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
            BackgroundColor(rgb(TRACK).with_alpha(0.5)),
            RelativeCursorPosition::default(),
            GlobalZIndex(50),
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
            BackgroundColor(rgb(THUMB)),
            Interaction::default(),
            ScrollThumb { viewport, track },
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("scroll-thumb"),
        ))
        .id();
    commands.entity(track).add_child(thumb);
    commands.entity(viewport).insert(EmberScroll {
        target: 0.0,
        stick,
        thumb,
        track,
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
    commands.entity(outer).add_children(&[viewport, track]);
    outer
}

/// Wraps `content` in a flex-filling scrollable viewport (grows to fill its
/// parent; scrolls when content overflows).
pub fn scroll_view(commands: &mut Commands, content: Entity) -> Entity {
    build_scroll(commands, content, None, false)
}

/// Like [`scroll_view`] but auto-follows the bottom as content grows (for logs /
/// chat); releases when the user scrolls up, re-follows at the bottom.
pub fn scroll_view_pinned(commands: &mut Commands, content: Entity) -> Entity {
    build_scroll(commands, content, None, true)
}

/// Wraps `content` in a scrollable viewport capped at `max_height` px.
pub fn scroll_area(commands: &mut Commands, content: Entity, max_height: f32) -> Entity {
    build_scroll(commands, content, Some(max_height), false)
}

/// Content height (logical px) of a viewport = its single content child's size.
fn content_h(kids: &Children, computed: &Query<&ComputedNode>, inv: f32) -> f32 {
    kids.iter()
        .next()
        .and_then(|c| computed.get(c).ok())
        .map(|cn| cn.size().y * inv)
        .unwrap_or(0.0)
}

/// Wheel over a viewport → nudge its smooth-scroll target.
pub(crate) fn scroll_wheel(
    mut wheel: MessageReader<MouseWheel>,
    mut areas: Query<(&RelativeCursorPosition, &mut EmberScroll)>,
) {
    let mut dy = 0.0;
    for ev in wheel.read() {
        dy += ev.y;
    }
    if dy == 0.0 {
        return;
    }
    for (rcp, mut s) in &mut areas {
        if rcp.cursor_over {
            s.target -= dy * WHEEL_STEP;
            s.stick = false; // user took control; scroll_update re-sticks at bottom
        }
    }
}

/// Each frame: clamp the target to the scrollable range, ease the actual scroll
/// toward it, and size/place (or hide) the scrollbar thumb.
pub(crate) fn scroll_update(
    time: Res<Time>,
    mut viewports: Query<(
        &mut EmberScroll,
        &mut ScrollPosition,
        &ComputedNode,
        &Children,
        &RelativeCursorPosition,
    )>,
    computed: Query<&ComputedNode>,
    interactions: Query<&Interaction>,
    mut nodes: Query<&mut Node>,
) {
    let lerp = 1.0 - (-time.delta_secs() * EASE).exp();
    for (mut s, mut sp, vcn, kids, rcp) in &mut viewports {
        let inv = vcn.inverse_scale_factor();
        let vh = vcn.size().y * inv;
        let ch = content_h(kids, &computed, inv);
        let max = (ch - vh).max(0.0);

        s.target = s.target.clamp(0.0, max);
        // Auto-follow the bottom (logs); re-engage once scrolled back near it.
        if s.stick {
            s.target = max;
        } else if max - s.target < 6.0 {
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
        let dragging = interactions
            .get(s.thumb)
            .is_ok_and(|i| *i == Interaction::Pressed);
        let show = max > 0.5 && (rcp.cursor_over || dragging);
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
    }
}

/// The viewport currently being grab-scrolled (press-drag on its content).
#[derive(Resource, Default)]
pub(crate) struct ScrollGrab {
    viewport: Option<Entity>,
    last_y: f32,
}

/// Grab-to-scroll: press and drag on a panel's non-interactive content (gaps,
/// labels — *not* text inputs/buttons/sliders, which capture their own press)
/// to scroll it, content tracking the cursor 1:1.
pub(crate) fn scroll_grab(
    mut grab: ResMut<ScrollGrab>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    pressed: Query<(Entity, &Interaction), With<EmberScroll>>,
    mut areas: Query<(&mut EmberScroll, &mut ScrollPosition, &ComputedNode, &Children)>,
    computed: Query<&ComputedNode>,
) {
    let cursor_y = windows
        .single()
        .ok()
        .and_then(|w| w.cursor_position())
        .map(|p| p.y);

    if mouse.just_released(MouseButton::Left) {
        grab.viewport = None;
    }
    if grab.viewport.is_none() && mouse.just_pressed(MouseButton::Left) {
        if let Some(cy) = cursor_y {
            for (e, interaction) in &pressed {
                if *interaction == Interaction::Pressed {
                    grab.viewport = Some(e);
                    grab.last_y = cy;
                    break;
                }
            }
        }
    }

    let (Some(vp), Some(cy)) = (grab.viewport, cursor_y) else {
        return;
    };
    let dy = grab.last_y - cy; // content follows the cursor (natural drag)
    grab.last_y = cy;
    if dy == 0.0 {
        return;
    }
    if let Ok((mut s, mut sp, vcn, kids)) = areas.get_mut(vp) {
        let inv = vcn.inverse_scale_factor();
        let vh = vcn.size().y * inv;
        let ch = content_h(kids, &computed, inv);
        let max = (ch - vh).max(0.0);
        s.target = (s.target + dy).clamp(0.0, max);
        s.stick = false;
        sp.y = s.target;
    }
}

/// Drag the thumb to scroll; hover tint.
pub(crate) fn scroll_thumb_drag(
    thumbs: Query<(&Interaction, &ScrollThumb)>,
    tracks: Query<&RelativeCursorPosition>,
    mut viewports: Query<(&mut EmberScroll, &mut ScrollPosition, &ComputedNode, &Children)>,
    computed: Query<&ComputedNode>,
    mut tints: Query<(&Interaction, &mut BackgroundColor), With<ScrollThumb>>,
) {
    for (interaction, thumb) in &thumbs {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Ok(rcp) = tracks.get(thumb.track) else {
            continue;
        };
        let Some(n) = rcp.normalized else {
            continue;
        };
        let frac = (n.y + 0.5).clamp(0.0, 1.0);
        if let Ok((mut s, mut sp, vcn, kids)) = viewports.get_mut(thumb.viewport) {
            let inv = vcn.inverse_scale_factor();
            let vh = vcn.size().y * inv;
            let ch = content_h(kids, &computed, inv);
            let max = (ch - vh).max(0.0);
            let pos = frac * max;
            s.target = pos;
            s.stick = false;
            // Snap directly (no easing) so the drag tracks the cursor 1:1.
            sp.y = pos;
        }
    }
    for (interaction, mut bg) in &mut tints {
        let target = match interaction {
            Interaction::Hovered | Interaction::Pressed => rgb(THUMB_HOVER),
            Interaction::None => rgb(THUMB),
        };
        if bg.0 != target {
            bg.0 = target;
        }
    }
}
