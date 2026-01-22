use bevy_egui::egui::{self, RichText, Color32, CornerRadius, Margin};

use crate::core::WorldEnvironmentMarker;
use crate::scene_file::{SkyMode, TonemappingMode};

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

/// Render a section header with background
fn section_header(ui: &mut egui::Ui, title: &str) {
    ui.add_space(8.0);
    egui::Frame::NONE
        .fill(Color32::from_rgb(40, 40, 48))
        .corner_radius(CornerRadius::same(3))
        .inner_margin(Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(RichText::new(title).color(Color32::from_rgb(180, 180, 190)).strong());
        });
    ui.add_space(4.0);
}

/// Render a subsection label
fn subsection_label(ui: &mut egui::Ui, title: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(title).size(11.0).color(Color32::from_rgb(140, 140, 155)));
    ui.add_space(2.0);
}

/// Render the world environment inspector
pub fn render_world_environment_inspector(ui: &mut egui::Ui, world_env: &mut WorldEnvironmentMarker) -> bool {
    let mut changed = false;
    let data = &mut world_env.data;

    // Ambient Light section
    section_header(ui, "Ambient Light");

    ui.horizontal(|ui| {
        ui.label("Color");
        let mut color = rgb_to_color32(data.ambient_color.0, data.ambient_color.1, data.ambient_color.2);
        if ui.color_edit_button_srgba(&mut color).changed() {
            let (r, g, b) = color32_to_rgb(color);
            data.ambient_color = (r, g, b);
            changed = true;
        }
    });

    ui.horizontal(|ui| {
        ui.label("Brightness");
        if ui.add(egui::Slider::new(&mut data.ambient_brightness, 0.0..=1000.0)).changed() {
            changed = true;
        }
    });

    // Sky / Background section
    section_header(ui, "Sky");

    // Sky mode dropdown
    ui.horizontal(|ui| {
        ui.label("Mode");
        let sky_options = ["Color", "Procedural Sky", "Panorama (HDR)"];
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

    ui.add_space(4.0);

    // Show settings based on sky mode
    match data.sky_mode {
        SkyMode::Color => {
            ui.horizontal(|ui| {
                ui.label("Background Color");
                let mut color = rgb_to_color32(data.clear_color.0, data.clear_color.1, data.clear_color.2);
                if ui.color_edit_button_srgba(&mut color).changed() {
                    let (r, g, b) = color32_to_rgb(color);
                    data.clear_color = (r, g, b);
                    changed = true;
                }
            });
        }
        SkyMode::Procedural => {
            let sky = &mut data.procedural_sky;

            // Sky colors
            subsection_label(ui, "Sky Colors");

            ui.horizontal(|ui| {
                ui.label("Top Color");
                let mut color = rgb_to_color32(sky.sky_top_color.0, sky.sky_top_color.1, sky.sky_top_color.2);
                if ui.color_edit_button_srgba(&mut color).changed() {
                    sky.sky_top_color = color32_to_rgb(color);
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Horizon Color");
                let mut color = rgb_to_color32(sky.sky_horizon_color.0, sky.sky_horizon_color.1, sky.sky_horizon_color.2);
                if ui.color_edit_button_srgba(&mut color).changed() {
                    sky.sky_horizon_color = color32_to_rgb(color);
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Sky Curve");
                if ui.add(egui::Slider::new(&mut sky.sky_curve, 0.01..=1.0).logarithmic(true)).changed() {
                    changed = true;
                }
            });

            // Ground colors
            subsection_label(ui, "Ground Colors");

            ui.horizontal(|ui| {
                ui.label("Horizon Color");
                let mut color = rgb_to_color32(sky.ground_horizon_color.0, sky.ground_horizon_color.1, sky.ground_horizon_color.2);
                if ui.color_edit_button_srgba(&mut color).changed() {
                    sky.ground_horizon_color = color32_to_rgb(color);
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Bottom Color");
                let mut color = rgb_to_color32(sky.ground_bottom_color.0, sky.ground_bottom_color.1, sky.ground_bottom_color.2);
                if ui.color_edit_button_srgba(&mut color).changed() {
                    sky.ground_bottom_color = color32_to_rgb(color);
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Ground Curve");
                if ui.add(egui::Slider::new(&mut sky.ground_curve, 0.01..=1.0).logarithmic(true)).changed() {
                    changed = true;
                }
            });

            // Sun settings
            subsection_label(ui, "Sun");

            ui.horizontal(|ui| {
                ui.label("Azimuth");
                if ui.add(egui::Slider::new(&mut sky.sun_angle_azimuth, -180.0..=180.0).suffix("°")).changed() {
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Elevation");
                if ui.add(egui::Slider::new(&mut sky.sun_angle_elevation, -90.0..=90.0).suffix("°")).changed() {
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Sun Color");
                let mut color = rgb_to_color32(sky.sun_color.0, sky.sun_color.1, sky.sun_color.2);
                if ui.color_edit_button_srgba(&mut color).changed() {
                    sky.sun_color = color32_to_rgb(color);
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Sun Energy");
                if ui.add(egui::Slider::new(&mut sky.sun_energy, 0.0..=10.0)).changed() {
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Disk Scale");
                if ui.add(egui::Slider::new(&mut sky.sun_disk_scale, 0.0..=5.0)).changed() {
                    changed = true;
                }
            });
        }
        SkyMode::Panorama => {
            let pano = &mut data.panorama_sky;

            ui.horizontal(|ui| {
                ui.label("HDR File");
                ui.add(egui::TextEdit::singleline(&mut pano.panorama_path).desired_width(150.0));
                if ui.button("Browse...").clicked() {
                    // TODO: Open file dialog for HDR/EXR files
                }
            });

            ui.horizontal(|ui| {
                ui.label("Rotation");
                if ui.add(egui::Slider::new(&mut pano.rotation, 0.0..=360.0).suffix("°")).changed() {
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Energy");
                if ui.add(egui::Slider::new(&mut pano.energy, 0.0..=10.0)).changed() {
                    changed = true;
                }
            });
        }
    }

    // Fog section
    section_header(ui, "Fog");

    if ui.checkbox(&mut data.fog_enabled, "Enabled").changed() {
        changed = true;
    }

    if data.fog_enabled {
        ui.horizontal(|ui| {
            ui.label("Color");
            let mut color = rgb_to_color32(data.fog_color.0, data.fog_color.1, data.fog_color.2);
            if ui.color_edit_button_srgba(&mut color).changed() {
                let (r, g, b) = color32_to_rgb(color);
                data.fog_color = (r, g, b);
                changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Start");
            if ui.add(egui::DragValue::new(&mut data.fog_start).speed(0.1)).changed() {
                changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("End");
            if ui.add(egui::DragValue::new(&mut data.fog_end).speed(0.1)).changed() {
                changed = true;
            }
        });
    }

    // Anti-Aliasing section
    section_header(ui, "Anti-Aliasing");

    ui.horizontal(|ui| {
        ui.label("MSAA");
        let msaa_options = ["Off (1x)", "2x", "4x", "8x"];
        let mut msaa_index = match data.msaa_samples {
            1 => 0,
            2 => 1,
            4 => 2,
            8 => 3,
            _ => 2,
        };
        egui::ComboBox::from_id_salt("msaa_combo")
            .selected_text(msaa_options[msaa_index])
            .show_ui(ui, |ui| {
                for (i, option) in msaa_options.iter().enumerate() {
                    if ui.selectable_value(&mut msaa_index, i, *option).changed() {
                        data.msaa_samples = match msaa_index {
                            0 => 1,
                            1 => 2,
                            2 => 4,
                            3 => 8,
                            _ => 4,
                        };
                        changed = true;
                    }
                }
            });
    });

    if ui.checkbox(&mut data.fxaa_enabled, "FXAA").changed() {
        changed = true;
    }

    // SSAO section
    section_header(ui, "Ambient Occlusion (SSAO)");

    if ui.checkbox(&mut data.ssao_enabled, "Enabled").changed() {
        changed = true;
    }

    if data.ssao_enabled {
        ui.horizontal(|ui| {
            ui.label("Intensity");
            if ui.add(egui::Slider::new(&mut data.ssao_intensity, 0.0..=3.0)).changed() {
                changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Radius");
            if ui.add(egui::Slider::new(&mut data.ssao_radius, 0.01..=2.0)).changed() {
                changed = true;
            }
        });
    }

    // SSR section
    section_header(ui, "Screen Space Reflections");

    if ui.checkbox(&mut data.ssr_enabled, "Enabled").changed() {
        changed = true;
    }

    if data.ssr_enabled {
        ui.horizontal(|ui| {
            ui.label("Intensity");
            if ui.add(egui::Slider::new(&mut data.ssr_intensity, 0.0..=1.0)).changed() {
                changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Max Steps");
            let mut steps = data.ssr_max_steps as i32;
            if ui.add(egui::Slider::new(&mut steps, 16..=256)).changed() {
                data.ssr_max_steps = steps as u32;
                changed = true;
            }
        });
    }

    // Bloom section
    section_header(ui, "Bloom");

    if ui.checkbox(&mut data.bloom_enabled, "Enabled").changed() {
        changed = true;
    }

    if data.bloom_enabled {
        ui.horizontal(|ui| {
            ui.label("Intensity");
            if ui.add(egui::Slider::new(&mut data.bloom_intensity, 0.0..=1.0)).changed() {
                changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Threshold");
            if ui.add(egui::Slider::new(&mut data.bloom_threshold, 0.0..=5.0)).changed() {
                changed = true;
            }
        });
    }

    // Tonemapping section
    section_header(ui, "Tonemapping");

    ui.horizontal(|ui| {
        ui.label("Mode");
        let tonemap_options = [
            "None", "Reinhard", "Reinhard Luminance", "ACES Fitted",
            "AgX", "SomewhatBoringDisplayTransform", "TonyMcMapface", "Blender Filmic"
        ];
        let mut tonemap_index = match data.tonemapping {
            TonemappingMode::None => 0,
            TonemappingMode::Reinhard => 1,
            TonemappingMode::ReinhardLuminance => 2,
            TonemappingMode::AcesFitted => 3,
            TonemappingMode::AgX => 4,
            TonemappingMode::SomewhatBoringDisplayTransform => 5,
            TonemappingMode::TonyMcMapface => 6,
            TonemappingMode::BlenderFilmic => 7,
        };
        egui::ComboBox::from_id_salt("tonemap_combo")
            .selected_text(tonemap_options[tonemap_index])
            .show_ui(ui, |ui| {
                for (i, option) in tonemap_options.iter().enumerate() {
                    if ui.selectable_value(&mut tonemap_index, i, *option).changed() {
                        data.tonemapping = match tonemap_index {
                            0 => TonemappingMode::None,
                            1 => TonemappingMode::Reinhard,
                            2 => TonemappingMode::ReinhardLuminance,
                            3 => TonemappingMode::AcesFitted,
                            4 => TonemappingMode::AgX,
                            5 => TonemappingMode::SomewhatBoringDisplayTransform,
                            6 => TonemappingMode::TonyMcMapface,
                            7 => TonemappingMode::BlenderFilmic,
                            _ => TonemappingMode::Reinhard,
                        };
                        changed = true;
                    }
                }
            });
    });

    ui.horizontal(|ui| {
        ui.label("Exposure");
        if ui.add(egui::Slider::new(&mut data.exposure, 0.1..=5.0)).changed() {
            changed = true;
        }
    });

    // Depth of Field section
    section_header(ui, "Depth of Field");

    if ui.checkbox(&mut data.dof_enabled, "Enabled").changed() {
        changed = true;
    }

    if data.dof_enabled {
        ui.horizontal(|ui| {
            ui.label("Focal Distance");
            if ui.add(egui::Slider::new(&mut data.dof_focal_distance, 0.1..=100.0)).changed() {
                changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Aperture");
            if ui.add(egui::Slider::new(&mut data.dof_aperture, 0.001..=0.5)).changed() {
                changed = true;
            }
        });
    }

    // Motion Blur section
    section_header(ui, "Motion Blur");

    if ui.checkbox(&mut data.motion_blur_enabled, "Enabled").changed() {
        changed = true;
    }

    if data.motion_blur_enabled {
        ui.horizontal(|ui| {
            ui.label("Intensity");
            if ui.add(egui::Slider::new(&mut data.motion_blur_intensity, 0.0..=1.0)).changed() {
                changed = true;
            }
        });
    }

    ui.add_space(4.0);

    changed
}
