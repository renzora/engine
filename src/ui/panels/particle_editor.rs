//! Particle Editor Panel
//!
//! Provides a full-featured editor for bevy_hanabi particle effects.
//! Supports both asset-based and inline effect definitions.

use std::path::PathBuf;
use bevy_egui::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use egui_phosphor::regular::{
    PLUS, MINUS, FLOPPY_DISK, FOLDER_OPEN,
    SPARKLE, TIMER, SHAPES, ARROWS_OUT, WIND, RESIZE, PALETTE, CUBE, GEAR, CODE,
    PROHIBIT, ATOM,
};

use crate::particles::{
    HanabiEffectDefinition, HanabiEmitShape, ShapeDimension, SpawnMode, VelocityMode,
    SimulationSpace, SimulationCondition, GradientStop,
    EffectVariable, ParticleEditorState,
    ParticleAlphaMode, ParticleOrientMode, ParticleColorBlendMode,
    MotionIntegrationMode, KillZone, ConformToSphere, FlipbookSettings,
};
use crate::core::{SceneManagerState, TabKind};
use crate::theming::Theme;

use super::inspector::{render_category, inline_property_themed};

/// Load a particle effect definition from a file
fn load_effect_from_file(path: &PathBuf) -> Option<HanabiEffectDefinition> {
    bevy::log::info!("[ParticleEditor] Loading effect from: {:?}", path);
    match std::fs::read_to_string(path) {
        Ok(contents) => {
            bevy::log::info!("[ParticleEditor] File read successfully, {} bytes", contents.len());
            match ron::from_str::<HanabiEffectDefinition>(&contents) {
                Ok(effect) => {
                    bevy::log::info!("[ParticleEditor] Parsed effect '{}': capacity={}, spawn_rate={}, size_start={}",
                        effect.name, effect.capacity, effect.spawn_rate, effect.size_start);
                    Some(effect)
                }
                Err(e) => {
                    bevy::log::error!("[ParticleEditor] Failed to parse particle effect {:?}: {}", path, e);
                    let mut effect = HanabiEffectDefinition::default();
                    effect.name = path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Untitled")
                        .to_string();
                    Some(effect)
                }
            }
        }
        Err(e) => {
            bevy::log::error!("[ParticleEditor] Failed to read particle effect {:?}: {}", path, e);
            let mut effect = HanabiEffectDefinition::default();
            effect.name = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string();
            Some(effect)
        }
    }
}

/// Save a particle effect definition to a file
fn save_effect_to_file(path: &PathBuf, effect: &HanabiEffectDefinition) -> bool {
    let pretty = ron::ser::PrettyConfig::new()
        .depth_limit(4)
        .separate_tuple_members(true);

    match ron::ser::to_string_pretty(effect, pretty) {
        Ok(contents) => {
            match std::fs::write(path, contents) {
                Ok(_) => {
                    bevy::log::info!("Saved particle effect to {:?}", path);
                    true
                }
                Err(e) => {
                    bevy::log::error!("Failed to write particle effect {:?}: {}", path, e);
                    false
                }
            }
        }
        Err(e) => {
            bevy::log::error!("Failed to serialize particle effect: {}", e);
            false
        }
    }
}

/// Sync the particle editor state with the active document tab
fn sync_with_active_document(state: &mut ParticleEditorState, scene_state: &SceneManagerState) {
    let active_particle_path = match &scene_state.active_document {
        Some(TabKind::ParticleFX(idx)) => {
            scene_state.open_particles.get(*idx).map(|p| p.path.clone())
        }
        _ => None,
    };

    let current_path = state.current_file_path.as_ref().map(|s| PathBuf::from(s));

    if active_particle_path != current_path {
        bevy::log::info!("[ParticleEditor] Document changed: {:?} -> {:?}", current_path, active_particle_path);
        if let Some(path) = active_particle_path {
            if let Some(effect) = load_effect_from_file(&path) {
                bevy::log::info!("[ParticleEditor] Loaded effect into state: '{}'", effect.name);
                state.current_effect = Some(effect);
                state.current_file_path = Some(path.to_string_lossy().to_string());
                state.is_modified = false;
            }
        } else if current_path.is_some() {
            bevy::log::info!("[ParticleEditor] Keeping existing effect state");
        }
    }
}

/// Render the particle editor panel content
pub fn render_particle_editor_content(
    ui: &mut egui::Ui,
    state: &mut ParticleEditorState,
    scene_state: &mut SceneManagerState,
    theme: &Theme,
) {
    sync_with_active_document(state, scene_state);
    let bg_color = theme.surfaces.panel.to_color32();

    let available = ui.available_rect_before_wrap();
    ui.painter().rect_filled(available, 0.0, bg_color);

    if state.current_effect.is_none() {
        render_welcome_screen(ui, state, scene_state, theme);
        return;
    }

    let mut save_requested = false;

    egui::ScrollArea::vertical()
        .id_salt("particle_editor_scroll")
        .show(ui, |ui| {
            if let Some(mut effect) = state.current_effect.take() {
                let (modified, save) = render_effect_editor(ui, &mut effect, state.current_file_path.as_ref(), theme);
                if modified {
                    state.is_modified = true;
                }
                if save {
                    save_requested = true;
                }
                state.current_effect = Some(effect);
            }
        });

    if save_requested {
        if let (Some(effect), Some(path_str)) = (&state.current_effect, &state.current_file_path) {
            let path = PathBuf::from(path_str);
            if save_effect_to_file(&path, effect) {
                state.is_modified = false;
                if let Some(TabKind::ParticleFX(idx)) = &scene_state.active_document {
                    if let Some(doc) = scene_state.open_particles.get_mut(*idx) {
                        doc.is_modified = false;
                    }
                }
            }
        }
    }

    if state.is_modified {
        if let Some(TabKind::ParticleFX(idx)) = &scene_state.active_document {
            if let Some(doc) = scene_state.open_particles.get_mut(*idx) {
                doc.is_modified = true;
            }
        }
    }
}

