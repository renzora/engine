//! Timeline — a general ruler/tracks/playhead widget for video editing,
//! sequencing, audio regions and keyframe animation.
//!
//! A track is either a **clip** lane (ranged draggable blocks — video/audio
//! regions, sequencer events) or a **keyframe** lane (draggable diamond markers).
//! A fixed left column holds the track headers; the right side is a horizontally
//! pannable (middle-drag) / zoomable (wheel) time area sharing one time→pixel
//! mapping. Scrub the playhead by dragging in the ruler; drag clips/keys to move
//! them. All positions derive from time, so zoom/pan/scrub stay consistent.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition, UiTransform};
use bevy::window::SystemCursorIcon;

use crate::font::{ui_font, EmberFonts};
use crate::theme::*;

const HEADER_W: f32 = 110.0;
const RULER_H: f32 = 22.0;
const TRACK_H: f32 = 34.0;
const CLIP_H: f32 = 22.0;
const INITIAL_PPS: f32 = 70.0;
const SNAP: f32 = 0.25;
const MIN_CLIP: f32 = 0.25;

fn snap(t: f32) -> f32 {
    (t / SNAP).round() * SNAP
}

/// A track's content: ranged clips or point keyframes.
pub enum Lane<'a> {
    /// `(start_sec, length_sec, label)` blocks.
    Clips(&'a [(f32, f32, &'a str)]),
    /// keyframe times in seconds.
    Keys(&'a [f32]),
}

/// One timeline track.
pub struct Track<'a> {
    pub name: &'a str,
    pub color: (u8, u8, u8),
    pub lane: Lane<'a>,
}

/// Registers the timeline interaction systems.
pub(crate) struct TimelinePlugin;

impl Plugin for TimelinePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                timeline_layout,
                timeline_zoom,
                timeline_pan,
                timeline_scrub,
                timeline_drag,
            ),
        );
    }
}

#[derive(Component)]
pub(crate) struct TimelineRoot {
    pps: f32,
    scroll: f32,
    duration: f32,
    playhead: f32,
}

#[derive(Component)]
pub(crate) struct TlCanvas {
    root: Entity,
}

#[derive(Component)]
pub(crate) struct TlLanes {
    root: Entity,
}

#[derive(Component)]
pub(crate) struct TlRuler {
    root: Entity,
}

#[derive(Component)]
pub(crate) struct TlTick {
    time: f32,
}

#[derive(Component)]
pub(crate) struct TlClip {
    root: Entity,
    start: f32,
    len: f32,
}

#[derive(Component)]
pub(crate) struct TlKey {
    root: Entity,
    time: f32,
}

#[derive(Component)]
pub(crate) struct TlPlayhead {
    root: Entity,
}

#[derive(Clone, Copy)]
pub(crate) enum Drag {
    ClipMove,
    ClipLeft,
    ClipRight,
    Key,
}

fn cursor(windows: &Query<&Window>) -> Option<Vec2> {
    windows.single().ok().and_then(|w| w.cursor_position())
}

