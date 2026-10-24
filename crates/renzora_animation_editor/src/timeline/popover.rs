//! Click a key: a small card with its time, its value, how it eases into the
//! next key, and Copy / Delete. Also the key clipboard behind Ctrl+C / Ctrl+V.
//!
//! The card's fields edit the clip buffer directly, so every change lands on
//! the undo stack through the clip's snapshot observer like any other edit.
//! They resolve their key through [`PopoverState::target`] rather than a
//! captured index, because retiming a key re-sorts its lane and moves it to a
//! new index under the card.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::ui::UiTransform;

use renzora::{Interp, KeyCurve, KeyMode, PropertyKey, TrackValue};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::reactive::tracked::{bind_2way, bind_bg};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::{drag_value, screen_menu_parts, MenuAction};
use renzora_ember::UiInteraction;

use crate::AnimationEditorState;

use super::clip::{cur_clip, Lane, SelKey, SelectedKey, SelectedKeys, TimelineClip};
use super::props::{delete_selected_keys, set_key_interp, upsert_key};
use renzora_particles::asset::PinValue;

use super::snapshots::{channel_color, title_case};
use super::{MORPH, PARTICLE, PROPERTY, SELECTED};

/// The open key card, if any, and a pending request to open one.
#[derive(Resource, Default)]
pub(crate) struct PopoverState {
    /// Window position to open the card at, set by a click on a key.
    pub(super) request: Option<Vec2>,
    pub(super) target: Option<SelKey>,
    pub(super) open: bool,
}

#[derive(Component)]
pub(super) struct KeyPopoverCard;

/// The easing choices the card offers, in order: label, interpolation.
fn ease_choices() -> [(&'static str, &'static str, Interp); 6] {
    use bevy::curve::EaseFunction as E;
    [
        ("timeline.ease_linear", "Linear", Interp::Linear),
        ("timeline.ease_smooth", "Smooth", Interp::Eased(E::SmoothStep)),
        ("timeline.ease_in", "Ease in", Interp::Eased(E::QuadraticIn)),
        ("timeline.ease_out", "Ease out", Interp::Eased(E::QuadraticOut)),
        ("timeline.ease_step", "Step", Interp::Stepped),
        ("timeline.ease_bounce", "Bounce", Interp::Eased(E::BounceOut)),
    ]
}

// ── Reading and writing one key's fields ─────────────────────────────────────

/// Field labels and colours of a key's value.
fn key_fields(clip: &renzora_animation::AnimClip, k: SelKey) -> Vec<(&'static str, (u8, u8, u8))> {
    let xyz = vec![("X", channel_color(0)), ("Y", channel_color(1)), ("Z", channel_color(2))];
    match k.lane {
        Lane::Bone { .. } => xyz,
        Lane::Prop { track } => match clip.property_tracks.get(track).and_then(|p| p.keys.get(k.index)).map(|k| k.value) {
            Some(TrackValue::Vec3(_)) | Some(TrackValue::Quat(_)) => xyz,
            Some(TrackValue::Color(_)) => vec![("R", (248, 113, 113)), ("G", (110, 231, 160)), ("B", (96, 165, 250)), ("A", (200, 200, 210))],
            Some(_) => vec![("", PROPERTY)],
            None => Vec::new(),
        },
        // One weight; the unnamed field reads "Value".
        Lane::Morph { .. } => vec![("", MORPH)],
        Lane::Fx { .. } => Vec::new(),
    }
}

// An effect key's fields. Separate from the clip's because the key lives in
// the effect's copy (`TimelineClip::fx`), not in the clip.

fn fx_key(cache: &TimelineClip, k: SelKey) -> Option<&renzora_particles::params::ParamKey> {
    let Lane::Fx { track } = k.lane else { return None };
    cache.fx.as_ref()?.doc.tracks.get(track)?.keys.get(k.index)
}

fn fx_key_fields(cache: &TimelineClip, k: SelKey) -> Vec<(&'static str, (u8, u8, u8))> {
    let xyz = [("X", channel_color(0)), ("Y", channel_color(1)), ("Z", channel_color(2))];
    match fx_key(cache, k).map(|key| &key.value) {
        Some(PinValue::Vec2(_)) => xyz[..2].to_vec(),
        Some(PinValue::Vec3(_)) => xyz.to_vec(),
        Some(PinValue::Color(_)) => vec![("R", (248, 113, 113)), ("G", (110, 231, 160)), ("B", (96, 165, 250)), ("A", (200, 200, 210))],
        Some(_) => vec![("", PARTICLE)],
        None => Vec::new(),
    }
}

