//! The Profiler screen: a frame graph, a zoomable timeline, the slowest spans.
//!
//! Both graphs are canvases (see [`super::canvas`]): their bars are one mesh,
//! rebuilt only when the view or the data changed, at most [`REDRAW_HZ`] times
//! a second while live.

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use renzora_ember::font::EmberFonts;
use renzora_ember::theme::*;
use renzora_ember::UiInteraction;
use renzora_profiler::{ProfileFrame, Span};

use super::canvas::{spawn_canvas, CLabel, CRect, CanvasContent};
use super::{button, card, mono, text, LiveView, UiAction};
use crate::app::capture::{frame_ms, is_wrapper, slowest_in, Capture};

/// Live views redraw at most this often.
const REDRAW_HZ: f32 = 30.0;
/// The frame-time line on the graph.
const BUDGET_MS: f32 = 16.6;
/// Width of the thread-name column inside the timeline.
const LANE_LABEL_W: f32 = 132.0;
const ROW_H: f32 = 22.0;
const MAX_DEPTH: u16 = 5;
const SLOWEST_ROWS: usize = 12;
/// Rows per column of the slowest strip: twelve rows as three columns of four.
const SLOWEST_PER_COLUMN: usize = 4;

/// The frame the side panel and "Fit frame" are about. `None` follows the
/// newest one.
#[derive(Resource, Default)]
pub(crate) struct SelectedFrame(pub Option<u64>);

/// The timeline's window onto time, in the recorder's nanoseconds.
#[derive(Resource)]
pub(crate) struct TimelineView {
    pub start_ns: u64,
    pub width_ns: u64,
    /// Move to the newest frame as frames arrive, at [`FOLLOW_HZ`].
    pub follow: bool,
    /// The start of the frame the view was last moved to. Following keeps the
    /// view's offset from it, so a zoom or a nudge made while following holds
    /// as the view steps from frame to frame.
    pub followed_start_ns: u64,
    /// The first thread lane shown: the timeline scrolls by whole lanes, so
    /// no lane is ever cut in half under the ruler.
    pub first_lane: usize,
}

impl Default for TimelineView {
    fn default() -> Self {
        Self { start_ns: 0, width_ns: 20_000_000, follow: true, followed_start_ns: 0, first_lane: 0 }
    }
}

/// How often a following timeline steps to the newest frame. Every frame
/// (it was) redrew a different frame thirty times a second, which no one can
/// read; twice a second leaves time to look.
const FOLLOW_HZ: f32 = 2.0;

/// A toolbar request for the timeline.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TimelineOp {
    ZoomIn,
    ZoomOut,
    PanLeft,
    PanRight,
    FitFrame,
    ThreeFrames,
    /// Go back to following the newest frame, at the current zoom.
    Follow,
}

#[derive(Component)]
struct FrameGraph;
#[derive(Component)]
struct Timeline;
#[derive(Component)]
struct RangeText;
#[derive(Component)]
struct StatText;
#[derive(Component)]
struct SlowRow(usize);
#[derive(Component)]
struct SlowTitle;

pub(crate) fn build(app: &mut App) {
    app.init_resource::<SelectedFrame>()
        .init_resource::<TimelineView>()
        .add_message::<TimelineOp>()
        .add_systems(
            Update,
            (apply_ops, timeline_input, graph_click, follow_newest, redraw, side_panel).chain(),
        );
}

