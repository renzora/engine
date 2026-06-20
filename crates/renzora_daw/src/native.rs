//! Bevy-native (ember) port of the egui DAW / arrangement [`crate::panel`].
//!
//! Faithfully reproduces the transport bar (skip-back, play/pause pill, stop, an
//! LCD time readout, TEMPO, GRID snap, ZOOM), the track-header column (name +
//! rename, mute/solo chips, delete, bus selector, add-track), and the
//! arrangement: a shared [`timeline_view`] shell driving ruler + striped lanes +
//! playhead + click/drag scrub, with positioned clips (selectable, waveform
//! preview, right-click delete) mounted over the lanes. An empty-state card shows
//! when there are no tracks yet.
//!
//! All mutation flows through the existing [`DawIntentBuffer`] + `apply_intents`
//! path (identical write-back to the egui panel) — the native systems just push
//! the same [`DawIntent`]s.
//!
//! Deferred (clearly): dragging an audio *file* from the asset browser onto a
//! lane / the empty state to create a clip, and dragging a clip's body to move it
//! or its right edge to resize. Both rely on egui-space `AssetDragPayload`
//! hit-testing and per-clip pixel drag math the shared `timeline_view` shell does
//! not expose; clip move/resize is still reachable programmatically via
//! `DawIntent`. Everything else is ported.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_audio::{ClipId, MixerState, TimelineState, TrackId};
use renzora::SplashState;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{
    bind_2way, bind_bg, bind_display, bind_text, bind_text_color, keyed_list, KeyedSnapshot,
};
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    bind_text_input, drag_value_flat, dropdown, menu_item_styled, screen_menu, text_input,
    timeline_view, DragRange, EmberTextInput, TimelineView,
};

use crate::panel::{DawIntent, DawIntentBuffer, TRACK_H};
use crate::waveform_cache::WaveformCache;

const PLAY_RED: (u8, u8, u8) = (220, 60, 60);

/// Registers the bevy-native DAW content + its interaction systems.
pub struct NativeDaw;

impl Plugin for NativeDaw {
    fn build(&self, app: &mut App) {
        app.register_panel_content("daw", false, build);
        app.add_systems(
            Update,
            (
                transport_btn_click,
                add_track_click,
                chip_and_trash_click,
                daw_sync,
                update_play_icon,
                clip_select_click,
                clip_context_menu,
                delete_key,
                clip_waveform_fill,
            )
                .run_if(in_state(SplashState::Editor)),
        );
    }
}

// ── World accessors ────────────────────────────────────────────────────────────

fn timeline(w: &World) -> Option<&TimelineState> {
    w.get_resource::<TimelineState>()
}

fn buffer(w: &World) -> DawIntentBuffer {
    w.get_resource::<DawIntentBuffer>().cloned().unwrap_or_default()
}

fn pick_default_bus(mixer: Option<&MixerState>) -> String {
    if let Some(m) = mixer {
        if !m.custom_buses.is_empty() {
            return m.custom_buses[0].0.clone();
        }
    }
    "Music".to_string()
}

fn available_buses(mixer: Option<&MixerState>) -> Vec<String> {
    let mut out = vec![
        "Master".to_string(),
        "Music".to_string(),
        "Sfx".to_string(),
        "Ambient".to_string(),
    ];
    if let Some(m) = mixer {
        for (name, _) in &m.custom_buses {
            out.push(name.clone());
        }
    }
    out
}

// ── Markers ─────────────────────────────────────────────────────────────────────

#[derive(Component, Clone, Copy)]
enum TransportBtn {
    SkipBack,
    PlayPause,
    Stop,
}

#[derive(Component)]
struct PlayIcon;

/// Marks the DAW's `timeline_view` root so the sync system targets it.
#[derive(Component)]
struct DawTimeline;

/// Add-a-track button — the bus it lands on is resolved at click time.
#[derive(Component)]
struct AddTrackBtn;

/// A clickable clip body carrying its id (selection + context menu).
#[derive(Component, Clone, Copy)]
struct ClipMarker(ClipId);

/// Marks the empty-state add-track button.
#[derive(Component)]
struct EmptyAddTrackBtn;

