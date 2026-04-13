//! Terrain Tools panel — Sculpt & Paint tabs with tool grids, layer management, brush config.

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CursorIcon, RichText, Rounding, Vec2};
use egui_phosphor::regular::{
    MOUNTAINS, WAVES, EQUALS, ARROW_FAT_LINE_UP, TREE, DROP, WAVEFORM,
    STAIRS, ARROWS_IN_CARDINAL, ACTIVITY, GRAPH, CHART_BAR,
    CIRCLE, SQUARE, DIAMOND, CARET_DOWN, CARET_RIGHT,
    ARROWS_OUT_CARDINAL, ERASER, PAINT_BRUSH, PALETTE, PLUS, TRASH,
};

use renzora_terrain::data::*;
use renzora_terrain::paint::*;
#[allow(unused_imports)]
use renzora_terrain::splatmap_material::LayerAnimationType;
use renzora_theme::{Theme, ThemeManager};
use renzora_editor_framework::EditorCommands;
use renzora_editor_framework::EditorPanel;
use renzora_editor_framework::{asset_drop_target, AssetDragPayload};
use renzora::core::CurrentProject;

// ── Panel ────────────────────────────────────────────────────────────────────

struct PanelState {
    section_tool_settings: bool,
    section_brush_settings: bool,
    section_layers: bool,
    section_paint_settings: bool,
    section_heightmap: bool,
    section_foliage: bool,
}

impl Default for PanelState {
    fn default() -> Self {
        Self {
            section_tool_settings: true,
            section_brush_settings: true,
            section_layers: true,
            section_paint_settings: true,
            section_heightmap: false,
            section_foliage: false,
        }
    }
}

pub struct TerrainToolsPanel {
    state: RwLock<PanelState>,
}

impl TerrainToolsPanel {
    pub fn new() -> Self {
        Self {
            state: RwLock::new(PanelState::default()),
        }
    }
}

impl EditorPanel for TerrainToolsPanel {
    fn id(&self) -> &str {
        "terrain_tools"
    }

    fn title(&self) -> &str {
        "Terrain Tools"
    }

    fn icon(&self) -> Option<&str> {
        Some(MOUNTAINS)
    }

    fn default_location(&self) -> renzora_editor_framework::PanelLocation {
        renzora_editor_framework::PanelLocation::Left
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };
        let theme = &theme;
        let Some(cmds) = world.get_resource::<EditorCommands>() else { return };
        let Some(tool_state) = world.get_resource::<TerrainToolState>() else { return };
        let Some(settings) = world.get_resource::<TerrainSettings>() else { return };

        let accent = theme.semantic.accent.to_color32();
        let text_primary = theme.text.primary.to_color32();
        let text_secondary = theme.text.secondary.to_color32();

        let mut state = self.state.write().unwrap();

        // Activate / deactivate toggle
        let active = tool_state.active;
        let toggle_label = if active { "Terrain Mode Active" } else { "Enable Terrain Mode" };
        let toggle_bg = if active { accent } else { theme.widgets.inactive_bg.to_color32() };
        let toggle_fg = if active { Color32::WHITE } else { text_primary };

        if ui
            .add(
                egui::Button::new(
                    RichText::new(format!("{MOUNTAINS}  {toggle_label}"))
                        .size(13.0)
                        .color(toggle_fg),
                )
                .fill(toggle_bg)
                .min_size(Vec2::new(ui.available_width(), 32.0)),
            )
            .clicked()
        {
            let new_active = !active;
            cmds.push(move |world: &mut World| {
                if let Some(mut ts) = world.get_resource_mut::<TerrainToolState>() {
                    ts.active = new_active;
                }
            });
        }

        if !active {
            ui.add_space(8.0);
            ui.label(
                RichText::new("Select a terrain entity and enable terrain mode to begin editing.")
                    .color(text_secondary)
                    .size(11.0),
            );
            return;
        }

        ui.add_space(8.0);

