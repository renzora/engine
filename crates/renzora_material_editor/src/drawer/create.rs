//! The "New material" overlay: name it, pick where it goes.
//!
//! It used to write straight into `<project>/materials/` and jump to the
//! Material Editor. Both were assumptions: a project that files materials by
//! area now has to move the file afterwards, and being thrown into a node graph
//! is the wrong answer when what you wanted was a material on this mesh — the
//! texture slots in the drawer are where a new material actually gets filled in.

use std::path::Path;

use bevy::prelude::*;

use renzora::core::CurrentProject;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::theme::{rgb, text_muted};
use renzora_ember::widgets::{
    button, folder_new_button, folder_picker, overlay_sized, text_input, EmberForm, EmberTextInput,
    FolderPick,
};

use super::drop::{create_material_at, default_material_stem, sanitize_stem};
use super::MatCreateBtn;

/// Conventional home for materials the editor creates, and the seeded
/// destination in the overlay's tree.
pub(super) const MATERIALS_DIR: &str = "materials";

/// How far below the project root the destination tree walks. Two levels is
/// enough for `materials/` and a category under it without turning the overlay
/// into a file manager.
const MAT_PICKER_DEPTH: usize = 2;

/// The open "New material" overlay.
#[derive(Resource)]
pub(super) struct PendingMatCreate {
    entity: Entity,
    overlay: Entity,
    name_input: Entity,
    ticks: u8,
}

#[derive(Component)]
pub(super) struct MatCreateConfirmBtn;
#[derive(Component)]
pub(super) struct MatCreateCancelBtn;

/// "New material" → ask where it should go.
pub(super) fn mat_create_click(q: Query<(&Interaction, &MatCreateBtn), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, b) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let e = b.entity;
        commands.queue(move |w: &mut World| open_create_overlay(w, e));
    }
}

/// Build the name + destination overlay. Exclusive-world so it can read the
/// project, pre-create the conventional folder and walk the tree in one shot.
fn open_create_overlay(world: &mut World, entity: Entity) {
    if world.contains_resource::<PendingMatCreate>() {
        return;
    }
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
    let Some(root) = world.get_resource::<CurrentProject>().map(|p| p.path.clone()) else {
        return;
    };
    // Pre-create the conventional folder so the default destination is a real
    // row in the tree even on a project that has never had one.
    let default_dest = root.join(MATERIALS_DIR);
    let _ = std::fs::create_dir_all(&default_dest);
    let stem = default_material_stem(world, entity);

    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        let (overlay, content) = overlay_sized(&mut commands, &fonts, "New material", 480.0, 440.0, true);

        let name_input = text_input(&mut commands, &fonts.ui, &stem, &stem);
        let name_row = overlay_field(&mut commands, &fonts, "Name", name_input);
        let dest_label = overlay_label(&mut commands, &fonts, "Destination");
        let picker = folder_picker(&mut commands, &fonts, &root, &default_dest, MAT_PICKER_DEPTH);

        let buttons = commands
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::FlexEnd,
                column_gap: Val::Px(8.0),
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            })
            .id();
        // New Folder rides in the button row rather than under the tree — one
        // row of controls, not two. It floats at the row's left edge (absolute,
        // out of flow), so Cancel and Create lay out untouched.
        let new_folder = folder_new_button(&mut commands, &fonts, picker);
        let cancel = button(&mut commands, &fonts.ui, "Cancel");
        commands.entity(cancel).insert(MatCreateCancelBtn);
        let confirm = button(&mut commands, &fonts.ui, "Create");
        commands.entity(confirm).insert(MatCreateConfirmBtn);
        commands.entity(buttons).add_children(&[new_folder, cancel, confirm]);

        let body = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    min_height: Val::Px(0.0),
                    row_gap: Val::Px(6.0),
                    padding: UiRect::all(Val::Px(14.0)),
                    ..default()
                },
                // Enter in the name field = Create. Typing a name and then
                // reaching for the mouse is the one interaction this overlay
                // would otherwise force on every single use.
                EmberForm { submit: confirm },
            ))
            .id();
        commands.entity(body).add_children(&[name_row, dest_label, picker, buttons]);
        commands.entity(content).add_child(body);

        commands.insert_resource(PendingMatCreate { entity, overlay, name_input, ticks: 0 });
    }
    queue.apply(world);
}

