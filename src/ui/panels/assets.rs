use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CursorIcon, RichText, Sense, Vec2};
use std::path::PathBuf;

use crate::core::{AssetViewMode, EditorState};
use crate::project::CurrentProject;

// Icon constants from phosphor
use egui_phosphor::regular::{
    FOLDER, FILE, IMAGE, CUBE, SPEAKER_HIGH, FILE_RS, FILE_TEXT,
    GEAR, FILM_SCRIPT, FILE_CODE, DOWNLOAD, SCROLL, FOLDER_PLUS, CARET_RIGHT,
    MAGNIFYING_GLASS, LIST, SQUARES_FOUR, ARROW_LEFT, HOUSE,
};
use egui_phosphor::fill::FOLDER as FOLDER_FILL;

const MIN_TILE_SIZE: f32 = 64.0;
const MAX_TILE_SIZE: f32 = 128.0;
const DEFAULT_TILE_SIZE: f32 = 80.0;
const LIST_ROW_HEIGHT: f32 = 24.0;

/// Render the asset browser panel
pub fn render_assets(
    ctx: &egui::Context,
    current_project: Option<&CurrentProject>,
    editor_state: &mut EditorState,
    _left_panel_width: f32,
    _right_panel_width: f32,
    _bottom_panel_height: f32,
) {
    let panel_height = editor_state.assets_height;

    egui::TopBottomPanel::bottom("assets_panel")
        .exact_height(panel_height)
        .show_separator_line(false)
        .show(ctx, |ui| {
            // Custom resize handle at the top of the panel
            let resize_height = 4.0;
            let (resize_rect, resize_response) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), resize_height),
                Sense::drag(),
            );

            let resize_color = if resize_response.dragged() || resize_response.hovered() {
                Color32::from_rgb(100, 149, 237)
            } else {
                Color32::from_rgb(50, 50, 58)
            };
            ui.painter().rect_filled(resize_rect, 0.0, resize_color);

            if resize_response.hovered() || resize_response.dragged() {
                ctx.set_cursor_icon(CursorIcon::ResizeVertical);
            }

            if resize_response.dragged() {
                let delta = resize_response.drag_delta().y;
                editor_state.assets_height = (panel_height - delta).clamp(100.0, 600.0);
            }

            ui.add_space(4.0);

            render_assets_content(ui, current_project, editor_state);
        });

    // Dialogs
    render_create_script_dialog(ctx, editor_state);
    render_create_folder_dialog(ctx, editor_state);
    handle_import_request(editor_state);
}

/// Render assets content (for use in docking)
pub fn render_assets_content(
    ui: &mut egui::Ui,
    current_project: Option<&CurrentProject>,
    editor_state: &mut EditorState,
) {
    let ctx = ui.ctx().clone();

    // Toolbar with breadcrumb, search, view toggle, and zoom
    render_toolbar(ui, &ctx, editor_state, current_project);

    ui.add_space(4.0);
    ui.separator();
    ui.add_space(4.0);

    // Main content area
    egui::ScrollArea::vertical().show(ui, |ui| {
        if let Some(project) = current_project {
            let items = collect_items(editor_state, project);
            let filtered_items = filter_items(&items, &editor_state.assets_search);

            match editor_state.assets_view_mode {
                AssetViewMode::Grid => {
                    render_grid_view(ui, &ctx, editor_state, &filtered_items);
                }
                AssetViewMode::List => {
                    render_list_view(ui, &ctx, editor_state, &filtered_items);
                }
            }

            // Context menu (only when inside a folder)
            if editor_state.current_assets_folder.is_some() {
                ui.allocate_response(ui.available_size(), Sense::click())
                    .context_menu(|ui| {
                        render_context_menu(ui, editor_state);
                    });
            }
        } else {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(FOLDER).size(48.0).color(Color32::from_rgb(80, 80, 90)));
                ui.add_space(8.0);
                ui.label(RichText::new("No project loaded").color(Color32::from_rgb(120, 120, 130)));
            });
        }
    });
}

