//! Bevy-native (ember) port of the egui `SequencerPanel`: transport toolbar,
//! track headers (mute/lock/delete), a time ruler, clip lanes with positioned
//! clips + keyframe ticks, a draggable playhead, and click-to-scrub. Clip
//! drag/resize and wheel zoom/pan are deferred (toolbar zoom buttons work).

use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora_editor::SplashState;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_text, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;

use crate::model::TrackKind;
use crate::runtime::{push_action, track_clip_views, SequencerAction, SequencerBridge, SequencerState};

const HEADER_W: f32 = 200.0;
const RULER_H: f32 = 22.0;
const PLAYHEAD: (u8, u8, u8) = (255, 80, 80);
const CAMERA: (u8, u8, u8) = (100, 149, 237);
const TRANSFORM: (u8, u8, u8) = (120, 200, 120);
const MEDIA: (u8, u8, u8) = (220, 140, 60);
const MARKER: (u8, u8, u8) = (220, 200, 80);

pub struct NativeSequencer;

impl Plugin for NativeSequencer {
    fn build(&self, app: &mut App) {
        app.register_panel_content("sequencer", false, build);
        app.add_systems(
            Update,
            (seq_btn_click, track_btn_click, seq_scrub, update_playhead, update_play_icon).run_if(in_state(SplashState::Editor)),
        );
    }
}

fn track_color(kind: &TrackKind) -> (u8, u8, u8) {
    match kind {
        TrackKind::Camera { .. } => CAMERA,
        TrackKind::Transform { .. } => TRANSFORM,
        TrackKind::Media { .. } => MEDIA,
        TrackKind::Marker { .. } => MARKER,
    }
}

fn seq(w: &World) -> Option<&SequencerState> {
    w.get_resource::<SequencerState>()
}

#[derive(Component, Clone, Copy)]
enum SeqBtn {
    SkipBack,
    PlayPause,
    Stop,
    SkipForward,
    Loop,
    Key,
    Mark,
    Bake,
    ZoomIn,
    ZoomOut,
    AddTrack,
}
#[derive(Component)]
struct TrackBtn {
    track: usize,
    op: TrackOp,
}
#[derive(Clone, Copy)]
enum TrackOp {
    Mute,
    Lock,
    Delete,
}
#[derive(Component)]
struct Playhead;
#[derive(Component)]
struct PlayIcon;
/// Full-timeline click/drag layer → scrub; carries the live geometry for the
/// scrub system.
#[derive(Component)]
struct ScrubLayer;

// ── Build ────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() },
            BackgroundColor(rgb(panel_bg())),
            Name::new("native-sequencer"),
        ))
        .id();

    let toolbar = build_toolbar(commands, fonts);

    let content = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, min_height: Val::Px(0.0), flex_direction: FlexDirection::Row, ..default() })
        .id();

    // Header pane.
    let headers = commands
        .spawn((Node { width: Val::Px(HEADER_W), flex_shrink: 0.0, flex_direction: FlexDirection::Column, ..default() }, BackgroundColor(rgb(section_bg()))))
        .id();
    let htop = commands
        .spawn((Node { width: Val::Percent(100.0), height: Val::Px(RULER_H), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), padding: UiRect::horizontal(Val::Px(6.0)), ..default() }, BackgroundColor(rgb(header_bg()))))
        .id();
    let htitle = commands.spawn((Text::new("Tracks"), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())))).id();
    let hgap = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let add = icon_btn(commands, fonts, "plus", text_muted(), SeqBtn::AddTrack);
    commands.entity(htop).add_children(&[htitle, hgap, add]);
    let header_list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() }).id();
    keyed_list(commands, header_list, header_snapshot);
    commands.entity(headers).add_children(&[htop, header_list]);

    let sep = commands.spawn((Node { width: Val::Px(1.0), height: Val::Percent(100.0), flex_shrink: 0.0, ..default() }, BackgroundColor(rgb(border())))).id();

    // Timeline pane.
    let timeline = commands
        .spawn((Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Column, position_type: PositionType::Relative, overflow: Overflow::clip(), ..default() }, Name::new("seq-timeline")))
        .id();
    let ruler = commands
        .spawn((Node { width: Val::Percent(100.0), height: Val::Px(RULER_H), flex_shrink: 0.0, position_type: PositionType::Relative, overflow: Overflow::clip(), ..default() }, BackgroundColor(rgb(section_bg()))))
        .id();
    keyed_list(commands, ruler, ruler_snapshot);
    let lanes = commands
        .spawn(Node { width: Val::Percent(100.0), flex_grow: 1.0, min_height: Val::Px(0.0), position_type: PositionType::Relative, overflow: Overflow::clip(), ..default() })
        .id();
    let lanes_bg = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() }).id();
    keyed_list(commands, lanes_bg, lane_bg_snapshot);
    let clips = commands.spawn(Node { position_type: PositionType::Absolute, top: Val::Px(0.0), left: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }).id();
    keyed_list(commands, clips, clips_snapshot);
    commands.entity(lanes).add_children(&[lanes_bg, clips]);

    // Playhead (spans ruler + lanes).
    let playhead = commands
        .spawn((
            Node { position_type: PositionType::Absolute, top: Val::Px(0.0), left: Val::Px(0.0), width: Val::Px(1.5), height: Val::Percent(100.0), ..default() },
            BackgroundColor(rgb(PLAYHEAD)),
            Playhead,
            bevy::ui::FocusPolicy::Pass,
            Name::new("seq-playhead"),
        ))
        .id();
    // Scrub layer (transparent, full timeline).
    let scrub = commands
        .spawn((
            Node { position_type: PositionType::Absolute, top: Val::Px(0.0), left: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            RelativeCursorPosition::default(),
            ScrubLayer,
            Name::new("seq-scrub"),
        ))
        .id();
    commands.entity(timeline).add_children(&[ruler, lanes, playhead, scrub]);

    commands.entity(content).add_children(&[headers, sep, timeline]);
    commands.entity(root).add_children(&[toolbar, content]);
    root
}