fn fmt_time(t: f32) -> String {
    let s = t.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Build a timeline of `duration_sec` with the given `tracks`.
pub fn timeline(commands: &mut Commands, fonts: &EmberFonts, duration_sec: f32, tracks: &[Track]) -> Entity {
    let n = tracks.len();
    let height = RULER_H + n as f32 * TRACK_H;
    let root = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                width: Val::Percent(100.0),
                height: Val::Px(height),
                overflow: Overflow::clip(),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            BorderColor::all(rgb(border())),
            TimelineRoot {
                pps: INITIAL_PPS,
                scroll: 0.0,
                duration: duration_sec,
                playhead: duration_sec * 0.2,
            },
            Name::new("timeline"),
        ))
        .id();

    // ── Left header column ──
    let left = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                width: Val::Px(HEADER_W),
                flex_shrink: 0.0,
                border: UiRect::right(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(rgb(border())),
            Name::new("tl-headers"),
        ))
        .id();
    let corner = commands
        .spawn((
            Node {
                height: Val::Px(RULER_H),
                align_items: AlignItems::Center,
                padding: UiRect::left(Val::Px(8.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(rgb(header_bg())),
            BorderColor::all(rgb(border())),
            Name::new("tl-corner"),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("Tracks"),
                ui_font(&fonts.ui, 10.0),
                TextColor(rgb(text_muted())),
            ));
        })
        .id();
    commands.entity(left).add_child(corner);
    for track in tracks {
        let header = commands
            .spawn((
                Node {
                    height: Val::Px(TRACK_H),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(6.0),
                    padding: UiRect::left(Val::Px(8.0)),
                    border: UiRect::bottom(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(rgb(panel_bg())),
                BorderColor::all(rgb(hover_bg())),
                Name::new("tl-header"),
            ))
            .id();
        let dot = commands
            .spawn((
                Node {
                    width: Val::Px(8.0),
                    height: Val::Px(8.0),
                    border_radius: BorderRadius::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(rgb(track.color)),
                Name::new("tl-dot"),
            ))
            .id();
        let label = commands
            .spawn((
                Text::new(track.name),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(text_primary())),
            ))
            .id();
        commands.entity(header).add_children(&[dot, label]);
        commands.entity(left).add_child(header);
    }

    // ── Right time viewport + canvas ──
    let viewport = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Percent(100.0),
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                ..default()
            },
            RelativeCursorPosition::default(),
            TlLanes { root },
            Name::new("tl-viewport"),
        ))
        .id();
    let canvas = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                height: Val::Percent(100.0),
                width: Val::Px(duration_sec * INITIAL_PPS),
                ..default()
            },
            TlCanvas { root },
            Name::new("tl-canvas"),
        ))
        .id();

    // Lane background stripes.
    for (i, _) in tracks.iter().enumerate() {
        let bg = if i % 2 == 0 { (28, 28, 35) } else { (31, 31, 39) };
        let lane = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(RULER_H + i as f32 * TRACK_H),
                    width: Val::Percent(100.0),
                    height: Val::Px(TRACK_H),
                    ..default()
                },
                BackgroundColor(rgb(bg)),
                Name::new("tl-lane"),
            ))
            .id();
        commands.entity(canvas).add_child(lane);
    }

    // Ruler + ticks.
    let ruler = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(RULER_H),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(rgb(header_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            RelativeCursorPosition::default(),
            TlRuler { root },
            crate::cursor_icon::HoverCursor(SystemCursorIcon::EwResize),
            Name::new("tl-ruler"),
        ))
        .id();
    commands.entity(canvas).add_child(ruler);
    let seconds = duration_sec.ceil() as i32;
    for s in 0..=seconds {
        let tick = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Px(2.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    ..default()
                },
                TlTick { time: s as f32 },
                bevy::ui::FocusPolicy::Pass,
                Name::new("tl-tick"),
            ))
            .with_children(|p| {
                p.spawn((
                    Text::new(fmt_time(s as f32)),
                    ui_font(&fonts.ui, 9.0),
                    TextColor(rgb(text_muted())),
                ));
                p.spawn((
                    Node {
                        width: Val::Px(1.0),
                        height: Val::Px(5.0),
                        ..default()
                    },
                    BackgroundColor(rgb(tree_line())),
                ));
            })
            .id();
        commands.entity(canvas).add_child(tick);
    }

    // Clips / keyframes.
    for (i, track) in tracks.iter().enumerate() {
        let lane_top = RULER_H + i as f32 * TRACK_H;
        match track.lane {
            Lane::Clips(clips) => {
                for &(start, len, label) in clips {
                    let clip = commands
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                top: Val::Px(lane_top + (TRACK_H - CLIP_H) / 2.0),
                                height: Val::Px(CLIP_H),
                                align_items: AlignItems::Center,
                                padding: UiRect::horizontal(Val::Px(6.0)),
                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                overflow: Overflow::clip(),
                                ..default()
                            },
                            BackgroundColor(rgb(track.color)),
                            Interaction::default(),
                            RelativeCursorPosition::default(),
                            TlClip { root, start, len },
                            crate::cursor_icon::HoverCursor(SystemCursorIcon::Grab),
                            Name::new("tl-clip"),
                        ))
                        .with_children(|p| {
                            p.spawn((
                                Text::new(label),
                                ui_font(&fonts.ui, 10.0),
                                TextColor(rgb((20, 20, 26))),
                                TextLayout::no_wrap(),
                            ));
                        })
                        .id();
                    commands.entity(canvas).add_child(clip);
                }
            }
            Lane::Keys(keys) => {
                for &time in keys {
                    let key = commands
                        .spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                top: Val::Px(lane_top + TRACK_H / 2.0 - 6.0),
                                width: Val::Px(12.0),
                                height: Val::Px(12.0),
                                ..default()
                            },
                            BackgroundColor(rgb(track.color)),
                            UiTransform::from_rotation(Rot2::degrees(45.0)),
                            Interaction::default(),
                            TlKey { root, time },
                            crate::cursor_icon::HoverCursor(SystemCursorIcon::Grab),
                            Name::new("tl-key"),
                        ))
                        .id();
                    commands.entity(canvas).add_child(key);
                }
            }
        }
    }

    // Playhead (above everything).
    let playhead = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                width: Val::Px(2.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(rgb(accent())),
            GlobalZIndex(10),
            bevy::ui::FocusPolicy::Pass,
            TlPlayhead { root },
            Name::new("tl-playhead"),
        ))
        .id();
    commands.entity(canvas).add_child(playhead);

    commands.entity(viewport).add_child(canvas);
    commands.entity(root).add_children(&[left, viewport]);
    root
}

