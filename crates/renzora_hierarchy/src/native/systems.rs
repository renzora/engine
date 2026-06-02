//! Hierarchy interaction: caret toggles expansion, a row click selects, and a
//! per-frame visual pass paints selection/hover (in place — no rebuild).

use bevy::prelude::*;

use renzora_editor::{EditorCommands, EditorSelection};
use renzora_ember::theme::{rgb, ACCENT_BLUE};
use renzora_undo::{execute, LockToggleCmd, UndoContext, VisibilityToggleCmd};

use super::components::{HierLockToggle, HierRow, HierRowClick, HierVisToggle};
use super::HierExpanded;

/// Row click → select the entity and, for a parent, toggle its expansion. The
/// click layer covers the caret/name/empty space but not the eye/lock zone, so
/// clicking a toggle never selects/expands.
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

/// Composite `top` over `base` (both straight sRGBA).
fn over(base: Color, top: Color) -> Color {
    let b = base.to_srgba();
    let t = top.to_srgba();
    let a = t.alpha + b.alpha * (1.0 - t.alpha);
    if a <= 0.0 {
        return Color::NONE;
    }
    Color::srgba(
        (t.red * t.alpha + b.red * b.alpha * (1.0 - t.alpha)) / a,
        (t.green * t.alpha + b.green * b.alpha * (1.0 - t.alpha)) / a,
        (t.blue * t.alpha + b.blue * b.alpha * (1.0 - t.alpha)) / a,
        a,
    )
}

/// Paint each row's background + label color from selection/hover state. Runs
/// every frame but writes only on change, so it never churns.
pub(crate) fn hierarchy_row_visual(
    selection: Option<Res<EditorSelection>>,
    mut rows: Query<(&HierRow, &mut BackgroundColor)>,
    clicks: Query<&Interaction>,
    mut colors: Query<&mut TextColor>,
) {
    let Some(selection) = selection else {
        return;
    };
    let hover = Color::srgba(1.0, 1.0, 1.0, 0.06);
    let sel_bg = rgb(ACCENT_BLUE).with_alpha(0.63);
    for (row, mut bg) in &mut rows {
        let selected = selection.is_selected(row.entity);
        let hovered = clicks
            .get(row.click)
            .is_ok_and(|i| matches!(i, Interaction::Hovered | Interaction::Pressed));
        let (target_bg, label_col) = if selected {
            (sel_bg, Color::WHITE)
        } else if hovered {
            (over(row.base_bg, hover), row.label_color)
        } else {
            (row.base_bg, row.label_color)
        };
        if bg.0 != target_bg {
            bg.0 = target_bg;
        }
        if let Ok(mut c) = colors.get_mut(row.label) {
            if c.0 != label_col {
                c.0 = label_col;
            }
        }
    }
}
