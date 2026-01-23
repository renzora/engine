use bevy::prelude::*;
use bevy::window::{WindowMode, WindowPosition};
use bevy_egui::egui::{self, Color32, CornerRadius, Id, Pos2, Sense, Stroke, Vec2};

use crate::core::{EditorEntity, SceneNode, SelectionState, WindowState, SceneManagerState, EditorSettings};
use crate::plugin_core::{MenuLocation, MenuItem, PluginHost};
use crate::scene::{spawn_primitive, PrimitiveType};
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
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    plugin_host: &PluginHost,
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
            let icon_text = format!("{} Renzora Engine", SQUARES_FOUR);
            let title_galley = painter.layout_no_wrap(
                icon_text.clone(),
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
                ui_events = render_menu_items(ui, selection, scene_state, settings, commands, meshes, materials, plugin_host);

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
}

fn render_menu_items(
    ui: &mut egui::Ui,
    selection: &mut SelectionState,
    scene_state: &mut SceneManagerState,
    settings: &mut EditorSettings,
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    plugin_host: &PluginHost,
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
        if ui.button("Exit").clicked() {
            std::process::exit(0);
        }
    });

    ui.menu_button("Edit", |ui| {
        if ui.button("Undo").clicked() {
            ui.close();
        }
        if ui.button("Redo").clicked() {
            ui.close();
        }
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
                commands.entity(entity).despawn();
                selection.selected_entity = None;
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

    ui.menu_button("View", |ui| {
        ui.checkbox(&mut settings.show_demo_window, "egui Demo");
    });

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
            let icon_text = format!("{} Renzora Engine", SQUARES_FOUR);
            let title_galley = painter.layout_no_wrap(
                icon_text.clone(),
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