fn render_welcome_screen(ui: &mut egui::Ui, state: &mut ParticleEditorState, scene_state: &mut SceneManagerState, theme: &Theme) {
    let available = ui.available_rect_before_wrap();
    let text_muted = theme.text.muted.to_color32();
    let accent = theme.semantic.accent.to_color32();

    if let Some(TabKind::ParticleFX(idx)) = &scene_state.active_document {
        if let Some(doc) = scene_state.open_particles.get(*idx) {
            if let Some(effect) = load_effect_from_file(&doc.path) {
                state.current_effect = Some(effect);
                state.current_file_path = Some(doc.path.to_string_lossy().to_string());
                state.is_modified = doc.is_modified;
                return;
            }
        }
    }

    ui.scope_builder(egui::UiBuilder::new().max_rect(available), |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(available.height() * 0.3);

            ui.label(RichText::new("Particle Editor")
                .size(20.0)
                .color(text_muted));

            ui.add_space(16.0);

            ui.label(RichText::new("Create or load a particle effect to begin")
                .size(12.0)
                .color(text_muted));

            ui.add_space(24.0);

            ui.horizontal(|ui| {
                ui.add_space(available.width() * 0.3);

                if ui.add(egui::Button::new(RichText::new(format!("{} New Effect", PLUS))
                    .color(accent))).clicked()
                {
                    state.current_effect = Some(HanabiEffectDefinition::default());
                    state.is_modified = true;
                    state.current_file_path = None;
                }

                ui.add_space(8.0);

                if ui.add(egui::Button::new(RichText::new(format!("{} Open", FOLDER_OPEN))
                    .color(text_muted))).clicked()
                {
                    // TODO: Open file dialog
                }
            });
        });
    });
}

/// Render the effect editor, returns (modified, save_requested)
fn render_effect_editor(
    ui: &mut egui::Ui,
    effect: &mut HanabiEffectDefinition,
    file_path: Option<&String>,
    theme: &Theme,
) -> (bool, bool) {
    let mut modified = false;
    let mut save_requested = false;
    let text_color = theme.text.primary.to_color32();

    // Header with save button
    ui.horizontal(|ui| {
        ui.label(RichText::new("Effect Settings").strong().color(text_color));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let save_enabled = file_path.is_some();
            if ui.add_enabled(save_enabled, egui::Button::new(format!("{} Save", FLOPPY_DISK)).small()).clicked() {
                save_requested = true;
            }
        });
    });

    ui.add_space(4.0);

    // Basic settings
    let mut row = 0;
    modified |= inline_property_themed(ui, row, "Name", theme, |ui| {
        ui.text_edit_singleline(&mut effect.name).changed()
    });
    row += 1;
    modified |= inline_property_themed(ui, row, "Capacity", theme, |ui| {
        ui.add(egui::DragValue::new(&mut effect.capacity).range(10..=100000)).changed()
    });

    ui.add_space(6.0);

    // Sections
    modified |= render_spawning_section(ui, effect, theme);
    modified |= render_lifetime_section(ui, effect, theme);
    modified |= render_shape_section(ui, effect, theme);
    modified |= render_velocity_section(ui, effect, theme);
    modified |= render_forces_section(ui, effect, theme);
    modified |= render_size_section(ui, effect, theme);
    modified |= render_color_section(ui, effect, theme);
    modified |= render_rendering_section(ui, effect, theme);
    modified |= render_simulation_section(ui, effect, theme);
    modified |= render_kill_zones_section(ui, effect, theme);
    modified |= render_conform_section(ui, effect, theme);
    modified |= render_variables_section(ui, effect, theme);

    (modified, save_requested)
}

fn render_spawning_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    render_category(ui, SPARKLE, "Spawning", "effects", theme, "particle_spawning", true, |ui| {
        let mut row = 0;
        modified |= inline_property_themed(ui, row, "Mode", theme, |ui| {
            let mut changed = false;
            egui::ComboBox::from_id_salt("spawn_mode_editor")
                .selected_text(match effect.spawn_mode {
                    SpawnMode::Rate => "Continuous",
                    SpawnMode::Burst => "Single Burst",
                    SpawnMode::BurstRate => "Repeated Bursts",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(effect.spawn_mode == SpawnMode::Rate, "Continuous").clicked() {
                        effect.spawn_mode = SpawnMode::Rate; changed = true;
                    }
                    if ui.selectable_label(effect.spawn_mode == SpawnMode::Burst, "Single Burst").clicked() {
                        effect.spawn_mode = SpawnMode::Burst; changed = true;
                    }
                    if ui.selectable_label(effect.spawn_mode == SpawnMode::BurstRate, "Repeated Bursts").clicked() {
                        effect.spawn_mode = SpawnMode::BurstRate; changed = true;
                    }
                });
            changed
        });
        row += 1;

        match effect.spawn_mode {
            SpawnMode::Rate => {
                modified |= inline_property_themed(ui, row, "Rate/sec", theme, |ui| {
                    ui.add(egui::DragValue::new(&mut effect.spawn_rate).speed(1.0).range(0.1..=10000.0)).changed()
                });
            }
            SpawnMode::Burst => {
                modified |= inline_property_themed(ui, row, "Count", theme, |ui| {
                    ui.add(egui::DragValue::new(&mut effect.spawn_count).range(1..=10000)).changed()
                });
            }
            SpawnMode::BurstRate => {
                modified |= inline_property_themed(ui, row, "Count", theme, |ui| {
                    ui.add(egui::DragValue::new(&mut effect.spawn_count).range(1..=10000)).changed()
                });
                row += 1;
                modified |= inline_property_themed(ui, row, "Bursts/sec", theme, |ui| {
                    ui.add(egui::DragValue::new(&mut effect.spawn_rate).speed(0.1).range(0.1..=100.0)).changed()
                });
            }
        }
        row += 1;

        modified |= inline_property_themed(ui, row, "Duration", theme, |ui| {
            ui.add(egui::DragValue::new(&mut effect.spawn_duration).speed(0.1).range(0.0..=600.0).suffix("s")).changed()
        });
        row += 1;
        modified |= inline_property_themed(ui, row, "Cycles", theme, |ui| {
            ui.add(egui::DragValue::new(&mut effect.spawn_cycle_count).range(0..=1000)).changed()
        });
        row += 1;
        modified |= inline_property_themed(ui, row, "Starts Active", theme, |ui| {
            ui.checkbox(&mut effect.spawn_starts_active, "").changed()
        });
    });
    modified
}

fn render_lifetime_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    render_category(ui, TIMER, "Lifetime", "effects", theme, "particle_lifetime", true, |ui| {
        modified |= inline_property_themed(ui, 0, "Min", theme, |ui| {
            let changed = ui.add(egui::DragValue::new(&mut effect.lifetime_min).speed(0.1).range(0.01..=60.0).suffix("s")).changed();
            if changed && effect.lifetime_min > effect.lifetime_max {
                effect.lifetime_max = effect.lifetime_min;
            }
            changed
        });
        modified |= inline_property_themed(ui, 1, "Max", theme, |ui| {
            let changed = ui.add(egui::DragValue::new(&mut effect.lifetime_max).speed(0.1).range(0.01..=60.0).suffix("s")).changed();
            if changed && effect.lifetime_max < effect.lifetime_min {
                effect.lifetime_min = effect.lifetime_max;
            }
            changed
        });
    });
    modified
}

