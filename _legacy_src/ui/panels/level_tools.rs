//! Level Tools panel for terrain editing
//!
//! Provides Unreal Engine-style terrain tools with Sculpt and Paint tabs.

use bevy_egui::egui::{self, Color32, CursorIcon, RichText, Vec2, Rounding};
use egui_phosphor::regular::{
    SELECTION, ARROWS_OUT_CARDINAL, PAINT_BRUSH, PENCIL, MOUNTAINS,
    WAVES, EQUALS, ARROW_FAT_LINE_UP, TREE,
    DROP, WAVEFORM, GRAPH, EYE, ARROWS_HORIZONTAL, CURSOR, COPY,
    CARET_DOWN, CARET_RIGHT, CIRCLE, SQUARE, DIAMOND,
    STAIRS, ARROWS_IN_CARDINAL, ACTIVITY, CHART_BAR,
};

use crate::brushes::{BrushSettings, BrushType, BlockEditState};
use crate::core::AssetBrowserState;
use crate::gizmo::{EditorTool, GizmoState};
use crate::terrain::{TerrainBrushType, TerrainSettings, TerrainTab, BrushShape, BrushFalloffType, FlattenMode};
use crate::surface_painting::{SurfacePaintSettings, SurfacePaintState, SurfacePaintCommand, PaintBrushType};
use renzora_theme::Theme;

/// Render the Level Tools panel content
pub fn render_level_tools_content(
    ui: &mut egui::Ui,
    gizmo_state: &mut GizmoState,
    brush_settings: &mut BrushSettings,
    _block_edit: &BlockEditState,
    terrain_settings: &mut TerrainSettings,
    paint_settings: &mut SurfacePaintSettings,
    paint_state: &mut SurfacePaintState,
    asset_browser: &mut AssetBrowserState,
    theme: &Theme,
) {
    let terrain_active = gizmo_state.tool == EditorTool::TerrainSculpt;
    let surface_paint_active = gizmo_state.tool == EditorTool::SurfacePaint;

    // Auto-sync terrain tab with active tool
    if surface_paint_active && terrain_settings.tab != TerrainTab::Paint {
        terrain_settings.tab = TerrainTab::Paint;
    }

    if terrain_active || surface_paint_active {
        render_terrain_panel(ui, gizmo_state, terrain_settings, paint_settings, paint_state, asset_browser, theme);
    } else {
        render_standard_tools(ui, gizmo_state, brush_settings, theme);
    }
}

/// Render Unreal Engine style terrain panel
fn render_terrain_panel(
    ui: &mut egui::Ui,
    gizmo_state: &mut GizmoState,
    settings: &mut TerrainSettings,
    paint_settings: &mut SurfacePaintSettings,
    paint_state: &mut SurfacePaintState,
    asset_browser: &mut AssetBrowserState,
    theme: &Theme,
) {
    let accent = theme.semantic.accent.to_color32();
    let text_primary = theme.text.primary.to_color32();
    let text_secondary = theme.text.secondary.to_color32();
    let bg_dark = theme.surfaces.popup.to_color32();

    // === Tab Bar ===
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;

        let tab_width = ui.available_width() / 2.0;

        // Sculpt tab
        let sculpt_selected = settings.tab == TerrainTab::Sculpt;
        if tab_button(ui, "Sculpt", sculpt_selected, tab_width, accent, theme).clicked() {
            settings.tab = TerrainTab::Sculpt;
            gizmo_state.tool = EditorTool::TerrainSculpt;
        }

        // Paint tab
        let paint_selected = settings.tab == TerrainTab::Paint;
        if tab_button(ui, "Paint", paint_selected, tab_width, accent, theme).clicked() {
            settings.tab = TerrainTab::Paint;
            gizmo_state.tool = EditorTool::SurfacePaint;
        }
    });

    ui.add_space(8.0);

    // === Content ===
    egui::ScrollArea::vertical().show(ui, |ui| {
        if settings.tab == TerrainTab::Sculpt {
            render_sculpt_tools(ui, settings, accent, theme);

            ui.add_space(8.0);

            // === Tool Settings Section ===
            if collapsible_header(ui, "Tool Settings", &mut settings.section_tool_settings, theme) {
                ui.add_space(4.0);
                ui.indent("tool_settings", |ui| {
                    render_tool_settings(ui, settings, text_primary, text_secondary, bg_dark, theme);
                });
                ui.add_space(4.0);
            }

            // === Brush Settings Section ===
            if collapsible_header(ui, "Brush Settings", &mut settings.section_brush_settings, theme) {
                ui.add_space(4.0);
                ui.indent("brush_settings", |ui| {
                    render_brush_settings(ui, settings, text_primary, text_secondary, bg_dark, theme);
                });
                ui.add_space(4.0);
            }

            // === Layers Section ===
            if collapsible_header(ui, "Layers", &mut settings.section_layers, theme) {
                ui.add_space(4.0);
                ui.indent("layers", |ui| {
                    render_layers(ui, settings, text_secondary, bg_dark, theme);
                });
                ui.add_space(4.0);
            }
        } else {
            // Paint tab shows the surface paint panel
            render_surface_paint_panel(ui, paint_settings, paint_state, asset_browser, theme);
        }
    });
}

