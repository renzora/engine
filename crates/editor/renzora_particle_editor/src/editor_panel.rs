//! Particle Editor Panel — full-featured editor for bevy_hanabi particle effects.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};
use egui_phosphor::regular::{
    PLUS, MINUS, FLOPPY_DISK, FOLDER_OPEN,
    SPARKLE, TIMER, SHAPES, ARROWS_OUT, WIND, RESIZE, PALETTE, CUBE, GEAR, CODE,
    PROHIBIT, ATOM, SPIRAL, PLANET, GAUGE,
};

use renzora_editor::{collapsible_section, EditorCommands, EditorPanel, PanelLocation};
use renzora_theme::{Theme, ThemeManager};

use renzora_hanabi::{
    HanabiEffectDefinition, HanabiEmitShape, ShapeDimension, SpawnMode, VelocityMode,
    SimulationSpace, SimulationCondition, GradientStop,
    EffectVariable, ParticleEditorState, EditorMode,
    ParticleAlphaMode, ParticleOrientMode, ParticleColorBlendMode,
    MotionIntegrationMode, KillZone, ConformToSphere, FlipbookSettings, OrbitSettings,
    load_effect_from_file,
};

use crate::widgets::inline_property;

pub struct ParticleEditorPanel;

impl Default for ParticleEditorPanel {
    fn default() -> Self {
        Self
    }
}

impl EditorPanel for ParticleEditorPanel {
    fn id(&self) -> &str {
        "particle_editor"
    }

    fn title(&self) -> &str {
        "Particle Editor"
    }

    fn icon(&self) -> Option<&str> {
        Some(SPARKLE)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };
        let cmds = match world.get_resource::<EditorCommands>() {
            Some(c) => c,
            None => return,
        };

        let Some(editor_state) = world.get_resource::<ParticleEditorState>() else {
            return;
        };

        // Check if we have an effect loaded
        if editor_state.current_effect.is_none() {
            self.render_welcome_screen(ui, cmds, &theme);
            return;
        }

        // "Advanced" button to switch to graph layout
        let is_advanced = editor_state.editor_mode == EditorMode::Graph;
        ui.horizontal(|ui| {
            let label = if is_advanced { "Switch to Simple" } else { "Switch to Advanced" };
            if ui.small_button(label).clicked() {
                cmds.push(move |world: &mut World| {
                    use renzora_editor::dock_tree::DockingState;
                    let mut state = world.resource_mut::<ParticleEditorState>();
                    if is_advanced {
                        state.editor_mode = EditorMode::Simple;
                    } else {
                        state.editor_mode = EditorMode::Graph;
                        // Always regenerate graph from current effect
                        if let Some(ref effect) = state.current_effect {
                            state.node_graph = Some(renzora_hanabi::node_graph::ParticleNodeGraph::from_effect(effect));
                        } else {
                            let name = "New Effect".to_string();
                            state.node_graph = Some(renzora_hanabi::node_graph::ParticleNodeGraph::new(&name));
                        }
                    }
                    // Swap layout
                    if let Some(mut docking) = world.get_resource_mut::<DockingState>() {
                        docking.tree = if is_advanced {
                            renzora_editor::layouts::layout_particles()
                        } else {
                            renzora_editor::layouts::layout_particles_advanced()
                        };
                    }
                });
            }
        });
        ui.separator();

        if is_advanced {
            // Advanced mode: show selected node properties
            self.render_graph_inspector(ui, cmds, editor_state, &theme);
            return;
        }

        // Clone the current effect for editing
        let mut effect = editor_state.current_effect.as_ref().unwrap().clone();
        let file_path = editor_state.current_file_path.clone();
        let mut modified = false;
        let mut save_requested = false;
        let mut save_as_requested = false;

        egui::ScrollArea::vertical()
            .id_salt("particle_editor_scroll")
            .show(ui, |ui| {
                let (m, s, sa) = render_effect_editor(ui, &mut effect, file_path.as_ref(), &theme);
                modified = m;
                save_requested = s;
                save_as_requested = sa;
            });

        // Apply changes via EditorCommands
        if modified || save_requested || save_as_requested {
            let effect_clone = effect.clone();
            cmds.push(move |world: &mut World| {
                let mut state = world.resource_mut::<ParticleEditorState>();
                state.current_effect = Some(effect_clone);
                if modified {
                    state.is_modified = true;
                }
            });
        }

        if save_requested {
            if let Some(path_str) = file_path.as_ref() {
                let path = PathBuf::from(path_str);
                let effect_to_save = effect.clone();
                let path_str_clone = path_str.clone();
                cmds.push(move |world: &mut World| {
                    if save_effect_to_file(&path, &effect_to_save) {
                        let mut state = world.resource_mut::<ParticleEditorState>();
                        state.is_modified = false;
                        state.recently_saved_paths.push(path_str_clone);
                    }
                });
            }
        }

        if save_as_requested {
            let effect_to_save = effect.clone();
            cmds.push(move |world: &mut World| {
                let target_dir = PathBuf::from(".");
                let sanitized = effect_to_save.name.replace(|c: char| !c.is_alphanumeric() && c != '_' && c != '-' && c != ' ', "");
                let base = if sanitized.is_empty() { "effect".to_string() } else { sanitized };
                let mut filename = format!("{}.particle", base);
                let mut counter = 1;
                while target_dir.join(&filename).exists() {
                    filename = format!("{}_{}.particle", base, counter);
                    counter += 1;
                }
                let save_path = target_dir.join(&filename);
                if save_effect_to_file(&save_path, &effect_to_save) {
                    let path_str = save_path.to_string_lossy().to_string();
                    let mut state = world.resource_mut::<ParticleEditorState>();
                    state.current_file_path = Some(path_str.clone());
                    state.is_modified = false;
                    state.recently_saved_paths.push(path_str);
                }
            });
        }
    }

    fn closable(&self) -> bool {
        true
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Center
    }
}

