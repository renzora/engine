//! Unified Tool Settings panel — contextual content based on ActiveTool.
//!
//! Replaces the separate TerrainToolsPanel and FoliagePanel.
//! Uses renzora_ui themed widgets (collapsible_section, inline_property, etc.).

use std::sync::RwLock;

use bevy::prelude::*;
use renzora::bevy_egui::egui::{self, Color32, CursorIcon, RichText, Rounding};
use renzora::egui_phosphor::regular::*;

use renzora_terrain::data::*;
use renzora_terrain::paint::*;
use renzora::theme::{Theme, ThemeManager};
use renzora::editor::{ActiveTool, EditorCommands};
use renzora::editor::{EditorPanel, PanelLocation, collapsible_section, inline_property};
use renzora::editor::{asset_drop_target, AssetDragPayload};
use renzora::core::CurrentProject;

use renzora_terrain::foliage::{FoliageBrushType, FoliageConfig, FoliageDensityMap, FoliagePaintSettings, FoliageType};

pub struct ToolSettingsPanel {
    _state: RwLock<()>,
}

impl ToolSettingsPanel {
    pub fn new() -> Self {
        Self { _state: RwLock::new(()) }
    }
}

impl EditorPanel for ToolSettingsPanel {
    fn id(&self) -> &str { "tool_settings" }
    fn title(&self) -> &str { "Tool Settings" }
    fn icon(&self) -> Option<&str> { Some(WRENCH) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Left }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };
        let theme = &theme;
        let active_tool = world.get_resource::<ActiveTool>().copied().unwrap_or_default();
        let Some(cmds) = world.get_resource::<EditorCommands>() else { return };

        match active_tool {
            ActiveTool::TerrainSculpt => render_sculpt(ui, world, theme, cmds),
            ActiveTool::TerrainPaint => render_paint(ui, world, theme, cmds),
            ActiveTool::FoliagePaint => render_foliage(ui, world, theme, cmds),
            _ => render_empty(ui, theme),
        }
    }
}

// ── Empty state ─────────────────────────────────────────────────────────────

fn render_empty(ui: &mut egui::Ui, theme: &Theme) {
    ui.add_space(16.0);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new(MOUNTAINS)
                .size(32.0)
                .color(theme.text.muted.to_color32()),
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new("Select a terrain and choose\na tool from the viewport toolbar.")
                .size(11.0)
                .color(theme.text.secondary.to_color32()),
        );
    });
}

// ── Sculpt mode ─────────────────────────────────────────────────────────────

