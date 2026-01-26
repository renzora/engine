use bevy::prelude::*;
use bevy::window::{WindowMode, WindowPosition};
use bevy_egui::egui::{self, Color32, CornerRadius, Id, Pos2, Sense, Stroke, Vec2};

use crate::commands::{CommandHistory, DeleteEntityCommand, queue_command};
use crate::core::{AssetBrowserState, DockingState, EditorEntity, ExportState, SceneNode, SelectionState, ViewportState, WindowState, SceneManagerState, EditorSettings, ResizeEdge};
use crate::plugin_core::{MenuLocation, MenuItem, PluginHost};
use crate::scene::{spawn_primitive, PrimitiveType};
use crate::ui::docking::{builtin_layouts, PanelId};
use crate::ui_api::UiEvent;

use egui_phosphor::regular::{MINUS, SQUARE, X, SQUARES_FOUR};

/// Height of the custom title bar
pub const TITLE_BAR_HEIGHT: f32 = 28.0;

pub fn render_title_bar(
    ctx: &egui::Context,
    window_state: &mut WindowState,
    selection: &mut SelectionState,
    scene_state: &mut SceneManagerState,
    settings: &mut EditorSettings,
    export_state: &mut ExportState,
    assets: &mut AssetBrowserState,
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    plugin_host: &PluginHost,
    command_history: &mut CommandHistory,
    docking_state: &mut DockingState,
    viewport_state: &mut ViewportState,
) -> Vec<UiEvent> {
    let mut ui_events = Vec::new();
    let is_maximized = window_state.is_maximized;

    egui::TopBottomPanel::top("title_bar")
        .exact_height(TITLE_BAR_HEIGHT)
        .frame(egui::Frame::NONE.fill(Color32::from_rgb(30, 30, 36)))
        .show(ctx, |ui| {
            let panel_rect = ui.max_rect();
            let painter = ui.painter();

            // Window button dimensions
            let button_width = 40.0;
            let window_buttons_width = button_width * 3.0;

            // Draw centered title with icon
            let title_text = "Renzora Engine r4".to_string();
            let title_galley = painter.layout_no_wrap(
                title_text.clone(),
                egui::FontId::proportional(13.0),
                Color32::from_rgb(140, 140, 150),
            );
            let title_pos = Pos2::new(
                panel_rect.center().x - title_galley.size().x / 2.0,
                panel_rect.center().y - title_galley.size().y / 2.0,
            );
            painter.galley(title_pos, title_galley, Color32::from_rgb(140, 140, 150));

            // Draw bottom border
            painter.line_segment(
                [
                    Pos2::new(panel_rect.left(), panel_rect.bottom() - 1.0),
                    Pos2::new(panel_rect.right(), panel_rect.bottom() - 1.0),
                ],
                Stroke::new(1.0, Color32::from_rgb(20, 20, 24)),
            );

            // Drag area (everything except window buttons) - interact with it FIRST
            // This puts the drag area at the bottom of the interaction stack
            let drag_rect = egui::Rect::from_min_max(
                panel_rect.min,
                Pos2::new(panel_rect.max.x - window_buttons_width, panel_rect.max.y),
            );

            // Create an interactive drag area that sits behind other elements
            let drag_response = ui.interact(drag_rect, Id::new("title_bar_drag"), Sense::click_and_drag());

            // Handle double-click to maximize
            if drag_response.double_clicked() {
                window_state.request_toggle_maximize = true;
            }

            // Handle drag - set flag when drag starts
            if drag_response.drag_started() {
                window_state.start_drag = true;
            }

            // Stop manual drag when mouse released
            if drag_response.drag_stopped() || !drag_response.dragged() && !drag_response.drag_started() {
                if window_state.is_being_dragged {
                    window_state.is_being_dragged = false;
                    window_state.drag_offset = None;
                }
            }

            // Layout menus on the left (offset down slightly for visual balance)
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.add_space(8.0);

                // Menu bar items
                ui_events = render_menu_items(ui, selection, scene_state, settings, export_state, assets, commands, meshes, materials, plugin_host, command_history, docking_state, viewport_state);

                // Fill remaining space
                ui.add_space(ui.available_width() - window_buttons_width);

                // Window buttons on the right
                ui.spacing_mut().item_spacing.x = 0.0;

                // Minimize button
                let min_resp = window_button(ui, MINUS, Color32::from_rgb(60, 60, 70), button_width);
                if min_resp.clicked() {
                    window_state.request_minimize = true;
                }

                // Maximize/Restore button
                let max_icon = if is_maximized { SQUARES_FOUR } else { SQUARE };
                let max_resp = window_button(ui, max_icon, Color32::from_rgb(60, 60, 70), button_width);
                if max_resp.clicked() {
                    window_state.request_toggle_maximize = true;
                }

                // Close button (red on hover)
                let close_resp = window_button(ui, X, Color32::from_rgb(200, 60, 60), button_width);
                if close_resp.clicked() {
                    window_state.request_close = true;
                }
            });
        });

    ui_events
}