// ── Build ────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            Name::new("native-daw"),
        ))
        .id();

    let transport = build_transport(commands, fonts);

    // Body holds two mutually-exclusive children: the timeline (when there are
    // tracks) and the empty-state card (when there are none). `bind_display`
    // toggles them so the panel never shows both.
    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();

    // Shared, themeable timeline shell.
    let tl = timeline_view(commands, fonts);
    commands.entity(tl.root).insert(DawTimeline);
    bind_display(commands, tl.root, |w| {
        timeline(w).map(|t| !t.tracks.is_empty()).unwrap_or(false)
    });

    // Header corner: "Tracks" label + add-track button.
    let htitle = commands
        .spawn((Text::new("Tracks"), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted()))))
        .id();
    let hgap = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let add = icon_btn(commands, fonts, "plus", text_muted(), AddTrackBtn);
    commands.entity(tl.header_corner).add_children(&[htitle, hgap, add]);

    keyed_list(commands, tl.header_list, header_snapshot);
    keyed_list(commands, tl.clips, clips_snapshot);

    // Empty state.
    let empty = build_empty_state(commands, fonts);
    bind_display(commands, empty, |w| {
        timeline(w).map(|t| t.tracks.is_empty()).unwrap_or(true)
    });

    commands.entity(body).add_children(&[tl.root, empty]);
    commands.entity(root).add_children(&[transport, body]);
    root
}

// ── Transport bar ────────────────────────────────────────────────────────────

fn build_transport(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(46.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(rgb(header_bg())),
            BorderColor::all(rgb(border())),
            Name::new("daw-transport"),
        ))
        .id();

    // ── Transport cluster ──
    let cluster = cluster_frame(commands);
    let skip_back = icon_btn(commands, fonts, "skip-back", text_primary(), TransportBtn::SkipBack);
    let play = play_pill(commands, fonts);
    let stop = icon_btn(commands, fonts, "stop", text_primary(), TransportBtn::Stop);
    commands.entity(cluster).add_children(&[skip_back, play, stop]);

    let div1 = divider(commands);

    // ── LCD time display ──
    let lcd = lcd_panel(commands);
    let big = commands
        .spawn((Text::new(""), ui_font(&fonts.mono, 15.0), TextColor(rgb(accent()))))
        .id();
    bind_text(commands, big, lcd_bars_text);
    let small = commands
        .spawn((Text::new(""), ui_font(&fonts.mono, 10.0), TextColor(rgb(text_muted()))))
        .id();
    bind_text(commands, small, lcd_secs_text);
    commands.entity(lcd).add_children(&[big, small]);

    let div2 = divider(commands);

    // ── Tempo ──
    let tempo = labelled_chip(commands, fonts, "TEMPO", |c, f| {
        let dv = drag_value_flat(c, &f.ui, "", text_primary(), 120.0, 0.5);
        c.entity(dv).insert(DragRange { min: 20.0, max: 999.0 });
        bind_2way(
            c,
            dv,
            |w| timeline(w).map(|t| t.transport.bpm).unwrap_or(120.0),
            |w, v| buffer(w).push(DawIntent::SetBpm(*v)),
        );
        dv
    });

    let div3 = divider(commands);

    // ── Grid / snap ──
    let grid = labelled_chip(commands, fonts, "GRID", |c, f| {
        // Option order mirrors the egui combo: off, 1/4, 1/8, 1/16, 1/32.
        let dd = dropdown(c, f, &["off", "1/4", "1/8", "1/16", "1/32"], 3);
        bind_2way(
            c,
            dd,
            |w| snap_to_index(timeline(w).map(|t| t.transport.snap_div).unwrap_or(4)),
            |w, idx| buffer(w).push(DawIntent::SetSnapDiv(index_to_snap(*idx))),
        );
        dd
    });

    let gap = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();

    // ── Zoom (right edge) ──
    let zoom = labelled_chip(commands, fonts, "ZOOM", |c, f| {
        let dv = drag_value_flat(c, &f.ui, "", text_primary(), 80.0, 2.0);
        c.entity(dv).insert(DragRange { min: 20.0, max: 600.0 });
        bind_2way(
            c,
            dv,
            |w| timeline(w).map(|t| t.pixels_per_second).unwrap_or(80.0),
            |w, v| buffer(w).push(DawIntent::SetPixelsPerSecond(*v)),
        );
        dv
    });

    commands
        .entity(bar)
        .add_children(&[cluster, div1, lcd, div2, tempo, div3, grid, gap, zoom]);
    bar
}

