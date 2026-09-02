//! The three keyed lists the timeline draws: track headers, keyframes, markers.
//!
//! The keyframe list clusters dense runs into bars rather than drawing a wall of
//! overlapping diamonds — a baked 30 Hz capture is thousands of keys, and the
//! reactive list rebuilds on every dirty frame. Zooming in past the threshold
//! splits a bar back into individually editable diamonds.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::ui::UiTransform;

use renzora::{PropertyTrack, TrackValue};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::LANE_INSET;

use super::clip::{cur_clip, state, Lane, SelKey, SelectedKey};
use super::{
    AddKeyTrackBtn, DeletePropTrack, PropTrackCombo, MARKER, PROPERTY, ROTATION, SCALE,
    TRANSLATION,
};

/// Toolbar readout for the selected keyframe: "Rotation @ 1.33s = (…)". Empty
/// when nothing is selected. Rotation values are shown as Euler degrees.
pub(super) fn selected_key_label(w: &Rx) -> String {
    let Some(sel) = w.get_resource::<SelectedKey>().and_then(|s| s.0) else { return String::new() };
    let Some(clip) = cur_clip(w.untracked()) else { return String::new() };
    match sel.lane {
        Lane::Prop { track } => {
            let Some(pt) = clip.property_tracks.get(track) else { return String::new() };
            let Some(key) = pt.keys.get(sel.index) else { return String::new() };
            format!("{} @ {:.2}s = {}", title_case(&pt.field), key.time, fmt_track_value(&key.value))
        }
        Lane::Bone { track, channel } => {
            let Some(bt) = clip.tracks.get(track) else { return String::new() };
            let parts = match channel {
                0 => bt.translations.get(sel.index).map(|(t, v)| ("Translation", *t, fmt_vec3(v))),
                1 => bt.rotations.get(sel.index).map(|(t, v)| ("Rotation", *t, fmt_quat(v))),
                _ => bt.scales.get(sel.index).map(|(t, v)| ("Scale", *t, fmt_vec3(v))),
            };
            match parts {
                Some((label, time, val)) => format!("{} @ {:.2}s = {}", label, time, val),
                None => String::new(),
            }
        }
    }
}

fn fmt_vec3(v: &[f32; 3]) -> String {
    format!("({:.2}, {:.2}, {:.2})", v[0], v[1], v[2])
}

fn fmt_quat(v: &[f32; 4]) -> String {
    // YXZ so a keyed spin reads as yaw climbing, not as X/Z snapping to ±180
    // every time it crosses 90° — same order the inspector shows.
    let (y, x, z) = bevy::prelude::Quat::from_array(*v).to_euler(bevy::math::EulerRot::YXZ);
    format!("({:.0}°, {:.0}°, {:.0}°)", x.to_degrees(), y.to_degrees(), z.to_degrees())
}

fn fmt_track_value(v: &TrackValue) -> String {
    match v {
        TrackValue::Float(x) => format!("{:.3}", x),
        TrackValue::Vec3(a) => fmt_vec3(a),
        TrackValue::Quat(a) => fmt_quat(a),
        TrackValue::Color(a) => format!("({:.2}, {:.2}, {:.2}, {:.2})", a[0], a[1], a[2], a[3]),
        TrackValue::Bool(b) => b.to_string(),
    }
}

// ── Track headers ────────────────────────────────────────────────────────────

/// A track-header row: a skeletal bone (T/R/S channels) or a property track.
enum HeaderRow {
    Bone { name: String, ht: bool, hr: bool, hs: bool },
    /// Property track: index + its label (`None` until a property is picked).
    Prop { track: usize, label: Option<String> },
}

pub(super) fn header_snapshot(world: &Rx) -> KeyedSnapshot {
    let Some(clip) = cur_clip(world.untracked()) else { return empty() };
    let th = state(world.untracked()).map(|s| s.track_height).unwrap_or(22.0);
    let mut rows: Vec<HeaderRow> = clip
        .tracks
        .iter()
        .map(|t| HeaderRow::Bone {
            name: t.bone_name.clone(),
            ht: !t.translations.is_empty(),
            hr: !t.rotations.is_empty(),
            hs: !t.scales.is_empty(),
        })
        .collect();
    for (pi, pt) in clip.property_tracks.iter().enumerate() {
        let label = if pt.component.is_empty() { None } else { Some(property_label(pt)) };
        rows.push(HeaderRow::Prop { track: pi, label });
    }
    let items: Vec<(u64, u64)> = rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            match row {
                HeaderRow::Bone { name, ht, hr, hs } => (0u8, name, ht, hr, hs, th.to_bits()).hash(&mut h),
                HeaderRow::Prop { track, label } => (1u8, track, label, th.to_bits()).hash(&mut h),
            }
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| match &rows[i] {
            HeaderRow::Bone { name, ht, hr, hs } => header_row(c, f, i, name, *ht, *hr, *hs, th),
            HeaderRow::Prop { track, label } => prop_header_row(c, f, i, *track, label.as_deref(), th),
        }),
    }
}