pub(crate) fn spawn(
    commands: &mut Commands,
    fonts: &EmberFonts,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    layers: &mut usize,
) -> Entity {
    // Frame graph.
    let graph_card = card(commands);
    commands.entity(graph_card).insert(Node {
        height: Val::Px(150.0),
        flex_shrink: 0.0,
        flex_direction: FlexDirection::Column,
        padding: UiRect::all(Val::Px(10.0)),
        row_gap: Val::Px(6.0),
        border_radius: BorderRadius::all(Val::Px(8.0)),
        ..default()
    });
    // The frame's numbers ride the graph's header: they describe the frame
    // the graph has highlighted, and a column of their own cost the timeline
    // a quarter of the window.
    let graph_header = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(10.0), ..default() })
        .id();
    let graph_title = text(commands, fonts, "Frames  ·  click one to inspect it", 12.0, text_muted());
    let header_spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let stats_line = mono(commands, fonts, "", 12.0, text_primary());
    commands.entity(stats_line).insert(StatText);
    commands.entity(graph_header).add_children(&[graph_title, header_spacer, stats_line]);
    let graph_box = commands.spawn(Node { flex_grow: 1.0, min_height: Val::Px(0.0), ..default() }).id();
    let graph = spawn_canvas(commands, meshes, materials, layers);
    commands.entity(graph).insert((FrameGraph, UiInteraction::default()));
    commands.entity(graph_box).add_child(graph);
    commands.entity(graph_card).add_children(&[graph_header, graph_box]);

    // Timeline.
    let tl_card = card(commands);
    commands.entity(tl_card).insert(Node {
        flex_grow: 1.0,
        min_height: Val::Px(0.0),
        flex_direction: FlexDirection::Column,
        padding: UiRect::all(Val::Px(10.0)),
        row_gap: Val::Px(8.0),
        border_radius: BorderRadius::all(Val::Px(8.0)),
        ..default()
    });
    let tools = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() })
        .id();
    let tl_title = text(commands, fonts, "Timeline", 13.0, text_primary());
    let range = mono(commands, fonts, "", 11.0, text_muted());
    commands.entity(range).insert(RangeText);
    let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let hint = text(commands, fonts, "wheel: zoom   drag: pan", 11.0, text_muted());
    commands.entity(hint).insert(Node { flex_shrink: 0.0, margin: UiRect::right(Val::Px(6.0)), ..default() });
    let mut row = vec![tl_title, range, spacer, hint];
    for (icon, op) in [
        ("caret-left", TimelineOp::PanLeft),
        ("minus", TimelineOp::ZoomOut),
        ("plus", TimelineOp::ZoomIn),
        ("caret-right", TimelineOp::PanRight),
    ] {
        row.push(button(commands, fonts, Some(icon), "", UiAction::Timeline(op)));
    }
    row.push(button(commands, fonts, None, "Fit frame", UiAction::Timeline(TimelineOp::FitFrame)));
    row.push(button(commands, fonts, None, "3 frames", UiAction::Timeline(TimelineOp::ThreeFrames)));
    row.push(button(commands, fonts, Some("play"), "Follow", UiAction::Timeline(TimelineOp::Follow)));
    commands.entity(tools).add_children(&row);
    let tl_box = commands.spawn(Node { flex_grow: 1.0, min_height: Val::Px(0.0), ..default() }).id();
    let timeline = spawn_canvas(commands, meshes, materials, layers);
    commands.entity(timeline).insert((Timeline, UiInteraction::default()));
    commands.entity(tl_box).add_child(timeline);
    commands.entity(tl_card).add_children(&[tools, tl_box]);

    // The slowest spans in view: a short strip under the timeline, filled
    // across in columns, so the timeline keeps the window's full width.
    let slow = card(commands);
    commands.entity(slow).insert(Node {
        flex_shrink: 0.0,
        flex_direction: FlexDirection::Column,
        padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
        row_gap: Val::Px(4.0),
        border_radius: BorderRadius::all(Val::Px(8.0)),
        ..default()
    });
    let slow_title = text(commands, fonts, "Slowest in view", 12.0, text_muted());
    commands.entity(slow_title).insert(SlowTitle);
    let grid = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            flex_wrap: FlexWrap::Wrap,
            height: Val::Px(SLOWEST_PER_COLUMN as f32 * 19.0),
            ..default()
        })
        .id();
    for i in 0..SLOWEST_ROWS {
        let line = mono(commands, fonts, "", 11.0, text_muted());
        commands.entity(line).insert((
            SlowRow(i),
            Node { width: Val::Percent(100.0 / (SLOWEST_ROWS / SLOWEST_PER_COLUMN) as f32), height: Val::Px(19.0), ..default() },
        ));
        commands.entity(grid).add_child(line);
    }
    commands.entity(slow).add_children(&[slow_title, grid]);

    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        })
        .id();
    commands.entity(root).add_children(&[graph_card, tl_card, slow]);
    root
}

/// The frame the view is about: the selected one, or the newest.
fn focus<'a>(capture: &'a Capture, selected: &SelectedFrame) -> Option<&'a ProfileFrame> {
    selected
        .0
        .and_then(|i| capture.frame(i))
        .or_else(|| capture.frames.back())
}

fn fit_frame(view: &mut TimelineView, frame: &ProfileFrame) {
    let len = frame.end_ns.saturating_sub(frame.start_ns).max(1);
    let pad = len / 40;
    view.start_ns = frame.start_ns.saturating_sub(pad);
    view.width_ns = len + pad * 2;
    view.followed_start_ns = frame.start_ns;
}