fn render_sculpt(ui: &mut egui::Ui, world: &World, theme: &Theme, cmds: &EditorCommands) {
    let settings = match world.get_resource::<TerrainSettings>() {
        Some(s) => s,
        None => return,
    };
    let accent = theme.semantic.accent.to_color32();

    // Brush type grid
    collapsible_section(ui, MOUNTAINS, "Brush", "environment", theme, "sculpt_brush", true, |ui| {
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
            (TerrainBrushType::Stamp,     STAMP,              "Stamp"),
            (TerrainBrushType::Raise,     ARROWS_OUT_CARDINAL,"Raise"),
            (TerrainBrushType::Lower,     ARROW_FAT_LINE_DOWN,"Lower"),
            (TerrainBrushType::SetHeight, EQUALS,             "Set H"),
            (TerrainBrushType::Erase,     ERASER,             "Erase"),
        ];
        render_tool_grid(ui, tools, settings.brush_type, accent, theme, cmds);
    });

    // Tool-specific settings
    collapsible_section(ui, GEAR, "Settings", "environment", theme, "sculpt_settings", true, |ui| {
        let mut row = 0;

        let mut strength = settings.brush_strength;
        inline_property(ui, row, "Strength", theme, |ui| {
            ui.add(egui::Slider::new(&mut strength, 0.01..=1.0).show_value(true))
        });
        if strength != settings.brush_strength {
            cmds.push(move |w: &mut World| {
                if let Some(mut s) = w.get_resource_mut::<TerrainSettings>() { s.brush_strength = strength; }
            });
        }
        row += 1;

        // Flatten-specific
        if settings.brush_type == TerrainBrushType::Flatten {
            let mut flatten_mode = settings.flatten_mode;
            inline_property(ui, row, "Mode", theme, |ui| {
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
                    })
            });
            if flatten_mode != settings.flatten_mode {
                cmds.push(move |w: &mut World| {
                    if let Some(mut s) = w.get_resource_mut::<TerrainSettings>() { s.flatten_mode = flatten_mode; }
                });
            }
            row += 1;

            let mut target_h = settings.target_height;
            inline_property(ui, row, "Target Height", theme, |ui| {
                ui.add(egui::DragValue::new(&mut target_h).speed(0.005).range(0.0..=1.0))
            });
            if target_h != settings.target_height {
                cmds.push(move |w: &mut World| {
                    if let Some(mut s) = w.get_resource_mut::<TerrainSettings>() { s.target_height = target_h; }
                });
            }
            row += 1;
        }

        // Noise-specific
        if settings.brush_type == TerrainBrushType::Noise {
            let mut scale = settings.noise_scale;
            let mut octaves = settings.noise_octaves;
            let mut lac = settings.noise_lacunarity;
            let mut pers = settings.noise_persistence;
            let mut seed = settings.noise_seed;
            let mut noise_mode = settings.noise_mode;

            inline_property(ui, row, "Mode", theme, |ui| {
                egui::ComboBox::from_id_salt("noise_mode")
                    .selected_text(noise_mode.display_name())
                    .show_ui(ui, |ui| {
                        for mode in NoiseMode::all() {
                            ui.selectable_value(&mut noise_mode, *mode, mode.display_name());
                        }
                    })
            });
            row += 1;

            inline_property(ui, row, "Scale", theme, |ui| {
                ui.add(egui::DragValue::new(&mut scale).speed(0.5).range(1.0..=500.0))
            });
            row += 1;
            inline_property(ui, row, "Octaves", theme, |ui| {
                ui.add(egui::DragValue::new(&mut octaves).speed(0.1).range(1..=8))
            });
            row += 1;
            inline_property(ui, row, "Lacunarity", theme, |ui| {
                ui.add(egui::DragValue::new(&mut lac).speed(0.05).range(1.0..=4.0))
            });
            row += 1;
            inline_property(ui, row, "Persistence", theme, |ui| {
                ui.add(egui::DragValue::new(&mut pers).speed(0.01).range(0.1..=0.9))
            });
            row += 1;

            if scale != settings.noise_scale || octaves != settings.noise_octaves
                || lac != settings.noise_lacunarity || pers != settings.noise_persistence
                || seed != settings.noise_seed || noise_mode != settings.noise_mode
            {
                cmds.push(move |w: &mut World| {
                    if let Some(mut s) = w.get_resource_mut::<TerrainSettings>() {
                        s.noise_scale = scale; s.noise_octaves = octaves;
                        s.noise_lacunarity = lac; s.noise_persistence = pers;
                        s.noise_seed = seed; s.noise_mode = noise_mode;
                    }
                });
            }
        }

        // Stamp-specific
        if settings.brush_type == TerrainBrushType::Stamp {
            let stamp = world.get_resource::<StampBrushData>();
            let stamp_name = stamp.map_or("None".to_string(), |s| {
                if s.is_loaded() { format!("{} ({}x{})", s.name, s.width, s.height) } else { "None".to_string() }
            });

            // Preset stamps
            inline_property(ui, row, "Preset", theme, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for preset in StampPreset::all() {
                        let is_current = stamp.map_or(false, |s| s.name == preset.display_name());
                        if small_toggle(ui, preset.display_name(), is_current, theme).clicked() {
                            let p = *preset;
                            cmds.push(move |w: &mut World| {
                                let generated = StampBrushData::generate(p, 128);
                                w.insert_resource(generated);
                            });
                        }
                    }
                })
            });
            row += 1;

            inline_property(ui, row, "Image", theme, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&stamp_name).size(11.0).color(theme.text.secondary.to_color32()));
                    if ui.button(RichText::new(format!("{FILE_IMAGE} Load")).size(11.0)).clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Image", &["png", "jpg", "jpeg", "bmp", "tga"])
                            .pick_file()
                        {
                            if let Ok(data) = std::fs::read(&path) {
                                let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                                match StampBrushData::load_from_png(&data) {
                                    Ok((w, h, pixels)) => {
                                        cmds.push(move |w_: &mut World| {
                                            if let Some(mut stamp) = w_.get_resource_mut::<StampBrushData>() {
                                                stamp.width = w;
                                                stamp.height = h;
                                                stamp.pixels = pixels;
                                                stamp.name = file_name;
                                            }
                                        });
                                    }
                                    Err(e) => {
                                        warn!("Failed to load stamp: {e}");
                                    }
                                }
                            }
                        }
                    }
                })
            });
            row += 1;

            let mut blend_mode = settings.stamp_blend_mode;
            inline_property(ui, row, "Blend", theme, |ui| {
                egui::ComboBox::from_id_salt("stamp_blend_mode")
                    .selected_text(blend_mode.display_name())
                    .show_ui(ui, |ui| {
                        for mode in StampBlendMode::all() {
                            ui.selectable_value(&mut blend_mode, *mode, mode.display_name());
                        }
                    })
            });
            row += 1;

            let mut rotation = settings.stamp_rotation.to_degrees();
            inline_property(ui, row, "Rotation", theme, |ui| {
                ui.add(egui::Slider::new(&mut rotation, 0.0..=360.0).suffix("°").show_value(true))
            });
            row += 1;

            let mut height_scale = settings.stamp_height_scale;
            inline_property(ui, row, "Height Scale", theme, |ui| {
                ui.add(egui::Slider::new(&mut height_scale, 0.01..=2.0).show_value(true))
            });
            row += 1;
            let _ = row;

            let rotation_rad = rotation.to_radians();
            if blend_mode != settings.stamp_blend_mode
                || (rotation_rad - settings.stamp_rotation).abs() > 0.001
                || (height_scale - settings.stamp_height_scale).abs() > 0.001
            {
                cmds.push(move |w: &mut World| {
                    if let Some(mut s) = w.get_resource_mut::<TerrainSettings>() {
                        s.stamp_blend_mode = blend_mode;
                        s.stamp_rotation = rotation_rad;
                        s.stamp_height_scale = height_scale;
                    }
                });
            }
        }

        // Terrace-specific
        if settings.brush_type == TerrainBrushType::Terrace {
            let mut steps = settings.terrace_steps;
            let mut sharpness = settings.terrace_sharpness;

            inline_property(ui, row, "Steps", theme, |ui| {
                ui.add(egui::DragValue::new(&mut steps).speed(0.1).range(2..=32))
            });
            row += 1;
            inline_property(ui, row, "Sharpness", theme, |ui| {
                ui.add(egui::Slider::new(&mut sharpness, 0.0..=1.0))
            });
            let _ = row;

            if steps != settings.terrace_steps || sharpness != settings.terrace_sharpness {
                cmds.push(move |w: &mut World| {
                    if let Some(mut s) = w.get_resource_mut::<TerrainSettings>() {
                        s.terrace_steps = steps; s.terrace_sharpness = sharpness;
                    }
                });
            }
        }
    });

    // Brush shape/size
    collapsible_section(ui, CIRCLE, "Brush Shape", "environment", theme, "sculpt_brush_shape", true, |ui| {
        let mut row = 0;
        let mut radius = settings.brush_radius;
        let mut falloff = settings.falloff;

        inline_property(ui, row, "Size", theme, |ui| {
            ui.add(egui::Slider::new(&mut radius, 1.0..=200.0).show_value(true))
        });
        row += 1;
        inline_property(ui, row, "Falloff", theme, |ui| {
            ui.add(egui::Slider::new(&mut falloff, 0.0..=1.0).show_value(true))
        });
        row += 1;

        // Shape buttons
        let mut shape = settings.brush_shape;
        inline_property(ui, row, "Shape", theme, |ui| {
            ui.horizontal(|ui| {
                for (s, icon) in [(BrushShape::Circle, CIRCLE), (BrushShape::Square, SQUARE), (BrushShape::Diamond, DIAMOND)] {
                    let active = shape == s;
                    if small_toggle(ui, icon, active, theme).clicked() { shape = s; }
                }
            })
        });
        row += 1;

        // Falloff type
        let mut falloff_type = settings.falloff_type;
        inline_property(ui, row, "Falloff Type", theme, |ui| {
            ui.horizontal(|ui| {
                for (ft, label) in [
                    (BrushFalloffType::Smooth, "S"), (BrushFalloffType::Linear, "L"),
                    (BrushFalloffType::Spherical, "O"), (BrushFalloffType::Tip, "T"),
                    (BrushFalloffType::Flat, "F"),
                ] {
                    if small_toggle(ui, label, falloff_type == ft, theme).clicked() { falloff_type = ft; }
                }
            })
        });

        if radius != settings.brush_radius || falloff != settings.falloff
            || shape != settings.brush_shape || falloff_type != settings.falloff_type
        {
            cmds.push(move |w: &mut World| {
                if let Some(mut s) = w.get_resource_mut::<TerrainSettings>() {
                    s.brush_radius = radius; s.falloff = falloff;
                    s.brush_shape = shape; s.falloff_type = falloff_type;
                }
            });
        }
    });
}