fn render_shape_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    render_category(ui, SHAPES, "Emission Shape", "effects", theme, "particle_shape", true, |ui| {
        let shape_name = match &effect.emit_shape {
            HanabiEmitShape::Point => "Point",
            HanabiEmitShape::Circle { .. } => "Circle",
            HanabiEmitShape::Sphere { .. } => "Sphere",
            HanabiEmitShape::Cone { .. } => "Cone",
            HanabiEmitShape::Rect { .. } => "Rectangle",
            HanabiEmitShape::Box { .. } => "Box",
        };

        let mut row = 0;
        modified |= inline_property_themed(ui, row, "Shape", theme, |ui| {
            let mut changed = false;
            egui::ComboBox::from_id_salt("emit_shape_editor")
                .selected_text(shape_name)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(matches!(&effect.emit_shape, HanabiEmitShape::Point), "Point").clicked() {
                        effect.emit_shape = HanabiEmitShape::Point; changed = true;
                    }
                    if ui.selectable_label(matches!(&effect.emit_shape, HanabiEmitShape::Circle { .. }), "Circle").clicked() {
                        effect.emit_shape = HanabiEmitShape::Circle { radius: 1.0, dimension: ShapeDimension::Volume }; changed = true;
                    }
                    if ui.selectable_label(matches!(&effect.emit_shape, HanabiEmitShape::Sphere { .. }), "Sphere").clicked() {
                        effect.emit_shape = HanabiEmitShape::Sphere { radius: 1.0, dimension: ShapeDimension::Volume }; changed = true;
                    }
                    if ui.selectable_label(matches!(&effect.emit_shape, HanabiEmitShape::Cone { .. }), "Cone").clicked() {
                        effect.emit_shape = HanabiEmitShape::Cone { base_radius: 0.5, top_radius: 0.0, height: 1.0, dimension: ShapeDimension::Volume }; changed = true;
                    }
                    if ui.selectable_label(matches!(&effect.emit_shape, HanabiEmitShape::Rect { .. }), "Rectangle").clicked() {
                        effect.emit_shape = HanabiEmitShape::Rect { half_extents: [1.0, 1.0], dimension: ShapeDimension::Volume }; changed = true;
                    }
                    if ui.selectable_label(matches!(&effect.emit_shape, HanabiEmitShape::Box { .. }), "Box").clicked() {
                        effect.emit_shape = HanabiEmitShape::Box { half_extents: [1.0, 1.0, 1.0] }; changed = true;
                    }
                });
            changed
        });
        row += 1;

        match &mut effect.emit_shape {
            HanabiEmitShape::Point => {}
            HanabiEmitShape::Circle { radius, dimension } |
            HanabiEmitShape::Sphere { radius, dimension } => {
                modified |= inline_property_themed(ui, row, "Radius", theme, |ui| {
                    ui.add(egui::DragValue::new(radius).speed(0.1).range(0.001..=100.0)).changed()
                });
                row += 1;
                modified |= inline_property_themed(ui, row, "Emit from", theme, |ui| {
                    let mut changed = false;
                    if ui.radio(*dimension == ShapeDimension::Volume, "Volume").clicked() {
                        *dimension = ShapeDimension::Volume; changed = true;
                    }
                    if ui.radio(*dimension == ShapeDimension::Surface, "Surface").clicked() {
                        *dimension = ShapeDimension::Surface; changed = true;
                    }
                    changed
                });
            }
            HanabiEmitShape::Cone { base_radius, top_radius, height, dimension } => {
                modified |= inline_property_themed(ui, row, "Base Radius", theme, |ui| {
                    ui.add(egui::DragValue::new(base_radius).speed(0.1).range(0.0..=100.0)).changed()
                });
                row += 1;
                modified |= inline_property_themed(ui, row, "Top Radius", theme, |ui| {
                    ui.add(egui::DragValue::new(top_radius).speed(0.1).range(0.0..=100.0)).changed()
                });
                row += 1;
                modified |= inline_property_themed(ui, row, "Height", theme, |ui| {
                    ui.add(egui::DragValue::new(height).speed(0.1).range(0.001..=100.0)).changed()
                });
                row += 1;
                modified |= inline_property_themed(ui, row, "Emit from", theme, |ui| {
                    let mut changed = false;
                    if ui.radio(*dimension == ShapeDimension::Volume, "Volume").clicked() {
                        *dimension = ShapeDimension::Volume; changed = true;
                    }
                    if ui.radio(*dimension == ShapeDimension::Surface, "Surface").clicked() {
                        *dimension = ShapeDimension::Surface; changed = true;
                    }
                    changed
                });
            }
            HanabiEmitShape::Rect { half_extents, dimension } => {
                modified |= inline_property_themed(ui, row, "Extents X", theme, |ui| {
                    ui.add(egui::DragValue::new(&mut half_extents[0]).speed(0.1).range(0.001..=100.0)).changed()
                });
                row += 1;
                modified |= inline_property_themed(ui, row, "Extents Y", theme, |ui| {
                    ui.add(egui::DragValue::new(&mut half_extents[1]).speed(0.1).range(0.001..=100.0)).changed()
                });
                row += 1;
                modified |= inline_property_themed(ui, row, "Emit from", theme, |ui| {
                    let mut changed = false;
                    if ui.radio(*dimension == ShapeDimension::Volume, "Volume").clicked() {
                        *dimension = ShapeDimension::Volume; changed = true;
                    }
                    if ui.radio(*dimension == ShapeDimension::Surface, "Surface").clicked() {
                        *dimension = ShapeDimension::Surface; changed = true;
                    }
                    changed
                });
            }
            HanabiEmitShape::Box { half_extents } => {
                modified |= inline_property_themed(ui, row, "Extents X", theme, |ui| {
                    ui.add(egui::DragValue::new(&mut half_extents[0]).speed(0.1).range(0.001..=100.0)).changed()
                });
                row += 1;
                modified |= inline_property_themed(ui, row, "Extents Y", theme, |ui| {
                    ui.add(egui::DragValue::new(&mut half_extents[1]).speed(0.1).range(0.001..=100.0)).changed()
                });
                row += 1;
                modified |= inline_property_themed(ui, row, "Extents Z", theme, |ui| {
                    ui.add(egui::DragValue::new(&mut half_extents[2]).speed(0.1).range(0.001..=100.0)).changed()
                });
            }
        }
    });
    modified
}

