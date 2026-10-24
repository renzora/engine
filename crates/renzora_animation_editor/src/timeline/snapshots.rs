//! The keyed lists the timeline draws: track rows, lanes, the ruler overlay and
//! event markers.
//!
//! Lanes are drawn from [`KeyRuns`]: a run of every-frame keys is one bar with
//! diamonds at its ends, so a baked clip reads as where each bone moves rather
//! than as a wall of overlapping diamonds. Once zoomed in far enough that a
//! run's keys land [`CLUSTER_PX`] apart, its inner keys are drawn (and become
//! pickable) as well. Only the visible rows and time window are built, and the
//! lists are token-gated, so nothing is rebuilt until the clip, the selection
//! or the view actually moves.

use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::ui::UiTransform;

use renzora::{Interp, KeyMode, PropertyTrack};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::LANE_INSET;
use renzora_ember::UiInteraction;

use super::clip::{cur_clip, state, Lane, SelKey, SelectedKey, SelectedKeys, TimelineClip};
use super::edit::loop_strip_h;
use super::layout::{RowKind, TrackLayout};
use super::runs::{KeyRuns, Run};
use super::{
    AddKeyTrackBtn, CollapseBtn, DeletePropTrack, PropTrackCombo, TrackRowBtn, MARKER, MORPH,
    PARTICLE, PROPERTY, ROTATION, SCALE, SELECTED, TRANSLATION,
};

/// Inner keys of a run are drawn once they land at least this far apart.
pub(super) const CLUSTER_PX: f32 = 9.0;
/// How far past the visible window keys are still built, in pixels.
const CULL_MARGIN_PX: f32 = 48.0;
/// Hard cap on lane elements per rebuild: a safety net, not a design limit
/// (the run bars keep real clips far under it).
const MAX_ELEMS: usize = 12_000;

pub(super) fn x_of(t: f32, zoom: f32, scroll: f32) -> f32 {
    (t - scroll) * zoom + LANE_INSET
}

/// Vertical centre of a lane channel inside row `row` (lane space). Bone
/// channels stack T / R / S; property and summary lanes sit centred.
pub(super) fn lane_y(layout: &TrackLayout, row: usize, ch: Option<u8>, th: f32) -> f32 {
    let center = layout.row_top(row, th) + th * 0.5;
    let off = (th * 0.26).min(14.0);
    match ch {
        Some(0) => center - off,
        Some(2) => center + off,
        _ => center,
    }
}

pub(super) fn channel_color(ch: u8) -> (u8, u8, u8) {
    match ch {
        0 => TRANSLATION,
        1 => ROTATION,
        _ => SCALE,
    }
}

/// Times of one lane, in key order.
pub(super) fn lane_key_times(clip: &renzora_animation::AnimClip, lane: Lane) -> Vec<f32> {
    match lane {
        Lane::Bone { track, channel } => clip
            .tracks
            .get(track)
            .map(|t| match channel {
                0 => t.translations.iter().map(|k| k.0).collect(),
                1 => t.rotations.iter().map(|k| k.0).collect(),
                _ => t.scales.iter().map(|k| k.0).collect(),
            })
            .unwrap_or_default(),
        Lane::Prop { track } => clip.property_tracks.get(track).map(|p| p.keys.iter().map(|k| k.time).collect()).unwrap_or_default(),
        Lane::Morph { track } => clip.morph_tracks.get(track).map(|m| m.keys.iter().map(|k| k.0).collect()).unwrap_or_default(),
        // Not the clip's: `TimelineClip::key_times` reads the effect's.
        Lane::Fx { .. } => Vec::new(),
    }
}

/// The summary row's diamonds at a zoom: every distinct key time, thinned so
/// no two land closer than [`CLUSTER_PX`] (a baked clip has a key on every
/// frame, which would otherwise draw as a solid wall). Thinned greedily from
/// the clip's first key, never from the visible window, so a diamond does not
/// hop between neighbouring times as the view scrolls, and drawing and
/// picking (`edit::pick_summary`) always agree.
pub(super) fn summary_diamonds(times: &[f32], zoom: f32) -> Vec<f32> {
    let min_gap = CLUSTER_PX / zoom.max(1.0);
    let mut out: Vec<f32> = Vec::new();
    for &t in times {
        if out.last().is_none_or(|last| t - last >= min_gap) {
            out.push(t);
        }
    }
    out
}