/// Render sculpt tool grid
fn render_sculpt_tools(
    ui: &mut egui::Ui,
    settings: &mut TerrainSettings,
    accent: Color32,
    theme: &Theme,
) {
    // 4 columns, 4 rows — the 16 most useful sculpt tools
    let tools: &[(TerrainBrushType, &str, &str)] = &[
        // Row 1: Core deformation
        (TerrainBrushType::Sculpt,  MOUNTAINS,          "Sculpt"),
        (TerrainBrushType::Smooth,  WAVES,              "Smooth"),
        (TerrainBrushType::Flatten, EQUALS,             "Flatten"),
        (TerrainBrushType::Ramp,    ARROW_FAT_LINE_UP,  "Ramp"),
        // Row 2: Erosion & procedural
        (TerrainBrushType::Erosion, TREE,               "Erosion"),
        (TerrainBrushType::Hydro,   DROP,               "Hydro"),
        (TerrainBrushType::Noise,   WAVEFORM,           "Noise"),
        (TerrainBrushType::Terrace, STAIRS,             "Terrace"),
        // Row 3: Refinement
        (TerrainBrushType::Pinch,   ARROWS_IN_CARDINAL, "Pinch"),
        (TerrainBrushType::Relax,   ACTIVITY,           "Relax"),
        (TerrainBrushType::Retop,   GRAPH,              "Retop"),
        (TerrainBrushType::Cliff,   CHART_BAR,          "Cliff"),
        // Row 4: Utility
        (TerrainBrushType::Visibility, EYE,             "Visibility"),
        (TerrainBrushType::Mirror,  ARROWS_HORIZONTAL,  "Mirror"),
        (TerrainBrushType::Select,  CURSOR,             "Select"),
        (TerrainBrushType::Copy,    COPY,               "Copy"),
    ];

    render_tool_grid(ui, tools, &mut settings.brush_type, accent, theme);
}

