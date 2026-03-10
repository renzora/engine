//! Terrain Tools panel — Sculpt tab with tool grid, settings, brush config.

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CursorIcon, RichText, Rounding, Vec2};
use egui_phosphor::regular::{
    MOUNTAINS, WAVES, EQUALS, ARROW_FAT_LINE_UP, TREE, DROP, WAVEFORM,
    STAIRS, ARROWS_IN_CARDINAL, ACTIVITY, GRAPH, CHART_BAR,
    CIRCLE, SQUARE, DIAMOND, CARET_DOWN, CARET_RIGHT,
    ARROWS_OUT_CARDINAL, ERASER,
};

use renzora_terrain::data::*;
use renzora_theme::{Theme, ThemeManager};
use renzora_editor::EditorCommands;
use renzora_ui::EditorPanel;

use renzora_terrain::data::TerrainToolState;

// ── Panel ────────────────────────────────────────────────────────────────────

struct PanelState {
    section_tool_settings: bool,
    section_brush_settings: bool,
}

impl Default for PanelState {
    fn default() -> Self {
        Self {
            section_tool_settings: true,
            section_brush_settings: true,
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

    fn default_location(&self) -> renzora_ui::PanelLocation {
        renzora_ui::PanelLocation::Left
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
        let toggle_label = if active { "Sculpt Mode Active" } else { "Enable Sculpt Mode" };
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
                RichText::new("Select a terrain entity and enable sculpt mode to begin editing.")
                    .color(text_secondary)
                    .size(11.0),
            );
            return;
        }

        ui.add_space(8.0);

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
        });
    }
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

        labeled_drag(ui, "Scale", label_width, text_secondary, &mut scale, 0.5, 1.0..=500.0);
        labeled_drag(ui, "Octaves", label_width, text_secondary, &mut octaves, 0.1, 1..=8);
        labeled_drag(ui, "Lacunarity", label_width, text_secondary, &mut lac, 0.05, 1.0..=4.0);
        labeled_drag(ui, "Persistence", label_width, text_secondary, &mut pers, 0.01, 0.1..=0.9);
        labeled_drag(ui, "Seed", label_width, text_secondary, &mut seed, 1.0, 0..=u32::MAX);

        if scale != settings.noise_scale
            || octaves != settings.noise_octaves
            || lac != settings.noise_lacunarity
            || pers != settings.noise_persistence
            || seed != settings.noise_seed
        {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_resource_mut::<TerrainSettings>() {
                    s.noise_scale = scale;
                    s.noise_octaves = octaves;
                    s.noise_lacunarity = lac;
                    s.noise_persistence = pers;
                    s.noise_seed = seed;
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
