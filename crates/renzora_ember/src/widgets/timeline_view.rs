//! A **reusable, data-driven** timeline canvas: a fixed left track-header column,
//! a time ruler, flat keyframe/clip lanes, a playhead and a click/drag scrub
//! layer — all sharing one time→pixel mapping driven by the caller's own state.
//!
//! Lanes are flat, split by a faint hairline, not striped odd/even: the
//! stripes made a dense dopesheet read as a grid of bands competing with the
//! keys, and they had to be rebuilt whenever rows scrolled past each other.
//!
//! Unlike the self-contained [`crate::widgets::timeline`] demo (which owns its
//! pps/scroll/playhead), `timeline_view` is a *shell*: the caller mounts its own
//! track headers + clips/keyframes into the exposed host entities and drives the
//! geometry through the [`TimelineView`] component (zoom / scroll / playhead /
//! duration / track count + height). The widget reads that component to lay out
//! the ruler ticks, lane stripes and playhead, and reports scrubbing back via
//! `TimelineView::scrub_out`. Every colour + the header/ruler geometry comes from
//! the themeable [`crate::style::TimelineStyle`], so timelines re-skin with the
//! active theme like the dock.
//!
//! Backing panel: the animation-editor timeline.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition, UiTransform};

use crate::font::{ui_font, EmberFonts};
use crate::reactive::{KeyedSnapshot, Rx};
use crate::reactive::tracked::{bind_bg, keyed_list};
use crate::style::{Rgba, Theme, TimelineStyle};
use crate::theme::{border, header_bg, rgb, section_bg};
use crate::interaction::PASS_THROUGH;
use crate::UiInteraction;

/// Left gutter (px) between the lane pane's edge and `t=0`, so the playhead and
/// the `t=0` keyframe aren't flush against (and clipped by) the header column.
/// The ruler, playhead, scrub mapping AND the caller's keyframe placement all
/// add this, so everything stays aligned. 24 rather than the 14 it was: at 14
/// the loop region's start handle sat half under the header column and time
/// zero read as jammed against it.
pub const LANE_INSET: f32 = 24.0;

/// Live geometry of a [`timeline_view`], owned on its root entity. The caller
/// writes the fields each frame from its own state (via [`TimelineView::set_geom`])
/// and drains [`TimelineView::scrub_out`] to apply user scrubbing.
#[derive(Component, Default)]
pub struct TimelineView {
    /// Pixels per second.
    pub zoom: f32,
    /// Left edge of the visible window, seconds.
    pub scroll: f32,
    /// Playhead position, seconds.
    pub playhead: f32,
    /// Total content length, seconds.
    pub duration: f32,
    /// Per-track row height, px.
    pub track_height: f32,
    /// Number of track rows.
    pub track_count: usize,
    /// Vertical scroll of the rows, px: row `i` sits at `i * track_height -
    /// vscroll`. The caller owns it (and offsets its own rows to match); the
    /// widget only uses it to place the lane separators.
    pub vscroll: f32,
    /// Set by the widget when the user scrubs; the caller takes it and applies.
    pub scrub_out: Option<f32>,
    /// Header column width the user dragged the splitter to, px. `None`
    /// follows the theme's [`TimelineStyle::header_width`].
    pub header_width: Option<f32>,
}

/// Header column width limits for the splitter, px.
const HEADER_MIN: f32 = 120.0;
const HEADER_MAX: f32 = 640.0;

impl TimelineView {
    /// Push the caller's current geometry in one call.
    pub fn set_geom(
        &mut self,
        zoom: f32,
        scroll: f32,
        playhead: f32,
        duration: f32,
        track_height: f32,
        track_count: usize,
    ) {
        self.zoom = zoom;
        self.scroll = scroll;
        self.playhead = playhead;
        self.duration = duration;
        self.track_height = track_height;
        self.track_count = track_count;
    }

    /// Take a pending scrub request (the time the user dragged the playhead to).
    pub fn take_scrub(&mut self) -> Option<f32> {
        self.scrub_out.take()
    }
}

/// Entities the caller mounts content into.
pub struct TimelineHandle {
    /// Root row carrying the [`TimelineView`] component (add a marker + sync it).
    pub root: Entity,
    /// Top-left corner cell of the header column (add a title / add-track button).
    pub header_corner: Entity,
    /// Column the caller fills with one header row per track (e.g. `keyed_list`).
    pub header_list: Entity,
    /// Absolute layer over the lanes — mount positioned clips / keyframes here.
    pub clips: Entity,
    /// Absolute layer over the whole pane (above the scrub surface) — mount
    /// event markers / flags here so they stay clickable.
    pub markers: Entity,
}

