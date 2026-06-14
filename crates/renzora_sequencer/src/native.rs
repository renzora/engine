//! Bevy-native (ember) port of the egui `SequencerPanel`: transport toolbar,
//! track headers (mute/lock/delete), a time ruler, clip lanes with positioned
//! clips + keyframe ticks, a draggable playhead, and click-to-scrub. Clip
//! drag/resize and wheel zoom/pan are deferred (toolbar zoom buttons work).

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora::SplashState;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_text, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{timeline_view, TimelineView, LANE_INSET};

use crate::model::TrackKind;
use crate::runtime::{push_action, track_clip_views, SequencerAction, SequencerBridge, SequencerState};

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
            (seq_btn_click, track_btn_click, seq_sync, update_play_icon).run_if(in_state(SplashState::Editor)),
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
struct PlayIcon;
/// Marks the sequencer's `timeline_view` root so the sync system targets it.
#[derive(Component)]
struct SeqTimeline;

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

    // Shared, themeable timeline shell — we just mount headers + clips and sync.
    let tl = timeline_view(commands, fonts);
    commands.entity(tl.root).insert(SeqTimeline);

    // Header corner: "Tracks" label + add-track button.
    let htitle = commands.spawn((Text::new("Tracks"), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())))).id();
    let hgap = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let add = icon_btn(commands, fonts, "plus", text_muted(), SeqBtn::AddTrack);
    commands.entity(tl.header_corner).add_children(&[htitle, hgap, add]);

    keyed_list(commands, tl.header_list, header_snapshot);
    keyed_list(commands, tl.clips, clips_snapshot);

    commands.entity(root).add_children(&[toolbar, tl.root]);
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
    let left = (start - scroll) * zoom + LANE_INSET;
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

/// Push the sequencer's geometry into the shared timeline + apply scrubbing.
fn seq_sync(
    mut q: Query<&mut TimelineView, With<SeqTimeline>>,
    state: Option<Res<SequencerState>>,
    bridge: Option<Res<SequencerBridge>>,
) {
    let Some(state) = state else { return };
    for mut v in &mut q {
        v.set_geom(
            state.timeline_zoom,
            state.timeline_scroll,
            state.playhead,
            state.sequence.duration,
            state.track_height,
            state.sequence.tracks.len(),
        );
        if let Some(t) = v.take_scrub() {
            if let Some(bridge) = &bridge {
                push_action(bridge, SequencerAction::SetPlayhead(t.clamp(0.0, state.sequence.duration)));
            }
        }
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