fn fx_key_component(cache: &TimelineClip, k: SelKey, i: usize) -> Option<f32> {
    let key = fx_key(cache, k)?;
    renzora_particles::params::pin_lanes(&key.value).map(|l| l[i.min(3)])
}

fn set_fx_key_component(cache: &mut TimelineClip, k: SelKey, i: usize, v: f32) {
    let Lane::Fx { track } = k.lane else { return };
    let Some(key) = cache.fx.as_mut().filter(|f| f.editable).and_then(|f| f.doc.tracks.get_mut(track)).and_then(|t| t.keys.get_mut(k.index)) else { return };
    match &mut key.value {
        PinValue::Float(x) => *x = v,
        PinValue::Vec2(a) => a[i.min(1)] = v,
        PinValue::Vec3(a) => a[i.min(2)] = v,
        PinValue::Color(c) => c[i.min(3)] = v,
        PinValue::Bool(b) => *b = v >= 0.5,
        PinValue::Curve(_) | PinValue::Gradient(_) | PinValue::Text(_) => {}
    }
}

/// One value field of the card's key, from the clip or the effect.
fn component(world: &World, k: SelKey, i: usize) -> Option<f32> {
    match k.lane {
        Lane::Fx { .. } => fx_key_component(world.get_resource::<TimelineClip>()?, k, i),
        _ => key_component(cur_clip(world)?, k, i),
    }
}

fn euler_deg(q: [f32; 4]) -> [f32; 3] {
    let (y, x, z) = Quat::from_array(q).to_euler(EulerRot::YXZ);
    [x.to_degrees(), y.to_degrees(), z.to_degrees()]
}

fn quat_from_deg(e: [f32; 3]) -> [f32; 4] {
    Quat::from_euler(EulerRot::YXZ, e[1].to_radians(), e[0].to_radians(), e[2].to_radians()).to_array()
}

pub(super) fn key_component(clip: &renzora_animation::AnimClip, k: SelKey, i: usize) -> Option<f32> {
    match k.lane {
        Lane::Bone { track, channel } => {
            let t = clip.tracks.get(track)?;
            match channel {
                0 => t.translations.get(k.index).map(|v| v.1[i.min(2)]),
                1 => t.rotations.get(k.index).map(|v| euler_deg(v.1)[i.min(2)]),
                _ => t.scales.get(k.index).map(|v| v.1[i.min(2)]),
            }
        }
        Lane::Prop { track } => match clip.property_tracks.get(track)?.keys.get(k.index)?.value {
            TrackValue::Float(x) => Some(x),
            TrackValue::Vec3(a) => Some(a[i.min(2)]),
            TrackValue::Quat(q) => Some(euler_deg(q)[i.min(2)]),
            TrackValue::Color(c) => Some(c[i.min(3)]),
            TrackValue::Bool(b) => Some(if b { 1.0 } else { 0.0 }),
        },
        Lane::Morph { track } => clip.morph_tracks.get(track)?.keys.get(k.index).map(|k| k.1),
        Lane::Fx { .. } => None,
    }
}

fn set_key_component(clip: &mut renzora_animation::AnimClip, k: SelKey, i: usize, v: f32) {
    match k.lane {
        Lane::Bone { track, channel } => {
            let Some(t) = clip.tracks.get_mut(track) else { return };
            match channel {
                0 => {
                    if let Some(key) = t.translations.get_mut(k.index) {
                        key.1[i.min(2)] = v;
                    }
                }
                1 => {
                    if let Some(key) = t.rotations.get_mut(k.index) {
                        let mut e = euler_deg(key.1);
                        e[i.min(2)] = v;
                        // Kept in the key's sign: a hand-set tangent is
                        // stored against it (see `renzora::KeyCurve`).
                        let q = quat_from_deg(e);
                        let dot: f32 = q.iter().zip(&key.1).map(|(a, b)| a * b).sum();
                        key.1 = if dot < 0.0 { q.map(|c| -c) } else { q };
                    }
                }
                _ => {
                    if let Some(key) = t.scales.get_mut(k.index) {
                        key.1[i.min(2)] = v;
                    }
                }
            }
        }
        Lane::Prop { track } => {
            let Some(key) = clip.property_tracks.get_mut(track).and_then(|p| p.keys.get_mut(k.index)) else { return };
            match &mut key.value {
                TrackValue::Float(x) => *x = v,
                TrackValue::Vec3(a) => a[i.min(2)] = v,
                TrackValue::Quat(q) => {
                    let mut e = euler_deg(*q);
                    e[i.min(2)] = v;
                    *q = quat_from_deg(e);
                }
                TrackValue::Color(c) => c[i.min(3)] = v,
                TrackValue::Bool(b) => *b = v >= 0.5,
            }
        }
        Lane::Morph { track } => {
            if let Some(key) = clip.morph_tracks.get_mut(track).and_then(|m| m.keys.get_mut(k.index)) {
                key.1 = v;
            }
        }
        Lane::Fx { .. } => {}
    }
}