/// Header label for a property track: "Component · Field" with the target node
/// in parentheses when not the animator entity itself.
fn property_label(pt: &PropertyTrack) -> String {
    let base = format!("{} · {}", title_case(&pt.component), title_case(&pt.field));
    if pt.target.is_empty() || pt.target == "self" {
        base
    } else {
        format!("{} ({})", base, pt.target)
    }
}

pub(super) fn title_case(s: &str) -> String {
    s.replace(['_', '.'], " ")
        .split_whitespace()
        .map(|w| {
            let mut ch = w.chars();
            match ch.next() {
                Some(f) => f.to_uppercase().collect::<String>() + ch.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A property-track header row: property icon + a clickable property dropdown
/// ("Select property…" until picked) + a delete button.
fn prop_header_row(commands: &mut Commands, fonts: &EmberFonts, idx: usize, track: usize, label: Option<&str>, th: f32) -> Entity {
    let bg = if idx.is_multiple_of(2) { row_even() } else { row_odd() };
    let row = commands
        .spawn((Node { width: Val::Percent(100.0), height: Val::Px(th), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::horizontal(Val::Px(6.0)), ..default() }, BackgroundColor(rgb(bg))))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, "sliders-horizontal", PROPERTY, 10.0);

    // Property dropdown (combo): click to pick / change the bound property.
    let combo = commands
        .spawn((
            Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            PropTrackCombo(track),
        ))
        .id();
    let (txt, col) = match label {
        Some(l) => (l.to_string(), text_primary()),
        None => (renzora::lang::t("timeline.select_property"), text_muted()),
    };
    let combo_lbl = commands.spawn((Text::new(txt), ui_font(&fonts.ui, 11.0), TextColor(rgb(col)), bevy::text::TextLayout::no_wrap(), Node { flex_grow: 1.0, overflow: Overflow::clip(), ..default() })).id();
    let caret = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 8.0);
    commands.entity(combo).add_children(&[combo_lbl, caret]);

    // Add-key button (keys THIS track at the playhead from the live value).
    let addk = commands
        .spawn((Node { width: Val::Px(16.0), height: Val::Px(16.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, flex_shrink: 0.0, ..default() }, BackgroundColor(Color::NONE), Interaction::default(), AddKeyTrackBtn(track)))
        .id();
    let addk_ic = icon_text(commands, &fonts.phosphor, "diamond", PROPERTY, 11.0);
    commands.entity(addk).add_child(addk_ic);

    // Delete-track button.
    let del = commands
        .spawn((Node { width: Val::Px(16.0), height: Val::Px(16.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, flex_shrink: 0.0, ..default() }, BackgroundColor(Color::NONE), Interaction::default(), DeletePropTrack(track)))
        .id();
    let del_ic = icon_text(commands, &fonts.phosphor, "x", text_muted(), 11.0);
    commands.entity(del).add_child(del_ic);

    commands.entity(row).add_children(&[ic, combo, addk, del]);
    row
}

#[allow(clippy::too_many_arguments)]
fn header_row(commands: &mut Commands, fonts: &EmberFonts, idx: usize, name: &str, ht: bool, hr: bool, hs: bool, th: f32) -> Entity {
    let bg = if idx.is_multiple_of(2) { row_even() } else { row_odd() };
    let row = commands
        .spawn((Node { width: Val::Percent(100.0), height: Val::Px(th), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::horizontal(Val::Px(6.0)), ..default() }, BackgroundColor(rgb(bg))))
        .id();
    let bone = icon_text(commands, &fonts.phosphor, "bone", text_muted(), 10.0);
    let lbl = commands.spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), bevy::text::TextLayout::no_wrap(), Node { flex_grow: 1.0, overflow: Overflow::clip(), ..default() })).id();
    let t = channel_letter(commands, fonts, "T", ht, TRANSLATION);
    let r = channel_letter(commands, fonts, "R", hr, ROTATION);
    let s = channel_letter(commands, fonts, "S", hs, SCALE);
    commands.entity(row).add_children(&[bone, lbl, t, r, s]);
    row
}

fn channel_letter(commands: &mut Commands, fonts: &EmberFonts, ch: &str, active: bool, color: (u8, u8, u8)) -> Entity {
    let col = if active { color } else { text_muted() };
    commands.spawn((Text::new(ch.to_string()), ui_font(&fonts.ui, 9.0), TextColor(rgb(col)))).id()
}

// ── Keyframes ────────────────────────────────────────────────────────────────

/// One renderable timeline element after clustering.
#[derive(Clone, Copy)]
enum KeyElem {
    /// A lone (editable) bone keyframe: (track, channel, key index, time).
    Key(usize, u8, usize, f32),
    /// A run of bone keys denser than the cluster threshold, drawn as one bar:
    /// (track, channel, first time, last time, count).
    Bar(usize, u8, f32, f32, usize),
    /// A lone (editable) property keyframe: (property track, key index, time).
    PKey(usize, usize, f32),
    /// A dense run of property keys drawn as one bar: (property track, first,
    /// last, count).
    PBar(usize, f32, f32, usize),
}

/// Keys closer together than this many pixels merge into a bar. Baked 30 Hz
/// captures render as clean per-channel range bars instead of a wall of
/// overlapping diamonds; zooming in past the threshold reveals (editable)
/// individual keys.
pub(super) const CLUSTER_PX: f32 = 9.0;
/// How far past the visible window keys are still spawned, in pixels.
const CULL_MARGIN_PX: f32 = 64.0;
/// Upper bound on the lane width used for culling — the actual panel is
/// narrower, so this only ever over-includes slightly.
const MAX_LANE_PX: f32 = 4096.0;

/// Cluster one channel's sorted key list into renderable elements, culled to
/// the visible window. `mk_key`/`mk_bar` build the lane-specific [`KeyElem`]
/// (bone channel vs property track) from `(index, time)` / `(t0, t1, count)`.
fn cluster_channel(
    out: &mut Vec<KeyElem>,
    times: impl Iterator<Item = f32>,
    zoom: f32,
    t_min: f32,
    t_max: f32,
    mk_key: impl Fn(usize, f32) -> KeyElem,
    mk_bar: impl Fn(f32, f32, usize) -> KeyElem,
) {
    let gap = CLUSTER_PX / zoom;
    // (first index, first time, last time, count) of the open cluster.
    let mut run: Option<(usize, f32, f32, usize)> = None;
    let flush = |out: &mut Vec<KeyElem>, run: (usize, f32, f32, usize)| {
        let (i0, t0, t1, n) = run;
        if n >= 3 {
            out.push(mk_bar(t0, t1, n));
        } else {
            for k in 0..n {
                // 1–2 keys: emit individually.
                let t = if k == 0 { t0 } else { t1 };
                out.push(mk_key(i0 + k, t));
            }
        }
    };
    for (idx, t) in times.enumerate() {
        if t < t_min - gap || t > t_max + gap {
            // Outside the window — close any open run that ended in view.
            if let Some(r) = run.take() {
                flush(out, r);
            }
            continue;
        }
        match run.as_mut() {
            Some((_, _, last, n)) if t - *last <= gap => {
                *last = t;
                *n += 1;
            }
            Some(_) => {
                let r = run.take().unwrap();
                flush(out, r);
                run = Some((idx, t, t, 1));
            }
            None => run = Some((idx, t, t, 1)),
        }
    }
    if let Some(r) = run.take() {
        flush(out, r);
    }
}

/// Whether a clustered element corresponds to the selected keyframe.
fn elem_selected(e: &KeyElem, sel: Option<SelKey>) -> bool {
    let Some(sel) = sel else { return false };
    match (e, sel.lane) {
        (KeyElem::Key(ti, ch, idx, _), Lane::Bone { track, channel }) => {
            *ti == track && *ch == channel && *idx == sel.index
        }
        (KeyElem::PKey(pi, idx, _), Lane::Prop { track }) => *pi == track && *idx == sel.index,
        _ => false,
    }
}

pub(super) fn keyframe_snapshot(world: &Rx) -> KeyedSnapshot {
    let Some(clip) = cur_clip(world.untracked()) else { return empty() };
    let Some(s) = state(world.untracked()) else { return empty() };
    let sel = world.get_resource::<SelectedKey>().and_then(|s| s.0);
    let (zoom, scroll, th) = (s.timeline_zoom, s.timeline_scroll, s.track_height);
    let t_min = scroll - CULL_MARGIN_PX / zoom;
    let t_max = scroll + (MAX_LANE_PX + CULL_MARGIN_PX) / zoom;

    let bone_count = clip.tracks.len();
    let mut elems: Vec<KeyElem> = Vec::new();
    for (ti, track) in clip.tracks.iter().enumerate() {
        cluster_channel(&mut elems, track.translations.iter().map(|k| k.0), zoom, t_min, t_max,
            move |idx, t| KeyElem::Key(ti, 0, idx, t), move |t0, t1, n| KeyElem::Bar(ti, 0, t0, t1, n));
        cluster_channel(&mut elems, track.rotations.iter().map(|k| k.0), zoom, t_min, t_max,
            move |idx, t| KeyElem::Key(ti, 1, idx, t), move |t0, t1, n| KeyElem::Bar(ti, 1, t0, t1, n));
        cluster_channel(&mut elems, track.scales.iter().map(|k| k.0), zoom, t_min, t_max,
            move |idx, t| KeyElem::Key(ti, 2, idx, t), move |t0, t1, n| KeyElem::Bar(ti, 2, t0, t1, n));
    }
    for (pi, pt) in clip.property_tracks.iter().enumerate() {
        cluster_channel(&mut elems, pt.keys.iter().map(|k| k.time), zoom, t_min, t_max,
            move |idx, t| KeyElem::PKey(pi, idx, t), move |t0, t1, n| KeyElem::PBar(pi, t0, t1, n));
    }

    let items: Vec<(u64, u64)> = elems
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            match e {
                KeyElem::Key(ti, ch, idx, time) => {
                    (0u8, ti, ch, idx, time.to_bits()).hash(&mut h)
                }
                KeyElem::Bar(ti, ch, t0, t1, n) => {
                    (1u8, ti, ch, t0.to_bits(), t1.to_bits(), n).hash(&mut h)
                }
                KeyElem::PKey(pi, idx, time) => {
                    (2u8, pi, idx, time.to_bits()).hash(&mut h)
                }
                KeyElem::PBar(pi, t0, t1, n) => {
                    (3u8, pi, t0.to_bits(), t1.to_bits(), n).hash(&mut h)
                }
            }
            (zoom.to_bits(), scroll.to_bits(), th.to_bits()).hash(&mut h);
            elem_selected(e, sel).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, _f, i| {
            let sd = elem_selected(&elems[i], sel);
            match elems[i] {
                KeyElem::Key(ti, ch, idx, time) => diamond(c, ti, ch, idx, time, zoom, scroll, th, sd),
                KeyElem::Bar(ti, ch, t0, t1, _) => key_bar(c, ti, ch, t0, t1, zoom, scroll, th),
                KeyElem::PKey(pi, _idx, time) => prop_diamond(c, bone_count + pi, time, zoom, scroll, th, sd),
                KeyElem::PBar(pi, t0, t1, _) => prop_bar(c, bone_count + pi, t0, t1, zoom, scroll, th),
            }
        }),
    }
}