        // ── Tab bar: Sculpt / Paint ──────────────────────────────────────────
        let current_tab = settings.tab;
        ui.horizontal(|ui| {
            let half_w = (ui.available_width() - 4.0) / 2.0;

            if tab_button(ui, MOUNTAINS, "Sculpt", current_tab == TerrainTab::Sculpt, half_w, accent, theme).clicked() {
                cmds.push(|world: &mut World| {
                    if let Some(mut s) = world.get_resource_mut::<TerrainSettings>() {
                        s.tab = TerrainTab::Sculpt;
                    }
                });
            }

            if tab_button(ui, PAINT_BRUSH, "Paint", current_tab == TerrainTab::Paint, half_w, accent, theme).clicked() {
                cmds.push(|world: &mut World| {
                    if let Some(mut s) = world.get_resource_mut::<TerrainSettings>() {
                        s.tab = TerrainTab::Paint;
                    }
                });
            }
        });

        ui.add_space(8.0);

        match current_tab {
            TerrainTab::Sculpt => render_sculpt_tab(ui, settings, &mut state, accent, text_primary, text_secondary, theme, cmds),
            TerrainTab::Paint => render_paint_tab(ui, world, &mut state, accent, text_primary, text_secondary, theme, cmds),
        }
    }
}

// ── Sculpt Tab ──────────────────────────────────────────────────────────────

