//! The Curves view: the selected track's channels drawn as value-over-time
//! curves on a grid, toggled with the toolbar's Keys / Curves switch.
//!
//! Clips store no tangents (bone channels blend linearly, property keys ease
//! by a per-key [`Interp`]), so a curve is drawn by sampling the clip the same
//! way playback does and joining the samples. It is sampled every few pixels
//! across the visible window only, and rebuilt only when the clip, the track,
//! the channel group or the view changes.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};
use bevy::ui::UiTransform;

use renzora::{Interp, TrackValue};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_text_color};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{timeline_ticks, HoverTooltip, LANE_INSET};
use renzora_ember::UiInteraction;

use crate::AnimationEditorState;

use super::build::hover_wash;
use super::clip::{cur_clip, state, Lane, SelKey, SelectedKey, SelectedKeys};
use super::layout::{RowKind, TrackLayout};
use super::popover::{apply_interp_to_selection, segment};
use super::snapshots::{channel_color, lane_key_times, title_case, x_of};
use super::{AnimBtn, KeyLane, PROPERTY, SELECTED};

/// Keys or Curves, and which bone channel group the Curves view shows.
#[derive(Resource)]
pub(crate) struct TimelineMode {
    pub(crate) curves: bool,
    /// 0 = Position, 1 = Rotation, 2 = Scale.
    pub(crate) group: u8,
}

impl Default for TimelineMode {
    fn default() -> Self {
        Self { curves: false, group: 1 }
    }
}

#[derive(Component)]
pub(super) struct ModeBtn(bool);
#[derive(Component)]
pub(super) struct GroupBtn(u8);
#[derive(Component)]
pub(super) struct InterpBtn(Interp);
#[derive(Component)]
pub(super) struct HoverBox;
#[derive(Component)]
pub(super) struct HoverText;

/// Pixels between curve samples.
const SAMPLE_PX: f32 = 5.0;
/// Space above and below the curves inside the lane area.
const PAD_Y: f32 = 18.0;

// ── Toolbar pieces ───────────────────────────────────────────────────────────