fn build_toolbar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn((Node { width: Val::Percent(100.0), height: Val::Px(26.0), flex_shrink: 0.0, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(3.0), padding: UiRect::horizontal(Val::Px(6.0)), border: UiRect::bottom(Val::Px(1.0)), ..default() }, BackgroundColor(rgb(header_bg())), BorderColor::all(rgb(border()))))
        .id();
    let skip_back = icon_btn(commands, fonts, "skip-back", text_primary(), SeqBtn::SkipBack);
    let play = commands
        .spawn((Node { width: Val::Px(22.0), height: Val::Px(20.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(3.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(Color::NONE), Interaction::default(), SeqBtn::PlayPause))
        .id();
    let play_glyph = icon_text(commands, &fonts.phosphor, "play", text_primary(), 14.0);
    commands.entity(play_glyph).insert(PlayIcon);
    commands.entity(play).add_child(play_glyph);
    let stop = icon_btn(commands, fonts, "stop", text_primary(), SeqBtn::Stop);
    let skip_fwd = icon_btn(commands, fonts, "skip-forward", text_primary(), SeqBtn::SkipForward);
    let loop_b = icon_btn(commands, fonts, "repeat", text_muted(), SeqBtn::Loop);
    let key = label_btn(commands, fonts, "diamond", "Key", SeqBtn::Key);
    let mark = label_btn(commands, fonts, "flag", "Mark", SeqBtn::Mark);
    let bake = label_btn(commands, fonts, "film-strip", "Bake", SeqBtn::Bake);
    let gap = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let time = commands.spawn((Text::new(""), ui_font(&fonts.mono, 11.0), TextColor(rgb(text_primary())))).id();
    bind_text(commands, time, |w| {
        let Some(s) = seq(w) else { return String::new() };
        let secs = s.playhead;
        let frame = (secs * s.sequence.fps as f32) as u32;
        format!("{:02}:{:05.2}  f{}", (secs / 60.0) as u32, secs % 60.0, frame)
    });
    let zoom_out = icon_btn(commands, fonts, "magnifying-glass-minus", text_muted(), SeqBtn::ZoomOut);
    let zoom_lbl = commands.spawn((Text::new(""), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())))).id();
    bind_text(commands, zoom_lbl, |w| format!("{:.0}px/s", seq(w).map(|s| s.timeline_zoom).unwrap_or(0.0)));
    let zoom_in = icon_btn(commands, fonts, "magnifying-glass-plus", text_muted(), SeqBtn::ZoomIn);
    commands.entity(bar).add_children(&[skip_back, play, stop, skip_fwd, loop_b, key, mark, bake, gap, time, zoom_out, zoom_lbl, zoom_in]);
    bar
}