/// Render the Surface Paint tool panel
fn render_surface_paint_panel(
    ui: &mut egui::Ui,
    settings: &mut SurfacePaintSettings,
    paint_state: &mut SurfacePaintState,
    asset_browser: &mut AssetBrowserState,
    theme: &Theme,
) {
    let text_secondary = theme.text.secondary.to_color32();
    let text_primary = theme.text.primary.to_color32();
    let accent = theme.semantic.accent.to_color32();

    ui.add_space(4.0);
    ui.label(RichText::new("Surface Paint").color(text_primary).size(13.0));
    ui.add_space(4.0);

    // Brush type buttons
    ui.horizontal(|ui| {
        let types = [
            (PaintBrushType::Paint, "Paint"),
            (PaintBrushType::Erase, "Erase"),
            (PaintBrushType::Smooth, "Smooth"),
            (PaintBrushType::Fill, "Fill"),
        ];
        for (bt, label) in types {
            let active = settings.brush_type == bt;
            let (bg, fg) = if active {
                (accent, Color32::WHITE)
            } else {
                (theme.widgets.inactive_bg.to_color32(), theme.text.primary.to_color32())
            };
            if ui.add(egui::Button::new(RichText::new(label).size(11.0).color(fg)).fill(bg).min_size(Vec2::new(50.0, 24.0))).clicked() {
                settings.brush_type = bt;
            }
        }
    });

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    // --- Layer selector ---
    ui.horizontal(|ui| {
        ui.label(RichText::new("Layers").color(text_primary).size(12.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if paint_state.layer_count < 4 && paint_state.layer_count > 0 {
                if ui.small_button("+ Add").clicked() {
                    paint_state.pending_commands.push(SurfacePaintCommand::AddLayer);
                }
            }
        });
    });
    ui.add_space(4.0);

    if paint_state.layers_preview.is_empty() {
        ui.label(RichText::new("No paintable surface selected.\nSelect a mesh with Paintable Surface.").color(text_secondary).size(10.0));
    } else {
        let available_width = ui.available_width();

        // Update cached drag path from current frame's drag state
        let current_drag: Option<String> = asset_browser.dragging_asset.as_ref().and_then(|p| {
            let s = p.to_string_lossy();
            if s.ends_with(".wgsl") || s.ends_with(".material_bp") {
                Some(s.to_string())
            } else {
                None
            }
        });
        if current_drag.is_some() {
            paint_state.last_dragging_material = current_drag;
        }

        // Use cached path for visuals and drop detection (survives cross-panel clearing)
        let is_material_dragging = paint_state.last_dragging_material.is_some();

        let mut drop_target_layer: Option<usize> = None;
        let mut remove_layer: Option<usize> = None;

        // Snapshot layers to avoid borrow issues
        let layers: Vec<_> = paint_state.layers_preview.clone();

        for (i, layer) in layers.iter().enumerate() {
            let is_active = settings.active_layer == i;

            let bg = if is_active {
                accent.linear_multiply(0.25)
            } else {
                theme.widgets.inactive_bg.to_color32()
            };

            let frame = egui::Frame::new()
                .fill(bg)
                .rounding(Rounding::same(4))
                .inner_margin(egui::Margin::same(6));

            let resp = frame.show(ui, |ui| {
                ui.set_min_width(available_width - 16.0);
                ui.horizontal(|ui| {
                    // Layer name + material info
                    ui.vertical(|ui| {
                        let name_text = if is_active {
                            RichText::new(format!("{} {}", PAINT_BRUSH, &layer.name)).color(Color32::WHITE).size(11.0).strong()
                        } else {
                            RichText::new(&layer.name).color(text_primary).size(11.0)
                        };
                        ui.label(name_text);

                        // Show material source or drop hint
                        if let Some(src) = &layer.material_source {
                            ui.horizontal(|ui| {
                                // Show filename only
                                let filename = src.rsplit(['/', '\\']).next().unwrap_or(src);
                                ui.label(RichText::new(filename).color(accent).size(9.0));
                                if ui.small_button("\u{00d7}").clicked() {
                                    paint_state.pending_commands.push(SurfacePaintCommand::ClearMaterial(i));
                                }
                            });
                        } else if is_material_dragging {
                            ui.label(RichText::new("Drop material here").color(accent).size(9.0));
                        } else {
                            ui.label(RichText::new("Drag .wgsl / .material_bp here").color(text_secondary).size(9.0));
                        }
                    });

                    // Remove button (right side)
                    if paint_state.layer_count > 1 {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("\u{00d7}").clicked() {
                                remove_layer = Some(i);
                            }
                        });
                    }
                });
            });

            // Click to select layer
            if resp.response.interact(egui::Sense::click()).clicked() {
                settings.active_layer = i;
            }

            // Drag-drop: use pointer position for reliable cross-panel detection
            let card_rect = resp.response.rect;
            let pointer_pos = ui.input(|inp| inp.pointer.hover_pos());
            let pointer_in_card = pointer_pos.map_or(false, |pos| card_rect.contains(pos));

            if is_material_dragging && pointer_in_card {
                ui.painter().rect_stroke(
                    card_rect,
                    Rounding::same(4),
                    egui::Stroke::new(2.0, accent),
                    egui::StrokeKind::Outside,
                );
                drop_target_layer = Some(i);
            }

            ui.add_space(2.0);
        }

        // Handle drag-drop: if mouse released while pointer was in a layer card
        let pointer_released = ui.input(|i| i.pointer.any_released());
        if let Some(target_layer) = drop_target_layer {
            if pointer_released {
                if let Some(path) = paint_state.last_dragging_material.take() {
                    // Also clear the source so other panels don't double-process
                    asset_browser.dragging_asset = None;
                    paint_state.pending_commands.push(SurfacePaintCommand::AssignMaterial {
                        layer: target_layer,
                        path,
                    });
                }
            }
        }
        // Clear cached drag path when pointer released (no drop or drop already handled)
        if pointer_released {
            paint_state.last_dragging_material = None;
        }

        // Handle remove
        if let Some(idx) = remove_layer {
            paint_state.pending_commands.push(SurfacePaintCommand::RemoveLayer(idx));
            if settings.active_layer >= paint_state.layer_count.saturating_sub(1) {
                settings.active_layer = paint_state.layer_count.saturating_sub(2);
            }
        }

        // Clamp active layer
        if settings.active_layer >= paint_state.layer_count {
            settings.active_layer = paint_state.layer_count.saturating_sub(1);
        }
    }

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    // --- Brush settings ---
    ui.label(RichText::new("Brush").color(text_primary).size(12.0));
    ui.add_space(4.0);

    let label_width = 100.0;

    // Brush radius
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(label_width, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| { ui.label(RichText::new("Radius").color(text_secondary).size(11.0)); }
        );
        ui.add(egui::Slider::new(&mut settings.brush_radius, 0.005..=0.5)
            .show_value(true)
            .custom_formatter(|v, _| format!("{:.3}", v)));
    });

    // Brush strength
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(label_width, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| { ui.label(RichText::new("Strength").color(text_secondary).size(11.0)); }
        );
        ui.add(egui::Slider::new(&mut settings.brush_strength, 0.01..=1.0)
            .show_value(true)
            .custom_formatter(|v, _| format!("{:.2}", v)));
    });

    // Brush falloff
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(label_width, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| { ui.label(RichText::new("Falloff").color(text_secondary).size(11.0)); }
        );
        ui.add(egui::Slider::new(&mut settings.brush_falloff, 0.0..=1.0)
            .show_value(true)
            .custom_formatter(|v, _| format!("{:.2}", v)));
    });

    // Brush shape
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(label_width, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| { ui.label(RichText::new("Shape").color(text_secondary).size(11.0)); }
        );
        let shapes = [
            (BrushShape::Circle, CIRCLE),
            (BrushShape::Square, SQUARE),
            (BrushShape::Diamond, DIAMOND),
        ];
        for (shape, icon) in shapes {
            let active = settings.brush_shape == shape;
            if shape_button(ui, icon, active, theme).clicked() {
                settings.brush_shape = shape;
            }
        }
    });

    // Falloff type
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(label_width, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| { ui.label(RichText::new("Falloff Type").color(text_secondary).size(11.0)); }
        );
        let falloffs = [
            (BrushFalloffType::Smooth, "S"),
            (BrushFalloffType::Linear, "L"),
            (BrushFalloffType::Spherical, "O"),
            (BrushFalloffType::Tip, "T"),
            (BrushFalloffType::Flat, "F"),
        ];
        for (falloff, label) in falloffs {
            let active = settings.falloff_type == falloff;
            if falloff_button(ui, label, active, theme).clicked() {
                settings.falloff_type = falloff;
            }
        }
    });
}