/// `bar.beat.ticks` readout (matches egui's `{:>3}.{:>1}.{:03}`).
fn lcd_bars_text(w: &World) -> String {
    let Some(t) = timeline(w) else { return String::new() };
    let secs = t.transport.position;
    let beats = t.transport.seconds_to_beats(secs);
    let bar_n = (beats / 4.0).floor() as i32 + 1;
    let beat_in_bar = (beats % 4.0).floor() as i32 + 1;
    let ticks = ((beats - beats.floor()) * 1000.0) as i32;
    format!("{:>3}.{:>1}.{:03}", bar_n, beat_in_bar, ticks)
}

/// `mm:ss.ss` readout.
fn lcd_secs_text(w: &World) -> String {
    let Some(t) = timeline(w) else { return String::new() };
    let secs = t.transport.position;
    let mins = (secs / 60.0) as u32;
    let sec_part = secs % 60.0;
    format!("{:02}:{:05.2}s", mins, sec_part)
}

/// snap_div → dropdown index (off=0, 1/4=1, 1/8=2, 1/16=4, 1/32=8).
fn snap_to_index(snap: u32) -> usize {
    match snap {
        0 => 0,
        1 => 1,
        2 => 2,
        4 => 3,
        8 => 4,
        _ => 3,
    }
}

fn index_to_snap(idx: usize) -> u32 {
    match idx {
        0 => 0,
        1 => 1,
        2 => 2,
        3 => 4,
        4 => 8,
        _ => 4,
    }
}

// ── Transport building blocks ───────────────────────────────────────────────

fn cluster_frame(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                height: Val::Px(30.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id()
}

fn lcd_panel(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                height: Val::Px(30.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                row_gap: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id()
}

fn divider(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(1.0),
                height: Val::Px(28.0),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(border())),
        ))
        .id()
}

/// A small-caps label stacked above its control, mirroring the egui `labelled_chip`.
fn labelled_chip(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    build_value: impl FnOnce(&mut Commands, &EmberFonts) -> Entity,
) -> Entity {
    let col = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            ..default()
        })
        .id();
    let l = commands
        .spawn((Text::new(label), ui_font(&fonts.ui, 8.5), TextColor(rgb(text_muted()))))
        .id();
    let v = build_value(commands, fonts);
    commands.entity(col).add_children(&[l, v]);
    col
}

