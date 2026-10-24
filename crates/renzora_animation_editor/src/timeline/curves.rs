//! The Curves view: the selected track's channels drawn as value-over-time
//! curves on a grid, toggled with the toolbar's Keys / Curves switch. This
//! module draws it and holds its geometry; [`super::graph`] is what the
//! pointer does to it (select, drag keys and tangent handles, pan, zoom).
//!
//! A curve is drawn by sampling the clip the same way playback does
//! ([`renzora::ChannelSpline`] for a bone channel, the property sampler for a
//! property track) and joining the samples, so the drawing is exactly what
//! plays. It is sampled every few pixels across the visible window only, and
//! rebuilt only when the clip, the track, the selection or the view changes.
//!
//! A bone's rotation is a quaternion, which has no curves an animator can
//! read, so it is shown as its Euler angles (`YXZ`, like the inspector),
//! unwrapped from key to key so a turn through 180 degrees reads as one
//! smooth line instead of a jump to -180. Edits made on those curves are
//! converted back into the quaternion keys and tangents.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};

use renzora::{euler_slope_of_quat_tangent, quat_to_euler_deg, wrap_deg_near, ChannelSpline, Interp, KeyMode, TrackValue};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_text_color};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{timeline_ticks, HoverTooltip, LANE_INSET};
use renzora_ember::UiInteraction;

use crate::AnimationEditorState;

use super::build::hover_wash;
use super::clip::{cur_clip, state, Lane, SelKey, SelectedKey};
use super::layout::{RowKind, TrackLayout};
use super::popover::{apply_interp_to_selection, segment};
use super::snapshots::{bone_key_shape, channel_color, prop_key_shape, spawn_key, title_case, x_of, KeyShape};
use super::{AnimBtn, KeyLane, MORPH, PROPERTY, SELECTED};

/// Keys or Curves, which bone channel group the Curves view shows, and which
/// of its channels are hidden.
#[derive(Resource)]
pub(crate) struct TimelineMode {
    pub(crate) curves: bool,
    /// 0 = Position, 1 = Rotation, 2 = Scale.
    pub(crate) group: u8,
    /// Channels switched off in the Curves view, by component (X Y Z, or
    /// R G B A): not drawn, not pickable, and left out of framing.
    pub(crate) hidden: [bool; 4],
}

impl Default for TimelineMode {
    fn default() -> Self {
        Self { curves: false, group: 1, hidden: [false; 4] }
    }
}

/// The Curves view's value window. `None` frames every key of the subject,
/// which is what a fresh subject gets; panning, zooming and the frame
/// commands set it. Held still while anything is dragged, which is the reason
/// it exists: a window refitted to the keys every frame would slide under
/// the cursor of whoever is dragging one.
#[derive(Resource, Default)]
pub(crate) struct CurveView {
    pub(super) range: Option<(f32, f32)>,
    /// The `(row, group)` the range was set for. A different subject ignores
    /// it until [`super::graph::graph_sync`] reframes.
    pub(super) subject: Option<(RowKind, u8)>,
}

/// One channel component of one key, as the Curves view selects it: the
/// dopesheet's [`SelKey`] names the key, `comp` which of its curves.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct CurvePick {
    pub(super) key: SelKey,
    pub(super) comp: usize,
}

/// The Curves view's selection. Kept in step with [`super::clip::SelectedKeys`]
/// (see [`super::graph::graph_sync`]) so Delete, Copy and the key card act on
/// the same keys whichever view picked them.
#[derive(Resource, Default)]
pub(crate) struct CurveSel {
    pub(super) picks: Vec<CurvePick>,
    pub(super) gen: u64,
}

impl CurveSel {
    pub(super) fn set(&mut self, picks: Vec<CurvePick>) {
        if self.picks != picks {
            self.picks = picks;
            self.gen = self.gen.wrapping_add(1);
        }
    }
    pub(super) fn contains(&self, p: &CurvePick) -> bool {
        self.picks.contains(p)
    }
}

#[derive(Component)]
pub(super) struct ModeBtn(bool);
#[derive(Component)]
pub(super) struct GroupBtn(u8);
#[derive(Component)]
pub(super) struct InterpBtn(Interp);
/// A bone key mode in the Curves toolbar: applies to the selected keys.
#[derive(Component)]
pub(super) struct KeyModeBtn(pub(super) KeyMode);
/// A channel's visibility chip in the Curves toolbar (component index).
#[derive(Component)]
pub(super) struct ChannelBtn(pub(super) usize);
/// "Frame selected" in the Curves toolbar.
#[derive(Component)]
pub(super) struct FrameSelBtn;
#[derive(Component)]
pub(super) struct HoverBox;
#[derive(Component)]
pub(super) struct HoverText;

