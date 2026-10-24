//! Animate mode's Controls tab: a model's own sliders (`super::controls`),
//! making them from the pose on the stage, editing them, and keying them; the
//! model's raw blend shapes sit underneath (`face_panel::blend_shapes`).
//!
//! This is where the Face tab's fixed expression list went. A face rig with
//! no controls file yet is offered the expressions as a preset, written as
//! ordinary controls it can then rename, re-capture or delete.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_2way, bind_display, bind_text, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{accent_button, bind_text_input, button, section, spin_slider, text_input};

use super::controls::{self, ControlsState};
use super::on_click;

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

fn note(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((Text::new(text), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), Node { margin: UiRect::vertical(Val::Px(2.0)), ..default() }))
        .id()
}

fn column(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() })
        .id()
}

fn row(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id()
}

fn action(commands: &mut Commands, fonts: &EmberFonts, label: &str, f: impl Fn(&mut World) + Send + Sync + 'static) -> Entity {
    let b = button(commands, &fonts.ui, label);
    on_click(commands, b, f);
    b
}

fn has_model(w: &Rx) -> bool {
    w.get_resource::<ControlsState>().is_some_and(|s| s.path.is_some())
}

/// Whether to offer the face preset: a face rig with no controls file yet.
fn offer_preset(w: &Rx) -> bool {
    let fresh = w.get_resource::<ControlsState>().is_some_and(|s| s.path.is_some() && !s.on_disk && s.set.controls.is_empty());
    fresh && super::face_panel::rigged(w.untracked())
}

/// The Controls tab.
pub(crate) fn controls_tab(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = column(commands);
    let none = note(commands, fonts, "Put a model on the stage to make controls for it.");
    bind_display(commands, none, |w| !has_model(w));

    let (made, made_body) = section(commands, fonts, "sliders-horizontal", "Controls", accent());
    bind_display(commands, made, has_model);
    let hint = note(
        commands,
        fonts,
        "Pose bones on the stage and set blend shapes below, name the pose, then Save as Control. Its slider dials that pose in from rest; drag several to mix them, then Key Controls (or K) keys them at the playhead.",
    );
    let save_row = row(commands);
    let name = text_input(commands, &fonts.ui, "Control name", "");
    commands.entity(name).entry::<Node>().and_modify(|mut n| n.flex_grow = 1.0);
    bind_text_input(
        commands,
        name,
        |w: &Rx| w.get_resource::<ControlsState>().map(|s| s.name.clone()).unwrap_or_default(),
        |w: &mut World, v: String| w.resource_mut::<ControlsState>().name = v,
    );
    let save = accent_button(commands, fonts, accent(), "Save as Control");
    on_click(commands, save, controls::save_new);
    commands.entity(save_row).add_children(&[name, save]);

    let preset = column(commands);
    bind_display(commands, preset, offer_preset);
    let preset_hint = note(commands, fonts, "This head has a face rig. Start from its built-in expressions (jaw, blinks, smile, brows, looks), written as controls you can change.");
    let preset_b = action(commands, fonts, "Use Face Preset", controls::use_face_preset);
    commands.entity(preset).add_children(&[preset_hint, preset_b]);

    let empty = note(commands, fonts, "No controls yet.");
    bind_display(commands, empty, |w| w.get_resource::<ControlsState>().is_some_and(|s| s.set.controls.is_empty()));
    let list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() }).id();
    keyed_list(commands, list, controls_snapshot);

    let keys = row(commands);
    let key = action(commands, fonts, "Key Controls", controls::key_controls);
    let reset = action(commands, fonts, "Reset Controls", controls::reset);
    commands.entity(keys).add_children(&[key, reset]);
    bind_display(commands, keys, |w| w.get_resource::<ControlsState>().is_some_and(|s| !s.set.controls.is_empty()));

    let edit = edit_section(commands, fonts);
    commands.entity(made_body).add_children(&[hint, save_row, preset, empty, list, keys, edit]);

    let shapes = super::face_panel::blend_shapes(commands, fonts);
    commands.entity(col).add_children(&[none, made, shapes]);
    col
}

