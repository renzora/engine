//! Shapes mode's two panel columns: the shape list and the brush on the left
//! (`anim_tools`), the shape being sculpted and Save on the right
//! (`anim_inspector`). Laid out like Skin's, which `panels` arranges them
//! beside.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_2way, bind_display, bind_text, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{button, icon_button, section, segmented, spin_slider, toggle_switch};

use renzora_rig::sculpt::Brush;
use renzora_rig::weights::Falloff;

use super::shapes::{self, ShapeEdit};
use super::{StageMode, StageUi};
use crate::rig::on_click;

fn column(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() })
        .id()
}

fn row(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node { width: Val::Percent(100.0), flex_wrap: FlexWrap::Wrap, align_items: AlignItems::Center, column_gap: Val::Px(6.0), row_gap: Val::Px(6.0), ..default() })
        .id()
}

fn note(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands.spawn((Text::new(text), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id()
}

fn titled(commands: &mut Commands, fonts: &EmberFonts, icon: &str, title: &str, body: Entity) -> Entity {
    let (s, inner) = section(commands, fonts, icon, title, accent());
    commands.entity(inner).add_child(body);
    s
}

fn action(commands: &mut Commands, fonts: &EmberFonts, label: &str, f: impl Fn(&mut World) + Send + Sync + 'static) -> Entity {
    let b = button(commands, &fonts.ui, label);
    on_click(commands, b, f);
    b
}

fn ready(w: &Rx) -> bool {
    w.get_resource::<ShapeEdit>().is_some_and(|s| s.active() && s.has_meshes())
}

fn slider(commands: &mut Commands, fonts: &EmberFonts, label: &str, min: f32, max: f32, get: fn(&ShapeEdit) -> f32, set: fn(&mut ShapeEdit, f32)) -> Entity {
    let s = spin_slider(commands, &fonts.ui, label, 0.0, min, max);
    commands.entity(s).entry::<Node>().and_modify(|mut n| n.width = Val::Percent(100.0));
    bind_2way(
        commands,
        s,
        move |w: &Rx| w.get_resource::<ShapeEdit>().map(get).unwrap_or(0.0),
        move |w: &mut World, v: &f32| set(&mut w.resource_mut::<ShapeEdit>(), *v),
    );
    s
}

fn choice(commands: &mut Commands, fonts: &EmberFonts, labels: &[&str], get: fn(&ShapeEdit) -> usize, set: fn(&mut ShapeEdit, usize)) -> Entity {
    let s = segmented(commands, &fonts.ui, labels, 0);
    commands.entity(s).entry::<Node>().and_modify(|mut n| n.width = Val::Percent(100.0));
    bind_2way(
        commands,
        s,
        move |w: &Rx| w.get_resource::<ShapeEdit>().map(get).unwrap_or(0),
        move |w: &mut World, i: &usize| set(&mut w.resource_mut::<ShapeEdit>(), *i),
    );
    s
}

fn switch(commands: &mut Commands, fonts: &EmberFonts, label: &str, get: fn(&ShapeEdit) -> bool, set: fn(&mut ShapeEdit, bool)) -> Entity {
    let r = row(commands);
    let l = commands.spawn((Text::new(label), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())), Node { flex_grow: 1.0, ..default() })).id();
    let t = toggle_switch(commands, false);
    bind_2way(
        commands,
        t,
        move |w: &Rx| w.get_resource::<ShapeEdit>().is_some_and(get),
        move |w: &mut World, on: &bool| set(&mut w.resource_mut::<ShapeEdit>(), *on),
    );
    commands.entity(r).add_children(&[l, t]);
    r
}

// ── Left ─────────────────────────────────────────────────────────────────────