fn render_toolbar(
    ui: &mut egui::Ui,
    _ctx: &egui::Context,
    editor_state: &mut EditorState,
    current_project: Option<&CurrentProject>,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        // Back button
        let can_go_back = editor_state.current_assets_folder.is_some();
        ui.add_enabled_ui(can_go_back, |ui| {
            if ui.button(RichText::new(ARROW_LEFT).size(16.0)).clicked() {
                if let Some(ref current) = editor_state.current_assets_folder {
                    if let Some(project) = current_project {
                        if current == &project.path {
                            // Already at project root, can't go back
                        } else if let Some(parent) = current.parent() {
                            if parent >= project.path.as_path() {
                                editor_state.current_assets_folder = Some(parent.to_path_buf());
                            }
                        }
                    }
                }
            }
        });

        // Home button
        if ui.button(RichText::new(HOUSE).size(16.0)).on_hover_text("Go to project root").clicked() {
            if let Some(project) = current_project {
                editor_state.current_assets_folder = Some(project.path.clone());
            }
        }

        ui.separator();

        // Breadcrumb path
        ui.label(RichText::new(FOLDER_FILL).size(14.0).color(Color32::from_rgb(217, 191, 115)));

        if let Some(project) = current_project {
            let project_name = project.path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Project");

            if let Some(ref current_folder) = editor_state.current_assets_folder {
                // Build breadcrumb parts
                let mut parts: Vec<(String, PathBuf)> = vec![];

                // Start with project root
                parts.push((project_name.to_string(), project.path.clone()));

                // Add path components
                if let Ok(relative) = current_folder.strip_prefix(&project.path) {
                    let mut accumulated = project.path.clone();
                    for component in relative.components() {
                        if let std::path::Component::Normal(name) = component {
                            accumulated = accumulated.join(name);
                            if let Some(name_str) = name.to_str() {
                                parts.push((name_str.to_string(), accumulated.clone()));
                            }
                        }
                    }
                }

                // Render breadcrumb
                for (i, (name, path)) in parts.iter().enumerate() {
                    if i > 0 {
                        ui.label(RichText::new(CARET_RIGHT).size(12.0).color(Color32::from_rgb(100, 100, 110)));
                    }

                    let is_current = Some(path) == editor_state.current_assets_folder.as_ref();
                    let text_color = if is_current {
                        Color32::WHITE
                    } else {
                        Color32::from_rgb(150, 150, 160)
                    };

                    if ui.link(RichText::new(name).color(text_color).size(13.0)).clicked() {
                        editor_state.current_assets_folder = Some(path.clone());
                    }
                }
            } else {
                // Not in any folder yet
                ui.label(RichText::new(project_name).color(Color32::from_rgb(150, 150, 160)));
            }
        }

        // Right-aligned controls
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Zoom slider
            ui.spacing_mut().slider_width = 80.0;
            ui.add(egui::Slider::new(&mut editor_state.assets_zoom, 0.5..=1.5).show_value(false));

            ui.separator();

            // View mode toggle
            let grid_color = if editor_state.assets_view_mode == AssetViewMode::Grid {
                Color32::WHITE
            } else {
                Color32::from_rgb(120, 120, 130)
            };
            let list_color = if editor_state.assets_view_mode == AssetViewMode::List {
                Color32::WHITE
            } else {
                Color32::from_rgb(120, 120, 130)
            };

            if ui.button(RichText::new(LIST).size(16.0).color(list_color)).clicked() {
                editor_state.assets_view_mode = AssetViewMode::List;
            }
            if ui.button(RichText::new(SQUARES_FOUR).size(16.0).color(grid_color)).clicked() {
                editor_state.assets_view_mode = AssetViewMode::Grid;
            }

            ui.separator();

            // Search input
            ui.add_sized(
                [150.0, 20.0],
                egui::TextEdit::singleline(&mut editor_state.assets_search)
                    .hint_text(format!("{} Search...", MAGNIFYING_GLASS))
            );
        });
    });
}