fn apply_ops(
    mut ops: MessageReader<TimelineOp>,
    mut view: ResMut<TimelineView>,
    mut selected: ResMut<SelectedFrame>,
    capture: Res<Capture>,
) {
    for op in ops.read() {
        match op {
            // Zooming keeps following: it is how you pick how much of each
            // frame to watch.
            TimelineOp::ZoomIn => zoom(&mut view, 0.5, 0.5),
            TimelineOp::ZoomOut => zoom(&mut view, 2.0, 0.5),
            TimelineOp::PanLeft => {
                pan(&mut view, -0.25);
                view.follow = false;
            }
            TimelineOp::PanRight => {
                pan(&mut view, 0.25);
                view.follow = false;
            }
            TimelineOp::FitFrame => {
                if let Some(frame) = focus(&capture, &selected) {
                    fit_frame(&mut view, frame);
                    view.follow = selected.0.is_none();
                }
            }
            TimelineOp::ThreeFrames => {
                if let Some(frame) = focus(&capture, &selected) {
                    let before = capture.frame(frame.index.saturating_sub(1)).unwrap_or(frame);
                    let after = capture.frame(frame.index + 1).unwrap_or(frame);
                    view.start_ns = before.start_ns;
                    view.width_ns = after.end_ns.saturating_sub(before.start_ns).max(1);
                    view.followed_start_ns = before.start_ns;
                    view.follow = false;
                }
            }
            TimelineOp::Follow => {
                selected.0 = None;
                if let Some(frame) = capture.frames.back() {
                    // Onto the newest frame now, at the zoom already chosen.
                    let offset = view.start_ns as i64 - view.followed_start_ns as i64;
                    view.start_ns = (frame.start_ns as i64 + offset).max(0) as u64;
                    view.followed_start_ns = frame.start_ns;
                }
                view.follow = true;
            }
        }
    }
}

/// Zoom by `factor` keeping the time under `anchor` (0..1 across) in place.
fn zoom(view: &mut TimelineView, factor: f64, anchor: f64) {
    let at = view.start_ns as f64 + view.width_ns as f64 * anchor;
    // 10 µs to 10 s across.
    let width = (view.width_ns as f64 * factor).clamp(10_000.0, 10_000_000_000.0);
    view.start_ns = (at - width * anchor).max(0.0) as u64;
    view.width_ns = width as u64;
}

fn pan(view: &mut TimelineView, fraction: f64) {
    let delta = view.width_ns as f64 * fraction;
    view.start_ns = (view.start_ns as f64 + delta).max(0.0) as u64;
}

/// Wheel zoom around the pointer, Shift+wheel and drag to pan.
fn timeline_input(
    timeline: Query<(&RelativeCursorPosition, &UiInteraction), With<Timeline>>,
    mut wheel: MessageReader<MouseWheel>,
    mut motion: MessageReader<MouseMotion>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut view: ResMut<TimelineView>,
    canvases: Query<&super::canvas::Canvas, With<Timeline>>,
    mut dragging: Local<bool>,
    mut drag_y: Local<f32>,
) {
    let Ok((cursor, interaction)) = timeline.single() else {
        wheel.clear();
        motion.clear();
        return;
    };
    let track_w = canvases
        .single()
        .map(|c| (c.logical_size().x - LANE_LABEL_W).max(1.0))
        .unwrap_or(1.0) as f64;
    let over = cursor.cursor_over();
    let anchor = cursor
        .normalized
        .map(|n| {
            let total = track_w + LANE_LABEL_W as f64;
            ((n.x as f64 + 0.5) * total - LANE_LABEL_W as f64) / track_w
        })
        .unwrap_or(0.5)
        .clamp(0.0, 1.0);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    // Over the thread names, the wheel scrolls the lanes rather than zooming:
    // there is no time under the pointer there to zoom around.
    let over_names = cursor.normalized.is_some_and(|n| {
        let total = track_w + LANE_LABEL_W as f64;
        ((n.x as f64 + 0.5) * total) < LANE_LABEL_W as f64
    });
    for event in wheel.read() {
        if !over {
            continue;
        }
        let amount = event.y + event.x;
        if amount == 0.0 {
            continue;
        }
        if ctrl || over_names {
            if event.y > 0.0 {
                view.first_lane = view.first_lane.saturating_sub(1);
            } else if event.y < 0.0 {
                view.first_lane += 1;
            }
        } else if shift {
            pan(&mut view, if amount > 0.0 { -0.15 } else { 0.15 });
            view.follow = false;
        } else {
            zoom(&mut view, if amount > 0.0 { 0.8 } else { 1.25 }, anchor);
        }
    }
    if *interaction == UiInteraction::Pressed && buttons.just_pressed(MouseButton::Left) {
        *dragging = true;
    }
    if !buttons.pressed(MouseButton::Left) {
        *dragging = false;
    }
    let delta: Vec2 = motion.read().map(|m| m.delta).sum();
    if *dragging && delta.x != 0.0 {
        let ns_per_px = view.width_ns as f64 / track_w;
        view.start_ns = (view.start_ns as f64 - delta.x as f64 * ns_per_px).max(0.0) as u64;
        view.follow = false;
    }
    // Dragging moves up and down too, a lane per LANE_DRAG_PX of travel.
    if *dragging {
        *drag_y += delta.y;
        while *drag_y >= LANE_DRAG_PX {
            *drag_y -= LANE_DRAG_PX;
            view.first_lane = view.first_lane.saturating_sub(1);
        }
        while *drag_y <= -LANE_DRAG_PX {
            *drag_y += LANE_DRAG_PX;
            view.first_lane += 1;
        }
    } else {
        *drag_y = 0.0;
    }
}