fn render_velocity_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    render_category(ui, ARROWS_OUT, "Velocity", "effects", theme, "particle_velocity", true, |ui| {
        let mut row = 0;
        modified |= inline_property_themed(ui, row, "Mode", theme, |ui| {
            let mut changed = false;
            egui::ComboBox::from_id_salt("velocity_mode_editor")
                .selected_text(match effect.velocity_mode {
                    VelocityMode::Directional => "Directional",
                    VelocityMode::Radial => "Radial",
                    VelocityMode::Tangent => "Tangent",
                    VelocityMode::Random => "Random",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(effect.velocity_mode == VelocityMode::Directional, "Directional").clicked() {
                        effect.velocity_mode = VelocityMode::Directional; changed = true;
                    }
                    if ui.selectable_label(effect.velocity_mode == VelocityMode::Radial, "Radial").clicked() {
                        effect.velocity_mode = VelocityMode::Radial; changed = true;
                    }
                    if ui.selectable_label(effect.velocity_mode == VelocityMode::Tangent, "Tangent").clicked() {
                        effect.velocity_mode = VelocityMode::Tangent; changed = true;
                    }
                    if ui.selectable_label(effect.velocity_mode == VelocityMode::Random, "Random").clicked() {
                        effect.velocity_mode = VelocityMode::Random; changed = true;
                    }
                });
            changed
        });
        row += 1;

        modified |= inline_property_themed(ui, row, "Speed", theme, |ui| {
            ui.add(egui::DragValue::new(&mut effect.velocity_magnitude).speed(0.1).range(0.0..=100.0)).changed()
        });
        row += 1;

        modified |= inline_property_themed(ui, row, "Speed Min", theme, |ui| {
            ui.add(egui::DragValue::new(&mut effect.velocity_speed_min).speed(0.1).range(0.0..=100.0)).changed()
        });
        row += 1;
        modified |= inline_property_themed(ui, row, "Speed Max", theme, |ui| {
            ui.add(egui::DragValue::new(&mut effect.velocity_speed_max).speed(0.1).range(0.0..=100.0)).changed()
        });
        row += 1;

        if effect.velocity_mode == VelocityMode::Directional {
            modified |= inline_property_themed(ui, row, "Spread", theme, |ui| {
                ui.add(egui::DragValue::new(&mut effect.velocity_spread).speed(0.05).range(0.0..=std::f32::consts::PI)).changed()
            });
            row += 1;
            modified |= inline_property_themed(ui, row, "Direction", theme, |ui| {
                let mut changed = false;
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.label(RichText::new("X").size(10.0));
                changed |= ui.add(egui::DragValue::new(&mut effect.velocity_direction[0]).speed(0.1)).changed();
                ui.label(RichText::new("Y").size(10.0));
                changed |= ui.add(egui::DragValue::new(&mut effect.velocity_direction[1]).speed(0.1)).changed();
                ui.label(RichText::new("Z").size(10.0));
                changed |= ui.add(egui::DragValue::new(&mut effect.velocity_direction[2]).speed(0.1)).changed();
                changed
            });
        }

        if effect.velocity_mode == VelocityMode::Tangent {
            modified |= inline_property_themed(ui, row, "Axis", theme, |ui| {
                let mut changed = false;
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.label(RichText::new("X").size(10.0));
                changed |= ui.add(egui::DragValue::new(&mut effect.velocity_axis[0]).speed(0.1)).changed();
                ui.label(RichText::new("Y").size(10.0));
                changed |= ui.add(egui::DragValue::new(&mut effect.velocity_axis[1]).speed(0.1)).changed();
                ui.label(RichText::new("Z").size(10.0));
                changed |= ui.add(egui::DragValue::new(&mut effect.velocity_axis[2]).speed(0.1)).changed();
                changed
            });
        }
    });
    modified
}

fn render_forces_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    render_category(ui, WIND, "Forces", "effects", theme, "particle_forces", true, |ui| {
        modified |= inline_property_themed(ui, 0, "Accel", theme, |ui| {
            let mut changed = false;
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.label(RichText::new("X").size(10.0));
            changed |= ui.add(egui::DragValue::new(&mut effect.acceleration[0]).speed(0.1)).changed();
            ui.label(RichText::new("Y").size(10.0));
            changed |= ui.add(egui::DragValue::new(&mut effect.acceleration[1]).speed(0.1)).changed();
            ui.label(RichText::new("Z").size(10.0));
            changed |= ui.add(egui::DragValue::new(&mut effect.acceleration[2]).speed(0.1)).changed();
            changed
        });

        inline_property_themed(ui, 1, "Presets", theme, |ui| {
            if ui.small_button("None").clicked() {
                effect.acceleration = [0.0, 0.0, 0.0]; modified = true;
            }
            if ui.small_button("Light").clicked() {
                effect.acceleration = [0.0, -2.0, 0.0]; modified = true;
            }
            if ui.small_button("Normal").clicked() {
                effect.acceleration = [0.0, -9.8, 0.0]; modified = true;
            }
        });

        modified |= inline_property_themed(ui, 2, "Drag", theme, |ui| {
            ui.add(egui::DragValue::new(&mut effect.linear_drag).speed(0.05).range(0.0..=10.0)).changed()
        });

        modified |= inline_property_themed(ui, 3, "Radial Accel", theme, |ui| {
            ui.add(egui::DragValue::new(&mut effect.radial_acceleration).speed(0.1).range(-100.0..=100.0)).changed()
        });

        modified |= inline_property_themed(ui, 4, "Tangent Accel", theme, |ui| {
            ui.add(egui::DragValue::new(&mut effect.tangent_acceleration).speed(0.1).range(-100.0..=100.0)).changed()
        });

        if effect.tangent_acceleration.abs() > 0.001 {
            modified |= inline_property_themed(ui, 5, "Tangent Axis", theme, |ui| {
                let mut changed = false;
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.label(RichText::new("X").size(10.0));
                changed |= ui.add(egui::DragValue::new(&mut effect.tangent_accel_axis[0]).speed(0.1)).changed();
                ui.label(RichText::new("Y").size(10.0));
                changed |= ui.add(egui::DragValue::new(&mut effect.tangent_accel_axis[1]).speed(0.1)).changed();
                ui.label(RichText::new("Z").size(10.0));
                changed |= ui.add(egui::DragValue::new(&mut effect.tangent_accel_axis[2]).speed(0.1)).changed();
                changed
            });
        }
    });
    modified
}