/// System to sync window state and apply pending window actions
pub fn handle_window_actions(
    mut window_state: ResMut<WindowState>,
    mut windows: Query<&mut Window>,
) {
    let Ok(mut window) = windows.single_mut() else { return };

    // Sync window state
    window_state.is_maximized = window.mode == WindowMode::BorderlessFullscreen(MonitorSelection::Current);

    // Apply pending actions
    if window_state.request_close {
        std::process::exit(0);
    }

    if window_state.request_minimize {
        window.set_minimized(true);
        window_state.request_minimize = false;
    }

    if window_state.request_toggle_maximize {
        if window.mode == WindowMode::BorderlessFullscreen(MonitorSelection::Current) {
            window.mode = WindowMode::Windowed;
        } else {
            window.mode = WindowMode::BorderlessFullscreen(MonitorSelection::Current);
        }
        window_state.request_toggle_maximize = false;
    }

    // Handle window drag start - use manual drag (native winit drag has timing issues)
    if window_state.start_drag && window.mode == WindowMode::Windowed {
        // Get current cursor position relative to window
        if let Some(cursor_pos) = window.cursor_position() {
            // Store the offset from window corner to cursor
            window_state.drag_offset = Some((cursor_pos.x, cursor_pos.y));
            window_state.is_being_dragged = true;
        }

        window_state.start_drag = false;
    }

    // Manual window dragging fallback
    if window_state.is_being_dragged && window.mode == WindowMode::Windowed {
        #[cfg(target_os = "windows")]
        {
            use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
            use windows_sys::Win32::Foundation::POINT;

            let mut screen_cursor = POINT { x: 0, y: 0 };
            unsafe {
                if GetCursorPos(&mut screen_cursor) != 0 {
                    if let Some((offset_x, offset_y)) = window_state.drag_offset {
                        // Calculate new window position
                        let new_x = screen_cursor.x as f32 - offset_x;
                        let new_y = screen_cursor.y as f32 - offset_y;

                        window.position = WindowPosition::At(IVec2::new(new_x as i32, new_y as i32));
                    }
                }
            }
        }
    }

    // Handle window resizing
    if window_state.is_resizing && window.mode == WindowMode::Windowed {
        #[cfg(target_os = "windows")]
        {
            use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
            use windows_sys::Win32::Foundation::POINT;

            let mut screen_cursor = POINT { x: 0, y: 0 };
            unsafe {
                if GetCursorPos(&mut screen_cursor) != 0 {
                    // Initialize resize start state if not set
                    if window_state.resize_start_rect.is_none() {
                        let pos = match window.position {
                            WindowPosition::At(p) => p,
                            _ => IVec2::ZERO,
                        };
                        window_state.resize_start_rect = Some((
                            pos.x,
                            pos.y,
                            window.resolution.width() as u32,
                            window.resolution.height() as u32,
                        ));
                        window_state.resize_start_cursor = Some((screen_cursor.x, screen_cursor.y));
                    }

                    if let (Some((start_x, start_y, start_w, start_h)), Some((cursor_start_x, cursor_start_y))) =
                        (window_state.resize_start_rect, window_state.resize_start_cursor)
                    {
                        let dx = screen_cursor.x - cursor_start_x;
                        let dy = screen_cursor.y - cursor_start_y;

                        let min_w = 800i32;
                        let min_h = 600i32;

                        let (new_x, new_y, new_w, new_h) = match window_state.resize_edge {
                            ResizeEdge::Right => {
                                let new_w = (start_w as i32 + dx).max(min_w) as u32;
                                (start_x, start_y, new_w, start_h)
                            }
                            ResizeEdge::Bottom => {
                                let new_h = (start_h as i32 + dy).max(min_h) as u32;
                                (start_x, start_y, start_w, new_h)
                            }
                            ResizeEdge::Left => {
                                let new_w = (start_w as i32 - dx).max(min_w) as u32;
                                let new_x = start_x + (start_w as i32 - new_w as i32);
                                (new_x, start_y, new_w, start_h)
                            }
                            ResizeEdge::Top => {
                                let new_h = (start_h as i32 - dy).max(min_h) as u32;
                                let new_y = start_y + (start_h as i32 - new_h as i32);
                                (start_x, new_y, start_w, new_h)
                            }
                            ResizeEdge::BottomRight => {
                                let new_w = (start_w as i32 + dx).max(min_w) as u32;
                                let new_h = (start_h as i32 + dy).max(min_h) as u32;
                                (start_x, start_y, new_w, new_h)
                            }
                            ResizeEdge::BottomLeft => {
                                let new_w = (start_w as i32 - dx).max(min_w) as u32;
                                let new_h = (start_h as i32 + dy).max(min_h) as u32;
                                let new_x = start_x + (start_w as i32 - new_w as i32);
                                (new_x, start_y, new_w, new_h)
                            }
                            ResizeEdge::TopRight => {
                                let new_w = (start_w as i32 + dx).max(min_w) as u32;
                                let new_h = (start_h as i32 - dy).max(min_h) as u32;
                                let new_y = start_y + (start_h as i32 - new_h as i32);
                                (start_x, new_y, new_w, new_h)
                            }
                            ResizeEdge::TopLeft => {
                                let new_w = (start_w as i32 - dx).max(min_w) as u32;
                                let new_h = (start_h as i32 - dy).max(min_h) as u32;
                                let new_x = start_x + (start_w as i32 - new_w as i32);
                                let new_y = start_y + (start_h as i32 - new_h as i32);
                                (new_x, new_y, new_w, new_h)
                            }
                            ResizeEdge::None => (start_x, start_y, start_w, start_h),
                        };

                        window.position = WindowPosition::At(IVec2::new(new_x, new_y));
                        window.resolution.set(new_w as f32, new_h as f32);
                    }
                }
            }
        }
    }
}