/// The big filled play / pause pill — the visual anchor of the transport bar.
fn play_pill(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let pill = commands
        .spawn((
            Node {
                width: Val::Px(34.0),
                height: Val::Px(24.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(5.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(PLAY_RED)),
            Interaction::default(),
            TransportBtn::PlayPause,
            Name::new("daw-play-pill"),
        ))
        .id();
    // Fill follows play state: accent while playing, red while stopped.
    bind_bg(commands, pill, |w| {
        let playing = timeline(w).map(|t| t.transport.is_playing()).unwrap_or(false);
        if playing {
            rgb(accent())
        } else {
            rgb(PLAY_RED)
        }
    });
    let glyph = icon_text(commands, &fonts.phosphor, "play", (255, 255, 255), 15.0);
    commands.entity(glyph).insert(PlayIcon);
    commands.entity(pill).add_child(glyph);
    pill
}

fn icon_btn<M: Component>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    color: (u8, u8, u8),
    marker: M,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(24.0),
                height: Val::Px(22.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            marker,
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, color, 14.0);
    commands.entity(btn).add_child(ic);
    btn
}

// ── Track header rows ──────────────────────────────────────────────────────────

fn header_snapshot(world: &World) -> KeyedSnapshot {
    let Some(t) = timeline(world) else { return empty() };
    let buses = available_buses(world.get_resource::<MixerState>());
    // (id, name, bus, muted, soloed, color)
    let rows: Vec<(TrackId, String, String, bool, bool, (u8, u8, u8))> = t
        .tracks
        .iter()
        .map(|tr| {
            (
                tr.id,
                tr.name.clone(),
                tr.bus_name.clone(),
                tr.muted,
                tr.soloed,
                (tr.color[0], tr.color[1], tr.color[2]),
            )
        })
        .collect();
    let buses_key = buses.join("|");
    let items: Vec<(u64, u64)> = rows
        .iter()
        .map(|(id, name, bus, muted, soloed, color)| {
            let mut k = hasher();
            id.0.hash(&mut k);
            let mut h = hasher();
            (name, bus, muted, soloed, color, &buses_key).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (id, name, bus, muted, soloed, color) = &rows[i];
            header_row(c, f, i, *id, name, bus, *muted, *soloed, *color, &buses)
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn header_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    idx: usize,
    id: TrackId,
    name: &str,
    bus: &str,
    muted: bool,
    soloed: bool,
    color: (u8, u8, u8),
    buses: &[String],
) -> Entity {
    let bg = if idx.is_multiple_of(2) { row_even() } else { row_odd() };
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(TRACK_H),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(2.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(bg)),
        ))
        .id();
    // Left accent stripe.
    let stripe = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(3.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(rgb(color)),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    commands.entity(row).add_child(stripe);

    // Row 1: name (rename field) + spacer + M / S / trash.
    let r1 = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(3.0),
            ..default()
        })
        .id();
    // Inline rename field two-way bound to the track name (committed when blurred
    // via the text-input binding). Sized to the header width.
    let name_input = text_input(commands, &fonts.ui, "name", name);
    commands.entity(name_input).insert((
        Node {
            min_width: Val::Px(0.0),
            flex_grow: 1.0,
            padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        },
        TrackNameField(id),
    ));
    bind_text_input(
        commands,
        name_input,
        {
            move |w| {
                timeline(w)
                    .and_then(|t| t.tracks.iter().find(|tr| tr.id == id))
                    .map(|tr| tr.name.clone())
                    .unwrap_or_default()
            }
        },
        {
            move |w, v| {
                let v = v.trim().to_string();
                if !v.is_empty() {
                    buffer(w).push(DawIntent::SetTrackName(id, v));
                }
            }
        },
    );

    let gap = commands.spawn(Node { flex_grow: 1.0, min_width: Val::Px(2.0), ..default() }).id();
    let mute = state_chip(commands, fonts, "M", id, ChipKind::Mute, muted, close_red());
    let solo = state_chip(commands, fonts, "S", id, ChipKind::Solo, soloed, warn_amber());
    let trash = track_icon_btn(commands, fonts, "trash", id, TrackOp::Delete);
    commands.entity(r1).add_children(&[name_input, gap, mute, solo, trash]);

    // Row 2: bus selector (a compact dropdown of available buses).
    let r2 = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            ..default()
        })
        .id();
    let sel = buses.iter().position(|b| b == bus).unwrap_or(1);
    let bus_opts: Vec<&str> = buses.iter().map(|s| s.as_str()).collect();
    let bus_dd = dropdown(commands, fonts, &bus_opts, sel);
    commands.entity(bus_dd).insert(Node {
        min_width: Val::Px(96.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(6.0),
        padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
        border_radius: BorderRadius::all(Val::Px(3.0)),
        position_type: PositionType::Relative,
        ..default()
    });
    let buses_owned = buses.to_vec();
    bind_2way(
        commands,
        bus_dd,
        {
            let buses_owned = buses_owned.clone();
            move |w| {
                let cur = timeline(w)
                    .and_then(|t| t.tracks.iter().find(|tr| tr.id == id))
                    .map(|tr| tr.bus_name.clone())
                    .unwrap_or_default();
                buses_owned.iter().position(|b| *b == cur).unwrap_or(1)
            }
        },
        {
            move |w, idx| {
                if let Some(name) = buses_owned.get(*idx) {
                    buffer(w).push(DawIntent::SetTrackBus(id, name.clone()));
                }
            }
        },
    );
    commands.entity(r2).add_child(bus_dd);

    commands.entity(row).add_children(&[r1, r2]);
    row
}