impl ParticleEditorPanel {
    fn render_graph_inspector(&self, ui: &mut egui::Ui, cmds: &EditorCommands, editor_state: &ParticleEditorState, theme: &Theme) {
        use renzora_hanabi::node_graph::{PinDir, PinValue, ParticleNodeType};

        let text_muted = theme.text.muted.to_color32();

        let Some(ref graph) = editor_state.node_graph else {
            ui.label(RichText::new("No graph initialized").size(11.0).color(text_muted));
            return;
        };

        // Collect all connected nodes grouped by category
        let emitter_id = graph.nodes.iter()
            .find(|n| n.node_type == ParticleNodeType::Emitter)
            .map(|n| n.id);

        // Build ordered list: emitter first, then nodes by category
        let categories = [
            ("Emitter", GEAR, "emitter"),
            ("Spawn", SPARKLE, "effects"),
            ("Init", ARROWS_OUT, "effects"),
            ("Update", WIND, "effects"),
            ("Render", PALETTE, "effects"),
            ("Math", GEAR, "effects"),
            ("Constants", CODE, "effects"),
        ];

        // Collect all modified values across all nodes
        let mut all_modified_values: Vec<(u64, std::collections::HashMap<String, PinValue>)> = Vec::new();
        let mut any_modified = false;

        egui::ScrollArea::vertical()
            .id_salt("graph_inspector_scroll")
            .show(ui, |ui| {
                for &(category, icon, theme_cat) in &categories {
                    let nodes_in_cat: Vec<_> = graph.nodes.iter()
                        .filter(|n| n.node_type.category() == category)
                        .collect();
                    if nodes_in_cat.is_empty() { continue; }

                    // Check if any of these nodes are connected to the emitter
                    let connected_to_emitter: Vec<u64> = if let Some(eid) = emitter_id {
                        graph.connections.iter()
                            .filter(|c| c.to_node == eid)
                            .map(|c| c.from_node)
                            .collect()
                    } else {
                        Vec::new()
                    };

                    for node in &nodes_in_cat {
                        let display_name = node.node_type.display_name();
                        let is_connected = node.node_type == ParticleNodeType::Emitter
                            || connected_to_emitter.contains(&node.id);
                        let is_selected = editor_state.selected_node == Some(node.id);

                        // Use node-specific icon where possible
                        let node_icon = match node.node_type {
                            ParticleNodeType::Emitter => GEAR,
                            ParticleNodeType::SpawnRate | ParticleNodeType::SpawnBurst => SPARKLE,
                            ParticleNodeType::InitLifetime => TIMER,
                            ParticleNodeType::InitPosition => SHAPES,
                            ParticleNodeType::InitVelocity => ARROWS_OUT,
                            ParticleNodeType::InitSize => RESIZE,
                            ParticleNodeType::InitColor => PALETTE,
                            ParticleNodeType::Gravity | ParticleNodeType::LinearDrag |
                            ParticleNodeType::RadialAccel | ParticleNodeType::TangentAccel => WIND,
                            ParticleNodeType::Noise => SPIRAL,
                            ParticleNodeType::Orbit => PLANET,
                            ParticleNodeType::VelocityLimit => GAUGE,
                            ParticleNodeType::ConformToSphere => ATOM,
                            ParticleNodeType::KillSphere | ParticleNodeType::KillAabb => PROHIBIT,
                            ParticleNodeType::SizeOverLifetime => RESIZE,
                            ParticleNodeType::ColorOverLifetime => PALETTE,
                            ParticleNodeType::Orient => CUBE,
                            ParticleNodeType::Texture => CUBE,
                            _ => icon,
                        };

                        // Dim label for disconnected nodes
                        let section_label = if !is_connected {
                            format!("{} (disconnected)", display_name)
                        } else {
                            display_name.to_string()
                        };

                        let id_source = format!("graph_node_{}", node.id);

                        collapsible_section(ui, node_icon, &section_label, theme_cat, theme, &id_source, is_selected, |ui| {
                            let pins = node.node_type.pins();
                            let input_pins: Vec<_> = pins.iter()
                                .filter(|p| p.direction == PinDir::Input)
                                .collect();

                            if input_pins.is_empty() { return; }

                            // Connected input pins for this node
                            let connected_pins: Vec<String> = graph.connections.iter()
                                .filter(|c| c.to_node == node.id)
                                .map(|c| c.to_pin.clone())
                                .collect();

                            let mut values = node.values.clone();
                            let mut node_modified = false;

                            for (row, pin) in input_pins.iter().enumerate() {
                                let is_pin_connected = connected_pins.contains(&pin.name);

                                if is_pin_connected {
                                    inline_property(ui, row, &pin.label, theme, |ui| {
                                        ui.colored_label(
                                            Color32::from_rgb(100, 200, 100),
                                            "(connected)",
                                        );
                                    });
                                } else {
                                    let val = values.entry(pin.name.clone())
                                        .or_insert_with(|| pin.default_value.clone());

                                    match val {
                                        PinValue::Float(v) => {
                                            inline_property(ui, row, &pin.label, theme, |ui| {
                                                if ui.add(egui::DragValue::new(v).speed(0.01)).changed() {
                                                    node_modified = true;
                                                }
                                            });
                                        }
                                        PinValue::Vec3(v) => {
                                            inline_property(ui, row, &pin.label, theme, |ui| {
                                                let muted = theme.text.muted.to_color32();
                                                for (i, lbl) in ["X", "Y", "Z"].iter().enumerate() {
                                                    ui.label(RichText::new(*lbl).size(10.0).color(muted));
                                                    if ui.add(egui::DragValue::new(&mut v[i]).speed(0.01).max_decimals(3)).changed() {
                                                        node_modified = true;
                                                    }
                                                }
                                            });
                                        }
                                        PinValue::Vec4(v) => {
                                            inline_property(ui, row, &pin.label, theme, |ui| {
                                                let mut color = [v[0], v[1], v[2], v[3]];
                                                if ui.color_edit_button_rgba_unmultiplied(&mut color).changed() {
                                                    *v = color;
                                                    node_modified = true;
                                                }
                                            });
                                        }
                                        PinValue::Bool(b) => {
                                            inline_property(ui, row, &pin.label, theme, |ui| {
                                                if ui.checkbox(b, "").changed() {
                                                    node_modified = true;
                                                }
                                            });
                                        }
                                        _ => {}
                                    }
                                }
                            }

                            if node_modified {
                                all_modified_values.push((node.id, values));
                                any_modified = true;
                            }
                        });
                    }
                }
            });

        if any_modified {
            cmds.push(move |world: &mut World| {
                let mut state = world.resource_mut::<ParticleEditorState>();
                if let Some(ref mut graph) = state.node_graph {
                    for (node_id, values) in all_modified_values {
                        if let Some(node) = graph.get_node_mut(node_id) {
                            node.values = values;
                        }
                    }
                }
                state.is_modified = true;
            });
        }
    }

    fn render_welcome_screen(&self, ui: &mut egui::Ui, cmds: &EditorCommands, theme: &Theme) {
        let available = ui.available_rect_before_wrap();
        let text_muted = theme.text.muted.to_color32();
        let accent = theme.semantic.accent.to_color32();

        ui.scope_builder(egui::UiBuilder::new().max_rect(available), |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(available.height() * 0.3);
                ui.label(RichText::new("Particle Editor").size(20.0).color(text_muted));
                ui.add_space(16.0);
                ui.label(RichText::new("Create or load a particle effect to begin").size(12.0).color(text_muted));
                ui.add_space(24.0);

                ui.horizontal(|ui| {
                    ui.add_space(available.width() * 0.3);

                    if ui.add(egui::Button::new(
                        RichText::new(format!("{} New Effect", PLUS)).color(accent),
                    )).clicked() {
                        cmds.push(move |world: &mut World| {
                            let mut state = world.resource_mut::<ParticleEditorState>();
                            state.current_effect = Some(HanabiEffectDefinition::default());
                            state.is_modified = true;
                            state.current_file_path = None;
                        });
                    }

                    ui.add_space(8.0);

                    if ui.add(egui::Button::new(
                        RichText::new(format!("{} Open", FOLDER_OPEN)).color(text_muted),
                    )).clicked() {
                        cmds.push(move |world: &mut World| {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Particle files", &["particle"])
                                .pick_file()
                            {
                                if let Some(effect) = load_effect_from_file(&path) {
                                    let mut state = world.resource_mut::<ParticleEditorState>();
                                    state.current_effect = Some(effect);
                                    state.current_file_path = Some(path.to_string_lossy().to_string());
                                    state.is_modified = false;
                                }
                            }
                        });
                    }
                });
            });
        });
    }
}

// ── File I/O ────────────────────────────────────────────────────────────────