/// A labelled row in the overlay: fixed-width caption, control filling the rest.
fn overlay_field(commands: &mut Commands, fonts: &EmberFonts, label: &str, control: Entity) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let caption = commands
        .spawn((
            Node { width: Val::Px(72.0), flex_shrink: 0.0, ..default() },
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(control).entry::<Node>().and_modify(|mut n| {
        n.flex_grow = 1.0;
        n.min_width = Val::Px(0.0);
    });
    commands.entity(row).add_children(&[caption, control]);
    row
}

fn overlay_label(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    commands
        .spawn((
            Node { margin: UiRect::top(Val::Px(6.0)), ..default() },
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id()
}

/// Focus the name field with its default selected, so the overlay is "type the
/// name, press Enter" with no click first.
///
/// Deliberately a tick late. The overlay is spawned from a button press, and
/// ember's `text_input_focus` blurs every input on any left press that didn't
/// land *on* an input — this field doesn't exist yet when that press is read, so
/// focusing on the opening frame would be undone by whichever order the two
/// systems happen to run in. The next tick has no press to blur against.
pub(super) fn mat_create_focus(pending: Option<ResMut<PendingMatCreate>>, mut inputs: Query<&mut EmberTextInput>) {
    let Some(mut pending) = pending else { return };
    if pending.ticks > 1 {
        return;
    }
    pending.ticks += 1;
    if pending.ticks != 2 {
        return;
    }
    if let Ok(mut input) = inputs.get_mut(pending.name_input) {
        input.focused = true;
        // Select-all, so the first keystroke replaces the default name rather
        // than prepending to it.
        input.select_all = true;
        input.caret_index = input.value.chars().count();
    }
}

/// Create → write the material into the picked folder and bind it; cancel (or a
/// backdrop/Escape dismiss, which despawns the overlay out from under us) → drop
/// the pending state and leave the mesh alone.
pub(super) fn mat_create_overlay_buttons(
    confirm: Query<&Interaction, (With<MatCreateConfirmBtn>, Changed<Interaction>)>,
    cancel: Query<&Interaction, (With<MatCreateCancelBtn>, Changed<Interaction>)>,
    pending: Option<Res<PendingMatCreate>>,
    inputs: Query<&EmberTextInput>,
    pick: Res<FolderPick>,
    project: Option<Res<CurrentProject>>,
    nodes: Query<(), With<Node>>,
    mut commands: Commands,
) {
    let Some(pending) = pending else { return };

    // Escape and backdrop clicks are ember's, and they despawn the root without
    // telling us — so a vanished overlay is a cancel.
    if nodes.get(pending.overlay).is_err() {
        commands.remove_resource::<PendingMatCreate>();
        return;
    }
    if cancel.iter().any(|i| *i == Interaction::Pressed) {
        commands.entity(pending.overlay).despawn();
        commands.remove_resource::<PendingMatCreate>();
        return;
    }
    if !confirm.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }

    let typed = inputs.get(pending.name_input).map(|i| i.value.trim().to_string()).unwrap_or_default();
    let stem = sanitize_stem(&typed);
    let Some(root) = project.as_ref().map(|p| p.path.clone()) else { return };
    let dir = pick.path().map(Path::to_path_buf).unwrap_or_else(|| root.join(MATERIALS_DIR));
    let entity = pending.entity;

    commands.entity(pending.overlay).despawn();
    commands.remove_resource::<PendingMatCreate>();
    commands.queue(move |w: &mut World| {
        // The drawer keys off (entity, path, rev); the path changed, so the
        // rebuild picks the new file up — and its texture slots appear — without
        // anything here poking it. No editor tab: filling the material in is
        // what those slots are for.
        create_material_at(w, entity, &dir, &stem);
    });
}