/// Whether key `idx` of a run is drawn as its own diamond (and so pickable).
pub(super) fn key_drawn(run: &Run, idx: usize, zoom: f32) -> bool {
    idx == run.i0 || idx == run.i1 || run.spacing() * zoom >= CLUSTER_PX
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

/// Header label for a property track: "Component · Field" with the target node
/// in parentheses when not the animator entity itself.
pub(super) fn property_label(pt: &PropertyTrack) -> String {
    let base = format!("{} · {}", title_case(&pt.component), title_case(&pt.field));
    if pt.target.is_empty() || pt.target == "self" {
        base
    } else {
        format!("{} ({})", base, pt.target)
    }
}

// ── Track rows ───────────────────────────────────────────────────────────────

/// Rebuild the track rows only when the layout, the scroll window, the
/// selection or a property label changed.
pub(super) fn header_token(w: &Rx) -> u64 {
    let world = w.untracked();
    let Some(layout) = world.get_resource::<TrackLayout>() else { return 0 };
    let th = state(world).map(|s| s.track_height).unwrap_or(super::ROW_DEFAULT);
    let (first, last) = layout.visible_range(th);
    let mut h = hasher();
    (layout.gen, first, last, layout.selected, th.to_bits()).hash(&mut h);
    if let Some(clip) = cur_clip(world) {
        clip.tracks.len().hash(&mut h);
        for t in &clip.tracks {
            t.muted.hash(&mut h);
        }
        for p in &clip.property_tracks {
            (&p.component, &p.field, &p.target, p.muted).hash(&mut h);
        }
        for m in &clip.morph_tracks {
            (&m.target, m.index, &m.name, m.muted).hash(&mut h);
        }
    }
    if let Some(fx) = world.get_resource::<TimelineClip>().and_then(|c| c.fx.as_ref()) {
        super::fx::structure_hash(fx, &mut h);
    }
    h.finish()
}

/// Alpha a disabled track's header and lane are drawn at: still legible (it
/// can be switched back on from the same menu), but plainly not playing.
pub(super) const MUTED_ALPHA: f32 = 0.38;

#[derive(Clone)]
enum HeaderRow {
    Summary { count: usize },
    Bone { track: usize, name: String, depth: u8, caret: Option<bool>, chans: [bool; 3], muted: bool },
    Prop { track: usize, label: Option<String>, muted: bool },
    /// A blend shape: its name, and the mesh node it belongs to (the tooltip).
    Morph { label: String, node: String, muted: bool },
    /// "Particles · Campfire", with the effect's track count.
    FxGroup { title: String, count: usize },
    /// An emitter's on/off row. `muted` when it is switched off with no keys
    /// to switch it on.
    FxEmitter { name: String, depth: u8, muted: bool },
    /// "Flames · Spawn Rate". `editable` hides the buttons on a built-in.
    FxTrack { track: usize, label: String, depth: u8, editable: bool },
}

pub(super) fn header_snapshot(world: &Rx) -> KeyedSnapshot {
    let w = world.untracked();
    let (Some(clip), Some(layout)) = (cur_clip(w), w.get_resource::<TrackLayout>()) else { return empty() };
    let fx = w.get_resource::<TimelineClip>().and_then(|c| c.fx.as_ref());
    let th = state(w).map(|s| s.track_height).unwrap_or(super::ROW_DEFAULT);
    let (first, last) = layout.visible_range(th);
    let total = clip.tracks.len() + clip.property_tracks.len() + clip.morph_tracks.len() + fx.map(|f| f.doc.tracks.len()).unwrap_or(0);
    let rows: Vec<(RowKind, bool, HeaderRow)> = layout.rows[first..last]
        .iter()
        .map(|r| {
            let row = match r.kind {
                RowKind::Summary => HeaderRow::Summary { count: total },
                RowKind::Bone(i) => {
                    let t = &clip.tracks[i];
                    HeaderRow::Bone {
                        track: i,
                        name: t.bone_name.clone(),
                        depth: r.depth,
                        caret: r.has_children.then_some(r.collapsed),
                        chans: [!t.translations.is_empty(), !t.rotations.is_empty(), !t.scales.is_empty()],
                        muted: t.muted,
                    }
                }
                RowKind::Prop(i) => {
                    let pt = &clip.property_tracks[i];
                    HeaderRow::Prop { track: i, label: (!pt.component.is_empty()).then(|| property_label(pt)), muted: pt.muted }
                }
                RowKind::Morph(i) => {
                    let m = &clip.morph_tracks[i];
                    HeaderRow::Morph { label: m.label(), node: m.target.clone(), muted: m.muted }
                }
                RowKind::FxGroup => HeaderRow::FxGroup {
                    title: fx.map(|f| f.title()).unwrap_or_default(),
                    count: fx.map(|f| f.doc.tracks.len()).unwrap_or(0),
                },
                RowKind::FxEmitter(e, track) => HeaderRow::FxEmitter {
                    name: fx.and_then(|f| f.doc.emitters.get(e)).map(|em| em.name.clone()).unwrap_or_default(),
                    depth: r.depth,
                    muted: track.is_none() && fx.and_then(|f| f.doc.emitters.get(e)).is_some_and(|em| !em.enabled),
                },
                RowKind::FxTrack(i) => HeaderRow::FxTrack {
                    track: i,
                    label: fx
                        .and_then(|f| f.doc.tracks.get(i).map(|t| renzora_particles::params::track_label(&f.doc, t)))
                        .unwrap_or_default(),
                    depth: r.depth,
                    editable: fx.is_some_and(|f| f.editable),
                },
            };
            (r.kind, layout.selected == Some(r.kind), row)
        })
        .collect();
    let items: Vec<(u64, u64)> = rows
        .iter()
        .map(|(kind, sel, row)| {
            let mut k = hasher();
            kind.hash(&mut k);
            let mut h = hasher();
            (sel, th.to_bits()).hash(&mut h);
            match row {
                HeaderRow::Summary { count } => (0u8, count).hash(&mut h),
                HeaderRow::Bone { name, depth, caret, chans, muted, .. } => (1u8, name, depth, caret, chans, muted).hash(&mut h),
                HeaderRow::Prop { label, muted, .. } => (2u8, label, muted).hash(&mut h),
                HeaderRow::Morph { label, node, muted } => (6u8, label, node, muted).hash(&mut h),
                HeaderRow::FxGroup { title, count } => (3u8, title, count).hash(&mut h),
                HeaderRow::FxEmitter { name, depth, muted } => (4u8, name, depth, muted).hash(&mut h),
                HeaderRow::FxTrack { label, depth, editable, .. } => (5u8, label, depth, editable).hash(&mut h),
            }
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (kind, sel, row) = &rows[i];
            header_row(c, f, *kind, *sel, row, th)
        }),
    }
}