/// One row per control: its slider, and Edit.
///
/// Keyed by name and hashed on the range only, never the value: dragging a
/// slider must not rebuild the row it is being dragged in.
fn controls_snapshot(w: &Rx) -> KeyedSnapshot {
    let rows: Vec<(String, f32, f32, f32)> = w
        .get_resource::<ControlsState>()
        .map(|s| s.set.controls.iter().map(|c| (c.name.clone(), c.min.min(c.max), c.max.max(c.min), c.default)).collect())
        .unwrap_or_default();
    let items = rows
        .iter()
        .map(|(n, min, max, default)| {
            let mut k = hasher();
            n.hash(&mut k);
            let mut h = hasher();
            (min.to_bits(), max.to_bits(), default.to_bits()).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (name, min, max, rest_value) = rows[i].clone();
            let r = c
                .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), ..default() })
                .id();
            let s = spin_slider(c, &f.ui, &name, rest_value, min, max);
            c.entity(s).entry::<Node>().and_modify(|mut n| n.flex_grow = 1.0);
            let get_name = name.clone();
            let set_name = name.clone();
            bind_2way(
                c,
                s,
                move |w: &Rx| w.get_resource::<ControlsState>().and_then(|s| s.set.find(&get_name).map(|ctl| s.value(ctl))).unwrap_or(0.0),
                move |w: &mut World, v: &f32| controls::set_value(w, &set_name, *v),
            );
            let edit = button(c, &f.ui, "Edit");
            on_click(c, edit, move |w| {
                let mut s = w.resource_mut::<ControlsState>();
                if s.editing.as_deref() == Some(name.as_str()) {
                    s.editing = None;
                } else {
                    s.editing = Some(name.clone());
                    s.rename = name.clone();
                }
            });
            c.entity(r).add_children(&[s, edit]);
            r
        }),
    }
}

/// A range slider bound to the edited control's `min` or `max`.
fn range_slider(commands: &mut Commands, fonts: &EmberFonts, label: &str, lo: f32, hi: f32, is_max: bool) -> Entity {
    let s = spin_slider(commands, &fonts.ui, label, 0.0, lo, hi);
    commands.entity(s).entry::<Node>().and_modify(|mut n| n.width = Val::Percent(100.0));
    bind_2way(
        commands,
        s,
        move |w: &Rx| w.get_resource::<ControlsState>().and_then(|s| s.editing_control().map(|c| if is_max { c.max } else { c.min })).unwrap_or(0.0),
        move |w: &mut World, v: &f32| {
            if is_max {
                controls::set_range(w, None, Some(*v));
            } else {
                controls::set_range(w, Some(*v), None);
            }
        },
    );
    s
}

/// The edit section for the control whose Edit was pressed.
fn edit_section(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (s, body) = section(commands, fonts, "pencil-simple", "Edit Control", accent());
    bind_display(commands, s, |w| w.get_resource::<ControlsState>().is_some_and(|s| s.editing_control().is_some()));
    let what = note(commands, fonts, "");
    bind_text(commands, what, |w| {
        let Some(c) = w.get_resource::<ControlsState>().and_then(|s| s.editing_control().cloned()) else { return String::new() };
        let count = |ts: &[renzora_rig::controls::ControlTarget]| {
            let bones = ts.iter().filter(|t| matches!(t, renzora_rig::controls::ControlTarget::Bone { .. })).count();
            format!("{bones} bones, {} blend shapes", ts.len() - bones)
        };
        let neg = if !c.signed() {
            "No negative end: set Min below zero, or capture one.".to_string()
        } else if c.negative.is_empty() {
            "Negative end: the pose reversed.".to_string()
        } else {
            format!("Negative end: its own pose, {}.", count(&c.negative))
        };
        format!("{}: {}. {neg}", c.name, count(&c.targets))
    });
    let rename_row = row(commands);
    let field = text_input(commands, &fonts.ui, "Name", "");
    commands.entity(field).entry::<Node>().and_modify(|mut n| n.flex_grow = 1.0);
    bind_text_input(
        commands,
        field,
        |w: &Rx| w.get_resource::<ControlsState>().map(|s| s.rename.clone()).unwrap_or_default(),
        |w: &mut World, v: String| w.resource_mut::<ControlsState>().rename = v,
    );
    let rename = action(commands, fonts, "Rename", controls::rename);
    commands.entity(rename_row).add_children(&[field, rename]);
    let min = range_slider(commands, fonts, "Min", -2.0, 0.0, false);
    let max = range_slider(commands, fonts, "Max", 0.05, 2.0, true);
    let capture_hint = note(commands, fonts, "Re-capture replaces the pose with the one on the stage. A negative end of its own makes the slider two poses, one each side of zero.");
    let captures = row(commands);
    let recapture = action(commands, fonts, "Re-capture", |w| controls::recapture(w, false));
    let negative = action(commands, fonts, "Capture Negative End", |w| controls::recapture(w, true));
    let mirror = action(commands, fonts, "Clear Negative End", controls::clear_negative);
    commands.entity(captures).add_children(&[recapture, negative, mirror]);
    let last = row(commands);
    let delete = action(commands, fonts, "Delete", controls::delete);
    let done = action(commands, fonts, "Done", |w| w.resource_mut::<ControlsState>().editing = None);
    commands.entity(last).add_children(&[delete, done]);
    commands.entity(body).add_children(&[what, rename_row, min, max, capture_hint, captures, last]);
    s
}
