use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, Pos2, Sense, Vec2};

use crate::core::{EditorEntity, SceneNode, SelectionState, EditorSettings};
use crate::gizmo::{GizmoMode, GizmoState};
use crate::scene::{spawn_primitive, PrimitiveType};

// Phosphor icons for toolbar
use egui_phosphor::regular::{
    ARROWS_OUT_CARDINAL, ARROW_CLOCKWISE, ARROWS_OUT, PLAY, PAUSE, STOP, GEAR,
};

#[allow(dead_code)]
pub fn render_menu_bar(
    ctx: &egui::Context,
    selection: &mut SelectionState,
    settings: &mut EditorSettings,
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Scene").clicked() {
                    ui.close();
                }
                if ui.button("Open Scene...").clicked() {
                    ui.close();
                }
                ui.separator();
                if ui.button("Save Scene").clicked() {
                    ui.close();
                }
                if ui.button("Save Scene As...").clicked() {
                    ui.close();
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
        });
    });
}

pub fn render_toolbar(
    ctx: &egui::Context,
    gizmo: &mut GizmoState,
    settings: &mut EditorSettings,
    _menu_bar_height: f32,
    toolbar_height: f32,
    _window_width: f32,
) {
    egui::TopBottomPanel::top("toolbar")
        .exact_height(toolbar_height)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let button_size = Vec2::new(28.0, 24.0);
                let active_color = Color32::from_rgb(66, 150, 250);
                let inactive_color = Color32::from_rgb(46, 46, 56);

                // Move button (Translate)
                let is_translate = gizmo.mode == GizmoMode::Translate;
                let translate_resp = tool_button(ui, ARROWS_OUT_CARDINAL, button_size, is_translate, active_color, inactive_color);
                if translate_resp.clicked() {
                    gizmo.mode = GizmoMode::Translate;
                }
                translate_resp.on_hover_text("Move (W)");

                // Rotate button
                let is_rotate = gizmo.mode == GizmoMode::Rotate;
                let rotate_resp = tool_button(ui, ARROW_CLOCKWISE, button_size, is_rotate, active_color, inactive_color);
                if rotate_resp.clicked() {
                    gizmo.mode = GizmoMode::Rotate;
                }
                rotate_resp.on_hover_text("Rotate (E)");

                // Scale button
                let is_scale = gizmo.mode == GizmoMode::Scale;
                let scale_resp = tool_button(ui, ARROWS_OUT, button_size, is_scale, active_color, inactive_color);
                if scale_resp.clicked() {
                    gizmo.mode = GizmoMode::Scale;
                }
                scale_resp.on_hover_text("Scale (R)");

                ui.add_space(12.0);

                // Separator
                let rect = ui.available_rect_before_wrap();
                ui.painter().line_segment(
                    [Pos2::new(rect.left(), rect.top() + 4.0), Pos2::new(rect.left(), rect.bottom() - 4.0)],
                    egui::Stroke::new(1.0, Color32::from_rgb(77, 77, 89)),
                );

                ui.add_space(12.0);

                // Play controls
                let play_color = Color32::from_rgb(64, 166, 89);
                let play_resp = tool_button(ui, PLAY, button_size, false, play_color, inactive_color);
                if play_resp.clicked() {
                    // Play
                }
                play_resp.on_hover_text("Play");

                let pause_resp = tool_button(ui, PAUSE, button_size, false, active_color, inactive_color);
                if pause_resp.clicked() {
                    // Pause
                }
                pause_resp.on_hover_text("Pause");

                let stop_resp = tool_button(ui, STOP, button_size, false, Color32::from_rgb(200, 80, 80), inactive_color);
                if stop_resp.clicked() {
                    // Stop
                }
                stop_resp.on_hover_text("Stop");

                // Spacer to push settings to the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Settings button
                    let settings_resp = tool_button(ui, GEAR, button_size, settings.show_settings_window, active_color, inactive_color);
                    if settings_resp.clicked() {
                        settings.show_settings_window = !settings.show_settings_window;
                    }
                    settings_resp.on_hover_text("Settings (Ctrl+,)");
                });
            });
        });

    // Show demo window if enabled
    if settings.show_demo_window {
        egui::Window::new("egui Demo").show(ctx, |ui| {
            ui.label("This is the egui demo window.");
            if ui.button("Close").clicked() {
                settings.show_demo_window = false;
            }
        });
    }
}

fn tool_button(
    ui: &mut egui::Ui,
    label: &str,
    size: Vec2,
    active: bool,
    active_color: Color32,
    inactive_color: Color32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    if ui.is_rect_visible(rect) {
        let bg_color = if active {
            active_color
        } else if response.hovered() {
            Color32::from_rgb(56, 56, 68)
        } else {
            inactive_color
        };

        ui.painter().rect_filled(rect, CornerRadius::same(4), bg_color);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(14.0),
            Color32::WHITE,
        );
    }

    response
}