fn collect_items(editor_state: &EditorState, project: &CurrentProject) -> Vec<AssetItem> {
    let mut items = Vec::new();

    let folder_to_read = editor_state.current_assets_folder.as_ref()
        .unwrap_or(&project.path);

    if let Ok(entries) = std::fs::read_dir(folder_to_read) {
        let mut folders: Vec<PathBuf> = Vec::new();
        let mut files: Vec<PathBuf> = Vec::new();

        for entry in entries.flatten() {
            let path = entry.path();
            // Skip hidden files/folders
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') {
                    continue;
                }
            }
            // Skip target directory and other build artifacts
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name == "target" || name == "Cargo.lock" {
                    continue;
                }
            }

            if path.is_dir() {
                folders.push(path);
            } else {
                files.push(path);
            }
        }

        folders.sort();
        files.sort();

        // Add folders first
        for folder in folders {
            if let Some(name) = folder.file_name().and_then(|n| n.to_str()) {
                let (icon, color) = get_folder_icon_and_color(name);
                items.push(AssetItem {
                    name: name.to_string(),
                    path: folder,
                    is_folder: true,
                    icon,
                    color,
                });
            }
        }

        // Add files
        for file in files {
            if let Some(name) = file.file_name().and_then(|n| n.to_str()) {
                let (icon, color) = get_file_icon_and_color(name);
                items.push(AssetItem {
                    name: name.to_string(),
                    path: file,
                    is_folder: false,
                    icon,
                    color,
                });
            }
        }
    }

    items
}

fn filter_items<'a>(items: &'a [AssetItem], search: &str) -> Vec<&'a AssetItem> {
    if search.is_empty() {
        items.iter().collect()
    } else {
        let search_lower = search.to_lowercase();
        items.iter()
            .filter(|item| item.name.to_lowercase().contains(&search_lower))
            .collect()
    }
}

fn render_grid_view(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    editor_state: &mut EditorState,
    items: &[&AssetItem],
) {
    let tile_size = DEFAULT_TILE_SIZE * editor_state.assets_zoom;
    let icon_size = (tile_size * 0.45).max(24.0);

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);

        for item in items {
            let is_draggable = !item.is_folder && is_model_file(&item.name);

            let (rect, response) = ui.allocate_exact_size(
                Vec2::new(tile_size, tile_size + 20.0),
                if is_draggable { Sense::click_and_drag() } else { Sense::click() },
            );

            let is_hovered = response.hovered();
            let is_selected = editor_state.selected_asset.as_ref() == Some(&item.path);

            // Background
            let bg_color = if is_selected {
                Color32::from_rgb(60, 90, 140)
            } else if is_hovered {
                Color32::from_rgb(50, 50, 60)
            } else {
                Color32::from_rgb(38, 38, 45)
            };

            ui.painter().rect_filled(rect, 6.0, bg_color);

            // Selection border
            if is_selected {
                ui.painter().rect_stroke(
                    rect,
                    6.0,
                    egui::Stroke::new(2.0, Color32::from_rgb(100, 150, 220)),
                    egui::StrokeKind::Inside,
                );
            }

            // Icon
            let icon_rect = egui::Rect::from_center_size(
                egui::pos2(rect.center().x, rect.min.y + tile_size / 2.0 - 8.0),
                Vec2::splat(icon_size),
            );

            ui.painter().text(
                icon_rect.center(),
                egui::Align2::CENTER_CENTER,
                item.icon,
                egui::FontId::proportional(icon_size),
                item.color,
            );

            // Label
            let label_rect = egui::Rect::from_min_size(
                egui::pos2(rect.min.x + 4.0, rect.max.y - 18.0),
                Vec2::new(tile_size - 8.0, 16.0),
            );

            let max_chars = ((tile_size - 8.0) / 7.0) as usize;
            let truncated_name = truncate_name(&item.name, max_chars.max(6));
            ui.painter().text(
                label_rect.center(),
                egui::Align2::CENTER_CENTER,
                &truncated_name,
                egui::FontId::proportional(11.0),
                Color32::from_rgb(200, 200, 210),
            );

            // Handle interactions
            handle_item_interaction(ctx, editor_state, item, &response, is_draggable);

            // Tooltip
            if is_hovered && editor_state.dragging_asset.is_none() {
                response.on_hover_text(&item.name);
            }
        }
    });
}

