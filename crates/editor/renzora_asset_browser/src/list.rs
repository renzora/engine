use std::path::PathBuf;

use bevy_egui::egui::{self, Sense};
use egui_phosphor::regular;
use renzora_editor::AssetDragPayload;
use renzora_theme::Theme;

use crate::grid::{collect_entries, GridResult};
use crate::state::{file_icon, folder_icon_color, AssetBrowserState};

const ROW_HEIGHT: f32 = 22.0;

/// Renders the file list with multi-selection, context menu, rename, and delete.
pub fn list_ui_interactive(ui: &mut egui::Ui, state: &mut AssetBrowserState, theme: &Theme) -> GridResult {
    let entries = match collect_entries(state) {
        Some(e) => e,
        None => {
            if state.current_folder.is_none() {
                renzora_editor::empty_state(
                    ui,
                    regular::FOLDER_OPEN,
                    "No folder selected",
                    "Select a folder from the tree to browse files.",
                    theme,
                );
            } else {
                renzora_editor::empty_state(
                    ui,
                    regular::WARNING,
                    "Cannot read folder",
                    "The selected folder could not be read.",
                    theme,
                );
            }
            return GridResult { drag_payload: None, double_clicked_file: None, thumbnail_requests: Vec::new() };
        }
    };

    if entries.is_empty() {
        let (msg, desc) = if !state.search.is_empty() {
            ("No matches", "Try a different search term.")
        } else {
            ("Empty folder", "This folder has no files or subfolders.")
        };
        renzora_editor::empty_state(ui, regular::FOLDER_OPEN, msg, desc, theme);
        return GridResult { drag_payload: None, double_clicked_file: None, thumbnail_requests: Vec::new() };
    }

    // Build visible_item_order for range selection
    state.visible_item_order.clear();
    for entry in &entries {
        state.visible_item_order.push(entry.path.clone());
    }

    // Clear item rects for marquee hit testing
    state.item_rects.clear();

    let ctx = ui.ctx().clone();
    let ctrl_held = ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
    let shift_held = ctx.input(|i| i.modifiers.shift);

    // F2 to start rename (exactly one item selected)
    if ctx.input(|i| i.key_pressed(egui::Key::F2)) && state.renaming_asset.is_none() {
        if state.selected_assets.len() == 1 {
            if let Some(path) = state.selected_assets.iter().next() {
                state.renaming_asset = Some(path.clone());
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    state.rename_buffer = name.to_string();
                }
                state.rename_focus_set = false;
            }
        }
    }

    // Delete key — only when this panel area has focus
    if ui.ui_contains_pointer() && ui.input(|i| i.key_pressed(egui::Key::Delete)) && !state.selected_assets.is_empty() {
        state.pending_delete = state.selected_assets.iter().cloned().collect();
    }

    // Ctrl+D to duplicate
    if ctx.input(|i| (i.modifiers.ctrl || i.modifiers.command) && i.key_pressed(egui::Key::D)) && !state.selected_assets.is_empty() {
        ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::D));
        ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::D));
        state.duplicate_selected();
    }

    // Escape to cancel rename or close context menu
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        state.renaming_asset = None;
        state.context_menu_pos = None;
    }

    let text_primary = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();

    let mut clicked_path: Option<PathBuf> = None;
    let mut double_clicked_index: Option<usize> = None;
    let mut drag_started_index: Option<usize> = None;
    let mut right_clicked = false;
    let mut drop_target_folder: Option<PathBuf> = None;

    egui::ScrollArea::vertical()
        .id_salt("asset_list")
        .auto_shrink([false, false])
        .drag_to_scroll(false)
        .show(ui, |ui| {
            ui.add_space(2.0);

            for (index, entry) in entries.iter().enumerate() {
                let is_selected = state.selected_assets.contains(&entry.path);

                let (icon, color) = if entry.is_dir {
                    (regular::FOLDER, folder_icon_color(&entry.name))
                } else {
                    file_icon(&entry.path)
                };

                let row_rect = ui.allocate_space(egui::vec2(ui.available_width(), ROW_HEIGHT)).1;
                let resp = ui.interact(row_rect, ui.id().with(index), Sense::click_and_drag());

                // Track for marquee
                state.item_rects.push((entry.path.clone(), row_rect));

                // Background highlight
                if is_selected {
                    ui.painter().rect_filled(
                        row_rect,
                        2.0,
                        theme.semantic.accent.to_color32().linear_multiply(0.15),
                    );
                } else if resp.hovered() {
                    ui.painter().rect_filled(
                        row_rect,
                        2.0,
                        theme.widgets.border.to_color32().linear_multiply(0.3),
                    );
                }

                // Icon
                let icon_pos = egui::pos2(row_rect.min.x + 8.0, row_rect.center().y);
                ui.painter().text(
                    icon_pos,
                    egui::Align2::LEFT_CENTER,
                    icon,
                    egui::FontId::proportional(13.0),
                    color,
                );

                // Inline rename
                let is_renaming = state.renaming_asset.as_ref() == Some(&entry.path);
                if is_renaming {
                    let rename_rect = egui::Rect::from_min_max(
                        egui::pos2(row_rect.min.x + 28.0, row_rect.min.y + 1.0),
                        egui::pos2(row_rect.max.x - 8.0, row_rect.max.y - 1.0),
                    );
                    let rename_id = ui.id().with("rename_input");
                    let mut text = state.rename_buffer.clone();
                    let resp = ui.put(
                        rename_rect,
                        egui::TextEdit::singleline(&mut text)
                            .font(egui::FontId::proportional(12.0))
                            .desired_width(rename_rect.width())
                            .id(rename_id),
                    );
                    state.rename_buffer = text;

                    if !state.rename_focus_set {
                        resp.request_focus();
                        state.rename_focus_set = true;
                    }

                    if resp.lost_focus() {
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            let new_name = state.rename_buffer.trim().to_string();
                            if !new_name.is_empty() && new_name != entry.name {
                                state.pending_rename = Some((entry.path.clone(), new_name));
                            }
                        }
                        state.renaming_asset = None;
                    }
                } else {
                    // Name
                    let name_pos = egui::pos2(row_rect.min.x + 28.0, row_rect.center().y);
                    ui.painter().text(
                        name_pos,
                        egui::Align2::LEFT_CENTER,
                        &entry.name,
                        egui::FontId::proportional(12.0),
                        text_primary,
                    );
                }

                // Extension label for files
                if !entry.is_dir && !is_renaming {
                    if let Some(ext) = entry.path.extension().and_then(|e| e.to_str()) {
                        let ext_pos = egui::pos2(row_rect.max.x - 8.0, row_rect.center().y);
                        ui.painter().text(
                            ext_pos,
                            egui::Align2::RIGHT_CENTER,
                            ext.to_uppercase(),
                            egui::FontId::proportional(10.0),
                            text_muted,
                        );
                    }
                }

                if resp.clicked() {
                    clicked_path = Some(entry.path.clone());
                }
                if resp.double_clicked() {
                    double_clicked_index = Some(index);
                }
                if resp.secondary_clicked() {
                    right_clicked = true;
                    if !is_selected {
                        state.selected_assets.clear();
                        state.selected_assets.insert(entry.path.clone());
                        state.selected_path = Some(entry.path.clone());
                        state.selection_anchor = Some(entry.path.clone());
                    }
                }
                if resp.drag_started() {
                    drag_started_index = Some(index);
                }

                // Drop target: folder rows under the pointer during an active drag
                if entry.is_dir && !state.drag_moving.is_empty() && !state.drag_moving.contains(&entry.path) {
                    let pointer_over = ctx.input(|i| i.pointer.hover_pos().or(i.pointer.latest_pos()))
                        .map(|p| row_rect.contains(p))
                        .unwrap_or(false);
                    if pointer_over {
                        drop_target_folder = Some(entry.path.clone());
                        let accent = theme.semantic.accent.to_color32();
                        ui.painter().rect_filled(
                            row_rect,
                            2.0,
                            egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 40),
                        );
                        ui.painter().rect_stroke(
                            row_rect,
                            2.0,
                            egui::Stroke::new(1.5, accent),
                            egui::StrokeKind::Inside,
                        );
                    }
                }
            }
        });

    if right_clicked {
        state.context_menu_pos = ctx.pointer_latest_pos();
    }

    // Process interactions
    let mut double_clicked_file = None;
    if let Some(idx) = double_clicked_index {
        let entry = &entries[idx];
        if entry.is_dir {
            let path = entry.path.clone();
            state.navigate_to(path.clone());
            state.expanded_folders.insert(path);
        } else {
            double_clicked_file = Some(entry.path.clone());
        }
        state.selected_assets.clear();
        state.selected_assets.insert(entries[idx].path.clone());
        state.selected_path = Some(entries[idx].path.clone());
    } else if let Some(ref path) = clicked_path {
        state.handle_click(path, ctrl_held, shift_held);
    }

    let mut drag_payload = None;
    if let Some(idx) = drag_started_index {
        let entry = &entries[idx];
        if state.selected_assets.contains(&entry.path) && state.selected_assets.len() > 1 {
            state.drag_moving = state.selected_assets.iter().cloned().collect();
        } else {
            state.drag_moving = vec![entry.path.clone()];
        }
        if !entry.is_dir {
            let (icon, color) = file_icon(&entry.path);
            let origin = ui.ctx().pointer_latest_pos().unwrap_or_default();
            drag_payload = Some(AssetDragPayload {
                path: entry.path.clone(),
                name: entry.name.clone(),
                icon: icon.to_string(),
                color,
                origin,
                is_detached: false,
                drag_count: state.drag_moving.len(),
            });
        }
    }

    // Update drop target for ghost label
    if !state.drag_moving.is_empty() {
        state.move_drop_target = drop_target_folder.clone();
    }

    // Handle drop on a folder row
    if !state.drag_moving.is_empty() && ctx.input(|i| i.pointer.any_released()) {
        if let Some(target) = drop_target_folder {
            state.pending_move = Some((state.drag_moving.clone(), target));
        }
        state.drag_moving.clear();
        state.move_drop_target = None;
    }

    GridResult { drag_payload, double_clicked_file, thumbnail_requests: Vec::new() }
}
