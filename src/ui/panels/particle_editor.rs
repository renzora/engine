//! Particle Editor Panel
//!
//! Provides a full-featured editor for bevy_hanabi particle effects.
//! Supports both asset-based and inline effect definitions.

use std::path::PathBuf;
use bevy_egui::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, TextureId, Vec2};
use egui_phosphor::regular::{PLAY, PAUSE, STOP, ARROW_CLOCKWISE, PLUS, MINUS, FLOPPY_DISK, FOLDER_OPEN};

use crate::particles::{
    HanabiEffectDefinition, HanabiEmitShape, ShapeDimension, SpawnMode, VelocityMode,
    BlendMode, BillboardMode, SimulationSpace, SimulationCondition, GradientStop,
    EffectVariable, ParticleEditorState,
};
use crate::core::{SceneManagerState, TabKind};
use crate::theming::Theme;

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
                    // Return default effect with the file name
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
            // Return default effect for new files
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
    // Check if a particle document is active
    let active_particle_path = match &scene_state.active_document {
        Some(TabKind::ParticleFX(idx)) => {
            scene_state.open_particles.get(*idx).map(|p| p.path.clone())
        }
        _ => None,
    };

    // If the active document changed, load the new effect
    let current_path = state.current_file_path.as_ref().map(|s| PathBuf::from(s));

    if active_particle_path != current_path {
        bevy::log::info!("[ParticleEditor] Document changed: {:?} -> {:?}", current_path, active_particle_path);
        if let Some(path) = active_particle_path {
            // Load the effect from file
            if let Some(effect) = load_effect_from_file(&path) {
                bevy::log::info!("[ParticleEditor] Loaded effect into state: '{}'", effect.name);
                state.current_effect = Some(effect);
                state.current_file_path = Some(path.to_string_lossy().to_string());
                state.is_modified = false;
            }
        } else if current_path.is_some() {
            // No particle document active, but we had one - keep the editor state
            // (don't clear it when switching to other document types)
            bevy::log::info!("[ParticleEditor] Keeping existing effect state");
        }
    }
}

