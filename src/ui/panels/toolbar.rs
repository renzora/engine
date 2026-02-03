use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, Pos2, Sense, Vec2, RichText};

use crate::core::{EditorSettings, SelectionState, HierarchyState, PlayModeState, PlayState, DockingState};
use crate::gizmo::{GizmoState, EditorTool};
use crate::brushes::{BrushSettings, BrushType};
use crate::terrain::{TerrainData, TerrainSettings, TerrainBrushType};
use crate::spawn::{self, Category};
use crate::plugin_core::PluginHost;
use crate::ui_api::UiEvent;
use crate::ui::docking::{builtin_layouts, PanelId};
use crate::theming::Theme;

// Phosphor icons for toolbar
use egui_phosphor::regular::{
    PLAY, PAUSE, STOP, GEAR, CUBE, LIGHTBULB, VIDEO_CAMERA, PLUS, CARET_DOWN, LAYOUT,
    SQUARE, FRAME_CORNERS, STACK, ARROW_FAT_LINE_UP,
    ARROW_UP, ARROW_DOWN, WAVES, MINUS, CROSSHAIR,
};

pub fn render_toolbar(
    ctx: &egui::Context,
    gizmo: &mut GizmoState,
    _settings: &mut EditorSettings,
    _menu_bar_height: f32,
    toolbar_height: f32,
    _window_width: f32,
    plugin_host: &PluginHost,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    selection: &mut SelectionState,
    hierarchy: &mut HierarchyState,
    play_mode: &mut PlayModeState,
    docking_state: &mut DockingState,
    brush_settings: &mut BrushSettings,
    terrain_settings: &mut TerrainSettings,
    terrain_selected: bool,
    theme: &Theme,
) -> Vec<UiEvent> {
    let mut events = Vec::new();
    let api = plugin_host.api();

    egui::TopBottomPanel::top("toolbar")
        .exact_height(toolbar_height)
        .frame(egui::Frame::NONE
            .fill(theme.surfaces.extreme.to_color32())
            .stroke(egui::Stroke::new(1.0, theme.widgets.border.to_color32())))
        .show(ctx, |ui| {
            let _available_width = ui.available_width();

            let button_size = Vec2::new(28.0, 24.0);

            // Horizontal layout with vertical centering
            ui.horizontal_centered(|ui| {
                let active_color = theme.semantic.accent.to_color32();
                let inactive_color = theme.widgets.inactive_bg.to_color32();

                // === Add Object Dropdowns ===
                let mesh_color = theme.categories.rendering.accent.to_color32();
                let light_color = theme.categories.lighting.accent.to_color32();
                let camera_color = theme.categories.camera.accent.to_color32();

                // Meshes dropdown
                dropdown_button(ui, CUBE, "Mesh", mesh_color, inactive_color, |ui| {
                    for template in spawn::templates_by_category(Category::Mesh) {
                        if menu_item(ui, template.name) {
                            let entity = (template.spawn)(commands, meshes, materials, None);
                            selection.selected_entity = Some(entity);
                            ui.close();
                        }
                    }
                });

                // Lights dropdown
                dropdown_button(ui, LIGHTBULB, "Light", light_color, inactive_color, |ui| {
                    for template in spawn::templates_by_category(Category::Light) {
                        if menu_item(ui, template.name) {
                            let entity = (template.spawn)(commands, meshes, materials, None);
                            selection.selected_entity = Some(entity);
                            ui.close();
                        }
                    }
                });

                // Camera dropdown
                dropdown_button(ui, VIDEO_CAMERA, "Camera", camera_color, inactive_color, |ui| {
                    for template in spawn::templates_by_category(Category::Camera) {
                        if menu_item(ui, template.name) {
                            let entity = (template.spawn)(commands, meshes, materials, None);
                            selection.selected_entity = Some(entity);
                            ui.close();
                        }
                    }
                });

                // More objects dropdown
                let more_color = theme.text.muted.to_color32();
                let section_label_color = theme.text.muted.to_color32();
                dropdown_button(ui, PLUS, "More", more_color, inactive_color, |ui| {
                    // 3D Nodes
                    ui.label(RichText::new("Nodes").small().color(section_label_color));
                    for template in spawn::templates_by_category(Category::Nodes3D) {
                        if menu_item(ui, template.name) {
                            let entity = (template.spawn)(commands, meshes, materials, None);
                            selection.selected_entity = Some(entity);
                            ui.close();
                        }
                    }

                    ui.separator();

                    // Physics
                    ui.label(RichText::new("Physics").small().color(section_label_color));
                    for template in spawn::templates_by_category(Category::Physics) {
                        if menu_item(ui, template.name) {
                            let entity = (template.spawn)(commands, meshes, materials, None);
                            selection.selected_entity = Some(entity);
                            ui.close();
                        }
                    }

                    ui.separator();

                    // Environment
                    ui.label(RichText::new("Environment").small().color(section_label_color));
                    for template in spawn::templates_by_category(Category::Environment) {
                        if menu_item(ui, template.name) {
                            let _entity = (template.spawn)(commands, meshes, materials, None);
                            ui.close();
                        }
                    }

                    ui.separator();

                    // Terrain
                    ui.label(RichText::new("Terrain").small().color(section_label_color));
                    for template in spawn::templates_by_category(Category::Terrain) {
                        if menu_item(ui, template.name) {
                            let _entity = (template.spawn)(commands, meshes, materials, None);
                            ui.close();
                        }
                    }
                });

                separator(ui, theme);

                // === Brush Tools ===
                let brush_color = Color32::from_rgb(100, 180, 220); // Light blue for brushes
                let in_brush_mode = gizmo.tool == EditorTool::Brush;

                // Block brush
                let block_active = in_brush_mode && brush_settings.selected_brush == BrushType::Block;
                let block_resp = tool_button(ui, CUBE, button_size, block_active, brush_color, inactive_color);
                if block_resp.clicked() {
                    gizmo.tool = EditorTool::Brush;
                    brush_settings.selected_brush = BrushType::Block;
                }
                block_resp.on_hover_text("Block Brush (B)");

                // Floor brush
                let floor_active = in_brush_mode && brush_settings.selected_brush == BrushType::Floor;
                let floor_resp = tool_button(ui, SQUARE, button_size, floor_active, brush_color, inactive_color);
                if floor_resp.clicked() {
                    gizmo.tool = EditorTool::Brush;
                    brush_settings.selected_brush = BrushType::Floor;
                }
                floor_resp.on_hover_text("Floor Brush");

                // Wall brush
                let wall_active = in_brush_mode && brush_settings.selected_brush == BrushType::Wall;
                let wall_resp = tool_button(ui, FRAME_CORNERS, button_size, wall_active, brush_color, inactive_color);
                if wall_resp.clicked() {
                    gizmo.tool = EditorTool::Brush;
                    brush_settings.selected_brush = BrushType::Wall;
                }
                wall_resp.on_hover_text("Wall Brush");

                // Stairs brush
                let stairs_active = in_brush_mode && brush_settings.selected_brush == BrushType::Stairs;
                let stairs_resp = tool_button(ui, STACK, button_size, stairs_active, brush_color, inactive_color);
                if stairs_resp.clicked() {
                    gizmo.tool = EditorTool::Brush;
                    brush_settings.selected_brush = BrushType::Stairs;
                }
                stairs_resp.on_hover_text("Stairs Brush");

                // Ramp brush
                let ramp_active = in_brush_mode && brush_settings.selected_brush == BrushType::Ramp;
                let ramp_resp = tool_button(ui, ARROW_FAT_LINE_UP, button_size, ramp_active, brush_color, inactive_color);
                if ramp_resp.clicked() {
                    gizmo.tool = EditorTool::Brush;
                    brush_settings.selected_brush = BrushType::Ramp;
                }
                ramp_resp.on_hover_text("Ramp Brush");

                // === Terrain Tools (shown when terrain is selected) ===
                if terrain_selected {
                    separator(ui, theme);

                    let terrain_color = Color32::from_rgb(120, 180, 100); // Green for terrain
                    let in_terrain_mode = gizmo.tool == EditorTool::TerrainSculpt;

                    // Raise brush
                    let raise_active = in_terrain_mode && terrain_settings.brush_type == TerrainBrushType::Raise;
                    let raise_resp = tool_button(ui, ARROW_UP, button_size, raise_active, terrain_color, inactive_color);
                    if raise_resp.clicked() {
                        gizmo.tool = EditorTool::TerrainSculpt;
                        terrain_settings.brush_type = TerrainBrushType::Raise;
                    }
                    raise_resp.on_hover_text("Raise Terrain (T)");

                    // Lower brush
                    let lower_active = in_terrain_mode && terrain_settings.brush_type == TerrainBrushType::Lower;
                    let lower_resp = tool_button(ui, ARROW_DOWN, button_size, lower_active, terrain_color, inactive_color);
                    if lower_resp.clicked() {
                        gizmo.tool = EditorTool::TerrainSculpt;
                        terrain_settings.brush_type = TerrainBrushType::Lower;
                    }
                    lower_resp.on_hover_text("Lower Terrain");

                    // Smooth brush
                    let smooth_active = in_terrain_mode && terrain_settings.brush_type == TerrainBrushType::Smooth;
                    let smooth_resp = tool_button(ui, WAVES, button_size, smooth_active, terrain_color, inactive_color);
                    if smooth_resp.clicked() {
                        gizmo.tool = EditorTool::TerrainSculpt;
                        terrain_settings.brush_type = TerrainBrushType::Smooth;
                    }
                    smooth_resp.on_hover_text("Smooth Terrain");

                    // Flatten brush
                    let flatten_active = in_terrain_mode && terrain_settings.brush_type == TerrainBrushType::Flatten;
                    let flatten_resp = tool_button(ui, MINUS, button_size, flatten_active, terrain_color, inactive_color);
                    if flatten_resp.clicked() {
                        gizmo.tool = EditorTool::TerrainSculpt;
                        terrain_settings.brush_type = TerrainBrushType::Flatten;
                    }
                    flatten_resp.on_hover_text("Flatten Terrain");

                    // Set Height brush
                    let set_active = in_terrain_mode && terrain_settings.brush_type == TerrainBrushType::SetHeight;
                    let set_resp = tool_button(ui, CROSSHAIR, button_size, set_active, terrain_color, inactive_color);
                    if set_resp.clicked() {
                        gizmo.tool = EditorTool::TerrainSculpt;
                        terrain_settings.brush_type = TerrainBrushType::SetHeight;
                    }
                    set_resp.on_hover_text("Set Height");

                    // Brush settings sliders
                    ui.add_space(8.0);

                    // Size slider
                    ui.add(
                        egui::Slider::new(&mut terrain_settings.brush_radius, 1.0..=50.0)
                            .text("Size")
                            .fixed_decimals(1)
                    );

                    // Strength slider
                    ui.add(
                        egui::Slider::new(&mut terrain_settings.brush_strength, 0.01..=1.0)
                            .text("Strength")
                            .fixed_decimals(2)
                    );

                    // Hardness toggle (falloff)
                    let hardness_label = if terrain_settings.falloff > 0.5 { "Soft" } else { "Hard" };
                    if ui.small_button(hardness_label).clicked() {
                        terrain_settings.falloff = if terrain_settings.falloff > 0.5 { 0.0 } else { 1.0 };
                    }
                }

                separator(ui, theme);

                // === Play Controls ===
                let play_color = theme.semantic.success.to_color32();
                let is_playing = play_mode.state == PlayState::Playing;
                let is_paused = play_mode.state == PlayState::Paused;
                let is_scripts_only = play_mode.is_scripts_only();
                let is_scripts_paused = play_mode.state == PlayState::ScriptsPaused;
                let is_in_play_mode = play_mode.is_in_play_mode();

                // Play dropdown button - green when playing
                let scripts_color = theme.semantic.accent.to_color32();
                let current_play_color = if is_scripts_only { scripts_color } else { play_color };
                play_dropdown(
                    ui,
                    PLAY,
                    button_size,
                    is_playing || is_scripts_only,
                    current_play_color,
                    inactive_color,
                    play_mode,
                    theme,
                );

                // Pause button - active when paused
                let any_paused = is_paused || is_scripts_paused;
                let pause_resp = tool_button(ui, PAUSE, button_size, any_paused, active_color, inactive_color);
                if pause_resp.clicked() {
                    if is_playing {
                        play_mode.state = PlayState::Paused;
                    } else if play_mode.state == PlayState::ScriptsOnly {
                        play_mode.state = PlayState::ScriptsPaused;
                    }
                }
                pause_resp.on_hover_text("Pause (F6)");

                // Stop button - only enabled during play mode
                let stop_color = if is_in_play_mode { theme.semantic.error.to_color32() } else { theme.text.disabled.to_color32() };
                let stop_resp = tool_button(ui, STOP, button_size, false, stop_color, inactive_color);
                if stop_resp.clicked() && is_in_play_mode {
                    play_mode.request_stop = true;
                }
                stop_resp.on_hover_text("Stop (Escape)");

                // === Plugin Toolbar Items ===
                if !api.toolbar_items.is_empty() {
                    separator(ui, theme);

                    for (item, _plugin_id) in &api.toolbar_items {
                        let resp = tool_button(ui, &item.icon, button_size, false, active_color, inactive_color);
                        if resp.clicked() {
                            events.push(UiEvent::ButtonClicked(crate::ui_api::UiId(item.id.0)));
                        }
                        resp.on_hover_text(&item.tooltip);
                    }
                }

                separator(ui, theme);

                // === Settings ===
                let settings_panel = PanelId::Settings;
                let settings_visible = docking_state.is_panel_visible(&settings_panel);
                let settings_resp = tool_button(ui, GEAR, button_size, settings_visible, active_color, inactive_color);
                if settings_resp.clicked() {
                    if settings_visible {
                        docking_state.close_panel(&settings_panel);
                    } else {
                        docking_state.open_panel(settings_panel);
                    }
                }
                settings_resp.on_hover_text("Settings");

                ui.add_space(4.0);

                // === Layout Dropdown ===
                let layout_color = theme.text.secondary.to_color32();
                let current_layout = docking_state.active_layout.clone();
                layout_dropdown(ui, LAYOUT, &current_layout, layout_color, inactive_color, docking_state);
            });
        });

    // Keep hierarchy reference alive
    let _ = hierarchy;

    events
}