fn render_list_view(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    editor_state: &mut EditorState,
    items: &[&AssetItem],
) {
    for item in items {
        let is_draggable = !item.is_folder && is_model_file(&item.name);

        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), LIST_ROW_HEIGHT),
            if is_draggable { Sense::click_and_drag() } else { Sense::click() },
        );

        let is_hovered = response.hovered();
        let is_selected = editor_state.selected_asset.as_ref() == Some(&item.path);

        // Background
        let bg_color = if is_selected {
            Color32::from_rgb(60, 90, 140)
        } else if is_hovered {
            Color32::from_rgb(45, 45, 55)
        } else {
            Color32::TRANSPARENT
        };

        if bg_color != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, 2.0, bg_color);
        }

        // Icon
        ui.painter().text(
            egui::pos2(rect.min.x + 12.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            item.icon,
            egui::FontId::proportional(16.0),
            item.color,
        );

        // Name
        ui.painter().text(
            egui::pos2(rect.min.x + 32.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            &item.name,
            egui::FontId::proportional(12.0),
            Color32::from_rgb(200, 200, 210),
        );

        // File extension on the right
        if !item.is_folder {
            if let Some(ext) = item.path.extension().and_then(|e| e.to_str()) {
                ui.painter().text(
                    egui::pos2(rect.max.x - 8.0, rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    ext.to_uppercase(),
                    egui::FontId::proportional(10.0),
                    Color32::from_rgb(100, 100, 110),
                );
            }
        }

        handle_item_interaction(ctx, editor_state, item, &response, is_draggable);
    }
}

fn handle_item_interaction(
    ctx: &egui::Context,
    editor_state: &mut EditorState,
    item: &AssetItem,
    response: &egui::Response,
    is_draggable: bool,
) {
    if response.clicked() {
        if item.is_folder {
            editor_state.current_assets_folder = Some(item.path.clone());
        } else {
            editor_state.selected_asset = Some(item.path.clone());

            // Open script files in the editor
            if is_script_file(&item.path) {
                super::script_editor::open_script(editor_state, item.path.clone());
            }
        }
    }

    if response.double_clicked() && item.is_folder {
        editor_state.current_assets_folder = Some(item.path.clone());
    }

    // Drag support for models
    if is_draggable {
        if response.drag_started() {
            editor_state.dragging_asset = Some(item.path.clone());
        }

        if editor_state.dragging_asset.as_ref() == Some(&item.path) {
            if let Some(pos) = ctx.pointer_hover_pos() {
                egui::Area::new(egui::Id::new("drag_tooltip"))
                    .fixed_pos(pos + Vec2::new(10.0, 10.0))
                    .interactable(false)
                    .order(egui::Order::Tooltip)
                    .show(ctx, |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(CUBE).color(Color32::from_rgb(242, 166, 115)));
                                ui.label(format!("Drop: {}", item.name));
                            });
                        });
                    });
            }
        }
    }
}

fn render_context_menu(ui: &mut egui::Ui, editor_state: &mut EditorState) {
    ui.set_min_width(150.0);

    if ui.button(format!("{} New Folder", FOLDER_PLUS)).clicked() {
        editor_state.show_create_folder_dialog = true;
        editor_state.new_folder_name = "New Folder".to_string();
        ui.close();
    }

    if ui.button(format!("{} Create Script", SCROLL)).clicked() {
        editor_state.show_create_script_dialog = true;
        editor_state.new_script_name = "new_script".to_string();
        ui.close();
    }

    ui.separator();

    if ui.button(format!("{} Import", DOWNLOAD)).clicked() {
        editor_state.import_asset_requested = true;
        ui.close();
    }
}