pub(super) fn tools(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = column(commands);
    let none = note(commands, fonts, "Put a model on the stage to sculpt blend shapes on it.");
    bind_display(commands, none, |w| !ready(w));
    let body = column(commands);
    bind_display(commands, body, ready);

    let list_col = column(commands);
    let hint = note(commands, fonts, "Pick a shape to sculpt it, or make a new one. The sliders only preview it here; key it in Animate's Controls tab once saved.");
    let list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).id();
    keyed_list(commands, list, shape_snapshot);
    let empty = note(commands, fonts, "No blend shapes yet.");
    bind_display(commands, empty, |w| w.get_resource::<ShapeEdit>().is_some_and(|s| s.shapes.is_empty()));
    let r = row(commands);
    let name = renzora_ember::widgets::text_input(commands, &fonts.ui, "Shape name", "");
    commands.entity(name).entry::<Node>().and_modify(|mut n| n.flex_grow = 1.0);
    renzora_ember::widgets::bind_text_input(
        commands,
        name,
        |w: &Rx| w.get_resource::<ShapeEdit>().map(|s| s.name.clone()).unwrap_or_default(),
        |w: &mut World, v: String| w.resource_mut::<ShapeEdit>().name = v,
    );
    let new = action(commands, fonts, "New Shape", shapes::new_shape);
    commands.entity(r).add_children(&[name, new]);
    let rest = action(commands, fonts, "Preview at Rest", shapes::rest_weights);
    commands.entity(list_col).add_children(&[hint, list, empty, r, rest]);
    let list_s = titled(commands, fonts, "drop-half", "Blend Shapes", list_col);

    let brush = column(commands);
    let labels: Vec<&str> = Brush::ALL.iter().map(|b| b.label()).collect();
    let kind = choice(
        commands,
        fonts,
        &labels[..3],
        |s| Brush::ALL.iter().position(|b| *b == s.brush).filter(|i| *i < 3).unwrap_or(usize::MAX),
        |s, i| s.brush = Brush::ALL[i.min(2)],
    );
    let kind2 = choice(
        commands,
        fonts,
        &labels[3..],
        |s| Brush::ALL.iter().position(|b| *b == s.brush).and_then(|i| i.checked_sub(3)).unwrap_or(usize::MAX),
        |s, i| s.brush = Brush::ALL[3 + i.min(2)],
    );
    let radius = slider(commands, fonts, "Radius", 0.003, 0.5, |s| s.radius, |s, v| s.radius = v);
    let strength = slider(commands, fonts, "Strength", 0.0, 1.0, |s| s.strength, |s, v| s.strength = v);
    let falloff = choice(
        commands,
        fonts,
        &["Smooth", "Sharp", "Flat"],
        |s| match s.falloff {
            Falloff::Smooth => 0,
            Falloff::Sharp => 1,
            Falloff::Flat => 2,
        },
        |s, i| s.falloff = [Falloff::Smooth, Falloff::Sharp, Falloff::Flat][i.min(2)],
    );
    let mirror = switch(commands, fonts, "Mirror X", |s| s.mirror, |s, on| s.mirror = on);
    let every = switch(commands, fonts, "Every mesh", |s| s.every_mesh, |s, on| s.every_mesh = on);
    let b_hint = note(commands, fonts, "Ctrl inverts, Shift smooths, [ and ] size the brush. A stroke keeps to the mesh it starts on unless Every mesh is on.");
    commands.entity(brush).add_children(&[kind, kind2, radius, strength, falloff, mirror, every, b_hint]);
    let brush = titled(commands, fonts, "paint-brush", "Brush", brush);

    commands.entity(body).add_children(&[list_s, brush]);
    commands.entity(col).add_children(&[none, body]);
    col
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