/// Identifies a track-name rename field (currently informational; the binding
/// handles commit). Kept so a future "double-click to focus" gesture can target it.
#[derive(Component)]
struct TrackNameField(#[allow(dead_code)] TrackId);

#[derive(Clone, Copy)]
enum ChipKind {
    Mute,
    Solo,
}

#[derive(Component)]
struct StateChip {
    track: TrackId,
    kind: ChipKind,
}

/// Compact ON/OFF chip (M/S). Active fills with `accent`; inactive is a faint
/// outlined box. Two-way reactive: bg/text follow the track flag, click toggles.
fn state_chip(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    id: TrackId,
    kind: ChipKind,
    active: bool,
    accent_col: (u8, u8, u8),
) -> Entity {
    let chip = commands
        .spawn((
            Node {
                width: Val::Px(16.0),
                height: Val::Px(16.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(if active { rgb(accent_col) } else { Color::NONE }),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            StateChip { track: id, kind },
        ))
        .id();
    let txt = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 9.5),
            TextColor(rgb(if active { (255, 255, 255) } else { text_muted() })),
        ))
        .id();
    commands.entity(chip).add_child(txt);

    bind_bg(commands, chip, move |w| {
        if chip_active(w, id, kind) {
            rgb(accent_col)
        } else {
            Color::NONE
        }
    });
    bind_text_color(commands, txt, move |w| {
        if chip_active(w, id, kind) {
            Color::WHITE
        } else {
            rgb(text_muted())
        }
    });
    chip
}

fn chip_active(w: &World, id: TrackId, kind: ChipKind) -> bool {
    timeline(w)
        .and_then(|t| t.tracks.iter().find(|tr| tr.id == id))
        .map(|tr| match kind {
            ChipKind::Mute => tr.muted,
            ChipKind::Solo => tr.soloed,
        })
        .unwrap_or(false)
}

#[derive(Clone, Copy)]
enum TrackOp {
    Delete,
}

#[derive(Component)]
struct TrackOpBtn {
    track: TrackId,
    op: TrackOp,
}