/// The Keys / Curves segmented switch.
pub(super) fn build_mode_switch(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let wrap = commands
        .spawn((Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(2.0), padding: UiRect::all(Val::Px(2.0)), border_radius: BorderRadius::all(Val::Px(7.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(rgb(window_bg()))))
        .id();
    for (curves, key, fallback) in [(false, "timeline.keys", "Keys"), (true, "timeline.curves", "Curves")] {
        let b = seg_button(commands, fonts, &renzora::lang::t_or(key, fallback), ModeBtn(curves));
        bind_bg(commands, b, move |w| {
            let on = w.get_resource::<TimelineMode>().is_some_and(|m| m.curves == curves);
            if on {
                rgb(accent()).with_alpha(0.26)
            } else {
                match w.get::<UiInteraction>(b) {
                    Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(hover_bg()),
                    _ => Color::NONE,
                }
            }
        });
        commands.entity(wrap).add_child(b);
    }
    wrap
}

/// The Curves view's tools: the bone channel group, the interpolation of the
/// selected keys, and Frame all.
pub(super) fn build_interp_buttons(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let wrap = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), margin: UiRect::right(Val::Px(6.0)), flex_shrink: 0.0, ..default() }).id();

    let groups = seg_group(commands);
    for (g, key, fallback) in [(0u8, "timeline.position", "Position"), (1, "timeline.rotation", "Rotation"), (2, "timeline.scale", "Scale")] {
        let b = seg_button(commands, fonts, &renzora::lang::t_or(key, fallback), GroupBtn(g));
        bind_bg(commands, b, move |w| {
            let on = w.get_resource::<TimelineMode>().is_some_and(|m| m.group == g);
            if on {
                rgb(accent()).with_alpha(0.26)
            } else {
                Color::NONE
            }
        });
        commands.entity(groups).add_child(b);
    }
    bind_display(commands, groups, |w| matches!(w.get_resource::<TrackLayout>().and_then(|l| l.selected), Some(RowKind::Bone(_))));

    let label = commands.spawn((Text::new(renzora::lang::t_or("timeline.interpolation", "Interpolation")), ui_font(&fonts.ui, 12.0), TextColor(rgb((154, 154, 166))), bevy::text::TextLayout::no_wrap())).id();
    let interps = seg_group(commands);
    use bevy::curve::EaseFunction as E;
    for (interp, key, fallback) in [
        (Interp::Eased(E::SmoothStep), "timeline.ease_smooth", "Smooth"),
        (Interp::Linear, "timeline.ease_linear", "Linear"),
        (Interp::Stepped, "timeline.ease_step", "Step"),
    ] {
        let b = seg_button(commands, fonts, &renzora::lang::t_or(key, fallback), InterpBtn(interp));
        commands.entity(b).insert(HoverTooltip::new(renzora::lang::t_or("timeline.interp_tip", "Apply to the selected property keys")));
        bind_bg(commands, b, move |w| {
            let world = w.untracked();
            let current = world.get_resource::<SelectedKey>().and_then(|s| s.0).and_then(|k| match k.lane {
                Lane::Prop { track } => cur_clip(world).and_then(|c| c.property_tracks.get(track)).and_then(|p| p.keys.get(k.index)).map(|key| key.interp),
                _ => None,
            });
            if current == Some(interp) {
                rgb(accent()).with_alpha(0.26)
            } else {
                Color::NONE
            }
        });
        commands.entity(interps).add_child(b);
    }
    for e in [label, interps] {
        bind_display(commands, e, |w| matches!(w.get_resource::<TrackLayout>().and_then(|l| l.selected), Some(RowKind::Prop(_))));
    }

    let fit = commands
        .spawn((Node { height: Val::Px(26.0), padding: UiRect::horizontal(Val::Px(10.0)), align_items: AlignItems::Center, border_radius: BorderRadius::all(Val::Px(6.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(Color::NONE), UiInteraction::default(), AnimBtn::FrameAll))
        .id();
    hover_wash(commands, fit);
    let fit_t = commands.spawn((Text::new(renzora::lang::t_or("timeline.frame_all", "Frame all")), ui_font(&fonts.ui, 12.0), TextColor(rgb((196, 196, 206))), bevy::text::TextLayout::no_wrap(), PASS_THROUGH)).id();
    commands.entity(fit).add_child(fit_t);
    commands.entity(wrap).add_children(&[groups, label, interps, fit]);
    wrap
}

fn seg_group(commands: &mut Commands) -> Entity {
    commands
        .spawn((Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(2.0), padding: UiRect::all(Val::Px(2.0)), border_radius: BorderRadius::all(Val::Px(7.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(rgb(window_bg()))))
        .id()
}

fn seg_button<M: Component>(commands: &mut Commands, fonts: &EmberFonts, label: &str, marker: M) -> Entity {
    let b = commands
        .spawn((Node { height: Val::Px(24.0), padding: UiRect::horizontal(Val::Px(10.0)), align_items: AlignItems::Center, border_radius: BorderRadius::all(Val::Px(5.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(Color::NONE), UiInteraction::default(), marker))
        .id();
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 12.0), TextColor(rgb((154, 154, 166))), bevy::text::TextLayout::no_wrap(), PASS_THROUGH)).id();
    bind_text_color(commands, t, move |w| match w.get::<UiInteraction>(b) {
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(text_primary()),
        _ => rgb((196, 196, 206)),
    });
    commands.entity(b).add_child(t);
    b
}

pub(super) fn mode_click(
    modes: Query<(&UiInteraction, &ModeBtn), Changed<UiInteraction>>,
    groups: Query<(&UiInteraction, &GroupBtn), Changed<UiInteraction>>,
    mut mode: ResMut<TimelineMode>,
    mut layout: ResMut<TrackLayout>,
) {
    for (i, m) in &modes {
        if *i == UiInteraction::Pressed && mode.curves != m.0 {
            mode.curves = m.0;
            // The Curves view needs a subject: pick the first real track.
            if m.0 && !matches!(layout.selected, Some(RowKind::Bone(_) | RowKind::Prop(_))) {
                layout.selected = layout.rows.iter().map(|r| r.kind).find(|k| matches!(k, RowKind::Bone(_) | RowKind::Prop(_)));
            }
        }
    }
    for (i, g) in &groups {
        if *i == UiInteraction::Pressed && mode.group != g.0 {
            mode.group = g.0;
        }
    }
}

pub(super) fn interp_click(q: Query<(&UiInteraction, &InterpBtn), Changed<UiInteraction>>, mut commands: Commands) {
    for (i, b) in &q {
        if *i == UiInteraction::Pressed {
            let interp = b.0;
            commands.queue(move |w: &mut World| apply_interp_to_selection(w, interp));
        }
    }
}

// ── Geometry ─────────────────────────────────────────────────────────────────

/// One drawn channel: its label, colour, keys `(time, value)` and the lane
/// they belong to.
pub(super) struct Channel {
    pub(super) label: &'static str,
    pub(super) color: (u8, u8, u8),
    pub(super) keys: Vec<(f32, f32)>,
    pub(super) lane: Lane,
    /// Which component of the lane's value this channel is.
    pub(super) comp: usize,
}

/// The selected track's channels (and how to sample them), with the value
/// range they span.
pub(super) struct CurveSet {
    pub(super) title: String,
    pub(super) unit: &'static str,
    pub(super) channels: Vec<Channel>,
    pub(super) vmin: f32,
    pub(super) vmax: f32,
}

fn euler_deg(q: [f32; 4]) -> [f32; 3] {
    let (y, x, z) = Quat::from_array(q).to_euler(EulerRot::YXZ);
    [x.to_degrees(), y.to_degrees(), z.to_degrees()]
}

/// Value components of a property value.
fn comps(v: &TrackValue) -> Vec<f32> {
    match v {
        TrackValue::Float(x) => vec![*x],
        TrackValue::Vec3(a) => a.to_vec(),
        TrackValue::Quat(q) => euler_deg(*q).to_vec(),
        TrackValue::Color(c) => c.to_vec(),
        TrackValue::Bool(b) => vec![if *b { 1.0 } else { 0.0 }],
    }
}

pub(super) fn curve_set(clip: &renzora_animation::AnimClip, row: RowKind, group: u8) -> Option<CurveSet> {
    let xyz = [("X", channel_color(0)), ("Y", channel_color(1)), ("Z", channel_color(2))];
    let (title, unit, channels): (String, &'static str, Vec<Channel>) = match row {
        // An effect's rows are keyed in the dopesheet only.
        RowKind::Summary | RowKind::FxGroup | RowKind::FxEmitter(..) | RowKind::FxTrack(_) => return None,
        RowKind::Bone(ti) => {
            let t = clip.tracks.get(ti)?;
            let lane = Lane::Bone { track: ti, channel: group };
            let vals: Vec<(f32, [f32; 3])> = match group {
                0 => t.translations.clone(),
                1 => t.rotations.iter().map(|(tm, q)| (*tm, euler_deg(*q))).collect(),
                _ => t.scales.clone(),
            };
            let name = match group {
                0 => renzora::lang::t_or("timeline.position", "Position"),
                1 => renzora::lang::t_or("timeline.rotation", "Rotation"),
                _ => renzora::lang::t_or("timeline.scale", "Scale"),
            };
            let chans = (0..3)
                .map(|c| Channel { label: xyz[c].0, color: xyz[c].1, keys: vals.iter().map(|(tm, v)| (*tm, v[c])).collect(), lane, comp: c })
                .collect();
            (format!("{} · {}", t.bone_name, name), if group == 1 { "°" } else { "" }, chans)
        }
        RowKind::Prop(pi) => {
            let p = clip.property_tracks.get(pi)?;
            let lane = Lane::Prop { track: pi };
            let n = p.keys.first().map(|k| comps(&k.value).len()).unwrap_or(1);
            let labels: Vec<(&'static str, (u8, u8, u8))> = match n {
                1 => vec![("", PROPERTY)],
                3 => xyz.to_vec(),
                _ => vec![("R", (248, 113, 113)), ("G", (110, 231, 160)), ("B", (96, 165, 250)), ("A", (200, 200, 210))],
            };
            let unit = if p.keys.first().is_some_and(|k| matches!(k.value, TrackValue::Quat(_))) || p.field.contains("rotation") { "°" } else { "" };
            let chans = labels
                .iter()
                .enumerate()
                .map(|(c, (label, color))| Channel {
                    label,
                    color: *color,
                    keys: p.keys.iter().map(|k| (k.time, comps(&k.value).get(c).copied().unwrap_or(0.0))).collect(),
                    lane,
                    comp: c,
                })
                .collect();
            (format!("{} · {}", title_case(&p.component), title_case(&p.field)), unit, chans)
        }
    };
    let mut vmin = f32::INFINITY;
    let mut vmax = f32::NEG_INFINITY;
    for c in &channels {
        for (_, v) in &c.keys {
            vmin = vmin.min(*v);
            vmax = vmax.max(*v);
        }
    }
    if !vmin.is_finite() {
        (vmin, vmax) = (-1.0, 1.0);
    }
    if (vmax - vmin).abs() < 1e-4 {
        vmin -= 1.0;
        vmax += 1.0;
    }
    let pad = (vmax - vmin) * 0.08;
    Some(CurveSet { title, unit, channels, vmin: vmin - pad, vmax: vmax + pad })
}

/// A channel's value at `t`: what playback would show, for the drawing.
fn sample(clip: &renzora_animation::AnimClip, ch: &Channel, group: u8, t: f32) -> Option<f32> {
    match ch.lane {
        Lane::Prop { track } => {
            let p = clip.property_tracks.get(track)?;
            renzora::sample_property_track(p, t).map(|v| comps(&v).get(ch.comp).copied().unwrap_or(0.0))
        }
        Lane::Bone { track, .. } => {
            let tr = clip.tracks.get(track)?;
            if group == 1 {
                let keys = &tr.rotations;
                let i = keys.partition_point(|k| k.0 <= t);
                let q = match (i.checked_sub(1).and_then(|j| keys.get(j)), keys.get(i)) {
                    (Some(a), Some(b)) => {
                        let f = ((t - a.0) / (b.0 - a.0).max(1e-6)).clamp(0.0, 1.0);
                        Quat::from_array(a.1).slerp(Quat::from_array(b.1), f).to_array()
                    }
                    (Some(a), None) => a.1,
                    (None, Some(b)) => b.1,
                    _ => return None,
                };
                Some(euler_deg(q)[ch.comp])
            } else {
                let keys = &ch.keys;
                let i = keys.partition_point(|k| k.0 <= t);
                match (i.checked_sub(1).and_then(|j| keys.get(j)), keys.get(i)) {
                    (Some(a), Some(b)) => {
                        let f = ((t - a.0) / (b.0 - a.0).max(1e-6)).clamp(0.0, 1.0);
                        Some(a.1 + (b.1 - a.1) * f)
                    }
                    (Some(a), None) => Some(a.1),
                    (None, Some(b)) => Some(b.1),
                    _ => None,
                }
            }
        }
        Lane::Fx { .. } => None,
    }
}

fn y_of(v: f32, set: &CurveSet, h: f32) -> f32 {
    let f = (set.vmax - v) / (set.vmax - set.vmin).max(1e-6);
    PAD_Y + f * (h - PAD_Y * 2.0).max(1.0)
}

// ── Drawing ──────────────────────────────────────────────────────────────────

pub(super) fn curves_token(w: &Rx) -> u64 {
    let world = w.untracked();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    if let Some(m) = world.get_resource::<TimelineMode>() {
        (m.curves, m.group).hash(&mut h);
        if !m.curves {
            return h.finish();
        }
    }
    if let Some(s) = state(world) {
        (s.timeline_zoom.to_bits(), s.timeline_scroll.to_bits()).hash(&mut h);
    }
    if let Some(l) = world.get_resource::<TrackLayout>() {
        (l.selected, l.view.x.to_bits(), l.view.y.to_bits()).hash(&mut h);
    }
    if let Some(r) = world.get_resource::<super::runs::KeyRuns>() {
        r.gen.hash(&mut h);
    }
    if let Some(s) = world.get_resource::<SelectedKeys>() {
        s.gen.hash(&mut h);
    }
    world.get_resource::<SelectedKey>().and_then(|s| s.0).hash(&mut h);
    // Value edits move curves without moving key times.
    if let (Some(clip), Some(l)) = (cur_clip(world), world.get_resource::<TrackLayout>()) {
        if let Some(set) = l.selected.and_then(|r| curve_set(clip, r, world.get_resource::<TimelineMode>().map(|m| m.group).unwrap_or(1))) {
            for c in &set.channels {
                for (t, v) in &c.keys {
                    (t.to_bits(), v.to_bits()).hash(&mut h);
                }
            }
        }
    }
    h.finish()
}

enum CurveElem {
    Text(f32, f32, String, (u8, u8, u8), f32),
    Line(Vec2, Vec2, Color, f32),
    Key(Vec2, (u8, u8, u8), bool),
}

pub(super) fn curves_snapshot(world: &Rx) -> KeyedSnapshot {
    let w = world.untracked();
    let (Some(clip), Some(s), Some(layout), Some(mode)) =
        (cur_clip(w), state(w), w.get_resource::<TrackLayout>(), w.get_resource::<TimelineMode>())
    else {
        return empty();
    };
    if !mode.curves {
        return empty();
    }
    let (zoom, scroll) = (s.timeline_zoom.max(1.0), s.timeline_scroll);
    let (width, height) = (layout.view.x.max(100.0), layout.view.y.max(60.0));
    let mut elems: Vec<CurveElem> = Vec::new();

    let set = layout.selected.and_then(|r| curve_set(clip, r, mode.group));
    let Some(set) = set else {
        elems.push(CurveElem::Text(width * 0.5 - 120.0, height * 0.5 - 8.0, renzora::lang::t_or("timeline.curves_pick_track", "Select a track on the left to see its curves"), (138, 138, 150), 12.0));
        return build_snapshot(elems);
    };

    // Value grid with labels.
    for i in 0..5 {
        let v = set.vmax - (set.vmax - set.vmin) * (i as f32 / 4.0);
        let y = y_of(v, &set, height);
        elems.push(CurveElem::Line(Vec2::new(0.0, y), Vec2::new(width, y), rgb(text_primary()).with_alpha(0.05), 1.0));
        elems.push(CurveElem::Text(6.0, y - 13.0, format!("{:.1}{}", v, set.unit), (106, 106, 118), 10.0));
    }
    // Time grid aligned with the ruler.
    let (ticks, _) = timeline_ticks(zoom, scroll);
    for (t, major) in ticks {
        let x = x_of(t, zoom, scroll);
        if x < 0.0 || x > width {
            continue;
        }
        let a = if major { 0.07 } else { 0.03 };
        elems.push(CurveElem::Line(Vec2::new(x, 0.0), Vec2::new(x, height), rgb(text_primary()).with_alpha(a), 1.0));
    }
    elems.push(CurveElem::Text(width - 10.0 - set.title.len() as f32 * 6.5, 6.0, set.title.clone(), (154, 154, 166), 11.0));

    // Curves, sampled across the visible part of the clip.
    let x0 = x_of(0.0, zoom, scroll).max(0.0);
    let x1 = x_of(clip.duration, zoom, scroll).min(width);
    if x1 > x0 {
        for ch in &set.channels {
            let mut prev: Option<Vec2> = None;
            let mut x = x0;
            while x <= x1 + 0.01 {
                let t = (x - LANE_INSET) / zoom + scroll;
                if let Some(v) = sample(clip, ch, mode.group, t) {
                    let p = Vec2::new(x, y_of(v, &set, height));
                    if let Some(q) = prev {
                        elems.push(CurveElem::Line(q, p, rgb(ch.color).with_alpha(0.9), 2.0));
                    }
                    prev = Some(p);
                }
                x += SAMPLE_PX;
            }
        }
    }

    // Keys, where they are far enough apart to be told apart.
    let primary = w.get_resource::<SelectedKey>().and_then(|s| s.0);
    let sel = w.get_resource::<SelectedKeys>();
    for ch in &set.channels {
        let times = lane_key_times(clip, ch.lane);
        let dense = times.len() > 1 && (times[times.len() - 1] - times[0]) / (times.len() - 1) as f32 * zoom < 8.0;
        for (i, (t, v)) in ch.keys.iter().enumerate() {
            let x = x_of(*t, zoom, scroll);
            if x < -8.0 || x > width + 8.0 {
                continue;
            }
            let k = SelKey { lane: ch.lane, index: i };
            let selected = primary == Some(k) || sel.is_some_and(|s| s.contains(&k));
            if dense && !selected {
                continue;
            }
            elems.push(CurveElem::Key(Vec2::new(x, y_of(*v, &set, height)), if selected { SELECTED } else { ch.color }, selected));
        }
    }
    build_snapshot(elems)
}

fn build_snapshot(elems: Vec<CurveElem>) -> KeyedSnapshot {
    let items: Vec<(u64, u64)> = elems
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            i.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            match e {
                CurveElem::Text(x, y, s, c, sz) => (0u8, x.to_bits(), y.to_bits(), s, c, sz.to_bits()).hash(&mut h),
                CurveElem::Line(a, b, c, t) => (1u8, a.x.to_bits(), a.y.to_bits(), b.x.to_bits(), b.y.to_bits(), c.to_srgba().to_u8_array(), t.to_bits()).hash(&mut h),
                CurveElem::Key(p, c, s) => (2u8, p.x.to_bits(), p.y.to_bits(), c, s).hash(&mut h),
            }
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| match &elems[i] {
            CurveElem::Text(x, y, s, col, sz) => c
                .spawn((Text::new(s.clone()), ui_font(&f.ui, *sz), TextColor(rgb(*col)), bevy::text::TextLayout::no_wrap(), Node { position_type: PositionType::Absolute, left: Val::Px(*x), top: Val::Px(*y), ..default() }, PASS_THROUGH))
                .id(),
            CurveElem::Line(a, b, col, t) => segment(c, *a, *b, *t, *col),
            CurveElem::Key(p, col, s) => {
                let k = if *s { 10.0 } else { 8.0 };
                let mut e = c.spawn((
                    Node { position_type: PositionType::Absolute, left: Val::Px(p.x - k * 0.5), top: Val::Px(p.y - k * 0.5), width: Val::Px(k), height: Val::Px(k), border_radius: BorderRadius::all(Val::Px(1.5)), ..default() },
                    BackgroundColor(rgb(*col)),
                    UiTransform::from_rotation(Rot2::degrees(45.0)),
                    PASS_THROUGH,
                ));
                if *s {
                    e.insert(BoxShadow::new(rgb(SELECTED), Val::Px(0.0), Val::Px(0.0), Val::Px(2.5), Val::Px(0.0)));
                } else {
                    e.insert(BoxShadow::new(rgb(panel_bg()), Val::Px(0.0), Val::Px(0.0), Val::Px(1.5), Val::Px(0.0)));
                }
                e.id()
            }
        }),
    }
}

fn empty() -> KeyedSnapshot {
    KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) }
}