fn target(w: &World) -> Option<SelKey> {
    w.get_resource::<PopoverState>().and_then(|p| p.target)
}

/// Retime the card's key, then follow it to wherever the re-sort put it.
fn set_target_time(world: &mut World, t: f32) {
    let Some(k) = target(world) else { return };
    let t = t.max(0.0);
    let new_index = {
        let Some(mut cache) = world.get_resource_mut::<TimelineClip>() else { return };
        let Some(mut chan) = cache.lane_times(k.lane) else { return };
        if chan.time(k.index).is_some_and(|cur| (cur - t).abs() < 1e-6) {
            return;
        }
        chan.set_time(k.index, t);
        chan.sort();
        cache.dirty = true;
        cache.key_times(k.lane).iter().position(|x| (*x - t).abs() < 1e-6)
    };
    let Some(index) = new_index else { return };
    let nk = SelKey { lane: k.lane, index };
    if let Some(mut p) = world.get_resource_mut::<PopoverState>() {
        p.target = Some(nk);
    }
    if let Some(mut s) = world.get_resource_mut::<SelectedKey>() {
        s.0 = Some(nk);
    }
    if let Some(mut s) = world.get_resource_mut::<SelectedKeys>() {
        s.set(vec![nk]);
    }
}

// ── The card ─────────────────────────────────────────────────────────────────