fn save_effect_to_file(path: &PathBuf, effect: &HanabiEffectDefinition) -> bool {
    let pretty = ron::ser::PrettyConfig::new()
        .depth_limit(4)
        .separate_tuple_members(true);

    match ron::ser::to_string_pretty(effect, pretty) {
        Ok(contents) => match std::fs::write(path, contents) {
            Ok(_) => {
                bevy::log::info!("Saved particle effect to {:?}", path);
                true
            }
            Err(e) => {
                bevy::log::error!("Failed to write particle effect {:?}: {}", path, e);
                false
            }
        },
        Err(e) => {
            bevy::log::error!("Failed to serialize particle effect: {}", e);
            false
        }
    }
}

// ── Effect Editor ───────────────────────────────────────────────────────────

fn render_effect_editor(
    ui: &mut egui::Ui,
    effect: &mut HanabiEffectDefinition,
    file_path: Option<&String>,
    theme: &Theme,
) -> (bool, bool, bool) {
    let mut modified = false;
    let mut save_requested = false;
    let mut save_as_requested = false;
    let text_color = theme.text.primary.to_color32();

    // Header with save buttons
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Button::new("Save As").small()).clicked() {
                save_as_requested = true;
            }
            let save_enabled = file_path.is_some();
            if ui.add_enabled(save_enabled, egui::Button::new(format!("{} Save", FLOPPY_DISK)).small()).clicked() {
                save_requested = true;
            }
        });
    });

    ui.add_space(4.0);

    // General
    collapsible_section(ui, GEAR, "General", "effects", theme, "particle_general", true, |ui| {
        let mut row = 0;
        modified |= inline_property(ui, row, "Name", theme, |ui| {
            ui.text_edit_singleline(&mut effect.name).changed()
        });
        row += 1;
        modified |= inline_property(ui, row, "Capacity", theme, |ui| {
            ui.add(egui::DragValue::new(&mut effect.capacity).range(10..=100000)).changed()
        });
    });

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
    modified |= render_noise_section(ui, effect, theme);
    modified |= render_orbit_section(ui, effect, theme);
    modified |= render_velocity_limit_section(ui, effect, theme);
    modified |= render_variables_section(ui, effect, theme);

    (modified, save_requested, save_as_requested)
}

// ── Sections ────────────────────────────────────────────────────────────────

fn render_spawning_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    collapsible_section(ui, SPARKLE, "Spawning", "effects", theme, "particle_spawning", true, |ui| {
        let mut row = 0;
        modified |= inline_property(ui, row, "Mode", theme, |ui| {
            let mut changed = false;
            egui::ComboBox::from_id_salt("spawn_mode_editor")
                .selected_text(match effect.spawn_mode {
                    SpawnMode::Rate => "Continuous",
                    SpawnMode::Burst => "Single Burst",
                    SpawnMode::BurstRate => "Repeated Bursts",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(effect.spawn_mode == SpawnMode::Rate, "Continuous").clicked() { effect.spawn_mode = SpawnMode::Rate; changed = true; }
                    if ui.selectable_label(effect.spawn_mode == SpawnMode::Burst, "Single Burst").clicked() { effect.spawn_mode = SpawnMode::Burst; changed = true; }
                    if ui.selectable_label(effect.spawn_mode == SpawnMode::BurstRate, "Repeated Bursts").clicked() { effect.spawn_mode = SpawnMode::BurstRate; changed = true; }
                });
            changed
        });
        row += 1;

        match effect.spawn_mode {
            SpawnMode::Rate => {
                modified |= inline_property(ui, row, "Rate/sec", theme, |ui| {
                    ui.add(egui::DragValue::new(&mut effect.spawn_rate).speed(1.0).range(0.1..=10000.0)).changed()
                });
            }
            SpawnMode::Burst => {
                modified |= inline_property(ui, row, "Count", theme, |ui| {
                    ui.add(egui::DragValue::new(&mut effect.spawn_count).range(1..=10000)).changed()
                });
            }
            SpawnMode::BurstRate => {
                modified |= inline_property(ui, row, "Count", theme, |ui| {
                    ui.add(egui::DragValue::new(&mut effect.spawn_count).range(1..=10000)).changed()
                });
                row += 1;
                modified |= inline_property(ui, row, "Bursts/sec", theme, |ui| {
                    ui.add(egui::DragValue::new(&mut effect.spawn_rate).speed(0.1).range(0.1..=100.0)).changed()
                });
            }
        }
        row += 1;
        modified |= inline_property(ui, row, "Duration", theme, |ui| {
            ui.add(egui::DragValue::new(&mut effect.spawn_duration).speed(0.1).range(0.0..=600.0).suffix("s")).changed()
        });
        row += 1;
        modified |= inline_property(ui, row, "Cycles", theme, |ui| {
            ui.add(egui::DragValue::new(&mut effect.spawn_cycle_count).range(0..=1000)).changed()
        });
        row += 1;
        modified |= inline_property(ui, row, "Starts Active", theme, |ui| {
            ui.checkbox(&mut effect.spawn_starts_active, "").changed()
        });
    });
    modified
}

fn render_lifetime_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    collapsible_section(ui, TIMER, "Lifetime", "effects", theme, "particle_lifetime", true, |ui| {
        modified |= inline_property(ui, 0, "Min", theme, |ui| {
            let changed = ui.add(egui::DragValue::new(&mut effect.lifetime_min).speed(0.1).range(0.01..=60.0).suffix("s")).changed();
            if changed && effect.lifetime_min > effect.lifetime_max { effect.lifetime_max = effect.lifetime_min; }
            changed
        });
        modified |= inline_property(ui, 1, "Max", theme, |ui| {
            let changed = ui.add(egui::DragValue::new(&mut effect.lifetime_max).speed(0.1).range(0.01..=60.0).suffix("s")).changed();
            if changed && effect.lifetime_max < effect.lifetime_min { effect.lifetime_min = effect.lifetime_max; }
            changed
        });
    });
    modified
}