/// Position the canvas + every time-anchored item from the timeline's pps/scroll.
pub(crate) fn timeline_layout(
    roots: Query<&TimelineRoot>,
    canvases: Query<(Entity, &TlCanvas)>,
    clips: Query<(Entity, &TlClip)>,
    keys: Query<(Entity, &TlKey)>,
    ticks: Query<Entity, With<TlTick>>,
    tick_times: Query<&TlTick>,
    playheads: Query<(Entity, &TlPlayhead)>,
    mut nodes: Query<&mut Node>,
) {
    for (e, c) in &canvases {
        if let Ok(r) = roots.get(c.root) {
            if let Ok(mut n) = nodes.get_mut(e) {
                n.width = Val::Px(r.duration * r.pps);
                n.left = Val::Px(-(r.scroll * r.pps));
            }
        }
    }
    // Ticks share whichever root exists (single timeline per canvas).
    let pps = roots.iter().next().map(|r| r.pps).unwrap_or(INITIAL_PPS);
    for e in &ticks {
        if let (Ok(t), Ok(mut n)) = (tick_times.get(e), nodes.get_mut(e)) {
            n.left = Val::Px(t.time * pps);
        }
    }
    for (e, c) in &clips {
        if let Ok(r) = roots.get(c.root) {
            if let Ok(mut n) = nodes.get_mut(e) {
                n.left = Val::Px(c.start * r.pps);
                n.width = Val::Px((c.len * r.pps).max(2.0));
            }
        }
    }
    for (e, k) in &keys {
        if let Ok(r) = roots.get(k.root) {
            if let Ok(mut n) = nodes.get_mut(e) {
                n.left = Val::Px(k.time * r.pps - 6.0);
            }
        }
    }
    for (e, ph) in &playheads {
        if let Ok(r) = roots.get(ph.root) {
            if let Ok(mut n) = nodes.get_mut(e) {
                n.left = Val::Px(r.playhead * r.pps);
            }
        }
    }
}

pub(crate) fn timeline_zoom(
    mut wheel: MessageReader<MouseWheel>,
    lanes: Query<(&TlLanes, &RelativeCursorPosition)>,
    mut roots: Query<&mut TimelineRoot>,
) {
    let mut dy = 0.0;
    for ev in wheel.read() {
        dy += ev.y;
    }
    if dy == 0.0 {
        return;
    }
    for (lane, rcp) in &lanes {
        if rcp.cursor_over {
            if let Ok(mut r) = roots.get_mut(lane.root) {
                r.pps = (r.pps * (1.0 + dy * 0.1)).clamp(15.0, 400.0);
            }
            break;
        }
    }
}

