use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, Pos2, Sense, Vec2, RichText};

use crate::core::{EditorSettings, SelectionState, HierarchyState, PlayModeState, PlayState, DockingState};
use crate::gizmo::GizmoState;
use crate::spawn::{self, Category};
use crate::plugin_core::PluginHost;
use crate::ui_api::UiEvent;
use crate::ui::docking::builtin_layouts;
use crate::theming::Theme;

// Phosphor icons for toolbar
use egui_phosphor::regular::{
    PLAY, PAUSE, STOP, GEAR, CUBE, LIGHTBULB, VIDEO_CAMERA, PLUS, CARET_DOWN, LAYOUT,
};

pub fn render_toolbar(
    ctx: &egui::Context,
    gizmo: &mut GizmoState,
    settings: &mut EditorSettings,
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
    theme: &Theme,
) -> Vec<UiEvent> {
    let mut events = Vec::new();
    let api = plugin_host.api();

    egui::TopBottomPanel::top("toolbar")
        .exact_height(toolbar_height)
        .frame(egui::Frame::NONE
            .fill(theme.surfaces.panel.to_color32())
            .stroke(egui::Stroke::new(1.0, theme.widgets.border.to_color32())))
        .show(ctx, |ui| {
            let available_width = ui.available_width();

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
                            let entity = (template.spawn)(commands, meshes, materials, None);
                            selection.selected_entity = Some(entity);
                            ui.close();
                        }
                    }
                });

                separator(ui, theme);

                // === Play Controls ===
                let play_color = theme.semantic.success.to_color32();
                let is_playing = play_mode.state == PlayState::Playing;
                let is_paused = play_mode.state == PlayState::Paused;
                let is_in_play_mode = play_mode.is_in_play_mode();

                // Play button - green when playing
                let play_resp = tool_button(ui, PLAY, button_size, is_playing, play_color, inactive_color);
                if play_resp.clicked() {
                    if is_paused {
                        // Resume from pause
                        play_mode.state = PlayState::Playing;
                    } else if !is_playing {
                        // Start playing
                        play_mode.request_play = true;
                    }
                }
                play_resp.on_hover_text(if is_paused { "Resume (F5)" } else { "Play (F5)" });

                // Pause button - active when paused
                let pause_resp = tool_button(ui, PAUSE, button_size, is_paused, active_color, inactive_color);
                if pause_resp.clicked() && is_playing {
                    play_mode.state = PlayState::Paused;
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
                let settings_resp = tool_button(ui, GEAR, button_size, settings.show_settings_window, active_color, inactive_color);
                if settings_resp.clicked() {
                    settings.show_settings_window = !settings.show_settings_window;
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