// ── Paint mode ──────────────────────────────────────────────────────────────

fn render_paint(ui: &mut egui::Ui, world: &World, theme: &Theme, cmds: &EditorCommands) {
    let accent = theme.semantic.accent.to_color32();
    let text_primary = theme.text.primary.to_color32();

    // Paint brush type
    if let Some(ps) = world.get_resource::<SurfacePaintSettings>() {
        collapsible_section(ui, PAINT_BRUSH, "Brush", "environment", theme, "paint_brush", true, |ui| {
            let tools: &[(PaintBrushType, &str, &str)] = &[
                (PaintBrushType::Paint,  PAINT_BRUSH, "Paint"),
                (PaintBrushType::Erase,  ERASER,      "Erase"),
                (PaintBrushType::Smooth, WAVES,       "Smooth"),
                (PaintBrushType::Fill,   PALETTE,     "Fill"),
            ];
            let current = ps.brush_type;
            let available_width = ui.available_width();
            let cols = 4usize;
            let spacing = 4.0;
            let btn_size = ((available_width - spacing * (cols as f32 - 1.0)) / cols as f32).max(40.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = spacing;
                for (bt, icon, label) in tools {
                    if grid_tool_button(ui, icon, label, current == *bt, btn_size, accent, theme).clicked() {
                        let bt = *bt;
                        cmds.push(move |w: &mut World| {
                            if let Some(mut s) = w.get_resource_mut::<SurfacePaintSettings>() { s.brush_type = bt; }
                        });
                    }
                }
            });
        });
    }

    // Layers
    collapsible_section(ui, PALETTE, "Layers", "environment", theme, "paint_layers", true, |ui| {
        render_layer_list(ui, world, accent, text_primary, theme, cmds);
    });

    // Brush settings
    if let Some(ps) = world.get_resource::<SurfacePaintSettings>() {
        collapsible_section(ui, CIRCLE, "Brush Settings", "environment", theme, "paint_brush_settings", true, |ui| {
            let mut row = 0;
            let mut radius = ps.brush_radius;
            let mut strength = ps.brush_strength;
            let mut falloff = ps.brush_falloff;

            inline_property(ui, row, "Size", theme, |ui| {
                ui.add(egui::Slider::new(&mut radius, 0.01..=0.5).show_value(true))
            });
            row += 1;
            inline_property(ui, row, "Strength", theme, |ui| {
                ui.add(egui::Slider::new(&mut strength, 0.01..=1.0).show_value(true))
            });
            row += 1;
            inline_property(ui, row, "Falloff", theme, |ui| {
                ui.add(egui::Slider::new(&mut falloff, 0.0..=1.0).show_value(true))
            });
            row += 1;

            let mut shape = ps.brush_shape;
            inline_property(ui, row, "Shape", theme, |ui| {
                ui.horizontal(|ui| {
                    for (s, icon) in [(BrushShape::Circle, CIRCLE), (BrushShape::Square, SQUARE), (BrushShape::Diamond, DIAMOND)] {
                        if small_toggle(ui, icon, shape == s, theme).clicked() { shape = s; }
                    }
                })
            });

            if radius != ps.brush_radius || strength != ps.brush_strength
                || falloff != ps.brush_falloff || shape != ps.brush_shape
            {
                cmds.push(move |w: &mut World| {
                    if let Some(mut s) = w.get_resource_mut::<SurfacePaintSettings>() {
                        s.brush_radius = radius; s.brush_strength = strength;
                        s.brush_falloff = falloff; s.brush_shape = shape;
                    }
                });
            }
        });
    }
}