fn render_sculpt_tab(
    ui: &mut egui::Ui,
    settings: &TerrainSettings,
    state: &mut PanelState,
    accent: Color32,
    text_primary: Color32,
    text_secondary: Color32,
    theme: &Theme,
    cmds: &EditorCommands,
) {
    // ── Tool Grid ────────────────────────────────────────────────────────
    let tools: &[(TerrainBrushType, &str, &str)] = &[
        (TerrainBrushType::Sculpt,    MOUNTAINS,          "Sculpt"),
        (TerrainBrushType::Smooth,    WAVES,              "Smooth"),
        (TerrainBrushType::Flatten,   EQUALS,             "Flatten"),
        (TerrainBrushType::Ramp,      ARROW_FAT_LINE_UP,  "Ramp"),
        (TerrainBrushType::Erosion,   TREE,               "Erosion"),
        (TerrainBrushType::Hydro,     DROP,               "Hydro"),
        (TerrainBrushType::Noise,     WAVEFORM,           "Noise"),
        (TerrainBrushType::Terrace,   STAIRS,             "Terrace"),
        (TerrainBrushType::Pinch,     ARROWS_IN_CARDINAL, "Pinch"),
        (TerrainBrushType::Relax,     ACTIVITY,           "Relax"),
        (TerrainBrushType::Retop,     GRAPH,              "Retop"),
        (TerrainBrushType::Cliff,     CHART_BAR,          "Cliff"),
        (TerrainBrushType::Raise,     ARROWS_OUT_CARDINAL,"Raise"),
        (TerrainBrushType::Lower,     ARROWS_OUT_CARDINAL,"Lower"),
        (TerrainBrushType::SetHeight, EQUALS,             "Set H"),
        (TerrainBrushType::Erase,     ERASER,             "Erase"),
    ];

    let current_brush = settings.brush_type;
    render_tool_grid(ui, tools, current_brush, accent, theme, cmds);

    ui.add_space(8.0);

    egui::ScrollArea::vertical().show(ui, |ui| {
        // ── Tool Settings ────────────────────────────────────────────
        if collapsible_header(ui, "Tool Settings", &mut state.section_tool_settings, theme) {
            ui.add_space(4.0);
            ui.indent("tool_settings", |ui| {
                render_tool_settings(ui, settings, text_secondary, text_primary, accent, theme, cmds);
            });
            ui.add_space(4.0);
        }

        // ── Brush Settings ───────────────────────────────────────────
        if collapsible_header(ui, "Brush Settings", &mut state.section_brush_settings, theme) {
            ui.add_space(4.0);
            ui.indent("brush_settings", |ui| {
                render_brush_settings(ui, settings, text_secondary, theme, cmds);
            });
            ui.add_space(4.0);
        }

        // ── Heightmap Import ────────────────────────────────────────
        if collapsible_header(ui, "Heightmap Import", &mut state.section_heightmap, theme) {
            ui.add_space(4.0);
            ui.indent("heightmap_import", |ui| {
                ui.label(RichText::new("Import a heightmap PNG or RAW16 file.").color(text_secondary).size(11.0));
                ui.add_space(4.0);
                if ui.add(egui::Button::new(
                    RichText::new(format!("{PLUS}  Import Heightmap...")).size(11.0).color(text_primary),
                ).min_size(Vec2::new(ui.available_width(), 24.0))).clicked() {
                    cmds.push(move |world: &mut World| {
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                        // Open file dialog via rfd
                        let path = rfd::FileDialog::new()
                            .add_filter("Heightmap", &["png", "r16", "raw"])
                            .pick_file();
                        if let Some(path) = path {
                            let settings = renzora_terrain::heightmap_import::HeightmapImportSettings::default();
                            // Get terrain data
                            let mut terrain_query = world.query::<&renzora_terrain::data::TerrainData>();
                            let terrain_data = terrain_query.iter(world).next().cloned();
                            if let Some(terrain_data) = terrain_data {
                                match renzora_terrain::heightmap_import::import_heightmap(&path, &settings, &terrain_data) {
                                    Ok(imported) => {
                                        let mut chunk_query = world.query::<&mut renzora_terrain::data::TerrainChunkData>();
                                        for mut chunk in chunk_query.iter_mut(world) {
                                            if let Some((_, _, heights)) = imported.iter().find(|(cx, cz, _)| *cx == chunk.chunk_x && *cz == chunk.chunk_z) {
                                                chunk.heights = heights.clone();
                                                chunk.dirty = true;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        bevy::log::error!("Heightmap import failed: {e}");
                                    }
                                }
                            }
                        }
                        }
                    });
                }
                ui.add_space(4.0);
                if ui.add(egui::Button::new(
                    RichText::new("Export Heightmap...").size(11.0).color(text_secondary),
                ).min_size(Vec2::new(ui.available_width(), 22.0))).clicked() {
                    cmds.push(move |world: &mut World| {
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                        let path = rfd::FileDialog::new()
                            .add_filter("PNG", &["png"])
                            .save_file();
                        if let Some(path) = path {
                            let mut terrain_query = world.query::<&renzora_terrain::data::TerrainData>();
                            let terrain_data = terrain_query.iter(world).next().cloned();
                            let mut chunk_query = world.query::<&renzora_terrain::data::TerrainChunkData>();
                            if let Some(terrain_data) = terrain_data {
                                let chunks: Vec<_> = chunk_query.iter(world).collect();
                                let chunk_refs: Vec<&renzora_terrain::data::TerrainChunkData> = chunks.iter().map(|c| *c).collect();
                                match renzora_terrain::heightmap_import::export_heightmap_png16(&terrain_data, &chunk_refs) {
                                    Ok(data) => {
                                        if let Err(e) = std::fs::write(&path, &data) {
                                            bevy::log::error!("Failed to write heightmap: {e}");
                                        }
                                    }
                                    Err(e) => {
                                        bevy::log::error!("Heightmap export failed: {e}");
                                    }
                                }
                            }
                        }
                        }
                    });
                }
            });
            ui.add_space(4.0);
        }
    });
}

// ── Paint Tab ───────────────────────────────────────────────────────────────