/// Render the particle editor panel content
pub fn render_particle_editor_content(
    ui: &mut egui::Ui,
    state: &mut ParticleEditorState,
    scene_state: &mut SceneManagerState,
    preview_texture_id: Option<TextureId>,
    preview_size: (u32, u32),
    theme: &Theme,
) {
    // Sync with active document tab
    sync_with_active_document(state, scene_state);
    let available = ui.available_rect_before_wrap();
    let bg_color = theme.surfaces.panel.to_color32();

    // Draw background
    ui.painter().rect_filled(available, 0.0, bg_color);

    // If no effect is being edited, show welcome screen
    if state.current_effect.is_none() {
        render_welcome_screen(ui, state, scene_state, theme);
        return;
    }

    // Split into preview area and editor controls
    let preview_height = 200.0f32.min(available.height() * 0.3);
    let preview_rect = Rect::from_min_size(
        available.min,
        Vec2::new(available.width(), preview_height),
    );
    let editor_rect = Rect::from_min_max(
        Pos2::new(available.min.x, available.min.y + preview_height + 8.0),
        available.max,
    );

    // Render preview area
    render_preview_area(ui, preview_rect, preview_texture_id, preview_size, state, theme);

    // Track if save was requested
    let mut save_requested = false;

    // Render editor controls in scrollable area
    ui.allocate_ui_at_rect(editor_rect, |ui| {
        egui::ScrollArea::vertical()
            .id_salt("particle_editor_scroll")
            .show(ui, |ui| {
                // Take out the effect temporarily to avoid borrowing issues
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
    });

    // Handle save
    if save_requested {
        if let (Some(effect), Some(path_str)) = (&state.current_effect, &state.current_file_path) {
            let path = PathBuf::from(path_str);
            if save_effect_to_file(&path, effect) {
                state.is_modified = false;
                // Update the document tab's modified state
                if let Some(TabKind::ParticleFX(idx)) = &scene_state.active_document {
                    if let Some(doc) = scene_state.open_particles.get_mut(*idx) {
                        doc.is_modified = false;
                    }
                }
            }
        }
    }

    // Sync modified state to document tab
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

    // Check if there's an active particle document we should load
    if let Some(TabKind::ParticleFX(idx)) = &scene_state.active_document {
        if let Some(doc) = scene_state.open_particles.get(*idx) {
            // Auto-load the active document
            if let Some(effect) = load_effect_from_file(&doc.path) {
                state.current_effect = Some(effect);
                state.current_file_path = Some(doc.path.to_string_lossy().to_string());
                state.is_modified = doc.is_modified;
                return; // Will re-render with the loaded effect
            }
        }
    }

    ui.allocate_ui_at_rect(available, |ui| {
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

fn render_preview_area(
    ui: &mut egui::Ui,
    rect: Rect,
    texture_id: Option<TextureId>,
    size: (u32, u32),
    state: &mut ParticleEditorState,
    theme: &Theme,
) {
    let text_muted = theme.text.muted.to_color32();
    let border_color = theme.widgets.border.to_color32();
    let bg_dark = theme.surfaces.faint.to_color32();

    // Background
    ui.painter().rect_filled(rect, 4.0, bg_dark);
    ui.painter().rect_stroke(rect, 4.0, Stroke::new(1.0, border_color), egui::StrokeKind::Inside);

    // Preview image or placeholder
    if let Some(tex_id) = texture_id {
        let texture_aspect = size.0 as f32 / size.1.max(1) as f32;
        let panel_aspect = rect.width() / rect.height();

        let (display_width, display_height) = if texture_aspect > panel_aspect {
            (rect.width() - 16.0, (rect.width() - 16.0) / texture_aspect)
        } else {
            ((rect.height() - 40.0) * texture_aspect, rect.height() - 40.0)
        };

        let display_rect = Rect::from_center_size(
            Pos2::new(rect.center().x, rect.min.y + 20.0 + display_height / 2.0),
            Vec2::new(display_width, display_height),
        );

        ui.painter().image(
            tex_id,
            display_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        // Placeholder text
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Preview",
            egui::FontId::proportional(14.0),
            text_muted,
        );
    }

    // Playback controls at bottom of preview
    let controls_rect = Rect::from_min_size(
        Pos2::new(rect.min.x + 8.0, rect.max.y - 32.0),
        Vec2::new(rect.width() - 16.0, 24.0),
    );

    ui.allocate_ui_at_rect(controls_rect, |ui| {
        ui.horizontal(|ui| {
            // Play/Pause button
            let play_icon = if state.preview_playing { PAUSE } else { PLAY };
            if ui.small_button(play_icon).clicked() {
                state.preview_playing = !state.preview_playing;
            }

            // Stop button
            if ui.small_button(STOP).clicked() {
                state.preview_playing = false;
            }

            // Reset button
            if ui.small_button(ARROW_CLOCKWISE).clicked() {
                // TODO: Reset effect
            }

            ui.separator();

            // Effect name
            if let Some(ref effect) = state.current_effect {
                ui.label(RichText::new(&effect.name).size(11.0).color(text_muted));
            }

            // Modified indicator
            if state.is_modified {
                ui.label(RichText::new("*").color(theme.semantic.warning.to_color32()));
            }
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

    ui.separator();

    // Basic settings
    ui.horizontal(|ui| {
        ui.label("Name:");
        if ui.text_edit_singleline(&mut effect.name).changed() {
            modified = true;
        }
    });

    ui.horizontal(|ui| {
        ui.label("Capacity:");
        if ui.add(egui::DragValue::new(&mut effect.capacity).range(10..=100000)).changed() {
            modified = true;
        }
    });

    ui.add_space(8.0);

    // Spawning section
    modified |= render_spawning_section(ui, effect);

    // Lifetime section
    modified |= render_lifetime_section(ui, effect);

    // Emission shape section
    modified |= render_shape_section(ui, effect);

    // Velocity section
    modified |= render_velocity_section(ui, effect);

    // Forces section
    modified |= render_forces_section(ui, effect);

    // Size section
    modified |= render_size_section(ui, effect);

    // Color section
    modified |= render_color_section(ui, effect, theme);

    // Rendering section
    modified |= render_rendering_section(ui, effect);

    // Simulation section
    modified |= render_simulation_section(ui, effect);

    // Variables section
    modified |= render_variables_section(ui, effect, theme);

    (modified, save_requested)
}

fn render_spawning_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition) -> bool {
    let mut modified = false;
    ui.collapsing("Spawning", |ui| {
        ui.horizontal(|ui| {
            ui.label("Mode:");
            egui::ComboBox::from_id_salt("spawn_mode_editor")
                .selected_text(match effect.spawn_mode {
                    SpawnMode::Rate => "Continuous",
                    SpawnMode::Burst => "Single Burst",
                    SpawnMode::BurstRate => "Repeated Bursts",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(effect.spawn_mode == SpawnMode::Rate, "Continuous").clicked() {
                        effect.spawn_mode = SpawnMode::Rate;
                        modified = true;
                    }
                    if ui.selectable_label(effect.spawn_mode == SpawnMode::Burst, "Single Burst").clicked() {
                        effect.spawn_mode = SpawnMode::Burst;
                        modified = true;
                    }
                    if ui.selectable_label(effect.spawn_mode == SpawnMode::BurstRate, "Repeated Bursts").clicked() {
                        effect.spawn_mode = SpawnMode::BurstRate;
                        modified = true;
                    }
                });
        });

        match effect.spawn_mode {
            SpawnMode::Rate => {
                ui.horizontal(|ui| {
                    ui.label("Rate (per sec):");
                    if ui.add(egui::DragValue::new(&mut effect.spawn_rate)
                        .speed(1.0)
                        .range(0.1..=10000.0)).changed()
                    {
                        modified = true;
                    }
                });
            }
            SpawnMode::Burst => {
                ui.horizontal(|ui| {
                    ui.label("Count:");
                    if ui.add(egui::DragValue::new(&mut effect.spawn_count)
                        .range(1..=10000)).changed()
                    {
                        modified = true;
                    }
                });
            }
            SpawnMode::BurstRate => {
                ui.horizontal(|ui| {
                    ui.label("Count:");
                    if ui.add(egui::DragValue::new(&mut effect.spawn_count)
                        .range(1..=10000)).changed()
                    {
                        modified = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Bursts/sec:");
                    if ui.add(egui::DragValue::new(&mut effect.spawn_rate)
                        .speed(0.1)
                        .range(0.1..=100.0)).changed()
                    {
                        modified = true;
                    }
                });
            }
        }
    });
    modified
}

fn render_lifetime_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition) -> bool {
    let mut modified = false;
    ui.collapsing("Lifetime", |ui| {
        ui.horizontal(|ui| {
            ui.label("Min:");
            if ui.add(egui::DragValue::new(&mut effect.lifetime_min)
                .speed(0.1)
                .range(0.01..=60.0)
                .suffix("s")).changed()
            {
                modified = true;
                if effect.lifetime_min > effect.lifetime_max {
                    effect.lifetime_max = effect.lifetime_min;
                }
            }
        });

        ui.horizontal(|ui| {
            ui.label("Max:");
            if ui.add(egui::DragValue::new(&mut effect.lifetime_max)
                .speed(0.1)
                .range(0.01..=60.0)
                .suffix("s")).changed()
            {
                modified = true;
                if effect.lifetime_max < effect.lifetime_min {
                    effect.lifetime_min = effect.lifetime_max;
                }
            }
        });
    });
    modified
}

fn render_shape_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition) -> bool {
    let mut modified = false;
    ui.collapsing("Emission Shape", |ui| {
        let shape_name = match &effect.emit_shape {
            HanabiEmitShape::Point => "Point",
            HanabiEmitShape::Circle { .. } => "Circle",
            HanabiEmitShape::Sphere { .. } => "Sphere",
            HanabiEmitShape::Cone { .. } => "Cone",
            HanabiEmitShape::Rect { .. } => "Rectangle",
            HanabiEmitShape::Box { .. } => "Box",
        };

        ui.horizontal(|ui| {
            ui.label("Shape:");
            egui::ComboBox::from_id_salt("emit_shape_editor")
                .selected_text(shape_name)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(matches!(&effect.emit_shape, HanabiEmitShape::Point), "Point").clicked() {
                        effect.emit_shape = HanabiEmitShape::Point;
                        modified = true;
                    }
                    if ui.selectable_label(matches!(&effect.emit_shape, HanabiEmitShape::Circle { .. }), "Circle").clicked() {
                        effect.emit_shape = HanabiEmitShape::Circle { radius: 1.0, dimension: ShapeDimension::Volume };
                        modified = true;
                    }
                    if ui.selectable_label(matches!(&effect.emit_shape, HanabiEmitShape::Sphere { .. }), "Sphere").clicked() {
                        effect.emit_shape = HanabiEmitShape::Sphere { radius: 1.0, dimension: ShapeDimension::Volume };
                        modified = true;
                    }
                    if ui.selectable_label(matches!(&effect.emit_shape, HanabiEmitShape::Cone { .. }), "Cone").clicked() {
                        effect.emit_shape = HanabiEmitShape::Cone { base_radius: 0.5, top_radius: 0.0, height: 1.0, dimension: ShapeDimension::Volume };
                        modified = true;
                    }
                    if ui.selectable_label(matches!(&effect.emit_shape, HanabiEmitShape::Rect { .. }), "Rectangle").clicked() {
                        effect.emit_shape = HanabiEmitShape::Rect { half_extents: [1.0, 1.0], dimension: ShapeDimension::Volume };
                        modified = true;
                    }
                    if ui.selectable_label(matches!(&effect.emit_shape, HanabiEmitShape::Box { .. }), "Box").clicked() {
                        effect.emit_shape = HanabiEmitShape::Box { half_extents: [1.0, 1.0, 1.0] };
                        modified = true;
                    }
                });
        });

        // Shape-specific parameters
        match &mut effect.emit_shape {
            HanabiEmitShape::Point => {}
            HanabiEmitShape::Circle { radius, dimension } |
            HanabiEmitShape::Sphere { radius, dimension } => {
                ui.horizontal(|ui| {
                    ui.label("Radius:");
                    if ui.add(egui::DragValue::new(radius).speed(0.1).range(0.001..=100.0)).changed() {
                        modified = true;
                    }
                });
                modified |= render_dimension_selector(ui, dimension);
            }
            HanabiEmitShape::Cone { base_radius, top_radius, height, dimension } => {
                ui.horizontal(|ui| {
                    ui.label("Base Radius:");
                    if ui.add(egui::DragValue::new(base_radius).speed(0.1).range(0.0..=100.0)).changed() {
                        modified = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Top Radius:");
                    if ui.add(egui::DragValue::new(top_radius).speed(0.1).range(0.0..=100.0)).changed() {
                        modified = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Height:");
                    if ui.add(egui::DragValue::new(height).speed(0.1).range(0.001..=100.0)).changed() {
                        modified = true;
                    }
                });
                modified |= render_dimension_selector(ui, dimension);
            }
            HanabiEmitShape::Rect { half_extents, dimension } => {
                ui.horizontal(|ui| {
                    ui.label("Half Extents X:");
                    if ui.add(egui::DragValue::new(&mut half_extents[0]).speed(0.1).range(0.001..=100.0)).changed() {
                        modified = true;
                    }
                    ui.label("Y:");
                    if ui.add(egui::DragValue::new(&mut half_extents[1]).speed(0.1).range(0.001..=100.0)).changed() {
                        modified = true;
                    }
                });
                modified |= render_dimension_selector(ui, dimension);
            }
            HanabiEmitShape::Box { half_extents } => {
                ui.horizontal(|ui| {
                    ui.label("X:");
                    if ui.add(egui::DragValue::new(&mut half_extents[0]).speed(0.1).range(0.001..=100.0)).changed() {
                        modified = true;
                    }
                    ui.label("Y:");
                    if ui.add(egui::DragValue::new(&mut half_extents[1]).speed(0.1).range(0.001..=100.0)).changed() {
                        modified = true;
                    }
                    ui.label("Z:");
                    if ui.add(egui::DragValue::new(&mut half_extents[2]).speed(0.1).range(0.001..=100.0)).changed() {
                        modified = true;
                    }
                });
            }
        }
    });
    modified
}

fn render_dimension_selector(ui: &mut egui::Ui, dimension: &mut ShapeDimension) -> bool {
    let mut modified = false;
    ui.horizontal(|ui| {
        ui.label("Emit from:");
        if ui.radio(*dimension == ShapeDimension::Volume, "Volume").clicked() {
            *dimension = ShapeDimension::Volume;
            modified = true;
        }
        if ui.radio(*dimension == ShapeDimension::Surface, "Surface").clicked() {
            *dimension = ShapeDimension::Surface;
            modified = true;
        }
    });
    modified
}

fn render_velocity_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition) -> bool {
    let mut modified = false;
    ui.collapsing("Velocity", |ui| {
        ui.horizontal(|ui| {
            ui.label("Mode:");
            egui::ComboBox::from_id_salt("velocity_mode_editor")
                .selected_text(match effect.velocity_mode {
                    VelocityMode::Directional => "Directional",
                    VelocityMode::Radial => "Radial",
                    VelocityMode::Tangent => "Tangent",
                    VelocityMode::Random => "Random",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(effect.velocity_mode == VelocityMode::Directional, "Directional").clicked() {
                        effect.velocity_mode = VelocityMode::Directional;
                        modified = true;
                    }
                    if ui.selectable_label(effect.velocity_mode == VelocityMode::Radial, "Radial").clicked() {
                        effect.velocity_mode = VelocityMode::Radial;
                        modified = true;
                    }
                    if ui.selectable_label(effect.velocity_mode == VelocityMode::Tangent, "Tangent").clicked() {
                        effect.velocity_mode = VelocityMode::Tangent;
                        modified = true;
                    }
                    if ui.selectable_label(effect.velocity_mode == VelocityMode::Random, "Random").clicked() {
                        effect.velocity_mode = VelocityMode::Random;
                        modified = true;
                    }
                });
        });

        ui.horizontal(|ui| {
            ui.label("Speed:");
            if ui.add(egui::DragValue::new(&mut effect.velocity_magnitude).speed(0.1).range(0.0..=100.0)).changed() {
                modified = true;
            }
        });

        if effect.velocity_mode == VelocityMode::Directional {
            ui.horizontal(|ui| {
                ui.label("Spread:");
                if ui.add(egui::DragValue::new(&mut effect.velocity_spread).speed(0.05).range(0.0..=std::f32::consts::PI)).changed() {
                    modified = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Dir X:");
                if ui.add(egui::DragValue::new(&mut effect.velocity_direction[0]).speed(0.1)).changed() {
                    modified = true;
                }
                ui.label("Y:");
                if ui.add(egui::DragValue::new(&mut effect.velocity_direction[1]).speed(0.1)).changed() {
                    modified = true;
                }
                ui.label("Z:");
                if ui.add(egui::DragValue::new(&mut effect.velocity_direction[2]).speed(0.1)).changed() {
                    modified = true;
                }
            });
        }
    });
    modified
}

fn render_forces_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition) -> bool {
    let mut modified = false;
    ui.collapsing("Forces", |ui| {
        ui.label("Acceleration:");
        ui.horizontal(|ui| {
            ui.label("X:");
            if ui.add(egui::DragValue::new(&mut effect.acceleration[0]).speed(0.1)).changed() {
                modified = true;
            }
            ui.label("Y:");
            if ui.add(egui::DragValue::new(&mut effect.acceleration[1]).speed(0.1)).changed() {
                modified = true;
            }
            ui.label("Z:");
            if ui.add(egui::DragValue::new(&mut effect.acceleration[2]).speed(0.1)).changed() {
                modified = true;
            }
        });

        ui.horizontal(|ui| {
            if ui.small_button("No Gravity").clicked() {
                effect.acceleration = [0.0, 0.0, 0.0];
                modified = true;
            }
            if ui.small_button("Light").clicked() {
                effect.acceleration = [0.0, -2.0, 0.0];
                modified = true;
            }
            if ui.small_button("Normal").clicked() {
                effect.acceleration = [0.0, -9.8, 0.0];
                modified = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Drag:");
            if ui.add(egui::DragValue::new(&mut effect.linear_drag).speed(0.05).range(0.0..=10.0)).changed() {
                modified = true;
            }
        });
    });
    modified
}

fn render_size_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition) -> bool {
    let mut modified = false;
    ui.collapsing("Size Over Lifetime", |ui| {
        ui.horizontal(|ui| {
            ui.label("Start:");
            if ui.add(egui::DragValue::new(&mut effect.size_start).speed(0.01).range(0.001..=10.0)).changed() {
                modified = true;
            }
            ui.label("End:");
            if ui.add(egui::DragValue::new(&mut effect.size_end).speed(0.01).range(0.0..=10.0)).changed() {
                modified = true;
            }
        });

        ui.horizontal(|ui| {
            if ui.small_button("Constant").clicked() {
                effect.size_end = effect.size_start;
                modified = true;
            }
            if ui.small_button("Shrink").clicked() {
                effect.size_end = 0.0;
                modified = true;
            }
            if ui.small_button("Grow").clicked() {
                effect.size_end = effect.size_start * 2.0;
                modified = true;
            }
        });
    });
    modified
}

fn render_color_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    ui.collapsing("Color Over Lifetime", |ui| {
        let gradient_height = 24.0;
        let gradient_width = ui.available_width() - 40.0;

        let (rect, _) = ui.allocate_exact_size(Vec2::new(gradient_width, gradient_height), Sense::hover());

        // Draw gradient preview
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

        ui.horizontal(|ui| {
            ui.label("Stops:");
            if ui.small_button(format!("{}", PLUS)).clicked() && effect.color_gradient.len() < 8 {
                effect.color_gradient.push(GradientStop { position: 0.5, color: [1.0, 1.0, 1.0, 1.0] });
                effect.color_gradient.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());
                modified = true;
            }
        });

        let mut to_remove = None;
        let len = effect.color_gradient.len();
        for (i, stop) in effect.color_gradient.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{}:", i + 1));
                if ui.add(egui::DragValue::new(&mut stop.position).speed(0.01).range(0.0..=1.0)).changed() {
                    modified = true;
                }
                let mut color = egui::Rgba::from_rgba_unmultiplied(stop.color[0], stop.color[1], stop.color[2], stop.color[3]);
                if egui::color_picker::color_edit_button_rgba(ui, &mut color, egui::color_picker::Alpha::OnlyBlend).changed() {
                    stop.color = [color.r(), color.g(), color.b(), color.a()];
                    modified = true;
                }
                if len > 2 && ui.small_button(format!("{}", MINUS)).clicked() {
                    to_remove = Some(i);
                }
            });
        }
        if let Some(idx) = to_remove {
            effect.color_gradient.remove(idx);
            modified = true;
        }

        ui.horizontal(|ui| {
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
    });
    modified
}

fn render_rendering_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition) -> bool {
    let mut modified = false;
    ui.collapsing("Rendering", |ui| {
        ui.horizontal(|ui| {
            ui.label("Blend:");
            egui::ComboBox::from_id_salt("blend_mode_editor")
                .selected_text(match effect.blend_mode {
                    BlendMode::Blend => "Alpha",
                    BlendMode::Additive => "Additive",
                    BlendMode::Multiply => "Multiply",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(effect.blend_mode == BlendMode::Blend, "Alpha").clicked() {
                        effect.blend_mode = BlendMode::Blend;
                        modified = true;
                    }
                    if ui.selectable_label(effect.blend_mode == BlendMode::Additive, "Additive").clicked() {
                        effect.blend_mode = BlendMode::Additive;
                        modified = true;
                    }
                    if ui.selectable_label(effect.blend_mode == BlendMode::Multiply, "Multiply").clicked() {
                        effect.blend_mode = BlendMode::Multiply;
                        modified = true;
                    }
                });
        });

        ui.horizontal(|ui| {
            ui.label("Billboard:");
            egui::ComboBox::from_id_salt("billboard_mode_editor")
                .selected_text(match effect.billboard_mode {
                    BillboardMode::FaceCamera => "Face Camera",
                    BillboardMode::FaceCameraY => "Y Lock",
                    BillboardMode::Velocity => "Velocity",
                    BillboardMode::Fixed => "Fixed",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(effect.billboard_mode == BillboardMode::FaceCamera, "Face Camera").clicked() {
                        effect.billboard_mode = BillboardMode::FaceCamera;
                        modified = true;
                    }
                    if ui.selectable_label(effect.billboard_mode == BillboardMode::FaceCameraY, "Y Lock").clicked() {
                        effect.billboard_mode = BillboardMode::FaceCameraY;
                        modified = true;
                    }
                    if ui.selectable_label(effect.billboard_mode == BillboardMode::Velocity, "Velocity").clicked() {
                        effect.billboard_mode = BillboardMode::Velocity;
                        modified = true;
                    }
                    if ui.selectable_label(effect.billboard_mode == BillboardMode::Fixed, "Fixed").clicked() {
                        effect.billboard_mode = BillboardMode::Fixed;
                        modified = true;
                    }
                });
        });

        ui.horizontal(|ui| {
            ui.label("Texture:");
            let mut tex = effect.texture_path.clone().unwrap_or_default();
            if ui.text_edit_singleline(&mut tex).changed() {
                effect.texture_path = if tex.is_empty() { None } else { Some(tex) };
                modified = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Layer:");
            if ui.add(egui::DragValue::new(&mut effect.render_layer).range(0..=31)).changed() {
                modified = true;
            }
        });
    });
    modified
}

fn render_simulation_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition) -> bool {
    let mut modified = false;
    ui.collapsing("Simulation", |ui| {
        ui.horizontal(|ui| {
            ui.label("Space:");
            if ui.radio(effect.simulation_space == SimulationSpace::Local, "Local").clicked() {
                effect.simulation_space = SimulationSpace::Local;
                modified = true;
            }
            if ui.radio(effect.simulation_space == SimulationSpace::World, "World").clicked() {
                effect.simulation_space = SimulationSpace::World;
                modified = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Update:");
            if ui.radio(effect.simulation_condition == SimulationCondition::Always, "Always").clicked() {
                effect.simulation_condition = SimulationCondition::Always;
                modified = true;
            }
            if ui.radio(effect.simulation_condition == SimulationCondition::WhenVisible, "Visible").clicked() {
                effect.simulation_condition = SimulationCondition::WhenVisible;
                modified = true;
            }
        });
    });
    modified
}

fn render_variables_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    ui.collapsing("Variables", |ui| {
        ui.label(egui::RichText::new("Exposed to scripts").size(10.0).color(theme.text.muted.to_color32()));

        let var_names: Vec<String> = effect.variables.keys().cloned().collect();
        let mut to_remove = None;

        for name in &var_names {
            if let Some(var) = effect.variables.get_mut(name) {
                ui.horizontal(|ui| {
                    ui.label(name);
                    match var {
                        EffectVariable::Float { value, min, max } => {
                            if ui.add(egui::DragValue::new(value).range(*min..=*max)).changed() {
                                modified = true;
                            }
                        }
                        EffectVariable::Color { value } => {
                            let mut color = egui::Rgba::from_rgba_unmultiplied(value[0], value[1], value[2], value[3]);
                            if egui::color_picker::color_edit_button_rgba(ui, &mut color, egui::color_picker::Alpha::OnlyBlend).changed() {
                                *value = [color.r(), color.g(), color.b(), color.a()];
                                modified = true;
                            }
                        }
                        EffectVariable::Vec3 { value } => {
                            if ui.add(egui::DragValue::new(&mut value[0]).speed(0.1)).changed() { modified = true; }
                            if ui.add(egui::DragValue::new(&mut value[1]).speed(0.1)).changed() { modified = true; }
                            if ui.add(egui::DragValue::new(&mut value[2]).speed(0.1)).changed() { modified = true; }
                        }
                    }
                    if ui.small_button(format!("{}", MINUS)).clicked() {
                        to_remove = Some(name.clone());
                    }
                });
            }
        }

        if let Some(name) = to_remove {
            effect.variables.remove(&name);
            modified = true;
        }

        ui.horizontal(|ui| {
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