fn track_icon_btn(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    id: TrackId,
    op: TrackOp,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(16.0),
                height: Val::Px(16.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(2.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            TrackOpBtn { track: id, op },
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 11.0);
    commands.entity(btn).add_child(ic);
    btn
}

// ── Clips ───────────────────────────────────────────────────────────────────

fn clips_snapshot(world: &World) -> KeyedSnapshot {
    let Some(t) = timeline(world) else { return empty() };
    let pps = t.pixels_per_second.max(20.0);
    let selected = t.selected_clip;
    // (clip_id, track_idx, color, muted, selected, start, length, name, source)
    let mut clips: Vec<(ClipId, usize, (u8, u8, u8), bool, bool, f64, f64, String, std::path::PathBuf)> =
        Vec::new();
    for (ti, track) in t.tracks.iter().enumerate() {
        let color = (track.color[0], track.color[1], track.color[2]);
        for clip in t.clips.iter().filter(|c| c.track == track.id) {
            clips.push((
                clip.id,
                ti,
                color,
                clip.muted,
                Some(clip.id) == selected,
                clip.start,
                clip.length,
                clip.name.clone(),
                clip.source.clone(),
            ));
        }
    }
    let items: Vec<(u64, u64)> = clips
        .iter()
        .map(|(id, ti, color, muted, sel, start, len, name, _)| {
            let mut k = hasher();
            id.0.hash(&mut k);
            let mut h = hasher();
            (
                ti,
                color,
                muted,
                sel,
                start.to_bits(),
                len.to_bits(),
                name,
                pps.to_bits(),
            )
                .hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (id, ti, color, muted, sel, start, len, name, source) = &clips[i];
            clip_node(c, f, *id, *ti, *color, *muted, *sel, *start, *len, name, source, pps)
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn clip_node(
    commands: &mut Commands,
    fonts: &EmberFonts,
    id: ClipId,
    ti: usize,
    color: (u8, u8, u8),
    muted: bool,
    selected: bool,
    start: f64,
    length: f64,
    name: &str,
    source: &std::path::Path,
    pps: f32,
) -> Entity {
    let left = (start * pps as f64) as f32;
    let width = ((length * pps as f64) as f32).max(4.0);
    let top = ti as f32 * TRACK_H + 4.0;
    let height = (TRACK_H - 8.0).max(4.0);
    let fill = rgb(color).with_alpha(if muted { 0.18 } else { 0.43 });

    let clip = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left),
                top: Val::Px(top),
                width: Val::Px(width),
                height: Val::Px(height),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(if selected { 2.0 } else { 1.0 })),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                padding: UiRect::axes(Val::Px(5.0), Val::Px(2.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(fill),
            BorderColor::all(rgb(color)),
            Interaction::default(),
            ClipMarker(id),
            Name::new("daw-clip"),
        ))
        .id();

    let lbl = commands
        .spawn((
            Text::new(name.to_string()),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    commands.entity(clip).add_child(lbl);

    // Waveform — a lightweight bevy_ui approximation of the egui peak painter:
    // a row of vertical bars sized from the cached `(min,max)` peaks. (The
    // GPU `waveform` widget is capped at 32 samples + fixed size, so it's a poor
    // fit for variable-width arrangement clips; this bar field scales cleanly.)
    if width >= 8.0 && height >= 14.0 {
        let wf = build_clip_waveform(commands, source, color, width, height);
        commands.entity(clip).add_child(wf);
    }
    clip
}

/// Build the per-clip waveform bar field from the [`WaveformCache`]. Bars are
/// laid out in an absolutely-positioned row below the title; the bar count is
/// downsampled to roughly one bar per 2px of clip width to stay cheap.
fn build_clip_waveform(
    commands: &mut Commands,
    source: &std::path::Path,
    color: (u8, u8, u8),
    width: f32,
    height: f32,
) -> Entity {
    let container = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(2.0),
                right: Val::Px(2.0),
                top: Val::Px(16.0),
                bottom: Val::Px(2.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
            ClipWaveform {
                source: source.to_path_buf(),
                color,
                width,
                height,
            },
            Name::new("daw-clip-waveform"),
        ))
        .id();
    container
}

/// Marks a clip's waveform container; `clip_waveform_fill` populates it once the
/// peaks land in the cache (decoding is async).
#[derive(Component)]
struct ClipWaveform {
    source: std::path::PathBuf,
    color: (u8, u8, u8),
    width: f32,
    height: f32,
}

#[derive(Component)]
struct ClipWaveformFilled;

// ── Empty state ────────────────────────────────────────────────────────────

fn build_empty_state(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(8.0),
                ..default()
            },
            Name::new("daw-empty-state"),
        ))
        .id();
    let icon = icon_text(commands, &fonts.phosphor, "waveform", text_muted(), 40.0);
    let title = commands
        .spawn((Text::new("No tracks yet"), ui_font(&fonts.ui, 14.0), TextColor(rgb(text_primary()))))
        .id();
    let body = commands
        .spawn((
            Text::new("Add a track to get started, then drop audio onto its lane."),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let btn = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            Interaction::default(),
            EmptyAddTrackBtn,
        ))
        .id();
    let bi = icon_text(commands, &fonts.phosphor, "plus", text_primary(), 12.0);
    let bl = commands
        .spawn((Text::new("Add audio track"), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary()))))
        .id();
    commands.entity(btn).add_children(&[bi, bl]);
    commands.entity(root).add_children(&[icon, title, body, btn]);
    root
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn empty() -> KeyedSnapshot {
    KeyedSnapshot {
        items: Vec::new(),
        build: Box::new(|c, _, _| c.spawn(Node::default()).id()),
    }
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

// ── Systems ─────────────────────────────────────────────────────────────────

fn transport_btn_click(
    q: Query<(&Interaction, &TransportBtn), Changed<Interaction>>,
    timeline: Option<Res<TimelineState>>,
    buffer: Option<Res<DawIntentBuffer>>,
) {
    let (Some(timeline), Some(buffer)) = (timeline, buffer) else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match btn {
            TransportBtn::SkipBack => buffer.push(DawIntent::SeekTo(0.0)),
            TransportBtn::Stop => buffer.push(DawIntent::Stop),
            TransportBtn::PlayPause => {
                if timeline.transport.is_playing() {
                    buffer.push(DawIntent::Stop);
                } else {
                    buffer.push(DawIntent::Play);
                }
            }
        }
    }
}