fn render_shape_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    collapsible_section(ui, SHAPES, "Emission Shape", "effects", theme, "particle_shape", true, |ui| {
        let shape_name = match &effect.emit_shape {
            HanabiEmitShape::Point => "Point",
            HanabiEmitShape::Circle { .. } => "Circle",
            HanabiEmitShape::Sphere { .. } => "Sphere",
            HanabiEmitShape::Cone { .. } => "Cone",
            HanabiEmitShape::Rect { .. } => "Rectangle",
            HanabiEmitShape::Box { .. } => "Box",
        };
        let mut row = 0;
        modified |= inline_property(ui, row, "Shape", theme, |ui| {
            let mut changed = false;
            egui::ComboBox::from_id_salt("emit_shape_editor")
                .selected_text(shape_name)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(matches!(&effect.emit_shape, HanabiEmitShape::Point), "Point").clicked() { effect.emit_shape = HanabiEmitShape::Point; changed = true; }
                    if ui.selectable_label(matches!(&effect.emit_shape, HanabiEmitShape::Circle { .. }), "Circle").clicked() { effect.emit_shape = HanabiEmitShape::Circle { radius: 1.0, dimension: ShapeDimension::Volume }; changed = true; }
                    if ui.selectable_label(matches!(&effect.emit_shape, HanabiEmitShape::Sphere { .. }), "Sphere").clicked() { effect.emit_shape = HanabiEmitShape::Sphere { radius: 1.0, dimension: ShapeDimension::Volume }; changed = true; }
                    if ui.selectable_label(matches!(&effect.emit_shape, HanabiEmitShape::Cone { .. }), "Cone").clicked() { effect.emit_shape = HanabiEmitShape::Cone { base_radius: 0.5, top_radius: 0.0, height: 1.0, dimension: ShapeDimension::Volume }; changed = true; }
                    if ui.selectable_label(matches!(&effect.emit_shape, HanabiEmitShape::Rect { .. }), "Rectangle").clicked() { effect.emit_shape = HanabiEmitShape::Rect { half_extents: [1.0, 1.0], dimension: ShapeDimension::Volume }; changed = true; }
                    if ui.selectable_label(matches!(&effect.emit_shape, HanabiEmitShape::Box { .. }), "Box").clicked() { effect.emit_shape = HanabiEmitShape::Box { half_extents: [1.0, 1.0, 1.0] }; changed = true; }
                });
            changed
        });
        row += 1;

        match &mut effect.emit_shape {
            HanabiEmitShape::Point => {}
            HanabiEmitShape::Circle { radius, dimension } | HanabiEmitShape::Sphere { radius, dimension } => {
                modified |= inline_property(ui, row, "Radius", theme, |ui| {
                    ui.add(egui::DragValue::new(radius).speed(0.1).range(0.001..=100.0)).changed()
                });
                row += 1;
                modified |= inline_property(ui, row, "Emit from", theme, |ui| {
                    let mut changed = false;
                    if ui.radio(*dimension == ShapeDimension::Volume, "Volume").clicked() { *dimension = ShapeDimension::Volume; changed = true; }
                    if ui.radio(*dimension == ShapeDimension::Surface, "Surface").clicked() { *dimension = ShapeDimension::Surface; changed = true; }
                    changed
                });
            }
            HanabiEmitShape::Cone { base_radius, top_radius, height, dimension } => {
                modified |= inline_property(ui, row, "Base Radius", theme, |ui| { ui.add(egui::DragValue::new(base_radius).speed(0.1).range(0.0..=100.0)).changed() });
                row += 1;
                modified |= inline_property(ui, row, "Top Radius", theme, |ui| { ui.add(egui::DragValue::new(top_radius).speed(0.1).range(0.0..=100.0)).changed() });
                row += 1;
                modified |= inline_property(ui, row, "Height", theme, |ui| { ui.add(egui::DragValue::new(height).speed(0.1).range(0.001..=100.0)).changed() });
                row += 1;
                modified |= inline_property(ui, row, "Emit from", theme, |ui| {
                    let mut changed = false;
                    if ui.radio(*dimension == ShapeDimension::Volume, "Volume").clicked() { *dimension = ShapeDimension::Volume; changed = true; }
                    if ui.radio(*dimension == ShapeDimension::Surface, "Surface").clicked() { *dimension = ShapeDimension::Surface; changed = true; }
                    changed
                });
            }
            HanabiEmitShape::Rect { half_extents, dimension } => {
                modified |= inline_property(ui, row, "Extents X", theme, |ui| { ui.add(egui::DragValue::new(&mut half_extents[0]).speed(0.1).range(0.001..=100.0)).changed() });
                row += 1;
                modified |= inline_property(ui, row, "Extents Y", theme, |ui| { ui.add(egui::DragValue::new(&mut half_extents[1]).speed(0.1).range(0.001..=100.0)).changed() });
                row += 1;
                modified |= inline_property(ui, row, "Emit from", theme, |ui| {
                    let mut changed = false;
                    if ui.radio(*dimension == ShapeDimension::Volume, "Volume").clicked() { *dimension = ShapeDimension::Volume; changed = true; }
                    if ui.radio(*dimension == ShapeDimension::Surface, "Surface").clicked() { *dimension = ShapeDimension::Surface; changed = true; }
                    changed
                });
            }
            HanabiEmitShape::Box { half_extents } => {
                modified |= inline_property(ui, row, "Extents X", theme, |ui| { ui.add(egui::DragValue::new(&mut half_extents[0]).speed(0.1).range(0.001..=100.0)).changed() });
                row += 1;
                modified |= inline_property(ui, row, "Extents Y", theme, |ui| { ui.add(egui::DragValue::new(&mut half_extents[1]).speed(0.1).range(0.001..=100.0)).changed() });
                row += 1;
                modified |= inline_property(ui, row, "Extents Z", theme, |ui| { ui.add(egui::DragValue::new(&mut half_extents[2]).speed(0.1).range(0.001..=100.0)).changed() });
            }
        }
    });
    modified
}

fn render_velocity_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    collapsible_section(ui, ARROWS_OUT, "Velocity", "effects", theme, "particle_velocity", true, |ui| {
        let mut row = 0;
        modified |= inline_property(ui, row, "Mode", theme, |ui| {
            let mut changed = false;
            egui::ComboBox::from_id_salt("velocity_mode_editor")
                .selected_text(match effect.velocity_mode {
                    VelocityMode::Directional => "Directional",
                    VelocityMode::Radial => "Radial",
                    VelocityMode::Tangent => "Tangent",
                    VelocityMode::Random => "Random",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(effect.velocity_mode == VelocityMode::Directional, "Directional").clicked() { effect.velocity_mode = VelocityMode::Directional; changed = true; }
                    if ui.selectable_label(effect.velocity_mode == VelocityMode::Radial, "Radial").clicked() { effect.velocity_mode = VelocityMode::Radial; changed = true; }
                    if ui.selectable_label(effect.velocity_mode == VelocityMode::Tangent, "Tangent").clicked() { effect.velocity_mode = VelocityMode::Tangent; changed = true; }
                    if ui.selectable_label(effect.velocity_mode == VelocityMode::Random, "Random").clicked() { effect.velocity_mode = VelocityMode::Random; changed = true; }
                });
            changed
        });
        row += 1;
        modified |= inline_property(ui, row, "Speed", theme, |ui| { ui.add(egui::DragValue::new(&mut effect.velocity_magnitude).speed(0.1).range(0.0..=100.0)).changed() });
        row += 1;
        modified |= inline_property(ui, row, "Speed Min", theme, |ui| { ui.add(egui::DragValue::new(&mut effect.velocity_speed_min).speed(0.1).range(0.0..=100.0)).changed() });
        row += 1;
        modified |= inline_property(ui, row, "Speed Max", theme, |ui| { ui.add(egui::DragValue::new(&mut effect.velocity_speed_max).speed(0.1).range(0.0..=100.0)).changed() });
        row += 1;

        if effect.velocity_mode == VelocityMode::Directional {
            modified |= inline_property(ui, row, "Spread", theme, |ui| { ui.add(egui::DragValue::new(&mut effect.velocity_spread).speed(0.05).range(0.0..=std::f32::consts::PI)).changed() });
            row += 1;
            modified |= inline_property(ui, row, "Direction", theme, |ui| {
                let mut changed = false;
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.label(RichText::new("X").size(10.0)); changed |= ui.add(egui::DragValue::new(&mut effect.velocity_direction[0]).speed(0.1)).changed();
                ui.label(RichText::new("Y").size(10.0)); changed |= ui.add(egui::DragValue::new(&mut effect.velocity_direction[1]).speed(0.1)).changed();
                ui.label(RichText::new("Z").size(10.0)); changed |= ui.add(egui::DragValue::new(&mut effect.velocity_direction[2]).speed(0.1)).changed();
                changed
            });
        }
        if effect.velocity_mode == VelocityMode::Tangent {
            modified |= inline_property(ui, row, "Axis", theme, |ui| {
                let mut changed = false;
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.label(RichText::new("X").size(10.0)); changed |= ui.add(egui::DragValue::new(&mut effect.velocity_axis[0]).speed(0.1)).changed();
                ui.label(RichText::new("Y").size(10.0)); changed |= ui.add(egui::DragValue::new(&mut effect.velocity_axis[1]).speed(0.1)).changed();
                ui.label(RichText::new("Z").size(10.0)); changed |= ui.add(egui::DragValue::new(&mut effect.velocity_axis[2]).speed(0.1)).changed();
                changed
            });
        }
    });
    modified
}