#[derive(Component)]
struct TlPlayhead {
    root: Entity,
}
/// The transparent surface over the lanes that turns a press into a scrub.
/// Public so a caller that knows what is under the cursor (a key, a handle,
/// empty space) can set this surface's `HoverCursor` to match.
#[derive(Component)]
pub struct TlScrub {
    root: Entity,
}
#[derive(Component)]
struct TlHeaderCol {
    root: Entity,
}
#[derive(Component)]
struct TlRulerRow;
/// The splitter between the header column and the lanes. `grab` holds the
/// cursor x and column width at the press while a drag is under way.
#[derive(Component)]
struct TlSplitter {
    root: Entity,
    grab: Option<(f32, f32)>,
}

/// Build a timeline shell. Returns the host entities the caller fills.
pub fn timeline_view(commands: &mut Commands, _fonts: &EmberFonts) -> TimelineHandle {
    let st = TimelineStyle::default();

    let root = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            TimelineView::default(),
            Name::new("timeline-view"),
        ))
        .id();

    // ── Fixed left header column ──
    let header_col = commands
        .spawn((
            Node {
                width: Val::Px(st.header_width),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                // Rows scroll vertically with the lanes (the caller offsets
                // `header_list`), so the column clips what scrolls past it.
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            TlHeaderCol { root },
        ))
        .id();
    let header_corner = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(st.ruler_height),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(header_bg())),
            TlRulerRow,
        ))
        .id();
    let header_list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            position_type: PositionType::Relative,
            ..default()
        })
        .id();
    // The caller scrolls rows by nudging `header_list` up by part of a row, so
    // the rows need a clip of their own under the corner: clipping only the
    // column let the top row slide over the filter and "+ Track".
    let header_rows = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            overflow: Overflow::clip(),
            ..default()
        })
        .id();
    commands.entity(header_rows).add_child(header_list);
    commands.entity(header_col).add_children(&[header_corner, header_rows]);

    // The separator doubles as a resize handle: an 8px grab strip around a
    // 3px line (a hairline was too thin to find), which lights in the accent while hovered or dragged. A strip
    // overhanging its neighbours would lose its right half to the pane's scrub
    // layer, which is drawn later, so it takes its own few pixels instead.
    let sep = commands
        .spawn((
            Node { width: Val::Px(8.0), height: Val::Percent(100.0), flex_shrink: 0.0, justify_content: JustifyContent::Center, ..default() },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::EwResize),
            TlSplitter { root, grab: None },
            Name::new("timeline-view-splitter"),
        ))
        .id();
    let sep_line = commands
        .spawn((Node { width: Val::Px(3.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(rgb(border())), PASS_THROUGH))
        .id();
    bind_bg(commands, sep_line, move |w| {
        let live = w.get::<TlSplitter>(sep).is_some_and(|s| s.grab.is_some())
            || matches!(w.get::<UiInteraction>(sep), Some(UiInteraction::Hovered | UiInteraction::Pressed));
        rgb(if live { crate::theme::accent() } else { border() })
    });
    commands.entity(sep).add_child(sep_line);

    // ── Right timeline pane ──
    let pane = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                ..default()
            },
            Name::new("timeline-view-pane"),
        ))
        .id();

    let ruler = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(st.ruler_height),
                flex_shrink: 0.0,
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(st.ruler_bg.color()),
            TlRulerRow,
        ))
        .id();
    bind_bg(commands, ruler, |w| tl_style(w).ruler_bg.color());
    keyed_list(commands, ruler, move |w| ruler_snapshot(w, root));

    let lanes = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            position_type: PositionType::Relative,
            overflow: Overflow::clip(),
            ..default()
        })
        .id();
    let lanes_bg = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(0.0),
            left: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        })
        .id();
    keyed_list(commands, lanes_bg, move |w| lane_bg_snapshot(w, root));
    // Vertical gridlines (behind keyframes), aligned with the ruler ticks.
    let gridlines = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(0.0),
            left: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        })
        .id();
    keyed_list(commands, gridlines, move |w| gridlines_snapshot(w, root));
    let clips = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(0.0),
            left: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        })
        .id();
    commands.entity(lanes).add_children(&[lanes_bg, gridlines, clips]);

    let playhead = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Px(1.5),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(st.playhead.color()),
            TlPlayhead { root },
            PASS_THROUGH,
            Name::new("timeline-view-playhead"),
        ))
        .id();
    bind_bg(commands, playhead, |w| tl_style(w).playhead.color());

    // Grab handle at the top of the needle (a rounded tab + a downward point).
    let handle = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(-5.25),
                width: Val::Px(12.0),
                height: Val::Px(9.0),
                border_radius: BorderRadius::all(Val::Px(2.5)),
                ..default()
            },
            BackgroundColor(st.playhead.color()),
            PASS_THROUGH,
        ))
        .id();
    let point = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(6.0),
                left: Val::Px(-2.75),
                width: Val::Px(7.0),
                height: Val::Px(7.0),
                ..default()
            },
            BackgroundColor(st.playhead.color()),
            UiTransform::from_rotation(Rot2::degrees(45.0)),
            PASS_THROUGH,
        ))
        .id();
    bind_bg(commands, handle, |w| tl_style(w).playhead.color());
    bind_bg(commands, point, |w| tl_style(w).playhead.color());
    commands.entity(playhead).add_children(&[handle, point]);

    let scrub = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            RelativeCursorPosition::default(),
            // A plain cursor by default. A grab hand over the whole surface
            // said "drag me" over empty space where a drag does not move
            // anything; a caller that hit-tests its keys sets `Grab` over them.
            crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Default),
            TlScrub { root },
            Name::new("timeline-view-scrub"),
        ))
        .id();
    // Markers overlay: above the scrub surface so flags stay clickable.
    let markers = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            PASS_THROUGH,
            Name::new("timeline-view-markers"),
        ))
        .id();
    commands.entity(pane).add_children(&[ruler, lanes, playhead, scrub, markers]);

    commands.entity(root).add_children(&[header_col, sep, pane]);
    TimelineHandle { root, header_corner, header_list, clips, markers }
}

