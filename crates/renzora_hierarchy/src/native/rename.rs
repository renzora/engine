//! Inline rename for the native hierarchy: double-click a row (or the context
//! menu's "Rename") edits its `Name` in a text field. Enter or clicking away
//! commits (via `RenameCmd`, so it's undoable); Escape cancels. Mirrors the egui
//! panel's inline rename.

use bevy::prelude::*;

use renzora_ember::font::EmberFonts;
use renzora_ember::widgets::{text_input, EmberTextInput};
use renzora_undo::{execute, RenameCmd, UndoContext};

/// The entity currently being renamed (`None` = no active rename). Read by the
/// tree snapshot so the row renders its rename field.
#[derive(Resource, Default)]
pub(crate) struct HierRename(pub Option<Entity>);

/// Marks the inline rename text field, carrying the entity it renames.
#[derive(Component)]
pub(crate) struct HierRenameInput(pub Entity);

/// Build the inline rename field (used by `build_row` in place of the label),
/// seeded with the current name.
pub(crate) fn build_rename_field(commands: &mut Commands, fonts: &EmberFonts, entity: Entity, name: &str) -> Entity {
    let input = text_input(commands, &fonts.ui, "Name", name);
    commands.entity(input).insert((
        HierRenameInput(entity),
        Node {
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            height: Val::Px(20.0),
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(4.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        },
    ));
    input
}

/// Auto-focus the rename field the frame it appears — `text_input` only focuses
/// on a click otherwise, and the rename is triggered by a double-click/menu.
pub(crate) fn focus_rename_field(mut q: Query<&mut EmberTextInput, Added<HierRenameInput>>) {
    for mut inp in &mut q {
        inp.focused = true;
    }
}

/// Commit (Enter / click-away blur) or cancel (Escape) the active rename.
pub(crate) fn rename_commit(
    mut rename: ResMut<HierRename>,
    keys: Res<ButtonInput<KeyCode>>,
    inputs: Query<(&EmberTextInput, &HierRenameInput)>,
    mut commands: Commands,
) {
    let Some(entity) = rename.0 else { return };

    if keys.just_pressed(KeyCode::Escape) {
        rename.0 = None;
        return;
    }

    let Some((inp, _)) = inputs.iter().find(|(_, r)| r.0 == entity) else {
        // The field is gone (row rebuilt without it) — drop the rename.
        rename.0 = None;
        return;
    };

    let enter = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);
    // A click on another widget blurs the field (text_input's off-click) — treat
    // that as a commit, mirroring egui's lost-focus behaviour.
    let blurred = !inp.focused;
    if !enter && !blurred {
        return;
    }

    let new: String = inp.value.replace('\n', "").trim().to_string();
    rename.0 = None;
    if new.is_empty() {
        return;
    }
    commands.queue(move |world: &mut World| {
        let old = world.get::<Name>(entity).map(|n| n.as_str().to_string()).unwrap_or_default();
        if old == new {
            return;
        }
        execute(world, UndoContext::Scene, Box::new(RenameCmd { entity, old, new }));
    });
}