fn shape_snapshot(w: &Rx) -> KeyedSnapshot {
    if !w.get_resource::<StageUi>().is_some_and(|u| u.mode == StageMode::Shapes) {
        return KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) };
    }
    let (names, active): (Vec<String>, Option<usize>) =
        w.get_resource::<ShapeEdit>().map(|s| (s.shapes.iter().map(|s| s.name.clone()).collect(), s.active)).unwrap_or_default();
    let items = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let mut k = hasher();
            (i, n).hash(&mut k);
            let mut h = hasher();
            (active == Some(i)).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let on = active == Some(i);
            let r = c
                .spawn((
                    Node { width: Val::Percent(100.0), align_items: AlignItems::Center, column_gap: Val::Px(4.0), padding: UiRect::all(Val::Px(2.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
                    BackgroundColor(if on { rgb(accent()).with_alpha(0.22) } else { Color::NONE }),
                ))
                .id();
            let pick = icon_button(c, f, if on { "paint-brush" } else { "circle" });
            on_click(c, pick, move |w| {
                let mut s = w.resource_mut::<ShapeEdit>();
                if i < s.shapes.len() {
                    s.active = Some(i);
                }
            });
            let s = spin_slider(c, &f.ui, &names[i], 0.0, 0.0, 1.0);
            c.entity(s).entry::<Node>().and_modify(|mut n| n.flex_grow = 1.0);
            bind_2way(
                c,
                s,
                move |w: &Rx| w.get_resource::<ShapeEdit>().and_then(|s| s.shapes.get(i)).map(|s| s.weight).unwrap_or(0.0),
                move |w: &mut World, v: &f32| {
                    let mut e = w.resource_mut::<ShapeEdit>();
                    if let Some(s) = e.shapes.get_mut(i) {
                        s.weight = *v;
                    }
                    e.mark_dirty();
                },
            );
            c.entity(r).add_children(&[pick, s]);
            r
        }),
    }
}

// ── Right ────────────────────────────────────────────────────────────────────

pub(super) fn inspector(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = column(commands);
    let none = note(commands, fonts, "Nothing to sculpt until a model is on the stage.");
    bind_display(commands, none, |w| !ready(w));
    let body = column(commands);
    bind_display(commands, body, ready);

    let shape = column(commands);
    let title = commands.spawn((Text::new(""), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary())))).id();
    bind_text(commands, title, |w| match w.get_resource::<ShapeEdit>().and_then(|s| s.active_shape()) {
        Some(s) => format!("Sculpting \"{}\"", s.name),
        None => "No shape picked. Make one with New Shape.".to_string(),
    });
    let weight = slider(
        commands,
        fonts,
        "Preview weight",
        0.0,
        1.0,
        |s| s.active_shape().map(|a| a.weight).unwrap_or(0.0),
        |s, v| {
            if let Some(i) = s.active {
                if let Some(a) = s.shapes.get_mut(i) {
                    a.weight = v;
                }
            }
            s.mark_dirty();
        },
    );
    let r = row(commands);
    let rename = action(commands, fonts, "Rename", shapes::rename);
    let clear = action(commands, fonts, "Clear", shapes::clear);
    let delete = action(commands, fonts, "Delete", shapes::delete);
    commands.entity(r).add_children(&[rename, clear, delete]);
    let s_hint = note(
        commands,
        fonts,
        "Rename takes the name typed in the name field. Deleting a shape moves the ones after it up a place, so a clip keyed on them drives the next shape along.",
    );
    commands.entity(shape).add_children(&[title, weight, r, s_hint]);
    let shape = titled(commands, fonts, "drop-half", "Shape", shape);

    let save = column(commands);
    let state = note(commands, fonts, "");
    bind_text(commands, state, |w| {
        if w.get_resource::<ShapeEdit>().is_some_and(|s| s.unsaved) {
            "Sculpted shapes are not saved yet. They are saved into the model file when you Save, or when you leave Shapes.".to_string()
        } else {
            "The shapes match the model file.".to_string()
        }
    });
    let r2 = row(commands);
    let save_b = renzora_ember::widgets::accent_button(commands, fonts, accent(), "Save Shapes");
    on_click(commands, save_b, shapes::save);
    let revert = action(commands, fonts, "Revert", shapes::revert);
    commands.entity(r2).add_children(&[save_b, revert]);
    commands.entity(save).add_children(&[state, r2]);
    let save = titled(commands, fonts, "floppy-disk", "Save", save);

    commands.entity(body).add_children(&[shape, save]);
    commands.entity(col).add_children(&[none, body]);
    col
}