fn header_row(commands: &mut Commands, fonts: &EmberFonts, kind: RowKind, selected: bool, row: &HeaderRow, th: f32) -> Entity {
    let bg = if selected {
        rgb(accent()).with_alpha(0.22)
    } else if matches!(kind, RowKind::Summary | RowKind::FxGroup) {
        rgb(faint_bg())
    } else {
        Color::NONE
    };
    let indent = match row {
        HeaderRow::Bone { depth, .. } | HeaderRow::FxEmitter { depth, .. } | HeaderRow::FxTrack { depth, .. } => 6.0 + *depth as f32 * 14.0,
        _ => 6.0,
    };
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(th), flex_shrink: 0.0, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), padding: UiRect::new(Val::Px(indent), Val::Px(8.0), Val::Px(0.0), Val::Px(0.0)), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() },
            BackgroundColor(bg),
            UiInteraction::default(),
            TrackRowBtn(kind),
        ))
        .id();
    let mut kids = Vec::new();
    let caret_slot = |commands: &mut Commands, glyph: Option<&str>, track: Option<usize>| -> Entity {
        let slot = commands
            .spawn((Node { width: Val::Px(12.0), height: Val::Px(th), flex_shrink: 0.0, align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() }, BackgroundColor(Color::NONE)))
            .id();
        if let Some(g) = glyph {
            let ic = icon_text(commands, &fonts.phosphor, g, text_muted(), 10.0);
            commands.entity(ic).insert(PASS_THROUGH);
            commands.entity(slot).add_child(ic);
        }
        if let Some(t) = track {
            commands.entity(slot).insert((UiInteraction::default(), CollapseBtn(t)));
        }
        slot
    };
    match row {
        HeaderRow::Summary { count } => {
            // A stack, not a caret: this row merges every track and has
            // nothing to fold, and a caret that did nothing read as broken.
            // Same slot as the bones' carets, so the labels still line up.
            kids.push(caret_slot(commands, Some("stack"), None));
            let lbl = commands
                .spawn((Text::new(renzora::lang::t_or("timeline.all_tracks", "All tracks")), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())), bevy::text::TextLayout::no_wrap(), Node { flex_grow: 1.0, ..default() }, PASS_THROUGH))
                .id();
            let n = commands.spawn((Text::new(count.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb((138, 138, 150))), PASS_THROUGH)).id();
            kids.extend([lbl, n]);
        }
        HeaderRow::Bone { track, name, caret, chans, muted, .. } => {
            let glyph = caret.map(|collapsed| if collapsed { "caret-right" } else { "caret-down" });
            kids.push(caret_slot(commands, glyph, caret.map(|_| *track)));
            let bone = icon_text(commands, &fonts.phosphor, "bone", (154, 154, 166), 11.0);
            commands.entity(bone).insert(PASS_THROUGH);
            let lbl = commands
                .spawn((Text::new(name.clone()), ui_font(&fonts.ui, 12.0), TextColor(rgb(if selected { text_primary() } else { (196, 196, 206) })), PASS_THROUGH))
                .id();
            renzora_ember::widgets::ellipsize(commands, lbl, name.clone(), Some(root));
            commands.entity(lbl).entry::<Node>().or_default().and_modify(|mut n| n.flex_grow = 1.0);
            kids.extend([bone, lbl]);
            if *muted {
                dim_text(commands, &[bone, lbl], MUTED_ALPHA);
                kids.push(muted_badge(commands, fonts));
            }
            for (ch, letter) in [(0u8, "T"), (1, "R"), (2, "S")] {
                // A disabled bone's channels read as unkeyed: nothing plays.
                kids.push(chip(commands, fonts, letter, channel_color(ch), chans[ch as usize] && !*muted));
            }
        }
        HeaderRow::Prop { track, label, muted } => {
            kids.push(caret_slot(commands, None, None));
            let ic = icon_text(commands, &fonts.phosphor, "sliders-horizontal", PROPERTY, 11.0);
            commands.entity(ic).insert(PASS_THROUGH);
            if *muted {
                dim_text(commands, &[ic], MUTED_ALPHA);
            }
            let combo = commands
                .spawn((
                    Node { flex_grow: 1.0, min_width: Val::Px(0.0), height: Val::Px((th - 4.0).max(14.0)), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::horizontal(Val::Px(6.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
                    BackgroundColor(rgb(faint_bg())),
                    UiInteraction::default(),
                    bevy::ui::RelativeCursorPosition::default(),
                    PropTrackCombo(*track),
                ))
                .id();
            let (txt, col) = match label {
                Some(l) => (l.clone(), text_primary()),
                None => (renzora::lang::t("timeline.select_property"), text_muted()),
            };
            let combo_lbl = commands
                .spawn((Text::new(txt.clone()), ui_font(&fonts.ui, 12.0), TextColor(rgb(col).with_alpha(if *muted { MUTED_ALPHA } else { 1.0 })), PASS_THROUGH))
                .id();
            renzora_ember::widgets::ellipsize(commands, combo_lbl, txt, Some(combo));
            commands.entity(combo_lbl).entry::<Node>().or_default().and_modify(|mut n| n.flex_grow = 1.0);
            let caret = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 9.0);
            commands.entity(caret).insert(PASS_THROUGH);
            commands.entity(combo).add_children(&[combo_lbl, caret]);
            let addk = small_btn(commands, fonts, "diamond", PROPERTY, AddKeyTrackBtn(*track));
            commands.entity(addk).insert(renzora_ember::widgets::HoverTooltip::new(renzora::lang::t_or("timeline.key_track_tip", "Key this track at the playhead")));
            let del = small_btn(commands, fonts, "x", text_muted(), DeletePropTrack(*track));
            kids.extend([ic, combo]);
            if *muted {
                kids.push(muted_badge(commands, fonts));
            }
            kids.extend([addk, del]);
        }
        // Like a bone row: a glyph, the shape's name, and the mesh it lives on
        // on hover. Keyed from the Face panel's sliders (or K over a pose),
        // so the row carries no buttons of its own.
        HeaderRow::Morph { label, node, muted } => {
            kids.push(caret_slot(commands, None, None));
            let ic = icon_text(commands, &fonts.phosphor, "smiley", MORPH, 11.0);
            commands.entity(ic).insert(PASS_THROUGH);
            let lbl = commands
                .spawn((Text::new(label.clone()), ui_font(&fonts.ui, 12.0), TextColor(rgb(if selected { text_primary() } else { (196, 196, 206) })), PASS_THROUGH))
                .id();
            renzora_ember::widgets::ellipsize(commands, lbl, label.clone(), Some(root));
            commands.entity(lbl).entry::<Node>().or_default().and_modify(|mut n| n.flex_grow = 1.0);
            commands.entity(root).insert(renzora_ember::widgets::HoverTooltip::new(format!("{label} on {node}")));
            kids.extend([ic, lbl]);
            if *muted {
                dim_text(commands, &[ic, lbl], MUTED_ALPHA);
                kids.push(muted_badge(commands, fonts));
            }
        }
        // The Particles group reads as the clip's own rows do: the head like
        // "All tracks", each row a name with a sparkle where a bone has its
        // bone, so the effect's tracks sit in the list as tracks.
        HeaderRow::FxGroup { title, count } => {
            kids.push(caret_slot(commands, Some("sparkle"), None));
            let lbl = commands
                .spawn((Text::new(title.clone()), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())), PASS_THROUGH))
                .id();
            renzora_ember::widgets::ellipsize(commands, lbl, title.clone(), Some(root));
            commands.entity(lbl).entry::<Node>().or_default().and_modify(|mut n| n.flex_grow = 1.0);
            let n = commands.spawn((Text::new(count.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb((138, 138, 150))), PASS_THROUGH)).id();
            kids.extend([lbl, n]);
        }
        HeaderRow::FxEmitter { name, muted, .. } => {
            kids.push(caret_slot(commands, None, None));
            let ic = icon_text(commands, &fonts.phosphor, "sparkle", PARTICLE, 11.0);
            commands.entity(ic).insert(PASS_THROUGH);
            let lbl = commands
                .spawn((Text::new(name.clone()), ui_font(&fonts.ui, 12.0), TextColor(rgb(if selected { text_primary() } else { (196, 196, 206) })), PASS_THROUGH))
                .id();
            renzora_ember::widgets::ellipsize(commands, lbl, name.clone(), Some(root));
            commands.entity(lbl).entry::<Node>().or_default().and_modify(|mut n| n.flex_grow = 1.0);
            kids.extend([ic, lbl]);
            if *muted {
                dim_text(commands, &[ic, lbl], MUTED_ALPHA);
                kids.push(muted_badge(commands, fonts));
            }
        }
        HeaderRow::FxTrack { track, label, editable, .. } => {
            kids.push(caret_slot(commands, None, None));
            let ic = icon_text(commands, &fonts.phosphor, "sparkle", PARTICLE, 11.0);
            commands.entity(ic).insert(PASS_THROUGH);
            let lbl = commands
                .spawn((Text::new(label.clone()), ui_font(&fonts.ui, 12.0), TextColor(rgb(if selected { text_primary() } else { (196, 196, 206) })), PASS_THROUGH))
                .id();
            renzora_ember::widgets::ellipsize(commands, lbl, label.clone(), Some(root));
            commands.entity(lbl).entry::<Node>().or_default().and_modify(|mut n| n.flex_grow = 1.0);
            kids.extend([ic, lbl]);
            if *editable {
                let addk = small_btn(commands, fonts, "diamond", PARTICLE, super::fx::FxAddKeyBtn(*track));
                commands.entity(addk).insert(renzora_ember::widgets::HoverTooltip::new(renzora::lang::t_or("timeline.key_track_tip", "Key this track at the playhead")));
                let del = small_btn(commands, fonts, "x", text_muted(), super::fx::FxDeleteTrackBtn(*track));
                kids.extend([addk, del]);
            }
        }
    }
    commands.entity(root).add_children(&kids);
    root
}

