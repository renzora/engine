use std::path::PathBuf;

use renzora::bevy_egui::egui::{self, Align2, Color32, CursorIcon, FontId, Pos2, Sense, Vec2};
use renzora::egui_phosphor::regular::{self, CARET_DOWN, CARET_RIGHT, FOLDER, FOLDER_OPEN, HOUSE};
use renzora::theme::Theme;

use crate::state::{file_icon, folder_icon_color, is_hidden, AssetBrowserState};

/// Row height for tree entries.
const ROW_HEIGHT: f32 = 20.0;
/// Indentation per depth level.
const INDENT: f32 = 14.0;

/// Renders the folder tree in the left pane (legacy-matching style).
/// Returns a populated [`renzora::editor::AssetDragPayload`] when the user
/// starts dragging a file row, mirroring the grid's drag behavior so the
/// viewport can spawn the dragged asset on release.
pub fn tree_ui(
    ui: &mut egui::Ui,
    state: &mut AssetBrowserState,
    theme: &Theme,
) -> Option<renzora::editor::AssetDragPayload> {
    let root = state.root();
    let root_name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Project")
        .to_string();

    // Clear tree folder rects for drop hit-testing
    state.tree_folder_rects.clear();
    // Rebuild the flat visible order so shift-range selection works against
    // the same list the user actually sees in the tree (folders first, then
    // files inside each, depth-first).
    state.visible_item_order.clear();

    egui::ScrollArea::vertical()
        .id_salt("asset_tree")
        .auto_shrink([false, false])
        .drag_to_scroll(false)
        .show(ui, |ui| {
            ui.style_mut().spacing.item_spacing.y = 0.0;

            // Favorites section (always show header as drop target, items only when non-empty)
            {
                let text_muted = theme.text.muted.to_color32();
                let star_color = Color32::from_rgb(255, 200, 60);

                // Section header — doubles as drop target for adding favorites
                let (header_rect, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 18.0),
                    Sense::hover(),
                );

                // Drag-to-favorite: highlight header when dragging a folder over it
                let dragging_folders = !state.drag_moving.is_empty()
                    && state.drag_moving.iter().any(|p| p.is_dir());
                let pointer_over_header = if dragging_folders {
                    ui.ctx().input(|i| i.pointer.hover_pos().or(i.pointer.latest_pos()))
                        .map(|p| header_rect.contains(p))
                        .unwrap_or(false)
                } else {
                    false
                };

                if pointer_over_header {
                    let accent = theme.semantic.accent.to_color32();
                    ui.painter().rect_filled(
                        header_rect,
                        2.0,
                        Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 40),
                    );
                    // Drop on release — add dragged folders to favorites
                    if ui.ctx().input(|i| i.pointer.any_released()) {
                        for path in &state.drag_moving.clone() {
                            if path.is_dir() && !state.is_favorite(path) {
                                state.toggle_favorite(path);
                            }
                        }
                        state.drag_moving.clear();
                        state.move_drop_target = None;
                    }
                }

                ui.painter().text(
                    Pos2::new(header_rect.min.x + 10.0, header_rect.center().y),
                    Align2::LEFT_CENTER,
                    if dragging_folders && pointer_over_header {
                        format!("{} Add to Favorites", regular::STAR)
                    } else {
                        "Favorites".to_string()
                    },
                    FontId::proportional(10.0),
                    if pointer_over_header { star_color } else { text_muted },
                );

                // Render each favorite (only if there are any)
                let favorites_snapshot: Vec<std::path::PathBuf> = state.favorites.clone();
                for fav_path in &favorites_snapshot {
                    let fav_name = fav_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("???")
                        .to_string();

                    let is_current = state.current_folder.as_ref() == Some(fav_path);
                    let selection_bg = theme.semantic.selection.to_color32();
                    let item_hover = theme.panels.item_hover.to_color32();
                    let text_secondary = theme.text.secondary.to_color32();
                    let fav_icon_color = folder_icon_color(&fav_name);

                    let (rect, response) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), ROW_HEIGHT),
                        Sense::click(),
                    );

                    if response.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }

                    if is_current {
                        ui.painter().rect_filled(rect, 2.0, selection_bg);
                    } else if response.hovered() {
                        ui.painter().rect_filled(rect, 2.0, item_hover);
                    }

                    // Star icon
                    ui.painter().text(
                        Pos2::new(rect.min.x + 12.0, rect.center().y),
                        Align2::CENTER_CENTER,
                        regular::STAR,
                        FontId::proportional(10.0),
                        star_color,
                    );

                    // Folder icon
                    ui.painter().text(
                        Pos2::new(rect.min.x + 26.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        FOLDER,
                        FontId::proportional(12.0),
                        fav_icon_color,
                    );

                    // Folder name (truncated with ellipsis)
                    let fav_text_x = rect.min.x + 42.0;
                    let fav_max_w = (rect.max.x - fav_text_x - 4.0).max(0.0);
                    let fav_text_y = rect.center().y - 11.0 * 0.5;
                    paint_truncated_text(ui.painter(), Pos2::new(fav_text_x, fav_text_y), &fav_name, FontId::proportional(11.0), text_secondary, fav_max_w);

                    if response.clicked() {
                        state.current_folder = Some(fav_path.clone());
                    }
                }

                // Separator after favorites (only if there are items)
                if !favorites_snapshot.is_empty() {
                    ui.add_space(2.0);
                    let sep_rect = ui.allocate_space(egui::vec2(ui.available_width(), 1.0)).1;
                    ui.painter().hline(
                        (sep_rect.min.x + 6.0)..=(sep_rect.max.x - 6.0),
                        sep_rect.center().y,
                        egui::Stroke::new(1.0, theme.widgets.border.to_color32()),
                    );
                    ui.add_space(2.0);
                }
            }

            // Root node
            let is_expanded = state.expanded_folders.contains(&root);
            let is_current = state.current_folder.as_ref() == Some(&root);
            let is_drop_target = state.drop_target_folder.as_ref() == Some(&root);

            let icon = if is_expanded { FOLDER_OPEN } else { HOUSE };
            let color = folder_icon_color(&root_name);

            let (clicked, right_clicked, _drag_started, row_rect) = render_folder_row(
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
            if right_clicked {
                state.selected_assets.clear();
                state.selected_assets.insert(root.clone());
                state.selected_path = Some(root.clone());
                state.context_menu_pos = ui.ctx().pointer_latest_pos();
            }

            if is_expanded {
                let root_owned = root.clone();
                render_folder_children(ui, state, &root_owned, 1, theme);
                // The root folder's own files are missed by `render_folder_children`
                // (which only iterates subfolders), so render them here too.
                if state.tree_show_files {
                    render_folder_files(ui, state, &root_owned, 1, theme);
                }
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

        // Drop on release — only consume if dropping on a tree folder
        if ctx.input(|i| i.pointer.any_released()) {
            if let Some(target) = tree_drop_target {
                state.pending_move = Some((state.drag_moving.clone(), target));
                state.drag_moving.clear();
                state.move_drop_target = None;
            }
            // If no tree target, leave drag_moving for grid/list to handle
        }
    }

    state.pending_drag_payload.take()
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
        let is_current = state.current_folder.as_ref() == Some(folder)
            || state.selected_assets.contains(folder);
        let is_drop_target = state.drop_target_folder.as_ref() == Some(folder);
        let (icon, color) = get_folder_icon(is_expanded, &name);
        state.visible_item_order.push(folder.clone());

        let (clicked, right_clicked, drag_started, row_rect) = render_folder_row(
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

        let (ctrl_held, shift_held) =
            ui.ctx().input(|i| (i.modifiers.ctrl, i.modifiers.shift));

        if clicked {
            if shift_held {
                // Shift-click on a folder selects the folder + every
                // descendant (files and subfolders), so the user can grab a
                // whole branch in one click and drag it.
                state.selected_assets.insert(folder.clone());
                collect_descendants_into(folder, &mut state.selected_assets);
                state.selection_anchor = Some(folder.clone());
                state.selected_path = Some(folder.clone());
            } else if ctrl_held {
                state.handle_click(folder, true, false);
            } else {
                toggle_expanded(&mut state.expanded_folders, folder);
                state.current_folder = Some(folder.clone());
            }
        }

        if right_clicked {
            // Selecting a folder via right-click matches grid behavior so the
            // context menu's actions (rename / delete / etc.) target this row.
            // If the folder is already part of a multi-selection, leave the
            // selection alone so the menu can act on the whole set.
            if !state.selected_assets.contains(folder) {
                state.selected_assets.clear();
                state.selected_assets.insert(folder.clone());
                state.selected_path = Some(folder.clone());
            }
            state.context_menu_pos = ui.ctx().pointer_latest_pos();
        }

        if drag_started {
            // If the dragged folder is part of a multi-selection, drag the
            // whole selection together; otherwise just this folder.
            if state.selected_assets.contains(folder) && state.selected_assets.len() > 1 {
                state.drag_moving = state.selected_assets.iter().cloned().collect();
            } else {
                state.drag_moving = vec![folder.clone()];
            }

            let origin = ui.ctx().pointer_latest_pos().unwrap_or_default();
            state.pending_drag_payload = Some(renzora::editor::AssetDragPayload {
                path: folder.clone(),
                name: name.clone(),
                icon: icon.to_string(),
                color,
                origin,
                is_detached: false,
                drag_count: state.drag_moving.len(),
            });
        }

        if is_expanded {
            render_folder_children(ui, state, folder, depth + 1, theme);
            if state.tree_show_files {
                render_folder_files(ui, state, folder, depth + 1, theme);
            }
        }
    }
}

