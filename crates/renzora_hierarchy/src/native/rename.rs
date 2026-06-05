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
///
/// `had_focus` tracks whether the field has held focus yet: the field is spawned
/// by the keyed-list rebuild (deferred a frame or two after `HierRename` is set),
/// so we must *wait* for it rather than treating "no field yet" as gone, and we
/// only commit-on-blur once it has actually been focused (auto-focus runs the
/// frame after it appears).
pub(crate) fn rename_commit(
    mut rename: ResMut<HierRename>,
    keys: Res<ButtonInput<KeyCode>>,
    inputs: Query<(&EmberTextInput, &HierRenameInput)>,
    mut commands: Commands,
    mut had_focus: Local<bool>,
) {
    let Some(entity) = rename.0 else {
        *had_focus = false;
        return;
    };

    if keys.just_pressed(KeyCode::Escape) {
        rename.0 = None;
        *had_focus = false;
        return;
    }

    // Wait for the rename field to actually spawn (don't cancel in the meantime).
    let Some((inp, _)) = inputs.iter().find(|(_, r)| r.0 == entity) else {
        return;
    };
    if inp.focused {
        *had_focus = true;
    }

    let enter = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);
    // Click-away blur commits, but only after the field has held focus — so it
    // doesn't fire the frame the field first appears (before auto-focus runs).
    let blurred = *had_focus && !inp.focused;
    if !enter && !blurred {
        return;
    }

    let new: String = inp.value.replace('\n', "").trim().to_string();
    rename.0 = None;
    *had_focus = false;
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