fn render_paint_tab(
    ui: &mut egui::Ui,
    world: &World,
    state: &mut PanelState,
    accent: Color32,
    text_primary: Color32,
    text_secondary: Color32,
    theme: &Theme,
    cmds: &EditorCommands,
) {
    let paint_settings = world.get_resource::<SurfacePaintSettings>();
    let paint_state = world.get_resource::<SurfacePaintState>();

    // ── Paint Brush Tools ────────────────────────────────────────────────
    if let Some(ps) = paint_settings {
        let tools: &[(PaintBrushType, &str, &str)] = &[
            (PaintBrushType::Paint,  PAINT_BRUSH, "Paint"),
            (PaintBrushType::Erase,  ERASER,      "Erase"),
            (PaintBrushType::Smooth, WAVES,       "Smooth"),
            (PaintBrushType::Fill,   PALETTE,     "Fill"),
        ];

        let current = ps.brush_type;
        render_paint_tool_grid(ui, tools, current, accent, theme, cmds);
    }

    ui.add_space(8.0);

    egui::ScrollArea::vertical().show(ui, |ui| {
        // ── Layers ──────────────────────────────────────────────────────
        if collapsible_header(ui, "Layers", &mut state.section_layers, theme) {
            ui.add_space(4.0);
            ui.indent("paint_layers", |ui| {
                render_layer_list(ui, world, accent, text_primary, text_secondary, theme, cmds);
            });
            ui.add_space(4.0);
        }

        // ── Paint Brush Settings ────────────────────────────────────────
        if collapsible_header(ui, "Brush Settings", &mut state.section_paint_settings, theme) {
            ui.add_space(4.0);
            ui.indent("paint_brush_settings", |ui| {
                if let Some(ps) = paint_settings {
                    render_paint_brush_settings(ui, ps, text_secondary, theme, cmds);
                }
            });
            ui.add_space(4.0);
        }

        // ── Foliage ─────────────────────────────────────────────────────
        if collapsible_header(ui, "Foliage", &mut state.section_foliage, theme) {
            ui.add_space(4.0);
            ui.indent("foliage_settings", |ui| {
                ui.label(RichText::new("Auto-scatter foliage based on layer weights.").color(text_secondary).size(11.0));
                ui.add_space(4.0);
                ui.label(RichText::new("Configure foliage per-layer via TerrainFoliageConfig component.").color(text_secondary).size(10.0));
            });
            ui.add_space(4.0);
        }
    });
}

fn render_layer_list(
    ui: &mut egui::Ui,
    world: &World,
    accent: Color32,
    text_primary: Color32,
    text_secondary: Color32,
    theme: &Theme,
    cmds: &EditorCommands,
) {
    let Some(paint_settings) = world.get_resource::<SurfacePaintSettings>() else { return };
    let Some(paint_state) = world.get_resource::<SurfacePaintState>() else { return };

    let active_layer = paint_settings.active_layer;

    // Try to get layer info from an actual PaintableSurfaceData component
    // Fall back to cached preview
    let layer_count = paint_state.layer_count.max(2);

    for i in 0..layer_count.min(MAX_LAYERS) {
        let is_active = i == active_layer;
        let bg = if is_active { accent } else { theme.widgets.inactive_bg.to_color32() };
        let fg = if is_active { Color32::WHITE } else { text_primary };

        let name = if i < paint_state.layers_preview.len() {
            paint_state.layers_preview[i].name.clone()
        } else {
            match i {
                0 => "Grass".to_string(),
                1 => "Dirt".to_string(),
                2 => "Water".to_string(),
                3 => "Rock".to_string(),
                _ => format!("Layer {}", i + 1),
            }
        };

        ui.horizontal(|ui| {
            let resp = ui.add(
                egui::Button::new(
                    RichText::new(format!("{}  {}", i + 1, name))
                        .size(12.0)
                        .color(fg),
                )
                .fill(bg)
                .min_size(Vec2::new(ui.available_width() - 30.0, 26.0)),
            );
            if resp.clicked() {
                let idx = i;
                cmds.push(move |world: &mut World| {
                    if let Some(mut s) = world.get_resource_mut::<SurfacePaintSettings>() {
                        s.active_layer = idx;
                    }
                });
            }
        });

        // Material drop target for the active layer
        if is_active {
            let drag_payload = world.get_resource::<AssetDragPayload>();
            let material_source = if i < paint_state.layers_preview.len() {
                paint_state.layers_preview[i].material_source.as_deref()
            } else {
                None
            };
            let drop_result = asset_drop_target(
                ui,
                egui::Id::new(("terrain_layer_mat", i)),
                material_source,
                &["material"],
                "Drop .material file",
                theme,
                drag_payload,
            );
            if let Some(ref dropped) = drop_result.dropped_path {
                let path = if let Some(project) = world.get_resource::<CurrentProject>() {
                    project.make_asset_relative(dropped)
                } else {
                    dropped.to_string_lossy().to_string()
                };
                let layer = i;
                cmds.push(move |world: &mut World| {
                    if let Some(mut ps) = world.get_resource_mut::<SurfacePaintState>() {
                        ps.pending_commands.push(SurfacePaintCommand::AssignMaterial { layer, path });
                    }
                });
            }
            if drop_result.cleared {
                let layer = i;
                cmds.push(move |world: &mut World| {
                    if let Some(mut ps) = world.get_resource_mut::<SurfacePaintState>() {
                        ps.pending_commands.push(SurfacePaintCommand::ClearMaterial(layer));
                    }
                });
            }
        }

        ui.add_space(2.0);
    }

    // Add layer button
    if layer_count < MAX_LAYERS {
        if ui
            .add(
                egui::Button::new(
                    RichText::new(format!("{PLUS}  Add Layer"))
                        .size(11.0)
                        .color(text_secondary),
                )
                .min_size(Vec2::new(ui.available_width(), 22.0)),
            )
            .clicked()
        {
            cmds.push(|world: &mut World| {
                if let Some(mut ps) = world.get_resource_mut::<SurfacePaintState>() {
                    ps.pending_commands.push(SurfacePaintCommand::AddLayer);
                }
            });
        }
    }
}