/// Pixels between curve samples.
const SAMPLE_PX: f32 = 4.0;
/// Space above and below the curves inside the lane area.
pub(super) const PAD_Y: f32 = 18.0;
/// Length of a drawn tangent handle, px.
pub(super) const HANDLE_PX: f32 = 46.0;

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

fn bone_selected(w: &Rx) -> bool {
    matches!(w.get_resource::<TrackLayout>().and_then(|l| l.selected), Some(RowKind::Bone(_)))
}

/// A track whose keys take key modes: a bone, or a blend shape.
fn moded_selected(w: &Rx) -> bool {
    matches!(w.get_resource::<TrackLayout>().and_then(|l| l.selected), Some(RowKind::Bone(_) | RowKind::Morph(_)))
}

fn prop_selected(w: &Rx) -> bool {
    matches!(w.get_resource::<TrackLayout>().and_then(|l| l.selected), Some(RowKind::Prop(_)))
}

/// The Curves view's tools: the bone channel group, which channels show, the
/// interpolation of the selected keys (key modes for a bone, easings for a
/// property), and Frame selected / Frame all.
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
    bind_display(commands, groups, bone_selected);

    // Channel visibility: X Y Z for a bone, as many as the property has.
    let chans = seg_group(commands);
    for (c, label) in ["X", "Y", "Z", "W"].into_iter().enumerate() {
        let color = if c < 3 { channel_color(c as u8) } else { (200, 200, 210) };
        let b = seg_button(commands, fonts, label, ChannelBtn(c));
        commands.entity(b).insert(HoverTooltip::new(renzora::lang::t_or("timeline.channel_tip", "Show or hide this channel's curve")));
        bind_bg(commands, b, move |w| {
            let on = w.get_resource::<TimelineMode>().is_some_and(|m| !m.hidden[c]);
            if on {
                rgb(color).with_alpha(0.22)
            } else {
                Color::NONE
            }
        });
        bind_display(commands, b, move |w| {
            let world = w.untracked();
            let n = match world.get_resource::<TrackLayout>().and_then(|l| l.selected) {
                Some(RowKind::Bone(_)) => 3,
                Some(RowKind::Prop(p)) => cur_clip(world)
                    .and_then(|clip| clip.property_tracks.get(p))
                    .and_then(|t| t.keys.first())
                    .map(|k| comps(&k.value).len())
                    .unwrap_or(1),
                _ => 0,
            };
            c < n && n > 1
        });
        commands.entity(chans).add_child(b);
    }

    // A bone key's mode.
    let mode_label = commands.spawn((Text::new(renzora::lang::t_or("timeline.key_mode", "Key")), ui_font(&fonts.ui, 12.0), TextColor(rgb((154, 154, 166))), bevy::text::TextLayout::no_wrap())).id();
    let modes = seg_group(commands);
    for (mode, _, tip) in super::keymode::mode_choices() {
        let b = seg_button(commands, fonts, &super::keymode::mode_label_tr(mode, mode.label()), KeyModeBtn(mode));
        commands.entity(b).insert(HoverTooltip::new(super::keymode::mode_label_tr(mode, tip)));
        bind_bg(commands, b, move |w| {
            let world = w.untracked();
            let current = world.get_resource::<SelectedKey>().and_then(|s| s.0).and_then(|k| cur_clip(world).and_then(|c| super::keymode::key_mode(c, k)));
            if current == Some(mode) {
                rgb(accent()).with_alpha(0.26)
            } else {
                Color::NONE
            }
        });
        commands.entity(modes).add_child(b);
    }
    for e in [mode_label, modes] {
        bind_display(commands, e, moded_selected);
    }

    // A property key's easing.
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
        bind_display(commands, e, prop_selected);
    }

    let fit_sel = text_btn(commands, fonts, &renzora::lang::t_or("timeline.frame_selected", "Frame selected"), FrameSelBtn);
    commands.entity(fit_sel).insert(HoverTooltip::new(renzora::lang::t_or("timeline.frame_selected_tip", "Fit the view to the selected keys (F)")));
    let fit = text_btn(commands, fonts, &renzora::lang::t_or("timeline.frame_all", "Frame all"), AnimBtn::FrameAll);
    commands.entity(wrap).add_children(&[groups, chans, mode_label, modes, label, interps, fit_sel, fit]);
    wrap
}