/// Per-channel vertical placement within a track lane.
pub(super) fn channel_y(ti: usize, ch: u8, th: f32) -> (f32, (u8, u8, u8)) {
    let off = (th * 0.26).min(14.0);
    let center = ti as f32 * th + th * 0.5;
    match ch {
        0 => (center - off, TRANSLATION),
        1 => (center, ROTATION),
        _ => (center + off, SCALE),
    }
}

#[allow(clippy::too_many_arguments)]
fn diamond(commands: &mut Commands, ti: usize, ch: u8, idx: usize, time: f32, zoom: f32, scroll: f32, th: f32, selected: bool) -> Entity {
    let kf = (th * 0.38).clamp(4.0, 14.0) + if selected { 3.0 } else { 0.0 };
    let (y, color) = channel_y(ti, ch, th);
    let x = (time - scroll) * zoom + LANE_INSET;
    let _ = idx;
    spawn_diamond(commands, x, y, kf, color, selected)
}

/// Spawn a 45°-rotated keyframe diamond, with a white outline when selected.
fn spawn_diamond(commands: &mut Commands, x: f32, y: f32, kf: f32, color: (u8, u8, u8), selected: bool) -> Entity {
    let half = kf * 0.5;
    let mut e = commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(x - half),
            top: Val::Px(y - half),
            width: Val::Px(kf),
            height: Val::Px(kf),
            border: if selected { UiRect::all(Val::Px(1.5)) } else { UiRect::ZERO },
            ..default()
        },
        BackgroundColor(rgb(color)),
        UiTransform::from_rotation(Rot2::degrees(45.0)),
        bevy::ui::FocusPolicy::Pass,
    ));
    if selected {
        e.insert(BorderColor::all(rgb((255, 255, 255))));
    }
    e.id()
}