fn render_create_script_dialog(ctx: &egui::Context, editor_state: &mut EditorState) {
    if !editor_state.show_create_script_dialog {
        return;
    }

    let mut open = true;
    egui::Window::new("Create Script")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut editor_state.new_script_name);
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button("Create").clicked() {
                    if let Some(ref target_folder) = editor_state.current_assets_folder {
                        let script_name = if editor_state.new_script_name.ends_with(".rhai") {
                            editor_state.new_script_name.clone()
                        } else {
                            format!("{}.rhai", editor_state.new_script_name)
                        };

                        let script_path = target_folder.join(&script_name);
                        let template = create_script_template(&editor_state.new_script_name);

                        if let Err(e) = std::fs::write(&script_path, template) {
                            error!("Failed to create script: {}", e);
                        } else {
                            info!("Created script: {}", script_path.display());
                        }
                    }

                    editor_state.show_create_script_dialog = false;
                    editor_state.new_script_name.clear();
                }

                if ui.button("Cancel").clicked() {
                    editor_state.show_create_script_dialog = false;
                    editor_state.new_script_name.clear();
                }
            });
        });

    if !open {
        editor_state.show_create_script_dialog = false;
    }
}

fn render_create_folder_dialog(ctx: &egui::Context, editor_state: &mut EditorState) {
    if !editor_state.show_create_folder_dialog {
        return;
    }

    let mut open = true;
    egui::Window::new("New Folder")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut editor_state.new_folder_name);
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button("Create").clicked() {
                    if let Some(ref current_folder) = editor_state.current_assets_folder {
                        let new_folder_path = current_folder.join(&editor_state.new_folder_name);

                        if let Err(e) = std::fs::create_dir_all(&new_folder_path) {
                            error!("Failed to create folder: {}", e);
                        } else {
                            info!("Created folder: {}", new_folder_path.display());
                            editor_state.current_assets_folder = Some(new_folder_path);
                        }
                    }

                    editor_state.show_create_folder_dialog = false;
                    editor_state.new_folder_name.clear();
                }

                if ui.button("Cancel").clicked() {
                    editor_state.show_create_folder_dialog = false;
                    editor_state.new_folder_name.clear();
                }
            });
        });

    if !open {
        editor_state.show_create_folder_dialog = false;
    }
}

fn handle_import_request(editor_state: &mut EditorState) {
    if !editor_state.import_asset_requested {
        return;
    }

    editor_state.import_asset_requested = false;

    let Some(target_folder) = editor_state.current_assets_folder.clone() else {
        return;
    };

    if let Some(paths) = rfd::FileDialog::new()
        .add_filter("All Assets", &["glb", "gltf", "obj", "fbx", "png", "jpg", "jpeg", "wav", "ogg", "mp3", "rhai"])
        .add_filter("3D Models", &["glb", "gltf", "obj", "fbx"])
        .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "tga"])
        .add_filter("Audio", &["wav", "ogg", "mp3"])
        .add_filter("Scripts", &["rhai"])
        .pick_files()
    {
        let _ = std::fs::create_dir_all(&target_folder);

        for source_path in paths {
            if let Some(file_name) = source_path.file_name() {
                let dest_path = target_folder.join(file_name);
                if let Err(e) = std::fs::copy(&source_path, &dest_path) {
                    error!("Failed to import {}: {}", source_path.display(), e);
                } else {
                    info!("Imported: {}", dest_path.display());
                }
            }
        }
    }
}

