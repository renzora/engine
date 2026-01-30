//! Level Tools panel for brush-based level design
//!
//! Provides UI for selecting brush types, quick sizes, and editing brush properties.

use bevy_egui::egui::{self, Color32, RichText, Vec2};
use egui_phosphor::regular::{SELECTION, ARROWS_OUT_CARDINAL, PAINT_BRUSH, PENCIL, MOUNTAINS};

use crate::brushes::{BrushSettings, BrushType, BlockEditState};
use crate::gizmo::{EditorTool, GizmoState};
use crate::terrain::{TerrainBrushType, TerrainSettings};
use crate::theming::Theme;

/// Render the Level Tools panel content
pub fn render_level_tools_content(
    ui: &mut egui::Ui,
    gizmo_state: &mut GizmoState,
    brush_settings: &mut BrushSettings,
    block_edit: &BlockEditState,
    terrain_settings: &mut TerrainSettings,
    theme: &Theme,
) {
    let text_color = theme.text.primary.to_color32();
    let secondary_text = theme.text.secondary.to_color32();
    let accent_color = theme.semantic.accent.to_color32();

    ui.add_space(4.0);

    // Tool Mode Section
    ui.label(RichText::new("Tool Mode").color(secondary_text).size(11.0));
    ui.add_space(2.0);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;

        let select_active = gizmo_state.tool == EditorTool::Select;
        let transform_active = gizmo_state.tool == EditorTool::Transform;
        let brush_active = gizmo_state.tool == EditorTool::Brush;
        let edit_active = gizmo_state.tool == EditorTool::BlockEdit;
        let terrain_active = gizmo_state.tool == EditorTool::TerrainSculpt;

        // Select button
        if tool_button(ui, SELECTION, "Select", select_active, accent_color, theme).clicked() {
            gizmo_state.tool = EditorTool::Select;
        }

        // Transform button
        if tool_button(ui, ARROWS_OUT_CARDINAL, "Transform", transform_active, accent_color, theme).clicked() {
            gizmo_state.tool = EditorTool::Transform;
        }

        // Brush button
        if tool_button(ui, PAINT_BRUSH, "Brush", brush_active, accent_color, theme).clicked() {
            gizmo_state.tool = EditorTool::Brush;
        }

        // Edit button
        if tool_button(ui, PENCIL, "Edit", edit_active, accent_color, theme).clicked() {
            gizmo_state.tool = EditorTool::BlockEdit;
        }

        // Terrain button
        if tool_button(ui, MOUNTAINS, "Terrain", terrain_active, accent_color, theme).clicked() {
            gizmo_state.tool = EditorTool::TerrainSculpt;
        }
    });

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    // Brush Type Section (only show when in brush mode)
    if gizmo_state.tool == EditorTool::Brush {
        ui.label(RichText::new("Brush Type").color(secondary_text).size(11.0));
        ui.add_space(2.0);

        // First row: Block, Floor, Wall
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;

            for brush_type in &[BrushType::Block, BrushType::Floor, BrushType::Wall] {
                let active = brush_settings.selected_brush == *brush_type;
                if brush_type_button(ui, *brush_type, active, accent_color, theme).clicked() {
                    brush_settings.selected_brush = *brush_type;
                }
            }
        });

        // Second row: Stairs, Ramp
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;

            for brush_type in &[BrushType::Stairs, BrushType::Ramp] {
                let active = brush_settings.selected_brush == *brush_type;
                if brush_type_button(ui, *brush_type, active, accent_color, theme).clicked() {
                    brush_settings.selected_brush = *brush_type;
                }
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        // Quick Sizes Section
        ui.label(RichText::new("Quick Sizes").color(secondary_text).size(11.0));
        ui.add_space(2.0);

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;

            let size_labels = ["1x1", "2x2", "4x4", "8x8"];
            for (i, label) in size_labels.iter().enumerate() {
                let (w, d) = brush_settings.quick_sizes[i];
                let is_current = (brush_settings.custom_size.x - w).abs() < 0.01
                    && (brush_settings.custom_size.z - d).abs() < 0.01;

                if size_button(ui, label, is_current, accent_color, theme).clicked() {
                    brush_settings.custom_size.x = w;
                    brush_settings.custom_size.z = d;
                }
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        // Grid Snap Section
        ui.label(RichText::new("Grid Snap").color(secondary_text).size(11.0));
        ui.add_space(2.0);

        ui.horizontal(|ui| {
            let mut snap = brush_settings.snap_enabled;
            if ui.checkbox(&mut snap, "Enabled").changed() {
                brush_settings.snap_enabled = snap;
            }

            ui.add_space(8.0);
            ui.label(RichText::new("Size:").color(text_color).size(12.0));

            let mut snap_size = brush_settings.snap_size;
            ui.add(egui::DragValue::new(&mut snap_size)
                .speed(0.1)
                .range(0.1..=10.0)
                .suffix(" m"));
            if snap_size != brush_settings.snap_size {
                brush_settings.snap_size = snap_size;
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        // Material Toggle
        ui.label(RichText::new("Material").color(secondary_text).size(11.0));
        ui.add_space(2.0);

        let mut use_checker = brush_settings.use_checkerboard;
        if ui.checkbox(&mut use_checker, "Use Checkerboard").changed() {
            brush_settings.use_checkerboard = use_checker;
        }
    }

    // Block Edit Info (only show when in edit mode)
    if gizmo_state.tool == EditorTool::BlockEdit {
        ui.label(RichText::new("Block Edit Mode").color(secondary_text).size(11.0));
        ui.add_space(4.0);

        if block_edit.active {
            ui.label(RichText::new("Editing brush").color(text_color).size(12.0));
            ui.add_space(2.0);
            ui.label(RichText::new("Drag handles to resize").color(secondary_text).size(11.0));
            ui.label(RichText::new("Hold Shift for symmetric").color(secondary_text).size(11.0));
        } else {
            ui.label(RichText::new("Select a brush to edit").color(secondary_text).size(12.0));
        }

        ui.add_space(8.0);

        // Dimensions display if editing
        if block_edit.active {
            ui.separator();
            ui.add_space(8.0);

            ui.label(RichText::new("Dimensions").color(secondary_text).size(11.0));
            ui.add_space(2.0);

            ui.horizontal(|ui| {
                ui.label(RichText::new("X:").color(text_color));
                ui.label(RichText::new(format!("{:.2}", block_edit.drag_start_dimensions.x)).color(accent_color));
                ui.add_space(8.0);
                ui.label(RichText::new("Y:").color(text_color));
                ui.label(RichText::new(format!("{:.2}", block_edit.drag_start_dimensions.y)).color(accent_color));
                ui.add_space(8.0);
                ui.label(RichText::new("Z:").color(text_color));
                ui.label(RichText::new(format!("{:.2}", block_edit.drag_start_dimensions.z)).color(accent_color));
            });
        }
    }

    // Terrain Sculpt Mode UI
    if gizmo_state.tool == EditorTool::TerrainSculpt {
        ui.label(RichText::new("Sculpt Brush").color(secondary_text).size(11.0));
        ui.add_space(2.0);

        // Brush type selection - first row
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;

            for brush_type in &[TerrainBrushType::Raise, TerrainBrushType::Lower, TerrainBrushType::Smooth] {
                let active = terrain_settings.brush_type == *brush_type;
                if terrain_brush_button(ui, *brush_type, active, accent_color, theme).clicked() {
                    terrain_settings.brush_type = *brush_type;
                }
            }
        });

        // Second row
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;

            for brush_type in &[TerrainBrushType::Flatten, TerrainBrushType::SetHeight] {
                let active = terrain_settings.brush_type == *brush_type;
                if terrain_brush_button(ui, *brush_type, active, accent_color, theme).clicked() {
                    terrain_settings.brush_type = *brush_type;
                }
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        // Brush settings
        ui.label(RichText::new("Brush Settings").color(secondary_text).size(11.0));
        ui.add_space(2.0);

        // Radius
        ui.horizontal(|ui| {
            ui.label(RichText::new("Radius:").color(text_color).size(12.0));
            ui.add(egui::DragValue::new(&mut terrain_settings.brush_radius)
                .speed(0.5)
                .range(1.0..=50.0)
                .suffix(" m"));
        });

        // Strength
        ui.horizontal(|ui| {
            ui.label(RichText::new("Strength:").color(text_color).size(12.0));
            ui.add(egui::DragValue::new(&mut terrain_settings.brush_strength)
                .speed(0.01)
                .range(0.01..=1.0));
        });

        // Target height (for Set Height tool)
        if terrain_settings.brush_type == TerrainBrushType::SetHeight {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Target:").color(text_color).size(12.0));
                ui.add(egui::DragValue::new(&mut terrain_settings.target_height)
                    .speed(0.01)
                    .range(0.0..=1.0));
            });
        }

        // Falloff
        ui.horizontal(|ui| {
            ui.label(RichText::new("Falloff:").color(text_color).size(12.0));
            let mut smooth = terrain_settings.falloff > 0.5;
            if ui.checkbox(&mut smooth, "Smooth").changed() {
                terrain_settings.falloff = if smooth { 1.0 } else { 0.0 };
            }
        });
    }

    // Instructions
    ui.add_space(16.0);
    ui.separator();
    ui.add_space(8.0);

    ui.label(RichText::new("Instructions").color(secondary_text).size(11.0));
    ui.add_space(2.0);

    let instructions = match gizmo_state.tool {
        EditorTool::Brush => "Click and drag in viewport to create geometry.\nShift+drag to adjust height.",
        EditorTool::BlockEdit => "Click handles to resize.\nShift+drag for symmetric resize.\nPress Escape to exit.",
        EditorTool::TerrainSculpt => "Click and drag on terrain to sculpt.\nUse brush settings to adjust effect.",
        _ => "Press B for brush tool.\nPress T for terrain sculpt.\nSelect a brush entity and press E to edit.",
    };

    ui.label(RichText::new(instructions).color(secondary_text).size(10.0).weak());
}