/// Render a grid of tool buttons
fn render_tool_grid(
    ui: &mut egui::Ui,
    tools: &[(TerrainBrushType, &str, &str)],
    selected: &mut TerrainBrushType,
    accent: Color32,
    theme: &Theme,
) {
    let available_width = ui.available_width();
    let cols = 4; // 4 columns
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
                        let active = *selected == *brush_type;
                        if grid_tool_button(ui, icon, label, active, button_size, accent, theme).clicked() {
                            *selected = *brush_type;
                        }
                    }
                }
            });
        }
    });
}

/// Render tool-specific settings
fn render_tool_settings(
    ui: &mut egui::Ui,
    settings: &mut TerrainSettings,
    text_primary: Color32,
    text_secondary: Color32,
    bg_dark: Color32,
    theme: &Theme,
) {
    let label_width = 100.0;

    // Tool Strength
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(label_width, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| { ui.label(RichText::new("Tool Strength").color(text_secondary).size(11.0)); }
        );
        ui.add(egui::Slider::new(&mut settings.brush_strength, 0.01..=1.0)
            .show_value(true)
            .custom_formatter(|v, _| format!("{:.2}", v)));
    });

    // Flatten-specific settings
    if settings.brush_type == TerrainBrushType::Flatten {
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(label_width, 20.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| { ui.label(RichText::new("Flatten Mode").color(text_secondary).size(11.0)); }
            );
            egui::ComboBox::from_id_salt("flatten_mode")
                .selected_text(match settings.flatten_mode {
                    FlattenMode::Both  => "Both",
                    FlattenMode::Raise => "Raise",
                    FlattenMode::Lower => "Lower",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut settings.flatten_mode, FlattenMode::Both,  "Both");
                    ui.selectable_value(&mut settings.flatten_mode, FlattenMode::Raise, "Raise");
                    ui.selectable_value(&mut settings.flatten_mode, FlattenMode::Lower, "Lower");
                });
        });

        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(label_width, 20.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| { ui.label(RichText::new("Target Height").color(text_secondary).size(11.0)); }
            );
            ui.add(egui::DragValue::new(&mut settings.target_height)
                .speed(0.005).range(0.0..=1.0));
        });
    }

    // Noise-specific settings
    if settings.brush_type == TerrainBrushType::Noise {
        ui.add_space(4.0);
        ui.label(RichText::new("Noise Settings").color(text_primary).size(11.0));
        ui.add_space(2.0);

        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(label_width, 20.0), egui::Layout::left_to_right(egui::Align::Center),
                |ui| { ui.label(RichText::new("Scale").color(text_secondary).size(11.0)); });
            ui.add(egui::DragValue::new(&mut settings.noise_scale).speed(0.5).range(1.0..=500.0));
        });
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(label_width, 20.0), egui::Layout::left_to_right(egui::Align::Center),
                |ui| { ui.label(RichText::new("Octaves").color(text_secondary).size(11.0)); });
            ui.add(egui::DragValue::new(&mut settings.noise_octaves).speed(0.1).range(1..=8));
        });
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(label_width, 20.0), egui::Layout::left_to_right(egui::Align::Center),
                |ui| { ui.label(RichText::new("Lacunarity").color(text_secondary).size(11.0)); });
            ui.add(egui::DragValue::new(&mut settings.noise_lacunarity).speed(0.05).range(1.0..=4.0));
        });
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(label_width, 20.0), egui::Layout::left_to_right(egui::Align::Center),
                |ui| { ui.label(RichText::new("Persistence").color(text_secondary).size(11.0)); });
            ui.add(egui::DragValue::new(&mut settings.noise_persistence).speed(0.01).range(0.1..=0.9));
        });
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(label_width, 20.0), egui::Layout::left_to_right(egui::Align::Center),
                |ui| { ui.label(RichText::new("Seed").color(text_secondary).size(11.0)); });
            ui.add(egui::DragValue::new(&mut settings.noise_seed).speed(1.0));
        });
    }

    // Terrace-specific settings
    if settings.brush_type == TerrainBrushType::Terrace {
        ui.add_space(4.0);
        ui.label(RichText::new("Terrace Settings").color(text_primary).size(11.0));
        ui.add_space(2.0);

        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(label_width, 20.0), egui::Layout::left_to_right(egui::Align::Center),
                |ui| { ui.label(RichText::new("Steps").color(text_secondary).size(11.0)); });
            ui.add(egui::DragValue::new(&mut settings.terrace_steps).speed(0.1).range(2..=32));
        });
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(label_width, 20.0), egui::Layout::left_to_right(egui::Align::Center),
                |ui| { ui.label(RichText::new("Sharpness").color(text_secondary).size(11.0)); });
            ui.add(egui::Slider::new(&mut settings.terrace_sharpness, 0.0..=1.0)
                .custom_formatter(|v, _| format!("{:.2}", v)));
        });
    }

    // Brush Shape
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(label_width, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| { ui.label(RichText::new("Brush Shape").color(text_secondary).size(11.0)); }
        );

        let shapes = [
            (BrushShape::Circle, CIRCLE),
            (BrushShape::Square, SQUARE),
            (BrushShape::Diamond, DIAMOND),
        ];

        for (shape, icon) in shapes {
            let active = settings.brush_shape == shape;
            if shape_button(ui, icon, active, theme).clicked() {
                settings.brush_shape = shape;
            }
        }
    });

    // Brush Falloff Type
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(label_width, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| { ui.label(RichText::new("Brush Falloff").color(text_secondary).size(11.0)); }
        );

        let falloffs = [
            (BrushFalloffType::Smooth, "S"),
            (BrushFalloffType::Linear, "L"),
            (BrushFalloffType::Spherical, "O"),
            (BrushFalloffType::Tip, "T"),
            (BrushFalloffType::Flat, "F"),
        ];

        for (falloff, label) in falloffs {
            let active = settings.falloff_type == falloff;
            if falloff_button(ui, label, active, theme).clicked() {
                settings.falloff_type = falloff;
            }
        }
    });
}