fn icon_btn<M: Component>(commands: &mut Commands, fonts: &EmberFonts, icon: &str, color: (u8, u8, u8), marker: M) -> Entity {
    let btn = commands
        .spawn((Node { width: Val::Px(22.0), height: Val::Px(20.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(3.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(Color::NONE), Interaction::default(), marker))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, color, 14.0);
    commands.entity(btn).add_child(ic);
    btn
}

fn label_btn<M: Component>(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str, marker: M) -> Entity {
    let btn = commands
        .spawn((Node { height: Val::Px(20.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(3.0), padding: UiRect::horizontal(Val::Px(5.0)), border_radius: BorderRadius::all(Val::Px(3.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(rgb(card_bg())), Interaction::default(), marker))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), 11.0);
    let l = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())))).id();
    commands.entity(btn).add_children(&[ic, l]);
    btn
}

// ── Snapshots ────────────────────────────────────────────────────────────────

fn header_snapshot(world: &World) -> KeyedSnapshot {
    let Some(s) = seq(world) else { return empty() };
    let th = s.track_height;
    let tracks: Vec<(String, bool, bool, (u8, u8, u8))> = s
        .sequence
        .tracks
        .iter()
        .map(|t| (t.name.clone(), t.muted, t.locked, track_color(&t.kind)))
        .collect();
    let items: Vec<(u64, u64)> = tracks
        .iter()
        .enumerate()
        .map(|(i, (name, muted, locked, _))| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            (name, muted, locked, th.to_bits()).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (name, muted, locked, color) = &tracks[i];
            header_row(c, f, i, name, *muted, *locked, *color, th)
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn header_row(commands: &mut Commands, fonts: &EmberFonts, idx: usize, name: &str, muted: bool, locked: bool, color: (u8, u8, u8), th: f32) -> Entity {
    let bg = if idx.is_multiple_of(2) { row_even() } else { row_odd() };
    let row = commands
        .spawn((Node { width: Val::Percent(100.0), height: Val::Px(th), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::horizontal(Val::Px(6.0)), ..default() }, BackgroundColor(rgb(bg))))
        .id();
    let swatch = commands.spawn((Node { width: Val::Px(4.0), height: Val::Px((th - 10.0).max(4.0)), border_radius: BorderRadius::all(Val::Px(1.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(rgb(color)))).id();
    let lbl = commands.spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), bevy::text::TextLayout::new_with_no_wrap(), Node { flex_grow: 1.0, overflow: Overflow::clip(), ..default() })).id();
    let mute = icon_btn(commands, fonts, if muted { "speaker-slash" } else { "speaker-high" }, if muted { text_muted() } else { text_primary() }, TrackBtn { track: idx, op: TrackOp::Mute });
    let lock = icon_btn(commands, fonts, if locked { "lock" } else { "lock-open" }, text_primary(), TrackBtn { track: idx, op: TrackOp::Lock });
    let del = icon_btn(commands, fonts, "trash", text_muted(), TrackBtn { track: idx, op: TrackOp::Delete });
    commands.entity(row).add_children(&[swatch, lbl, mute, lock, del]);
    row
}

fn lane_bg_snapshot(world: &World) -> KeyedSnapshot {
    let Some(s) = seq(world) else { return empty() };
    let th = s.track_height;
    let n = s.sequence.tracks.len();
    let items: Vec<(u64, u64)> = (0..n)
        .map(|i| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            th.to_bits().hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, _f, i| {
            let bg = if i.is_multiple_of(2) { row_even() } else { row_odd() };
            c.spawn((Node { width: Val::Percent(100.0), height: Val::Px(th), ..default() }, BackgroundColor(rgb(bg)))).id()
        }),
    }
}

fn ruler_snapshot(world: &World) -> KeyedSnapshot {
    let Some(s) = seq(world) else { return empty() };
    let (zoom, scroll, dur) = (s.timeline_zoom, s.timeline_scroll, s.sequence.duration);
    let interval = if zoom >= 200.0 { 0.5 } else if zoom >= 80.0 { 1.0 } else if zoom >= 30.0 { 2.0 } else { 5.0 };
    // Bound the visible range generously (the container clips overflow).
    let mut ticks: Vec<(f32, bool)> = Vec::new();
    let mut t = (scroll / interval).floor() * interval;
    let end = scroll + 4000.0 / zoom.max(1.0);
    while t <= end && t <= dur + interval {
        if t >= 0.0 {
            let major = (t % (interval * 5.0)).abs() < 0.001;
            ticks.push((t, major));
        }
        t += interval;
    }
    let items: Vec<(u64, u64)> = ticks
        .iter()
        .map(|(time, major)| {
            let mut k = hasher();
            time.to_bits().hash(&mut k);
            let mut h = hasher();
            (zoom.to_bits(), scroll.to_bits(), major).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (time, major) = ticks[i];
            let x = (time - scroll) * zoom;
            let tick = c
                .spawn(Node { position_type: PositionType::Absolute, left: Val::Px(x), top: Val::Px(if major { RULER_H - 9.0 } else { RULER_H - 4.0 }), width: Val::Px(1.0), height: Val::Px(if major { 9.0 } else { 4.0 }), ..default() })
                .insert(BackgroundColor(rgb(if major { text_muted() } else { placeholder() })))
                .id();
            if major {
                let label = if time >= 60.0 { format!("{}:{:04.1}", (time / 60.0) as u32, time % 60.0) } else { format!("{:.0}s", time) };
                c.spawn((Text::new(label), ui_font(&f.ui, 9.0), TextColor(rgb(text_muted())), Node { position_type: PositionType::Absolute, left: Val::Px(x + 2.0), top: Val::Px(1.0), ..default() }));
            }
            tick
        }),
    }
}