/// Open the card a key click asked for (closing any card already open).
pub(super) fn open_requested_popover(world: &mut World) {
    let request = world.get_resource_mut::<PopoverState>().and_then(|mut p| p.request.take());
    let still_open = world.query_filtered::<(), With<KeyPopoverCard>>().iter(world).next().is_some();
    if let Some(mut p) = world.get_resource_mut::<PopoverState>() {
        if p.open != still_open {
            p.open = still_open;
        }
    }
    let Some(at) = request else { return };
    let Some(k) = world.get_resource::<SelectedKey>().and_then(|s| s.0) else { return };
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
    let Some(cache) = world.get_resource::<TimelineClip>() else { return };
    let Some(clip) = cache.clip.as_ref() else { return };
    let is_fx = matches!(k.lane, Lane::Fx { .. });
    // An effect key eases (a parameter) or does not (an on/off or burst key,
    // whose value is an event, not a curve).
    let fx_eases = is_fx
        && matches!(k.lane, Lane::Fx { track } if cache.fx.as_ref().and_then(|f| f.doc.tracks.get(track)).is_some_and(|t| matches!(t.target, renzora_particles::params::ParamTarget::Pin { .. })));
    let (title, fields) = if is_fx {
        let label = match k.lane {
            Lane::Fx { track } => cache.fx.as_ref().and_then(|f| f.doc.tracks.get(track).map(|t| renzora_particles::params::track_label(&f.doc, t))).unwrap_or_default(),
            _ => String::new(),
        };
        (format!("{label} {}", renzora::lang::t_or("timeline.key_suffix", "key")), fx_key_fields(cache, k))
    } else {
        (key_title(clip, k), key_fields(clip, k))
    };
    let is_prop = matches!(k.lane, Lane::Prop { .. }) || fx_eases;
    let is_bool = matches!(k.lane, Lane::Prop { track } if clip.property_tracks.get(track).and_then(|p| p.keys.get(k.index)).is_some_and(|key| matches!(key.value, TrackValue::Bool(_))))
        || fx_key(cache, k).is_some_and(|key| matches!(key.value, PinValue::Bool(_)));

    let old: Vec<Entity> = world.query_filtered::<Entity, With<KeyPopoverCard>>().iter(world).collect();
    for e in old {
        if let Ok(em) = world.get_entity_mut(e) {
            em.despawn();
        }
    }
    if let Some(mut p) = world.get_resource_mut::<PopoverState>() {
        p.target = Some(k);
        p.open = true;
    }

    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        let (content, card) = screen_menu_parts(&mut commands, at.x + 12.0, at.y + 12.0);
        commands.entity(card).insert(KeyPopoverCard);
        commands.entity(card).entry::<Node>().and_modify(|mut n| {
            n.min_width = Val::Px(360.0);
            n.padding = UiRect::all(Val::Px(12.0));
            n.border_radius = BorderRadius::all(Val::Px(10.0));
        });
        commands.entity(content).entry::<Node>().and_modify(|mut n| n.row_gap = Val::Px(10.0));

        // Title.
        let head = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).id();
        let gem = commands
            .spawn((Node { width: Val::Px(9.0), height: Val::Px(9.0), border_radius: BorderRadius::all(Val::Px(2.0)), ..default() }, BackgroundColor(rgb(SELECTED)), UiTransform::from_rotation(Rot2::degrees(45.0))))
            .id();
        let t = commands.spawn((Text::new(title), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary())), bevy::text::TextLayout::no_wrap(), Node { flex_grow: 1.0, ..default() })).id();
        let esc = commands.spawn((Text::new(renzora::lang::t_or("timeline.esc_to_close", "Esc to close")), ui_font(&fonts.ui, 11.0), TextColor(rgb((138, 138, 150))))).id();
        commands.entity(head).add_children(&[gem, t, esc]);

        // Time + value fields.
        let fields_row = commands.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), width: Val::Percent(100.0), ..default() }).id();
        let time_dv = drag_value(&mut commands, &fonts.mono, "", text_primary(), 0.0, 1.0 / 60.0);
        commands.entity(time_dv).entry::<Node>().and_modify(|mut n| {
            n.width = Val::Px(96.0);
            n.flex_shrink = 0.0;
        });
        bind_2way(
            &mut commands,
            time_dv,
            |w: &Rx| {
                let world = w.untracked();
                target(world)
                    .and_then(|k| world.get_resource::<TimelineClip>().and_then(|c| c.key_times(k.lane).get(k.index).copied()))
                    .unwrap_or(0.0)
            },
            |w: &mut World, v: &f32| set_target_time(w, *v),
        );
        let time_lbl = commands.spawn((Text::new(renzora::lang::t_or("timeline.time_s", "Time (s)")), ui_font(&fonts.ui, 11.0), TextColor(rgb((138, 138, 150))))).id();
        let time_col = commands.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() }).id();
        commands.entity(time_col).add_children(&[time_lbl, time_dv]);
        commands.entity(fields_row).add_child(time_col);
        for (i, (label, color)) in fields.iter().enumerate() {
            let step = if is_bool { 1.0 } else if matches!(k.lane, Lane::Bone { channel: 1, .. }) { 0.5 } else { 0.01 };
            let dv = drag_value(&mut commands, &fonts.mono, label, *color, 0.0, step);
            if is_bool {
                commands.entity(dv).insert((renzora_ember::widgets::DragRange { min: 0.0, max: 1.0 }, renzora_ember::widgets::DragSnap(1.0)));
            }
            commands.entity(dv).entry::<Node>().and_modify(|mut n| {
                n.flex_grow = 1.0;
                n.flex_basis = Val::Px(0.0);
                n.min_width = Val::Px(0.0);
            });
            bind_2way(
                &mut commands,
                dv,
                move |w: &Rx| {
                    let world = w.untracked();
                    target(world).and_then(|k| component(world, k, i)).unwrap_or(0.0)
                },
                move |w: &mut World, v: &f32| {
                    let Some(k) = target(w) else { return };
                    if let Some(mut cache) = w.get_resource_mut::<TimelineClip>() {
                        if matches!(k.lane, Lane::Fx { .. }) {
                            // The effect's copy: `fx::sync_fx` writes it on.
                            set_fx_key_component(&mut cache, k, i, *v);
                        } else {
                            if let Some(clip) = cache.clip.as_mut() {
                                set_key_component(clip, k, i, *v);
                            }
                            cache.dirty = true;
                        }
                    }
                },
            );
            let col = commands.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), flex_grow: 1.0, flex_basis: Val::Px(0.0), min_width: Val::Px(0.0), ..default() }).id();
            let lbl = commands.spawn((Text::new(if label.is_empty() { renzora::lang::t_or("timeline.value", "Value") } else { String::new() }), ui_font(&fonts.ui, 11.0), TextColor(rgb((138, 138, 150))))).id();
            commands.entity(col).add_children(&[lbl, dv]);
            commands.entity(fields_row).add_child(col);
        }
        let mut kids = vec![head, fields_row];

        // Easing (property keys only: bone channels interpolate linearly).
        if is_prop && !is_bool {
            let ease_lbl = commands.spawn((Text::new(renzora::lang::t_or("timeline.ease_into_next", "Ease into the next key")), ui_font(&fonts.ui, 11.0), TextColor(rgb((154, 154, 166))))).id();
            let row = commands.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(2.0), ..default() }).id();
            // An effect parameter samples only linear, smooth and step.
            for (key, fallback, interp) in ease_choices().into_iter().filter(|c| !is_fx || super::fx::interp_from_clip(c.2).is_some()) {
                let b = ease_button(&mut commands, &fonts, &renzora::lang::t_or(key, fallback), interp);
                commands.entity(row).add_child(b);
            }
            kids.extend([ease_lbl, row]);
        } else if !is_prop && !is_fx {
            // A bone key's mode: how it moves on to the next key.
            let mode_lbl = commands.spawn((Text::new(renzora::lang::t_or("timeline.key_into_next", "Into the next key")), ui_font(&fonts.ui, 11.0), TextColor(rgb((154, 154, 166))))).id();
            let row = commands.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(2.0), ..default() }).id();
            for (mode, _, tip) in super::keymode::mode_choices() {
                let b = mode_button(&mut commands, &fonts, mode, tip);
                commands.entity(row).add_child(b);
            }
            kids.extend([mode_lbl, row]);
        }

        // Copy / Delete.
        let actions = commands.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(6.0), justify_content: JustifyContent::FlexEnd, width: Val::Percent(100.0), ..default() }).id();
        let copy = card_button(&mut commands, &fonts, &renzora::lang::t_or("timeline.copy", "Copy"), rgb(hover_bg()), text_primary(), copy_selected_keys);
        let delete = card_button(&mut commands, &fonts, &renzora::lang::t_or("timeline.delete", "Delete"), Color::srgba(0.97, 0.44, 0.44, 0.14), (248, 161, 161), delete_selected_keys);
        commands.entity(actions).add_children(&[copy, delete]);
        kids.push(actions);
        commands.entity(content).add_children(&kids);
    }
    queue.apply(world);
}

