use bevy::prelude::*;
use bevy::window::{WindowMode, WindowPosition};
use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, Id, Pos2, Sense, Stroke, Vec2};

use crate::commands::{CommandHistory, DeleteEntityCommand, queue_command};
use crate::core::{AssetBrowserState, DockingState, ExportState, SelectionState, ViewportState, WindowState, SceneManagerState, EditorSettings, ResizeEdge};
use crate::plugin_core::{MenuLocation, MenuItem, PluginHost};
use crate::theming::Theme;
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
    plugin_host: &PluginHost,
    command_history: &mut CommandHistory,
    docking_state: &mut DockingState,
    viewport_state: &mut ViewportState,
    theme: &Theme,
) -> Vec<UiEvent> {
    let mut ui_events = Vec::new();
    let is_maximized = window_state.is_maximized;

    egui::TopBottomPanel::top("title_bar")
        .exact_height(TITLE_BAR_HEIGHT)
        .frame(egui::Frame::NONE.fill(theme.surfaces.window.to_color32()))
        .show(ctx, |ui| {
            let panel_rect = ui.max_rect();
            let painter = ui.painter();

            // Window button dimensions
            let button_width = 40.0;
            let window_buttons_width = button_width * 3.0;

            // Draw centered title with icon
            let title_text = "Renzora Engine r1".to_string();
            let title_galley = painter.layout_no_wrap(
                title_text.clone(),
                egui::FontId::proportional(13.0),
                theme.text.muted.to_color32(),
            );
            let title_pos = Pos2::new(
                panel_rect.center().x - title_galley.size().x / 2.0,
                panel_rect.center().y - title_galley.size().y / 2.0,
            );
            painter.galley(title_pos, title_galley, theme.text.muted.to_color32());

            // Draw bottom border
            painter.line_segment(
                [
                    Pos2::new(panel_rect.left(), panel_rect.bottom() - 1.0),
                    Pos2::new(panel_rect.right(), panel_rect.bottom() - 1.0),
                ],
                Stroke::new(1.0, theme.surfaces.extreme.to_color32()),
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
                ui_events = render_menu_items(ui, selection, scene_state, settings, export_state, assets, plugin_host, command_history, docking_state, viewport_state, theme);

                // Fill remaining space
                ui.add_space(ui.available_width() - window_buttons_width);

                // Window buttons on the right
                ui.spacing_mut().item_spacing.x = 0.0;

                // Minimize button
                let min_resp = window_button(ui, MINUS, theme.widgets.hovered_bg.to_color32(), button_width);
                if min_resp.clicked() {
                    window_state.request_minimize = true;
                }

                // Maximize/Restore button
                let max_icon = if is_maximized { SQUARES_FOUR } else { SQUARE };
                let max_resp = window_button(ui, max_icon, theme.widgets.hovered_bg.to_color32(), button_width);
                if max_resp.clicked() {
                    window_state.request_toggle_maximize = true;
                }

                // Close button (red on hover)
                let close_resp = window_button(ui, X, theme.panels.close_hover.to_color32(), button_width);
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
    plugin_host: &PluginHost,
    command_history: &mut CommandHistory,
    docking_state: &mut DockingState,
    viewport_state: &mut ViewportState,
    theme: &Theme,
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
    ui.style_mut().visuals.widgets.hovered.weak_bg_fill = theme.widgets.hovered_bg.to_color32();
    ui.style_mut().visuals.widgets.active.weak_bg_fill = theme.widgets.active_bg.to_color32();

    submenu(ui, "File", |ui| {
        if menu_item(ui, "New Scene") {
            scene_state.new_scene_requested = true;
            ui.close();
        }
        if menu_item(ui, "Open Scene...") {
            scene_state.open_scene_requested = true;
            ui.close();
        }
        ui.separator();
        if menu_item(ui, "Save Scene        Ctrl+S") {
            scene_state.save_scene_requested = true;
            ui.close();
        }
        if menu_item(ui, "Save Scene As...  Ctrl+Shift+S") {
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
        submenu(ui, "Import", |ui| {
            if menu_item(ui, "Import Assets...") {
                assets.import_asset_requested = true;
                ui.close();
            }
            ui.separator();
            if menu_item(ui, "Import 3D Model...") {
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
            if menu_item(ui, "Import Image...") {
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
            if menu_item(ui, "Import Audio...") {
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
        submenu(ui, "Export", |ui| {
            if menu_item(ui, "Export Game...") {
                export_state.show_dialog = true;
                ui.close();
            }
        });

        ui.separator();
        if menu_item(ui, "Settings                Ctrl+,") {
            docking_state.open_panel(PanelId::Settings);
            ui.close();
        }
        ui.separator();
        if menu_item(ui, "Exit") {
            std::process::exit(0);
        }
    });

    submenu(ui, "Edit", |ui| {
        // Undo with shortcut hint and disabled state
        let can_undo = command_history.can_undo();
        if menu_item_enabled(ui, "Undo                    Ctrl+Z", can_undo) {
            command_history.pending_undo = 1;
            ui.close();
        }

        // Redo with shortcut hint and disabled state
        let can_redo = command_history.can_redo();
        if menu_item_enabled(ui, "Redo                    Ctrl+Y", can_redo) {
            command_history.pending_redo = 1;
            ui.close();
        }

        ui.separator();
        if menu_item(ui, "Cut") {
            ui.close();
        }
        if menu_item(ui, "Copy") {
            ui.close();
        }
        if menu_item(ui, "Paste") {
            ui.close();
        }
        ui.separator();
        if menu_item(ui, "Duplicate") {
            ui.close();
        }
        if menu_item(ui, "Delete") {
            if let Some(entity) = selection.selected_entity {
                queue_command(command_history, Box::new(DeleteEntityCommand::new(entity)));
            }
            ui.close();
        }
    });

    // Tools menu (for plugins)
    if !tools_items.is_empty() {
        submenu(ui, "Tools", |ui| {
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
    submenu(ui, "Window", |ui| {
        // Layouts submenu
        submenu(ui, "Layouts", |ui| {
            // Built-in layouts
            for layout in builtin_layouts() {
                let is_active = docking_state.active_layout == layout.name;
                let label = if is_active {
                    format!("\u{f00c} {}", layout.name) // checkmark
                } else {
                    format!("   {}", layout.name)
                };

                if menu_item(ui, &label) {
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

                    if menu_item(ui, &label) {
                        apply_layout(&name, docking_state, viewport_state);
                        ui.close();
                    }
                }
            }
        });

        ui.separator();

        // Quick layout shortcuts
        ui.label(egui::RichText::new("Quick Switch").color(Color32::GRAY).small());
        if menu_item(ui, "Default             Ctrl+1") {
            apply_layout("Default", docking_state, viewport_state);
            ui.close();
        }
        if menu_item(ui, "Scripting           Ctrl+2") {
            apply_layout("Scripting", docking_state, viewport_state);
            ui.close();
        }
        if menu_item(ui, "Animation           Ctrl+3") {
            apply_layout("Animation", docking_state, viewport_state);
            ui.close();
        }
        if menu_item(ui, "Debug               Ctrl+4") {
            apply_layout("Debug", docking_state, viewport_state);
            ui.close();
        }

        ui.separator();

        // Save layout
        if menu_item(ui, "Save Layout As...") {
            // TODO: Show save layout dialog
            ui.close();
        }

        if menu_item(ui, "Reset Layout") {
            apply_layout("Default", docking_state, viewport_state);
            ui.close();
        }

        ui.separator();

        // Panel visibility toggles
        submenu(ui, "Panels", |ui| {
            let all_panels = vec![
                PanelId::Hierarchy,
                PanelId::Inspector,
                PanelId::Assets,
                PanelId::Console,
                PanelId::Animation,
                PanelId::History,
                PanelId::Settings,
                PanelId::Gamepad,
                PanelId::Performance,
                PanelId::RenderStats,
            ];

            for panel in all_panels {
                let is_visible = docking_state.is_panel_visible(&panel);
                let label = format!("{} {}", panel.icon(), panel.title());

                let mut visible = is_visible;
                let checkbox = ui.checkbox(&mut visible, label);
                if checkbox.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                if checkbox.changed() {
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
        submenu(ui, "Dev", |ui| {
            if menu_item(ui, "New Plugin...") {
                // TODO: Show new plugin dialog
                ui.close();
            }
            ui.separator();
            if menu_item(ui, "Open Plugin Source...") {
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
            if menu_item(ui, "Open Cargo.toml...") {
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

    submenu(ui, "Help", |ui| {
        if menu_item(ui, "Documentation") {
            ui.close();
        }
        ui.separator();
        if menu_item(ui, "Discord") {
            let _ = open::that("https://discord.gg/9UHUGUyDJv");
            ui.close();
        }
        if menu_item(ui, "YouTube") {
            let _ = open::that("https://youtube.com/@renzoragame");
            ui.close();
        }
        ui.separator();
        if menu_item(ui, "About") {
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

        if response.hovered() && item.enabled {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }

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

        submenu(ui, &label, |ui| {
            for child in &item.children {
                render_plugin_menu_item(ui, child);
            }
        });
    }

    None
}

/// Helper for menu items - shows pointer cursor on hover and returns if clicked
fn menu_item(ui: &mut egui::Ui, label: &str) -> bool {
    let btn = ui.button(label);
    if btn.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    btn.clicked()
}

/// Helper for menu items with enabled state
fn menu_item_enabled(ui: &mut egui::Ui, label: &str, enabled: bool) -> bool {
    let btn = ui.add_enabled(enabled, egui::Button::new(label));
    if btn.hovered() && enabled {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    btn.clicked()
}


/// Helper for nested submenus - shows pointer cursor on hover
fn submenu(ui: &mut egui::Ui, label: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    let response = ui.menu_button(label, add_contents);
    if response.response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
}

fn window_button(ui: &mut egui::Ui, icon: &str, hover_color: Color32, width: f32) -> egui::Response {
    let size = Vec2::new(width, TITLE_BAR_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

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
    theme: &Theme,
) {
    let is_maximized = window_state.is_maximized;

    egui::TopBottomPanel::top("splash_title_bar")
        .exact_height(TITLE_BAR_HEIGHT)
        .frame(egui::Frame::NONE.fill(theme.surfaces.window.to_color32()))
        .show(ctx, |ui| {
            let panel_rect = ui.max_rect();
            let painter = ui.painter();

            // Window button dimensions
            let button_width = 40.0;
            let window_buttons_width = button_width * 3.0;

            // Draw centered title with icon
            let title_text = "Renzora Engine r1".to_string();
            let title_galley = painter.layout_no_wrap(
                title_text.clone(),
                egui::FontId::proportional(13.0),
                theme.text.muted.to_color32(),
            );
            let title_pos = Pos2::new(
                panel_rect.center().x - title_galley.size().x / 2.0,
                panel_rect.center().y - title_galley.size().y / 2.0,
            );
            painter.galley(title_pos, title_galley, theme.text.muted.to_color32());

            // Draw bottom border
            painter.line_segment(
                [
                    Pos2::new(panel_rect.left(), panel_rect.bottom() - 1.0),
                    Pos2::new(panel_rect.right(), panel_rect.bottom() - 1.0),
                ],
                Stroke::new(1.0, theme.surfaces.extreme.to_color32()),
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
                let min_resp = window_button(ui, MINUS, theme.widgets.hovered_bg.to_color32(), button_width);
                if min_resp.clicked() {
                    window_state.request_minimize = true;
                }

                // Maximize/Restore button
                let max_icon = if is_maximized { SQUARES_FOUR } else { SQUARE };
                let max_resp = window_button(ui, max_icon, theme.widgets.hovered_bg.to_color32(), button_width);
                if max_resp.clicked() {
                    window_state.request_toggle_maximize = true;
                }

                // Close button (red on hover)
                let close_resp = window_button(ui, X, theme.panels.close_hover.to_color32(), button_width);
                if close_resp.clicked() {
                    window_state.request_close = true;
                }
            });
        });
}
