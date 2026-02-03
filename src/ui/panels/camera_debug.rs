//! Camera debug panel for camera inspection

use bevy_egui::egui::{self, Color32, CursorIcon, RichText};

use crate::core::{CameraDebugState, CameraProjectionType};
use crate::theming::Theme;

/// Render the camera debug panel content
pub fn render_camera_debug_content(
    ui: &mut egui::Ui,
    state: &mut CameraDebugState,
    theme: &Theme,
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());

                // Camera Count Header
                render_camera_count_header(ui, state, theme);

                ui.add_space(12.0);

                // Camera List
                render_camera_list(ui, state, theme);

                ui.add_space(16.0);

                // Selected Camera Details
                if state.selected_camera.is_some() {
                    render_selected_camera_details(ui, state, theme);
                    ui.add_space(16.0);
                }

                // Gizmo Toggles
                render_gizmo_toggles(ui, state, theme);
            });
        });
}

fn render_camera_count_header(ui: &mut egui::Ui, state: &CameraDebugState, theme: &Theme) {
    let scene_count = state.scene_camera_count();
    let total_count = state.cameras.len();

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{}", scene_count))
                .size(28.0)
                .color(theme.text.primary.to_color32())
                .strong(),
        );
        ui.label(RichText::new("scene cameras").size(12.0).color(theme.text.muted.to_color32()));
    });

    if total_count > scene_count {
        ui.label(
            RichText::new(format!("+ {} editor cameras", total_count - scene_count))
                .size(10.0)
                .color(theme.text.muted.to_color32()),
        );
    }
}

fn render_camera_list(ui: &mut egui::Ui, state: &mut CameraDebugState, theme: &Theme) {
    ui.label(RichText::new("Cameras").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    if state.cameras.is_empty() {
        ui.label(
            RichText::new("No cameras in scene")
                .size(11.0)
                .color(theme.text.muted.to_color32()),
        );
        return;
    }

    egui::Frame::NONE
        .fill(Color32::from_rgb(35, 37, 42))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::same(4))
        .show(ui, |ui| {
            for camera in &state.cameras {
                let is_selected = state.selected_camera == Some(camera.entity);

                let bg_color = if is_selected {
                    Color32::from_rgb(60, 80, 120)
                } else if camera.is_editor_camera {
                    Color32::from_rgb(40, 42, 48)
                } else {
                    Color32::TRANSPARENT
                };

                egui::Frame::NONE
                    .fill(bg_color)
                    .corner_radius(2.0)
                    .inner_margin(egui::Margin::symmetric(6, 4))
                    .show(ui, |ui| {
                        let response = ui.horizontal(|ui| {
                            // Status indicator
                            let status_color = if camera.is_active {
                                Color32::from_rgb(100, 200, 100)
                            } else {
                                Color32::from_rgb(120, 120, 130)
                            };
                            ui.label(RichText::new("\u{25cf}").size(8.0).color(status_color));

                            // Camera icon and name
                            let icon = if camera.is_editor_camera { "\u{e3af}" } else { "\u{e412}" };
                            let name_color = if camera.is_editor_camera {
                                theme.text.muted.to_color32()
                            } else {
                                theme.text.primary.to_color32()
                            };

                            ui.label(RichText::new(icon).size(12.0).color(name_color));
                            ui.label(RichText::new(&camera.name).size(11.0).color(name_color));

                            // Projection type badge
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let proj_text = match camera.projection_type {
                                    CameraProjectionType::Perspective => "P",
                                    CameraProjectionType::Orthographic => "O",
                                };
                                ui.label(
                                    RichText::new(proj_text)
                                        .size(9.0)
                                        .color(theme.text.muted.to_color32())
                                        .monospace(),
                                );

                                // Order badge
                                ui.label(
                                    RichText::new(format!("#{}", camera.order))
                                        .size(9.0)
                                        .color(theme.text.muted.to_color32()),
                                );
                            });
                        });

                        let camera_interact = response.response.interact(egui::Sense::click());
                        if camera_interact.hovered() {
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        }
                        if camera_interact.clicked() {
                            state.selected_camera = Some(camera.entity);
                        }
                    });
            }
        });
}