/// Render brush settings
fn render_brush_settings(
    ui: &mut egui::Ui,
    settings: &mut TerrainSettings,
    text_primary: Color32,
    text_secondary: Color32,
    bg_dark: Color32,
    theme: &Theme,
) {
    let label_width = 100.0;

    // Brush Size
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(label_width, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| { ui.label(RichText::new("Brush Size").color(text_secondary).size(11.0)); }
        );
        ui.add(egui::Slider::new(&mut settings.brush_radius, 1.0..=200.0)
            .show_value(true)
            .custom_formatter(|v, _| format!("{:.1}", v)));
    });

    // Brush Falloff
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(label_width, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| { ui.label(RichText::new("Brush Falloff").color(text_secondary).size(11.0)); }
        );
        ui.add(egui::Slider::new(&mut settings.falloff, 0.0..=1.0)
            .show_value(true)
            .custom_formatter(|v, _| format!("{:.2}", v)));
    });

    ui.add_space(4.0);

    // View Options
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(label_width, 20.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| { ui.label(RichText::new("Hide Meshes").color(text_secondary).size(11.0)); }
        );
        ui.checkbox(&mut settings.hide_non_terrain_meshes, "");
    });
}

/// Render layers section
fn render_layers(
    ui: &mut egui::Ui,
    settings: &mut TerrainSettings,
    text_secondary: Color32,
    bg_dark: Color32,
    theme: &Theme,
) {
    // Heightmap layer
    let layer_height = 50.0;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), layer_height),
        egui::Sense::click()
    );

    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, Rounding::same(4), bg_dark);

        // Thumbnail area
        let thumb_rect = egui::Rect::from_min_size(
            rect.min + egui::vec2(8.0, 8.0),
            egui::vec2(34.0, 34.0)
        );
        ui.painter().rect_filled(thumb_rect, Rounding::same(2), Color32::from_rgb(60, 80, 60));

        // Grid pattern
        let grid_color = Color32::from_rgb(80, 100, 80);
        for i in 0..5 {
            let y = thumb_rect.top() + (i as f32 + 1.0) * 6.0;
            ui.painter().line_segment(
                [egui::pos2(thumb_rect.left() + 2.0, y), egui::pos2(thumb_rect.right() - 2.0, y)],
                egui::Stroke::new(1.0, grid_color)
            );
        }

        // Label
        ui.painter().text(
            egui::pos2(rect.left() + 50.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            "Heightmap",
            egui::FontId::proportional(11.0),
            text_secondary,
        );
    }
}