fn render_menu_items(
    ui: &mut egui::Ui,
    selection: &mut SelectionState,
    scene_state: &mut SceneManagerState,
    settings: &mut EditorSettings,
    export_state: &mut ExportState,
    assets: &mut AssetBrowserState,
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    plugin_host: &PluginHost,
    command_history: &mut CommandHistory,
    docking_state: &mut DockingState,
    viewport_state: &mut ViewportState,
) -> Vec<UiEvent> {
    let mut events = Vec::new();
    let api = plugin_host.api();

    // Get plugin menu items grouped by location
    let file_items: Vec<_> = api.menu_items.iter()
        .filter(|(loc, _, _)| *loc == MenuLocation::File)
        .map(|(_, item, _)| item)
        .collect();

    let tools_items: Vec<_> = api.menu_items.iter()
        .filter(|(loc, _, _)| *loc == MenuLocation::Tools)
        .map(|(_, item, _)| item)
        .collect();

    ui.style_mut().visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    ui.style_mut().visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(50, 50, 60);
    ui.style_mut().visuals.widgets.active.weak_bg_fill = Color32::from_rgb(60, 60, 70);

    ui.menu_button("File", |ui| {
        if ui.button("New Scene").clicked() {
            scene_state.new_scene_requested = true;
            ui.close();
        }
        if ui.button("Open Scene...").clicked() {
            scene_state.open_scene_requested = true;
            ui.close();
        }
        ui.separator();
        if ui.button("Save Scene        Ctrl+S").clicked() {
            scene_state.save_scene_requested = true;
            ui.close();
        }
        if ui.button("Save Scene As...  Ctrl+Shift+S").clicked() {
            scene_state.save_scene_as_requested = true;
            ui.close();
        }

        // Plugin File menu items
        if !file_items.is_empty() {
            ui.separator();
            for item in &file_items {
                if let Some(event) = render_plugin_menu_item(ui, item) {
                    events.push(event);
                }
            }
        }

        ui.separator();

        // Import submenu
        ui.menu_button("Import", |ui| {
            if ui.button("Import Assets...").clicked() {
                assets.import_asset_requested = true;
                ui.close();
            }
            ui.separator();
            if ui.button("Import 3D Model...").clicked() {
                // Open file dialog specifically for 3D models
                if let Some(paths) = rfd::FileDialog::new()
                    .add_filter("3D Models", &["glb", "gltf", "obj", "fbx"])
                    .pick_files()
                {
                    if !paths.is_empty() {
                        assets.pending_import_files = paths;
                        assets.show_import_dialog = true;
                    }
                }
                ui.close();
            }
            if ui.button("Import Image...").clicked() {
                if let Some(paths) = rfd::FileDialog::new()
                    .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "tga", "hdr", "exr"])
                    .pick_files()
                {
                    if let Some(target_folder) = assets.current_folder.clone() {
                        for source_path in paths {
                            if let Some(file_name) = source_path.file_name() {
                                let dest_path = target_folder.join(file_name);
                                let _ = std::fs::copy(&source_path, &dest_path);
                            }
                        }
                    }
                }
                ui.close();
            }
            if ui.button("Import Audio...").clicked() {
                if let Some(paths) = rfd::FileDialog::new()
                    .add_filter("Audio", &["wav", "ogg", "mp3", "flac"])
                    .pick_files()
                {
                    if let Some(target_folder) = assets.current_folder.clone() {
                        for source_path in paths {
                            if let Some(file_name) = source_path.file_name() {
                                let dest_path = target_folder.join(file_name);
                                let _ = std::fs::copy(&source_path, &dest_path);
                            }
                        }
                    }
                }
                ui.close();
            }
        });

        // Export submenu
        ui.menu_button("Export", |ui| {
            if ui.button("Export Game...").clicked() {
                export_state.show_dialog = true;
                ui.close();
            }
        });

        ui.separator();
        if ui.button("Exit").clicked() {
            std::process::exit(0);
        }
    });

    ui.menu_button("Edit", |ui| {
        // Undo with shortcut hint and disabled state
        let can_undo = command_history.can_undo();
        let undo_text = if can_undo {
            format!("Undo                    Ctrl+Z")
        } else {
            format!("Undo                    Ctrl+Z")
        };
        ui.add_enabled_ui(can_undo, |ui| {
            if ui.button(undo_text).clicked() {
                command_history.pending_undo = 1;
                ui.close();
            }
        });

        // Redo with shortcut hint and disabled state
        let can_redo = command_history.can_redo();
        let redo_text = format!("Redo                    Ctrl+Y");
        ui.add_enabled_ui(can_redo, |ui| {
            if ui.button(redo_text).clicked() {
                command_history.pending_redo = 1;
                ui.close();
            }
        });

        ui.separator();
        if ui.button("Cut").clicked() {
            ui.close();
        }
        if ui.button("Copy").clicked() {
            ui.close();
        }
        if ui.button("Paste").clicked() {
            ui.close();
        }
        ui.separator();
        if ui.button("Duplicate").clicked() {
            ui.close();
        }
        if ui.button("Delete").clicked() {
            if let Some(entity) = selection.selected_entity {
                queue_command(command_history, Box::new(DeleteEntityCommand::new(entity)));
            }
            ui.close();
        }
    });

    ui.menu_button("GameObject", |ui| {
        ui.menu_button("3D Object", |ui| {
            if ui.button("Cube").clicked() {
                spawn_primitive(commands, meshes, materials, PrimitiveType::Cube, "Cube", None);
                ui.close();
            }
            if ui.button("Sphere").clicked() {
                spawn_primitive(commands, meshes, materials, PrimitiveType::Sphere, "Sphere", None);
                ui.close();
            }
            if ui.button("Cylinder").clicked() {
                spawn_primitive(commands, meshes, materials, PrimitiveType::Cylinder, "Cylinder", None);
                ui.close();
            }
            if ui.button("Plane").clicked() {
                spawn_primitive(commands, meshes, materials, PrimitiveType::Plane, "Plane", None);
                ui.close();
            }
        });
        ui.menu_button("Light", |ui| {
            if ui.button("Point Light").clicked() {
                ui.close();
            }
            if ui.button("Spot Light").clicked() {
                ui.close();
            }
            if ui.button("Directional Light").clicked() {
                ui.close();
            }
        });
        if ui.button("Empty").clicked() {
            commands.spawn((
                Transform::default(),
                Visibility::default(),
                EditorEntity {
                    name: "Empty".to_string(),
                    tag: String::new(),
                    visible: true,
                    locked: false,
                },
                SceneNode,
            ));
            ui.close();
        }
    });

    // Tools menu (for plugins)
    if !tools_items.is_empty() {
        ui.menu_button("Tools", |ui| {
            for item in &tools_items {
                if let Some(event) = render_plugin_menu_item(ui, item) {
                    events.push(event);
                }
            }
        });
    }

    // Helper to apply layout and sync viewport state
    let apply_layout = |name: &str, docking: &mut DockingState, viewport: &mut ViewportState| {
        if docking.switch_layout(name) {
            // Set default viewport state based on layout name
            match name {
                "Default" => {
                    viewport.hierarchy_width = 260.0;
                    viewport.inspector_width = 320.0;
                    viewport.assets_height = 200.0;
                }
                "Scripting" => {
                    viewport.hierarchy_width = 220.0;
                    viewport.inspector_width = 300.0;
                    viewport.assets_height = 180.0;
                }
                "Animation" => {
                    viewport.hierarchy_width = 260.0;
                    viewport.inspector_width = 320.0;
                    viewport.assets_height = 250.0;
                }
                "Debug" => {
                    viewport.hierarchy_width = 300.0;
                    viewport.inspector_width = 280.0;
                    viewport.assets_height = 200.0;
                }
                _ => {}
            }
        }
    };

    // Window menu for layout management
    ui.menu_button("Window", |ui| {
        // Layouts submenu
        ui.menu_button("Layouts", |ui| {
            // Built-in layouts
            for layout in builtin_layouts() {
                let is_active = docking_state.active_layout == layout.name;
                let label = if is_active {
                    format!("\u{f00c} {}", layout.name) // checkmark
                } else {
                    format!("   {}", layout.name)
                };

                if ui.button(label).clicked() {
                    apply_layout(&layout.name, docking_state, viewport_state);
                    ui.close();
                }
            }

            // Custom layouts
            let custom_layouts: Vec<String> = docking_state.config.custom_layouts
                .iter()
                .map(|l| l.name.clone())
                .collect();

            if !custom_layouts.is_empty() {
                ui.separator();
                for name in custom_layouts {
                    let is_active = docking_state.active_layout == name;
                    let label = if is_active {
                        format!("\u{f00c} {}", name)
                    } else {
                        format!("   {}", name)
                    };

                    if ui.button(label).clicked() {
                        apply_layout(&name, docking_state, viewport_state);
                        ui.close();
                    }
                }
            }
        });

        ui.separator();

        // Quick layout shortcuts
        ui.label(egui::RichText::new("Quick Switch").color(Color32::GRAY).small());
        if ui.button("Default             Ctrl+1").clicked() {
            apply_layout("Default", docking_state, viewport_state);
            ui.close();
        }
        if ui.button("Scripting           Ctrl+2").clicked() {
            apply_layout("Scripting", docking_state, viewport_state);
            ui.close();
        }
        if ui.button("Animation           Ctrl+3").clicked() {
            apply_layout("Animation", docking_state, viewport_state);
            ui.close();
        }
        if ui.button("Debug               Ctrl+4").clicked() {
            apply_layout("Debug", docking_state, viewport_state);
            ui.close();
        }

        ui.separator();

        // Save layout
        if ui.button("Save Layout As...").clicked() {
            // TODO: Show save layout dialog
            ui.close();
        }

        if ui.button("Reset Layout").clicked() {
            apply_layout("Default", docking_state, viewport_state);
            ui.close();
        }

        ui.separator();

        // Panel visibility toggles
        ui.menu_button("Panels", |ui| {
            let all_panels = vec![
                PanelId::Hierarchy,
                PanelId::Inspector,
                PanelId::Assets,
                PanelId::Console,
                PanelId::Animation,
                PanelId::History,
            ];

            for panel in all_panels {
                let is_visible = docking_state.is_panel_visible(&panel);
                let label = format!("{} {}", panel.icon(), panel.title());

                let mut visible = is_visible;
                if ui.checkbox(&mut visible, label).changed() {
                    if visible {
                        docking_state.open_panel(panel);
                    } else {
                        docking_state.close_panel(&panel);
                    }
                }
            }
        });
    });

    // Dev menu (only visible when dev_mode is enabled)
    if settings.dev_mode {
        ui.menu_button("Dev", |ui| {
            if ui.button("New Plugin...").clicked() {
                // TODO: Show new plugin dialog
                ui.close();
            }
            ui.separator();
            if ui.button("Open Plugin Source...").clicked() {
                // Open file dialog to select a .rs file
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Rust Source", &["rs"])
                    .add_filter("All Files", &["*"])
                    .set_title("Open Plugin Source")
                    .pick_file()
                {
                    crate::ui::panels::open_script(scene_state, path);
                }
                ui.close();
            }
            if ui.button("Open Cargo.toml...").clicked() {
                // Open file dialog to select Cargo.toml
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Cargo.toml", &["toml"])
                    .set_title("Open Plugin Cargo.toml")
                    .pick_file()
                {
                    crate::ui::panels::open_script(scene_state, path);
                }
                ui.close();
            }
            ui.separator();
            ui.label(egui::RichText::new("Shortcuts").color(Color32::GRAY).small());
            ui.label(egui::RichText::new("  Ctrl+B - Build Plugin").color(Color32::GRAY).small());
            ui.label(egui::RichText::new("  Ctrl+S - Save File").color(Color32::GRAY).small());
        });
    }

    ui.menu_button("Help", |ui| {
        if ui.button("Documentation").clicked() {
            ui.close();
        }
        if ui.button("About").clicked() {
            ui.close();
        }
    });

    events
}

