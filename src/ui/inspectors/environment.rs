//! Inspector widget for world environment

use bevy_egui::egui::{self, RichText, Color32, Sense};

use crate::core::WorldEnvironmentMarker;
use crate::shared::{SkyMode, TonemappingMode};
use crate::ui::inline_property;

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

    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 20.0), Sense::click());

    if response.clicked() {
        state.toggle(ui);
    }

    let bg_color = if response.hovered() {
        Color32::from_rgb(50, 53, 60)
    } else {
        Color32::from_rgb(45, 48, 55)
    };

    ui.painter().rect_filled(rect, 0.0, bg_color);
    ui.painter().text(
        rect.left_center() + egui::vec2(8.0, 0.0),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(12.0),
        Color32::from_rgb(180, 180, 190),
    );

    state.store(ui.ctx());
    state.is_open()
}

/// Render the world environment inspector
pub fn render_world_environment_inspector(ui: &mut egui::Ui, world_env: &mut WorldEnvironmentMarker) -> bool {
    let mut changed = false;
    let data = &mut world_env.data;
    let mut row;

    // Ambient Light section
    if section_header(ui, "env_ambient", "Ambient Light", true) {
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

                inline_property(ui, row, "HDR File", |ui| {
                    ui.button("Browse").clicked()
                    // TODO: file dialog
                });
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

        changed |= inline_property(ui, row, "Exposure", |ui| {
            ui.add(egui::DragValue::new(&mut data.exposure).speed(0.1).range(0.1..=5.0)).changed()
        });
    }

    // DOF section
    if section_header(ui, "env_dof", "Depth of Field", false) {
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