/// Re-tint already spawned text (an `icon_text` glyph or a label) at `alpha`.
fn dim_text(commands: &mut Commands, ents: &[Entity], alpha: f32) {
    for &e in ents {
        commands.entity(e).entry::<TextColor>().and_modify(move |mut c| {
            c.0 = c.0.with_alpha(alpha);
        });
    }
}

/// The eye-slash beside a disabled track's name, so the dimming reads as a
/// state and not as a rendering fault. Visual only: the row's right-click
/// menu switches it back on.
fn muted_badge(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let ic = icon_text(commands, &fonts.phosphor, "eye-slash", text_muted(), 11.0);
    commands.entity(ic).insert(PASS_THROUGH);
    ic
}

fn small_btn<M: Component>(commands: &mut Commands, fonts: &EmberFonts, icon: &str, color: (u8, u8, u8), marker: M) -> Entity {
    let b = commands
        .spawn((Node { width: Val::Px(18.0), height: Val::Px(18.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(4.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(Color::NONE), UiInteraction::default(), marker))
        .id();
    super::build::hover_wash(commands, b);
    let ic = icon_text(commands, &fonts.phosphor, icon, color, 11.0);
    commands.entity(ic).insert(PASS_THROUGH);
    commands.entity(b).add_child(ic);
    b
}

/// A T / R / S chip: the channel's colour as a tint, dimmed when the bone has
/// no keys on that channel.
fn chip(commands: &mut Commands, fonts: &EmberFonts, letter: &str, color: (u8, u8, u8), on: bool) -> Entity {
    let (bg, fg) = if on {
        (rgb(color).with_alpha(0.14), rgb(color))
    } else {
        (rgb(faint_bg()), rgb((90, 90, 102)))
    };
    let c = commands
        .spawn((Node { height: Val::Px(14.0), padding: UiRect::horizontal(Val::Px(4.0)), align_items: AlignItems::Center, border_radius: BorderRadius::all(Val::Px(3.0)), flex_shrink: 0.0, ..default() }, BackgroundColor(bg), PASS_THROUGH))
        .id();
    let t = commands.spawn((Text::new(letter.to_string()), ui_font(&fonts.ui, 10.0), TextColor(fg), PASS_THROUGH)).id();
    commands.entity(c).add_child(t);
    c
}

// ── Lanes ────────────────────────────────────────────────────────────────────

/// Rebuild the lanes only when the clip's keys, the rows, the view or the
/// selection moved.
pub(super) fn lanes_token(w: &Rx) -> u64 {
    let world = w.untracked();
    let mut h = hasher();
    if let Some(s) = state(world) {
        (s.timeline_zoom.to_bits(), s.timeline_scroll.to_bits(), s.track_height.to_bits()).hash(&mut h);
    }
    if let Some(l) = world.get_resource::<TrackLayout>() {
        (l.gen, l.vscroll.to_bits(), l.view.x.to_bits(), l.view.y.to_bits(), l.selected).hash(&mut h);
    }
    if let Some(r) = world.get_resource::<KeyRuns>() {
        r.gen.hash(&mut h);
    }
    if let Some(s) = world.get_resource::<SelectedKeys>() {
        s.gen.hash(&mut h);
    }
    world.get_resource::<SelectedKey>().and_then(|s| s.0).hash(&mut h);
    if let Some(c) = cur_clip(world) {
        c.duration.to_bits().hash(&mut h);
        // Muting changes no key, so the runs above do not see it; nor does a
        // key's interpolation, which its shape shows.
        for t in &c.tracks {
            t.muted.hash(&mut h);
            for ch in 0..3u8 {
                for k in t.curves.channel(ch) {
                    k.mode.hash(&mut h);
                }
            }
        }
        for p in &c.property_tracks {
            p.muted.hash(&mut h);
            for k in &p.keys {
                prop_key_shape(k.interp).hash(&mut h);
            }
        }
        for m in &c.morph_tracks {
            m.muted.hash(&mut h);
            for k in &m.curves {
                k.mode.hash(&mut h);
            }
        }
    }
    h.finish()
}

#[derive(Clone, Copy)]
enum LaneElem {
    /// The selected row's faint lane wash: (row).
    RowWash(usize),
    /// Everything past the clip's end, dimmed: (x).
    PastEnd(f32),
    /// A run bar: (row, channel, t0, t1, colour, amber).
    Bar(usize, Option<u8>, f32, f32, (u8, u8, u8), bool),
    /// A key: (row, channel, t, colour, selected, shape).
    Key(usize, Option<u8>, f32, (u8, u8, u8), bool, KeyShape),
}

pub(super) fn keyframe_snapshot(world: &Rx) -> KeyedSnapshot {
    let w = world.untracked();
    let (Some(clip), Some(s), Some(layout), Some(runs)) =
        (cur_clip(w), state(w), w.get_resource::<TrackLayout>(), w.get_resource::<KeyRuns>())
    else {
        return empty();
    };
    let cache = w.get_resource::<TimelineClip>();
    let fx = cache.and_then(|c| c.fx.as_ref());
    let sel: HashSet<SelKey> = w
        .get_resource::<SelectedKeys>()
        .map(|s| s.keys.iter().copied().collect())
        .unwrap_or_default();
    let primary = w.get_resource::<SelectedKey>().and_then(|s| s.0);
    let is_sel = |k: SelKey| sel.contains(&k) || primary == Some(k);
    let (zoom, scroll, th) = (s.timeline_zoom.max(1.0), s.timeline_scroll, s.track_height.max(1.0));
    let width = if layout.view.x > 1.0 { layout.view.x } else { 4096.0 };
    let t_min = scroll - CULL_MARGIN_PX / zoom;
    let t_max = scroll + (width + CULL_MARGIN_PX) / zoom;
    let (first, last) = layout.visible_range(th);

    let mut elems: Vec<LaneElem> = Vec::new();
    let end_x = x_of(clip.duration, zoom, scroll);
    if end_x < width {
        elems.push(LaneElem::PastEnd(end_x.max(0.0)));
    }
    if let Some(row) = layout.selected.and_then(|k| layout.row_of(k)) {
        if row >= first && row < last {
            elems.push(LaneElem::RowWash(row));
        }
    }

    let lane = |elems: &mut Vec<LaneElem>, row: usize, ch: Option<u8>, lane: Lane, runs: &[Run], times: &[f32], color: (u8, u8, u8)| {
        let start = runs.partition_point(|r| r.t1 < t_min);
        for r in &runs[start..] {
            if r.t0 > t_max || elems.len() > MAX_ELEMS {
                break;
            }
            if r.len() > 1 {
                // Selected keys hidden inside an undrawn run tint the stretch.
                let inner_drawn = r.spacing() * zoom >= CLUSTER_PX;
                let mut amber: Option<(f32, f32)> = None;
                if !inner_drawn {
                    for (idx, &t) in times.iter().enumerate().take(r.i1).skip(r.i0 + 1) {
                        if is_sel(SelKey { lane, index: idx }) {
                            amber = Some(amber.map_or((t, t), |(a, b)| (a.min(t), b.max(t))));
                        }
                    }
                }
                elems.push(LaneElem::Bar(row, ch, r.t0, r.t1, color, false));
                if let Some((a, b)) = amber {
                    elems.push(LaneElem::Bar(row, ch, a, b, SELECTED, true));
                }
            }
            let lo = r.i0.max(times.partition_point(|t| *t < t_min));
            let hi = r.i1.min(times.partition_point(|t| *t <= t_max).saturating_sub(1));
            if r.len() == 1 || r.spacing() * zoom < CLUSTER_PX {
                for idx in std::iter::once(r.i0).chain((r.i1 != r.i0).then_some(r.i1)) {
                    if idx >= lo && idx <= hi {
                        let sd = is_sel(SelKey { lane, index: idx });
                        elems.push(LaneElem::Key(row, ch, times[idx], if sd { SELECTED } else { color }, sd, lane_key_shape(clip, lane, idx)));
                    }
                }
            } else {
                for (idx, &t) in times.iter().enumerate().take(hi + 1).skip(lo) {
                    let sd = is_sel(SelKey { lane, index: idx });
                    elems.push(LaneElem::Key(row, ch, t, if sd { SELECTED } else { color }, sd, lane_key_shape(clip, lane, idx)));
                }
            }
        }
    };

    for row in first..last {
        match layout.rows[row].kind {
            RowKind::Summary => {
                // The bar marks where some track keys continuously (every
                // frame or two); the diamonds are the key times themselves,
                // each one draggable to retime that moment on every track.
                // Tinted with the theme accent: the neutral grey it used to be
                // read as a disabled control rather than as data.
                let start = runs.summary.partition_point(|(_, b)| *b < t_min);
                for (a, b) in &runs.summary[start..] {
                    if *a > t_max {
                        break;
                    }
                    elems.push(LaneElem::Bar(row, None, *a, *b, accent(), false));
                }
                for t in summary_diamonds(&runs.times, zoom) {
                    if t < t_min {
                        continue;
                    }
                    if t > t_max || elems.len() > MAX_ELEMS {
                        break;
                    }
                    elems.push(LaneElem::Key(row, None, t, accent(), false, KeyShape::Diamond));
                }
            }
            RowKind::Bone(ti) => {
                let Some(chans) = runs.bone.get(ti) else { continue };
                for ch in 0..3u8 {
                    let l = Lane::Bone { track: ti, channel: ch };
                    let times = lane_key_times(clip, l);
                    lane(&mut elems, row, Some(ch), l, &chans[ch as usize], &times, channel_color(ch));
                }
            }
            RowKind::Prop(pi) => {
                let Some(pr) = runs.prop.get(pi) else { continue };
                let l = Lane::Prop { track: pi };
                let times = lane_key_times(clip, l);
                lane(&mut elems, row, None, l, pr, &times, PROPERTY);
            }
            RowKind::Morph(mi) => {
                let Some(mr) = runs.morph.get(mi) else { continue };
                let l = Lane::Morph { track: mi };
                let times = lane_key_times(clip, l);
                lane(&mut elems, row, None, l, mr, &times, MORPH);
            }
            RowKind::FxGroup => {}
            RowKind::FxEmitter(e, track) => {
                // The bar is where the emitter is on; its keys, when it has
                // an `Enabled` track, are the switches.
                let Some(fx) = fx else { continue };
                for (a, b) in super::fx::enabled_spans(&fx.doc, e, clip.duration) {
                    if b < t_min || a > t_max {
                        continue;
                    }
                    elems.push(LaneElem::Bar(row, None, a, b, PARTICLE, false));
                }
                if let (Some(t), Some(cache)) = (track, cache) {
                    let l = Lane::Fx { track: t };
                    let times = cache.key_times(l);
                    lane(&mut elems, row, None, l, runs.fx.get(t).map(Vec::as_slice).unwrap_or(&[]), &times, PARTICLE);
                }
            }
            RowKind::FxTrack(t) => {
                let (Some(cache), Some(fr)) = (cache, runs.fx.get(t)) else { continue };
                let l = Lane::Fx { track: t };
                let times = cache.key_times(l);
                lane(&mut elems, row, None, l, fr, &times, PARTICLE);
            }
        }
    }

    // A disabled track's bars and keys are drawn faded (see `MUTED_ALPHA`).
    let row_muted = |row: usize| match layout.rows[row].kind {
        RowKind::Bone(ti) => clip.tracks.get(ti).is_some_and(|t| t.muted),
        RowKind::Prop(pi) => clip.property_tracks.get(pi).is_some_and(|p| p.muted),
        RowKind::Morph(mi) => clip.morph_tracks.get(mi).is_some_and(|m| m.muted),
        RowKind::Summary | RowKind::FxGroup | RowKind::FxEmitter(..) | RowKind::FxTrack(_) => false,
    };
    let dim: Vec<bool> = elems
        .iter()
        .map(|e| match e {
            LaneElem::Bar(row, ..) | LaneElem::Key(row, ..) => row_muted(*row),
            _ => false,
        })
        .collect();

    // Rows are positioned in lane space, so their y is part of the content.
    let layout_vs = layout.vscroll;
    let ys: Vec<(f32, f32)> = elems
        .iter()
        .map(|e| match e {
            LaneElem::RowWash(row) => (layout.row_top(*row, th), th),
            LaneElem::PastEnd(_) => (0.0, 0.0),
            LaneElem::Bar(row, ch, ..) | LaneElem::Key(row, ch, ..) => (lane_y(layout, *row, *ch, th), 0.0),
        })
        .collect();
    let items: Vec<(u64, u64)> = elems
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            match e {
                LaneElem::RowWash(row) => (0u8, row).hash(&mut h),
                LaneElem::PastEnd(x) => (1u8, x.to_bits()).hash(&mut h),
                LaneElem::Bar(row, ch, a, b, c, am) => (2u8, row, ch, a.to_bits(), b.to_bits(), c, am).hash(&mut h),
                LaneElem::Key(row, ch, t, c, sd, shape) => (3u8, row, ch, t.to_bits(), c, sd, shape).hash(&mut h),
            }
            (zoom.to_bits(), scroll.to_bits(), th.to_bits(), layout_vs.to_bits(), dim[i]).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, _f, i| match elems[i] {
            LaneElem::RowWash(_) => {
                let (top, hgt) = ys[i];
                c.spawn((Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(top), width: Val::Percent(100.0), height: Val::Px(hgt), ..default() }, BackgroundColor(Color::srgba(0.38, 0.55, 0.9, 0.07)), PASS_THROUGH)).id()
            }
            LaneElem::PastEnd(x) => c
                .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(x), top: Val::Px(0.0), right: Val::Px(0.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.22)), PASS_THROUGH))
                .id(),
            LaneElem::Bar(_, ch, a, b, color, amber) => {
                let y = ys[i].0;
                let h = if ch.is_some() { (th * 0.2).clamp(3.0, 6.0) } else { (th * 0.26).clamp(4.0, 7.0) };
                let x0 = x_of(a, zoom, scroll);
                let wdt = ((b - a) * zoom).max(h);
                let alpha = if amber { 0.55 } else if ch.is_none() { 0.22 } else { 0.28 };
                let alpha = if dim[i] { alpha * MUTED_ALPHA } else { alpha };
                c.spawn((
                    Node { position_type: PositionType::Absolute, left: Val::Px(x0 - h * 0.5), top: Val::Px(y - h * 0.5), width: Val::Px(wdt + h), height: Val::Px(h), border_radius: BorderRadius::all(Val::Px(h * 0.5)), ..default() },
                    BackgroundColor(rgb(color).with_alpha(alpha)),
                    PASS_THROUGH,
                ))
                .id()
            }
            LaneElem::Key(_, ch, t, color, sd, shape) => {
                let base = if ch.is_some() { (th * 0.34).clamp(5.0, 9.0) } else { (th * 0.38).clamp(6.0, 10.0) };
                let alpha = if dim[i] { MUTED_ALPHA } else { 1.0 };
                spawn_key(c, x_of(t, zoom, scroll), ys[i].0, base, color, alpha, sd, shape, false)
            }
        }),
    }
}

