use std::path::PathBuf;

use bevy_egui::egui::{self, Sense};
use egui_phosphor::regular;
use renzora_editor::AssetDragPayload;
use renzora_theme::Theme;

use crate::grid::GridResult;
use crate::state::{file_icon, folder_icon_color, is_hidden, AssetBrowserState};

/// Entry in the file list (folder or file).
struct ListEntry {
    path: PathBuf,
    name: String,
    is_dir: bool,
}

const ROW_HEIGHT: f32 = 22.0;

/// Renders the file list with click handling.
pub fn list_ui_interactive(ui: &mut egui::Ui, state: &mut AssetBrowserState, theme: &Theme) -> GridResult {
    let folder = match state.current_folder.clone() {
        Some(f) => f,
        None => {
            renzora_editor::empty_state(
                ui,
                regular::FOLDER_OPEN,
                "No folder selected",
                "Select a folder from the tree to browse files.",
                theme,
            );
            return GridResult { drag_payload: None, double_clicked_file: None, thumbnail_requests: Vec::new() };
        }
    };

    // Collect and sort entries
    #[cfg(target_arch = "wasm32")]
    let mut entries: Vec<ListEntry> = Vec::new();
    #[cfg(not(target_arch = "wasm32"))]
    let mut entries: Vec<ListEntry> = match std::fs::read_dir(&folder) {
        Ok(iter) => iter
            .filter_map(|e| e.ok())
            .filter(|e| !is_hidden(&e.path()))
            .map(|e| {
                let is_dir = e.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                let name = e.file_name().to_string_lossy().to_string();
                ListEntry {
                    path: e.path(),
                    name,
                    is_dir,
                }
            })
            .collect(),
        Err(_) => {
            renzora_editor::empty_state(
                ui,
                regular::WARNING,
                "Cannot read folder",
                "The selected folder could not be read.",
                theme,
            );
            return GridResult { drag_payload: None, double_clicked_file: None, thumbnail_requests: Vec::new() };
        }
    };

    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    // Apply search filter
    let search = state.search.to_lowercase();
    if !search.is_empty() {
        entries.retain(|e| e.name.to_lowercase().contains(&search));
    }

    if entries.is_empty() {
        let (msg, desc) = if !search.is_empty() {
            ("No matches", "Try a different search term.")
        } else {
            ("Empty folder", "This folder has no files or subfolders.")
        };
        renzora_editor::empty_state(ui, regular::FOLDER_OPEN, msg, desc, theme);
        return GridResult { drag_payload: None, double_clicked_file: None, thumbnail_requests: Vec::new() };
    }

    let text_primary = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let selected = state.selected_path.clone();

    let mut clicked_index: Option<usize> = None;
    let mut double_clicked_index: Option<usize> = None;
    let mut drag_started_index: Option<usize> = None;

    egui::ScrollArea::vertical()
        .id_salt("asset_list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(2.0);

            for (index, entry) in entries.iter().enumerate() {
                let is_selected = selected.as_ref() == Some(&entry.path);

                let (icon, color) = if entry.is_dir {
                    (regular::FOLDER, folder_icon_color(&entry.name))
                } else {
                    file_icon(&entry.path)
                };

                let row_rect = ui.allocate_space(egui::vec2(ui.available_width(), ROW_HEIGHT)).1;
                let resp = ui.interact(row_rect, ui.id().with(index), Sense::click_and_drag());

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

                // Name
                let name_pos = egui::pos2(row_rect.min.x + 28.0, row_rect.center().y);
                ui.painter().text(
                    name_pos,
                    egui::Align2::LEFT_CENTER,
                    &entry.name,
                    egui::FontId::proportional(12.0),
                    text_primary,
                );

                // Extension label for files
                if !entry.is_dir {
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
                    clicked_index = Some(index);
                }
                if resp.double_clicked() {
                    double_clicked_index = Some(index);
                }
                if !entry.is_dir && resp.drag_started() {
                    drag_started_index = Some(index);
                }
            }
        });

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
        state.selected_path = Some(entries[idx].path.clone());
    } else if let Some(idx) = clicked_index {
        state.selected_path = Some(entries[idx].path.clone());
    }

    let drag_payload = drag_started_index.map(|idx| {
        let entry = &entries[idx];
        let (icon, color) = file_icon(&entry.path);
        let origin = ui.ctx().pointer_latest_pos().unwrap_or_default();
        AssetDragPayload {
            path: entry.path.clone(),
            name: entry.name.clone(),
            icon: icon.to_string(),
            color,
            origin,
            is_detached: false,
        }
    });

    GridResult { drag_payload, double_clicked_file, thumbnail_requests: Vec::new() }
}