fn render_paint_brush_settings(
    ui: &mut egui::Ui,
    settings: &SurfacePaintSettings,
    text_secondary: Color32,
    theme: &Theme,
    cmds: &EditorCommands,
) {
    let label_width = 100.0;

    let mut radius = settings.brush_radius;
    let mut strength = settings.brush_strength;
    let mut falloff = settings.brush_falloff;

    labeled_slider(ui, "Size", label_width, text_secondary, &mut radius, 0.01..=0.5);
    labeled_slider(ui, "Strength", label_width, text_secondary, &mut strength, 0.01..=1.0);
    labeled_slider(ui, "Falloff", label_width, text_secondary, &mut falloff, 0.0..=1.0);

    if radius != settings.brush_radius
        || strength != settings.brush_strength
        || falloff != settings.brush_falloff
    {
        cmds.push(move |world: &mut World| {
            if let Some(mut s) = world.get_resource_mut::<SurfacePaintSettings>() {
                s.brush_radius = radius;
                s.brush_strength = strength;
                s.brush_falloff = falloff;
            }
        });
    }

    // Brush shape
    let mut shape = settings.brush_shape;
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(label_width, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| { ui.label(RichText::new("Shape").color(text_secondary).size(11.0)); },
        );
        for (s, icon) in [(BrushShape::Circle, CIRCLE), (BrushShape::Square, SQUARE), (BrushShape::Diamond, DIAMOND)] {
            if small_toggle_button(ui, icon, shape == s, theme).clicked() {
                shape = s;
            }
        }
    });
    if shape != settings.brush_shape {
        cmds.push(move |world: &mut World| {
            if let Some(mut s) = world.get_resource_mut::<SurfacePaintSettings>() {
                s.brush_shape = shape;
            }
        });
    }
}

fn render_paint_tool_grid(
    ui: &mut egui::Ui,
    tools: &[(PaintBrushType, &str, &str)],
    selected: PaintBrushType,
    accent: Color32,
    theme: &Theme,
    cmds: &EditorCommands,
) {
    let available_width = ui.available_width();
    let cols = 4usize;
    let spacing = 4.0;
    let button_size = ((available_width - spacing * (cols as f32 - 1.0)) / cols as f32).max(40.0);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = spacing;
        for (brush_type, icon, label) in tools {
            let active = selected == *brush_type;
            if grid_tool_button(ui, icon, label, active, button_size, accent, theme).clicked() {
                let bt = *brush_type;
                cmds.push(move |world: &mut World| {
                    if let Some(mut s) = world.get_resource_mut::<SurfacePaintSettings>() {
                        s.brush_type = bt;
                    }
                });
            }
        }
    });
}

// ── Tool Grid ────────────────────────────────────────────────────────────────