pub(crate) fn timeline_pan(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut last: Local<Option<Vec2>>,
    lanes: Query<(&TlLanes, &RelativeCursorPosition)>,
    mut roots: Query<&mut TimelineRoot>,
) {
    if !mouse.pressed(MouseButton::Middle) {
        *last = None;
        return;
    }
    let Some(c) = cursor(&windows) else {
        return;
    };
    if let Some(prev) = *last {
        let delta = c - prev;
        for (lane, rcp) in &lanes {
            if rcp.cursor_over {
                if let Ok(mut r) = roots.get_mut(lane.root) {
                    let max = (r.duration - 1.0).max(0.0);
                    r.scroll = (r.scroll - delta.x / r.pps).clamp(0.0, max);
                }
                break;
            }
        }
    }
    *last = Some(c);
}

pub(crate) fn timeline_scrub(
    rulers: Query<(&TlRuler, &Interaction, &RelativeCursorPosition)>,
    mut roots: Query<&mut TimelineRoot>,
) {
    for (ruler, interaction, rcp) in &rulers {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(nrm) = rcp.normalized {
            if let Ok(mut r) = roots.get_mut(ruler.root) {
                r.playhead = snap((nrm.x + 0.5) * r.duration).clamp(0.0, r.duration);
            }
        }
    }
}

pub(crate) fn timeline_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut active: Local<Option<(Entity, Vec2, Drag)>>,
    clip_press: Query<(Entity, &Interaction, &RelativeCursorPosition, &ComputedNode), With<TlClip>>,
    key_press: Query<(Entity, &Interaction), With<TlKey>>,
    mut clips: Query<&mut TlClip>,
    mut keys: Query<&mut TlKey>,
    roots: Query<&TimelineRoot>,
) {
    if active.is_none() {
        if mouse.just_pressed(MouseButton::Left) {
            if let Some(c) = cursor(&windows) {
                for (e, i, rcp, cn) in &clip_press {
                    if *i == Interaction::Pressed {
                        // Grab near an edge → resize, otherwise move.
                        let w = cn.size().x * cn.inverse_scale_factor();
                        let gx = rcp.normalized.map(|n| (n.x + 0.5) * w).unwrap_or(w * 0.5);
                        let kind = if gx < 8.0 {
                            Drag::ClipLeft
                        } else if gx > w - 8.0 {
                            Drag::ClipRight
                        } else {
                            Drag::ClipMove
                        };
                        *active = Some((e, c, kind));
                        return;
                    }
                }
                for (e, i) in &key_press {
                    if *i == Interaction::Pressed {
                        *active = Some((e, c, Drag::Key));
                        return;
                    }
                }
            }
        }
        return;
    }
    if !mouse.pressed(MouseButton::Left) {
        *active = None;
        return;
    }
    let (Some((e, last, kind)), Some(c)) = (*active, cursor(&windows)) else {
        return;
    };
    let dx = c.x - last.x;
    *active = Some((e, c, kind));
    if dx == 0.0 {
        return;
    }
    if let Drag::Key = kind {
        if let Ok(mut key) = keys.get_mut(e) {
            let (pps, dur) = roots
                .get(key.root)
                .map(|r| (r.pps, r.duration))
                .unwrap_or((INITIAL_PPS, 0.0));
            key.time = snap(key.time + dx / pps).clamp(0.0, dur);
        }
        return;
    }
    if let Ok(mut clip) = clips.get_mut(e) {
        let (pps, dur) = roots
            .get(clip.root)
            .map(|r| (r.pps, r.duration))
            .unwrap_or((INITIAL_PPS, 0.0));
        let dt = dx / pps;
        match kind {
            Drag::ClipMove => {
                let len = clip.len;
                clip.start = snap(clip.start + dt).clamp(0.0, (dur - len).max(0.0));
            }
            Drag::ClipLeft => {
                let right = clip.start + clip.len;
                let new_start = snap(clip.start + dt).clamp(0.0, right - MIN_CLIP);
                clip.start = new_start;
                clip.len = right - new_start;
            }
            Drag::ClipRight => {
                let new_end = snap(clip.start + clip.len + dt).clamp(clip.start + MIN_CLIP, dur);
                clip.len = new_end - clip.start;
            }
            Drag::Key => {}
        }
    }
}