// ── Hover readout and picking ────────────────────────────────────────────────

pub(super) fn build_hover_readout(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let b = commands
        .spawn((
            Node { position_type: PositionType::Absolute, display: Display::None, padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() },
            BackgroundColor(rgb((32, 32, 40))),
            BorderColor::all(rgb((48, 48, 58))),
            PASS_THROUGH,
            GlobalZIndex(5),
            HoverBox,
        ))
        .id();
    let t = commands.spawn((Text::new(""), ui_font(&fonts.mono, 11.0), TextColor(rgb(text_primary())), bevy::text::TextLayout::no_wrap(), PASS_THROUGH, HoverText)).id();
    commands.entity(b).add_child(t);
    b
}

/// The channel and value nearest the cursor, as "1.68s · X -18.0°" beside it.
#[allow(clippy::too_many_arguments)]
pub(super) fn curves_hover(
    mode: Res<TimelineMode>,
    lane: Query<(&RelativeCursorPosition, &ComputedNode), With<KeyLane>>,
    state: Option<Res<AnimationEditorState>>,
    layout: Res<TrackLayout>,
    cache: Res<super::clip::TimelineClip>,
    mut boxes: Query<&mut Node, With<HoverBox>>,
    mut texts: Query<&mut Text, With<HoverText>>,
) {
    let hide = |boxes: &mut Query<&mut Node, With<HoverBox>>| {
        for mut n in boxes.iter_mut() {
            if n.display != Display::None {
                n.display = Display::None;
            }
        }
    };
    let (Some(state), Some(clip)) = (state, cache.clip.as_ref()) else { return hide(&mut boxes) };
    if !mode.curves {
        return hide(&mut boxes);
    }
    let Some(p) = super::edit::lane_cursor(&lane) else { return hide(&mut boxes) };
    let Some(set) = layout.selected.and_then(|r| curve_set(clip, r, mode.group)) else { return hide(&mut boxes) };
    let height = layout.view.y.max(60.0);
    let zoom = state.timeline_zoom.max(1.0);
    let t = ((p.x - LANE_INSET) / zoom + state.timeline_scroll).clamp(0.0, clip.duration);
    let best = set
        .channels
        .iter()
        .filter_map(|c| sample(clip, c, mode.group, t).map(|v| (c, v)))
        .min_by(|a, b| (y_of(a.1, &set, height) - p.y).abs().total_cmp(&(y_of(b.1, &set, height) - p.y).abs()));
    let Some((ch, v)) = best else { return hide(&mut boxes) };
    let label = if ch.label.is_empty() { String::new() } else { format!("{} ", ch.label) };
    let s = format!("{:.2}s · {}{:.1}{}", t, label, v, set.unit);
    for mut text in &mut texts {
        if text.0 != s {
            text.0 = s.clone();
        }
    }
    let (left, top) = (Val::Px(p.x + 14.0), Val::Px((p.y - 30.0).max(2.0)));
    for mut n in &mut boxes {
        if n.display != Display::Flex {
            n.display = Display::Flex;
        }
        if n.left != left {
            n.left = left;
        }
        if n.top != top {
            n.top = top;
        }
    }
}

/// The curve key nearest a lane-space point, within a few pixels.
pub(super) fn pick_curve_key(
    clip: &renzora_animation::AnimClip,
    layout: &TrackLayout,
    mode: &TimelineMode,
    zoom: f32,
    scroll: f32,
    p: Vec2,
) -> Option<(SelKey, f32)> {
    let set = layout.selected.and_then(|r| curve_set(clip, r, mode.group))?;
    let height = layout.view.y.max(60.0);
    let mut best: Option<(f32, SelKey, f32)> = None;
    for ch in &set.channels {
        for (i, (t, v)) in ch.keys.iter().enumerate() {
            let q = Vec2::new(x_of(*t, zoom, scroll), y_of(*v, &set, height));
            let d = q.distance(p);
            if d <= 7.0 && best.as_ref().is_none_or(|b| d < b.0) {
                best = Some((d, SelKey { lane: ch.lane, index: i }, *t));
            }
        }
    }
    best.map(|(_, k, t)| (k, t))
}