fn clips_snapshot(world: &World) -> KeyedSnapshot {
    let Some(s) = seq(world) else { return empty() };
    let (zoom, scroll, th) = (s.timeline_zoom, s.timeline_scroll, s.track_height);
    // (track_idx, color, is_marker, start, duration, name)
    let mut clips: Vec<(usize, (u8, u8, u8), bool, f32, f32, String)> = Vec::new();
    for (ti, track) in s.sequence.tracks.iter().enumerate() {
        let color = track_color(&track.kind);
        let is_marker = matches!(track.kind, TrackKind::Marker { .. });
        for v in track_clip_views(track) {
            clips.push((ti, color, is_marker, v.start, v.duration, v.name));
        }
    }
    let items: Vec<(u64, u64)> = clips
        .iter()
        .enumerate()
        .map(|(i, (ti, _, _, start, dur, name))| {
            let mut k = hasher();
            (i, ti).hash(&mut k);
            let mut h = hasher();
            (start.to_bits(), dur.to_bits(), name, zoom.to_bits(), scroll.to_bits(), th.to_bits()).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (ti, color, is_marker, start, dur, name) = &clips[i];
            clip_node(c, f, *ti, *color, *is_marker, *start, *dur, name, zoom, scroll, th)
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn clip_node(commands: &mut Commands, fonts: &EmberFonts, ti: usize, color: (u8, u8, u8), is_marker: bool, start: f32, dur: f32, name: &str, zoom: f32, scroll: f32, th: f32) -> Entity {
    let left = (start - scroll) * zoom;
    let top = ti as f32 * th + 2.0;
    if is_marker {
        // A thin flag bar at the marker time.
        let m = commands
            .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(left), top: Val::Px(ti as f32 * th + 1.0), width: Val::Px(2.0), height: Val::Px(th - 2.0), ..default() }, BackgroundColor(rgb(color)), bevy::ui::FocusPolicy::Pass))
            .id();
        commands.spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 9.0), TextColor(rgb(color)), Node { position_type: PositionType::Absolute, left: Val::Px(left + 5.0), top: Val::Px(ti as f32 * th + 2.0), ..default() }));
        return m;
    }
    let width = (dur * zoom).max(2.0);
    let clip = commands
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px(left), top: Val::Px(top), width: Val::Px(width), height: Val::Px(th - 4.0), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(3.0)), padding: UiRect::axes(Val::Px(5.0), Val::Px(2.0)), overflow: Overflow::clip(), ..default() },
            BackgroundColor(rgb(color).with_alpha(0.35)),
            BorderColor::all(rgb(color)),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    let lbl = commands.spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_primary())), bevy::text::TextLayout::new_with_no_wrap())).id();
    commands.entity(clip).add_child(lbl);
    clip
}