/// Both add-track buttons (header-corner + empty-state) resolve their default
/// bus from the mixer and push `AddTrack`.
fn add_track_click(
    corner: Query<&Interaction, (With<AddTrackBtn>, Changed<Interaction>)>,
    empty: Query<&Interaction, (With<EmptyAddTrackBtn>, Changed<Interaction>)>,
    mixer: Option<Res<MixerState>>,
    buffer: Option<Res<DawIntentBuffer>>,
) {
    let Some(buffer) = buffer else { return };
    let clicked = corner.iter().chain(empty.iter()).any(|i| *i == Interaction::Pressed);
    if clicked {
        let bus = pick_default_bus(mixer.as_deref());
        buffer.push(DawIntent::AddTrack { bus });
    }
}

/// Mute/solo chip + trash clicks → toggle the flag / remove the track.
fn chip_and_trash_click(
    chips: Query<(&Interaction, &StateChip), Changed<Interaction>>,
    ops: Query<(&Interaction, &TrackOpBtn), Changed<Interaction>>,
    timeline: Option<Res<TimelineState>>,
    buffer: Option<Res<DawIntentBuffer>>,
) {
    let (Some(timeline), Some(buffer)) = (timeline, buffer) else { return };
    for (interaction, chip) in &chips {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(tr) = timeline.tracks.iter().find(|t| t.id == chip.track) else { continue };
        match chip.kind {
            ChipKind::Mute => buffer.push(DawIntent::SetTrackMute(chip.track, !tr.muted)),
            ChipKind::Solo => buffer.push(DawIntent::SetTrackSolo(chip.track, !tr.soloed)),
        }
    }
    for (interaction, op) in &ops {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match op.op {
            TrackOp::Delete => buffer.push(DawIntent::RemoveTrack(op.track)),
        }
    }
}

/// Push DAW geometry into the shared timeline + apply scrubbing (→ SeekTo).
fn daw_sync(
    mut q: Query<&mut TimelineView, With<DawTimeline>>,
    timeline: Option<Res<TimelineState>>,
    buffer: Option<Res<DawIntentBuffer>>,
) {
    let Some(timeline) = timeline else { return };
    for mut v in &mut q {
        v.set_geom(
            timeline.pixels_per_second.max(20.0),
            0.0,
            timeline.transport.position as f32,
            timeline.view_duration as f32,
            TRACK_H,
            timeline.tracks.len(),
        );
        if let Some(t) = v.take_scrub() {
            if let Some(buffer) = &buffer {
                let snapped = timeline.transport.snap_seconds(t as f64);
                buffer.push(DawIntent::SeekTo(snapped));
            }
        }
    }
}

fn update_play_icon(
    timeline: Option<Res<TimelineState>>,
    mut q: Query<&mut Text, With<PlayIcon>>,
) {
    let Some(timeline) = timeline else { return };
    let glyph = renzora_ember::font::icon_glyph(if timeline.transport.is_playing() {
        "pause"
    } else {
        "play"
    });
    if let Some(g) = glyph {
        let s = g.to_string();
        for mut t in &mut q {
            if t.0 != s {
                t.0 = s.clone();
            }
        }
    }
}

/// Click a clip body → select it.
fn clip_select_click(
    q: Query<(&Interaction, &ClipMarker), Changed<Interaction>>,
    buffer: Option<Res<DawIntentBuffer>>,
) {
    let Some(buffer) = buffer else { return };
    for (interaction, clip) in &q {
        if *interaction == Interaction::Pressed {
            buffer.push(DawIntent::SelectClip(Some(clip.0)));
        }
    }
}