/// A dense run of keys drawn as one slim rounded bar in the channel color.
#[allow(clippy::too_many_arguments)]
fn key_bar(commands: &mut Commands, ti: usize, ch: u8, t0: f32, t1: f32, zoom: f32, scroll: f32, th: f32) -> Entity {
    let h = (th * 0.22).clamp(3.0, 8.0);
    let (y, color) = channel_y(ti, ch, th);
    let x0 = (t0 - scroll) * zoom + LANE_INSET;
    let w = ((t1 - t0) * zoom).max(h);
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x0 - h * 0.5),
                top: Val::Px(y - h * 0.5),
                width: Val::Px(w + h),
                height: Val::Px(h),
                border_radius: BorderRadius::all(Val::Px(h * 0.5)),
                ..default()
            },
            BackgroundColor(Color::srgba_u8(color.0, color.1, color.2, 200)),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id()
}

/// A property keyframe diamond, centered in its lane in the property color.
fn prop_diamond(commands: &mut Commands, lane: usize, time: f32, zoom: f32, scroll: f32, th: f32, selected: bool) -> Entity {
    let kf = (th * 0.40).clamp(5.0, 16.0) + if selected { 3.0 } else { 0.0 };
    let y = lane as f32 * th + th * 0.5;
    let x = (time - scroll) * zoom + LANE_INSET;
    spawn_diamond(commands, x, y, kf, PROPERTY, selected)
}

