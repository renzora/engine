mod assets;
mod blueprint;
mod console;
mod document_tabs;
mod export;
mod gamepad;
mod hierarchy;
mod inspector;
mod material_preview;
mod node_library;
mod plugin_ui;
mod script_editor;
mod settings;
mod splash;
mod syntax_highlight;
mod title_bar;
mod toolbar;
mod viewport;

pub use assets::{render_assets_content, render_assets_dialogs};
pub use blueprint::render_blueprint_panel;
pub use material_preview::render_material_preview_content;
pub use node_library::render_node_library_panel;
pub use console::render_console_content;
pub use gamepad::render_gamepad_content;
pub use document_tabs::render_document_tabs;
pub use export::render_export_dialog;
pub use hierarchy::HierarchyQueries;
pub use hierarchy::render_hierarchy_content;
pub use inspector::{InspectorQueries, property_row, inline_property, LABEL_WIDTH};
pub use inspector::render_inspector_content;
pub use inspector::render_history_content;
pub use plugin_ui::{render_plugin_panels, render_status_bar};
pub(crate) use script_editor::render_script_editor;
pub use script_editor::render_script_editor_content;
pub use script_editor::open_script;
pub use settings::render_settings_content;
pub use splash::render_splash;
pub use title_bar::{render_title_bar, handle_window_actions, TITLE_BAR_HEIGHT};
pub use toolbar::render_toolbar;
pub use viewport::render_viewport;

use bevy_egui::egui::{self, Color32, Vec2};

/// Panel bar height constant
#[allow(dead_code)]
pub const PANEL_BAR_HEIGHT: f32 = 24.0;

/// Renders a panel bar with an action button on the right side
#[allow(dead_code)]
pub fn render_panel_bar_with_action(
    ui: &mut egui::Ui,
    icon: &str,
    title: &str,
    action_icon: &str,
    action_color: Color32,
) -> (egui::Response, bool) {
    let available_width = ui.available_width();
    let _bar_rect = egui::Rect::from_min_size(
        ui.cursor().min,
        Vec2::new(available_width, PANEL_BAR_HEIGHT),
    );

    // Allocate the full bar
    let (rect, bar_response) = ui.allocate_exact_size(
        Vec2::new(available_width, PANEL_BAR_HEIGHT),
        egui::Sense::hover(),
    );

    // Draw bar background
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius::ZERO,
        Color32::from_rgb(38, 40, 46),
    );

    // Draw bottom border
    ui.painter().line_segment(
        [
            egui::pos2(rect.min.x, rect.max.y),
            egui::pos2(rect.max.x, rect.max.y),
        ],
        egui::Stroke::new(1.0, Color32::from_rgb(50, 52, 58)),
    );

    // Draw icon and title
    let text = format!("{} {}", icon, title);
    ui.painter().text(
        egui::pos2(rect.min.x + 10.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        egui::FontId::proportional(12.0),
        Color32::from_rgb(180, 182, 190),
    );

    // Action button area (right side)
    let button_size = 20.0;
    let button_rect = egui::Rect::from_center_size(
        egui::pos2(rect.max.x - 16.0, rect.center().y),
        Vec2::splat(button_size),
    );

    let button_response = ui.interact(button_rect, ui.id().with("panel_action"), egui::Sense::click());
    let button_hovered = button_response.hovered();

    // Draw button background on hover
    if button_hovered {
        ui.painter().rect_filled(
            button_rect,
            4.0,
            Color32::from_rgb(55, 57, 65),
        );
    }

    // Draw button icon
    ui.painter().text(
        button_rect.center(),
        egui::Align2::CENTER_CENTER,
        action_icon,
        egui::FontId::proportional(14.0),
        if button_hovered { action_color } else { Color32::from_rgb(140, 142, 150) },
    );

    (bar_response, button_response.clicked())
}