/// Render a plugin menu item, returns UiEvent if clicked
fn render_plugin_menu_item(ui: &mut egui::Ui, item: &MenuItem) -> Option<UiEvent> {
    if item.children.is_empty() {
        // Leaf item
        let mut text = String::new();
        if let Some(icon) = &item.icon {
            text.push_str(icon);
            text.push(' ');
        }
        text.push_str(&item.label);
        if let Some(shortcut) = &item.shortcut {
            text.push_str("    ");
            text.push_str(shortcut);
        }

        let button = egui::Button::new(&text);
        let response = ui.add_enabled(item.enabled, button);

        if response.clicked() {
            ui.close();
            return Some(UiEvent::ButtonClicked(crate::ui_api::UiId(item.id.0)));
        }
    } else {
        // Submenu
        let label = if let Some(icon) = &item.icon {
            format!("{} {}", icon, item.label)
        } else {
            item.label.clone()
        };

        ui.menu_button(label, |ui| {
            for child in &item.children {
                render_plugin_menu_item(ui, child);
            }
        });
    }

    None
}

fn window_button(ui: &mut egui::Ui, icon: &str, hover_color: Color32, width: f32) -> egui::Response {
    let size = Vec2::new(width, TITLE_BAR_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    if ui.is_rect_visible(rect) {
        let bg_color = if response.hovered() {
            hover_color
        } else {
            Color32::TRANSPARENT
        };

        ui.painter().rect_filled(rect, CornerRadius::ZERO, bg_color);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(12.0),
            if response.hovered() {
                Color32::WHITE
            } else {
                Color32::from_rgb(160, 160, 170)
            },
        );
    }

    response
}