fn render_forces_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    collapsible_section(ui, WIND, "Forces", "effects", theme, "particle_forces", true, |ui| {
        modified |= inline_property(ui, 0, "Accel", theme, |ui| {
            let mut changed = false;
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.label(RichText::new("X").size(10.0)); changed |= ui.add(egui::DragValue::new(&mut effect.acceleration[0]).speed(0.1)).changed();
            ui.label(RichText::new("Y").size(10.0)); changed |= ui.add(egui::DragValue::new(&mut effect.acceleration[1]).speed(0.1)).changed();
            ui.label(RichText::new("Z").size(10.0)); changed |= ui.add(egui::DragValue::new(&mut effect.acceleration[2]).speed(0.1)).changed();
            changed
        });
        inline_property(ui, 1, "Presets", theme, |ui| {
            if ui.small_button("None").clicked() { effect.acceleration = [0.0, 0.0, 0.0]; modified = true; }
            if ui.small_button("Light").clicked() { effect.acceleration = [0.0, -2.0, 0.0]; modified = true; }
            if ui.small_button("Normal").clicked() { effect.acceleration = [0.0, -9.8, 0.0]; modified = true; }
        });
        modified |= inline_property(ui, 2, "Drag", theme, |ui| { ui.add(egui::DragValue::new(&mut effect.linear_drag).speed(0.05).range(0.0..=10.0)).changed() });
        modified |= inline_property(ui, 3, "Radial Accel", theme, |ui| { ui.add(egui::DragValue::new(&mut effect.radial_acceleration).speed(0.1).range(-100.0..=100.0)).changed() });
        modified |= inline_property(ui, 4, "Tangent Accel", theme, |ui| { ui.add(egui::DragValue::new(&mut effect.tangent_acceleration).speed(0.1).range(-100.0..=100.0)).changed() });
        if effect.tangent_acceleration.abs() > 0.001 {
            modified |= inline_property(ui, 5, "Tangent Axis", theme, |ui| {
                let mut changed = false;
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.label(RichText::new("X").size(10.0)); changed |= ui.add(egui::DragValue::new(&mut effect.tangent_accel_axis[0]).speed(0.1)).changed();
                ui.label(RichText::new("Y").size(10.0)); changed |= ui.add(egui::DragValue::new(&mut effect.tangent_accel_axis[1]).speed(0.1)).changed();
                ui.label(RichText::new("Z").size(10.0)); changed |= ui.add(egui::DragValue::new(&mut effect.tangent_accel_axis[2]).speed(0.1)).changed();
                changed
            });
        }
    });
    modified
}

fn render_size_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    collapsible_section(ui, RESIZE, "Size Over Lifetime", "effects", theme, "particle_size", true, |ui| {
        let mut row = 0;
        modified |= inline_property(ui, row, "Non-Uniform", theme, |ui| { ui.checkbox(&mut effect.size_non_uniform, "").changed() });
        row += 1;

        if effect.size_non_uniform {
            modified |= inline_property(ui, row, "Start X", theme, |ui| { ui.add(egui::DragValue::new(&mut effect.size_start_x).speed(0.01).range(0.001..=10.0)).changed() }); row += 1;
            modified |= inline_property(ui, row, "Start Y", theme, |ui| { ui.add(egui::DragValue::new(&mut effect.size_start_y).speed(0.01).range(0.001..=10.0)).changed() }); row += 1;
            modified |= inline_property(ui, row, "End X", theme, |ui| { ui.add(egui::DragValue::new(&mut effect.size_end_x).speed(0.01).range(0.0..=10.0)).changed() }); row += 1;
            modified |= inline_property(ui, row, "End Y", theme, |ui| { ui.add(egui::DragValue::new(&mut effect.size_end_y).speed(0.01).range(0.0..=10.0)).changed() });
        } else {
            modified |= inline_property(ui, row, "Start", theme, |ui| { ui.add(egui::DragValue::new(&mut effect.size_start).speed(0.01).range(0.001..=10.0)).changed() }); row += 1;
            modified |= inline_property(ui, row, "End", theme, |ui| { ui.add(egui::DragValue::new(&mut effect.size_end).speed(0.01).range(0.0..=10.0)).changed() }); row += 1;
            inline_property(ui, row, "Presets", theme, |ui| {
                if ui.small_button("Constant").clicked() { effect.size_end = effect.size_start; modified = true; }
                if ui.small_button("Shrink").clicked() { effect.size_end = 0.0; modified = true; }
                if ui.small_button("Grow").clicked() { effect.size_end = effect.size_start * 2.0; modified = true; }
            });
        }
        row += 1;
        modified |= inline_property(ui, row, "Random Min", theme, |ui| { ui.add(egui::DragValue::new(&mut effect.size_start_min).speed(0.01).range(0.0..=10.0)).changed() }); row += 1;
        modified |= inline_property(ui, row, "Random Max", theme, |ui| { ui.add(egui::DragValue::new(&mut effect.size_start_max).speed(0.01).range(0.0..=10.0)).changed() }); row += 1;
        modified |= inline_property(ui, row, "Screen Space", theme, |ui| { ui.checkbox(&mut effect.screen_space_size, "").changed() }); row += 1;
        modified |= inline_property(ui, row, "Roundness", theme, |ui| { ui.add(egui::Slider::new(&mut effect.roundness, 0.0..=1.0)).changed() });
    });
    modified
}