fn render_selected_camera_details(ui: &mut egui::Ui, state: &CameraDebugState, theme: &Theme) {
    let Some(camera) = state.selected_camera_info() else {
        return;
    };

    ui.label(RichText::new("Selected Camera").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    egui::Frame::NONE
        .fill(Color32::from_rgb(35, 37, 42))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            // Name and entity
            ui.horizontal(|ui| {
                ui.label(RichText::new(&camera.name).size(13.0).color(theme.text.primary.to_color32()).strong());
                ui.label(
                    RichText::new(format!("({:?})", camera.entity))
                        .size(9.0)
                        .color(theme.text.muted.to_color32())
                        .monospace(),
                );
            });

            ui.separator();

            // Projection section
            ui.label(RichText::new("Projection").size(10.0).color(theme.text.secondary.to_color32()));

            egui::Grid::new("camera_projection_grid")
                .num_columns(2)
                .spacing([12.0, 2.0])
                .show(ui, |ui| {
                    grid_row(ui, "Type", &format!("{}", camera.projection_type), theme);

                    if let Some(fov) = camera.fov_degrees {
                        grid_row(ui, "FOV", &format!("{:.1}\u{00b0}", fov), theme);
                    }

                    if let Some(scale) = camera.ortho_scale {
                        grid_row(ui, "Scale", &format!("{:.2}", scale), theme);
                    }

                    grid_row(ui, "Near", &format!("{:.3}", camera.near), theme);
                    grid_row(ui, "Far", &format!("{:.1}", camera.far), theme);
                    grid_row(ui, "Aspect", &format!("{:.2}", camera.aspect_ratio), theme);
                });

            ui.add_space(8.0);

            // Transform section
            ui.label(RichText::new("Transform").size(10.0).color(theme.text.secondary.to_color32()));

            egui::Grid::new("camera_transform_grid")
                .num_columns(2)
                .spacing([12.0, 2.0])
                .show(ui, |ui| {
                    grid_row(ui, "Position", &format_vec3(camera.position), theme);
                    grid_row(ui, "Rotation", &format_vec3(camera.rotation_degrees), theme);
                    grid_row(ui, "Forward", &format_vec3(camera.forward), theme);
                });

            // Clear color
            if let Some(color) = camera.clear_color {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Clear:").size(10.0).color(theme.text.secondary.to_color32()));

                    // Color preview
                    let rgba = color.to_srgba();
                    let preview_color = Color32::from_rgba_unmultiplied(
                        (rgba.red * 255.0) as u8,
                        (rgba.green * 255.0) as u8,
                        (rgba.blue * 255.0) as u8,
                        (rgba.alpha * 255.0) as u8,
                    );
                    let (rect, _) = ui.allocate_exact_size(egui::Vec2::splat(12.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 2.0, preview_color);
                    ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0, Color32::from_gray(80)), egui::StrokeKind::Inside);

                    ui.label(
                        RichText::new(format!("({:.2}, {:.2}, {:.2}, {:.2})", rgba.red, rgba.green, rgba.blue, rgba.alpha))
                            .size(9.0)
                            .color(theme.text.muted.to_color32())
                            .monospace(),
                    );
                });
            }

            // Viewport info
            if let Some(viewport) = camera.viewport {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Viewport:").size(10.0).color(theme.text.secondary.to_color32()));
                    ui.label(
                        RichText::new(format!("[{:.0}, {:.0}, {:.0}x{:.0}]", viewport[0], viewport[1], viewport[2], viewport[3]))
                            .size(9.0)
                            .color(theme.text.muted.to_color32())
                            .monospace(),
                    );
                });
            }
        });
}

fn render_gizmo_toggles(ui: &mut egui::Ui, state: &mut CameraDebugState, theme: &Theme) {
    ui.label(RichText::new("Debug Visualization").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    egui::Frame::NONE
        .fill(Color32::from_rgb(35, 37, 42))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.checkbox(&mut state.show_frustum_gizmos, "Show Frustum (selected)");
            ui.checkbox(&mut state.show_camera_axes, "Show Camera Axes");
            ui.checkbox(&mut state.show_all_frustums, "Show All Frustums");

            ui.add_space(8.0);

            // Frustum color picker
            ui.horizontal(|ui| {
                ui.label(RichText::new("Frustum Color:").size(10.0).color(theme.text.secondary.to_color32()));

                let rgba = state.frustum_color.to_srgba();
                let mut color = [rgba.red, rgba.green, rgba.blue, rgba.alpha];
                if ui.color_edit_button_rgba_unmultiplied(&mut color).changed() {
                    state.frustum_color = bevy::prelude::Color::srgba(color[0], color[1], color[2], color[3]);
                }
            });
        });
}

fn grid_row(ui: &mut egui::Ui, label: &str, value: &str, theme: &Theme) {
    ui.label(RichText::new(label).size(10.0).color(theme.text.muted.to_color32()));
    ui.label(RichText::new(value).size(10.0).color(theme.text.primary.to_color32()).monospace());
    ui.end_row();
}

fn format_vec3(v: bevy::prelude::Vec3) -> String {
    format!("({:.2}, {:.2}, {:.2})", v.x, v.y, v.z)
}