/// Vertical drag distance that scrolls the timeline by one thread lane.
const LANE_DRAG_PX: f32 = 40.0;

/// A click on the frame graph selects the frame under the pointer.
fn graph_click(
    graph: Query<(&RelativeCursorPosition, &UiInteraction, &super::canvas::Canvas), (With<FrameGraph>, Changed<UiInteraction>)>,
    capture: Res<Capture>,
    mut selected: ResMut<SelectedFrame>,
    mut view: ResMut<TimelineView>,
) {
    let Ok((cursor, interaction, canvas)) = graph.single() else { return };
    if *interaction != UiInteraction::Pressed {
        return;
    }
    let Some(n) = cursor.normalized else { return };
    let frames = visible_frames(&capture, canvas.logical_size().x);
    let at = ((n.x + 0.5) * frames.len() as f32) as usize;
    if let Some(frame) = frames.get(at.min(frames.len().saturating_sub(1))) {
        selected.0 = Some(frame.index);
        fit_frame(&mut view, frame);
        view.follow = false;
    }
}

/// While following, step the view to the newest frame [`FOLLOW_HZ`] times a
/// second, keeping its zoom and its offset into the frame.
fn follow_newest(
    capture: Res<Capture>,
    mut view: ResMut<TimelineView>,
    live: Res<LiveView>,
    time: Res<Time<Real>>,
    mut last: Local<Option<f32>>,
) {
    if !view.follow || !live.0 || !capture.is_changed() {
        return;
    }
    let now = time.elapsed_secs();
    if last.is_some_and(|last| now - last < 1.0 / FOLLOW_HZ) {
        return;
    }
    *last = Some(now);
    let Some(frame) = capture.frames.back() else { return };
    if view.followed_start_ns == 0 {
        // The first frame: fit it, which also sets a sensible zoom.
        fit_frame(&mut view, frame);
        return;
    }
    let offset = view.start_ns as i64 - view.followed_start_ns as i64;
    view.start_ns = (frame.start_ns as i64 + offset).max(0) as u64;
    view.followed_start_ns = frame.start_ns;
}

/// Bar width and gap on the frame graph.
const BAR: f32 = 4.0;
const GAP: f32 = 1.0;

/// The newest frames that fit `width` logical pixels of graph, oldest first.
fn visible_frames(capture: &Capture, width: f32) -> Vec<&ProfileFrame> {
    let n = ((width / (BAR + GAP)).floor() as usize).max(1);
    let skip = capture.frames.len().saturating_sub(n);
    capture.frames.iter().skip(skip).collect()
}

#[allow(clippy::too_many_arguments)]
fn redraw(
    capture: Res<Capture>,
    view: Res<TimelineView>,
    selected: Res<SelectedFrame>,
    live: Res<LiveView>,
    time: Res<Time<Real>>,
    mut last: Local<f32>,
    mut graph: Query<(Ref<super::canvas::Canvas>, &mut CanvasContent), (With<FrameGraph>, Without<Timeline>)>,
    mut timeline: Query<(Ref<super::canvas::Canvas>, &mut CanvasContent), (With<Timeline>, Without<FrameGraph>)>,
    mut range: Query<&mut Text, With<RangeText>>,
) {
    let data_changed = capture.is_changed() && live.0;
    // A canvas that changed size (including its first layout, which is when
    // it gets one at all) has content laid out for the old size. Without this
    // an idle companion showed two blank panels until the first frame arrived.
    let resized = graph.iter().any(|(c, _)| c.is_changed()) || timeline.iter().any(|(c, _)| c.is_changed());
    let view_changed = view.is_changed() || selected.is_changed() || live.is_changed() || resized;
    if !data_changed && !view_changed {
        return;
    }
    // Live data alone redraws at most REDRAW_HZ; a user's own zoom or click
    // redraws at once.
    let now = time.elapsed_secs();
    if !view_changed && now - *last < 1.0 / REDRAW_HZ {
        return;
    }
    *last = now;

    if let Ok((canvas, mut content)) = graph.single_mut() {
        draw_frame_graph(&capture, &view, &selected, canvas.logical_size(), &mut content);
    }
    if let Ok((canvas, mut content)) = timeline.single_mut() {
        draw_timeline(&capture, &view, canvas.logical_size(), &mut content);
    }
    let line = format!(
        "{} to {}  ·  {} shown",
        fmt_ms(view.start_ns as f64 / 1e6 - capture.frames.front().map_or(0.0, |f| f.start_ns as f64 / 1e6)),
        fmt_ms((view.start_ns + view.width_ns) as f64 / 1e6 - capture.frames.front().map_or(0.0, |f| f.start_ns as f64 / 1e6)),
        fmt_ms(view.width_ns as f64 / 1e6)
    );
    for mut t in &mut range {
        if t.0 != line {
            t.0.clone_from(&line);
        }
    }
}

