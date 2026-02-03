//! Inspector widget for world environment

use bevy_egui::egui::{self, Color32, RichText, Sense, Vec2};
use std::path::PathBuf;

use crate::core::{AssetBrowserState, WorldEnvironmentMarker};
use crate::shared::{SkyMode, TonemappingMode};
use crate::ui::{inline_property, get_inspector_theme};
use super::utils::sanitize_f32;

// Phosphor icons
use egui_phosphor::regular::{FILE, FOLDER_OPEN, IMAGE, X_CIRCLE};

/// Helper to convert float RGB (0-1) to Color32 and back
fn rgb_to_color32(r: f32, g: f32, b: f32) -> Color32 {
    Color32::from_rgb(
        (r * 255.0) as u8,
        (g * 255.0) as u8,
        (b * 255.0) as u8,
    )
}

fn color32_to_rgb(color: Color32) -> (f32, f32, f32) {
    (
        color.r() as f32 / 255.0,
        color.g() as f32 / 255.0,
        color.b() as f32 / 255.0,
    )
}

/// Render a collapsible section header, returns true if section is expanded
fn section_header(ui: &mut egui::Ui, id: &str, title: &str, default_open: bool) -> bool {
    let id = ui.make_persistent_id(id);
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, default_open);
    let theme_colors = get_inspector_theme(ui.ctx());

    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 20.0), Sense::click());

    if response.clicked() {
        state.toggle(ui);
    }

    let bg_color = if response.hovered() {
        theme_colors.widget_hovered_bg
    } else {
        theme_colors.widget_inactive_bg
    };

    ui.painter().rect_filled(rect, 0.0, bg_color);
    ui.painter().text(
        rect.left_center() + egui::vec2(8.0, 0.0),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(12.0),
        theme_colors.text_secondary,
    );

    state.store(ui.ctx());
    state.is_open()
}

/// Copy a sky HDR/EXR file to the project's assets/environments folder and return the relative path
fn copy_sky_to_project_assets(source_path: &PathBuf, project_path: Option<&PathBuf>) -> Option<String> {
    let project = project_path?;

    // Get the file name
    let file_name = source_path.file_name()?;

    // Create the assets/environments directory if it doesn't exist
    let environments_dir = project.join("assets").join("environments");
    if let Err(e) = std::fs::create_dir_all(&environments_dir) {
        bevy::log::error!("Failed to create environments directory: {}", e);
        return None;
    }

    // Destination path
    let dest_path = environments_dir.join(file_name);

    // Copy the file if it's not already in the project
    if !dest_path.exists() || source_path.canonicalize().ok() != dest_path.canonicalize().ok() {
        if let Err(e) = std::fs::copy(source_path, &dest_path) {
            bevy::log::error!("Failed to copy sky texture to project: {}", e);
            return None;
        }
        bevy::log::info!("Copied sky texture to project: {:?}", dest_path);
    }

    // Return relative path from project root (using forward slashes for cross-platform)
    Some(format!("assets/environments/{}", file_name.to_string_lossy()))
}