/// A dense run of property keys drawn as one slim rounded bar.
fn prop_bar(commands: &mut Commands, lane: usize, t0: f32, t1: f32, zoom: f32, scroll: f32, th: f32) -> Entity {
    let h = (th * 0.22).clamp(3.0, 8.0);
    let y = lane as f32 * th + th * 0.5;
    let x0 = (t0 - scroll) * zoom + LANE_INSET;
    let w = ((t1 - t0) * zoom).max(h);
    commands
        .spawn((
            Node { position_type: PositionType::Absolute, left: Val::Px(x0 - h * 0.5), top: Val::Px(y - h * 0.5), width: Val::Px(w + h), height: Val::Px(h), border_radius: BorderRadius::all(Val::Px(h * 0.5)), ..default() },
            BackgroundColor(Color::srgba_u8(PROPERTY.0, PROPERTY.1, PROPERTY.2, 200)),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id()
}

// ── Markers ──────────────────────────────────────────────────────────────────

/// Renders each clip marker as a labeled flag + thin full-height line. Visual
/// only (`FocusPolicy::Pass`); deletion is a math hit-test in `key_context_menu`.
pub(super) fn marker_snapshot(world: &Rx) -> KeyedSnapshot {
    let Some(clip) = cur_clip(world.untracked()) else { return empty() };
    let Some(s) = state(world.untracked()) else { return empty() };
    let (zoom, scroll) = (s.timeline_zoom, s.timeline_scroll);
    let markers: Vec<(f32, String)> = clip.markers.iter().map(|m| (m.time, m.name.clone())).collect();
    let items: Vec<(u64, u64)> = markers
        .iter()
        .enumerate()
        .map(|(i, (t, name))| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            (t.to_bits(), name, zoom.to_bits(), scroll.to_bits()).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (t, name) = &markers[i];
            marker_flag(c, f, *t, name, zoom, scroll)
        }),
    }
}

fn marker_flag(commands: &mut Commands, fonts: &EmberFonts, time: f32, name: &str, zoom: f32, scroll: f32) -> Entity {
    let x = (time - scroll) * zoom + LANE_INSET;
    let root = commands
        .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(x), top: Val::Px(0.0), width: Val::Px(0.0), height: Val::Percent(100.0), ..default() }, bevy::ui::FocusPolicy::Pass))
        .id();
    let line = commands
        .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Px(1.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(Color::srgba_u8(MARKER.0, MARKER.1, MARKER.2, 110)), bevy::ui::FocusPolicy::Pass))
        .id();
    let flag = commands
        .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(1.0), top: Val::Px(0.0), padding: UiRect::axes(Val::Px(3.0), Val::Px(0.0)), border_radius: BorderRadius::all(Val::Px(2.0)), ..default() }, BackgroundColor(rgb(MARKER)), bevy::ui::FocusPolicy::Pass))
        .id();
    let lbl = commands.spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 8.0), TextColor(rgb((25, 20, 30))), bevy::text::TextLayout::no_wrap())).id();
    commands.entity(flag).add_child(lbl);
    commands.entity(root).add_children(&[line, flag]);
    root
}

fn empty() -> KeyedSnapshot {
    KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) }
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}
