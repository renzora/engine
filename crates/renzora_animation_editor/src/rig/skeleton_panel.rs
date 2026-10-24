//! Rig mode's skeleton editor panel: the draft's bones as a list, the
//! selected bone's name and place, and adding, deleting, reparenting and
//! placing bones. The draft and its stage drawing are `stage::bone_edit`.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_display, bind_text, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{bind_text_input, button, drag_value, text_input};
use renzora_ember::UiInteraction;

use super::{model_file, on_click, subject};
use crate::stage::bone_edit::{self, SkeletonDraft};

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
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(4.0), row_gap: Val::Px(4.0), ..default() })
        .id()
}

fn action(commands: &mut Commands, fonts: &EmberFonts, label: &str, f: impl Fn(&mut World) + Send + Sync + 'static) -> Entity {
    let b = button(commands, &fonts.ui, label);
    on_click(commands, b, f);
    b
}

fn editing(w: &Rx) -> bool {
    w.get_resource::<SkeletonDraft>().is_some_and(|d| d.active())
}

fn draft_flag(w: &Rx, f: fn(&SkeletonDraft) -> bool) -> bool {
    w.get_resource::<SkeletonDraft>().is_some_and(f)
}

/// The skeleton editor, for any model with a file.
pub(crate) fn skeleton_editor(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = column(commands);

    let idle = column(commands);
    bind_display(commands, idle, |w| !editing(w) && subject(w.untracked()).is_some_and(|e| model_file(w.untracked(), e).is_some()));
    let about = note(
        commands,
        fonts,
        "Add, move, rename, reparent or delete bones by hand: a quadruped, a prop with a hinge, a creature, or one more bone on a generated rig. A model with no skeleton starts from one root bone.",
    );
    let start = action(commands, fonts, "Edit Skeleton", bone_edit::start);
    let export_note = note(commands, fonts, "Export with Clips writes the model, its skeleton and every clip on its animator to exports/ as one .glb, for use in other tools.");
    let export = action(commands, fonts, "Export with Clips", super::export::export_model);
    commands.entity(idle).add_children(&[about, start, export_note, export]);

    let body = column(commands);
    bind_display(commands, body, editing);
    let hint = note(
        commands,
        fonts,
        "Click a bone in the list or on the stage to select it. Place Joint, then a click on the model, puts the joint in the middle of the part under the cursor.",
    );
    let list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() }).id();
    keyed_list(commands, list, draft_snapshot);

    let name = text_input(commands, &fonts.ui, "Bone name", "");
    commands.entity(name).entry::<Node>().and_modify(|mut n| n.width = Val::Percent(100.0));
    bind_text_input(
        commands,
        name,
        |w: &Rx| w.get_resource::<SkeletonDraft>().and_then(|d| d.selected.and_then(|i| d.bones.get(i)).map(|b| b.name.clone())).unwrap_or_default(),
        |w: &mut World, v: String| bone_edit::rename(w, v),
    );
    let xyz = row(commands);
    for (axis, label, colour) in [(0usize, "X", (230, 90, 90)), (1, "Y", (120, 200, 110)), (2, "Z", (100, 150, 240))] {
        let field = drag_value(commands, &fonts.ui, label, colour, 0.0, 0.005);
        commands.entity(field).entry::<Node>().and_modify(|mut n| n.flex_grow = 1.0);
        bind_2way(
            commands,
            field,
            move |w: &Rx| w.get_resource::<SkeletonDraft>().and_then(|d| d.selected.and_then(|i| d.bones.get(i)).map(|b| b.head[axis])).unwrap_or(0.0),
            move |w: &mut World, v: &f32| bone_edit::set_head(w, axis, *v),
        );
        commands.entity(xyz).add_child(field);
    }

    let tools = row(commands);
    let add = action(commands, fonts, "Add Child", bone_edit::add_child);
    let del = action(commands, fonts, "Delete", bone_edit::delete);
    let parent = action(commands, fonts, "Pick Parent", |w| {
        let mut d = w.resource_mut::<SkeletonDraft>();
        d.picking_parent = !d.picking_parent;
        d.placing = false;
    });
    let place = action(commands, fonts, "Place Joint", |w| {
        let mut d = w.resource_mut::<SkeletonDraft>();
        d.placing = !d.placing;
        d.picking_parent = false;
    });
    commands.entity(tools).add_children(&[add, del, parent, place]);
    let picking = note(commands, fonts, "Click the new parent, in the list or on the stage.");
    bind_display(commands, picking, |w| draft_flag(w, |d| d.picking_parent));
    let placing = note(commands, fonts, "Click the model where the joint goes.");
    bind_display(commands, placing, |w| draft_flag(w, |d| d.placing));

    let finish = row(commands);
    let apply = renzora_ember::widgets::accent_button(commands, fonts, accent(), "Apply Skeleton");
    on_click(commands, apply, bone_edit::apply);
    let cancel = action(commands, fonts, "Cancel", bone_edit::cancel);
    commands.entity(finish).add_children(&[apply, cancel]);
    let warn = note(commands, fonts, "Apply rebuilds the skin weights from the bones' new places; the original file is kept.");

    commands.entity(body).add_children(&[hint, list, name, xyz, tools, picking, placing, finish, warn]);
    commands.entity(col).add_children(&[idle, body]);
    col
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

fn draft_snapshot(w: &Rx) -> KeyedSnapshot {
    let rows: Vec<(usize, String, usize)> = match w.get_resource::<SkeletonDraft>() {
        Some(d) if d.active() => d.tree().into_iter().map(|(i, depth)| (i, d.bones[i].name.clone(), depth)).collect(),
        _ => Vec::new(),
    };
    let items = rows
        .iter()
        .map(|(i, name, depth)| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            (name, depth).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, n| {
            let (i, name, depth) = rows[n].clone();
            draft_row(c, f, i, &name, depth)
        }),
    }
}

fn draft_row(commands: &mut Commands, fonts: &EmberFonts, bone: usize, name: &str, depth: usize) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(22.0),
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::left(Val::Px(6.0 + depth.min(14) as f32 * 12.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    bind_bg(commands, row, move |w| {
        let selected = w.get_resource::<SkeletonDraft>().is_some_and(|d| d.selected == Some(bone));
        let hot = matches!(w.get::<UiInteraction>(row), Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed));
        match (selected, hot) {
            (true, _) => rgb(accent()).with_alpha(0.28),
            (false, true) => rgb(hover_bg()),
            _ => Color::NONE,
        }
    });
    let icon = icon_text(commands, &fonts.phosphor, "bone", text_muted(), 12.0);
    let label = commands.spawn((Text::new(name), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())))).id();
    bind_text(commands, label, move |w| w.get_resource::<SkeletonDraft>().and_then(|d| d.bones.get(bone).map(|b| b.name.clone())).unwrap_or_default());
    commands.entity(row).add_children(&[icon, label]);
    on_click(commands, row, move |w| bone_edit::select(w, bone));
    row
}