fn render_size_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    render_category(ui, RESIZE, "Size Over Lifetime", "effects", theme, "particle_size", true, |ui| {
        let mut row = 0;

        modified |= inline_property_themed(ui, row, "Non-Uniform", theme, |ui| {
            ui.checkbox(&mut effect.size_non_uniform, "").changed()
        });
        row += 1;

        if effect.size_non_uniform {
            modified |= inline_property_themed(ui, row, "Start X", theme, |ui| {
                ui.add(egui::DragValue::new(&mut effect.size_start_x).speed(0.01).range(0.001..=10.0)).changed()
            });
            row += 1;
            modified |= inline_property_themed(ui, row, "Start Y", theme, |ui| {
                ui.add(egui::DragValue::new(&mut effect.size_start_y).speed(0.01).range(0.001..=10.0)).changed()
            });
            row += 1;
            modified |= inline_property_themed(ui, row, "End X", theme, |ui| {
                ui.add(egui::DragValue::new(&mut effect.size_end_x).speed(0.01).range(0.0..=10.0)).changed()
            });
            row += 1;
            modified |= inline_property_themed(ui, row, "End Y", theme, |ui| {
                ui.add(egui::DragValue::new(&mut effect.size_end_y).speed(0.01).range(0.0..=10.0)).changed()
            });
        } else {
            modified |= inline_property_themed(ui, row, "Start", theme, |ui| {
                ui.add(egui::DragValue::new(&mut effect.size_start).speed(0.01).range(0.001..=10.0)).changed()
            });
            row += 1;
            modified |= inline_property_themed(ui, row, "End", theme, |ui| {
                ui.add(egui::DragValue::new(&mut effect.size_end).speed(0.01).range(0.0..=10.0)).changed()
            });
            row += 1;
            inline_property_themed(ui, row, "Presets", theme, |ui| {
                if ui.small_button("Constant").clicked() {
                    effect.size_end = effect.size_start; modified = true;
                }
                if ui.small_button("Shrink").clicked() {
                    effect.size_end = 0.0; modified = true;
                }
                if ui.small_button("Grow").clicked() {
                    effect.size_end = effect.size_start * 2.0; modified = true;
                }
            });
        }
        row += 1;

        modified |= inline_property_themed(ui, row, "Random Min", theme, |ui| {
            ui.add(egui::DragValue::new(&mut effect.size_start_min).speed(0.01).range(0.0..=10.0)).changed()
        });
        row += 1;
        modified |= inline_property_themed(ui, row, "Random Max", theme, |ui| {
            ui.add(egui::DragValue::new(&mut effect.size_start_max).speed(0.01).range(0.0..=10.0)).changed()
        });
        row += 1;

        modified |= inline_property_themed(ui, row, "Screen Space", theme, |ui| {
            ui.checkbox(&mut effect.screen_space_size, "").changed()
        });
        row += 1;

        modified |= inline_property_themed(ui, row, "Roundness", theme, |ui| {
            ui.add(egui::Slider::new(&mut effect.roundness, 0.0..=1.0)).changed()
        });
    });
    modified
}

fn render_color_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    render_category(ui, PALETTE, "Color Over Lifetime", "effects", theme, "particle_color", true, |ui| {
        let mut row = 0;

        // Flat color toggle
        modified |= inline_property_themed(ui, row, "Flat Color", theme, |ui| {
            ui.checkbox(&mut effect.use_flat_color, "").changed()
        });
        row += 1;

        if effect.use_flat_color {
            modified |= inline_property_themed(ui, row, "Color", theme, |ui| {
                let mut color = egui::Rgba::from_rgba_unmultiplied(
                    effect.flat_color[0], effect.flat_color[1],
                    effect.flat_color[2], effect.flat_color[3],
                );
                if egui::color_picker::color_edit_button_rgba(ui, &mut color, egui::color_picker::Alpha::OnlyBlend).changed() {
                    effect.flat_color = [color.r(), color.g(), color.b(), color.a()];
                    true
                } else {
                    false
                }
            });
        } else {
            // Gradient preview bar
            let gradient_width = ui.available_width() - 8.0;
            let gradient_height = 20.0;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(gradient_width, gradient_height), Sense::hover());

            let stops = &effect.color_gradient;
            if stops.len() >= 2 {
                for i in 0..stops.len() - 1 {
                    let start = &stops[i];
                    let end = &stops[i + 1];
                    let x0 = rect.min.x + rect.width() * start.position;
                    let x1 = rect.min.x + rect.width() * end.position;
                    let color = Color32::from_rgba_unmultiplied(
                        (start.color[0] * 255.0) as u8,
                        (start.color[1] * 255.0) as u8,
                        (start.color[2] * 255.0) as u8,
                        (start.color[3] * 255.0) as u8,
                    );
                    let seg = Rect::from_min_max(Pos2::new(x0, rect.min.y), Pos2::new(x1, rect.max.y));
                    ui.painter().rect_filled(seg, 0.0, color);
                }
            }
            ui.painter().rect_stroke(rect, 2.0, Stroke::new(1.0, theme.widgets.border.to_color32()), egui::StrokeKind::Inside);

            ui.add_space(4.0);

            // Color stops
            let mut to_remove = None;
            let len = effect.color_gradient.len();
            for (i, stop) in effect.color_gradient.iter_mut().enumerate() {
                modified |= inline_property_themed(ui, row + i, &format!("Stop {}", i + 1), theme, |ui| {
                    let mut changed = false;
                    changed |= ui.add(egui::DragValue::new(&mut stop.position).speed(0.01).range(0.0..=1.0)).changed();
                    let mut color = egui::Rgba::from_rgba_unmultiplied(stop.color[0], stop.color[1], stop.color[2], stop.color[3]);
                    if egui::color_picker::color_edit_button_rgba(ui, &mut color, egui::color_picker::Alpha::OnlyBlend).changed() {
                        stop.color = [color.r(), color.g(), color.b(), color.a()];
                        changed = true;
                    }
                    if len > 2 && ui.small_button(format!("{}", MINUS)).clicked() {
                        to_remove = Some(i);
                    }
                    changed
                });
            }
            if let Some(idx) = to_remove {
                effect.color_gradient.remove(idx);
                modified = true;
            }
            row += len;

            // Add stop + presets
            inline_property_themed(ui, row, "Actions", theme, |ui| {
                if ui.small_button(format!("{} Add", PLUS)).clicked() && effect.color_gradient.len() < 8 {
                    effect.color_gradient.push(GradientStop { position: 0.5, color: [1.0, 1.0, 1.0, 1.0] });
                    effect.color_gradient.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());
                    modified = true;
                }
                if ui.small_button("Fire").clicked() {
                    effect.color_gradient = vec![
                        GradientStop { position: 0.0, color: [1.0, 1.0, 0.3, 1.0] },
                        GradientStop { position: 0.3, color: [1.0, 0.5, 0.0, 1.0] },
                        GradientStop { position: 1.0, color: [0.3, 0.1, 0.1, 0.0] },
                    ];
                    modified = true;
                }
                if ui.small_button("Smoke").clicked() {
                    effect.color_gradient = vec![
                        GradientStop { position: 0.0, color: [0.3, 0.3, 0.3, 0.8] },
                        GradientStop { position: 1.0, color: [0.5, 0.5, 0.5, 0.0] },
                    ];
                    modified = true;
                }
            });
        }
        row += 1;

        // HDR toggle
        modified |= inline_property_themed(ui, row, "HDR Color", theme, |ui| {
            ui.checkbox(&mut effect.use_hdr_color, "").changed()
        });
        row += 1;

        if effect.use_hdr_color {
            modified |= inline_property_themed(ui, row, "HDR Intensity", theme, |ui| {
                ui.add(egui::DragValue::new(&mut effect.hdr_intensity).speed(0.1).range(1.0..=100.0)).changed()
            });
            row += 1;
        }

        // Color blend mode
        modified |= inline_property_themed(ui, row, "Blend Mode", theme, |ui| {
            let mut changed = false;
            egui::ComboBox::from_id_salt("color_blend_mode_editor")
                .selected_text(match effect.color_blend_mode {
                    ParticleColorBlendMode::Modulate => "Modulate",
                    ParticleColorBlendMode::Overwrite => "Overwrite",
                    ParticleColorBlendMode::Add => "Add",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(effect.color_blend_mode == ParticleColorBlendMode::Modulate, "Modulate").clicked() {
                        effect.color_blend_mode = ParticleColorBlendMode::Modulate; changed = true;
                    }
                    if ui.selectable_label(effect.color_blend_mode == ParticleColorBlendMode::Overwrite, "Overwrite").clicked() {
                        effect.color_blend_mode = ParticleColorBlendMode::Overwrite; changed = true;
                    }
                    if ui.selectable_label(effect.color_blend_mode == ParticleColorBlendMode::Add, "Add").clicked() {
                        effect.color_blend_mode = ParticleColorBlendMode::Add; changed = true;
                    }
                });
            changed
        });
    });
    modified
}

