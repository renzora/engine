use std::path::PathBuf;

use bevy_egui::egui::{self, Align2, Color32, FontId, Stroke, StrokeKind, TextureId};
use egui_phosphor::regular;
use renzora_editor_framework::{split_label_two_lines, AssetDragPayload, TileGrid, TileState};
use renzora_theme::Theme;

use crate::state::{file_icon, folder_icon_color, format_file_size, is_hidden, AssetBrowserState, SortDirection, SortMode};
use crate::thumbnails::supports_thumbnail;

/// Entry in the file grid (folder or file).
pub(crate) struct GridEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
}

/// Result from the grid interaction.
#[derive(Default)]
pub struct GridResult {
    pub drag_payload: Option<AssetDragPayload>,
    /// File path if a non-directory file was double-clicked.
    pub double_clicked_file: Option<PathBuf>,
    /// Image files that need thumbnails loaded (collected during render).
    pub thumbnail_requests: Vec<PathBuf>,
}

/// Lookup for available thumbnails, passed in from the panel.
pub struct ThumbnailLookup {
    /// Returns egui texture ID for a path, if loaded.
    pub ids: std::collections::HashMap<PathBuf, TextureId>,
}

impl ThumbnailLookup {
    pub fn get(&self, path: &PathBuf) -> Option<TextureId> {
        self.ids.get(path).copied()
    }
}

/// Collect and sort directory entries for the current folder.
pub(crate) fn collect_entries(state: &AssetBrowserState) -> Option<Vec<GridEntry>> {
    let folder = state.current_folder.as_ref()?;

    #[cfg(target_arch = "wasm32")]
    let mut entries: Vec<GridEntry> = Vec::new();
    #[cfg(not(target_arch = "wasm32"))]
    let mut entries: Vec<GridEntry> = match std::fs::read_dir(folder) {
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
        Err(_) => return None,
    };

    // Folders always sort before files, then apply sort mode
    let sort_mode = state.sort_mode;
    let sort_dir = state.sort_direction;
    entries.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then_with(|| {
            let cmp = match sort_mode {
                SortMode::Name => {
                    a.name.to_lowercase().cmp(&b.name.to_lowercase())
                }
                SortMode::DateModified => {
                    let time_a = std::fs::metadata(&a.path)
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    let time_b = std::fs::metadata(&b.path)
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    time_a.cmp(&time_b)
                }
                SortMode::Type => {
                    let ext_a = a.path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                    let ext_b = b.path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                    ext_a.cmp(&ext_b).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                }
                SortMode::Size => {
                    let size_a = std::fs::metadata(&a.path).map(|m| m.len()).unwrap_or(0);
                    let size_b = std::fs::metadata(&b.path).map(|m| m.len()).unwrap_or(0);
                    size_a.cmp(&size_b)
                }
            };
            match sort_dir {
                SortDirection::Ascending => cmp,
                SortDirection::Descending => cmp.reverse(),
            }
        })
    });

    // Apply search filter
    let search = state.search.to_lowercase();
    if !search.is_empty() {
        entries.retain(|e| e.name.to_lowercase().contains(&search));
    }

    Some(entries)
}