fn key_title(clip: &renzora_animation::AnimClip, k: SelKey) -> String {
    let key = renzora::lang::t_or("timeline.key_suffix", "key");
    match k.lane {
        Lane::Bone { track, channel } => {
            let bone = clip.tracks.get(track).map(|t| t.bone_name.as_str()).unwrap_or("");
            let ch = match channel {
                0 => renzora::lang::t_or("timeline.position", "Position"),
                1 => renzora::lang::t_or("timeline.rotation", "Rotation"),
                _ => renzora::lang::t_or("timeline.scale", "Scale"),
            };
            format!("{bone} · {ch} {key}")
        }
        Lane::Prop { track } => {
            let field = clip.property_tracks.get(track).map(|p| title_case(&p.field)).unwrap_or_default();
            format!("{field} {key}")
        }
        Lane::Morph { track } => {
            let shape = clip.morph_tracks.get(track).map(|m| m.label()).unwrap_or_default();
            format!("{shape} · {} {key}", renzora::lang::t_or("timeline.blend_shape", "Blend shape"))
        }
        Lane::Fx { .. } => key,
    }
}

fn card_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    bg: Color,
    fg: (u8, u8, u8),
    action: fn(&mut World),
) -> Entity {
    let b = commands
        .spawn((Node { height: Val::Px(26.0), padding: UiRect::horizontal(Val::Px(10.0)), align_items: AlignItems::Center, border_radius: BorderRadius::all(Val::Px(6.0)), ..default() }, BackgroundColor(bg), UiInteraction::default(), MenuAction(Box::new(action))))
        .id();
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 12.0), TextColor(rgb(fg)), PASS_THROUGH)).id();
    commands.entity(b).add_child(t);
    b
}