/// How a key is drawn, which says how it leaves for the next key: a diamond
/// is linear, a square holds, a circle is smooth (an eased property key, or a
/// bone key with tangents). The convention other animation editors use, so
/// a stepped blocking pass and its splined version read apart at a glance.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub(super) enum KeyShape {
    #[default]
    Diamond,
    Square,
    Circle,
}

pub(super) fn bone_key_shape(mode: KeyMode) -> KeyShape {
    match mode {
        KeyMode::Linear => KeyShape::Diamond,
        KeyMode::Constant => KeyShape::Square,
        KeyMode::Auto | KeyMode::Flat | KeyMode::Free | KeyMode::Broken => KeyShape::Circle,
    }
}

pub(super) fn prop_key_shape(interp: Interp) -> KeyShape {
    match interp {
        Interp::Linear => KeyShape::Diamond,
        Interp::Stepped => KeyShape::Square,
        Interp::Eased(_) => KeyShape::Circle,
    }
}

/// The shape a lane's key `idx` is drawn with.
fn lane_key_shape(clip: &renzora_animation::AnimClip, lane: Lane, idx: usize) -> KeyShape {
    match lane {
        Lane::Bone { track, channel } => clip.tracks.get(track).map(|t| bone_key_shape(t.key_curve(channel, idx).mode)).unwrap_or_default(),
        Lane::Prop { track } => clip.property_tracks.get(track).and_then(|p| p.keys.get(idx)).map(|k| prop_key_shape(k.interp)).unwrap_or_default(),
        Lane::Morph { track } => clip.morph_tracks.get(track).map(|m| bone_key_shape(m.key_curve(idx).mode)).unwrap_or_default(),
        Lane::Fx { .. } => KeyShape::Diamond,
    }
}