/// Renders the file grid with multi-selection, marquee, context menu, rename, and delete.
pub fn grid_ui_interactive(
    ui: &mut egui::Ui,
    state: &mut AssetBrowserState,
    theme: &Theme,
    thumbnails: &ThumbnailLookup,
) -> GridResult {
    let entries = match collect_entries(state) {
        Some(e) => e,
        None => {
            if state.current_folder.is_none() {
                renzora_editor_framework::empty_state(
                    ui,
                    regular::FOLDER_OPEN,
                    "No folder selected",
                    "Select a folder from the tree to browse files.",
                    theme,
                );
            } else {
                renzora_editor_framework::empty_state(
                    ui,
                    regular::WARNING,
                    "Cannot read folder",
                    "The selected folder could not be read.",
                    theme,
                );
            }
            return GridResult {
                drag_payload: None,
                double_clicked_file: None,
                thumbnail_requests: Vec::new(),
            };
        }
    };

    if entries.is_empty() {
        let (msg, desc) = if !state.search.is_empty() {
            ("No matches", "Try a different search term.")
        } else {
            ("Empty folder", "This folder has no files or subfolders.")
        };
        renzora_editor_framework::empty_state(ui, regular::FOLDER_OPEN, msg, desc, theme);
        return GridResult {
            drag_payload: None,
            double_clicked_file: None,
            thumbnail_requests: Vec::new(),
        };
    }

    // Build visible_item_order for range selection
    state.visible_item_order.clear();
    for entry in &entries {
        state.visible_item_order.push(entry.path.clone());
    }

    // Clear item rects for marquee hit testing
    state.item_rects.clear();

    let ctx = ui.ctx().clone();

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
    let zoom = state.zoom;
    let accent_color = theme.semantic.accent.to_color32();

    let ctrl_held = ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
    let shift_held = ctx.input(|i| i.modifiers.shift);

    let mut clicked_path: Option<PathBuf> = None;
    let mut double_clicked_index: Option<usize> = None;
    let mut drag_started_index: Option<usize> = None;
    let mut thumbnail_requests: Vec<PathBuf> = Vec::new();
    let mut right_clicked = false;
    let mut pending_rename_rect: Option<egui::Rect> = None;
    let mut pending_rename_font: f32 = 11.0;
    let mut drop_target_folder: Option<PathBuf> = None;

    // The visible grid pane rect (used for hit-testing pointer vs grid area)
    let grid_pane_rect = ui.max_rect();

    egui::ScrollArea::vertical()
        .id_salt("asset_grid")
        .auto_shrink([false, false])
        .drag_to_scroll(false)
        .show(ui, |ui| {
            ui.add_space(5.0);
            let grid = TileGrid::new(theme)
                .zoom(zoom)
                .available_width(ui.available_width());

            let tile_size = grid.tile_size();

            grid.show(ui, entries.len(), |ui, index, tile| {
                let entry = &entries[index];
                let is_selected = state.selected_assets.contains(&entry.path);
                let is_hovered = tile.response.hovered();

                // Track item rect for marquee
                state.item_rects.push((entry.path.clone(), tile.rect));

                // Click detection
                if tile.response.clicked() {
                    clicked_path = Some(entry.path.clone());
                }
                if tile.response.double_clicked() {
                    double_clicked_index = Some(index);
                }
                // Right-click for context menu
                if tile.response.secondary_clicked() {
                    right_clicked = true;
                    // If right-clicking on unselected item, select it
                    if !is_selected {
                        state.selected_assets.clear();
                        state.selected_assets.insert(entry.path.clone());
                        state.selected_path = Some(entry.path.clone());
                        state.selection_anchor = Some(entry.path.clone());
                    }
                }
                // Drag detection — files and folders
                if tile.response.drag_started() {
                    drag_started_index = Some(index);
                }

                // Drop target: folder tiles under the pointer during an active drag
                // Can't use response.hovered() because egui gives hover to the drag source
                if entry.is_dir && !state.drag_moving.is_empty() && !state.drag_moving.contains(&entry.path) {
                    let pointer_over = ctx.input(|i| i.pointer.hover_pos().or(i.pointer.latest_pos()))
                        .map(|p| tile.rect.contains(p))
                        .unwrap_or(false);
                    if pointer_over {
                        drop_target_folder = Some(entry.path.clone());
                        let accent = theme.semantic.accent.to_color32();
                        ui.painter().rect_filled(
                            tile.rect,
                            8.0,
                            Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 40),
                        );
                        ui.painter().rect_stroke(
                            tile.rect,
                            8.0,
                            Stroke::new(2.0, accent),
                            StrokeKind::Inside,
                        );
                    }
                }

                let (icon, color) = if entry.is_dir {
                    (regular::FOLDER, folder_icon_color(&entry.name))
                } else {
                    file_icon(&entry.path)
                };

                // Inline rename UI
                let is_renaming = state.renaming_asset.as_ref() == Some(&entry.path);
                if is_renaming {
                    // Stash rename info to render outside the grid layout
                    pending_rename_rect = Some(tile.label_rect);
                    pending_rename_font = tile.font_size;
                }

                // Try to render an image thumbnail for supported file types
                let mut drew_thumbnail = false;
                if !entry.is_dir && supports_thumbnail(&entry.name) {
                    if let Some(tex_id) = thumbnails.get(&entry.path) {
                        let uv = egui::Rect::from_min_max(
                            egui::pos2(0.0, 0.0),
                            egui::pos2(1.0, 1.0),
                        );
                        ui.painter().image(
                            tex_id,
                            tile.thumbnail_rect,
                            uv,
                            egui::Color32::WHITE,
                        );
                        drew_thumbnail = true;
                    } else {
                        thumbnail_requests.push(entry.path.clone());
                    }
                }

                if !drew_thumbnail {
                    ui.painter().text(
                        tile.icon_rect.center(),
                        Align2::CENTER_CENTER,
                        icon,
                        FontId::proportional(tile.icon_size),
                        color,
                    );
                }

                // Star badge on favorited folders
                if entry.is_dir && state.is_favorite(&entry.path) {
                    let star_pos = egui::pos2(tile.rect.max.x - 10.0, tile.rect.min.y + 10.0);
                    ui.painter().text(
                        star_pos,
                        Align2::CENTER_CENTER,
                        regular::STAR,
                        FontId::proportional(10.0),
                        Color32::from_rgb(255, 200, 60),
                    );
                }

                // Draw label (skip if renaming)
                if !is_renaming {
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
                }

                // File hover tooltip (suppress during drag)
                if !entry.is_dir && tile.response.hovered() && state.drag_moving.is_empty() {
                    tile.response.clone().on_hover_ui_at_pointer(|ui| {
                        file_hover_tooltip(ui, &entry.path);
                    });
                }

                TileState {
                    is_selected,
                    is_hovered,
                    color: Some(color),
                }
            });
        });

    // Right-click in empty space
    if !right_clicked {
        if let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) {
            if ctx.input(|i| i.pointer.secondary_clicked()) && grid_pane_rect.contains(pos) {
                let on_item = state.item_rects.iter().any(|(_, r)| r.contains(pos));
                if !on_item {
                    right_clicked = true;
                    state.clear_selection();
                }
            }
        }
    }

    // --- Inline rename (rendered outside grid layout to avoid breaking flow) ---
    if let Some(rename_rect) = pending_rename_rect {
        let rename_id = egui::Id::new("asset_grid_rename_input");
        let mut text = state.rename_buffer.clone();
        let area_resp = egui::Area::new(rename_id.with("area"))
            .fixed_pos(rename_rect.min)
            .order(egui::Order::Foreground)
            .show(&ctx, |ui| {
                ui.set_max_width(rename_rect.width());
                ui.add(
                    egui::TextEdit::singleline(&mut text)
                        .font(FontId::proportional(pending_rename_font))
                        .desired_width(rename_rect.width() - 8.0)
                        .id(rename_id),
                )
            });
        let resp = area_resp.inner;
        state.rename_buffer = text;

        if !state.rename_focus_set {
            resp.request_focus();
            state.rename_focus_set = true;
        }

        if resp.lost_focus() {
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                // Confirm rename
                let new_name = state.rename_buffer.trim().to_string();
                if let Some(ref renaming) = state.renaming_asset {
                    let old_name = renaming.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !new_name.is_empty() && new_name != old_name {
                        state.pending_rename = Some((renaming.clone(), new_name));
                    }
                }
            }
            // Cancel on click-away or Escape — discard changes
            state.renaming_asset = None;
        }
    }

    if right_clicked {
        state.context_menu_pos = ctx.pointer_latest_pos();
    }

    // --- Marquee selection ---
    let primary_down = ctx.input(|i| i.pointer.primary_down());
    let primary_pressed = ctx.input(|i| i.pointer.primary_pressed());
    let primary_clicked = ctx.input(|i| i.pointer.primary_clicked());

    // Use hover_pos for position checks (works even when scroll area captures the drag)
    let pointer_pos = ctx.input(|i| i.pointer.hover_pos());

    // Check if press is on empty space (not over an item)
    let press_on_empty = if let Some(press_pos) = pointer_pos {
        if grid_pane_rect.contains(press_pos) {
            !state.item_rects.iter().any(|(_, r)| r.contains(press_pos))
        } else {
            false
        }
    } else {
        false
    };

    // Click on empty space to deselect (not during marquee)
    if primary_clicked && press_on_empty && state.marquee_start.is_none() {
        if !ctrl_held && !shift_held {
            state.clear_selection();
        }
    }

    // Start marquee on primary press in empty space
    if primary_pressed && state.marquee_start.is_none() && press_on_empty {
        state.marquee_start = pointer_pos;
        // Save current selection so we can restore it for items that leave the marquee
        if !ctrl_held && !shift_held {
            state.selected_assets.clear();
            state.pre_marquee_selection.clear();
        } else {
            state.pre_marquee_selection = state.selected_assets.clone();
        }
    }

    // Update marquee during drag
    if primary_down && state.marquee_start.is_some() {
        state.marquee_current = pointer_pos;
    }

    // Draw marquee rectangle on foreground layer and select intersecting items
    if let (Some(start), Some(current)) = (state.marquee_start, state.marquee_current) {
        let marquee_rect = egui::Rect::from_two_pos(start, current);

        // Paint on foreground layer so it's never clipped by scroll area
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("asset_marquee"),
        ));

        // Semi-transparent fill
        painter.rect_filled(
            marquee_rect,
            0.0,
            Color32::from_rgba_unmultiplied(100, 150, 255, 40),
        );
        // Border
        painter.rect_stroke(
            marquee_rect,
            0.0,
            Stroke::new(1.0, accent_color),
            StrokeKind::Inside,
        );

        // Recompute selection: pre-marquee selection + items currently intersecting
        state.selected_assets = state.pre_marquee_selection.clone();
        for (path, item_rect) in &state.item_rects {
            if marquee_rect.intersects(*item_rect) {
                state.selected_assets.insert(path.clone());
            }
        }
    }

    // End marquee on pointer release
    if ctx.input(|i| i.pointer.any_released()) {
        if state.marquee_start.is_some() {
            state.marquee_start = None;
            state.marquee_current = None;
            state.pre_marquee_selection.clear();
        }
    }

    // --- Process click interactions ---
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

    // Build drag payload if a file drag started
    let mut drag_payload = None;
    if let Some(idx) = drag_started_index {
        let entry = &entries[idx];

        // Start internal drag-move: include all selected items, or just the dragged one
        if state.selected_assets.contains(&entry.path) && state.selected_assets.len() > 1 {
            state.drag_moving = state.selected_assets.iter().cloned().collect();
        } else {
            state.drag_moving = vec![entry.path.clone()];
        }

        let (icon, color) = if entry.is_dir {
            (regular::FOLDER, folder_icon_color(&entry.name))
        } else {
            file_icon(&entry.path)
        };
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

    // Update drop target for ghost label
    if !state.drag_moving.is_empty() {
        state.move_drop_target = drop_target_folder.clone();
    }

    // Handle drop on a folder tile
    if !state.drag_moving.is_empty() && ctx.input(|i| i.pointer.any_released()) {
        if let Some(target) = drop_target_folder {
            state.pending_move = Some((state.drag_moving.clone(), target));
        }
        state.drag_moving.clear();
        state.move_drop_target = None;
    }

    GridResult {
        drag_payload,
        double_clicked_file,
        thumbnail_requests,
    }
}

/// Render a tooltip with file info (name, type, size, date modified).
pub(crate) fn file_hover_tooltip(ui: &mut egui::Ui, path: &std::path::Path) {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");
    ui.label(egui::RichText::new(name).strong());

    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        ui.label(format!("Type: {}", ext.to_uppercase()));
    }

    if let Ok(meta) = std::fs::metadata(path) {
        ui.label(format!("Size: {}", format_file_size(meta.len())));
        if let Ok(modified) = meta.modified() {
            if let Ok(duration) = modified.duration_since(std::time::SystemTime::UNIX_EPOCH) {
                let secs = duration.as_secs();
                // Simple date formatting: YYYY-MM-DD HH:MM
                let days = secs / 86400;
                let time_of_day = secs % 86400;
                let hours = time_of_day / 3600;
                let minutes = (time_of_day % 3600) / 60;

                // Approximate date from days since epoch
                let (year, month, day) = days_to_date(days);
                ui.label(format!("Modified: {}-{:02}-{:02} {:02}:{:02}", year, month, day, hours, minutes));
            }
        }
    }
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_date(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let month_days: &[u64] = if is_leap(year) {
        &[31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        &[31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1;
    for &md in month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}