/// One easing choice: a tiny drawing of its curve over its name. The one the
/// key uses is highlighted. Pressing it applies to every selected property key.
fn ease_button(commands: &mut Commands, fonts: &EmberFonts, label: &str, interp: Interp) -> Entity {
    let b = commands
        .spawn((
            Node { width: Val::Px(56.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(2.0), Val::Px(6.0)), border_radius: BorderRadius::all(Val::Px(7.0)), ..default() },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
        ))
        .id();
    bind_bg(commands, b, move |w| {
        let world = w.untracked();
        let current = target(world).and_then(|k| match k.lane {
            Lane::Prop { track } => cur_clip(world).and_then(|c| c.property_tracks.get(track)).and_then(|p| p.keys.get(k.index)).map(|key| key.interp),
            Lane::Fx { .. } => world.get_resource::<TimelineClip>().and_then(|c| fx_key(c, k)).map(|key| super::fx::interp_to_clip(key.interp)),
            _ => None,
        });
        if current == Some(interp) {
            rgb(accent()).with_alpha(0.22)
        } else {
            match world.get::<UiInteraction>(b) {
                Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(hover_bg()),
                _ => Color::NONE,
            }
        }
    });
    let icon = ease_icon(commands, interp, (154, 154, 166));
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 10.0), TextColor(rgb((196, 196, 206))), bevy::text::TextLayout::no_wrap(), PASS_THROUGH)).id();
    commands.entity(b).add_children(&[icon, t]);
    commands.entity(b).insert(EaseBtn(interp));
    b
}

#[derive(Component)]
pub(super) struct EaseBtn(pub(super) Interp);

/// A bone key mode in the key card: a drawing of the shape it gives over its
/// name, the key's own highlighted. Applies to every selected key.
fn mode_button(commands: &mut Commands, fonts: &EmberFonts, mode: KeyMode, tip: &str) -> Entity {
    let b = commands
        .spawn((
            Node { width: Val::Px(54.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(4.0), padding: UiRect::axes(Val::Px(2.0), Val::Px(6.0)), border_radius: BorderRadius::all(Val::Px(7.0)), ..default() },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            renzora_ember::widgets::HoverTooltip::new(super::keymode::mode_label_tr(mode, tip)),
            super::curves::KeyModeBtn(mode),
        ))
        .id();
    bind_bg(commands, b, move |w| {
        let world = w.untracked();
        let current = target(world).and_then(|k| cur_clip(world).and_then(|c| super::keymode::key_mode(c, k)));
        if current == Some(mode) {
            rgb(accent()).with_alpha(0.22)
        } else {
            match world.get::<UiInteraction>(b) {
                Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(hover_bg()),
                _ => Color::NONE,
            }
        }
    });
    let icon = mode_icon(commands, mode, (154, 154, 166));
    let t = commands.spawn((Text::new(super::keymode::mode_label_tr(mode, mode.label())), ui_font(&fonts.ui, 10.0), TextColor(rgb((196, 196, 206))), bevy::text::TextLayout::no_wrap(), PASS_THROUGH)).id();
    commands.entity(b).add_children(&[icon, t]);
    b
}

/// A 34x20 drawing of the shape a bone key mode gives a rise between two
/// keys, sampled from the same spline playback uses so the picture is honest.
fn mode_icon(commands: &mut Commands, mode: KeyMode, color: (u8, u8, u8)) -> Entity {
    let (w, h) = (34.0f32, 20.0f32);
    let root = commands.spawn((Node { width: Val::Px(w), height: Val::Px(h), position_type: PositionType::Relative, ..default() }, PASS_THROUGH)).id();
    // Three rising keys, so Auto (which keeps moving through the middle key)
    // and Flat (which stops at it) read apart.
    let tan = |mode: KeyMode, i: f32, o: f32| KeyCurve { mode, in_tan: [i, 0.0, 0.0, 0.0], out_tan: [o, 0.0, 0.0, 0.0] };
    let curves = match mode {
        // A steep exit from the first key, the same slope on both sides.
        KeyMode::Free => [tan(KeyMode::Free, 4.0, 4.0), KeyCurve::AUTO, KeyCurve::AUTO],
        // A corner at the middle key: arriving steep, leaving downhill.
        KeyMode::Broken => [KeyCurve::AUTO, tan(KeyMode::Broken, 3.0, -1.0), KeyCurve::AUTO],
        m => [KeyCurve::new(m); 3],
    };
    let keys = [(0.0, Vec4::ZERO), (0.5, Vec4::X * 0.55), (1.0, Vec4::X)];
    let spline = renzora::ChannelSpline::new(&keys, &curves, false);
    // Fine enough that a hold's riser draws as a near-vertical step.
    let n = 24;
    let pt = |i: usize| {
        let f = i as f32 / n as f32;
        Vec2::new(2.0 + f * (w - 4.0), h - 2.0 - spline.sample(f).x.clamp(-0.3, 1.3) * (h - 4.0))
    };
    let segs: Vec<Entity> = (0..n).map(|i| segment(commands, pt(i), pt(i + 1), 1.6, rgb(color))).collect();
    commands.entity(root).add_children(&segs);
    root
}

/// A 34x20 drawing of an easing curve, as short rotated segments.
pub(super) fn ease_icon(commands: &mut Commands, interp: Interp, color: (u8, u8, u8)) -> Entity {
    let (w, h) = (34.0f32, 20.0f32);
    let root = commands.spawn((Node { width: Val::Px(w), height: Val::Px(h), position_type: PositionType::Relative, ..default() }, PASS_THROUGH)).id();
    let n = 10;
    let pt = |i: usize| {
        let f = i as f32 / n as f32;
        let v = match interp {
            Interp::Stepped => {
                if f < 0.5 { 0.0 } else { 1.0 }
            }
            other => other.ease(f),
        };
        Vec2::new(2.0 + f * (w - 4.0), h - 2.0 - v.clamp(-0.2, 1.2) * (h - 4.0))
    };
    let mut segs = Vec::new();
    for i in 0..n {
        let (a, b) = (pt(i), pt(i + 1));
        segs.push(segment(commands, a, b, 1.6, rgb(color)));
    }
    if matches!(interp, Interp::Stepped) {
        // The step's riser.
        let x = 2.0 + 0.5 * (w - 4.0);
        segs.push(segment(commands, Vec2::new(x, h - 2.0), Vec2::new(x, 2.0), 1.6, rgb(color)));
    }
    commands.entity(root).add_children(&segs);
    root
}

/// A straight line from `a` to `b` as one rotated node.
pub(super) fn segment(commands: &mut Commands, a: Vec2, b: Vec2, thick: f32, color: Color) -> Entity {
    let d = b - a;
    let len = d.length().max(0.5);
    let mid = (a + b) * 0.5;
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(mid.x - len * 0.5),
                top: Val::Px(mid.y - thick * 0.5),
                width: Val::Px(len),
                height: Val::Px(thick),
                border_radius: BorderRadius::all(Val::Px(thick * 0.5)),
                ..default()
            },
            BackgroundColor(color),
            UiTransform::from_rotation(Rot2::radians(d.y.atan2(d.x))),
            PASS_THROUGH,
        ))
        .id()
}