fn fmt_ms(ms: f64) -> String {
    if ms.abs() < 1.0 {
        format!("{:.0} µs", ms * 1000.0)
    } else if ms.abs() < 100.0 {
        format!("{ms:.2} ms")
    } else {
        format!("{:.2} s", ms / 1000.0)
    }
}

fn draw_frame_graph(capture: &Capture, view: &TimelineView, selected: &SelectedFrame, size: Vec2, content: &mut CanvasContent) {
    content.rects.clear();
    content.labels.clear();
    content.background = Some(rgb(panel_bg()));
    if capture.frames.is_empty() {
        empty_state(size, "No frames yet. Pick a game to profile, then press Record.", content);
        return;
    }
    let frames = visible_frames(capture, size.x);
    let max_ms = frames.iter().map(|f| frame_ms(f)).fold(BUDGET_MS * 1.5, f32::max);
    let scale = size.y / max_ms;
    let newest = frames.last().map(|f| f.index);
    let selected = selected.0.or(newest);
    for (i, frame) in frames.iter().enumerate() {
        let ms = frame_ms(frame);
        let h = (ms * scale).max(1.0);
        let x = i as f32 * (BAR + GAP);
        let in_view = frame.end_ns > view.start_ns && frame.start_ns < view.start_ns + view.width_ns;
        if in_view {
            content.rects.push(CRect { x: x - 1.0, y: 0.0, w: BAR + 2.0, h: size.y, color: rgb(hover_bg()) });
        }
        let overlapped = capture.overlapped.contains(&frame.index);
        let color = if Some(frame.index) == selected {
            rgb(warn_amber())
        } else if overlapped {
            // Recorded while a companion job ran: its time is not the game's
            // alone, so it is greyed rather than judged against the budget.
            Color::srgb_u8(96, 98, 112)
        } else if ms > BUDGET_MS {
            Color::srgb_u8(185, 138, 74)
        } else {
            rgb(accent())
        };
        content.rects.push(CRect { x, y: size.y - h, w: BAR, h, color });
        if overlapped {
            content.rects.push(CRect { x, y: 0.0, w: BAR, h: 3.0, color: Color::srgb_u8(150, 152, 170) });
        }
    }
    let line_y = size.y - BUDGET_MS * scale;
    content.rects.push(CRect { x: 0.0, y: line_y, w: size.x, h: 1.0, color: rgb(border()) });
    content.labels.push(CLabel {
        x: size.x - 60.0,
        y: (line_y - 14.0).max(0.0),
        text: format!("{BUDGET_MS} ms"),
        size: 12.0,
        color: rgb(text_muted()),
    });
    content.dirty = true;
}

/// One muted line, roughly centred: what a view shows before there is any data.
/// The width is estimated from the character count, which is close enough for
/// centring and saves measuring text the canvas lays out itself.
fn empty_state(size: Vec2, message: &str, content: &mut CanvasContent) {
    let w = message.chars().count() as f32 * 7.0;
    content.labels.push(CLabel {
        x: ((size.x - w) * 0.5).max(8.0),
        y: (size.y * 0.5 - 8.0).max(4.0),
        text: message.into(),
        size: 14.0,
        color: rgb(text_muted()),
    });
    content.dirty = true;
}

/// What colour a span is, from its name: a rough grouping so the eye can
/// find physics or rendering in a busy frame.
fn span_color(name: &str) -> Color {
    let n = name.to_ascii_lowercase();
    if n.contains("render") || n.contains("pass") || n.contains("extract") || n.contains("queue") || n.contains("prepare") || n.contains("pipeline") || n.contains("command_buffer") {
        Color::srgb_u8(106, 79, 181)
    } else if n.contains("avian") || n.contains("rapier") || n.contains("physics") || n.contains("collision") || n.contains("solver") {
        Color::srgb_u8(35, 120, 110)
    } else if n.contains("script") {
        Color::srgb_u8(154, 101, 22)
    } else if n.contains("ui") || n.contains("ember") || n.contains("text") || n.contains("layout") {
        Color::srgb_u8(148, 65, 111)
    } else {
        Color::srgb_u8(59, 93, 170)
    }
}

use super::short as short_name;