// ── Generic snapshots (driven by the root's `TimelineView`) ───────────────────

fn ruler_snapshot(world: &Rx, root: Entity) -> KeyedSnapshot {
    let Some(v) = world.get::<TimelineView>(root) else { return empty() };
    let (zoom, scroll) = (v.zoom.max(1.0), v.scroll);
    let st = tl_style(world);
    let ruler_h = st.ruler_height;
    let (major_col, minor_col) = (st.tick_major.color(), st.tick_minor.color());

    let (ticks, label_step) = timeline_ticks(zoom, scroll);
    let items: Vec<(u64, u64)> = ticks
        .iter()
        .map(|(time, major)| {
            let mut k = hasher();
            time.to_bits().hash(&mut k);
            let mut h = hasher();
            (zoom.to_bits(), scroll.to_bits(), major, ruler_h.to_bits(), rgba_key(st.tick_major)).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (time, major) = ticks[i];
            let x = (time - scroll) * zoom + LANE_INSET;
            let (col, th) = if major { (major_col, 9.0) } else { (minor_col, 4.0) };
            let tick = c
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(x),
                        top: Val::Px(0.0),
                        width: Val::Px(1.0),
                        height: Val::Px(ruler_h),
                        ..default()
                    },
                    PASS_THROUGH,
                ))
                .id();
            let mark = c
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        bottom: Val::Px(0.0),
                        left: Val::Px(0.0),
                        width: Val::Px(1.0),
                        height: Val::Px(th),
                        ..default()
                    },
                    BackgroundColor(col),
                ))
                .id();
            c.entity(tick).add_child(mark);
            if major {
                let label = if label_step < 0.2 {
                    format!("f{}", (time * 30.0).round() as i64)
                } else if time >= 60.0 {
                    format!("{}:{:04.1}", (time / 60.0) as u32, time % 60.0)
                } else if label_step < 1.0 {
                    format!("{:.2}s", time)
                } else {
                    format!("{:.0}s", time)
                };
                let lbl = c
                    .spawn((
                        Text::new(label),
                        ui_font(&f.ui, 9.0),
                        TextColor(major_col),
                        Node { position_type: PositionType::Absolute, top: Val::Px(1.0), left: Val::Px(2.0), ..default() },
                    ))
                    .id();
                c.entity(tick).add_child(lbl);
            }
            tick
        }),
    }
}