/// Tab button
fn tab_button(
    ui: &mut egui::Ui,
    label: &str,
    selected: bool,
    width: f32,
    accent: Color32,
    theme: &Theme,
) -> egui::Response {
    let height = 28.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    if ui.is_rect_visible(rect) {
        let bg = if selected {
            accent
        } else if response.hovered() {
            theme.surfaces.popup.to_color32()
        } else {
            theme.surfaces.panel.to_color32()
        };

        ui.painter().rect_filled(rect, Rounding::ZERO, bg);

        let text_color = if selected { Color32::WHITE } else { theme.text.secondary.to_color32() };
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(12.0),
            text_color,
        );
    }

    response
}

/// Collapsible section header
fn collapsible_header(
    ui: &mut egui::Ui,
    label: &str,
    open: &mut bool,
    theme: &Theme,
) -> bool {
    let height = 24.0;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::click()
    );

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

        // Caret
        let caret = if *open { CARET_DOWN } else { CARET_RIGHT };
        ui.painter().text(
            egui::pos2(rect.left() + 12.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            caret,
            egui::FontId::proportional(10.0),
            theme.text.secondary.to_color32(),
        );

        // Label
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

/// Grid tool button
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

        // Icon area
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

        // Label below
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

/// Shape button
fn shape_button(ui: &mut egui::Ui, icon: &str, active: bool, theme: &Theme) -> egui::Response {
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
            icon,
            egui::FontId::proportional(12.0),
            color,
        );
    }

    response
}