fn draw_timeline(capture: &Capture, view: &TimelineView, size: Vec2, content: &mut CanvasContent) {
    content.rects.clear();
    content.labels.clear();
    content.background = Some(rgb(panel_bg()));
    if capture.frames.is_empty() {
        empty_state(size, "Each thread's systems appear here, frame by frame.", content);
        return;
    }
    let track_x = LANE_LABEL_W;
    let track_w = (size.x - LANE_LABEL_W).max(1.0);
    let end_ns = view.start_ns + view.width_ns;
    let px = |ns: u64| track_x + ((ns as f64 - view.start_ns as f64) / view.width_ns as f64 * track_w as f64) as f32;
    let origin = capture.frames.front().map_or(0, |f| f.start_ns);

    // Ruler, counted from the start of the frame the view begins in. Counted
    // from the start of the capture, a minute in, every tick of a 20 ms view
    // read "98.73 s" and the ruler said nothing.
    let anchor = capture
        .frames
        .iter()
        .rev()
        .find(|f| f.start_ns <= view.start_ns)
        .map_or(origin, |f| f.start_ns);
    let step = tick_step(view.width_ns as f64 / 1e6);
    let mut t = ((view.start_ns as f64 - anchor as f64) / 1e6 / step).ceil() * step;
    while anchor as f64 + t * 1e6 <= end_ns as f64 {
        let x = px((anchor as f64 + t * 1e6).max(0.0) as u64);
        let text = if t.abs() < step / 2.0 { "0".to_string() } else { format!("+{}", fmt_ms(t)) };
        content.rects.push(CRect { x, y: 0.0, w: 1.0, h: size.y, color: rgb(faint_bg()) });
        content.labels.push(CLabel { x: x + 3.0, y: 2.0, text, size: 12.0, color: rgb(text_muted()) });
        t += step;
    }

    // Frames in range, and the spans that overlap the view.
    const SLACK_NS: u64 = 1_000_000_000;
    let frames: Vec<&ProfileFrame> = capture
        .frames
        .iter()
        .filter(|f| f.end_ns > view.start_ns && f.start_ns < end_ns.saturating_add(SLACK_NS))
        .collect();
    for frame in &frames {
        if frame.start_ns >= view.start_ns && frame.start_ns <= end_ns {
            let x = px(frame.start_ns);
            content.rects.push(CRect { x, y: 16.0, w: 1.0, h: size.y - 16.0, color: rgb(text_muted()) });
            content.labels.push(CLabel {
                x: x + 4.0,
                y: size.y - 14.0,
                text: format!("frame {} · {:.1} ms", frame.index, frame_ms(frame)),
                size: 12.0,
                color: rgb(text_muted()),
            });
        }
    }

    // One lane per thread that has work in view: the main thread, then the
    // render thread, then the rest by name. Thread ids are the order threads
    // first recorded anything, which put pool workers above the main thread
    // and pushed it off the bottom.
    let mut threads: Vec<u32> = frames
        .iter()
        .flat_map(|f| f.spans.iter())
        .filter(|s| s.end_ns > view.start_ns && s.start_ns < end_ns)
        .map(|s| s.thread)
        .collect();
    threads.sort_unstable();
    threads.dedup();
    threads.sort_by_cached_key(|t| lane_order(capture.thread_name(*t)));
    // Wrappers (the app update, `schedule: Main`, the render sub-app) are
    // left out: they span the whole frame, which the frame markers already
    // show, and stacked five deep they took most of every lane's height
    // before any work appeared. The lane starts at the work.
    let lanes: Vec<(u32, Vec<&Span>)> = threads
        .into_iter()
        .map(|thread| {
            let spans: Vec<&Span> = frames
                .iter()
                .flat_map(|f| f.spans.iter())
                .filter(|s| s.thread == thread && s.end_ns > view.start_ns && s.start_ns < end_ns)
                .filter(|s| !is_wrapper(capture.name(s.name)))
                .collect();
            (thread, spans)
        })
        .filter(|(_, spans)| !spans.is_empty())
        .collect();
    let first = view.first_lane.min(lanes.len().saturating_sub(1));
    let mut y = 22.0;
    if first > 0 {
        content.labels.push(CLabel {
            x: 6.0,
            y,
            text: format!("{first} threads above: wheel over the names, or Ctrl + wheel"),
            size: 12.0,
            color: rgb(text_muted()),
        });
        y += 18.0;
    }
    // Kept free at the bottom for the GPU lane and the "more threads" line
    // above it, so thread lanes that run out of room stop above both.
    let gpu_room = ROW_H + 8.0 + 18.0 + 4.0;
    let lane_count = lanes.len();
    for (shown, (thread, spans)) in lanes.into_iter().enumerate().skip(first) {
        // Depth below the outermost span actually present on this lane.
        let base = spans.iter().map(|s| s.depth).min().unwrap_or(0);
        let depth = spans.iter().map(|s| (s.depth - base).min(MAX_DEPTH)).max().unwrap_or(0);
        let lane_h = ROW_H * (depth as f32 + 1.0) + 6.0;
        if y + lane_h > size.y - 18.0 - gpu_room {
            content.labels.push(CLabel {
                x: 6.0,
                y,
                text: format!("{} threads below: wheel over the names, or Ctrl + wheel", lane_count - shown),
                size: 12.0,
                color: rgb(text_muted()),
            });
            y += 18.0;
            break;
        }
        content.rects.push(CRect { x: 0.0, y, w: size.x, h: lane_h, color: rgb(faint_bg()) });
        content.labels.push(CLabel {
            x: 6.0,
            y: y + 3.0,
            text: capture.thread_name(thread).to_string(),
            size: 13.0,
            color: rgb(text_primary()),
        });
        // Where the last label in each row ends. Spans can overlap in one row
        // (work stolen onto this thread inside a system), and their labels
        // printed over each other into an unreadable smear.
        let mut label_end = [f32::MIN; MAX_DEPTH as usize + 1];
        for span in spans {
            let d = (span.depth - base).min(MAX_DEPTH) as f32;
            let x0 = px(span.start_ns.max(view.start_ns));
            let x1 = px(span.end_ns.min(end_ns));
            let w = x1 - x0;
            // Narrower than a third of a pixel: invisible either way, and at
            // wide zooms there are thousands of these.
            if w < 0.33 {
                continue;
            }
            let name = capture.name(span.name);
            let ry = y + 3.0 + d * ROW_H;
            content.rects.push(CRect { x: x0, y: ry, w: w.max(1.0), h: ROW_H - 2.0, color: span_color(name) });
            if w > 56.0 {
                let ms = span.end_ns.saturating_sub(span.start_ns) as f64 / 1e6;
                let label = if w > 160.0 {
                    format!("{}  {}", short_name(name), fmt_ms(ms))
                } else {
                    short_name(name).to_string()
                };
                // Roughly 7 px a character at 12 px; cut to fit the bar.
                let max_chars = ((w - 8.0) / 7.0).max(1.0) as usize;
                let shown: String = label.chars().take(max_chars).collect();
                let row = d as usize;
                if x0 + 4.0 >= label_end[row] {
                    label_end[row] = x0 + 4.0 + shown.chars().count() as f32 * 7.0;
                    content.labels.push(CLabel { x: x0 + 4.0, y: ry + 2.0, text: shown, size: 12.0, color: Color::srgb_u8(244, 246, 250) });
                }
            }
        }
        y += lane_h + 4.0;
    }

    // The GPU, as passes laid end to end: Bevy reports how long each pass
    // took, not when it started.
    if let Some(frame) = frames.iter().rev().find(|f| !f.gpu.is_empty() && f.start_ns <= end_ns) {
        if y + ROW_H + 8.0 < size.y - 18.0 {
            content.rects.push(CRect { x: 0.0, y, w: size.x, h: ROW_H + 6.0, color: rgb(faint_bg()) });
            content.labels.push(CLabel { x: 6.0, y: y + 3.0, text: "GPU (in order)".into(), size: 13.0, color: rgb(text_primary()) });
            let mut at = frame.start_ns;
            for pass in frame.gpu.iter().filter(|p| p.depth == 0) {
                let len = (pass.ms as f64 * 1e6) as u64;
                let x0 = px(at.max(view.start_ns));
                let x1 = px((at + len).min(end_ns));
                if x1 - x0 >= 0.33 {
                    content.rects.push(CRect { x: x0, y: y + 3.0, w: (x1 - x0).max(1.0), h: ROW_H - 2.0, color: Color::srgb_u8(106, 79, 181) });
                    if x1 - x0 > 48.0 {
                        let max_chars = ((x1 - x0 - 8.0) / 7.0).max(1.0) as usize;
                        let label: String = short_name(capture.name(pass.name)).chars().take(max_chars).collect();
                        content.labels.push(CLabel { x: x0 + 4.0, y: y + 5.0, text: label, size: 12.0, color: Color::srgb_u8(244, 246, 250) });
                    }
                }
                at += len;
            }
        }
    }
    content.dirty = true;
}