/// Render the world environment inspector
pub fn render_world_environment_inspector(
    ui: &mut egui::Ui,
    world_env: &mut WorldEnvironmentMarker,
    assets: &AssetBrowserState,
    project_path: Option<&PathBuf>,
) -> bool {
    let mut changed = false;
    let data = &mut world_env.data;
    let mut row;

    // Ambient Light section
    if section_header(ui, "env_ambient", "Ambient Light", true) {
        // Sanitize values
        sanitize_f32(&mut data.ambient_brightness, 0.0, 1000.0, 300.0);

        row = 0;
        changed |= inline_property(ui, row, "Color", |ui| {
            let mut color = rgb_to_color32(data.ambient_color.0, data.ambient_color.1, data.ambient_color.2);
            let resp = ui.color_edit_button_srgba(&mut color).changed();
            if resp {
                data.ambient_color = color32_to_rgb(color);
            }
            resp
        });
        row += 1;

        changed |= inline_property(ui, row, "Brightness", |ui| {
            ui.add(egui::DragValue::new(&mut data.ambient_brightness).speed(10.0).range(0.0..=1000.0)).changed()
        });
    }

    // Sky section
    if section_header(ui, "env_sky", "Sky", true) {
        row = 0;
        inline_property(ui, row, "Mode", |ui| {
            let sky_options = ["Color", "Procedural", "Panorama"];
            let mut sky_index = match data.sky_mode {
                SkyMode::Color => 0,
                SkyMode::Procedural => 1,
                SkyMode::Panorama => 2,
            };
            egui::ComboBox::from_id_salt("sky_mode_combo")
                .selected_text(sky_options[sky_index])
                .show_ui(ui, |ui| {
                    for (i, option) in sky_options.iter().enumerate() {
                        if ui.selectable_value(&mut sky_index, i, *option).changed() {
                            data.sky_mode = match sky_index {
                                0 => SkyMode::Color,
                                1 => SkyMode::Procedural,
                                2 => SkyMode::Panorama,
                                _ => SkyMode::Color,
                            };
                            changed = true;
                        }
                    }
                });
        });
        row += 1;

        match data.sky_mode {
            SkyMode::Color => {
                changed |= inline_property(ui, row, "Background", |ui| {
                    let mut color = rgb_to_color32(data.clear_color.0, data.clear_color.1, data.clear_color.2);
                    let resp = ui.color_edit_button_srgba(&mut color).changed();
                    if resp {
                        data.clear_color = color32_to_rgb(color);
                    }
                    resp
                });
            }
            SkyMode::Procedural => {
                let sky = &mut data.procedural_sky;

                // Sanitize all float values to prevent egui panics
                sanitize_f32(&mut sky.sky_curve, 0.01, 1.0, 0.15);
                sanitize_f32(&mut sky.ground_curve, 0.01, 1.0, 0.02);
                sanitize_f32(&mut sky.sun_energy, 0.0, 10.0, 1.0);
                sanitize_f32(&mut sky.sun_disk_scale, 0.0, 5.0, 1.0);
                sanitize_f32(&mut sky.sun_angle_azimuth, -180.0, 180.0, 0.0);
                sanitize_f32(&mut sky.sun_angle_elevation, -90.0, 90.0, 45.0);

                changed |= inline_property(ui, row, "Top Color", |ui| {
                    let mut color = rgb_to_color32(sky.sky_top_color.0, sky.sky_top_color.1, sky.sky_top_color.2);
                    let resp = ui.color_edit_button_srgba(&mut color).changed();
                    if resp {
                        sky.sky_top_color = color32_to_rgb(color);
                    }
                    resp
                });
                row += 1;

                changed |= inline_property(ui, row, "Horizon Color", |ui| {
                    let mut color = rgb_to_color32(sky.sky_horizon_color.0, sky.sky_horizon_color.1, sky.sky_horizon_color.2);
                    let resp = ui.color_edit_button_srgba(&mut color).changed();
                    if resp {
                        sky.sky_horizon_color = color32_to_rgb(color);
                    }
                    resp
                });
                row += 1;

                changed |= inline_property(ui, row, "Sky Curve", |ui| {
                    ui.add(egui::DragValue::new(&mut sky.sky_curve).speed(0.01).range(0.01..=1.0)).changed()
                });
                row += 1;

                changed |= inline_property(ui, row, "Ground Horizon", |ui| {
                    let mut color = rgb_to_color32(sky.ground_horizon_color.0, sky.ground_horizon_color.1, sky.ground_horizon_color.2);
                    let resp = ui.color_edit_button_srgba(&mut color).changed();
                    if resp {
                        sky.ground_horizon_color = color32_to_rgb(color);
                    }
                    resp
                });
                row += 1;

                changed |= inline_property(ui, row, "Ground Bottom", |ui| {
                    let mut color = rgb_to_color32(sky.ground_bottom_color.0, sky.ground_bottom_color.1, sky.ground_bottom_color.2);
                    let resp = ui.color_edit_button_srgba(&mut color).changed();
                    if resp {
                        sky.ground_bottom_color = color32_to_rgb(color);
                    }
                    resp
                });
                row += 1;

                changed |= inline_property(ui, row, "Ground Curve", |ui| {
                    ui.add(egui::DragValue::new(&mut sky.ground_curve).speed(0.01).range(0.01..=1.0)).changed()
                });
                row += 1;

                changed |= inline_property(ui, row, "Sun Azimuth", |ui| {
                    ui.add(egui::DragValue::new(&mut sky.sun_angle_azimuth).speed(1.0).range(-180.0..=180.0).suffix("°")).changed()
                });
                row += 1;

                changed |= inline_property(ui, row, "Sun Elevation", |ui| {
                    ui.add(egui::DragValue::new(&mut sky.sun_angle_elevation).speed(1.0).range(-90.0..=90.0).suffix("°")).changed()
                });
                row += 1;

                changed |= inline_property(ui, row, "Sun Color", |ui| {
                    let mut color = rgb_to_color32(sky.sun_color.0, sky.sun_color.1, sky.sun_color.2);
                    let resp = ui.color_edit_button_srgba(&mut color).changed();
                    if resp {
                        sky.sun_color = color32_to_rgb(color);
                    }
                    resp
                });
                row += 1;

                changed |= inline_property(ui, row, "Sun Energy", |ui| {
                    ui.add(egui::DragValue::new(&mut sky.sun_energy).speed(0.1).range(0.0..=10.0)).changed()
                });
                row += 1;

                changed |= inline_property(ui, row, "Disk Scale", |ui| {
                    ui.add(egui::DragValue::new(&mut sky.sun_disk_scale).speed(0.1).range(0.0..=5.0)).changed()
                });
            }
            SkyMode::Panorama => {
                let pano = &mut data.panorama_sky;
                let theme_colors = get_inspector_theme(ui.ctx());

                // Sanitize float values
                sanitize_f32(&mut pano.rotation, 0.0, 360.0, 0.0);
                sanitize_f32(&mut pano.energy, 0.0, 10.0, 1.0);

                // Sky texture label
                ui.horizontal(|ui| {
                    ui.label("Sky Texture");
                });
                ui.add_space(4.0);

                // Create a drop zone for HDR/EXR files
                let drop_zone_height = 60.0;
                let available_width = ui.available_width();

                ui.horizontal(|ui| {
                    // Drop zone takes most of the width
                    let drop_width = available_width - 34.0;
                    let (rect, response) = ui.allocate_exact_size(
                        Vec2::new(drop_width, drop_zone_height),
                        Sense::click_and_drag(),
                    );

                    // Check if we're currently dragging an asset
                    let is_drag_target = assets.dragging_asset.is_some();
                    let is_hovered = response.hovered();

                    // Check if dragging asset is a valid HDR/EXR file
                    let is_valid_drop = if let Some(dragging_path) = &assets.dragging_asset {
                        let ext = dragging_path.extension()
                            .and_then(|e| e.to_str())
                            .map(|s| s.to_lowercase())
                            .unwrap_or_default();
                        matches!(ext.as_str(), "hdr" | "exr")
                    } else {
                        false
                    };

                    // Background color based on state
                    let bg_color = if is_drag_target && is_hovered && is_valid_drop {
                        theme_colors.semantic_accent
                    } else if is_drag_target && is_valid_drop {
                        theme_colors.surface_faint
                    } else {
                        theme_colors.widget_inactive_bg
                    };

                    ui.painter().rect_filled(rect, 4.0, bg_color);
                    ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(1.0, theme_colors.widget_border), egui::StrokeKind::Outside);

                    // Content inside drop zone
                    if !pano.panorama_path.is_empty() {
                        // Show current texture
                        let file_name = pano.panorama_path.rsplit('/').next()
                            .or_else(|| pano.panorama_path.rsplit('\\').next())
                            .unwrap_or(&pano.panorama_path);

                        let center = rect.center();

                        // Icon
                        ui.painter().text(
                            egui::pos2(center.x, center.y - 10.0),
                            egui::Align2::CENTER_CENTER,
                            IMAGE,
                            egui::FontId::proportional(24.0),
                            theme_colors.semantic_warning,
                        );

                        // File name
                        ui.painter().text(
                            egui::pos2(center.x, center.y + 14.0),
                            egui::Align2::CENTER_CENTER,
                            file_name,
                            egui::FontId::proportional(12.0),
                            theme_colors.text_primary,
                        );
                    } else {
                        // Show empty state with hint
                        let center = rect.center();

                        if is_drag_target && is_valid_drop {
                            ui.painter().text(
                                center,
                                egui::Align2::CENTER_CENTER,
                                "Drop HDR/EXR here",
                                egui::FontId::proportional(12.0),
                                theme_colors.semantic_accent,
                            );
                        } else {
                            ui.painter().text(
                                egui::pos2(center.x, center.y - 8.0),
                                egui::Align2::CENTER_CENTER,
                                FILE,
                                egui::FontId::proportional(20.0),
                                theme_colors.text_disabled,
                            );
                            ui.painter().text(
                                egui::pos2(center.x, center.y + 12.0),
                                egui::Align2::CENTER_CENTER,
                                "Drag HDR/EXR here",
                                egui::FontId::proportional(11.0),
                                theme_colors.text_muted,
                            );
                        }
                    }

                    // Handle drop
                    if is_hovered && !response.dragged() {
                        if let Some(dragging_path) = &assets.dragging_asset {
                            let ext = dragging_path.extension()
                                .and_then(|e| e.to_str())
                                .map(|s| s.to_lowercase())
                                .unwrap_or_default();

                            if matches!(ext.as_str(), "hdr" | "exr") {
                                // Copy to project assets folder and get relative path
                                if let Some(rel_path) = copy_sky_to_project_assets(dragging_path, project_path) {
                                    pano.panorama_path = rel_path;
                                    changed = true;
                                } else {
                                    // Fallback to absolute path if copy failed
                                    pano.panorama_path = dragging_path.to_string_lossy().to_string();
                                    changed = true;
                                }
                            }
                        }
                    }

                    // Browse button
                    if ui.add_sized([26.0, drop_zone_height], egui::Button::new(FOLDER_OPEN.to_string())).clicked() {
                        if let Some(texture_path) = rfd::FileDialog::new()
                            .add_filter("HDR Images", &["hdr", "exr"])
                            .set_title("Select Sky Texture")
                            .pick_file()
                        {
                            // Copy to project assets folder and get relative path
                            if let Some(rel_path) = copy_sky_to_project_assets(&texture_path, project_path) {
                                pano.panorama_path = rel_path;
                                changed = true;
                            } else {
                                // Fallback to absolute path if copy failed
                                pano.panorama_path = texture_path.to_string_lossy().to_string();
                                changed = true;
                            }
                        }
                    }
                });

                ui.add_space(4.0);

                // Clear button if a texture is assigned
                if !pano.panorama_path.is_empty() {
                    if ui.button(RichText::new(format!("{} Clear", X_CIRCLE)).color(theme_colors.semantic_error)).clicked() {
                        pano.panorama_path.clear();
                        changed = true;
                    }
                    ui.add_space(4.0);
                }

                row += 1;

                changed |= inline_property(ui, row, "Rotation", |ui| {
                    ui.add(egui::DragValue::new(&mut pano.rotation).speed(1.0).range(0.0..=360.0).suffix("°")).changed()
                });
                row += 1;

                changed |= inline_property(ui, row, "Energy", |ui| {
                    ui.add(egui::DragValue::new(&mut pano.energy).speed(0.1).range(0.0..=10.0)).changed()
                });
            }
        }
    }

    // Fog section
    if section_header(ui, "env_fog", "Fog", false) {
        // Sanitize values
        sanitize_f32(&mut data.fog_start, 0.0, 10000.0, 10.0);
        sanitize_f32(&mut data.fog_end, 0.0, 10000.0, 100.0);

        row = 0;
        changed |= inline_property(ui, row, "Enabled", |ui| {
            ui.checkbox(&mut data.fog_enabled, "").changed()
        });
        row += 1;

        if data.fog_enabled {
            changed |= inline_property(ui, row, "Color", |ui| {
                let mut color = rgb_to_color32(data.fog_color.0, data.fog_color.1, data.fog_color.2);
                let resp = ui.color_edit_button_srgba(&mut color).changed();
                if resp {
                    data.fog_color = color32_to_rgb(color);
                }
                resp
            });
            row += 1;

            changed |= inline_property(ui, row, "Start", |ui| {
                ui.add(egui::DragValue::new(&mut data.fog_start).speed(0.1)).changed()
            });
            row += 1;

            changed |= inline_property(ui, row, "End", |ui| {
                ui.add(egui::DragValue::new(&mut data.fog_end).speed(0.1)).changed()
            });
        }
    }

    // Anti-Aliasing section
    if section_header(ui, "env_aa", "Anti-Aliasing", false) {
        row = 0;
        inline_property(ui, row, "MSAA", |ui| {
            let msaa_options = ["Off", "2x", "4x", "8x"];
            let mut msaa_index = match data.msaa_samples {
                1 => 0, 2 => 1, 4 => 2, 8 => 3, _ => 2,
            };
            egui::ComboBox::from_id_salt("msaa_combo")
                .selected_text(msaa_options[msaa_index])
                .show_ui(ui, |ui| {
                    for (i, option) in msaa_options.iter().enumerate() {
                        if ui.selectable_value(&mut msaa_index, i, *option).changed() {
                            data.msaa_samples = match msaa_index {
                                0 => 1, 1 => 2, 2 => 4, 3 => 8, _ => 4,
                            };
                            changed = true;
                        }
                    }
                });
        });
        row += 1;

        changed |= inline_property(ui, row, "FXAA", |ui| {
            ui.checkbox(&mut data.fxaa_enabled, "").changed()
        });
    }

    // SSAO section
    if section_header(ui, "env_ssao", "Ambient Occlusion", false) {
        // Sanitize values
        sanitize_f32(&mut data.ssao_intensity, 0.0, 3.0, 1.0);
        sanitize_f32(&mut data.ssao_radius, 0.01, 2.0, 0.5);

        row = 0;
        changed |= inline_property(ui, row, "Enabled", |ui| {
            ui.checkbox(&mut data.ssao_enabled, "").changed()
        });
        row += 1;

        if data.ssao_enabled {
            changed |= inline_property(ui, row, "Intensity", |ui| {
                ui.add(egui::DragValue::new(&mut data.ssao_intensity).speed(0.1).range(0.0..=3.0)).changed()
            });
            row += 1;

            changed |= inline_property(ui, row, "Radius", |ui| {
                ui.add(egui::DragValue::new(&mut data.ssao_radius).speed(0.01).range(0.01..=2.0)).changed()
            });
        }
    }

    // SSR section
    if section_header(ui, "env_ssr", "Reflections", false) {
        // Sanitize values
        sanitize_f32(&mut data.ssr_intensity, 0.0, 1.0, 0.5);

        row = 0;
        changed |= inline_property(ui, row, "SSR Enabled", |ui| {
            ui.checkbox(&mut data.ssr_enabled, "").changed()
        });
        row += 1;

        if data.ssr_enabled {
            changed |= inline_property(ui, row, "Intensity", |ui| {
                ui.add(egui::DragValue::new(&mut data.ssr_intensity).speed(0.1).range(0.0..=1.0)).changed()
            });
            row += 1;

            changed |= inline_property(ui, row, "Max Steps", |ui| {
                let mut steps = data.ssr_max_steps as i32;
                let resp = ui.add(egui::DragValue::new(&mut steps).range(16..=256)).changed();
                if resp {
                    data.ssr_max_steps = steps as u32;
                }
                resp
            });
        }
    }

    // Bloom section
    if section_header(ui, "env_bloom", "Bloom", false) {
        // Sanitize values
        sanitize_f32(&mut data.bloom_intensity, 0.0, 1.0, 0.15);
        sanitize_f32(&mut data.bloom_threshold, 0.0, 5.0, 1.0);

        row = 0;
        changed |= inline_property(ui, row, "Enabled", |ui| {
            ui.checkbox(&mut data.bloom_enabled, "").changed()
        });
        row += 1;

        if data.bloom_enabled {
            changed |= inline_property(ui, row, "Intensity", |ui| {
                ui.add(egui::DragValue::new(&mut data.bloom_intensity).speed(0.01).range(0.0..=1.0)).changed()
            });
            row += 1;

            changed |= inline_property(ui, row, "Threshold", |ui| {
                ui.add(egui::DragValue::new(&mut data.bloom_threshold).speed(0.1).range(0.0..=5.0)).changed()
            });
        }
    }

    // Tonemapping section
    if section_header(ui, "env_tonemap", "Tonemapping", false) {
        // Sanitize values
        sanitize_f32(&mut data.ev100, 0.0, 16.0, 9.7);

        row = 0;
        inline_property(ui, row, "Mode", |ui| {
            let tonemap_options = ["None", "Reinhard", "ACES", "AgX", "Filmic"];
            let mut tonemap_index = match data.tonemapping {
                TonemappingMode::None => 0,
                TonemappingMode::Reinhard | TonemappingMode::ReinhardLuminance => 1,
                TonemappingMode::AcesFitted => 2,
                TonemappingMode::AgX => 3,
                TonemappingMode::BlenderFilmic | TonemappingMode::SomewhatBoringDisplayTransform | TonemappingMode::TonyMcMapface => 4,
            };
            egui::ComboBox::from_id_salt("tonemap_combo")
                .selected_text(tonemap_options[tonemap_index])
                .show_ui(ui, |ui| {
                    for (i, option) in tonemap_options.iter().enumerate() {
                        if ui.selectable_value(&mut tonemap_index, i, *option).changed() {
                            data.tonemapping = match tonemap_index {
                                0 => TonemappingMode::None,
                                1 => TonemappingMode::Reinhard,
                                2 => TonemappingMode::AcesFitted,
                                3 => TonemappingMode::AgX,
                                4 => TonemappingMode::BlenderFilmic,
                                _ => TonemappingMode::Reinhard,
                            };
                            changed = true;
                        }
                    }
                });
        });
        row += 1;

        changed |= inline_property(ui, row, "EV100", |ui| {
            ui.add(egui::DragValue::new(&mut data.ev100).speed(0.1).range(0.0..=16.0)).changed()
        });
    }

    // DOF section
    if section_header(ui, "env_dof", "Depth of Field", false) {
        // Sanitize values
        sanitize_f32(&mut data.dof_focal_distance, 0.1, 100.0, 10.0);
        sanitize_f32(&mut data.dof_aperture, 0.001, 0.5, 0.05);

        row = 0;
        changed |= inline_property(ui, row, "Enabled", |ui| {
            ui.checkbox(&mut data.dof_enabled, "").changed()
        });
        row += 1;

        if data.dof_enabled {
            changed |= inline_property(ui, row, "Focal Distance", |ui| {
                ui.add(egui::DragValue::new(&mut data.dof_focal_distance).speed(0.1).range(0.1..=100.0)).changed()
            });
            row += 1;

            changed |= inline_property(ui, row, "Aperture", |ui| {
                ui.add(egui::DragValue::new(&mut data.dof_aperture).speed(0.01).range(0.001..=0.5)).changed()
            });
        }
    }

    // Motion Blur section
    if section_header(ui, "env_motionblur", "Motion Blur", false) {
        // Sanitize values
        sanitize_f32(&mut data.motion_blur_intensity, 0.0, 1.0, 0.5);

        row = 0;
        changed |= inline_property(ui, row, "Enabled", |ui| {
            ui.checkbox(&mut data.motion_blur_enabled, "").changed()
        });

        if data.motion_blur_enabled {
            row += 1;
            changed |= inline_property(ui, row, "Intensity", |ui| {
                ui.add(egui::DragValue::new(&mut data.motion_blur_intensity).speed(0.01).range(0.0..=1.0)).changed()
            });
        }
    }

    changed
}
