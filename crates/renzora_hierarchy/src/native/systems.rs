//! Hierarchy interaction: row click selects + toggles expansion; eye/lock push
//! undoable visibility/lock commands. (Selection/hover visuals are reactive
//! bindings declared in `row.rs`, not a system here.)

use bevy::prelude::*;

use renzora_editor::{EditorCommands, EditorSelection};
use renzora_undo::{execute, LockToggleCmd, UndoContext, VisibilityToggleCmd};

use super::components::{HierLockToggle, HierRowClick, HierVisToggle};
use super::HierExpanded;

/// Row click → select the entity and, for a parent, toggle its expansion. The
/// click layer excludes the eye/lock zone, so toggling those never lands here.
pub(crate) fn hierarchy_row_click(
    rows: Query<(&Interaction, &HierRowClick), Changed<Interaction>>,
    selection: Option<Res<EditorSelection>>,
    mut expanded: ResMut<HierExpanded>,
) {
    let Some(selection) = selection else {
        return;
    };
    for (interaction, row) in &rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        selection.set(Some(row.entity));
        if row.has_children {
            if expanded.0.contains(&row.entity) {
                expanded.0.remove(&row.entity);
            } else {
                expanded.0.insert(row.entity);
            }
        }
    }
}

/// Eye toggle click → push a visibility-toggle command (undoable).
pub(crate) fn hierarchy_vis_toggle(
    q: Query<(&Interaction, &HierVisToggle), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else {
        return;
    };
    for (interaction, t) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (entity, was_visible) = (t.entity, t.visible);
        cmds.push(move |world: &mut World| {
            execute(
                world,
                UndoContext::Scene,
                Box::new(VisibilityToggleCmd {
                    entity,
                    was_visible,
                }),
            );
        });
    }
}

/// Lock toggle click → push a lock-toggle command (undoable).
pub(crate) fn hierarchy_lock_toggle(
    q: Query<(&Interaction, &HierLockToggle), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else {
        return;
    };
    for (interaction, t) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (entity, was_locked) = (t.entity, t.locked);
        cmds.push(move |world: &mut World| {
            execute(
                world,
                UndoContext::Scene,
                Box::new(LockToggleCmd { entity, was_locked }),
            );
        });
    }
}