/// Falloff type button
fn falloff_button(ui: &mut egui::Ui, label: &str, active: bool, theme: &Theme) -> egui::Response {
    let size = egui::vec2(24.0, 24.0);
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
            egui::FontId::proportional(10.0),
            color,
        );
    }

    response
}

/// Render standard editor tools (non-terrain mode)
fn render_standard_tools(
    ui: &mut egui::Ui,
    gizmo_state: &mut GizmoState,
    brush_settings: &mut BrushSettings,
    theme: &Theme,
) {
    let secondary_text = theme.text.secondary.to_color32();
    let accent_color = theme.semantic.accent.to_color32();

    let select_active = gizmo_state.tool == EditorTool::Select;
    let transform_active = gizmo_state.tool == EditorTool::Transform;
    let brush_active = gizmo_state.tool == EditorTool::Brush;
    let edit_active = gizmo_state.tool == EditorTool::BlockEdit;
    let terrain_active = gizmo_state.tool == EditorTool::TerrainSculpt;
    let surface_paint_active = gizmo_state.tool == EditorTool::SurfacePaint;

    ui.add_space(4.0);

    ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;

        if large_tool_button(ui, SELECTION, "Select (Q)", select_active, accent_color, theme).clicked() {
            gizmo_state.tool = EditorTool::Select;
        }

        if large_tool_button(ui, ARROWS_OUT_CARDINAL, "Transform (W)", transform_active, accent_color, theme).clicked() {
            gizmo_state.tool = EditorTool::Transform;
        }

        if large_tool_button(ui, PAINT_BRUSH, "Brush (B)", brush_active, accent_color, theme).clicked() {
            gizmo_state.tool = EditorTool::Brush;
        }

        if large_tool_button(ui, PENCIL, "Edit (E)", edit_active, accent_color, theme).clicked() {
            gizmo_state.tool = EditorTool::BlockEdit;
        }

        if large_tool_button(ui, MOUNTAINS, "Terrain (T)", terrain_active, accent_color, theme).clicked() {
            gizmo_state.tool = EditorTool::TerrainSculpt;
        }

        if large_tool_button(ui, DROP, "Surface Paint (P)", surface_paint_active, accent_color, theme).clicked() {
            gizmo_state.tool = EditorTool::SurfacePaint;
        }
    });

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    if gizmo_state.tool == EditorTool::Brush {
        ui.label(RichText::new("Brush").color(secondary_text).size(10.0));
        ui.add_space(2.0);

        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;

            for brush_type in &[BrushType::Block, BrushType::Floor, BrushType::Wall, BrushType::Stairs, BrushType::Ramp] {
                let active = brush_settings.selected_brush == *brush_type;
                if compact_button(ui, brush_type.display_name(), active, accent_color, theme).clicked() {
                    brush_settings.selected_brush = *brush_type;
                }
            }
        });

        ui.add_space(6.0);

        let mut snap = brush_settings.snap_enabled;
        if ui.checkbox(&mut snap, RichText::new("Snap").size(10.0)).changed() {
            brush_settings.snap_enabled = snap;
        }
    }
}

/// Large tool button with icon only
fn large_tool_button(
    ui: &mut egui::Ui,
    icon: &str,
    tooltip: &str,
    active: bool,
    accent_color: Color32,
    theme: &Theme,
) -> egui::Response {
    let size = Vec2::new(40.0, 40.0);

    let (bg_color, text_color) = if active {
        (accent_color, Color32::WHITE)
    } else {
        (theme.widgets.inactive_bg.to_color32(), theme.text.primary.to_color32())
    };

    let button = egui::Button::new(RichText::new(icon).size(20.0).color(text_color))
        .fill(bg_color)
        .min_size(size);

    ui.add(button).on_hover_text(tooltip)
}

/// Compact text button for sub-options
fn compact_button(
    ui: &mut egui::Ui,
    label: &str,
    active: bool,
    accent_color: Color32,
    theme: &Theme,
) -> egui::Response {
    let size = Vec2::new(ui.available_width().min(60.0), 22.0);

    let (bg_color, text_color) = if active {
        (accent_color, Color32::WHITE)
    } else {
        (theme.widgets.inactive_bg.to_color32(), theme.text.primary.to_color32())
    };

    let button = egui::Button::new(RichText::new(label).size(10.0).color(text_color))
        .fill(bg_color)
        .min_size(size);

    ui.add(button)
}