/// Render a simplified title bar for the splash screen (no menu items)
pub fn render_splash_title_bar(
    ctx: &egui::Context,
    window_state: &mut WindowState,
) {
    let is_maximized = window_state.is_maximized;

    egui::TopBottomPanel::top("splash_title_bar")
        .exact_height(TITLE_BAR_HEIGHT)
        .frame(egui::Frame::NONE.fill(Color32::from_rgb(24, 24, 28)))
        .show(ctx, |ui| {
            let panel_rect = ui.max_rect();
            let painter = ui.painter();

            // Window button dimensions
            let button_width = 40.0;
            let window_buttons_width = button_width * 3.0;

            // Draw centered title with icon
            let title_text = "Renzora Engine r4".to_string();
            let title_galley = painter.layout_no_wrap(
                title_text.clone(),
                egui::FontId::proportional(13.0),
                Color32::from_rgb(140, 140, 150),
            );
            let title_pos = Pos2::new(
                panel_rect.center().x - title_galley.size().x / 2.0,
                panel_rect.center().y - title_galley.size().y / 2.0,
            );
            painter.galley(title_pos, title_galley, Color32::from_rgb(140, 140, 150));

            // Draw bottom border
            painter.line_segment(
                [
                    Pos2::new(panel_rect.left(), panel_rect.bottom() - 1.0),
                    Pos2::new(panel_rect.right(), panel_rect.bottom() - 1.0),
                ],
                Stroke::new(1.0, Color32::from_rgb(16, 16, 20)),
            );

            // Drag area (everything except window buttons)
            let drag_rect = egui::Rect::from_min_max(
                panel_rect.min,
                Pos2::new(panel_rect.max.x - window_buttons_width, panel_rect.max.y),
            );

            let drag_response = ui.interact(drag_rect, Id::new("splash_title_bar_drag"), Sense::click_and_drag());

            // Handle double-click to maximize
            if drag_response.double_clicked() {
                window_state.request_toggle_maximize = true;
            }

            // Handle drag
            if drag_response.drag_started() {
                window_state.start_drag = true;
            }

            if drag_response.drag_stopped() || !drag_response.dragged() && !drag_response.drag_started() {
                if window_state.is_being_dragged {
                    window_state.is_being_dragged = false;
                    window_state.drag_offset = None;
                }
            }

            // Layout window buttons on the right
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                // Fill space to push buttons to the right
                ui.add_space(ui.available_width() - window_buttons_width);

                ui.spacing_mut().item_spacing.x = 0.0;

                // Minimize button
                let min_resp = window_button(ui, MINUS, Color32::from_rgb(60, 60, 70), button_width);
                if min_resp.clicked() {
                    window_state.request_minimize = true;
                }

                // Maximize/Restore button
                let max_icon = if is_maximized { SQUARES_FOUR } else { SQUARE };
                let max_resp = window_button(ui, max_icon, Color32::from_rgb(60, 60, 70), button_width);
                if max_resp.clicked() {
                    window_state.request_toggle_maximize = true;
                }

                // Close button (red on hover)
                let close_resp = window_button(ui, X, Color32::from_rgb(200, 60, 60), button_width);
                if close_resp.clicked() {
                    window_state.request_close = true;
                }
            });
        });
}
