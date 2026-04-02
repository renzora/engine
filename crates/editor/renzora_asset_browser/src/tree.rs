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

    // Clear tree folder rects for drop hit-testing
    state.tree_folder_rects.clear();

    egui::ScrollArea::vertical()
        .id_salt("asset_tree")
        .auto_shrink([false, false])
        .drag_to_scroll(false)
        .show(ui, |ui| {
            ui.style_mut().spacing.item_spacing.y = 0.0;

            // Root node
            let is_expanded = state.expanded_folders.contains(&root);
            let is_current = state.current_folder.as_ref() == Some(&root);
            let is_drop_target = state.drop_target_folder.as_ref() == Some(&root);

            let icon = if is_expanded { FOLDER_OPEN } else { HOUSE };
            let color = folder_icon_color(&root_name);

            let (clicked, row_rect) = render_folder_row(
                ui,
                &root_name,
                icon,
                color,
                is_expanded,
                is_current,
                is_drop_target,
                0,
                &root,
                theme,
            );

            state.tree_folder_rects.push((root.clone(), row_rect));

            // Root is always expanded — clicking just navigates, never collapses
            if !is_expanded {
                state.expanded_folders.insert(root.clone());
            }
            if clicked {
                state.current_folder = Some(root.clone());
            }

            if is_expanded {
                render_folder_children(ui, state, &root.clone(), 1, theme);
            }
        });

    // Internal drag-move: detect drop on tree folders
    if !state.drag_moving.is_empty() {
        let ctx = ui.ctx().clone();
        let mut tree_drop_target: Option<PathBuf> = None;
        if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
            for (folder_path, rect) in &state.tree_folder_rects {
                if rect.contains(pos) && !state.drag_moving.contains(folder_path) {
                    tree_drop_target = Some(folder_path.clone());

                    // Visual feedback
                    let accent = theme.semantic.accent.to_color32();
                    ui.painter().rect_filled(
                        *rect,
                        2.0,
                        Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 40),
                    );
                    ui.painter().rect_stroke(
                        *rect,
                        2.0,
                        egui::Stroke::new(1.0, accent),
                        egui::StrokeKind::Inside,
                    );
                    break;
                }
            }
        }

        state.move_drop_target = tree_drop_target.clone();

        // Drop on release
        if ctx.input(|i| i.pointer.any_released()) {
            if let Some(target) = tree_drop_target {
                state.pending_move = Some((state.drag_moving.clone(), target));
            }
            state.drag_moving.clear();
            state.move_drop_target = None;
        }
    }
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
        let is_drop_target = state.drop_target_folder.as_ref() == Some(folder);
        let (icon, color) = get_folder_icon(is_expanded, &name);

        let (clicked, row_rect) = render_folder_row(
            ui,
            &name,
            icon,
            color,
            is_expanded,
            is_current,
            is_drop_target,
            depth,
            folder,
            theme,
        );

        state.tree_folder_rects.push((folder.clone(), row_rect));

        if clicked {
            toggle_expanded(&mut state.expanded_folders, folder);
            state.current_folder = Some(folder.clone());
        }

        if is_expanded {
            render_folder_children(ui, state, folder, depth + 1, theme);
        }
    }
}

/// Render a single folder row. Returns (clicked, row_rect).
fn render_folder_row(
    ui: &mut egui::Ui,
    name: &str,
    icon: &str,
    icon_color: Color32,
    is_expanded: bool,
    is_current: bool,
    is_drop_target: bool,
    depth: usize,
    path: &PathBuf,
    theme: &Theme,
) -> (bool, egui::Rect) {
    let selection_bg = theme.semantic.selection.to_color32();
    let item_hover = theme.panels.item_hover.to_color32();
    let text_secondary = theme.text.secondary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let accent_color = theme.semantic.accent.to_color32();

    let indent = depth as f32 * INDENT;

    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), ROW_HEIGHT),
        Sense::click_and_drag(),
    );

    let is_hovered = response.hovered();
    if is_hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    let painter = ui.painter();

    // Background — drop target highlight, selection, hover, or transparent
    if is_drop_target {
        painter.rect_filled(rect, 2.0, Color32::from_rgba_unmultiplied(
            accent_color.r(), accent_color.g(), accent_color.b(), 60,
        ));
        painter.rect_stroke(
            rect,
            2.0,
            egui::Stroke::new(1.0, accent_color),
            egui::StrokeKind::Inside,
        );
    } else if is_current {
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

    // Return (clicked, row_rect)
    (arrow_resp.clicked() || response.clicked(), rect)
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