/// Render a tool selection button
fn tool_button(
    ui: &mut egui::Ui,
    icon: &str,
    tooltip: &str,
    active: bool,
    accent_color: Color32,
    theme: &Theme,
) -> egui::Response {
    let size = Vec2::new(32.0, 28.0);

    let (bg_color, text_color) = if active {
        (accent_color, Color32::WHITE)
    } else {
        (theme.widgets.inactive_bg.to_color32(), theme.text.primary.to_color32())
    };

    let button = egui::Button::new(RichText::new(icon).size(14.0).color(text_color))
        .fill(bg_color)
        .min_size(size);

    ui.add(button).on_hover_text(tooltip)
}

/// Render a brush type selection button
fn brush_type_button(
    ui: &mut egui::Ui,
    brush_type: BrushType,
    active: bool,
    accent_color: Color32,
    theme: &Theme,
) -> egui::Response {
    let size = Vec2::new(50.0, 24.0);

    let (bg_color, text_color) = if active {
        (accent_color, Color32::WHITE)
    } else {
        (theme.widgets.inactive_bg.to_color32(), theme.text.primary.to_color32())
    };

    let label = brush_type.display_name();
    let button = egui::Button::new(RichText::new(label).size(11.0).color(text_color))
        .fill(bg_color)
        .min_size(size);

    ui.add(button)
}

/// Render a quick size button
fn size_button(
    ui: &mut egui::Ui,
    label: &str,
    active: bool,
    accent_color: Color32,
    theme: &Theme,
) -> egui::Response {
    let size = Vec2::new(36.0, 22.0);

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

/// Render a terrain brush type selection button
fn terrain_brush_button(
    ui: &mut egui::Ui,
    brush_type: TerrainBrushType,
    active: bool,
    accent_color: Color32,
    theme: &Theme,
) -> egui::Response {
    let size = Vec2::new(50.0, 24.0);

    let (bg_color, text_color) = if active {
        (accent_color, Color32::WHITE)
    } else {
        (theme.widgets.inactive_bg.to_color32(), theme.text.primary.to_color32())
    };

    let label = brush_type.display_name();
    let button = egui::Button::new(RichText::new(label).size(11.0).color(text_color))
        .fill(bg_color)
        .min_size(size);

    ui.add(button)
}