fn render_tool_grid(
    ui: &mut egui::Ui,
    tools: &[(TerrainBrushType, &str, &str)],
    selected: TerrainBrushType,
    accent: Color32,
    theme: &Theme,
    cmds: &EditorCommands,
) {
    let available_width = ui.available_width();
    let cols = 4usize;
    let spacing = 4.0;
    let button_size = ((available_width - spacing * (cols as f32 - 1.0)) / cols as f32).max(40.0);

    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = spacing;
        let rows = (tools.len() + cols - 1) / cols;
        for row in 0..rows {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = spacing;
                for col in 0..cols {
                    let idx = row * cols + col;
                    if idx < tools.len() {
                        let (brush_type, icon, label) = &tools[idx];
                        let active = selected == *brush_type;
                        if grid_tool_button(ui, icon, label, active, button_size, accent, theme).clicked() {
                            let bt = *brush_type;
                            cmds.push(move |world: &mut World| {
                                if let Some(mut s) = world.get_resource_mut::<TerrainSettings>() {
                                    s.brush_type = bt;
                                }
                            });
                        }
                    }
                }
            });
        }
    });
}

// ── Tool Settings ────────────────────────────────────────────────────────────

fn render_tool_settings(
    ui: &mut egui::Ui,
    settings: &TerrainSettings,
    text_secondary: Color32,
    text_primary: Color32,
    _accent: Color32,
    theme: &Theme,
    cmds: &EditorCommands,
) {
    let label_width = 100.0;
    let mut strength = settings.brush_strength;

    // Strength
    labeled_slider(ui, "Strength", label_width, text_secondary, &mut strength, 0.01..=1.0);
    if strength != settings.brush_strength {
        cmds.push(move |world: &mut World| {
            if let Some(mut s) = world.get_resource_mut::<TerrainSettings>() {
                s.brush_strength = strength;
            }
        });
    }

    // Flatten-specific
    if settings.brush_type == TerrainBrushType::Flatten {
        let mut flatten_mode = settings.flatten_mode;
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(label_width, 20.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| { ui.label(RichText::new("Mode").color(text_secondary).size(11.0)); },
            );
            egui::ComboBox::from_id_salt("flatten_mode")
                .selected_text(match flatten_mode {
                    FlattenMode::Both => "Both",
                    FlattenMode::Raise => "Raise",
                    FlattenMode::Lower => "Lower",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut flatten_mode, FlattenMode::Both, "Both");
                    ui.selectable_value(&mut flatten_mode, FlattenMode::Raise, "Raise");
                    ui.selectable_value(&mut flatten_mode, FlattenMode::Lower, "Lower");
                });
        });
        if flatten_mode != settings.flatten_mode {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_resource_mut::<TerrainSettings>() {
                    s.flatten_mode = flatten_mode;
                }
            });
        }

        let mut target_h = settings.target_height;
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(label_width, 20.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| { ui.label(RichText::new("Target Height").color(text_secondary).size(11.0)); },
            );
            ui.add(egui::DragValue::new(&mut target_h).speed(0.005).range(0.0..=1.0));
        });
        if target_h != settings.target_height {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_resource_mut::<TerrainSettings>() {
                    s.target_height = target_h;
                }
            });
        }
    }

    // Noise-specific
    if settings.brush_type == TerrainBrushType::Noise {
        ui.add_space(4.0);
        ui.label(RichText::new("Noise").color(text_primary).size(11.0));

        let mut scale = settings.noise_scale;
        let mut octaves = settings.noise_octaves;
        let mut lac = settings.noise_lacunarity;
        let mut pers = settings.noise_persistence;
        let mut seed = settings.noise_seed;
        let mut noise_mode = settings.noise_mode;
        let mut warp = settings.warp_strength;

        // Noise mode dropdown
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(label_width, 20.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| { ui.label(RichText::new("Mode").color(text_secondary).size(11.0)); },
            );
            egui::ComboBox::from_id_salt("noise_mode")
                .selected_text(noise_mode.display_name())
                .show_ui(ui, |ui| {
                    for mode in NoiseMode::all() {
                        ui.selectable_value(&mut noise_mode, *mode, mode.display_name());
                    }
                });
        });

        labeled_drag(ui, "Scale", label_width, text_secondary, &mut scale, 0.5, 1.0..=500.0);
        labeled_drag(ui, "Octaves", label_width, text_secondary, &mut octaves, 0.1, 1..=8);
        labeled_drag(ui, "Lacunarity", label_width, text_secondary, &mut lac, 0.05, 1.0..=4.0);
        labeled_drag(ui, "Persistence", label_width, text_secondary, &mut pers, 0.01, 0.1..=0.9);
        labeled_drag(ui, "Seed", label_width, text_secondary, &mut seed, 1.0, 0..=u32::MAX);

        if noise_mode == NoiseMode::Warped {
            labeled_slider(ui, "Warp", label_width, text_secondary, &mut warp, 0.0..=5.0);
        }

        if scale != settings.noise_scale
            || octaves != settings.noise_octaves
            || lac != settings.noise_lacunarity
            || pers != settings.noise_persistence
            || seed != settings.noise_seed
            || noise_mode != settings.noise_mode
            || warp != settings.warp_strength
        {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_resource_mut::<TerrainSettings>() {
                    s.noise_scale = scale;
                    s.noise_octaves = octaves;
                    s.noise_lacunarity = lac;
                    s.noise_persistence = pers;
                    s.noise_seed = seed;
                    s.noise_mode = noise_mode;
                    s.warp_strength = warp;
                }
            });
        }
    }

    // Terrace-specific
    if settings.brush_type == TerrainBrushType::Terrace {
        ui.add_space(4.0);
        ui.label(RichText::new("Terrace").color(text_primary).size(11.0));

        let mut steps = settings.terrace_steps;
        let mut sharpness = settings.terrace_sharpness;

        labeled_drag(ui, "Steps", label_width, text_secondary, &mut steps, 0.1, 2..=32);
        labeled_slider(ui, "Sharpness", label_width, text_secondary, &mut sharpness, 0.0..=1.0);

        if steps != settings.terrace_steps || sharpness != settings.terrace_sharpness {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_resource_mut::<TerrainSettings>() {
                    s.terrace_steps = steps;
                    s.terrace_sharpness = sharpness;
                }
            });
        }
    }
}