/// A key marker; a selected one gets a dark gap and an amber ring. `outline`
/// rings an unselected key in the panel colour, so it stands off a curve
/// drawn through it.
#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_key(commands: &mut Commands, x: f32, y: f32, kf: f32, color: (u8, u8, u8), alpha: f32, selected: bool, shape: KeyShape, outline: bool) -> Entity {
    let kf = if selected { kf + 2.0 } else { kf };
    // A square or circle of the same width reads heavier than the diamond
    // (whose corners are what you see), so they are drawn a little smaller.
    let kf = if shape == KeyShape::Diamond { kf } else { (kf * 0.86).round() };
    let half = kf * 0.5;
    let radius = match shape {
        KeyShape::Circle => half,
        _ => 1.5,
    };
    let mut e = commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(x - half),
            top: Val::Px(y - half),
            width: Val::Px(kf),
            height: Val::Px(kf),
            border_radius: BorderRadius::all(Val::Px(radius)),
            ..default()
        },
        BackgroundColor(rgb(color).with_alpha(alpha)),
        PASS_THROUGH,
    ));
    if shape == KeyShape::Diamond {
        e.insert(UiTransform::from_rotation(Rot2::degrees(45.0)));
    }
    if selected {
        e.insert(BoxShadow::new(rgb(SELECTED), Val::Px(0.0), Val::Px(0.0), Val::Px(2.5), Val::Px(0.0)));
    } else if outline {
        e.insert(BoxShadow::new(rgb(panel_bg()), Val::Px(0.0), Val::Px(0.0), Val::Px(1.5), Val::Px(0.0)));
    }
    e.id()
}