fn text_btn<M: Component>(commands: &mut Commands, fonts: &EmberFonts, label: &str, marker: M) -> Entity {
    let b = commands
        .spawn((Node { height: Val::Px(26.0), padding: UiRect::horizontal(Val::Px(10.0)), align_items: AlignItems::Center, border_radius: BorderRadius::all(Val::Px(6.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(Color::NONE), UiInteraction::default(), marker))
        .id();
    hover_wash(commands, b);
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 12.0), TextColor(rgb((196, 196, 206))), bevy::text::TextLayout::no_wrap(), PASS_THROUGH)).id();
    commands.entity(b).add_child(t);
    b
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

#[allow(clippy::too_many_arguments)]
pub(super) fn mode_click(
    modes: Query<(&UiInteraction, &ModeBtn), Changed<UiInteraction>>,
    groups: Query<(&UiInteraction, &GroupBtn), Changed<UiInteraction>>,
    chans: Query<(&UiInteraction, &ChannelBtn), Changed<UiInteraction>>,
    mut mode: ResMut<TimelineMode>,
    mut layout: ResMut<TrackLayout>,
) {
    for (i, m) in &modes {
        if *i == UiInteraction::Pressed && mode.curves != m.0 {
            mode.curves = m.0;
            // The Curves view needs a subject: pick the first real track.
            if m.0 && !matches!(layout.selected, Some(RowKind::Bone(_) | RowKind::Prop(_) | RowKind::Morph(_))) {
                layout.selected = layout.rows.iter().map(|r| r.kind).find(|k| matches!(k, RowKind::Bone(_) | RowKind::Prop(_) | RowKind::Morph(_)));
            }
        }
    }
    for (i, g) in &groups {
        if *i == UiInteraction::Pressed && mode.group != g.0 {
            mode.group = g.0;
        }
    }
    for (i, c) in &chans {
        if *i == UiInteraction::Pressed {
            let c = c.0.min(3);
            mode.hidden[c] = !mode.hidden[c];
            // Never all of them: an empty graph reads as a broken one.
            if mode.hidden.iter().all(|h| *h) {
                mode.hidden[c] = false;
            }
        }
    }
}

pub(super) fn interp_click(
    q: Query<(&UiInteraction, &InterpBtn), Changed<UiInteraction>>,
    modes: Query<(&UiInteraction, &KeyModeBtn), Changed<UiInteraction>>,
    mut commands: Commands,
) {
    for (i, b) in &q {
        if *i == UiInteraction::Pressed {
            let interp = b.0;
            commands.queue(move |w: &mut World| apply_interp_to_selection(w, interp));
        }
    }
    for (i, b) in &modes {
        if *i == UiInteraction::Pressed {
            let mode = b.0;
            commands.queue(move |w: &mut World| {
                let keys = super::keymode::selection(w);
                super::keymode::apply_mode(w, &keys, mode);
            });
        }
    }
}

// ── Geometry ─────────────────────────────────────────────────────────────────

/// One drawn channel: its label, colour, keys `(time, shown value)` and the
/// lane they belong to.
pub(super) struct Channel {
    pub(super) label: &'static str,
    pub(super) color: (u8, u8, u8),
    pub(super) keys: Vec<(f32, f32)>,
    pub(super) lane: Lane,
    /// Which component of the lane's value this channel is.
    pub(super) comp: usize,
}

/// A bone channel group's evaluator, shared by its three drawn channels.
pub(super) struct BoneCurves {
    pub(super) spline: ChannelSpline,
    /// The keys as stored (a rotation's quaternion in its stored sign).
    pub(super) raw: Vec<Vec4>,
    pub(super) modes: Vec<KeyMode>,
    pub(super) rotation: bool,
}

/// The selected track's channels (and how to sample them), with the value
/// range their keys span.
pub(super) struct CurveSet {
    pub(super) title: String,
    pub(super) unit: &'static str,
    pub(super) channels: Vec<Channel>,
    pub(super) vmin: f32,
    pub(super) vmax: f32,
    pub(super) bone: Option<BoneCurves>,
    pub(super) subject: (RowKind, u8),
}

fn euler_deg(q: [f32; 4]) -> [f32; 3] {
    quat_to_euler_deg(Quat::from_array(q)).to_array()
}

/// Value components of a property value.
pub(super) fn comps(v: &TrackValue) -> Vec<f32> {
    match v {
        TrackValue::Float(x) => vec![*x],
        TrackValue::Vec3(a) => a.to_vec(),
        TrackValue::Quat(q) => euler_deg(*q).to_vec(),
        TrackValue::Color(c) => c.to_vec(),
        TrackValue::Bool(b) => vec![if *b { 1.0 } else { 0.0 }],
    }
}

/// Euler degrees of each rotation key, unwrapped against the key before it.
fn unwrapped_euler(raw: &[Vec4]) -> Vec<[f32; 3]> {
    let mut out: Vec<[f32; 3]> = Vec::with_capacity(raw.len());
    for q in raw {
        let e = euler_deg(q.to_array());
        let e = match out.last() {
            Some(p) => [wrap_deg_near(e[0], p[0]), wrap_deg_near(e[1], p[1]), wrap_deg_near(e[2], p[2])],
            None => e,
        };
        out.push(e);
    }
    out
}

pub(super) fn curve_set(clip: &renzora_animation::AnimClip, row: RowKind, group: u8, hidden: [bool; 4]) -> Option<CurveSet> {
    let xyz = [("X", channel_color(0)), ("Y", channel_color(1)), ("Z", channel_color(2))];
    let mut bone = None;
    let (title, unit, channels): (String, &'static str, Vec<Channel>) = match row {
        // An effect's rows are keyed in the dopesheet only.
        RowKind::Summary | RowKind::FxGroup | RowKind::FxEmitter(..) | RowKind::FxTrack(_) => return None,
        RowKind::Bone(ti) => {
            let t = clip.tracks.get(ti)?;
            let lane = Lane::Bone { track: ti, channel: group };
            let keys = t.channel_keys(group);
            let raw: Vec<Vec4> = keys.iter().map(|k| k.1).collect();
            let vals: Vec<[f32; 3]> = if group == 1 { unwrapped_euler(&raw) } else { raw.iter().map(|v| v.truncate().to_array()).collect() };
            let name = match group {
                0 => renzora::lang::t_or("timeline.position", "Position"),
                1 => renzora::lang::t_or("timeline.rotation", "Rotation"),
                _ => renzora::lang::t_or("timeline.scale", "Scale"),
            };
            let chans = (0..3)
                .filter(|c| !hidden[*c])
                .map(|c| Channel { label: xyz[c].0, color: xyz[c].1, keys: keys.iter().zip(&vals).map(|(k, v)| (k.0, v[c])).collect(), lane, comp: c })
                .collect();
            bone = Some(BoneCurves {
                spline: t.channel_spline(group),
                modes: (0..raw.len()).map(|i| t.key_curve(group, i).mode).collect(),
                raw,
                rotation: group == 1,
            });
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
                // A one-value property has nothing to hide.
                .filter(|(c, _)| n == 1 || !hidden[(*c).min(3)])
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
        // One weight, evaluated and handled exactly like a bone channel.
        RowKind::Morph(mi) => {
            let m = clip.morph_tracks.get(mi)?;
            let lane = Lane::Morph { track: mi };
            let raw: Vec<Vec4> = m.keys.iter().map(|k| Vec4::new(k.1, 0.0, 0.0, 0.0)).collect();
            bone = Some(BoneCurves { spline: m.spline(), modes: (0..raw.len()).map(|i| m.key_curve(i).mode).collect(), raw, rotation: false });
            let chans = vec![Channel { label: "", color: MORPH, keys: m.keys.clone(), lane, comp: 0 }];
            (format!("{} · {}", m.label(), renzora::lang::t_or("timeline.blend_shape", "Blend shape")), "", chans)
        }
    };
    let (vmin, vmax) = span_of(channels.iter().flat_map(|c| c.keys.iter().map(|k| k.1)));
    Some(CurveSet { title, unit, channels, vmin, vmax, bone, subject: (row, group) })
}

/// A padded value range around `values`, never empty or zero-height.
pub(super) fn span_of(values: impl Iterator<Item = f32>) -> (f32, f32) {
    let mut vmin = f32::INFINITY;
    let mut vmax = f32::NEG_INFINITY;
    for v in values {
        vmin = vmin.min(v);
        vmax = vmax.max(v);
    }
    if !vmin.is_finite() {
        (vmin, vmax) = (-1.0, 1.0);
    }
    if (vmax - vmin).abs() < 1e-4 {
        vmin -= 1.0;
        vmax += 1.0;
    }
    let pad = (vmax - vmin) * 0.08;
    (vmin - pad, vmax + pad)
}

impl CurveSet {
    /// A channel's shown value at `t`: what playback would show, for the
    /// drawing.
    pub(super) fn sample(&self, clip: &renzora_animation::AnimClip, ch: &Channel, t: f32) -> Option<f32> {
        match ch.lane {
            Lane::Prop { track } => {
                let p = clip.property_tracks.get(track)?;
                renzora::sample_property_track(p, t).map(|v| comps(&v).get(ch.comp).copied().unwrap_or(0.0))
            }
            Lane::Bone { .. } | Lane::Morph { .. } => {
                let b = self.bone.as_ref()?;
                if b.spline.is_empty() {
                    return None;
                }
                if b.rotation {
                    let e = quat_to_euler_deg(b.spline.sample_quat(t))[ch.comp];
                    // Unwrapped against the key the segment starts from, so
                    // the drawn line meets the (unwrapped) key it leaves.
                    let i = ch.keys.partition_point(|k| k.0 <= t).saturating_sub(1);
                    Some(ch.keys.get(i).map_or(e, |k| wrap_deg_near(e, k.1)))
                } else {
                    Some(b.spline.sample(t)[ch.comp])
                }
            }
            Lane::Fx { .. } => None,
        }
    }

    /// A bone key's `(in, out)` slopes on one channel, in shown units per
    /// second (degrees for a rotation), for drawing and dragging its handles.
    pub(super) fn slopes(&self, comp: usize, index: usize) -> Option<(f32, f32)> {
        let b = self.bone.as_ref()?;
        let (a, o) = b.spline.tangents(index)?;
        if b.rotation {
            let q = *b.raw.get(index)?;
            Some((euler_slope_of_quat_tangent(q, a)[comp], euler_slope_of_quat_tangent(q, o)[comp]))
        } else {
            Some((a[comp], o[comp]))
        }
    }

    /// Whether key `index` shows tangent handles: it is smooth, so its
    /// tangents shape the curve and dragging them means something.
    pub(super) fn has_handles(&self, index: usize) -> bool {
        self.bone.as_ref().and_then(|b| b.modes.get(index)).is_some_and(|m| m.is_smooth())
    }

    pub(super) fn shape(&self, clip: &renzora_animation::AnimClip, ch: &Channel, index: usize) -> KeyShape {
        match (ch.lane, self.bone.as_ref()) {
            (Lane::Bone { .. } | Lane::Morph { .. }, Some(b)) => bone_key_shape(b.modes.get(index).copied().unwrap_or_default()),
            (Lane::Prop { track }, _) => clip.property_tracks.get(track).and_then(|p| p.keys.get(index)).map(|k| prop_key_shape(k.interp)).unwrap_or(KeyShape::Diamond),
            _ => KeyShape::Diamond,
        }
    }
}

/// The value window mapped onto the lane area's height.
#[derive(Clone, Copy)]
pub(super) struct Frame {
    pub(super) vmin: f32,
    pub(super) vmax: f32,
    pub(super) h: f32,
}

impl Frame {
    /// Pixels per value unit.
    pub(super) fn py(&self) -> f32 {
        (self.h - PAD_Y * 2.0).max(1.0) / (self.vmax - self.vmin).max(1e-6)
    }
    pub(super) fn y_of(&self, v: f32) -> f32 {
        PAD_Y + (self.vmax - v) * self.py()
    }
    pub(super) fn v_of(&self, y: f32) -> f32 {
        self.vmax - (y - PAD_Y) / self.py()
    }
}

/// The frame the view draws `set` in: the stored window when it was set for
/// this subject, else every key of it.
pub(super) fn frame_of(world: &World, set: &CurveSet, h: f32) -> Frame {
    let (vmin, vmax) = world
        .get_resource::<CurveView>()
        .filter(|v| v.subject == Some(set.subject))
        .and_then(|v| v.range)
        .unwrap_or((set.vmin, set.vmax));
    Frame { vmin, vmax, h: h.max(60.0) }
}

/// The ends of a key's tangent handles, `(in, out)`, in lane space: a fixed
/// length on screen along each slope.
pub(super) fn handle_points(key: Vec2, slopes: (f32, f32), zoom: f32, frame: &Frame) -> (Vec2, Vec2) {
    let dir = |slope: f32| Vec2::new(zoom, -slope * frame.py()).normalize_or(Vec2::X) * HANDLE_PX;
    (key - dir(slopes.0), key + dir(slopes.1))
}

/// The subject the Curves view shows now, and the set to draw for it.
pub(super) fn current_set(world: &World) -> Option<CurveSet> {
    let clip = cur_clip(world)?;
    let layout = world.get_resource::<TrackLayout>()?;
    let mode = world.get_resource::<TimelineMode>()?;
    layout.selected.and_then(|r| curve_set(clip, r, mode.group, mode.hidden))
}

// ── Drawing ──────────────────────────────────────────────────────────────────

pub(super) fn curves_token(w: &Rx) -> u64 {
    let world = w.untracked();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    if let Some(m) = world.get_resource::<TimelineMode>() {
        (m.curves, m.group, m.hidden).hash(&mut h);
        if !m.curves {
            return h.finish();
        }
    }
    if let Some(s) = state(world) {
        (s.timeline_zoom.to_bits(), s.timeline_scroll.to_bits(), s.scrub_time.to_bits()).hash(&mut h);
    }
    if let Some(l) = world.get_resource::<TrackLayout>() {
        (l.selected, l.view.x.to_bits(), l.view.y.to_bits()).hash(&mut h);
    }
    if let Some(v) = world.get_resource::<CurveView>() {
        (v.subject, v.range.map(|(a, b)| (a.to_bits(), b.to_bits()))).hash(&mut h);
    }
    if let Some(s) = world.get_resource::<CurveSel>() {
        s.gen.hash(&mut h);
    }
    world.get_resource::<SelectedKey>().and_then(|s| s.0).hash(&mut h);
    // The subject's whole track: values and tangents move curves without
    // moving key times.
    if let (Some(clip), Some(l)) = (cur_clip(world), world.get_resource::<TrackLayout>()) {
        let group = world.get_resource::<TimelineMode>().map(|m| m.group).unwrap_or(1);
        match l.selected {
            Some(RowKind::Bone(i)) => {
                if let Some(t) = clip.tracks.get(i) {
                    for (time, v) in t.channel_keys(group) {
                        (time.to_bits(), v.to_array().map(f32::to_bits)).hash(&mut h);
                    }
                    for c in t.curves.channel(group) {
                        (c.mode, c.in_tan.map(f32::to_bits), c.out_tan.map(f32::to_bits)).hash(&mut h);
                    }
                }
            }
            Some(RowKind::Prop(i)) => {
                if let Some(p) = clip.property_tracks.get(i) {
                    for k in &p.keys {
                        k.time.to_bits().hash(&mut h);
                        comps(&k.value).iter().for_each(|c| c.to_bits().hash(&mut h));
                        format!("{:?}", k.interp).hash(&mut h);
                    }
                }
            }
            Some(RowKind::Morph(i)) => {
                if let Some(m) = clip.morph_tracks.get(i) {
                    for (time, w) in &m.keys {
                        (time.to_bits(), w.to_bits()).hash(&mut h);
                    }
                    for c in &m.curves {
                        (c.mode, c.in_tan.map(f32::to_bits), c.out_tan.map(f32::to_bits)).hash(&mut h);
                    }
                }
            }
            _ => {}
        }
    }
    h.finish()
}

enum CurveElem {
    Text(f32, f32, String, (u8, u8, u8), f32),
    Line(Vec2, Vec2, Color, f32),
    Key(Vec2, (u8, u8, u8), bool, KeyShape),
    /// A tangent handle's end: a small round grip.
    Grip(Vec2, (u8, u8, u8)),
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

    let Some(set) = current_set(w) else {
        elems.push(CurveElem::Text(width * 0.5 - 120.0, height * 0.5 - 8.0, renzora::lang::t_or("timeline.curves_pick_track", "Select a track on the left to see its curves"), (138, 138, 150), 12.0));
        return build_snapshot(elems);
    };
    let frame = frame_of(w, &set, height);

    // Value grid with labels, on round steps of the visible range.
    let step = nice_step((frame.vmax - frame.vmin) / 5.0);
    let mut v = (frame.vmin / step).ceil() * step;
    let mut guard = 0;
    while v <= frame.vmax && guard < 64 {
        let y = frame.y_of(v);
        let a = if v.abs() < step * 0.01 { 0.12 } else { 0.05 };
        elems.push(CurveElem::Line(Vec2::new(0.0, y), Vec2::new(width, y), rgb(text_primary()).with_alpha(a), 1.0));
        elems.push(CurveElem::Text(6.0, y - 13.0, format!("{}{}", fmt_value(v, step), set.unit), (106, 106, 118), 10.0));
        v += step;
        guard += 1;
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
    // The playhead, so a key can be lined up against the pose it makes.
    // Always pushed (off-screen it is simply clipped), so it never shifts the
    // indices the keyed list matches the items after it by.
    let xp = x_of(s.scrub_time, zoom, scroll).clamp(-2.0, width + 2.0);
    elems.push(CurveElem::Line(Vec2::new(xp, 0.0), Vec2::new(xp, height), rgb((248, 113, 113)).with_alpha(0.55), 1.0));
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
                if let Some(v) = set.sample(clip, ch, t) {
                    let p = Vec2::new(x, frame.y_of(v));
                    if let Some(q) = prev {
                        elems.push(CurveElem::Line(q, p, rgb(ch.color).with_alpha(0.9), 2.0));
                    }
                    prev = Some(p);
                }
                x += SAMPLE_PX;
            }
        }
    }

    // Keys, where they are far enough apart to be told apart, and the
    // handles of the selected ones.
    let sel = w.get_resource::<CurveSel>();
    let primary = w.get_resource::<SelectedKey>().and_then(|s| s.0);
    for ch in &set.channels {
        let n = ch.keys.len();
        let dense = n > 1 && (ch.keys[n - 1].0 - ch.keys[0].0) / (n - 1) as f32 * zoom < 8.0;
        for (i, (t, v)) in ch.keys.iter().enumerate() {
            let x = x_of(*t, zoom, scroll);
            if x < -HANDLE_PX || x > width + HANDLE_PX {
                continue;
            }
            let pick = CurvePick { key: SelKey { lane: ch.lane, index: i }, comp: ch.comp };
            let selected = sel.is_some_and(|s| s.contains(&pick)) || (primary == Some(pick.key) && sel.is_none_or(|s| s.picks.is_empty()));
            if dense && !selected {
                continue;
            }
            let p = Vec2::new(x, frame.y_of(*v));
            if selected && set.has_handles(i) {
                if let Some(slopes) = set.slopes(ch.comp, i) {
                    let (a, b) = handle_points(p, slopes, zoom, &frame);
                    let col = rgb((210, 210, 220)).with_alpha(0.75);
                    if i > 0 {
                        elems.push(CurveElem::Line(a, p, col, 1.0));
                        elems.push(CurveElem::Grip(a, (230, 230, 238)));
                    }
                    if i + 1 < n {
                        elems.push(CurveElem::Line(p, b, col, 1.0));
                        elems.push(CurveElem::Grip(b, (230, 230, 238)));
                    }
                }
            }
            elems.push(CurveElem::Key(p, if selected { SELECTED } else { ch.color }, selected, set.shape(clip, ch, i)));
        }
    }
    build_snapshot(elems)
}