fn render_layer_list(
    ui: &mut egui::Ui,
    world: &World,
    accent: Color32,
    text_primary: Color32,
    theme: &Theme,
    cmds: &EditorCommands,
) {
    let Some(paint_settings) = world.get_resource::<SurfacePaintSettings>() else { return };
    let Some(paint_state) = world.get_resource::<SurfacePaintState>() else { return };

    let active_layer = paint_settings.active_layer;
    let layer_count = paint_state.layer_count.max(2);

    for i in 0..layer_count.min(MAX_LAYERS) {
        let is_active = i == active_layer;
        let bg = if is_active { accent } else { theme.widgets.inactive_bg.to_color32() };
        let fg = if is_active { Color32::WHITE } else { text_primary };

        let name = if i < paint_state.layers_preview.len() {
            paint_state.layers_preview[i].name.clone()
        } else {
            match i { 0 => "Grass", 1 => "Dirt", 2 => "Water", 3 => "Rock", _ => "Layer" }.to_string()
        };

        if ui.add(
            egui::Button::new(RichText::new(format!("{}  {}", i + 1, name)).size(12.0).color(fg))
                .fill(bg)
                .min_size(egui::vec2(ui.available_width(), 26.0)),
        ).clicked() {
            let idx = i;
            cmds.push(move |w: &mut World| {
                if let Some(mut s) = w.get_resource_mut::<SurfacePaintSettings>() { s.active_layer = idx; }
            });
        }

        // Material drop target for active layer
        if is_active {
            let drag_payload = world.get_resource::<AssetDragPayload>();
            let material_source = if i < paint_state.layers_preview.len() {
                paint_state.layers_preview[i].material_source.as_deref()
            } else {
                None
            };
            let drop_result = asset_drop_target(
                ui, egui::Id::new(("tool_layer_mat", i)), material_source,
                &["material"], "Drop .material file", theme, drag_payload,
            );
            if let Some(ref dropped) = drop_result.dropped_path {
                let path = if let Some(project) = world.get_resource::<CurrentProject>() {
                    project.make_asset_relative(dropped)
                } else {
                    dropped.to_string_lossy().to_string()
                };
                let layer = i;
                cmds.push(move |w: &mut World| {
                    if let Some(mut ps) = w.get_resource_mut::<SurfacePaintState>() {
                        ps.pending_commands.push(SurfacePaintCommand::AssignMaterial { layer, path });
                    }
                });
            }
            if drop_result.cleared {
                let layer = i;
                cmds.push(move |w: &mut World| {
                    if let Some(mut ps) = w.get_resource_mut::<SurfacePaintState>() {
                        ps.pending_commands.push(SurfacePaintCommand::ClearMaterial(layer));
                    }
                });
            }
        }

        ui.add_space(2.0);
    }

    // Add layer
    if layer_count < MAX_LAYERS {
        if ui.add(
            egui::Button::new(RichText::new(format!("{PLUS}  Add Layer")).size(11.0).color(theme.text.secondary.to_color32()))
                .min_size(egui::vec2(ui.available_width(), 22.0)),
        ).clicked() {
            cmds.push(|w: &mut World| {
                if let Some(mut ps) = w.get_resource_mut::<SurfacePaintState>() {
                    ps.pending_commands.push(SurfacePaintCommand::AddLayer);
                }
            });
        }
    }
}

