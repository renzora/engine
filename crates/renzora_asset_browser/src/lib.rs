#![allow(deprecated)] // egui API rename pending; will migrate at next bevy_egui bump.

mod grid;
mod list;
mod state;
pub mod thumbnails;
mod toolbar;
mod tree;

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, FontId, Sense, Stroke, Vec2};
use egui_phosphor::regular;
use renzora_editor_framework::{AppEditorExt, EditorCommands, EditorPanel, MaterialThumbnailRegistry, PanelLocation};
use renzora_theme::ThemeManager;

use state::{AssetBrowserState, ViewMode};

/// Panel that provides the asset browser UI.
pub struct AssetBrowserPanel {
    state: RwLock<AssetBrowserState>,
}

impl Default for AssetBrowserPanel {
    fn default() -> Self {
        Self {
            state: RwLock::new(AssetBrowserState::default()),
        }
    }
}

impl EditorPanel for AssetBrowserPanel {
    fn id(&self) -> &str {
        "assets"
    }

    fn title(&self) -> &str {
        "Assets"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::FOLDER_OPEN)
    }

    fn closable(&self) -> bool {
        true
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Bottom
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };

        let mut state = self.state.write().unwrap();

        // Use project directory if available
        if let Some(project) = world.get_resource::<renzora::core::CurrentProject>() {
            if state.project_root.as_ref() != Some(&project.path) {
                state.project_root = Some(project.path.clone());
                state.current_folder = Some(project.path.clone());
                state.load_favorites();
                state.load_recent();
            }
        }

        // Sync extension allow-list from the global filter resource each frame.
        let filter_now = world
            .get_resource::<renzora_editor_framework::AssetBrowserExtensionFilter>()
            .and_then(|r| r.0.clone());
        if state.extension_filter != filter_now {
            state.extension_filter = filter_now;
        }

        // Initialize current folder on first render
        if state.current_folder.is_none() {
            let root = state.root();
            state.current_folder = Some(root);
        }

        let available = ui.available_rect_before_wrap();

        // Toolbar
        let toolbar_height = 28.0;
        let toolbar_rect =
            egui::Rect::from_min_size(available.min, egui::vec2(available.width(), toolbar_height));
        let mut toolbar_ui = ui.new_child(egui::UiBuilder::new().max_rect(toolbar_rect));
        toolbar::toolbar_ui(&mut toolbar_ui, &mut state, &theme);

        // Separator line below toolbar
        let sep_y = available.min.y + toolbar_height;
        ui.painter().hline(
            available.min.x..=available.max.x,
            sep_y,
            Stroke::new(1.0, theme.widgets.border.to_color32()),
        );

        // Content area below toolbar
        let content_top = sep_y + 1.0;
        let content_rect = egui::Rect::from_min_max(
            egui::pos2(available.min.x, content_top),
            available.max,
        );

        if content_rect.height() < 10.0 {
            return;
        }

        // Collapse to tree-only when the panel is too narrow for a usable
        // grid alongside the folder tree. Below the threshold we give the
        // tree the full width and the grid a zero-size rect; downstream
        // drag/overlay code falls back to `content_rect` naturally.
        const TREE_ONLY_WIDTH: f32 = 310.0;
        let tree_only = content_rect.width() < TREE_ONLY_WIDTH;
        state.tree_show_files = tree_only;

        let splitter_width = 4.0;

        let (tree_rect, grid_rect);
        if tree_only {
            tree_rect = content_rect;
            // Zero-width rect docked at the right edge — drag/hover
            // predicates report `false` for any pointer position, which is
            // what we want when there's no visible grid.
            grid_rect = egui::Rect::from_min_max(content_rect.max, content_rect.max);
        } else {
            let tree_width = state
                .tree_width
                .clamp(100.0, (content_rect.width() - 100.0).max(100.0));
            state.tree_width = tree_width;

            tree_rect = egui::Rect::from_min_max(
                content_rect.min,
                egui::pos2(content_rect.min.x + tree_width, content_rect.max.y),
            );
            let splitter_rect = egui::Rect::from_min_max(
                egui::pos2(tree_rect.max.x, content_rect.min.y),
                egui::pos2(tree_rect.max.x + splitter_width, content_rect.max.y),
            );
            grid_rect = egui::Rect::from_min_max(
                egui::pos2(splitter_rect.max.x + 5.0, content_rect.min.y),
                content_rect.max,
            );

            let splitter_id = ui.id().with("asset_splitter");
            let splitter_resp =
                ui.interact(splitter_rect, splitter_id, egui::Sense::click_and_drag());
            if splitter_resp.dragged() {
                state.tree_width = (state.tree_width + splitter_resp.drag_delta().x)
                    .clamp(100.0, (content_rect.width() - 100.0).max(100.0));
            }
            if splitter_resp.hovered() || splitter_resp.dragged() {
                ui.ctx()
                    .set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }

            let splitter_color = if splitter_resp.hovered() || splitter_resp.dragged() {
                theme.semantic.accent.to_color32()
            } else {
                theme.widgets.border.to_color32()
            };
            ui.painter().vline(
                splitter_rect.center().x,
                splitter_rect.y_range(),
                Stroke::new(1.0, splitter_color),
            );
        }

        let mut tree_ui = ui.new_child(egui::UiBuilder::new().max_rect(tree_rect));
        let tree_drag_payload = tree::tree_ui(&mut tree_ui, &mut state, &theme);

        let grid_result = if tree_only {
            grid::GridResult::default()
        } else {
            let thumb_lookup = {
                let cache = world.get_resource::<thumbnails::ThumbnailCache>();
                let mut ids = cache.map(|c| c.texture_id_map()).unwrap_or_default();
                if let Some(registry) = world.get_resource::<MaterialThumbnailRegistry>() {
                    for (path, tid) in registry.entries() {
                        ids.insert(path.clone(), *tid);
                    }
                }
                grid::ThumbnailLookup { ids }
            };

            let mut grid_child =
                ui.new_child(egui::UiBuilder::new().max_rect(grid_rect));
            match state.view_mode {
                ViewMode::Grid => grid::grid_ui_interactive(
                    &mut grid_child,
                    &mut state,
                    &theme,
                    &thumb_lookup,
                ),
                ViewMode::List => list::list_ui_interactive(&mut grid_child, &mut state, &theme),
            }
        };

        // --- File drops from desktop ---
        #[cfg(not(target_arch = "wasm32"))]
        {
            let ctx = ui.ctx().clone();

            // Check if OS is dragging files over the window
            let has_file_hover = ctx.input(|i| !i.raw.hovered_files.is_empty());

            // Collect dropped files early so we can use `has_drops` in position logic
            let dropped: Vec<std::path::PathBuf> = ctx.input(|i| {
                i.raw.dropped_files
                    .iter()
                    .filter_map(|f| f.path.clone())
                    .collect()
            });
            let has_drops = !dropped.is_empty();

            // During OS file drags, pointer position is stale (frozen at pre-drag
            // location) and unreliable for hit-testing. Only trust it on the
            // actual drop frame when the OS sends a fresh cursor position.
            let drag_pos = if has_file_hover {
                None // stale — ignore
            } else {
                ctx.input(|i| i.pointer.hover_pos())
            };

            let over_tree = drag_pos.map(|p| tree_rect.contains(p)).unwrap_or(false);
            let over_grid = drag_pos.map(|p| grid_rect.contains(p)).unwrap_or(false);
            // Accept drops whenever the asset browser is visible — the OS already
            // confirmed the user targeted this window, and pointer position can be
            // unreliable (stale or None) during cross-process drag-and-drop.
            let over_panel = if has_drops {
                true
            } else {
                drag_pos.map(|p| available.contains(p))
                    .unwrap_or(has_file_hover)
            };

            state.drop_hover = has_file_hover && over_panel;

            // Update drop target folder only while files are hovering.
            // Don't clear it when hover ends — the drop handler needs the last
            // hovered folder. It gets cleared after the drop is processed.
            if has_file_hover && over_tree {
                state.drop_target_folder = drag_pos.and_then(|pos| {
                    state.tree_folder_rects.iter()
                        .find(|(_, rect)| rect.contains(pos))
                        .map(|(path, _)| path.clone())
                });
            } else if !has_file_hover && !has_drops {
                // Drag cancelled or ended without a drop — clear target
                state.drop_target_folder = None;
            }

            // Draw drop zone overlays
            if state.drop_hover {
                let painter = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new("asset_drop_hint"),
                ));

                if over_tree {
                    // === Tree drop zone ===
                    // Light tint on the tree pane
                    painter.rect_filled(
                        tree_rect,
                        0.0,
                        Color32::from_rgba_premultiplied(30, 80, 200, 25),
                    );
                    painter.rect_stroke(
                        tree_rect.shrink(1.0),
                        4.0,
                        Stroke::new(2.0, Color32::from_rgb(80, 140, 255)),
                        egui::StrokeKind::Inside,
                    );

                    // Highlight the specific folder row being hovered
                    if let Some(ref target) = state.drop_target_folder {
                        if let Some((_, rect)) = state.tree_folder_rects.iter()
                            .find(|(p, _)| p == target)
                        {
                            painter.rect_filled(
                                *rect,
                                2.0,
                                Color32::from_rgba_premultiplied(80, 140, 255, 60),
                            );
                            painter.rect_stroke(
                                *rect,
                                2.0,
                                Stroke::new(1.5, Color32::from_rgb(80, 140, 255)),
                                egui::StrokeKind::Inside,
                            );
                        }

                        let target_name = target.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("folder");
                        painter.text(
                            egui::pos2(tree_rect.center().x, tree_rect.max.y - 14.0),
                            egui::Align2::CENTER_CENTER,
                            format!("{} \"{}\"", regular::DOWNLOAD_SIMPLE, target_name),
                            FontId::proportional(11.0),
                            Color32::from_rgb(180, 210, 255),
                        );
                    }
                } else {
                    // === Grid/list drop zone — border only, no filled overlay ===
                    // When pointer position is unknown, show overlay on the whole content area
                    let overlay_rect = if over_grid { grid_rect } else { content_rect };
                    painter.rect_stroke(
                        overlay_rect.shrink(3.0),
                        6.0,
                        Stroke::new(2.0, Color32::from_rgb(80, 140, 255)),
                        egui::StrokeKind::Inside,
                    );

                    // Label at the bottom so it doesn't cover content
                    let folder_name = state.current_folder.as_ref()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("current folder");
                    let label_rect = egui::Rect::from_min_size(
                        egui::pos2(overlay_rect.min.x + 8.0, overlay_rect.max.y - 28.0),
                        egui::vec2(overlay_rect.width() - 16.0, 24.0),
                    );
                    painter.rect_filled(
                        label_rect,
                        4.0,
                        Color32::from_rgba_premultiplied(20, 60, 160, 200),
                    );
                    painter.text(
                        label_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("{} Drop files into \"{}\"", regular::DOWNLOAD_SIMPLE, folder_name),
                        FontId::proportional(12.0),
                        Color32::from_rgb(200, 220, 255),
                    );
                }
            }

            if has_drops && over_panel {
                // Use the drop target folder (tree folder hover) or fall back to current folder
                let import_target = state.drop_target_folder.clone()
                    .or_else(|| state.current_folder.clone());

                let mut model_files = Vec::new();
                let mut copy_files = Vec::new();
                let mut zip_files = Vec::new();
                let mut dir_drops = Vec::new();

                for path in dropped {
                    if path.is_dir() {
                        dir_drops.push(path);
                    } else if path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("zip")).unwrap_or(false) {
                        zip_files.push(path);
                    } else if state::is_3d_model(&path) {
                        model_files.push(path);
                    } else if state::is_copyable_asset(&path) {
                        copy_files.push(path);
                    }
                }

                if let Some(ref folder) = import_target {
                    let mut imported = 0usize;

                    // Copy individual files
                    for source_path in &copy_files {
                        let Some(file_name) = source_path.file_name() else {
                            continue;
                        };
                        let dest_path = folder.join(file_name);

                        if source_path == &dest_path {
                            continue;
                        }

                        match std::fs::copy(source_path, &dest_path) {
                            Ok(_) => {
                                imported += 1;
                                info!("Imported to assets: {}", dest_path.display());
                            }
                            Err(e) => {
                                state.last_error = Some(format!(
                                    "Failed to import {}: {}",
                                    source_path.display(),
                                    e
                                ));
                                state.error_timeout = 3.0;
                            }
                        }
                    }

                    // Copy dropped folders recursively
                    for source_dir in &dir_drops {
                        let Some(dir_name) = source_dir.file_name() else { continue };
                        let dest_dir = folder.join(dir_name);
                        if source_dir == &dest_dir {
                            continue;
                        }
                        match state::copy_dir_all(source_dir, &dest_dir) {
                            Ok(count) => {
                                imported += count;
                                info!("Imported folder to assets: {}", dest_dir.display());
                            }
                            Err(e) => {
                                state.last_error = Some(format!(
                                    "Failed to import folder {}: {}",
                                    source_dir.display(),
                                    e
                                ));
                                state.error_timeout = 3.0;
                            }
                        }
                    }

                    // Extract zip archives
                    for zip_path in &zip_files {
                        match state::extract_zip(zip_path, folder) {
                            Ok(count) => {
                                imported += count;
                                info!("Extracted zip to assets: {}", zip_path.display());
                            }
                            Err(e) => {
                                state.last_error = Some(format!(
                                    "Failed to extract {}: {}",
                                    zip_path.display(),
                                    e
                                ));
                                state.error_timeout = 3.0;
                            }
                        }
                    }

                    if imported > 0 {
                        info!("Imported {} item(s) to {}", imported, folder.display());
                    }
                } else if !copy_files.is_empty() || !dir_drops.is_empty() || !zip_files.is_empty() {
                    state.last_error = Some("No folder selected for import".to_string());
                    state.error_timeout = 3.0;
                }

                // Route 3D model files to the import overlay
                if !model_files.is_empty() {
                    if let Some(cmds) = world.get_resource::<EditorCommands>() {
                        let target_dir = import_target.as_ref().and_then(|folder| {
                            let project = world.get_resource::<renzora::core::CurrentProject>()?;
                            folder.strip_prefix(&project.path).ok().map(|rel| {
                                rel.to_string_lossy().replace('\\', "/")
                            })
                        }).unwrap_or_default();

                        cmds.push(move |world: &mut bevy::prelude::World| {
                            world.insert_resource(renzora::core::ImportRequested);
                            if !target_dir.is_empty() {
                                world.insert_resource(renzora::core::ImportTargetDir(target_dir));
                            }
                        });
                    }
                }

                state.drop_target_folder = None;
            }
        }

        // --- Context menu ---
        if let Some(pos) = state.context_menu_pos {
            render_context_menu(ui, &mut state, &theme, pos);
        }

        // --- Process pending rename ---
        if let Some((old_path, new_name)) = state.pending_rename.take() {
            if let Some(parent) = old_path.parent() {
                let new_path = parent.join(&new_name);
                let is_dir = old_path.is_dir();
                match std::fs::rename(&old_path, &new_path) {
                    Ok(_) => {
                        // Update selection to new path
                        state.selected_assets.remove(&old_path);
                        state.selected_assets.insert(new_path.clone());
                        if state.selected_path.as_ref() == Some(&old_path) {
                            state.selected_path = Some(new_path.clone());
                        }
                        emit_asset_path_change(world, &old_path, &new_path, is_dir);
                    }
                    Err(e) => {
                        state.last_error = Some(format!("Rename failed: {}", e));
                        state.error_timeout = 3.0;
                    }
                }
            }
        }

        // --- Batch rename dialog ---
        if state.batch_rename_active {
            let count = state.batch_rename_assets.len();
            let preview_ext = state
                .batch_rename_assets
                .first()
                .and_then(|p| p.extension())
                .and_then(|e| e.to_str())
                .map(|s| s.to_string());
            let mut open = true;
            egui::Window::new("Batch Rename")
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ui.ctx(), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Base name:");
                        ui.text_edit_singleline(&mut state.batch_rename_base);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Start at:");
                        ui.add(
                            egui::DragValue::new(&mut state.batch_rename_start).range(0..=9999),
                        );
                    });
                    ui.add_space(4.0);
                    let preview_text = match &preview_ext {
                        Some(ext) => format!(
                            "Preview: {}_{:02}.{}, {}_{:02}.{}, … ({} files)",
                            state.batch_rename_base, state.batch_rename_start, ext,
                            state.batch_rename_base, state.batch_rename_start + 1, ext,
                            count,
                        ),
                        None => format!(
                            "Preview: {}_{:02}, {}_{:02}, … ({} items)",
                            state.batch_rename_base,
                            state.batch_rename_start,
                            state.batch_rename_base,
                            state.batch_rename_start + 1,
                            count,
                        ),
                    };
                    ui.label(
                        egui::RichText::new(preview_text)
                            .size(11.0)
                            .color(theme.text.muted.to_color32()),
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let base_ok = !state.batch_rename_base.trim().is_empty();
                        let rename_btn = egui::Button::new("Rename");
                        if ui.add_enabled(base_ok, rename_btn).clicked() {
                            state.pending_batch_rename = Some((
                                state.batch_rename_base.clone(),
                                state.batch_rename_start,
                                state.batch_rename_assets.clone(),
                            ));
                            state.batch_rename_active = false;
                        }
                        if ui.button("Cancel").clicked() {
                            state.batch_rename_active = false;
                        }
                    });
                });
            if !open {
                state.batch_rename_active = false;
            }
        }

        // --- Process pending batch rename ---
        if let Some((base, start, assets)) = state.pending_batch_rename.take() {
            let mut new_selection: std::collections::HashSet<std::path::PathBuf> =
                std::collections::HashSet::new();
            for (i, old_path) in assets.iter().enumerate() {
                let Some(parent) = old_path.parent() else {
                    new_selection.insert(old_path.clone());
                    continue;
                };
                let ext = old_path.extension().and_then(|e| e.to_str());
                let new_name = match ext {
                    Some(e) => format!("{}_{:02}.{}", base, start as usize + i, e),
                    None => format!("{}_{:02}", base, start as usize + i),
                };
                let new_path = parent.join(&new_name);
                if new_path == *old_path {
                    new_selection.insert(old_path.clone());
                    continue;
                }
                if new_path.exists() {
                    state.last_error = Some(format!("Skipped: {} already exists", new_name));
                    state.error_timeout = 3.0;
                    new_selection.insert(old_path.clone());
                    continue;
                }
                let is_dir = old_path.is_dir();
                match std::fs::rename(old_path, &new_path) {
                    Ok(_) => {
                        new_selection.insert(new_path.clone());
                        if state.selected_path.as_ref() == Some(old_path) {
                            state.selected_path = Some(new_path.clone());
                        }
                        emit_asset_path_change(world, old_path, &new_path, is_dir);
                    }
                    Err(e) => {
                        state.last_error = Some(format!("Rename failed: {}", e));
                        state.error_timeout = 3.0;
                        new_selection.insert(old_path.clone());
                    }
                }
            }
            state.selected_assets = new_selection;
        }

        // --- Process pending move (drag-to-folder) ---
        if let Some((sources, target)) = state.pending_move.take() {
            let mut moved = 0usize;
            for source in &sources {
                if let Some(file_name) = source.file_name() {
                    let dest = target.join(file_name);
                    if source == &dest || source == &target {
                        continue;
                    }
                    // Don't move a folder into itself
                    if dest.starts_with(source) {
                        continue;
                    }
                    let is_dir = source.is_dir();
                    match std::fs::rename(source, &dest) {
                        Ok(_) => {
                            moved += 1;
                            emit_asset_path_change(world, source, &dest, is_dir);
                        }
                        Err(e) => {
                            state.last_error = Some(format!("Move failed: {}", e));
                            state.error_timeout = 3.0;
                        }
                    }
                }
            }
            state.selected_assets.clear();
            state.selected_path = None;
            // Navigate to the target folder so the user can see the moved files
            if moved > 0 {
                state.navigate_to(target.clone());
                state.expanded_folders.insert(target);
            }
        }

        // --- Process pending delete ---
        if !state.pending_delete.is_empty() {
            let to_delete: Vec<_> = state.pending_delete.drain(..).collect();
            for path in &to_delete {
                let result = if path.is_dir() {
                    std::fs::remove_dir_all(path)
                } else {
                    std::fs::remove_file(path)
                };
                if let Err(e) = result {
                    state.last_error = Some(format!("Delete failed: {}", e));
                    state.error_timeout = 3.0;
                }
                state.selected_assets.remove(path);
            }
            state.selected_path = state.selected_assets.iter().next().cloned();
        }

        // --- Error display ---
        if let Some(ref error) = state.last_error {
            let error_rect = egui::Rect::from_min_size(
                egui::pos2(grid_rect.min.x + 8.0, grid_rect.max.y - 28.0),
                egui::vec2(grid_rect.width() - 16.0, 24.0),
            );
            ui.painter().rect_filled(
                error_rect,
                4.0,
                theme.semantic.error.to_color32().linear_multiply(0.2),
            );
            ui.painter().text(
                error_rect.center(),
                egui::Align2::CENTER_CENTER,
                error,
                FontId::proportional(11.0),
                theme.semantic.error.to_color32(),
            );
        }

        // Submit thumbnail load requests via EditorCommands
        if !grid_result.thumbnail_requests.is_empty() {
            if let Some(cmds) = world.get_resource::<EditorCommands>() {
                let requests = grid_result.thumbnail_requests;
                cmds.push(move |world: &mut bevy::prelude::World| {
                    let asset_server = world.resource::<bevy::prelude::AssetServer>().clone();
                    let project = world.get_resource::<renzora::core::CurrentProject>().cloned();
                    let mut cache = world.resource_mut::<thumbnails::ThumbnailCache>();
                    for path in requests {
                        cache.request(path, &asset_server, project.as_ref());
                    }
                });
            }
        }
        // Submit .material thumbnail requests to the material renderer registry
        if !grid_result.material_thumbnail_requests.is_empty() {
            if let Some(cmds) = world.get_resource::<EditorCommands>() {
                let requests = grid_result.material_thumbnail_requests;
                cmds.push(move |world: &mut bevy::prelude::World| {
                    if let Some(mut registry) = world.get_resource_mut::<MaterialThumbnailRegistry>() {
                        for path in requests {
                            registry.request(path);
                        }
                    }
                });
            }
        }
        // Either pane can produce a drag payload; the tree wins if both fire
        // in the same frame, since tree-only mode is the only context where
        // the tree is the sole source of drags.
        if let Some(payload) = tree_drag_payload.or(grid_result.drag_payload) {
            if !payload.path.is_dir() {
                state.track_recent(payload.path.as_path());
            }
            if let Some(cmds) = world.get_resource::<EditorCommands>() {
                cmds.push(move |world: &mut bevy::prelude::World| {
                    world.insert_resource(payload);
                });
            }
        }
        // Import button clicked — request import overlay
        if state.import_clicked {
            state.import_clicked = false;
            if let Some(cmds) = world.get_resource::<renzora_editor_framework::EditorCommands>() {
                let target_dir = state.current_folder.as_ref().and_then(|folder| {
                    let project = world.get_resource::<renzora::core::CurrentProject>()?;
                    folder.strip_prefix(&project.path).ok().map(|rel| {
                        rel.to_string_lossy().replace('\\', "/")
                    })
                }).unwrap_or_default();

                cmds.push(move |world: &mut bevy::prelude::World| {
                    world.insert_resource(renzora::core::ImportRequested);
                    if !target_dir.is_empty() {
                        world.insert_resource(renzora::core::ImportTargetDir(target_dir));
                    }
                });
            }
        }

        // Double-click on a recent file in the tree
        if let Some(path) = state.double_clicked_recent.take() {
            state.track_recent(&path);
            open_double_clicked(world, path);
        }

        // Double-click on a file opens it in the matching editor + layout
        if let Some(path) = grid_result.double_clicked_file {
            state.track_recent(&path);
            open_double_clicked(world, path);
        }
    }
}