/// A round step (1, 2 or 5 times a power of ten) near `raw`, for grid lines.
pub(super) fn nice_step(raw: f32) -> f32 {
    let raw = raw.abs().max(1e-6);
    let pow = 10f32.powf(raw.log10().floor());
    let f = raw / pow;
    let m = if f < 1.5 {
        1.0
    } else if f < 3.5 {
        2.0
    } else if f < 7.5 {
        5.0
    } else {
        10.0
    };
    m * pow
}

fn fmt_value(v: f32, step: f32) -> String {
    let decimals = if step >= 1.0 { 0 } else { (-step.log10().floor()) as usize };
    let v = if v.abs() < step * 1e-3 { 0.0 } else { v };
    format!("{v:.decimals$}")
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
                CurveElem::Key(p, c, s, shape) => (2u8, p.x.to_bits(), p.y.to_bits(), c, s, shape).hash(&mut h),
                CurveElem::Grip(p, c) => (3u8, p.x.to_bits(), p.y.to_bits(), c).hash(&mut h),
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
            CurveElem::Key(p, col, s, shape) => spawn_key(c, p.x, p.y, if *s { 10.0 } else { 8.0 }, *col, 1.0, *s, *shape, true),
            CurveElem::Grip(p, col) => c
                .spawn((
                    Node { position_type: PositionType::Absolute, left: Val::Px(p.x - 3.5), top: Val::Px(p.y - 3.5), width: Val::Px(7.0), height: Val::Px(7.0), border_radius: BorderRadius::all(Val::Px(3.5)), ..default() },
                    BackgroundColor(rgb(*col)),
                    BoxShadow::new(rgb(panel_bg()), Val::Px(0.0), Val::Px(0.0), Val::Px(1.5), Val::Px(0.0)),
                    PASS_THROUGH,
                ))
                .id(),
        }),
    }
}