/// Right-click a clip → a one-item "Delete clip" context menu (ember `screen_menu`).
fn clip_context_menu(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    clips: Query<(&Interaction, &ClipMarker)>,
    buffer: Option<Res<DawIntentBuffer>>,
    mut commands: Commands,
) {
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    let (Some(fonts), Some(buffer)) = (fonts, buffer) else { return };
    // The clip the cursor is over (hovered or pressed).
    let Some((_, clip)) = clips
        .iter()
        .find(|(i, _)| matches!(i, Interaction::Hovered | Interaction::Pressed))
    else {
        return;
    };
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else { return };
    let id = clip.0;
    let buffer = buffer.clone();
    let menu = screen_menu(&mut commands, cursor.x, cursor.y);
    let del = menu_item_styled(
        &mut commands,
        &fonts,
        "trash",
        "Delete clip",
        close_red(),
        close_red(),
        move |_| {
            buffer.push(DawIntent::RemoveClip(id));
            buffer.push(DawIntent::SelectClip(None));
        },
    );
    commands.entity(menu).add_child(del);
}

/// Delete / Backspace removes the selected clip (only while the DAW timeline is
/// hovered, so a Delete elsewhere doesn't wipe the selection). A focused text
/// field (track rename) suppresses it.
fn delete_key(
    keys: Res<ButtonInput<KeyCode>>,
    hovered: Query<&Interaction, With<ClipMarker>>,
    tl_hover: Query<&Interaction, With<DawTimeline>>,
    inputs: Query<&EmberTextInput>,
    timeline: Option<Res<TimelineState>>,
    buffer: Option<Res<DawIntentBuffer>>,
) {
    if !keys.just_pressed(KeyCode::Delete) && !keys.just_pressed(KeyCode::Backspace) {
        return;
    }
    let (Some(timeline), Some(buffer)) = (timeline, buffer) else { return };
    let Some(clip_id) = timeline.selected_clip else { return };
    if inputs.iter().any(|i| i.focused) {
        return;
    }
    // Only when the timeline / a clip is under the cursor.
    let over = hovered.iter().chain(tl_hover.iter()).any(|i| *i != Interaction::None);
    if over {
        buffer.push(DawIntent::RemoveClip(clip_id));
        buffer.push(DawIntent::SelectClip(None));
    }
}

/// Fill clip waveforms once their peaks decode (the cache loads async on a worker
/// thread), and re-fill if the clip's geometry changed (rebuilt entity).
fn clip_waveform_fill(
    mut commands: Commands,
    cache: Option<Res<WaveformCache>>,
    pending: Query<(Entity, &ClipWaveform), Without<ClipWaveformFilled>>,
) {
    let Some(cache) = cache else { return };
    for (entity, wf) in &pending {
        let Some(peaks) = cache.get(&wf.source) else { continue };
        if peaks.peaks.is_empty() {
            commands.entity(entity).insert(ClipWaveformFilled);
            continue;
        }
        // Downsample to ~1 bar per 2px to keep the node count modest.
        let bars = ((wf.width / 2.0) as usize).clamp(4, peaks.peaks.len());
        let stroke = rgb(wf.color).with_alpha(0.85);
        let usable_h = (wf.height - 18.0).max(4.0);
        let mut kids: Vec<Entity> = Vec::with_capacity(bars);
        for b in 0..bars {
            let src_i = b * peaks.peaks.len() / bars;
            let (mn, mx) = peaks.peaks[src_i];
            let amp = (mx - mn).clamp(0.0, 2.0) * 0.5; // 0..1 envelope.
            let h = (amp * usable_h).max(1.0);
            let bar = commands
                .spawn((
                    Node {
                        flex_grow: 1.0,
                        height: Val::Px(h),
                        margin: UiRect::horizontal(Val::Px(0.0)),
                        ..default()
                    },
                    BackgroundColor(stroke),
                    bevy::ui::FocusPolicy::Pass,
                ))
                .id();
            kids.push(bar);
        }
        commands.entity(entity).insert(ClipWaveformFilled);
        commands.entity(entity).add_children(&kids);
    }
}