fn render_color_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    collapsible_section(ui, PALETTE, "Color Over Lifetime", "effects", theme, "particle_color", true, |ui| {
        let mut row = 0;
        modified |= inline_property(ui, row, "Flat Color", theme, |ui| { ui.checkbox(&mut effect.use_flat_color, "").changed() });
        row += 1;

        if effect.use_flat_color {
            modified |= inline_property(ui, row, "Color", theme, |ui| {
                let mut color = egui::Rgba::from_rgba_unmultiplied(effect.flat_color[0], effect.flat_color[1], effect.flat_color[2], effect.flat_color[3]);
                if egui::color_picker::color_edit_button_rgba(ui, &mut color, egui::color_picker::Alpha::OnlyBlend).changed() {
                    effect.flat_color = [color.r(), color.g(), color.b(), color.a()];
                    true
                } else { false }
            });
        } else {
            // Gradient preview
            let gradient_width = ui.available_width() - 8.0;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(gradient_width, 20.0), Sense::hover());
            let stops = &effect.color_gradient;
            if stops.len() >= 2 {
                for i in 0..stops.len() - 1 {
                    let start = &stops[i];
                    let end = &stops[i + 1];
                    let x0 = rect.min.x + rect.width() * start.position;
                    let x1 = rect.min.x + rect.width() * end.position;
                    let color = Color32::from_rgba_unmultiplied(
                        (start.color[0] * 255.0) as u8, (start.color[1] * 255.0) as u8,
                        (start.color[2] * 255.0) as u8, (start.color[3] * 255.0) as u8,
                    );
                    let seg = Rect::from_min_max(Pos2::new(x0, rect.min.y), Pos2::new(x1, rect.max.y));
                    ui.painter().rect_filled(seg, 0.0, color);
                }
            }
            ui.painter().rect_stroke(rect, 2.0, Stroke::new(1.0, theme.widgets.border.to_color32()), StrokeKind::Inside);
            ui.add_space(4.0);

            let mut to_remove = None;
            let len = effect.color_gradient.len();
            for (i, stop) in effect.color_gradient.iter_mut().enumerate() {
                modified |= inline_property(ui, row + i, &format!("Stop {}", i + 1), theme, |ui| {
                    let mut changed = false;
                    changed |= ui.add(egui::DragValue::new(&mut stop.position).speed(0.01).range(0.0..=1.0)).changed();
                    let mut color = egui::Rgba::from_rgba_unmultiplied(stop.color[0], stop.color[1], stop.color[2], stop.color[3]);
                    if egui::color_picker::color_edit_button_rgba(ui, &mut color, egui::color_picker::Alpha::OnlyBlend).changed() {
                        stop.color = [color.r(), color.g(), color.b(), color.a()];
                        changed = true;
                    }
                    if len > 2 && ui.small_button(format!("{}", MINUS)).clicked() { to_remove = Some(i); }
                    changed
                });
            }
            if let Some(idx) = to_remove { effect.color_gradient.remove(idx); modified = true; }
            row += len;

            inline_property(ui, row, "Actions", theme, |ui| {
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
        modified |= inline_property(ui, row, "HDR Color", theme, |ui| { ui.checkbox(&mut effect.use_hdr_color, "").changed() });
        row += 1;
        if effect.use_hdr_color {
            modified |= inline_property(ui, row, "HDR Intensity", theme, |ui| { ui.add(egui::DragValue::new(&mut effect.hdr_intensity).speed(0.1).range(1.0..=100.0)).changed() });
            row += 1;
        }
        modified |= inline_property(ui, row, "Blend Mode", theme, |ui| {
            let mut changed = false;
            egui::ComboBox::from_id_salt("color_blend_mode_editor")
                .selected_text(match effect.color_blend_mode { ParticleColorBlendMode::Modulate => "Modulate", ParticleColorBlendMode::Overwrite => "Overwrite", ParticleColorBlendMode::Add => "Add" })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(effect.color_blend_mode == ParticleColorBlendMode::Modulate, "Modulate").clicked() { effect.color_blend_mode = ParticleColorBlendMode::Modulate; changed = true; }
                    if ui.selectable_label(effect.color_blend_mode == ParticleColorBlendMode::Overwrite, "Overwrite").clicked() { effect.color_blend_mode = ParticleColorBlendMode::Overwrite; changed = true; }
                    if ui.selectable_label(effect.color_blend_mode == ParticleColorBlendMode::Add, "Add").clicked() { effect.color_blend_mode = ParticleColorBlendMode::Add; changed = true; }
                });
            changed
        });
    });
    modified
}

fn render_rendering_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    collapsible_section(ui, CUBE, "Rendering", "effects", theme, "particle_rendering", false, |ui| {
        let mut row = 0;
        modified |= inline_property(ui, row, "Alpha Mode", theme, |ui| {
            let mut changed = false;
            egui::ComboBox::from_id_salt("alpha_mode_editor")
                .selected_text(match effect.alpha_mode {
                    ParticleAlphaMode::Blend => "Blend", ParticleAlphaMode::Premultiply => "Premultiply",
                    ParticleAlphaMode::Add => "Additive", ParticleAlphaMode::Multiply => "Multiply",
                    ParticleAlphaMode::Mask => "Mask", ParticleAlphaMode::Opaque => "Opaque",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(effect.alpha_mode == ParticleAlphaMode::Blend, "Blend").clicked() { effect.alpha_mode = ParticleAlphaMode::Blend; changed = true; }
                    if ui.selectable_label(effect.alpha_mode == ParticleAlphaMode::Premultiply, "Premultiply").clicked() { effect.alpha_mode = ParticleAlphaMode::Premultiply; changed = true; }
                    if ui.selectable_label(effect.alpha_mode == ParticleAlphaMode::Add, "Additive").clicked() { effect.alpha_mode = ParticleAlphaMode::Add; changed = true; }
                    if ui.selectable_label(effect.alpha_mode == ParticleAlphaMode::Multiply, "Multiply").clicked() { effect.alpha_mode = ParticleAlphaMode::Multiply; changed = true; }
                    if ui.selectable_label(effect.alpha_mode == ParticleAlphaMode::Mask, "Mask").clicked() { effect.alpha_mode = ParticleAlphaMode::Mask; changed = true; }
                    if ui.selectable_label(effect.alpha_mode == ParticleAlphaMode::Opaque, "Opaque").clicked() { effect.alpha_mode = ParticleAlphaMode::Opaque; changed = true; }
                });
            changed
        });
        row += 1;
        if effect.alpha_mode == ParticleAlphaMode::Mask {
            modified |= inline_property(ui, row, "Mask Threshold", theme, |ui| { ui.add(egui::Slider::new(&mut effect.alpha_mask_threshold, 0.0..=1.0)).changed() });
            row += 1;
        }
        modified |= inline_property(ui, row, "Orient Mode", theme, |ui| {
            let mut changed = false;
            egui::ComboBox::from_id_salt("orient_mode_editor")
                .selected_text(match effect.orient_mode {
                    ParticleOrientMode::ParallelCameraDepthPlane => "Camera Plane",
                    ParticleOrientMode::FaceCameraPosition => "Face Camera",
                    ParticleOrientMode::AlongVelocity => "Along Velocity",
                })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(effect.orient_mode == ParticleOrientMode::ParallelCameraDepthPlane, "Camera Plane").clicked() { effect.orient_mode = ParticleOrientMode::ParallelCameraDepthPlane; changed = true; }
                    if ui.selectable_label(effect.orient_mode == ParticleOrientMode::FaceCameraPosition, "Face Camera").clicked() { effect.orient_mode = ParticleOrientMode::FaceCameraPosition; changed = true; }
                    if ui.selectable_label(effect.orient_mode == ParticleOrientMode::AlongVelocity, "Along Velocity").clicked() { effect.orient_mode = ParticleOrientMode::AlongVelocity; changed = true; }
                });
            changed
        });
        row += 1;
        modified |= inline_property(ui, row, "Rotation Speed", theme, |ui| { ui.add(egui::DragValue::new(&mut effect.rotation_speed).speed(0.1).range(-20.0..=20.0).suffix(" rad/s")).changed() });
        row += 1;
        modified |= inline_property(ui, row, "Texture", theme, |ui| {
            let mut tex = effect.texture_path.clone().unwrap_or_default();
            let changed = ui.text_edit_singleline(&mut tex).changed();
            if changed { effect.texture_path = if tex.is_empty() { None } else { Some(tex) }; }
            changed
        });
        row += 1;
        if effect.texture_path.is_some() {
            let has_flipbook = effect.flipbook.is_some();
            modified |= inline_property(ui, row, "Flipbook", theme, |ui| {
                let mut enabled = has_flipbook;
                let changed = ui.checkbox(&mut enabled, "").changed();
                if changed { effect.flipbook = if enabled { Some(FlipbookSettings::default()) } else { None }; }
                changed
            });
            row += 1;
            if let Some(ref mut fb) = effect.flipbook {
                modified |= inline_property(ui, row, "Grid Cols", theme, |ui| { ui.add(egui::DragValue::new(&mut fb.grid_columns).range(1..=16)).changed() }); row += 1;
                modified |= inline_property(ui, row, "Grid Rows", theme, |ui| { ui.add(egui::DragValue::new(&mut fb.grid_rows).range(1..=16)).changed() }); row += 1;
            }
        }
        modified |= inline_property(ui, row, "Layer", theme, |ui| { ui.add(egui::DragValue::new(&mut effect.render_layer).range(0..=31)).changed() });
    });
    modified
}

