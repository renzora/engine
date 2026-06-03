//! Hierarchy interaction: row click selects (plain / ctrl-toggle / shift-range);
//! the caret toggles expansion; eye/lock push undoable visibility/lock commands.
//! (Selection/hover visuals are reactive bindings declared in `row.rs`.)

use bevy::platform::collections::HashSet;
use bevy::prelude::*;

use renzora_editor::{EditorCommands, EditorSelection};
use renzora_undo::{execute, LockToggleCmd, UndoContext, VisibilityToggleCmd};

use crate::cache::HierarchyTreeCache;
use crate::state::EntityNode;

use super::components::{HierCaretToggle, HierLockToggle, HierRowClick, HierVisToggle};
use super::HierExpanded;

/// Visible (flattened, respecting expansion) entity order — the anchor list for
/// shift-range selection.
fn visible_order(cache: &HierarchyTreeCache, expanded: &HashSet<Entity>) -> Vec<Entity> {
    fn walk(nodes: &[EntityNode], expanded: &HashSet<Entity>, out: &mut Vec<Entity>) {
        for n in nodes {
            out.push(n.entity);
            if !n.children.is_empty() && expanded.contains(&n.entity) {
                walk(&n.children, expanded, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(&cache.nodes, expanded, &mut out);
    out
}

/// Row click → select the entity. Ctrl toggles it in the selection; Shift selects
/// the range from the current anchor; a plain click replaces the selection.
pub(crate) fn hierarchy_row_click(
    rows: Query<(&Interaction, &HierRowClick), Changed<Interaction>>,
    selection: Option<Res<EditorSelection>>,
    keys: Res<ButtonInput<KeyCode>>,
    cache: Res<HierarchyTreeCache>,
    expanded: Res<HierExpanded>,
) {
    let Some(selection) = selection else {
        return;
    };
    let ctrl = keys.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]);
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    for (interaction, row) in &rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if ctrl {
            selection.toggle(row.entity);
        } else if shift {
            match selection.get() {
                Some(anchor) => {
                    let order = visible_order(&cache, &expanded.0);
                    selection.select_range(&order, anchor, row.entity);
                }
                None => selection.set(Some(row.entity)),
            }
        } else {
            selection.set(Some(row.entity));
        }
    }
}

/// Caret click → toggle the row's expansion.
pub(crate) fn hierarchy_caret_click(
    q: Query<(&Interaction, &HierCaretToggle), Changed<Interaction>>,
    mut expanded: ResMut<HierExpanded>,
) {
    for (interaction, caret) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if expanded.0.contains(&caret.0) {
            expanded.0.remove(&caret.0);
        } else {
            expanded.0.insert(caret.0);
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