fn empty() -> KeyedSnapshot {
    KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) }
}

// ── Hover readout ────────────────────────────────────────────────────────────

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
pub(super) fn curves_hover(world: &mut World) {
    let readout = (|| {
        let mode = world.get_resource::<TimelineMode>()?;
        if !mode.curves {
            return None;
        }
        let p = {
            let mut q = world.query_filtered::<(&RelativeCursorPosition, &ComputedNode), With<KeyLane>>();
            let (rcp, cn) = q.iter(world).next()?;
            if !rcp.cursor_over {
                return None;
            }
            (rcp.normalized? + Vec2::splat(0.5)) * (cn.size() * cn.inverse_scale_factor())
        };
        let state = world.get_resource::<AnimationEditorState>()?;
        let clip = cur_clip(world)?;
        let layout = world.get_resource::<TrackLayout>()?;
        let set = current_set(world)?;
        let frame = frame_of(world, &set, layout.view.y);
        let zoom = state.timeline_zoom.max(1.0);
        let t = ((p.x - LANE_INSET) / zoom + state.timeline_scroll).clamp(0.0, clip.duration);
        let (ch, v) = set
            .channels
            .iter()
            .filter_map(|c| set.sample(clip, c, t).map(|v| (c, v)))
            .min_by(|a, b| (frame.y_of(a.1) - p.y).abs().total_cmp(&(frame.y_of(b.1) - p.y).abs()))?;
        let label = if ch.label.is_empty() { String::new() } else { format!("{} ", ch.label) };
        Some((format!("{:.2}s · {}{:.1}{}", t, label, v, set.unit), p))
    })();
    let mut boxes = world.query_filtered::<&mut Node, With<HoverBox>>();
    let Some((s, p)) = readout else {
        for mut n in boxes.iter_mut(world) {
            if n.display != Display::None {
                n.display = Display::None;
            }
        }
        return;
    };
    let mut texts = world.query_filtered::<&mut Text, With<HoverText>>();
    for mut text in texts.iter_mut(world) {
        if text.0 != s {
            text.0 = s.clone();
        }
    }
    let (left, top) = (Val::Px(p.x + 14.0), Val::Px((p.y - 30.0).max(2.0)));
    for mut n in boxes.iter_mut(world) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use renzora::{BoneTrack, KeyCurve};

    fn clip_with_spin() -> renzora_animation::AnimClip {
        let mut c = renzora_animation::extract::create_empty_clip("c", 2.0);
        let mut t = BoneTrack::new("arm");
        // A turn about Y through 180 degrees: stored Euler would read 170 then -170.
        t.rotations = vec![
            (0.0, Quat::from_rotation_y(170f32.to_radians()).to_array()),
            (1.0, Quat::from_rotation_y(190f32.to_radians()).to_array()),
        ];
        t.set_key_curve(1, 0, KeyCurve::AUTO);
        t.set_key_curve(1, 1, KeyCurve::AUTO);
        c.tracks.push(t);
        c
    }

    #[test]
    fn rotation_curves_are_unwrapped_across_180() {
        let clip = clip_with_spin();
        let set = curve_set(&clip, RowKind::Bone(0), 1, [false; 4]).unwrap();
        let y = &set.channels[1];
        assert!((y.keys[0].1 - 170.0).abs() < 0.1 && (y.keys[1].1 - 190.0).abs() < 0.1, "{:?}", y.keys);
        let mid = set.sample(&clip, y, 0.5).unwrap();
        assert!((mid - 180.0).abs() < 0.5, "mid {mid}");
    }

    #[test]
    fn hidden_channels_are_left_out() {
        let clip = clip_with_spin();
        let set = curve_set(&clip, RowKind::Bone(0), 1, [true, false, true, false]).unwrap();
        assert_eq!(set.channels.len(), 1);
        assert_eq!(set.channels[0].comp, 1);
    }

    #[test]
    fn a_frame_maps_values_and_back() {
        let f = Frame { vmin: -10.0, vmax: 10.0, h: 236.0 };
        assert!((f.y_of(10.0) - PAD_Y).abs() < 1e-4);
        assert!((f.v_of(f.y_of(3.5)) - 3.5).abs() < 1e-4);
    }

    #[test]
    fn grid_steps_are_round() {
        assert_eq!(nice_step(0.9), 1.0);
        assert_eq!(nice_step(2.6), 2.0);
        assert_eq!(nice_step(40.0), 50.0);
    }
}
