use bevy_egui::egui::{self, RichText, Color32};

use crate::core::WorldEnvironmentMarker;
use crate::scene_file::{SkyMode, TonemappingMode};
use crate::ui::property_row;

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

/// Render a section header
fn section_header(ui: &mut egui::Ui, title: &str) {
    ui.add_space(6.0);
    ui.label(RichText::new(title).color(Color32::from_rgb(160, 160, 175)).strong().size(11.0));
    ui.add_space(2.0);
}

/// Render the world environment inspector
pub fn render_world_environment_inspector(ui: &mut egui::Ui, world_env: &mut WorldEnvironmentMarker) -> bool {
    let mut changed = false;
    let data = &mut world_env.data;
    let mut row = 0;

    // Ambient Light section
    section_header(ui, "AMBIENT LIGHT");

    property_row(ui, row, |ui| {
        ui.horizontal(|ui| {
            ui.label("Color");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut color = rgb_to_color32(data.ambient_color.0, data.ambient_color.1, data.ambient_color.2);
                if ui.color_edit_button_srgba(&mut color).changed() {
                    let (r, g, b) = color32_to_rgb(color);
                    data.ambient_color = (r, g, b);
                    changed = true;
                }
            });
        });
    });
    row += 1;

    property_row(ui, row, |ui| {
        ui.horizontal(|ui| {
            ui.label("Brightness");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::DragValue::new(&mut data.ambient_brightness).speed(10.0).range(0.0..=1000.0)).changed() {
                    changed = true;
                }
            });
        });
    });

    // Sky section
    section_header(ui, "SKY");
    row = 0;

    property_row(ui, row, |ui| {
        ui.horizontal(|ui| {
            ui.label("Mode");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
        });
    });
    row += 1;

    match data.sky_mode {
        SkyMode::Color => {
            property_row(ui, row, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Background");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut color = rgb_to_color32(data.clear_color.0, data.clear_color.1, data.clear_color.2);
                        if ui.color_edit_button_srgba(&mut color).changed() {
                            let (r, g, b) = color32_to_rgb(color);
                            data.clear_color = (r, g, b);
                            changed = true;
                        }
                    });
                });
            });
        }
        SkyMode::Procedural => {
            let sky = &mut data.procedural_sky;

            property_row(ui, row, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Top Color");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut color = rgb_to_color32(sky.sky_top_color.0, sky.sky_top_color.1, sky.sky_top_color.2);
                        if ui.color_edit_button_srgba(&mut color).changed() {
                            sky.sky_top_color = color32_to_rgb(color);
                            changed = true;
                        }
                    });
                });
            });
            row += 1;

            property_row(ui, row, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Horizon Color");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut color = rgb_to_color32(sky.sky_horizon_color.0, sky.sky_horizon_color.1, sky.sky_horizon_color.2);
                        if ui.color_edit_button_srgba(&mut color).changed() {
                            sky.sky_horizon_color = color32_to_rgb(color);
                            changed = true;
                        }
                    });
                });
            });
            row += 1;

            property_row(ui, row, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Sky Curve");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::DragValue::new(&mut sky.sky_curve).speed(0.01).range(0.01..=1.0)).changed() {
                            changed = true;
                        }
                    });
                });
            });
            row += 1;

            property_row(ui, row, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Ground Horizon");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut color = rgb_to_color32(sky.ground_horizon_color.0, sky.ground_horizon_color.1, sky.ground_horizon_color.2);
                        if ui.color_edit_button_srgba(&mut color).changed() {
                            sky.ground_horizon_color = color32_to_rgb(color);
                            changed = true;
                        }
                    });
                });
            });
            row += 1;

            property_row(ui, row, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Ground Bottom");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut color = rgb_to_color32(sky.ground_bottom_color.0, sky.ground_bottom_color.1, sky.ground_bottom_color.2);
                        if ui.color_edit_button_srgba(&mut color).changed() {
                            sky.ground_bottom_color = color32_to_rgb(color);
                            changed = true;
                        }
                    });
                });
            });
            row += 1;

            property_row(ui, row, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Ground Curve");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::DragValue::new(&mut sky.ground_curve).speed(0.01).range(0.01..=1.0)).changed() {
                            changed = true;
                        }
                    });
                });
            });
            row += 1;

            property_row(ui, row, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Sun Azimuth");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::DragValue::new(&mut sky.sun_angle_azimuth).speed(1.0).range(-180.0..=180.0).suffix("°")).changed() {
                            changed = true;
                        }
                    });
                });
            });
            row += 1;

            property_row(ui, row, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Sun Elevation");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::DragValue::new(&mut sky.sun_angle_elevation).speed(1.0).range(-90.0..=90.0).suffix("°")).changed() {
                            changed = true;
                        }
                    });
                });
            });
            row += 1;

            property_row(ui, row, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Sun Color");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut color = rgb_to_color32(sky.sun_color.0, sky.sun_color.1, sky.sun_color.2);
                        if ui.color_edit_button_srgba(&mut color).changed() {
                            sky.sun_color = color32_to_rgb(color);
                            changed = true;
                        }
                    });
                });
            });
            row += 1;

            property_row(ui, row, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Sun Energy");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::DragValue::new(&mut sky.sun_energy).speed(0.1).range(0.0..=10.0)).changed() {
                            changed = true;
                        }
                    });
                });
            });
            row += 1;

            property_row(ui, row, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Disk Scale");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::DragValue::new(&mut sky.sun_disk_scale).speed(0.1).range(0.0..=5.0)).changed() {
                            changed = true;
                        }
                    });
                });
            });
        }
        SkyMode::Panorama => {
            let pano = &mut data.panorama_sky;

            property_row(ui, row, |ui| {
                ui.horizontal(|ui| {
                    ui.label("HDR File");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Browse").clicked() {
                            // TODO: file dialog
                        }
                    });
                });
            });
            row += 1;

            property_row(ui, row, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Rotation");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::DragValue::new(&mut pano.rotation).speed(1.0).range(0.0..=360.0).suffix("°")).changed() {
                            changed = true;
                        }
                    });
                });
            });
            row += 1;

            property_row(ui, row, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Energy");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::DragValue::new(&mut pano.energy).speed(0.1).range(0.0..=10.0)).changed() {
                            changed = true;
                        }
                    });
                });
            });
        }
    }

    // Fog section
    section_header(ui, "FOG");
    row = 0;

    property_row(ui, row, |ui| {
        ui.horizontal(|ui| {
            ui.label("Enabled");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut data.fog_enabled, "").changed() {
                    changed = true;
                }
            });
        });
    });
    row += 1;

    if data.fog_enabled {
        property_row(ui, row, |ui| {
            ui.horizontal(|ui| {
                ui.label("Color");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut color = rgb_to_color32(data.fog_color.0, data.fog_color.1, data.fog_color.2);
                    if ui.color_edit_button_srgba(&mut color).changed() {
                        let (r, g, b) = color32_to_rgb(color);
                        data.fog_color = (r, g, b);
                        changed = true;
                    }
                });
            });
        });
        row += 1;

        property_row(ui, row, |ui| {
            ui.horizontal(|ui| {
                ui.label("Start");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::DragValue::new(&mut data.fog_start).speed(0.1)).changed() {
                        changed = true;
                    }
                });
            });
        });
        row += 1;

        property_row(ui, row, |ui| {
            ui.horizontal(|ui| {
                ui.label("End");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::DragValue::new(&mut data.fog_end).speed(0.1)).changed() {
                        changed = true;
                    }
                });
            });
        });
    }

    // Anti-Aliasing section
    section_header(ui, "ANTI-ALIASING");
    row = 0;

    property_row(ui, row, |ui| {
        ui.horizontal(|ui| {
            ui.label("MSAA");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
        });
    });
    row += 1;

    property_row(ui, row, |ui| {
        ui.horizontal(|ui| {
            ui.label("FXAA");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut data.fxaa_enabled, "").changed() {
                    changed = true;
                }
            });
        });
    });

    // SSAO section
    section_header(ui, "AMBIENT OCCLUSION");
    row = 0;

    property_row(ui, row, |ui| {
        ui.horizontal(|ui| {
            ui.label("Enabled");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut data.ssao_enabled, "").changed() {
                    changed = true;
                }
            });
        });
    });
    row += 1;

    if data.ssao_enabled {
        property_row(ui, row, |ui| {
            ui.horizontal(|ui| {
                ui.label("Intensity");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::DragValue::new(&mut data.ssao_intensity).speed(0.1).range(0.0..=3.0)).changed() {
                        changed = true;
                    }
                });
            });
        });
        row += 1;

        property_row(ui, row, |ui| {
            ui.horizontal(|ui| {
                ui.label("Radius");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::DragValue::new(&mut data.ssao_radius).speed(0.01).range(0.01..=2.0)).changed() {
                        changed = true;
                    }
                });
            });
        });
    }

    // SSR section
    section_header(ui, "REFLECTIONS");
    row = 0;

    property_row(ui, row, |ui| {
        ui.horizontal(|ui| {
            ui.label("SSR Enabled");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut data.ssr_enabled, "").changed() {
                    changed = true;
                }
            });
        });
    });
    row += 1;

    if data.ssr_enabled {
        property_row(ui, row, |ui| {
            ui.horizontal(|ui| {
                ui.label("Intensity");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::DragValue::new(&mut data.ssr_intensity).speed(0.1).range(0.0..=1.0)).changed() {
                        changed = true;
                    }
                });
            });
        });
        row += 1;

        property_row(ui, row, |ui| {
            ui.horizontal(|ui| {
                ui.label("Max Steps");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut steps = data.ssr_max_steps as i32;
                    if ui.add(egui::DragValue::new(&mut steps).range(16..=256)).changed() {
                        data.ssr_max_steps = steps as u32;
                        changed = true;
                    }
                });
            });
        });
    }

    // Bloom section
    section_header(ui, "BLOOM");
    row = 0;

    property_row(ui, row, |ui| {
        ui.horizontal(|ui| {
            ui.label("Enabled");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut data.bloom_enabled, "").changed() {
                    changed = true;
                }
            });
        });
    });
    row += 1;

    if data.bloom_enabled {
        property_row(ui, row, |ui| {
            ui.horizontal(|ui| {
                ui.label("Intensity");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::DragValue::new(&mut data.bloom_intensity).speed(0.01).range(0.0..=1.0)).changed() {
                        changed = true;
                    }
                });
            });
        });
        row += 1;

        property_row(ui, row, |ui| {
            ui.horizontal(|ui| {
                ui.label("Threshold");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::DragValue::new(&mut data.bloom_threshold).speed(0.1).range(0.0..=5.0)).changed() {
                        changed = true;
                    }
                });
            });
        });
    }

    // Tonemapping section
    section_header(ui, "TONEMAPPING");
    row = 0;

    property_row(ui, row, |ui| {
        ui.horizontal(|ui| {
            ui.label("Mode");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
        });
    });
    row += 1;

    property_row(ui, row, |ui| {
        ui.horizontal(|ui| {
            ui.label("Exposure");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::DragValue::new(&mut data.exposure).speed(0.1).range(0.1..=5.0)).changed() {
                    changed = true;
                }
            });
        });
    });

    // DOF section
    section_header(ui, "DEPTH OF FIELD");
    row = 0;

    property_row(ui, row, |ui| {
        ui.horizontal(|ui| {
            ui.label("Enabled");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut data.dof_enabled, "").changed() {
                    changed = true;
                }
            });
        });
    });
    row += 1;

    if data.dof_enabled {
        property_row(ui, row, |ui| {
            ui.horizontal(|ui| {
                ui.label("Focal Distance");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::DragValue::new(&mut data.dof_focal_distance).speed(0.1).range(0.1..=100.0)).changed() {
                        changed = true;
                    }
                });
            });
        });
        row += 1;

        property_row(ui, row, |ui| {
            ui.horizontal(|ui| {
                ui.label("Aperture");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::DragValue::new(&mut data.dof_aperture).speed(0.01).range(0.001..=0.5)).changed() {
                        changed = true;
                    }
                });
            });
        });
    }

    // Motion Blur section
    section_header(ui, "MOTION BLUR");
    row = 0;

    property_row(ui, row, |ui| {
        ui.horizontal(|ui| {
            ui.label("Enabled");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.checkbox(&mut data.motion_blur_enabled, "").changed() {
                    changed = true;
                }
            });
        });
    });

    if data.motion_blur_enabled {
        row += 1;
        property_row(ui, row, |ui| {
            ui.horizontal(|ui| {
                ui.label("Intensity");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::DragValue::new(&mut data.motion_blur_intensity).speed(0.01).range(0.0..=1.0)).changed() {
                        changed = true;
                    }
                });
            });
        });
    }

    changed
}