/// List non-hidden files inside `parent`, indented at `depth`. Shown only when
/// `state.tree_show_files` is on (narrow tree-only layout).
#[cfg(not(target_arch = "wasm32"))]
fn render_folder_files(
    ui: &mut egui::Ui,
    state: &mut AssetBrowserState,
    parent: &PathBuf,
    depth: usize,
    theme: &Theme,
) {
    let mut files: Vec<PathBuf> = match std::fs::read_dir(parent) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
            .filter(|e| !is_hidden(&e.path()))
            .map(|e| e.path())
            .collect(),
        Err(_) => return,
    };
    files.sort_by(|a, b| {
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

    for file in &files {
        let name = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("???")
            .to_string();

        if !state.search.is_empty()
            && !name.to_lowercase().contains(&state.search.to_lowercase())
        {
            continue;
        }

        let is_selected = state.selected_assets.contains(file);
        state.visible_item_order.push(file.clone());
        let (clicked, drag_started, right_clicked) =
            render_file_row(ui, &name, file, depth, is_selected, theme);

        let (ctrl_held, shift_held) = ui.ctx().input(|i| (i.modifiers.ctrl, i.modifiers.shift));

        if clicked {
            state.handle_click(file, ctrl_held, shift_held);
        }

        if right_clicked {
            // Match grid behavior: right-click on an unselected item picks it
            // first so the menu acts on the right-clicked item, not whatever
            // happened to be selected before.
            if !state.selected_assets.contains(file) {
                state.selected_assets.clear();
                state.selected_assets.insert(file.clone());
                state.selected_path = Some(file.clone());
            }
            state.context_menu_pos = ui.ctx().pointer_latest_pos();
        }

        if drag_started {
            // Match grid behavior: include all selected items if the dragged
            // file is part of the multi-selection, otherwise just this one.
            if state.selected_assets.contains(file) && state.selected_assets.len() > 1 {
                state.drag_moving = state.selected_assets.iter().cloned().collect();
            } else {
                state.drag_moving = vec![file.clone()];
            }

            let (icon, color) = file_icon(file);
            let origin = ui.ctx().pointer_latest_pos().unwrap_or_default();
            state.pending_drag_payload = Some(renzora::editor::AssetDragPayload {
                path: file.clone(),
                name: name.clone(),
                icon: icon.to_string(),
                color,
                origin,
                is_detached: false,
                drag_count: state.drag_moving.len(),
            });
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn render_folder_files(
    _ui: &mut egui::Ui,
    _state: &mut AssetBrowserState,
    _parent: &PathBuf,
    _depth: usize,
    _theme: &Theme,
) {}

/// Render a single file row — no expand arrow, file-type icon. Returns
/// `(clicked, drag_started, right_clicked)` so the caller can wire
/// selection, drag, and context-menu flow.
fn render_file_row(
    ui: &mut egui::Ui,
    name: &str,
    path: &PathBuf,
    depth: usize,
    is_selected: bool,
    theme: &Theme,
) -> (bool, bool, bool) {
    let selection_bg = theme.semantic.selection.to_color32();
    let item_hover = theme.panels.item_hover.to_color32();
    let text_secondary = theme.text.secondary.to_color32();
    let text_muted = theme.text.muted.to_color32();

    let indent = depth as f32 * INDENT;

    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), ROW_HEIGHT),
        Sense::click_and_drag(),
    );

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    let painter = ui.painter();
    if is_selected {
        painter.rect_filled(rect, 2.0, selection_bg);
    } else if response.hovered() {
        painter.rect_filled(rect, 2.0, item_hover);
    }

    // Match the folder-row layout: arrow slot (blank for files) + icon + name.
    let arrow_x = rect.min.x + indent + 8.0;
    let icon_x = arrow_x + 12.0;
    let (file_glyph, file_color) = file_icon(path);
    let _ = text_muted;
    painter.text(
        Pos2::new(icon_x, rect.center().y),
        Align2::LEFT_CENTER,
        file_glyph,
        FontId::proportional(12.0),
        file_color,
    );

    let text_x = icon_x + 16.0;
    let max_text_width = (rect.max.x - text_x - 4.0).max(0.0);
    let text_y = rect.center().y - 11.0 * 0.5;
    paint_truncated_text(
        painter,
        Pos2::new(text_x, text_y),
        name,
        FontId::proportional(11.0),
        text_secondary,
        max_text_width,
    );

    (response.clicked(), response.drag_started(), response.secondary_clicked())
}

/// Render a single folder row. Returns
/// `(clicked, right_clicked, drag_started, row_rect)`.
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
) -> (bool, bool, bool, egui::Rect) {
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

    // Folder name (truncated with ellipsis if too long)
    let text_x = icon_x + 16.0;
    let max_text_width = (rect.max.x - text_x - 4.0).max(0.0);
    let text_y = rect.center().y - 11.0 * 0.5; // vertically center for proportional 11
    paint_truncated_text(painter, Pos2::new(text_x, text_y), name, FontId::proportional(11.0), text_secondary, max_text_width);

    (
        arrow_resp.clicked() || response.clicked(),
        response.secondary_clicked(),
        response.drag_started(),
        rect,
    )
}

/// Recursively add every file and subfolder under `root` to `out`. Used by
/// shift-click on a folder so the user can select a whole branch at once.
fn collect_descendants_into(
    root: &PathBuf,
    out: &mut std::collections::HashSet<PathBuf>,
) {
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if is_hidden(&path) {
            continue;
        }
        if path.is_dir() {
            out.insert(path.clone());
            collect_descendants_into(&path, out);
        } else {
            out.insert(path);
        }
    }
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

/// Paint text truncated with "…" if it exceeds `max_width`.
fn paint_truncated_text(
    painter: &egui::Painter,
    pos: Pos2,
    text: &str,
    font: FontId,
    color: Color32,
    max_width: f32,
) {
    let mut job = egui::text::LayoutJob::single_section(text.to_string(), egui::TextFormat {
        font_id: font,
        color,
        ..Default::default()
    });
    job.wrap = egui::text::TextWrapping {
        max_width,
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('\u{2026}'),
    };
    let galley = painter.layout_job(job);
    painter.galley(pos, galley, Color32::TRANSPARENT);
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