/// Shared tick generation for the ruler and the lane gridlines, so the two stay
/// aligned. Returns `(ticks, label_step)` where each tick is `(time, is_major)`;
/// major (labeled) ticks land on round `label_step`-second boundaries, with
/// `divs` minor gridlines between each. `label_step` drives the label format.
///
/// Public so a caller drawing its own grid over the lanes (the animation
/// timeline's curves view) lines up with the ruler exactly.
///
/// Ticks run across the whole visible window, not just the content's length.
/// They used to stop one tick past the duration, so dragging a clip's end
/// handle inward wiped every time label to its right as it went, and the
/// ruler past the end was blank exactly where the handle was being dragged
/// back out to.
pub fn timeline_ticks(zoom: f32, scroll: f32) -> (Vec<(f32, bool)>, f32) {
    // Labeled spacing (seconds) — a round number scaled to keep labels readable.
    let label_step = if zoom >= 1400.0 {
        1.0 / 30.0 // per-frame
    } else if zoom >= 700.0 {
        0.1
    } else if zoom >= 360.0 {
        0.25
    } else if zoom >= 160.0 {
        0.5
    } else if zoom >= 80.0 {
        1.0
    } else if zoom >= 40.0 {
        2.0
    } else if zoom >= 16.0 {
        5.0
    } else {
        10.0
    };
    // Minor gridlines between labels.
    let divs: i64 = if zoom >= 80.0 { 4 } else { 2 };
    let minor = label_step / divs as f32;
    // Index-based generation avoids float-modulo drift; major = on a label boundary.
    let mut ticks: Vec<(f32, bool)> = Vec::new();
    let start_i = (scroll / minor).floor() as i64;
    let end = scroll + 4000.0 / zoom;
    let mut i = start_i;
    loop {
        let t = i as f32 * minor;
        if t > end {
            break;
        }
        if t >= 0.0 {
            ticks.push((t, i.rem_euclid(divs) == 0));
        }
        i += 1;
    }
    (ticks, label_step)
}

/// Vertical gridlines down the lanes, aligned with the ruler ticks. Major ticks
/// are slightly brighter than minor ones; both are dim so keyframes stay legible.
fn gridlines_snapshot(world: &Rx, root: Entity) -> KeyedSnapshot {
    let Some(v) = world.get::<TimelineView>(root) else { return empty() };
    let (zoom, scroll) = (v.zoom.max(1.0), v.scroll);
    let st = tl_style(world);
    let (ticks, _) = timeline_ticks(zoom, scroll);
    let major = st.tick_major.color().with_alpha(0.16);
    let minor = st.tick_minor.color().with_alpha(0.07);
    let items: Vec<(u64, u64)> = ticks
        .iter()
        .map(|(time, is_major)| {
            let mut k = hasher();
            time.to_bits().hash(&mut k);
            let mut h = hasher();
            (zoom.to_bits(), scroll.to_bits(), is_major, rgba_key(st.tick_major)).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, _f, i| {
            let (time, is_major) = ticks[i];
            let x = (time - scroll) * zoom + LANE_INSET;
            c.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(x),
                    top: Val::Px(0.0),
                    width: Val::Px(1.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(if is_major { major } else { minor }),
                PASS_THROUGH,
            ))
            .id()
        }),
    }
}

/// One faint hairline under each row, placed at `(i + 1) * th - vscroll`.
/// The theme's `lane_even` / `lane_odd` are no longer painted (see the module
/// doc); they stay in [`TimelineStyle`] so themes written against them load.
fn lane_bg_snapshot(world: &Rx, root: Entity) -> KeyedSnapshot {
    let Some(v) = world.get::<TimelineView>(root) else { return empty() };
    let (n, th, vs) = (v.track_count, v.track_height.max(1.0), v.vscroll.max(0.0));
    let line = crate::theme::rgb(crate::theme::text_primary()).with_alpha(0.035);
    let items: Vec<(u64, u64)> = (0..n)
        .map(|i| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            (th.to_bits(), vs.to_bits()).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, _f, i| {
            c.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px((i as f32 + 1.0) * th - vs - 1.0),
                    width: Val::Percent(100.0),
                    height: Val::Px(1.0),
                    ..default()
                },
                BackgroundColor(line),
                PASS_THROUGH,
            ))
            .id()
        }),
    }
}

// ── Systems ───────────────────────────────────────────────────────────────────