fn render_simulation_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    collapsible_section(ui, GEAR, "Simulation", "effects", theme, "particle_simulation", false, |ui| {
        modified |= inline_property(ui, 0, "Space", theme, |ui| {
            let mut changed = false;
            if ui.radio(effect.simulation_space == SimulationSpace::Local, "Local").clicked() { effect.simulation_space = SimulationSpace::Local; changed = true; }
            if ui.radio(effect.simulation_space == SimulationSpace::World, "World").clicked() { effect.simulation_space = SimulationSpace::World; changed = true; }
            changed
        });
        modified |= inline_property(ui, 1, "Update", theme, |ui| {
            let mut changed = false;
            if ui.radio(effect.simulation_condition == SimulationCondition::Always, "Always").clicked() { effect.simulation_condition = SimulationCondition::Always; changed = true; }
            if ui.radio(effect.simulation_condition == SimulationCondition::WhenVisible, "Visible").clicked() { effect.simulation_condition = SimulationCondition::WhenVisible; changed = true; }
            changed
        });
        modified |= inline_property(ui, 2, "Integration", theme, |ui| {
            let mut changed = false;
            egui::ComboBox::from_id_salt("motion_integration_editor")
                .selected_text(match effect.motion_integration { MotionIntegrationMode::PostUpdate => "Post-Update", MotionIntegrationMode::PreUpdate => "Pre-Update", MotionIntegrationMode::None => "None" })
                .show_ui(ui, |ui| {
                    if ui.selectable_label(effect.motion_integration == MotionIntegrationMode::PostUpdate, "Post-Update").clicked() { effect.motion_integration = MotionIntegrationMode::PostUpdate; changed = true; }
                    if ui.selectable_label(effect.motion_integration == MotionIntegrationMode::PreUpdate, "Pre-Update").clicked() { effect.motion_integration = MotionIntegrationMode::PreUpdate; changed = true; }
                    if ui.selectable_label(effect.motion_integration == MotionIntegrationMode::None, "None").clicked() { effect.motion_integration = MotionIntegrationMode::None; changed = true; }
                });
            changed
        });
    });
    modified
}

fn render_kill_zones_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    collapsible_section(ui, PROHIBIT, "Kill Zones", "effects", theme, "particle_kill_zones", false, |ui| {
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
                    modified |= inline_property(ui, base_row, "Center", theme, |ui| {
                        let mut c = false; ui.spacing_mut().item_spacing.x = 2.0;
                        c |= ui.add(egui::DragValue::new(&mut center[0]).speed(0.1)).changed();
                        c |= ui.add(egui::DragValue::new(&mut center[1]).speed(0.1)).changed();
                        c |= ui.add(egui::DragValue::new(&mut center[2]).speed(0.1)).changed(); c
                    });
                    modified |= inline_property(ui, base_row + 1, "Radius", theme, |ui| { ui.add(egui::DragValue::new(radius).speed(0.1).range(0.1..=100.0)).changed() });
                    modified |= inline_property(ui, base_row + 2, "Kill Inside", theme, |ui| { ui.checkbox(kill_inside, "").changed() });
                }
                KillZone::Aabb { center, half_size, kill_inside } => {
                    modified |= inline_property(ui, base_row, "Center", theme, |ui| {
                        let mut c = false; ui.spacing_mut().item_spacing.x = 2.0;
                        c |= ui.add(egui::DragValue::new(&mut center[0]).speed(0.1)).changed();
                        c |= ui.add(egui::DragValue::new(&mut center[1]).speed(0.1)).changed();
                        c |= ui.add(egui::DragValue::new(&mut center[2]).speed(0.1)).changed(); c
                    });
                    modified |= inline_property(ui, base_row + 1, "Half Size", theme, |ui| {
                        let mut c = false; ui.spacing_mut().item_spacing.x = 2.0;
                        c |= ui.add(egui::DragValue::new(&mut half_size[0]).speed(0.1).range(0.1..=100.0)).changed();
                        c |= ui.add(egui::DragValue::new(&mut half_size[1]).speed(0.1).range(0.1..=100.0)).changed();
                        c |= ui.add(egui::DragValue::new(&mut half_size[2]).speed(0.1).range(0.1..=100.0)).changed(); c
                    });
                    modified |= inline_property(ui, base_row + 2, "Kill Inside", theme, |ui| { ui.checkbox(kill_inside, "").changed() });
                }
            }
            inline_property(ui, base_row + 3, "Remove", theme, |ui| {
                if ui.small_button(format!("{} Remove", MINUS)).clicked() { to_remove = Some(i); }
            });
            ui.add_space(4.0);
        }
        if let Some(idx) = to_remove { effect.kill_zones.remove(idx); modified = true; }
        ui.horizontal(|ui| {
            if ui.small_button(format!("{} Sphere Zone", PLUS)).clicked() {
                effect.kill_zones.push(KillZone::Sphere { center: [0.0, 0.0, 0.0], radius: 5.0, kill_inside: false }); modified = true;
            }
            if ui.small_button(format!("{} AABB Zone", PLUS)).clicked() {
                effect.kill_zones.push(KillZone::Aabb { center: [0.0, 0.0, 0.0], half_size: [5.0, 5.0, 5.0], kill_inside: false }); modified = true;
            }
        });
    });
    modified
}