/// Where a thread's lane goes: main, render, then by name with the number in
/// it compared as a number, so "Pool (2)" comes before "Pool (10)".
fn lane_order(name: &str) -> (u8, String, u32) {
    let rank = match name {
        "main" => 0,
        n if n.starts_with("Render") => 1,
        _ => 2,
    };
    let number = name
        .split(|c: char| !c.is_ascii_digit())
        .find(|part| !part.is_empty())
        .and_then(|part| part.parse().ok())
        .unwrap_or(0);
    let text: String = name.chars().filter(|c| !c.is_ascii_digit()).collect();
    (rank, text, number)
}

/// A ruler step giving 5 to 10 ticks across `width_ms`.
fn tick_step(width_ms: f64) -> f64 {
    const STEPS: [f64; 13] = [0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0];
    STEPS.into_iter().find(|s| width_ms / s <= 10.0).unwrap_or(250.0)
}

#[allow(clippy::type_complexity)]
fn side_panel(
    capture: Res<Capture>,
    selected: Res<SelectedFrame>,
    view: Res<TimelineView>,
    live: Res<LiveView>,
    mut stats: Query<&mut Text, (With<StatText>, Without<SlowRow>, Without<SlowTitle>)>,
    mut rows: Query<(&SlowRow, &mut Text), (Without<StatText>, Without<SlowTitle>)>,
    time: Res<Time<Real>>,
    mut last: Local<f32>,
) {
    let changed = (capture.is_changed() && live.0) || selected.is_changed() || view.is_changed();
    let now = time.elapsed_secs();
    if !changed || now - *last < 0.25 {
        return;
    }
    *last = now;
    let Some(frame) = focus(&capture, &selected) else {
        for mut t in &mut stats {
            t.0.clear();
        }
        return;
    };
    let gpu_ms: f32 = frame.gpu.iter().filter(|p| p.depth == 0).map(|p| p.ms).sum();
    let counter = |name: &str| {
        frame
            .counters
            .iter()
            .find(|c| capture.name(c.name) == name)
            .map(|c| c.value)
    };
    let threads = {
        let mut t: Vec<u32> = frame.spans.iter().map(|s| s.thread).collect();
        t.sort_unstable();
        t.dedup();
        t.len()
    };
    let mut parts = vec![
        format!("Frame {}", frame.index),
        format!("CPU {:.2} ms", frame_ms(frame)),
        if gpu_ms > 0.0 { format!("GPU {gpu_ms:.2} ms") } else { "GPU not timed".into() },
        format!("{} spans", frame.spans.len()),
    ];
    if let Some(n) = counter("Entities") {
        parts.push(format!("{n:.0} entities"));
    }
    parts.push(format!("{threads} threads"));
    let line = parts.join("  ·  ");
    for mut t in &mut stats {
        if t.0 != line {
            t.0.clone_from(&line);
        }
    }
    let slowest = slowest_in(&capture, view.start_ns, view.start_ns + view.width_ns, SLOWEST_ROWS);
    for (row, mut t) in &mut rows {
        let line = slowest
            .get(row.0)
            .map(|(name, ms, runs)| {
                // Cut to what fits beside the numbers in a third of the
                // window: a longer name wraps onto a line of its own.
                const ROOM: usize = 28;
                let name = short_name(capture.name(*name));
                let name: String = if name.chars().count() > ROOM {
                    name.chars().take(ROOM - 1).chain(std::iter::once('…')).collect()
                } else {
                    name.to_string()
                };
                format!("{ms:>7.2} ms  x{runs:<3} {name}")
            })
            .unwrap_or_default();
        if t.0 != line {
            t.0 = line;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_keeps_the_anchor_in_place() {
        let mut view = TimelineView { start_ns: 1_000_000, width_ns: 20_000_000, follow: true, followed_start_ns: 1_000_000, first_lane: 0 };
        let anchor_ns = view.start_ns as f64 + view.width_ns as f64 * 0.25;
        zoom(&mut view, 0.5, 0.25);
        let after = view.start_ns as f64 + view.width_ns as f64 * 0.25;
        assert!((after - anchor_ns).abs() < 2.0);
        assert_eq!(view.width_ns, 10_000_000);
        assert!(view.follow, "zooming keeps following");
    }

    /// Following moves the view frame to frame without touching its zoom or
    /// where in the frame it is looking.
    #[test]
    fn following_keeps_zoom_and_offset() {
        use renzora_profiler::wire::GamePacket;
        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin)
            .init_resource::<Capture>()
            .insert_resource(LiveView(true))
            .insert_resource(TimelineView { start_ns: 1_004_000, width_ns: 2_000_000, follow: true, followed_start_ns: 1_000_000, first_lane: 0 })
            .add_systems(Update, follow_newest);
        app.world_mut()
            .resource_mut::<Capture>()
            .receive(GamePacket::Frame(ProfileFrame { index: 0, start_ns: 9_000_000, end_ns: 25_000_000, ..default() }));
        app.update();
        let view = app.world().resource::<TimelineView>();
        assert_eq!(view.width_ns, 2_000_000);
        assert_eq!(view.start_ns, 9_004_000);
        assert_eq!(view.followed_start_ns, 9_000_000);
    }

    #[test]
    fn lanes_put_main_and_render_first_and_count_pools_numerically() {
        let mut names = vec!["Compute Task Pool (10)", "Render thread", "Compute Task Pool (2)", "main", "IO Task Pool (0)"];
        names.sort_by_key(|n| lane_order(n));
        assert_eq!(names, ["main", "Render thread", "Compute Task Pool (2)", "Compute Task Pool (10)", "IO Task Pool (0)"]);
    }

    #[test]
    fn ticks_stay_readable_at_any_zoom() {
        for width in [0.05, 0.3, 2.0, 16.6, 60.0, 900.0] {
            let n = width / tick_step(width);
            assert!(n <= 10.0 && n >= 0.5, "width {width} ms gave {n} ticks");
        }
    }

    #[test]
    fn names_shorten_to_the_function() {
        assert_eq!(short_name("bevy_ui::layout::ui_layout_system"), "ui_layout_system");
        assert_eq!(short_name("my::thing<Foo>"), "thing");
        assert_eq!(short_name("main_opaque_pass_3d"), "main_opaque_pass_3d");
    }
}