/// Escape closes the key card; and pressing an easing choice applies it.
pub(super) fn popover_escape(
    keys: Res<ButtonInput<KeyCode>>,
    cards: Query<Entity, With<KeyPopoverCard>>,
    eases: Query<(&UiInteraction, &EaseBtn), Changed<UiInteraction>>,
    mut state: ResMut<PopoverState>,
    mut commands: Commands,
) {
    if keys.just_pressed(KeyCode::Escape) {
        for e in &cards {
            commands.entity(e).despawn();
        }
        state.open = false;
    }
    for (i, b) in &eases {
        if *i == UiInteraction::Pressed {
            let interp = b.0;
            commands.queue(move |w: &mut World| apply_interp_to_selection(w, interp));
        }
    }
}

/// Set the interpolation of every selected property key (bone keys have none).
pub(super) fn apply_interp_to_selection(world: &mut World, interp: Interp) {
    let mut keys: Vec<SelKey> = world.get_resource::<SelectedKeys>().map(|s| s.keys.clone()).unwrap_or_default();
    if let Some(p) = world.get_resource::<SelectedKey>().and_then(|s| s.0) {
        if !keys.contains(&p) {
            keys.push(p);
        }
    }
    for k in keys {
        match k.lane {
            Lane::Prop { track } => set_key_interp(world, track, k.index, interp),
            Lane::Fx { track } => {
                if let (Some(fx_interp), Some(mut cache)) = (super::fx::interp_from_clip(interp), world.get_resource_mut::<TimelineClip>()) {
                    super::fx::set_key_interp(&mut cache, track, k.index, fx_interp);
                }
            }
            Lane::Bone { .. } | Lane::Morph { .. } => {}
        }
    }
}

// ── Clipboard ────────────────────────────────────────────────────────────────

#[derive(Clone)]
enum LaneId {
    Bone(String, u8),
    Prop(String, String),
    /// A blend shape: its mesh node and target index.
    Morph(String, usize),
}

/// A copied key: a bone or blend shape key's value with its curve (so a
/// pasted key eases as the copied one did), or a whole property key.
#[derive(Clone)]
enum Payload {
    V3([f32; 3], KeyCurve),
    Q([f32; 4], KeyCurve),
    W(f32, KeyCurve),
    P(PropertyKey),
}

/// Copied keys, as offsets from the earliest one, addressed by bone or
/// property name so they paste into another clip with the same tracks.
#[derive(Resource, Default)]
pub(crate) struct KeyClipboard {
    entries: Vec<(LaneId, f32, Payload)>,
}