/// Route a double-clicked asset to the right editor: scripts/shaders go to
/// the code editor, .material / .particle / .blueprint get their dedicated
/// layout. All recognized kinds also spawn a document tab. Unknown file
/// types fall through to the legacy code-editor "plain text" flow.
fn open_double_clicked(world: &bevy::prelude::World, path: std::path::PathBuf) {
    use renzora_editor_framework::DocTabKind;

    if let Some(kind) = asset_doc_kind(&path) {
        if let Some(cmds) = world.get_resource::<EditorCommands>() {
            let p = path.clone();
            cmds.push(move |world: &mut bevy::prelude::World| {
                renzora_editor_framework::open_asset_tab(world, &p, kind);
            });
        }
        return;
    }

    // Unrecognized kind — fall back to opening in code editor if it's a text-ish file.
    // `.ron` is intentionally absent: it's the engine's scene format and is
    // routed to a Scene doc tab via `asset_doc_kind` so the scene system
    // can load it instead of dumping the raw text into the code editor.
    let is_editable = path.extension()
        .and_then(|e| e.to_str())
        .map(|e| matches!(e.to_lowercase().as_str(),
            "rs" | "json" | "toml" | "yaml" | "yml" | "txt" | "md"
        ))
        .unwrap_or(false);
    if is_editable {
        if let Some(cmds) = world.get_resource::<EditorCommands>() {
            cmds.push(move |world: &mut bevy::prelude::World| {
                renzora_editor_framework::open_asset_tab(world, &path, DocTabKind::Script);
            });
        }
    }
}

