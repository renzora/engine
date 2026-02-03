//! Level Tools panel for terrain editing
//!
//! Provides Unreal Engine-style terrain tools with Sculpt and Paint tabs.

use bevy_egui::egui::{self, Color32, CursorIcon, RichText, Vec2, Rounding};
use egui_phosphor::regular::{
    SELECTION, ARROWS_OUT_CARDINAL, PAINT_BRUSH, PENCIL, MOUNTAINS,
    WAVES, EQUALS, ARROW_FAT_LINE_UP, TREE,
    DROP, WAVEFORM, GRAPH, EYE, ARROWS_HORIZONTAL, CURSOR, COPY,
    CARET_DOWN, CARET_RIGHT, CIRCLE, SQUARE, DIAMOND,
};

use crate::brushes::{BrushSettings, BrushType, BlockEditState};
use crate::gizmo::{EditorTool, GizmoState};
use crate::terrain::{TerrainBrushType, TerrainSettings, TerrainTab, BrushShape, BrushFalloffType, FlattenMode};
use crate::theming::Theme;

/// Render the Level Tools panel content
pub fn render_level_tools_content(
    ui: &mut egui::Ui,
    gizmo_state: &mut GizmoState,
    brush_settings: &mut BrushSettings,
    _block_edit: &BlockEditState,
    terrain_settings: &mut TerrainSettings,
    theme: &Theme,
) {
    let terrain_active = gizmo_state.tool == EditorTool::TerrainSculpt;

    if terrain_active {
        render_terrain_panel(ui, terrain_settings, theme);
    } else {
        render_standard_tools(ui, gizmo_state, brush_settings, theme);
    }
}

/// Render Unreal Engine style terrain panel
fn render_terrain_panel(
    ui: &mut egui::Ui,
    settings: &mut TerrainSettings,
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
        }

        // Paint tab
        let paint_selected = settings.tab == TerrainTab::Paint;
        if tab_button(ui, "Paint", paint_selected, tab_width, accent, theme).clicked() {
            settings.tab = TerrainTab::Paint;
        }
    });

    ui.add_space(8.0);

    // === Tool Grid ===
    egui::ScrollArea::vertical().show(ui, |ui| {
        if settings.tab == TerrainTab::Sculpt {
            render_sculpt_tools(ui, settings, accent, theme);
        } else {
            render_paint_tools(ui, settings, text_secondary, theme);
        }

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
    });
}

/// Render sculpt tool grid
fn render_sculpt_tools(
    ui: &mut egui::Ui,
    settings: &mut TerrainSettings,
    accent: Color32,
    theme: &Theme,
) {
    let tools: &[(TerrainBrushType, &str, &str)] = &[
        (TerrainBrushType::Sculpt, MOUNTAINS, "Sculpt"),
        (TerrainBrushType::Smooth, WAVES, "Smooth"),
        (TerrainBrushType::Flatten, EQUALS, "Flatten"),
        (TerrainBrushType::Ramp, ARROW_FAT_LINE_UP, "Ramp"),
        (TerrainBrushType::Erosion, TREE, "Erosion"),
        (TerrainBrushType::Hydro, DROP, "Hydro"),
        (TerrainBrushType::Noise, WAVEFORM, "Noise"),
        (TerrainBrushType::Retop, GRAPH, "Retop"),
        (TerrainBrushType::Visibility, EYE, "Visibility"),
        (TerrainBrushType::Mirror, ARROWS_HORIZONTAL, "Mirror"),
        (TerrainBrushType::Select, CURSOR, "Select"),
        (TerrainBrushType::Copy, COPY, "Copy"),
    ];

    render_tool_grid(ui, tools, &mut settings.brush_type, accent, theme);
}

/// Render paint tool grid
fn render_paint_tools(
    ui: &mut egui::Ui,
    _settings: &mut TerrainSettings,
    text_secondary: Color32,
    _theme: &Theme,
) {
    ui.vertical_centered(|ui| {
        ui.add_space(20.0);
        ui.label(RichText::new("Paint tools").color(text_secondary).size(12.0));
        ui.label(RichText::new("Coming soon...").color(text_secondary).size(10.0).weak());
        ui.add_space(20.0);
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
        // Flatten Mode
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(label_width, 20.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| { ui.label(RichText::new("Flatten Mode").color(text_secondary).size(11.0)); }
            );
            egui::ComboBox::from_id_salt("flatten_mode")
                .selected_text(match settings.flatten_mode {
                    FlattenMode::Both => "Both",
                    FlattenMode::Raise => "Raise",
                    FlattenMode::Lower => "Lower",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut settings.flatten_mode, FlattenMode::Both, "Both");
                    ui.selectable_value(&mut settings.flatten_mode, FlattenMode::Raise, "Raise");
                    ui.selectable_value(&mut settings.flatten_mode, FlattenMode::Lower, "Lower");
                });
        });

        // Use Slope Flatten
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(label_width, 20.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| { ui.label(RichText::new("Use Slope Flatten").color(text_secondary).size(11.0)); }
            );
            ui.checkbox(&mut settings.use_slope_flatten, "");
        });

        // Flatten Target
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(label_width, 20.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| { ui.label(RichText::new("Flatten Target").color(text_secondary).size(11.0)); }
            );
            ui.add(egui::DragValue::new(&mut settings.target_height)
                .speed(0.01)
                .range(0.0..=1.0));
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