// ── Ruler overlay ────────────────────────────────────────────────────────────

/// Pane-space overlay on the ruler, in its two halves (see `edit::RulerGrab`):
/// the loop band and its two handles in the upper strip, the clip-length
/// marker in the lower half, and the playhead's time badge over both. All
/// visual: `edit::ruler_drag` hit-tests them.
///
/// Each item hashes only what it draws, so dragging one handle rebuilds that
/// handle and the band, not the badge or the other handle.
pub(super) fn ruler_snapshot(world: &Rx) -> KeyedSnapshot {
    let w = world.untracked();
    let (Some(clip), Some(s)) = (cur_clip(w), state(w)) else { return empty() };
    let (zoom, scroll) = (s.timeline_zoom.max(1.0), s.timeline_scroll);
    let ruler_h = w.get_resource::<renzora_ember::style::Theme>().map(|t| t.timeline.ruler_height).unwrap_or(28.0);
    let strip_h = loop_strip_h(ruler_h);
    let (ls, le) = s.loop_range(clip.duration);
    let (x0, x1) = (x_of(ls, zoom, scroll), x_of(le, zoom, scroll));
    let xd = x_of(clip.duration, zoom, scroll);
    let xp = x_of(s.scrub_time, zoom, scroll);
    let looping = s.preview_looping;
    let label = format!("{:.2}", s.scrub_time);
    let key = |parts: &dyn Fn(&mut std::collections::hash_map::DefaultHasher)| {
        let mut h = hasher();
        ruler_h.to_bits().hash(&mut h);
        parts(&mut h);
        h.finish()
    };
    let items = vec![
        (0u64, key(&|h| (x0.to_bits(), x1.to_bits(), looping).hash(h))),
        (1u64, key(&|h| x0.to_bits().hash(h))),
        (2u64, key(&|h| x1.to_bits().hash(h))),
        (3u64, key(&|h| xd.to_bits().hash(h))),
        (4u64, key(&|h| (xp.to_bits(), &label).hash(h))),
    ];
    // The loop region: a faint wash down the whole ruler so the region reads
    // against the ticks, and a stronger band in the upper strip, which is the
    // part you grab. Brighter while looping, with a 1px line along its top
    // (3px read as a heavy bar competing with the playhead badge).
    const RANGE: (u8, u8, u8) = (122, 162, 247);
    let loop_handle = move |c: &mut Commands, x: f32| {
        let root = c
            .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(x - 4.0), top: Val::Px(0.0), width: Val::Px(8.0), height: Val::Px(strip_h), ..default() }, PASS_THROUGH))
            .id();
        let line = c
            .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(3.0), top: Val::Px(0.0), width: Val::Px(2.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(rgb(RANGE).with_alpha(0.8)), PASS_THROUGH))
            .id();
        let grip = c
            .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(2.0), width: Val::Px(8.0), height: Val::Px((strip_h - 4.0).max(6.0)), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() }, BackgroundColor(rgb(RANGE)), PASS_THROUGH))
            .id();
        c.entity(root).add_children(&[line, grip]);
        root
    };
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| match i {
            0 => {
                let wash = c
                    .spawn((
                        Node { position_type: PositionType::Absolute, left: Val::Px(x0), top: Val::Px(0.0), width: Val::Px((x1 - x0).max(0.0)), height: Val::Px(ruler_h), ..default() },
                        BackgroundColor(rgb(RANGE).with_alpha(if looping { 0.07 } else { 0.04 })),
                        PASS_THROUGH,
                    ))
                    .id();
                let band = c
                    .spawn((
                        Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Px(strip_h), ..default() },
                        BackgroundColor(rgb(RANGE).with_alpha(if looping { 0.20 } else { 0.11 })),
                        PASS_THROUGH,
                    ))
                    .id();
                let line = c
                    .spawn((
                        Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Percent(100.0), height: Val::Px(1.0), display: if looping { Display::Flex } else { Display::None }, ..default() },
                        BackgroundColor(rgb(RANGE).with_alpha(0.8)),
                        PASS_THROUGH,
                    ))
                    .id();
                c.entity(band).add_child(line);
                c.entity(wash).add_child(band);
                wash
            }
            1 => loop_handle(c, x0),
            2 => loop_handle(c, x1),
            3 => {
                // The clip's real length: a thin line down the lower half and a
                // tab at the bottom edge, in the muted text tone rather than the
                // loop blue, so it is never mistaken for the loop end.
                let root = c
                    .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(xd - 4.5), top: Val::Px(strip_h), width: Val::Px(9.0), height: Val::Px(ruler_h - strip_h), ..default() }, PASS_THROUGH))
                    .id();
                let line = c
                    .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(4.0), top: Val::Px(0.0), width: Val::Px(1.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(rgb(text_muted())), PASS_THROUGH))
                    .id();
                let tab = c
                    .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(0.0), bottom: Val::Px(0.0), width: Val::Px(9.0), height: Val::Px(6.0), border_radius: BorderRadius::top(Val::Px(2.0)), ..default() }, BackgroundColor(rgb(text_muted())), PASS_THROUGH))
                    .id();
                c.entity(root).add_children(&[line, tab]);
                root
            }
            _ => {
                let badge = c
                    .spawn((
                        Node { position_type: PositionType::Absolute, left: Val::Px(xp - 20.0), top: Val::Px(5.0), width: Val::Px(40.0), height: Val::Px(15.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
                        BackgroundColor(rgb((248, 113, 113))),
                        PASS_THROUGH,
                    ))
                    .id();
                let t = c.spawn((Text::new(label.clone()), ui_font(&f.mono, 10.0), TextColor(rgb((26, 26, 33))), bevy::text::TextLayout::no_wrap(), PASS_THROUGH)).id();
                c.entity(badge).add_child(t);
                badge
            }
        }),
    }
}

// ── Markers ──────────────────────────────────────────────────────────────────

/// Renders each clip marker as a labeled flag + thin full-height line. Visual
/// only (`PASS_THROUGH`); deletion is a math hit-test in `key_context_menu`.
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
    let x = x_of(time, zoom, scroll);
    let root = commands
        .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(x), top: Val::Px(0.0), width: Val::Px(0.0), height: Val::Percent(100.0), ..default() }, PASS_THROUGH))
        .id();
    let line = commands
        .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(0.0), top: Val::Px(0.0), width: Val::Px(1.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(Color::srgba_u8(MARKER.0, MARKER.1, MARKER.2, 110)), PASS_THROUGH))
        .id();
    let flag = commands
        .spawn((Node { position_type: PositionType::Absolute, left: Val::Px(1.0), top: Val::Px(0.0), padding: UiRect::axes(Val::Px(3.0), Val::Px(0.0)), border_radius: BorderRadius::all(Val::Px(2.0)), ..default() }, BackgroundColor(rgb(MARKER)), PASS_THROUGH))
        .id();
    let lbl = commands.spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 9.0), TextColor(rgb((25, 20, 30))), bevy::text::TextLayout::no_wrap())).id();
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