fn render_rendering_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    render_category(ui, CUBE, "Rendering", "effects", theme, "particle_rendering", false, |ui| {
        let mut row = 0;

        // Alpha mode
        modified |= inline_property_themed(ui, row, "Alpha Mode", theme, |ui| {
            let mut changed = false;
            egui::ComboBox::from_id_salt("alpha_mode_editor")
                .selected_text(match effect.alpha_mode {
                    ParticleAlphaMode::Blend => "Blend",
                    ParticleAlphaMode::Premultiply => "Premultiply",
                    ParticleAlphaMode::Add => "Additive",
                    ParticleAlphaMode::Multiply => "Multiply",
                    ParticleAlphaMode::Mask => "Mask",
                    ParticleAlphaMode::Opaque => "Opaque",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(effect.alpha_mode == ParticleAlphaMode::Blend, "Blend").clicked() {
                        effect.alpha_mode = ParticleAlphaMode::Blend; changed = true;
                    }
                    if ui.selectable_label(effect.alpha_mode == ParticleAlphaMode::Premultiply, "Premultiply").clicked() {
                        effect.alpha_mode = ParticleAlphaMode::Premultiply; changed = true;
                    }
                    if ui.selectable_label(effect.alpha_mode == ParticleAlphaMode::Add, "Additive").clicked() {
                        effect.alpha_mode = ParticleAlphaMode::Add; changed = true;
                    }
                    if ui.selectable_label(effect.alpha_mode == ParticleAlphaMode::Multiply, "Multiply").clicked() {
                        effect.alpha_mode = ParticleAlphaMode::Multiply; changed = true;
                    }
                    if ui.selectable_label(effect.alpha_mode == ParticleAlphaMode::Mask, "Mask").clicked() {
                        effect.alpha_mode = ParticleAlphaMode::Mask; changed = true;
                    }
                    if ui.selectable_label(effect.alpha_mode == ParticleAlphaMode::Opaque, "Opaque").clicked() {
                        effect.alpha_mode = ParticleAlphaMode::Opaque; changed = true;
                    }
                });
            changed
        });
        row += 1;

        if effect.alpha_mode == ParticleAlphaMode::Mask {
            modified |= inline_property_themed(ui, row, "Mask Threshold", theme, |ui| {
                ui.add(egui::Slider::new(&mut effect.alpha_mask_threshold, 0.0..=1.0)).changed()
            });
            row += 1;
        }

        // Orient mode
        modified |= inline_property_themed(ui, row, "Orient Mode", theme, |ui| {
            let mut changed = false;
            egui::ComboBox::from_id_salt("orient_mode_editor")
                .selected_text(match effect.orient_mode {
                    ParticleOrientMode::ParallelCameraDepthPlane => "Camera Plane",
                    ParticleOrientMode::FaceCameraPosition => "Face Camera",
                    ParticleOrientMode::AlongVelocity => "Along Velocity",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(effect.orient_mode == ParticleOrientMode::ParallelCameraDepthPlane, "Camera Plane").clicked() {
                        effect.orient_mode = ParticleOrientMode::ParallelCameraDepthPlane; changed = true;
                    }
                    if ui.selectable_label(effect.orient_mode == ParticleOrientMode::FaceCameraPosition, "Face Camera").clicked() {
                        effect.orient_mode = ParticleOrientMode::FaceCameraPosition; changed = true;
                    }
                    if ui.selectable_label(effect.orient_mode == ParticleOrientMode::AlongVelocity, "Along Velocity").clicked() {
                        effect.orient_mode = ParticleOrientMode::AlongVelocity; changed = true;
                    }
                });
            changed
        });
        row += 1;

        // Rotation speed
        modified |= inline_property_themed(ui, row, "Rotation Speed", theme, |ui| {
            ui.add(egui::DragValue::new(&mut effect.rotation_speed).speed(0.1).range(-20.0..=20.0).suffix(" rad/s")).changed()
        });
        row += 1;

        // Texture
        modified |= inline_property_themed(ui, row, "Texture", theme, |ui| {
            let mut tex = effect.texture_path.clone().unwrap_or_default();
            let changed = ui.text_edit_singleline(&mut tex).changed();
            if changed {
                effect.texture_path = if tex.is_empty() { None } else { Some(tex) };
            }
            changed
        });
        row += 1;

        // Flipbook (only when texture is set)
        if effect.texture_path.is_some() {
            let has_flipbook = effect.flipbook.is_some();
            modified |= inline_property_themed(ui, row, "Flipbook", theme, |ui| {
                let mut enabled = has_flipbook;
                let changed = ui.checkbox(&mut enabled, "").changed();
                if changed {
                    if enabled {
                        effect.flipbook = Some(FlipbookSettings::default());
                    } else {
                        effect.flipbook = None;
                    }
                }
                changed
            });
            row += 1;

            if let Some(ref mut fb) = effect.flipbook {
                modified |= inline_property_themed(ui, row, "Grid Cols", theme, |ui| {
                    ui.add(egui::DragValue::new(&mut fb.grid_columns).range(1..=16)).changed()
                });
                row += 1;
                modified |= inline_property_themed(ui, row, "Grid Rows", theme, |ui| {
                    ui.add(egui::DragValue::new(&mut fb.grid_rows).range(1..=16)).changed()
                });
                row += 1;
            }
        }

        // Render layer
        modified |= inline_property_themed(ui, row, "Layer", theme, |ui| {
            ui.add(egui::DragValue::new(&mut effect.render_layer).range(0..=31)).changed()
        });
    });
    modified
}