fn create_script_template(name: &str) -> String {
    format!(r#"// Script: {}
// Called once when the script starts
fn on_ready() {{
    // Initialize your script here
}}

// Called every frame
fn on_update() {{
    // Update logic here
    // Available variables:
    //   delta - time since last frame
    //   elapsed - total elapsed time
    //   position_x, position_y, position_z - current position
    //   rotation_x, rotation_y, rotation_z - current rotation (degrees)
    //   scale_x, scale_y, scale_z - current scale
    //   input_x, input_y - movement input (-1 to 1)
    //
    // Available functions:
    //   set_position(x, y, z) - set absolute position
    //   set_rotation(x, y, z) - set rotation in degrees
    //   translate(x, y, z) - move by delta
    //   print_log(message) - print to console
    //   lerp(a, b, t), clamp(v, min, max)
    //   sin, cos, tan, sqrt, abs, floor, ceil, round
    //   min, max, pow, deg_to_rad, rad_to_deg
}}
"#, name)
}

struct AssetItem {
    name: String,
    path: PathBuf,
    is_folder: bool,
    icon: &'static str,
    color: Color32,
}

fn truncate_name(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        name.to_string()
    } else {
        format!("{}...", &name[..max_len.saturating_sub(3)])
    }
}

fn is_model_file(filename: &str) -> bool {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    matches!(ext.as_str(), "glb" | "gltf" | "obj" | "fbx")
}

fn is_script_file(path: &PathBuf) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase() == "rhai")
        .unwrap_or(false)
}

/// Get icon and color for folders based on name
fn get_folder_icon_and_color(name: &str) -> (&'static str, Color32) {
    match name.to_lowercase().as_str() {
        "assets" => (FOLDER, Color32::from_rgb(217, 191, 115)),
        "scenes" => (FOLDER, Color32::from_rgb(115, 191, 217)),
        "scripts" => (FOLDER, Color32::from_rgb(140, 217, 191)),
        "materials" => (FOLDER, Color32::from_rgb(217, 140, 191)),
        "textures" | "images" => (FOLDER, Color32::from_rgb(166, 217, 140)),
        "models" | "meshes" => (FOLDER, Color32::from_rgb(242, 166, 115)),
        "audio" | "sounds" | "music" => (FOLDER, Color32::from_rgb(217, 140, 217)),
        "prefabs" => (FOLDER, Color32::from_rgb(140, 166, 217)),
        "src" => (FOLDER, Color32::from_rgb(242, 140, 89)),
        _ => (FOLDER, Color32::from_rgb(180, 180, 190)),
    }
}

/// Get icon and color for a file based on its extension
fn get_file_icon_and_color(filename: &str) -> (&'static str, Color32) {
    let ext = filename.rsplit('.').next().unwrap_or("");
    match ext.to_lowercase().as_str() {
        // Scene files
        "scene" => (FILM_SCRIPT, Color32::from_rgb(115, 191, 242)),
        // Config files
        "toml" => (GEAR, Color32::from_rgb(179, 179, 191)),
        // Images
        "png" | "jpg" | "jpeg" | "bmp" | "tga" | "hdr" | "exr" => (IMAGE, Color32::from_rgb(166, 217, 140)),
        // 3D Models
        "gltf" | "glb" | "obj" | "fbx" => (CUBE, Color32::from_rgb(242, 166, 115)),
        // Audio
        "wav" | "ogg" | "mp3" | "flac" => (SPEAKER_HIGH, Color32::from_rgb(217, 140, 217)),
        // Rust source
        "rs" => (FILE_RS, Color32::from_rgb(242, 140, 89)),
        // Scripts
        "rhai" => (SCROLL, Color32::from_rgb(140, 217, 191)),
        // Text/docs
        "txt" | "md" => (FILE_TEXT, Color32::from_rgb(191, 191, 204)),
        // Data files
        "json" | "ron" | "yaml" | "yml" => (FILE_CODE, Color32::from_rgb(179, 179, 191)),
        // Materials (custom extension)
        "mat" | "material" => (GEAR, Color32::from_rgb(217, 140, 191)),
        // Default
        _ => (FILE, Color32::from_rgb(140, 140, 150)),
    }
}
