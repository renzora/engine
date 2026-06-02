//! Hierarchy interaction: caret toggles expansion, a row click selects, and a
//! per-frame visual pass paints selection/hover (in place — no rebuild).

use bevy::prelude::*;

use renzora_editor::EditorSelection;
use renzora_ember::theme::{rgb, ACCENT_BLUE};

use super::components::{HierCaret, HierRow};
use super::HierExpanded;

/// Caret click → toggle the entity's expansion.
pub(crate) fn hierarchy_caret_click(
    carets: Query<(&Interaction, &HierCaret), Changed<Interaction>>,
    mut expanded: ResMut<HierExpanded>,
) {
    for (interaction, caret) in &carets {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if expanded.0.contains(&caret.entity) {
            expanded.0.remove(&caret.entity);
        } else {
            expanded.0.insert(caret.entity);
        }
    }
}

/// Row body click → select that entity (single-select; ctrl/shift range come in
/// a later stage).
pub(crate) fn hierarchy_row_click(
    rows: Query<(&Interaction, &HierRow), Changed<Interaction>>,
    selection: Option<Res<EditorSelection>>,
) {
    let Some(selection) = selection else {
        return;
    };
    for (interaction, row) in &rows {
        if *interaction == Interaction::Pressed {
            selection.set(Some(row.entity));
        }
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
    mut rows: Query<(&Interaction, &HierRow, &mut BackgroundColor)>,
    mut colors: Query<&mut TextColor>,
) {
    let Some(selection) = selection else {
        return;
    };
    let hover = Color::srgba(1.0, 1.0, 1.0, 0.06);
    let sel_bg = rgb(ACCENT_BLUE).with_alpha(0.63);
    for (interaction, row, mut bg) in &mut rows {
        let selected = selection.is_selected(row.entity);
        let (target_bg, label_col) = if selected {
            (sel_bg, Color::WHITE)
        } else if matches!(interaction, Interaction::Hovered | Interaction::Pressed) {
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