// ── Foliage mode ────────────────────────────────────────────────────────────

fn render_foliage(ui: &mut egui::Ui, world: &World, theme: &Theme, cmds: &EditorCommands) {
    let accent = theme.semantic.accent.to_color32();
    let config = world.get_resource::<FoliageConfig>().cloned().unwrap_or_default();
    let settings = world.get_resource::<FoliagePaintSettings>().cloned().unwrap_or_default();

    let mut config_mut = config.clone();
    let mut settings_mut = settings.clone();

    // Foliage types
    collapsible_section(ui, TREE, "Foliage Types", "environment", theme, "foliage_types", true, |ui| {
        for (i, ft) in config_mut.types.iter().enumerate() {
            let selected = i == settings_mut.active_type;
            if ui.selectable_label(selected, RichText::new(format!("{}. {}", i + 1, ft.name)).size(12.0)).clicked() {
                settings_mut.active_type = i;
            }
        }
        ui.horizontal(|ui| {
            if ui.button(RichText::new(format!("{PLUS} Add")).size(11.0)).clicked() {
                config_mut.types.push(FoliageType::default());
            }
            if config_mut.types.len() > 1 {
                if ui.button(RichText::new(format!("{TRASH} Remove")).size(11.0)).clicked() {
                    let idx = settings_mut.active_type.min(config_mut.types.len().saturating_sub(1));
                    config_mut.types.remove(idx);
                    if settings_mut.active_type >= config_mut.types.len() {
                        settings_mut.active_type = config_mut.types.len().saturating_sub(1);
                    }
                }
            }
        });
    });

    // Brush
    collapsible_section(ui, PAINT_BRUSH, "Brush", "environment", theme, "foliage_brush", true, |ui| {
        ui.horizontal(|ui| {
            if small_toggle(ui, PAINT_BRUSH, settings_mut.brush_type == FoliageBrushType::Paint, theme).clicked() {
                settings_mut.brush_type = FoliageBrushType::Paint;
            }
            if small_toggle(ui, ERASER, settings_mut.brush_type == FoliageBrushType::Erase, theme).clicked() {
                settings_mut.brush_type = FoliageBrushType::Erase;
            }
        });

        let mut row = 0;
        inline_property(ui, row, "Size", theme, |ui| {
            ui.add(egui::Slider::new(&mut settings_mut.brush_radius, 0.01..=0.5).show_value(true))
        });
        row += 1;
        inline_property(ui, row, "Strength", theme, |ui| {
            ui.add(egui::Slider::new(&mut settings_mut.brush_strength, 0.01..=1.0).show_value(true))
        });
        row += 1;
        inline_property(ui, row, "Falloff", theme, |ui| {
            ui.add(egui::Slider::new(&mut settings_mut.brush_falloff, 0.0..=1.0).show_value(true))
        });
    });

    // Properties of selected type
    if let Some(ft) = config_mut.types.get_mut(settings_mut.active_type) {
        collapsible_section(ui, GEAR, "Properties", "environment", theme, "foliage_props", true, |ui| {
            let mut row = 0;
            inline_property(ui, row, "Name", theme, |ui| {
                ui.text_edit_singleline(&mut ft.name)
            });
            row += 1;
            inline_property(ui, row, "Density", theme, |ui| {
                ui.add(egui::Slider::new(&mut ft.density, 1.0..=50.0).show_value(true))
            });
            row += 1;
            inline_property(ui, row, "Height Min", theme, |ui| {
                ui.add(egui::DragValue::new(&mut ft.height_range.x).speed(0.01).range(0.01..=2.0))
            });
            row += 1;
            inline_property(ui, row, "Height Max", theme, |ui| {
                ui.add(egui::DragValue::new(&mut ft.height_range.y).speed(0.01).range(0.01..=2.0))
            });
            row += 1;
            inline_property(ui, row, "Wind", theme, |ui| {
                ui.add(egui::Slider::new(&mut ft.wind_strength, 0.0..=2.0).show_value(true))
            });
            row += 1;
            inline_property(ui, row, "Enabled", theme, |ui| {
                ui.checkbox(&mut ft.enabled, "")
            });
        });
    }

    // Ensure chunks have density maps
    cmds.push(|w: &mut World| {
        let entities: Vec<Entity> = w
            .query_filtered::<Entity, (With<TerrainChunkData>, Without<FoliageDensityMap>)>()
            .iter(w)
            .collect();
        for entity in entities {
            w.entity_mut(entity).insert(FoliageDensityMap::new(64));
        }
    });

    // Write back changes
    if config_mut.types != config.types {
        let val = config_mut;
        cmds.push(move |w: &mut World| { w.insert_resource(val); });
    }
    if settings_mut.active_type != settings.active_type
        || settings_mut.brush_type != settings.brush_type
        || settings_mut.brush_radius != settings.brush_radius
        || settings_mut.brush_strength != settings.brush_strength
        || settings_mut.brush_falloff != settings.brush_falloff
    {
        let val = settings_mut;
        cmds.push(move |w: &mut World| { w.insert_resource(val); });
    }
}

