use std::path::PathBuf;

use bevy_egui::egui::{self, Align2, FontId};
use egui_phosphor::regular;
use renzora_editor::{split_label_two_lines, TileGrid, TileState};
use renzora_theme::Theme;

use crate::state::{file_icon, folder_icon_color, is_hidden, AssetBrowserState};

/// Entry in the file grid (folder or file).
struct GridEntry {
    path: PathBuf,
    name: String,
    is_dir: bool,
}

/// Renders the file grid with click handling.
pub fn grid_ui_interactive(ui: &mut egui::Ui, state: &mut AssetBrowserState, theme: &Theme) {
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
            return;
        }
    };

    // Collect and sort entries
    #[cfg(target_arch = "wasm32")]
    let mut entries: Vec<GridEntry> = Vec::new();
    #[cfg(not(target_arch = "wasm32"))]
    let mut entries: Vec<GridEntry> = match std::fs::read_dir(&folder) {
        Ok(iter) => iter
            .filter_map(|e| e.ok())
            .filter(|e| !is_hidden(&e.path()))
            .map(|e| {
                let is_dir = e.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                let name = e.file_name().to_string_lossy().to_string();
                GridEntry {
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
            return;
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
        return;
    }

    let text_primary = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let zoom = state.zoom;
    let selected = state.selected_path.clone();

    // Track which entry was clicked/double-clicked
    let mut clicked_index: Option<usize> = None;
    let mut double_clicked_index: Option<usize> = None;

    egui::ScrollArea::vertical()
        .id_salt("asset_grid")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(4.0);

            let grid = TileGrid::new(theme)
                .zoom(zoom)
                .available_width(ui.available_width());

            let tile_size = grid.tile_size();

            grid.show(ui, entries.len(), |ui, index, tile| {
                let entry = &entries[index];
                let is_selected = selected.as_ref() == Some(&entry.path);
                let is_hovered = tile.response.hovered();

                // Click detection
                if tile.response.clicked() {
                    clicked_index = Some(index);
                }
                if tile.response.double_clicked() {
                    double_clicked_index = Some(index);
                }

                let (icon, color) = if entry.is_dir {
                    (regular::FOLDER, folder_icon_color(&entry.name))
                } else {
                    file_icon(&entry.path)
                };

                // Draw icon
                ui.painter().text(
                    tile.icon_rect.center(),
                    Align2::CENTER_CENTER,
                    icon,
                    FontId::proportional(tile.icon_size),
                    color,
                );

                // Draw label
                let (line1, line2) =
                    split_label_two_lines(&entry.name, tile_size, tile.font_size);
                ui.painter().text(
                    tile.label_line1_pos(),
                    Align2::CENTER_CENTER,
                    &line1,
                    FontId::proportional(tile.font_size),
                    text_primary,
                );
                if !line2.is_empty() {
                    ui.painter().text(
                        tile.label_line2_pos(),
                        Align2::CENTER_CENTER,
                        &line2,
                        FontId::proportional(tile.font_size),
                        text_muted,
                    );
                }

                TileState {
                    is_selected,
                    is_hovered,
                    color: Some(color),
                }
            });
        });

    // Process interactions after rendering
    if let Some(idx) = double_clicked_index {
        let entry = &entries[idx];
        if entry.is_dir {
            let path = entry.path.clone();
            state.navigate_to(path.clone());
            // Also expand in tree
            state.expanded_folders.insert(path);
        }
        // Single select on double-click too
        state.selected_path = Some(entries[idx].path.clone());
    } else if let Some(idx) = clicked_index {
        state.selected_path = Some(entries[idx].path.clone());
    }
}