/// Position every playhead from its root's [`TimelineView`].
fn timeline_view_playhead(
    views: Query<&TimelineView>,
    phs: Query<(Entity, &TlPlayhead)>,
    mut nodes: Query<&mut Node>,
) {
    for (e, ph) in &phs {
        let Ok(v) = views.get(ph.root) else { continue };
        let raw = (v.playhead - v.scroll) * v.zoom;
        let x = raw + LANE_INSET;
        if let Ok(mut n) = nodes.get_mut(e) {
            let l = Val::Px(x);
            if n.left != l {
                n.left = l;
            }
            // Hide only when the playhead is left of the window start.
            let d = if raw >= 0.0 { Display::Flex } else { Display::None };
            if n.display != d {
                n.display = d;
            }
        }
    }
}

/// Map cursor-x on the scrub layer → time, written to `scrub_out` for the caller.
fn timeline_view_scrub(
    scrubs: Query<(&UiInteraction, &RelativeCursorPosition, &ComputedNode, &TlScrub)>,
    mut views: Query<&mut TimelineView>,
) {
    for (interaction, rcp, cn, s) in &scrubs {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        let Some(norm) = rcp.normalized else { continue };
        let Ok(mut v) = views.get_mut(s.root) else { continue };
        let width = cn.size().x * cn.inverse_scale_factor();
        // bevy's `normalized` is centered: (-0.5,-0.5) top-left .. (0.5,0.5) bottom-right.
        let px = (norm.x + 0.5) * width;
        let t = (px - LANE_INSET).max(0.0) / v.zoom.max(1.0) + v.scroll;
        v.scrub_out = Some(t.clamp(0.0, v.duration.max(0.0)));
    }
}

/// Apply the themeable header-column width + ruler height live.
fn timeline_view_geometry(
    theme: Option<Res<Theme>>,
    views: Query<&TimelineView>,
    mut cols: Query<(&mut Node, &TlHeaderCol), Without<TlRulerRow>>,
    mut rulers: Query<&mut Node, With<TlRulerRow>>,
) {
    let st = theme.map(|t| t.timeline.clone()).unwrap_or_default();
    for (mut n, col) in &mut cols {
        let w = Val::Px(views.get(col.root).ok().and_then(|v| v.header_width).unwrap_or(st.header_width));
        if n.width != w {
            n.width = w;
        }
    }
    let h = Val::Px(st.ruler_height);
    for mut n in &mut rulers {
        if n.height != h {
            n.height = h;
        }
    }
}

/// Drag the splitter to resize the header column. Tracked against the raw
/// mouse button once grabbed, so the drag keeps going when the cursor runs
/// ahead of the 5px strip.
fn timeline_view_splitter(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    theme: Option<Res<Theme>>,
    ui_scale: Option<Res<UiScale>>,
    mut splitters: Query<(&UiInteraction, &mut TlSplitter)>,
    mut views: Query<&mut TimelineView>,
) {
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else { return };
    let scale = ui_scale.map(|s| s.0).unwrap_or(1.0).max(0.01);
    let theme_w = theme.map(|t| t.timeline.header_width).unwrap_or_else(|| TimelineStyle::default().header_width);
    for (interaction, mut sp) in &mut splitters {
        let Ok(mut view) = views.get_mut(sp.root) else { continue };
        if !mouse.pressed(MouseButton::Left) {
            if sp.grab.is_some() {
                sp.grab = None;
            }
            continue;
        }
        if sp.grab.is_none() && *interaction == UiInteraction::Pressed && mouse.just_pressed(MouseButton::Left) {
            sp.grab = Some((cursor.x, view.header_width.unwrap_or(theme_w)));
        }
        if let Some((x0, w0)) = sp.grab {
            let w = (w0 + (cursor.x - x0) / scale).clamp(HEADER_MIN, HEADER_MAX);
            if view.header_width != Some(w) {
                view.header_width = Some(w);
            }
        }
    }
}

pub(crate) struct TimelineViewPlugin;

impl Plugin for TimelineViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (timeline_view_playhead, timeline_view_scrub, timeline_view_splitter, timeline_view_geometry).chain(),
        );
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn tl_style(w: &Rx) -> TimelineStyle {
    w.get_resource::<Theme>().map(|t| t.timeline.clone()).unwrap_or_default()
}

fn rgba_key(c: Rgba) -> u32 {
    ((c.r as u32) << 24) | ((c.g as u32) << 16) | ((c.b as u32) << 8) | c.a as u32
}

fn empty() -> KeyedSnapshot {
    KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) }
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}