// ── Brush Settings ───────────────────────────────────────────────────────────

fn render_brush_settings(
    ui: &mut egui::Ui,
    settings: &TerrainSettings,
    text_secondary: Color32,
    theme: &Theme,
    cmds: &EditorCommands,
) {
    let label_width = 100.0;

    let mut radius = settings.brush_radius;
    let mut falloff = settings.falloff;
    let mut shape = settings.brush_shape;
    let mut falloff_type = settings.falloff_type;

    labeled_slider(ui, "Size", label_width, text_secondary, &mut radius, 1.0..=200.0);
    labeled_slider(ui, "Falloff", label_width, text_secondary, &mut falloff, 0.0..=1.0);

    // Shape buttons
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(label_width, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| { ui.label(RichText::new("Shape").color(text_secondary).size(11.0)); },
        );
        for (s, icon) in [(BrushShape::Circle, CIRCLE), (BrushShape::Square, SQUARE), (BrushShape::Diamond, DIAMOND)] {
            if small_toggle_button(ui, icon, shape == s, theme).clicked() {
                shape = s;
            }
        }
    });

    // Falloff type buttons
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(label_width, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| { ui.label(RichText::new("Falloff Type").color(text_secondary).size(11.0)); },
        );
        for (ft, label) in [
            (BrushFalloffType::Smooth, "S"),
            (BrushFalloffType::Linear, "L"),
            (BrushFalloffType::Spherical, "O"),
            (BrushFalloffType::Tip, "T"),
            (BrushFalloffType::Flat, "F"),
        ] {
            if small_toggle_button(ui, label, falloff_type == ft, theme).clicked() {
                falloff_type = ft;
            }
        }
    });

    // Commit changes
    if radius != settings.brush_radius
        || falloff != settings.falloff
        || shape != settings.brush_shape
        || falloff_type != settings.falloff_type
    {
        cmds.push(move |world: &mut World| {
            if let Some(mut s) = world.get_resource_mut::<TerrainSettings>() {
                s.brush_radius = radius;
                s.falloff = falloff;
                s.brush_shape = shape;
                s.falloff_type = falloff_type;
            }
        });
    }
}

// ── Tab Button ───────────────────────────────────────────────────────────────