fn render_conform_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    collapsible_section(ui, ATOM, "Conform to Sphere", "effects", theme, "particle_conform", false, |ui| {
        let has_conform = effect.conform_to_sphere.is_some();
        modified |= inline_property(ui, 0, "Enabled", theme, |ui| {
            let mut enabled = has_conform;
            let changed = ui.checkbox(&mut enabled, "").changed();
            if changed { effect.conform_to_sphere = if enabled { Some(ConformToSphere::default()) } else { None }; }
            changed
        });
        if let Some(ref mut conform) = effect.conform_to_sphere {
            modified |= inline_property(ui, 1, "Origin", theme, |ui| {
                let mut c = false; ui.spacing_mut().item_spacing.x = 2.0;
                c |= ui.add(egui::DragValue::new(&mut conform.origin[0]).speed(0.1)).changed();
                c |= ui.add(egui::DragValue::new(&mut conform.origin[1]).speed(0.1)).changed();
                c |= ui.add(egui::DragValue::new(&mut conform.origin[2]).speed(0.1)).changed(); c
            });
            modified |= inline_property(ui, 2, "Radius", theme, |ui| { ui.add(egui::DragValue::new(&mut conform.radius).speed(0.1).range(0.1..=100.0)).changed() });
            modified |= inline_property(ui, 3, "Influence Dist", theme, |ui| { ui.add(egui::DragValue::new(&mut conform.influence_dist).speed(0.1).range(0.0..=100.0)).changed() });
            modified |= inline_property(ui, 4, "Accel", theme, |ui| { ui.add(egui::DragValue::new(&mut conform.attraction_accel).speed(0.1).range(0.0..=100.0)).changed() });
            modified |= inline_property(ui, 5, "Max Speed", theme, |ui| { ui.add(egui::DragValue::new(&mut conform.max_attraction_speed).speed(0.1).range(0.0..=100.0)).changed() });
            modified |= inline_property(ui, 6, "Shell Thick.", theme, |ui| { ui.add(egui::DragValue::new(&mut conform.shell_half_thickness).speed(0.01).range(0.0..=10.0)).changed() });
            modified |= inline_property(ui, 7, "Sticky Factor", theme, |ui| { ui.add(egui::DragValue::new(&mut conform.sticky_factor).speed(0.01).range(0.0..=10.0)).changed() });
        }
    });
    modified
}

fn render_noise_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    collapsible_section(ui, SPIRAL, "Noise Turbulence", "effects", theme, "particle_noise", false, |ui| {
        let mut row = 0;
        modified |= inline_property(ui, row, "Frequency", theme, |ui| {
            ui.add(egui::DragValue::new(&mut effect.noise_frequency).speed(0.01).range(0.0..=50.0)).changed()
        });
        row += 1;
        modified |= inline_property(ui, row, "Amplitude", theme, |ui| {
            ui.add(egui::DragValue::new(&mut effect.noise_amplitude).speed(0.01).range(0.0..=50.0)).changed()
        });
        row += 1;
        modified |= inline_property(ui, row, "Octaves", theme, |ui| {
            let mut val = effect.noise_octaves as i32;
            let changed = ui.add(egui::DragValue::new(&mut val).range(1..=8)).changed();
            if changed { effect.noise_octaves = val as u32; }
            changed
        });
        row += 1;
        modified |= inline_property(ui, row, "Lacunarity", theme, |ui| {
            ui.add(egui::DragValue::new(&mut effect.noise_lacunarity).speed(0.01).range(1.0..=4.0)).changed()
        });
    });
    modified
}

fn render_orbit_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    collapsible_section(ui, PLANET, "Orbit", "effects", theme, "particle_orbit", false, |ui| {
        let has_orbit = effect.orbit.is_some();
        modified |= inline_property(ui, 0, "Enabled", theme, |ui| {
            let mut enabled = has_orbit;
            let changed = ui.checkbox(&mut enabled, "").changed();
            if changed { effect.orbit = if enabled { Some(OrbitSettings::default()) } else { None }; }
            changed
        });
        if let Some(ref mut orbit) = effect.orbit {
            modified |= inline_property(ui, 1, "Center", theme, |ui| {
                let mut c = false; ui.spacing_mut().item_spacing.x = 2.0;
                c |= ui.add(egui::DragValue::new(&mut orbit.center[0]).speed(0.1)).changed();
                c |= ui.add(egui::DragValue::new(&mut orbit.center[1]).speed(0.1)).changed();
                c |= ui.add(egui::DragValue::new(&mut orbit.center[2]).speed(0.1)).changed(); c
            });
            modified |= inline_property(ui, 2, "Axis", theme, |ui| {
                let mut c = false; ui.spacing_mut().item_spacing.x = 2.0;
                c |= ui.add(egui::DragValue::new(&mut orbit.axis[0]).speed(0.1)).changed();
                c |= ui.add(egui::DragValue::new(&mut orbit.axis[1]).speed(0.1)).changed();
                c |= ui.add(egui::DragValue::new(&mut orbit.axis[2]).speed(0.1)).changed(); c
            });
            modified |= inline_property(ui, 3, "Speed", theme, |ui| {
                ui.add(egui::DragValue::new(&mut orbit.speed).speed(0.01).range(-20.0..=20.0)).changed()
            });
            modified |= inline_property(ui, 4, "Radial Pull", theme, |ui| {
                ui.add(egui::DragValue::new(&mut orbit.radial_pull).speed(0.01).range(0.0..=20.0)).changed()
            });
            modified |= inline_property(ui, 5, "Orbit Radius", theme, |ui| {
                ui.add(egui::DragValue::new(&mut orbit.orbit_radius).speed(0.1).range(0.1..=100.0)).changed()
            });
        }
    });
    modified
}

fn render_velocity_limit_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    collapsible_section(ui, GAUGE, "Velocity Limit", "effects", theme, "particle_vel_limit", false, |ui| {
        modified |= inline_property(ui, 0, "Max Speed", theme, |ui| {
            ui.add(egui::DragValue::new(&mut effect.velocity_limit).speed(0.1).range(0.0..=1000.0)).changed()
        });
        ui.label(egui::RichText::new("0 = no limit").size(10.0).color(theme.text.muted.to_color32()));
    });
    modified
}

fn render_variables_section(ui: &mut egui::Ui, effect: &mut HanabiEffectDefinition, theme: &Theme) -> bool {
    let mut modified = false;
    collapsible_section(ui, CODE, "Variables", "effects", theme, "particle_variables", false, |ui| {
        ui.label(egui::RichText::new("Exposed to scripts").size(10.0).color(theme.text.muted.to_color32()));
        let var_names: Vec<String> = effect.variables.keys().cloned().collect();
        let mut to_remove = None;
        for (i, name) in var_names.iter().enumerate() {
            if let Some(var) = effect.variables.get_mut(name) {
                modified |= inline_property(ui, i, name, theme, |ui| {
                    let mut changed = false;
                    match var {
                        EffectVariable::Float { value, min, max } => { changed |= ui.add(egui::DragValue::new(value).range(*min..=*max)).changed(); }
                        EffectVariable::Color { value } => {
                            let mut color = egui::Rgba::from_rgba_unmultiplied(value[0], value[1], value[2], value[3]);
                            if egui::color_picker::color_edit_button_rgba(ui, &mut color, egui::color_picker::Alpha::OnlyBlend).changed() {
                                *value = [color.r(), color.g(), color.b(), color.a()]; changed = true;
                            }
                        }
                        EffectVariable::Vec3 { value } => {
                            changed |= ui.add(egui::DragValue::new(&mut value[0]).speed(0.1)).changed();
                            changed |= ui.add(egui::DragValue::new(&mut value[1]).speed(0.1)).changed();
                            changed |= ui.add(egui::DragValue::new(&mut value[2]).speed(0.1)).changed();
                        }
                    }
                    if ui.small_button(format!("{}", MINUS)).clicked() { to_remove = Some(name.clone()); }
                    changed
                });
            }
        }
        if let Some(name) = to_remove { effect.variables.remove(&name); modified = true; }
        inline_property(ui, var_names.len(), "Add", theme, |ui| {
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