fn separator(ui: &mut egui::Ui, theme: &Theme) {
    ui.add_space(8.0);
    let rect = ui.available_rect_before_wrap();
    ui.painter().line_segment(
        [Pos2::new(rect.left(), rect.top() + 6.0), Pos2::new(rect.left(), rect.bottom() - 6.0)],
        egui::Stroke::new(1.0, theme.widgets.border.to_color32()),
    );
    ui.add_space(8.0);
}

fn tool_button(
    ui: &mut egui::Ui,
    icon: &str,
    size: Vec2,
    active: bool,
    active_color: Color32,
    inactive_color: Color32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    if ui.is_rect_visible(rect) {
        let bg_color = if active {
            active_color
        } else if response.hovered() {
            // Use a slightly lighter inactive color for hover
            let [r, g, b, _] = inactive_color.to_array();
            Color32::from_rgb(r.saturating_add(15), g.saturating_add(15), b.saturating_add(18))
        } else {
            inactive_color
        };

        ui.painter().rect_filled(rect, CornerRadius::same(4), bg_color);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(14.0),
            Color32::WHITE,
        );
    }

    response
}

fn dropdown_button(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    icon_color: Color32,
    bg_color: Color32,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let button_id = ui.make_persistent_id(label);
    let size = Vec2::new(38.0, 24.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        let fill = if hovered {
            // Use a slightly lighter inactive color for hover
            let [r, g, b, _] = bg_color.to_array();
            Color32::from_rgb(r.saturating_add(15), g.saturating_add(15), b.saturating_add(18))
        } else {
            bg_color
        };

        ui.painter().rect_filled(rect, CornerRadius::same(4), fill);

        // Icon
        ui.painter().text(
            Pos2::new(rect.left() + 12.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(13.0),
            icon_color,
        );

        // Caret - use muted text color
        let [r, g, b, _] = bg_color.to_array();
        let caret_color = Color32::from_rgb(r.saturating_add(90), g.saturating_add(90), b.saturating_add(95));
        ui.painter().text(
            Pos2::new(rect.right() - 10.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            CARET_DOWN,
            egui::FontId::proportional(10.0),
            caret_color,
        );
    }

    if response.clicked() {
        #[allow(deprecated)]
        ui.memory_mut(|mem| mem.toggle_popup(button_id));
    }

    #[allow(deprecated)]
    egui::popup_below_widget(
        ui,
        button_id,
        &response,
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_min_width(120.0);
            ui.style_mut().spacing.item_spacing.y = 2.0;
            add_contents(ui);
        },
    );

    response.on_hover_text(label);
}

fn menu_item(ui: &mut egui::Ui, label: &str) -> bool {
    let response = ui.add(
        egui::Button::new(label)
            .fill(Color32::TRANSPARENT)
            .corner_radius(CornerRadius::same(2))
            .min_size(Vec2::new(ui.available_width(), 0.0))
    );
    response.clicked()
}

fn layout_dropdown(
    ui: &mut egui::Ui,
    icon: &str,
    current_layout: &str,
    icon_color: Color32,
    bg_color: Color32,
    docking_state: &mut DockingState,
) {
    let button_id = ui.make_persistent_id("layout_dropdown");
    let size = Vec2::new(90.0, 24.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        let fill = if hovered {
            // Use a slightly lighter inactive color for hover
            let [r, g, b, _] = bg_color.to_array();
            Color32::from_rgb(r.saturating_add(15), g.saturating_add(15), b.saturating_add(18))
        } else {
            bg_color
        };

        ui.painter().rect_filled(rect, CornerRadius::same(4), fill);

        // Icon
        ui.painter().text(
            Pos2::new(rect.left() + 12.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(13.0),
            icon_color,
        );

        // Layout name (truncated if needed)
        let text = if current_layout.len() > 8 {
            format!("{}...", &current_layout[..6])
        } else {
            current_layout.to_string()
        };
        // Text color - lighter than background
        let [r, g, b, _] = bg_color.to_array();
        let text_color = Color32::from_rgb(r.saturating_add(155), g.saturating_add(155), b.saturating_add(155));
        ui.painter().text(
            Pos2::new(rect.left() + 26.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            text,
            egui::FontId::proportional(11.0),
            text_color,
        );

        // Caret - use muted text color
        let caret_color = Color32::from_rgb(r.saturating_add(90), g.saturating_add(90), b.saturating_add(95));
        ui.painter().text(
            Pos2::new(rect.right() - 10.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            CARET_DOWN,
            egui::FontId::proportional(10.0),
            caret_color,
        );
    }

    if response.clicked() {
        #[allow(deprecated)]
        ui.memory_mut(|mem| mem.toggle_popup(button_id));
    }

    #[allow(deprecated)]
    egui::popup_below_widget(
        ui,
        button_id,
        &response,
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_min_width(140.0);
            ui.style_mut().spacing.item_spacing.y = 2.0;

            for layout in builtin_layouts() {
                let is_selected = docking_state.active_layout == layout.name;
                let label = if is_selected {
                    format!("• {}", layout.name)
                } else {
                    format!("  {}", layout.name)
                };

                if ui.add(
                    egui::Button::new(&label)
                        .fill(Color32::TRANSPARENT)
                        .corner_radius(CornerRadius::same(2))
                        .min_size(Vec2::new(ui.available_width(), 0.0))
                ).clicked() {
                    docking_state.switch_layout(&layout.name);
                    ui.close();
                }
            }
        },
    );

    response.on_hover_text("Layout");
}

fn play_dropdown(
    ui: &mut egui::Ui,
    icon: &str,
    size: Vec2,
    active: bool,
    active_color: Color32,
    inactive_color: Color32,
    play_mode: &mut PlayModeState,
    theme: &Theme,
) {
    let button_id = ui.make_persistent_id("play_dropdown");
    let total_size = Vec2::new(size.x + 14.0, size.y); // Extra width for dropdown arrow
    let (rect, response) = ui.allocate_exact_size(total_size, Sense::hover());

    // Define sub-areas
    let main_rect = egui::Rect::from_min_max(rect.min, Pos2::new(rect.right() - 14.0, rect.max.y));
    let dropdown_rect = egui::Rect::from_min_max(Pos2::new(rect.right() - 14.0, rect.min.y), rect.max);

    // Check hover on each area
    let main_response = ui.interact(main_rect, button_id.with("main"), Sense::click());
    let dropdown_response = ui.interact(dropdown_rect, button_id.with("dropdown"), Sense::click());

    let is_hovered = response.hovered() || main_response.hovered() || dropdown_response.hovered();

    if ui.is_rect_visible(rect) {
        // Background color based on state
        let bg_color = if active {
            active_color
        } else if is_hovered {
            let [r, g, b, _] = inactive_color.to_array();
            Color32::from_rgb(r.saturating_add(20), g.saturating_add(20), b.saturating_add(25))
        } else {
            inactive_color
        };

        ui.painter().rect_filled(rect, CornerRadius::same(4), bg_color);

        // Highlight main area on hover
        if main_response.hovered() && !active {
            ui.painter().rect_filled(
                main_rect.shrink(1.0),
                CornerRadius::same(3),
                Color32::from_white_alpha(15),
            );
        }

        // Highlight dropdown area on hover
        if dropdown_response.hovered() && !active {
            ui.painter().rect_filled(
                dropdown_rect.shrink(1.0),
                CornerRadius::same(3),
                Color32::from_white_alpha(15),
            );
        }

        // Main icon
        ui.painter().text(
            Pos2::new(rect.left() + 14.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(14.0),
            Color32::WHITE,
        );

        // Divider line
        let divider_x = rect.right() - 14.0;
        ui.painter().line_segment(
            [
                Pos2::new(divider_x, rect.top() + 4.0),
                Pos2::new(divider_x, rect.bottom() - 4.0),
            ],
            egui::Stroke::new(1.0, Color32::from_white_alpha(40)),
        );

        // Dropdown arrow
        ui.painter().text(
            Pos2::new(rect.right() - 7.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            CARET_DOWN,
            egui::FontId::proportional(9.0),
            Color32::from_white_alpha(180),
        );
    }

    // Handle main button click - quick play
    if main_response.clicked() {
        if play_mode.state == PlayState::Paused {
            play_mode.state = PlayState::Playing;
        } else if play_mode.state == PlayState::ScriptsPaused {
            play_mode.state = PlayState::ScriptsOnly;
        } else if play_mode.is_editing() {
            play_mode.request_play = true;
        }
    }

    // Handle dropdown click - show menu
    if dropdown_response.clicked() {
        #[allow(deprecated)]
        ui.memory_mut(|mem| mem.toggle_popup(button_id));
    }

    #[allow(deprecated)]
    egui::popup_below_widget(
        ui,
        button_id,
        &response,
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_min_width(160.0);
            ui.style_mut().spacing.item_spacing.y = 2.0;

            let play_icon_color = theme.semantic.success.to_color32();
            let scripts_icon_color = theme.semantic.accent.to_color32();

            // Play (Fullscreen)
            if play_menu_item(ui, PLAY, "Play (Fullscreen)", "F5", play_icon_color) {
                if play_mode.is_editing() {
                    play_mode.request_play = true;
                }
                ui.close();
            }

            // Run Scripts Only
            if play_menu_item(ui, PLAY, "Run Scripts", "Shift+F5", scripts_icon_color) {
                if play_mode.is_editing() {
                    play_mode.request_scripts_only = true;
                }
                ui.close();
            }
        },
    );

    // Tooltip
    let tooltip = if play_mode.is_paused() || play_mode.state == PlayState::ScriptsPaused {
        "Resume"
    } else if play_mode.is_in_play_mode() {
        "Playing..."
    } else {
        "Play (click arrow for options)"
    };
    response.on_hover_text(tooltip);
}

fn play_menu_item(ui: &mut egui::Ui, icon: &str, label: &str, shortcut: &str, icon_color: Color32) -> bool {
    let response = ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.colored_label(icon_color, icon);
        ui.label(label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(shortcut).small().weak());
        });
    }).response;

    let clicked = response.interact(Sense::click()).clicked();
    if response.hovered() {
        ui.painter().rect_filled(
            response.rect,
            CornerRadius::same(2),
            Color32::from_white_alpha(15),
        );
    }
    clicked
}