pub(super) fn copy_selected_keys(world: &mut World) {
    let mut keys: Vec<SelKey> = world.get_resource::<SelectedKeys>().map(|s| s.keys.clone()).unwrap_or_default();
    if let Some(p) = world.get_resource::<SelectedKey>().and_then(|s| s.0) {
        if !keys.contains(&p) {
            keys.push(p);
        }
    }
    let Some(clip) = cur_clip(world) else { return };
    let mut raw: Vec<(LaneId, f32, Payload)> = Vec::new();
    for k in keys {
        let item = match k.lane {
            Lane::Bone { track, channel } => clip.tracks.get(track).and_then(|t| {
                let id = LaneId::Bone(t.bone_name.clone(), channel);
                let c = t.key_curve(channel, k.index);
                match channel {
                    0 => t.translations.get(k.index).map(|(tm, v)| (id, *tm, Payload::V3(*v, c))),
                    1 => t.rotations.get(k.index).map(|(tm, v)| (id, *tm, Payload::Q(*v, c))),
                    _ => t.scales.get(k.index).map(|(tm, v)| (id, *tm, Payload::V3(*v, c))),
                }
            }),
            Lane::Prop { track } => clip.property_tracks.get(track).and_then(|p| {
                p.keys.get(k.index).map(|key| (LaneId::Prop(p.component.clone(), p.field.clone()), key.time, Payload::P(key.clone())))
            }),
            Lane::Morph { track } => clip.morph_tracks.get(track).and_then(|m| {
                m.keys.get(k.index).map(|(t, w)| (LaneId::Morph(m.target.clone(), m.index), *t, Payload::W(*w, m.key_curve(k.index))))
            }),
            // The clipboard pastes into clips by track name; an effect's
            // keys have no such name to paste back under.
            Lane::Fx { .. } => None,
        };
        if let Some(i) = item {
            raw.push(i);
        }
    }
    if raw.is_empty() {
        return;
    }
    let t0 = raw.iter().map(|(_, t, _)| *t).fold(f32::INFINITY, f32::min);
    let entries = raw.into_iter().map(|(l, t, p)| (l, t - t0, p)).collect();
    world.insert_resource(KeyClipboard { entries });
}

pub(super) fn paste_keys(world: &mut World) {
    let entries = match world.get_resource::<KeyClipboard>() {
        Some(c) if !c.entries.is_empty() => c.entries.clone(),
        _ => return,
    };
    let at = world.get_resource::<AnimationEditorState>().map(|s| s.scrub_time).unwrap_or(0.0);
    let Some(mut cache) = world.get_resource_mut::<TimelineClip>() else { return };
    let Some(clip) = cache.clip.as_mut() else { return };
    let mut any = false;
    for (lane, dt, payload) in entries {
        let t = at + dt;
        match (lane, payload) {
            (LaneId::Bone(name, ch), p) => {
                let Some(track) = clip.tracks.iter_mut().find(|tr| tr.bone_name == name) else { continue };
                let (value, curve) = match (ch, p) {
                    (1, Payload::Q(v, c)) => (Vec4::from_array(v), c),
                    (0 | 2, Payload::V3(v, c)) => (Vec3::from_array(v).extend(0.0), c),
                    _ => continue,
                };
                let i = track.upsert_key(ch, t, value, curve, 1e-4);
                // Pasted over an existing key, the copied curve wins too.
                track.set_key_curve(ch, i, curve);
                any = true;
            }
            (LaneId::Prop(component, field), Payload::P(key)) => {
                let Some(pt) = clip.property_tracks.iter_mut().find(|p| p.component == component && p.field == field) else { continue };
                upsert_key(pt, t, key.value);
                if let Some(k) = pt.keys.iter_mut().find(|k| (k.time - t).abs() < 1e-4) {
                    k.interp = key.interp;
                }
                any = true;
            }
            (LaneId::Morph(node, index), Payload::W(w, curve)) => {
                let node = renzora::sanitize_id(&node);
                let Some(m) = clip.morph_tracks.iter_mut().find(|m| m.index == index && renzora::sanitize_id(&m.target) == node) else { continue };
                let i = m.upsert_key(t, w, curve, 1e-4);
                m.set_key_curve(i, curve);
                any = true;
            }
            _ => {}
        }
        if t > clip.duration {
            clip.duration = t;
        }
    }
    if any {
        cache.dirty = true;
    }
}
