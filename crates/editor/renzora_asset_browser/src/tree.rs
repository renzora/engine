use std::path::PathBuf;

use bevy_egui::egui::{self, Align2, Color32, CursorIcon, FontId, Pos2, Sense, Vec2};
use egui_phosphor::regular::{CARET_DOWN, CARET_RIGHT, FOLDER, FOLDER_OPEN, HOUSE};
use renzora_theme::Theme;

use crate::state::{folder_icon_color, is_hidden, AssetBrowserState};

/// Row height for tree entries.
const ROW_HEIGHT: f32 = 20.0;
/// Indentation per depth level.
const INDENT: f32 = 14.0;

/// Renders the folder tree in the left pane (legacy-matching style).
pub fn tree_ui(ui: &mut egui::Ui, state: &mut AssetBrowserState, theme: &Theme) {
    let root = state.root();
    let root_name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Project")
        .to_string();

    egui::ScrollArea::vertical()
        .id_salt("asset_tree")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.style_mut().spacing.item_spacing.y = 0.0;

            // Root node
            let is_expanded = state.expanded_folders.contains(&root);
            let is_current = state.current_folder.as_ref() == Some(&root);

            let icon = if is_expanded { FOLDER_OPEN } else { HOUSE };
            let color = folder_icon_color(&root_name);

            let clicked = render_folder_row(
                ui,
                &root_name,
                icon,
                color,
                is_expanded,
                is_current,
                0,
                &root,
                theme,
            );

            if clicked {
                toggle_expanded(&mut state.expanded_folders, &root);
                state.current_folder = Some(root.clone());
            }

            if is_expanded {
                render_folder_children(ui, state, &root.clone(), 1, theme);
            }
        });
}

fn render_folder_children(
    ui: &mut egui::Ui,
    state: &mut AssetBrowserState,
    parent: &PathBuf,
    depth: usize,
    theme: &Theme,
) {
    #[cfg(target_arch = "wasm32")]
    let mut folders: Vec<PathBuf> = Vec::new();
    #[cfg(not(target_arch = "wasm32"))]
    let mut folders: Vec<PathBuf> = match std::fs::read_dir(parent) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
            .filter(|e| !is_hidden(&e.path()))
            .filter(|e| {
                // Skip target and Cargo.lock
                let name = e.file_name().to_string_lossy().to_string();
                name != "target" && name != "Cargo.lock"
            })
            .map(|e| e.path())
            .collect(),
        Err(_) => return,
    };
    folders.sort_by(|a, b| {
        a.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase()
            .cmp(
                &b.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_lowercase(),
            )
    });

    for folder in &folders {
        let name = folder
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("???")
            .to_string();

        // Search filter
        if !state.search.is_empty() {
            if !name.to_lowercase().contains(&state.search.to_lowercase()) {
                if !folder_contains_match(folder, &state.search) {
                    continue;
                }
            }
        }

        let is_expanded = state.expanded_folders.contains(folder);
        let is_current = state.current_folder.as_ref() == Some(folder);
        let (icon, color) = get_folder_icon(is_expanded, &name);

        let clicked = render_folder_row(
            ui,
            &name,
            icon,
            color,
            is_expanded,
            is_current,
            depth,
            folder,
            theme,
        );

        if clicked {
            toggle_expanded(&mut state.expanded_folders, folder);
            state.current_folder = Some(folder.clone());
        }

        if is_expanded {
            render_folder_children(ui, state, folder, depth + 1, theme);
        }
    }
}

/// Render a single folder row. Returns true if clicked.
fn render_folder_row(
    ui: &mut egui::Ui,
    name: &str,
    icon: &str,
    icon_color: Color32,
    is_expanded: bool,
    is_current: bool,
    depth: usize,
    path: &PathBuf,
    theme: &Theme,
) -> bool {
    let selection_bg = theme.semantic.selection.to_color32();
    let item_hover = theme.panels.item_hover.to_color32();
    let text_secondary = theme.text.secondary.to_color32();
    let text_muted = theme.text.muted.to_color32();

    let indent = depth as f32 * INDENT;

    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), ROW_HEIGHT),
        Sense::click(),
    );

    let is_hovered = response.hovered();
    if is_hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    let painter = ui.painter();

    // Background — transparent by default, selection or hover fill with rounded corners
    if is_current {
        painter.rect_filled(rect, 2.0, selection_bg);
    } else if is_hovered {
        painter.rect_filled(rect, 2.0, item_hover);
    }

    // Expand/collapse arrow
    let arrow_x = rect.min.x + indent + 8.0;
    let arrow_icon = if is_expanded { CARET_DOWN } else { CARET_RIGHT };

    let arrow_rect = egui::Rect::from_center_size(
        Pos2::new(arrow_x, rect.center().y),
        Vec2::splat(14.0),
    );
    let arrow_id = ui.id().with(("nav_arrow", path.as_os_str()));
    let arrow_resp = ui.interact(arrow_rect, arrow_id, Sense::click());
    if arrow_resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    painter.text(
        Pos2::new(arrow_x, rect.center().y),
        Align2::CENTER_CENTER,
        arrow_icon,
        FontId::proportional(9.0),
        if arrow_resp.hovered() { text_secondary } else { text_muted },
    );

    // Folder icon
    let icon_x = arrow_x + 12.0;
    painter.text(
        Pos2::new(icon_x, rect.center().y),
        Align2::LEFT_CENTER,
        icon,
        FontId::proportional(12.0),
        icon_color,
    );

    // Folder name
    painter.text(
        Pos2::new(icon_x + 16.0, rect.center().y),
        Align2::LEFT_CENTER,
        name,
        FontId::proportional(11.0),
        text_secondary,
    );

    // Return true if arrow or row was clicked
    arrow_resp.clicked() || response.clicked()
}

fn get_folder_icon(is_expanded: bool, name: &str) -> (&'static str, Color32) {
    let color = folder_icon_color(name);
    let icon = if is_expanded { FOLDER_OPEN } else { FOLDER };
    (icon, color)
}

fn toggle_expanded(set: &mut std::collections::HashSet<PathBuf>, path: &PathBuf) {
    if set.contains(path) {
        set.remove(path);
    } else {
        set.insert(path.clone());
    }
}

/// Check recursively if any file/folder within `dir` matches the search.
fn folder_contains_match(dir: &PathBuf, search: &str) -> bool {
    #[cfg(target_arch = "wasm32")]
    { let _ = (dir, search); return false; }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let search_lower = search.to_lowercase();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return false;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.starts_with('.') {
                continue;
            }
            if name.contains(&search_lower) {
                return true;
            }
            if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                if folder_contains_match(&entry.path(), search) {
                    return true;
                }
            }
        }
        false
    }
}