// ── Shared helpers ──────────────────────────────────────────────────────────

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
    let btn_size = ((available_width - spacing * (cols as f32 - 1.0)) / cols as f32).max(40.0);

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
                        if grid_tool_button(ui, icon, label, active, btn_size, accent, theme).clicked() {
                            let bt = *brush_type;
                            cmds.push(move |w: &mut World| {
                                if let Some(mut s) = w.get_resource_mut::<TerrainSettings>() { s.brush_type = bt; }
                            });
                        }
                    }
                }
            });
        }
    });
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
    if response.hovered() { ui.ctx().set_cursor_icon(CursorIcon::PointingHand); }
    if ui.is_rect_visible(rect) {
        let bg = if active { accent } else if response.hovered() { theme.widgets.hovered_bg.to_color32() } else { theme.widgets.inactive_bg.to_color32() };
        let fg = if active { Color32::WHITE } else { theme.text.primary.to_color32() };
        ui.painter().rect_filled(rect, Rounding::same(4), bg);
        let icon_center = egui::pos2(rect.center().x, rect.center().y - 7.0);
        ui.painter().text(icon_center, egui::Align2::CENTER_CENTER, icon, egui::FontId::proportional(16.0), fg);
        let label_center = egui::pos2(rect.center().x, rect.max.y - 8.0);
        ui.painter().text(label_center, egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(9.0), fg);
    }
    response
}

fn small_toggle(ui: &mut egui::Ui, label: &str, active: bool, theme: &Theme) -> egui::Response {
    let size = egui::vec2(24.0, 20.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if response.hovered() { ui.ctx().set_cursor_icon(CursorIcon::PointingHand); }
    if ui.is_rect_visible(rect) {
        let bg = if active { theme.semantic.accent.to_color32() } else if response.hovered() { theme.widgets.hovered_bg.to_color32() } else { theme.widgets.inactive_bg.to_color32() };
        let fg = if active { Color32::WHITE } else { theme.text.primary.to_color32() };
        ui.painter().rect_filled(rect, Rounding::same(3), bg);
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(12.0), fg);
    }
    response
}