fn render_simulation_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    render_category(ui, GEAR, "Simulation", "effects", theme, "particle_simulation", false, |ui| {
        modified |= inline_property_themed(ui, 0, "Space", theme, |ui| {
            let mut changed = false;
            if ui.radio(effect.simulation_space == SimulationSpace::Local, "Local").clicked() {
                effect.simulation_space = SimulationSpace::Local; changed = true;
            }
            if ui.radio(effect.simulation_space == SimulationSpace::World, "World").clicked() {
                effect.simulation_space = SimulationSpace::World; changed = true;
            }
            changed
        });
        modified |= inline_property_themed(ui, 1, "Update", theme, |ui| {
            let mut changed = false;
            if ui.radio(effect.simulation_condition == SimulationCondition::Always, "Always").clicked() {
                effect.simulation_condition = SimulationCondition::Always; changed = true;
            }
            if ui.radio(effect.simulation_condition == SimulationCondition::WhenVisible, "Visible").clicked() {
                effect.simulation_condition = SimulationCondition::WhenVisible; changed = true;
            }
            changed
        });
        modified |= inline_property_themed(ui, 2, "Integration", theme, |ui| {
            let mut changed = false;
            egui::ComboBox::from_id_salt("motion_integration_editor")
                .selected_text(match effect.motion_integration {
                    MotionIntegrationMode::PostUpdate => "Post-Update",
                    MotionIntegrationMode::PreUpdate => "Pre-Update",
                    MotionIntegrationMode::None => "None",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(effect.motion_integration == MotionIntegrationMode::PostUpdate, "Post-Update").clicked() {
                        effect.motion_integration = MotionIntegrationMode::PostUpdate; changed = true;
                    }
                    if ui.selectable_label(effect.motion_integration == MotionIntegrationMode::PreUpdate, "Pre-Update").clicked() {
                        effect.motion_integration = MotionIntegrationMode::PreUpdate; changed = true;
                    }
                    if ui.selectable_label(effect.motion_integration == MotionIntegrationMode::None, "None").clicked() {
                        effect.motion_integration = MotionIntegrationMode::None; changed = true;
                    }
                });
            changed
        });
    });
    modified
}

fn render_kill_zones_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    render_category(ui, PROHIBIT, "Kill Zones", "effects", theme, "particle_kill_zones", false, |ui| {
        let mut to_remove = None;

        for (i, zone) in effect.kill_zones.iter_mut().enumerate() {
            let zone_label = match zone {
                KillZone::Sphere { .. } => format!("Sphere {}", i + 1),
                KillZone::Aabb { .. } => format!("AABB {}", i + 1),
            };

            ui.label(RichText::new(&zone_label).strong().size(11.0));

            let base_row = i * 5;
            match zone {
                KillZone::Sphere { center, radius, kill_inside } => {
                    modified |= inline_property_themed(ui, base_row, "Center", theme, |ui| {
                        let mut changed = false;
                        ui.spacing_mut().item_spacing.x = 2.0;
                        changed |= ui.add(egui::DragValue::new(&mut center[0]).speed(0.1)).changed();
                        changed |= ui.add(egui::DragValue::new(&mut center[1]).speed(0.1)).changed();
                        changed |= ui.add(egui::DragValue::new(&mut center[2]).speed(0.1)).changed();
                        changed
                    });
                    modified |= inline_property_themed(ui, base_row + 1, "Radius", theme, |ui| {
                        ui.add(egui::DragValue::new(radius).speed(0.1).range(0.1..=100.0)).changed()
                    });
                    modified |= inline_property_themed(ui, base_row + 2, "Kill Inside", theme, |ui| {
                        ui.checkbox(kill_inside, "").changed()
                    });
                }
                KillZone::Aabb { center, half_size, kill_inside } => {
                    modified |= inline_property_themed(ui, base_row, "Center", theme, |ui| {
                        let mut changed = false;
                        ui.spacing_mut().item_spacing.x = 2.0;
                        changed |= ui.add(egui::DragValue::new(&mut center[0]).speed(0.1)).changed();
                        changed |= ui.add(egui::DragValue::new(&mut center[1]).speed(0.1)).changed();
                        changed |= ui.add(egui::DragValue::new(&mut center[2]).speed(0.1)).changed();
                        changed
                    });
                    modified |= inline_property_themed(ui, base_row + 1, "Half Size", theme, |ui| {
                        let mut changed = false;
                        ui.spacing_mut().item_spacing.x = 2.0;
                        changed |= ui.add(egui::DragValue::new(&mut half_size[0]).speed(0.1).range(0.1..=100.0)).changed();
                        changed |= ui.add(egui::DragValue::new(&mut half_size[1]).speed(0.1).range(0.1..=100.0)).changed();
                        changed |= ui.add(egui::DragValue::new(&mut half_size[2]).speed(0.1).range(0.1..=100.0)).changed();
                        changed
                    });
                    modified |= inline_property_themed(ui, base_row + 2, "Kill Inside", theme, |ui| {
                        ui.checkbox(kill_inside, "").changed()
                    });
                }
            }

            inline_property_themed(ui, base_row + 3, "Remove", theme, |ui| {
                if ui.small_button(format!("{} Remove", MINUS)).clicked() {
                    to_remove = Some(i);
                }
            });

            ui.add_space(4.0);
        }

        if let Some(idx) = to_remove {
            effect.kill_zones.remove(idx);
            modified = true;
        }

        // Add zone buttons
        ui.horizontal(|ui| {
            if ui.small_button(format!("{} Sphere Zone", PLUS)).clicked() {
                effect.kill_zones.push(KillZone::Sphere {
                    center: [0.0, 0.0, 0.0],
                    radius: 5.0,
                    kill_inside: false,
                });
                modified = true;
            }
            if ui.small_button(format!("{} AABB Zone", PLUS)).clicked() {
                effect.kill_zones.push(KillZone::Aabb {
                    center: [0.0, 0.0, 0.0],
                    half_size: [5.0, 5.0, 5.0],
                    kill_inside: false,
                });
                modified = true;
            }
        });
    });
    modified
}