/// Map a file path to the document tab kind it represents, or `None` if the
/// file doesn't correspond to a known editor-opening asset type.
fn asset_doc_kind(path: &std::path::Path) -> Option<renzora_editor_framework::DocTabKind> {
    use renzora_editor_framework::DocTabKind;
    let name = path.file_name().and_then(|n| n.to_str()).map(|s| s.to_lowercase())?;
    if name.ends_with(".material_bp") || name.ends_with(".material") {
        return Some(DocTabKind::Material);
    }
    if name.ends_with(".particle") {
        return Some(DocTabKind::Particle);
    }
    if name.ends_with(".blueprint") || name.ends_with(".bp") {
        return Some(DocTabKind::Blueprint);
    }
    let ext = name.rsplit('.').next().unwrap_or("");
    Some(match ext {
        "ron" => DocTabKind::Scene,
        "rhai" | "lua" | "js" | "ts" | "py" => DocTabKind::Script,
        "wgsl" | "glsl" | "vert" | "frag" => DocTabKind::Shader,
        _ => return None,
    })
}

// ── Context menu ────────────────────────────────────────────────────────────

fn render_context_menu(
    ui: &mut egui::Ui,
    state: &mut AssetBrowserState,
    theme: &renzora_theme::Theme,
    pos: egui::Pos2,
) {
    let ctx = ui.ctx().clone();
    let menu_width = 200.0;
    let item_height = 28.0;
    let item_font = 12.0;
    let icon_font = 14.0;
    let shortcut_font = 10.0;

    let text_primary = theme.text.primary.to_color32();
    let text_secondary = theme.text.secondary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let hover_bg = theme.panels.item_hover.to_color32();

    let folder_color = Color32::from_rgb(255, 196, 0);
    let material_color = Color32::from_rgb(0, 200, 83);
    let scene_color = Color32::from_rgb(115, 191, 242);
    let rhai_color = Color32::from_rgb(130, 230, 180);
    let lua_color = Color32::from_rgb(80, 130, 230);
    let blueprint_color = Color32::from_rgb(100, 180, 255);
    let shader_color = Color32::from_rgb(220, 120, 255);

    // Estimate menu height to decide if we need to flip upward
    // 8 create items + header + separator + selection section (variable) + separator + import
    let has_selection = !state.selected_assets.is_empty();
    let selection_items = if has_selection {
        let rename_item = if state.selected_assets.len() == 1 { 1 } else { 0 };
        1 + rename_item + 1 // header + rename? + delete
    } else {
        0
    };
    let total_items = 8 + selection_items + 1; // 8 create items + selection + import
    let separators = if has_selection { 2 } else { 1 };
    let headers = if has_selection { 2 } else { 1 };
    let estimated_height = (total_items as f32 * item_height)
        + (separators as f32 * 9.0)
        + (headers as f32 * 18.0)
        + 12.0; // padding

    // Anchor bottom-center of the menu at the cursor position
    let total_width = menu_width + 12.0; // menu_width + inner margin
    let menu_pos = egui::pos2(
        (pos.x - total_width * 0.5).max(10.0),
        (pos.y - estimated_height).max(10.0),
    );

    let area_resp = egui::Area::new(egui::Id::new("asset_context_menu"))
        .fixed_pos(menu_pos)
        .order(egui::Order::Foreground)
        .constrain(true)
        .show(&ctx, |ui| {
            egui::Frame::popup(ui.style())
                .inner_margin(egui::Margin::symmetric(6, 6))
                .rounding(8.0)
                .shadow(egui::Shadow {
                    spread: 0,
                    blur: 16,
                    offset: [0, 4],
                    color: Color32::from_black_alpha(80),
                })
                .show(ui, |ui| {
                    ui.set_min_width(menu_width);
                    ui.set_max_width(menu_width);
                    ui.spacing_mut().item_spacing.y = 1.0;

                    let menu_item = |ui: &mut egui::Ui, icon: &str, label: &str, shortcut: &str, icon_color: Color32| -> bool {
                        let desired_size = Vec2::new(menu_width, item_height);
                        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

                        if response.hovered() {
                            ui.painter().rect_filled(rect, 4.0, hover_bg);
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }

                        ui.painter().text(
                            egui::pos2(rect.min.x + 16.0, rect.center().y),
                            egui::Align2::CENTER_CENTER,
                            icon,
                            FontId::proportional(icon_font),
                            icon_color,
                        );

                        ui.painter().text(
                            egui::pos2(rect.min.x + 34.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            label,
                            FontId::proportional(item_font),
                            text_primary,
                        );

                        if !shortcut.is_empty() {
                            ui.painter().text(
                                egui::pos2(rect.max.x - 10.0, rect.center().y),
                                egui::Align2::RIGHT_CENTER,
                                shortcut,
                                FontId::proportional(shortcut_font),
                                text_muted,
                            );
                        }

                        response.clicked()
                    };

                    let separator = |ui: &mut egui::Ui| {
                        ui.add_space(4.0);
                        let rect = ui.allocate_space(egui::vec2(menu_width, 1.0)).1;
                        ui.painter().hline(
                            (rect.min.x + 8.0)..=(rect.max.x - 8.0),
                            rect.center().y,
                            egui::Stroke::new(1.0, theme.widgets.border.to_color32()),
                        );
                        ui.add_space(4.0);
                    };

                    let section_header = |ui: &mut egui::Ui, label: &str| {
                        let desired_size = Vec2::new(menu_width, 18.0);
                        let (rect, _) = ui.allocate_exact_size(desired_size, Sense::hover());
                        ui.painter().text(
                            egui::pos2(rect.min.x + 10.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            label,
                            FontId::proportional(10.0),
                            text_secondary,
                        );
                    };

                    // === Create section ===
                    section_header(ui, "Create");

                    if menu_item(ui, regular::FOLDER_PLUS, "New Folder", "", folder_color) {
                        state.create_inline("New Folder", "");
                        state.context_menu_pos = None;
                    }
                    if menu_item(ui, regular::PALETTE, "Material", "", material_color) {
                        state.create_inline("NewMaterial.material", "{}");
                        state.context_menu_pos = None;
                    }
                    if menu_item(ui, regular::FILM_SCRIPT, "Scene", "", scene_color) {
                        state.create_inline("NewScene.ron", "(resources: {}, entities: {})");
                        state.context_menu_pos = None;
                    }
                    if menu_item(ui, regular::BLUEPRINT, "Blueprint", "", blueprint_color) {
                        state.create_inline("NewBlueprint.blueprint", "{}");
                        state.context_menu_pos = None;
                    }
                    if menu_item(ui, regular::CODE, "Lua Script", "", lua_color) {
                        state.create_inline("new_script.lua", "-- New Lua script\n");
                        state.context_menu_pos = None;
                    }
                    if menu_item(ui, regular::CODE, "Rhai Script", "", rhai_color) {
                        state.create_inline("new_script.rhai", "// New Rhai script\n");
                        state.context_menu_pos = None;
                    }
                    if menu_item(ui, regular::GRAPHICS_CARD, "Shader", "", shader_color) {
                        state.create_inline("new_shader.wgsl", "// New shader\n");
                        state.context_menu_pos = None;
                    }

                    // === Selection actions ===
                    if has_selection {
                        separator(ui);
                        section_header(ui, "Selection");

                        // Favorite toggle for single folder selection
                        if state.selected_assets.len() == 1 {
                            let selected_path = state.selected_assets.iter().next().unwrap().clone();
                            if selected_path.is_dir() {
                                let is_fav = state.is_favorite(&selected_path);
                                let fav_label = if is_fav { "Unfavorite" } else { "Favorite" };
                                let star_color = if is_fav {
                                    Color32::from_rgb(255, 200, 60)
                                } else {
                                    text_primary
                                };
                                if menu_item(ui, regular::STAR, fav_label, "", star_color) {
                                    state.toggle_favorite(&selected_path);
                                    state.context_menu_pos = None;
                                }
                            }
                        }

                        if state.selected_assets.len() == 1 {
                            if menu_item(ui, regular::PENCIL, "Rename", "F2", text_primary) {
                                if let Some(path) = state.selected_assets.iter().next() {
                                    state.renaming_asset = Some(path.clone());
                                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                                        state.rename_buffer = name.to_string();
                                    }
                                    state.rename_focus_set = false;
                                }
                                state.context_menu_pos = None;
                            }
                        } else if state.selected_assets.len() > 1 {
                            if menu_item(ui, regular::TEXT_AA, "Batch Rename…", "", text_primary) {
                                let mut assets: Vec<std::path::PathBuf> =
                                    state.selected_assets.iter().cloned().collect();
                                assets.sort();
                                state.batch_rename_base = assets
                                    .first()
                                    .and_then(|p| p.file_stem())
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("file")
                                    .to_string();
                                state.batch_rename_start = 1;
                                state.batch_rename_assets = assets;
                                state.batch_rename_active = true;
                                state.context_menu_pos = None;
                            }
                        }

                        if menu_item(ui, regular::COPY, "Duplicate", "Ctrl+D", text_primary) {
                            state.duplicate_selected();
                            state.context_menu_pos = None;
                        }

                        let delete_label = if state.selected_assets.len() > 1 {
                            format!("Delete ({})", state.selected_assets.len())
                        } else {
                            "Delete".to_string()
                        };
                        if menu_item(ui, regular::TRASH, &delete_label, "Del", theme.semantic.error.to_color32()) {
                            state.pending_delete = state.selected_assets.iter().cloned().collect();
                            state.context_menu_pos = None;
                        }
                    }

                    // === Import ===
                    separator(ui);

                    if menu_item(ui, regular::DOWNLOAD_SIMPLE, "Import", "", text_primary) {
                        state.import_clicked = true;
                        state.context_menu_pos = None;
                    }
                });
        });

    // Close context menu on primary click outside (skip secondary — that
    // re-opens a new menu via the grid handler). Also skip the first frame
    // so the menu isn't immediately dismissed by the same click that opened it.
    if area_resp.response.rect.area() > 0.0 {
        if ctx.input(|i| i.pointer.primary_clicked()) {
            if let Some(pointer_pos) = ctx.pointer_latest_pos() {
                if !area_resp.response.rect.contains(pointer_pos) {
                    state.context_menu_pos = None;
                }
            }
        }
    }
}

/// Plugin that registers the `AssetBrowserPanel` with the editor.
#[derive(Default)]
pub struct AssetBrowserPlugin;

impl Plugin for AssetBrowserPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] AssetBrowserPlugin");
        app.init_resource::<thumbnails::ThumbnailCache>()
            .add_systems(Update, thumbnails::update_thumbnail_cache)
            .register_panel(AssetBrowserPanel::default());
    }
}

/// Fire an `AssetPathChanged` event via `EditorCommands` so scene entities
/// that reference the moved asset patch their stored paths. Paths are
/// computed asset-relative (to the current project) before the event fires.
fn emit_asset_path_change(
    world: &World,
    old_abs: &std::path::Path,
    new_abs: &std::path::Path,
    is_dir: bool,
) {
    let Some(project) = world.get_resource::<renzora::core::CurrentProject>() else {
        return;
    };
    let old_rel = project.make_asset_relative(old_abs);
    let new_rel = project.make_asset_relative(new_abs);
    if old_rel.is_empty() || new_rel.is_empty() || old_rel == new_rel {
        return;
    }

    let Some(cmds) = world.get_resource::<EditorCommands>() else {
        return;
    };
    cmds.push(move |world: &mut bevy::prelude::World| {
        world.trigger(renzora::core::AssetPathChanged {
            old: old_rel,
            new: new_rel,
            is_dir,
        });
    });
}

renzora::add!(AssetBrowserPlugin, Editor);