fn empty() -> KeyedSnapshot {
    KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) }
}
fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn seq_btn_click(
    q: Query<(&Interaction, &SeqBtn), Changed<Interaction>>,
    state: Option<Res<SequencerState>>,
    bridge: Option<Res<SequencerBridge>>,
) {
    let (Some(state), Some(bridge)) = (state, bridge) else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let action = match btn {
            SeqBtn::SkipBack => SequencerAction::SetPlayhead(0.0),
            SeqBtn::PlayPause => SequencerAction::TogglePlay,
            SeqBtn::Stop => SequencerAction::Stop,
            SeqBtn::SkipForward => SequencerAction::SetPlayhead(state.sequence.duration),
            SeqBtn::Loop => SequencerAction::SetLooping(!state.looping),
            SeqBtn::Key => SequencerAction::AddCameraKeyAtPlayhead,
            SeqBtn::Mark => SequencerAction::AddMarkerAtPlayhead(format!("M{:02}", marker_count(&state) + 1)),
            SeqBtn::Bake => SequencerAction::StubBakeRange { from: 0.0, to: state.sequence.duration },
            SeqBtn::ZoomIn => SequencerAction::SetZoom(state.timeline_zoom * 1.25),
            SeqBtn::ZoomOut => SequencerAction::SetZoom(state.timeline_zoom * 0.8),
            SeqBtn::AddTrack => SequencerAction::AddTrack(match state.sequence.tracks.len() % 3 {
                0 => TrackKind::Camera { clips: vec![] },
                1 => TrackKind::Transform { target_tag: "target".into(), clips: vec![] },
                _ => TrackKind::Marker { clips: vec![] },
            }),
        };
        push_action(&bridge, action);
    }
}

fn track_btn_click(
    q: Query<(&Interaction, &TrackBtn), Changed<Interaction>>,
    state: Option<Res<SequencerState>>,
    bridge: Option<Res<SequencerBridge>>,
) {
    let (Some(state), Some(bridge)) = (state, bridge) else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(track) = state.sequence.tracks.get(btn.track) else { continue };
        let action = match btn.op {
            TrackOp::Mute => SequencerAction::SetTrackMuted { track: btn.track, muted: !track.muted },
            TrackOp::Lock => SequencerAction::SetTrackLocked { track: btn.track, locked: !track.locked },
            TrackOp::Delete => SequencerAction::RemoveTrack(btn.track),
        };
        push_action(&bridge, action);
    }
}

fn seq_scrub(
    q: Query<(&Interaction, &RelativeCursorPosition, &ComputedNode), (With<ScrubLayer>, Changed<Interaction>)>,
    state: Option<Res<SequencerState>>,
    bridge: Option<Res<SequencerBridge>>,
) {
    let (Some(state), Some(bridge)) = (state, bridge) else { return };
    for (interaction, rcp, cn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(norm) = rcp.normalized else { continue };
        let width = cn.size().x * cn.inverse_scale_factor();
        let t = norm.x * width / state.timeline_zoom.max(1.0) + state.timeline_scroll;
        push_action(&bridge, SequencerAction::SetPlayhead(t.clamp(0.0, state.sequence.duration)));
    }
}

fn update_playhead(state: Option<Res<SequencerState>>, mut q: Query<&mut Node, With<Playhead>>) {
    let Some(state) = state else { return };
    let x = (state.playhead - state.timeline_scroll) * state.timeline_zoom;
    for mut node in &mut q {
        node.left = Val::Px(x.max(0.0));
        node.display = if x >= 0.0 { Display::Flex } else { Display::None };
    }
}

fn update_play_icon(state: Option<Res<SequencerState>>, mut q: Query<&mut Text, With<PlayIcon>>) {
    let Some(state) = state else { return };
    let glyph = renzora_ember::font::icon_glyph(if state.playing { "pause" } else { "play" });
    if let Some(g) = glyph {
        let s = g.to_string();
        for mut t in &mut q {
            if t.0 != s {
                t.0 = s.clone();
            }
        }
    }
}

fn marker_count(state: &SequencerState) -> usize {
    state
        .sequence
        .tracks
        .iter()
        .filter_map(|t| match &t.kind {
            TrackKind::Marker { clips } => Some(clips.len()),
            _ => None,
        })
        .sum()
}