fn render_conform_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    render_category(ui, ATOM, "Conform to Sphere", "effects", theme, "particle_conform", false, |ui| {
        let has_conform = effect.conform_to_sphere.is_some();
        modified |= inline_property_themed(ui, 0, "Enabled", theme, |ui| {
            let mut enabled = has_conform;
            let changed = ui.checkbox(&mut enabled, "").changed();
            if changed {
                if enabled {
                    effect.conform_to_sphere = Some(ConformToSphere::default());
                } else {
                    effect.conform_to_sphere = None;
                }
            }
            changed
        });

        if let Some(ref mut conform) = effect.conform_to_sphere {
            modified |= inline_property_themed(ui, 1, "Origin", theme, |ui| {
                let mut changed = false;
                ui.spacing_mut().item_spacing.x = 2.0;
                changed |= ui.add(egui::DragValue::new(&mut conform.origin[0]).speed(0.1)).changed();
                changed |= ui.add(egui::DragValue::new(&mut conform.origin[1]).speed(0.1)).changed();
                changed |= ui.add(egui::DragValue::new(&mut conform.origin[2]).speed(0.1)).changed();
                changed
            });
            modified |= inline_property_themed(ui, 2, "Radius", theme, |ui| {
                ui.add(egui::DragValue::new(&mut conform.radius).speed(0.1).range(0.1..=100.0)).changed()
            });
            modified |= inline_property_themed(ui, 3, "Influence Dist", theme, |ui| {
                ui.add(egui::DragValue::new(&mut conform.influence_dist).speed(0.1).range(0.0..=100.0)).changed()
            });
            modified |= inline_property_themed(ui, 4, "Accel", theme, |ui| {
                ui.add(egui::DragValue::new(&mut conform.attraction_accel).speed(0.1).range(0.0..=100.0)).changed()
            });
            modified |= inline_property_themed(ui, 5, "Max Speed", theme, |ui| {
                ui.add(egui::DragValue::new(&mut conform.max_attraction_speed).speed(0.1).range(0.0..=100.0)).changed()
            });
            modified |= inline_property_themed(ui, 6, "Shell Thick.", theme, |ui| {
                ui.add(egui::DragValue::new(&mut conform.shell_half_thickness).speed(0.01).range(0.0..=10.0)).changed()
            });
            modified |= inline_property_themed(ui, 7, "Sticky Factor", theme, |ui| {
                ui.add(egui::DragValue::new(&mut conform.sticky_factor).speed(0.01).range(0.0..=10.0)).changed()
            });
        }
    });
    modified
}

fn render_variables_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    render_category(ui, CODE, "Variables", "effects", theme, "particle_variables", false, |ui| {
        ui.label(egui::RichText::new("Exposed to scripts").size(10.0).color(theme.text.muted.to_color32()));

        let var_names: Vec<String> = effect.variables.keys().cloned().collect();
        let mut to_remove = None;

        for (i, name) in var_names.iter().enumerate() {
            if let Some(var) = effect.variables.get_mut(name) {
                modified |= inline_property_themed(ui, i, name, theme, |ui| {
                    let mut changed = false;
                    match var {
                        EffectVariable::Float { value, min, max } => {
                            changed |= ui.add(egui::DragValue::new(value).range(*min..=*max)).changed();
                        }
                        EffectVariable::Color { value } => {
                            let mut color = egui::Rgba::from_rgba_unmultiplied(value[0], value[1], value[2], value[3]);
                            if egui::color_picker::color_edit_button_rgba(ui, &mut color, egui::color_picker::Alpha::OnlyBlend).changed() {
                                *value = [color.r(), color.g(), color.b(), color.a()];
                                changed = true;
                            }
                        }
                        EffectVariable::Vec3 { value } => {
                            changed |= ui.add(egui::DragValue::new(&mut value[0]).speed(0.1)).changed();
                            changed |= ui.add(egui::DragValue::new(&mut value[1]).speed(0.1)).changed();
                            changed |= ui.add(egui::DragValue::new(&mut value[2]).speed(0.1)).changed();
                        }
                    }
                    if ui.small_button(format!("{}", MINUS)).clicked() {
                        to_remove = Some(name.clone());
                    }
                    changed
                });
            }
        }

        if let Some(name) = to_remove {
            effect.variables.remove(&name);
            modified = true;
        }

        inline_property_themed(ui, var_names.len(), "Add", theme, |ui| {
            if ui.small_button(format!("{} Float", PLUS)).clicked() {
                effect.variables.insert(format!("var_{}", effect.variables.len()), EffectVariable::Float { value: 1.0, min: 0.0, max: 10.0 });
                modified = true;
            }
            if ui.small_button(format!("{} Color", PLUS)).clicked() {
                effect.variables.insert(format!("color_{}", effect.variables.len()), EffectVariable::Color { value: [1.0, 1.0, 1.0, 1.0] });
                modified = true;
            }
        });
    });
    modified
}