fn tab_button(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    active: bool,
    width: f32,
    accent: Color32,
    theme: &Theme,
) -> egui::Response {
    let height = 30.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    if ui.is_rect_visible(rect) {
        let bg = if active {
            accent
        } else if response.hovered() {
            theme.surfaces.popup.to_color32()
        } else {
            theme.widgets.inactive_bg.to_color32()
        };
        let fg = if active { Color32::WHITE } else { theme.text.primary.to_color32() };

        ui.painter().rect_filled(rect, Rounding::same(4), bg);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("{icon}  {label}"),
            egui::FontId::proportional(12.0),
            fg,
        );
    }

    response
}

// ── UI Helpers ───────────────────────────────────────────────────────────────

fn labeled_slider(
    ui: &mut egui::Ui,
    label: &str,
    label_width: f32,
    text_color: Color32,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(label_width, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| { ui.label(RichText::new(label).color(text_color).size(11.0)); },
        );
        ui.add(
            egui::Slider::new(value, range)
                .show_value(true)
                .custom_formatter(|v, _| format!("{v:.2}")),
        );
    });
}

fn labeled_drag<N: egui::emath::Numeric>(
    ui: &mut egui::Ui,
    label: &str,
    label_width: f32,
    text_color: Color32,
    value: &mut N,
    speed: impl Into<f64>,
    range: std::ops::RangeInclusive<N>,
) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(label_width, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| { ui.label(RichText::new(label).color(text_color).size(11.0)); },
        );
        ui.add(egui::DragValue::new(value).speed(speed).range(range));
    });
}

fn collapsible_header(ui: &mut egui::Ui, label: &str, open: &mut bool, theme: &Theme) -> bool {
    let height = 24.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if response.clicked() {
        *open = !*open;
    }

    if ui.is_rect_visible(rect) {
        let bg = if response.hovered() {
            theme.surfaces.popup.to_color32()
        } else {
            theme.surfaces.panel.to_color32()
        };
        ui.painter().rect_filled(rect, Rounding::ZERO, bg);

        let caret = if *open { CARET_DOWN } else { CARET_RIGHT };
        ui.painter().text(
            egui::pos2(rect.left() + 12.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            caret,
            egui::FontId::proportional(10.0),
            theme.text.secondary.to_color32(),
        );
        ui.painter().text(
            egui::pos2(rect.left() + 28.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(11.0),
            theme.text.primary.to_color32(),
        );
    }

    *open
}

fn grid_tool_button(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    active: bool,
    size: f32,
    accent: Color32,
    theme: &Theme,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size + 18.0), egui::Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    if ui.is_rect_visible(rect) {
        let bg = if active {
            accent
        } else if response.hovered() {
            theme.surfaces.popup.to_color32()
        } else {
            theme.widgets.inactive_bg.to_color32()
        };

        let icon_rect = egui::Rect::from_min_size(rect.min, egui::vec2(size, size));
        ui.painter().rect_filled(icon_rect, Rounding::same(4), bg);

        let icon_color = if active { Color32::WHITE } else { theme.text.primary.to_color32() };
        ui.painter().text(
            icon_rect.center(),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(22.0),
            icon_color,
        );

        let label_color = if active { theme.text.primary.to_color32() } else { theme.text.secondary.to_color32() };
        ui.painter().text(
            egui::pos2(rect.center().x, rect.bottom() - 2.0),
            egui::Align2::CENTER_BOTTOM,
            label,
            egui::FontId::proportional(11.0),
            label_color,
        );
    }

    response
}

fn small_toggle_button(ui: &mut egui::Ui, label: &str, active: bool, theme: &Theme) -> egui::Response {
    let size = egui::vec2(28.0, 24.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    if ui.is_rect_visible(rect) {
        let bg = if active {
            theme.semantic.accent.to_color32()
        } else if response.hovered() {
            theme.surfaces.popup.to_color32()
        } else {
            theme.widgets.inactive_bg.to_color32()
        };
        ui.painter().rect_filled(rect, Rounding::same(3), bg);

        let color = if active { Color32::WHITE } else { theme.text.primary.to_color32() };
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(12.0),
            color,
        );
    }

    response
}
