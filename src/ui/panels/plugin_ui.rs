//! Plugin UI rendering
//!
//! This module renders UI elements registered by plugins, including:
//! - Menu items
//! - Panels
//! - Toolbar items

use bevy_egui::egui::{self, Color32, RichText};

use crate::plugin_core::{MenuLocation, MenuItem, PanelDefinition, PluginHost};
use crate::ui_api::{renderer::UiRenderer, types::UiId, UiEvent, Widget};

/// Convert from editor_plugin_api UiId to internal UiId
fn convert_ui_id(id: editor_plugin_api::ui::UiId) -> UiId {
    UiId(id.0)
}

/// Render plugin-registered menu items in a menu bar
pub fn render_plugin_menus(ui: &mut egui::Ui, plugin_host: &PluginHost) -> Vec<UiEvent> {
    let mut events = Vec::new();
    let api = plugin_host.api();

    // Group menu items by location
    let file_items: Vec<_> = api
        .menu_items
        .iter()
        .filter(|(loc, _)| *loc == MenuLocation::File)
        .map(|(_, item)| item)
        .collect();

    let edit_items: Vec<_> = api
        .menu_items
        .iter()
        .filter(|(loc, _)| *loc == MenuLocation::Edit)
        .map(|(_, item)| item)
        .collect();

    let view_items: Vec<_> = api
        .menu_items
        .iter()
        .filter(|(loc, _)| *loc == MenuLocation::View)
        .map(|(_, item)| item)
        .collect();

    let tools_items: Vec<_> = api
        .menu_items
        .iter()
        .filter(|(loc, _)| *loc == MenuLocation::Tools)
        .map(|(_, item)| item)
        .collect();

    // Render plugin menu items in each menu
    if !file_items.is_empty() {
        ui.menu_button("File (Plugins)", |ui| {
            for item in &file_items {
                if render_menu_item(ui, item) {
                    events.push(UiEvent::ButtonClicked(convert_ui_id(item.id)));
                }
            }
        });
    }

    if !edit_items.is_empty() {
        ui.menu_button("Edit (Plugins)", |ui| {
            for item in &edit_items {
                if render_menu_item(ui, item) {
                    events.push(UiEvent::ButtonClicked(convert_ui_id(item.id)));
                }
            }
        });
    }

    if !view_items.is_empty() {
        ui.menu_button("View (Plugins)", |ui| {
            for item in &view_items {
                if render_menu_item(ui, item) {
                    events.push(UiEvent::ButtonClicked(convert_ui_id(item.id)));
                }
            }
        });
    }

    if !tools_items.is_empty() {
        ui.menu_button("Tools", |ui| {
            for item in &tools_items {
                if render_menu_item(ui, item) {
                    events.push(UiEvent::ButtonClicked(convert_ui_id(item.id)));
                }
            }
        });
    }

    events
}

/// Render a single menu item, returns true if clicked
fn render_menu_item(ui: &mut egui::Ui, item: &MenuItem) -> bool {
    if item.children.is_empty() {
        // Leaf item
        let mut text = item.label.clone();
        if let Some(shortcut) = &item.shortcut {
            text = format!("{}\t{}", text, shortcut);
        }

        let button = egui::Button::new(&text);
        let response = ui.add_enabled(item.enabled, button);

        if response.clicked() {
            ui.close();
            return true;
        }
    } else {
        // Submenu
        ui.menu_button(&item.label, |ui| {
            for child in &item.children {
                render_menu_item(ui, child);
            }
        });
    }

    false
}

/// Render plugin-registered panels
pub fn render_plugin_panels(
    ctx: &egui::Context,
    plugin_host: &PluginHost,
    renderer: &mut UiRenderer,
) -> Vec<UiEvent> {
    let api = plugin_host.api();

    for panel in &api.panels {
        render_panel(ctx, panel, &api.panel_contents, renderer);
    }

    // Collect events from renderer
    renderer.drain_events().collect()
}

fn render_panel(
    ctx: &egui::Context,
    panel: &PanelDefinition,
    contents: &std::collections::HashMap<String, Vec<Widget>>,
    renderer: &mut UiRenderer,
) {
    let widgets = contents.get(&panel.id);

    // Clone panel data needed for egui IDs (they require 'static or owned data)
    let panel_id = panel.id.clone();
    let panel_title = panel.title.clone();
    let panel_icon = panel.icon.clone();
    let min_size = panel.min_size;

    match panel.default_location {
        crate::plugin_core::PanelLocation::Left => {
            egui::SidePanel::left(panel_id)
                .min_width(min_size[0])
                .show(ctx, |ui| {
                    render_panel_content_inline(ui, &panel_title, panel_icon.as_deref(), widgets, renderer);
                });
        }
        crate::plugin_core::PanelLocation::Right => {
            egui::SidePanel::right(panel_id)
                .min_width(min_size[0])
                .show(ctx, |ui| {
                    render_panel_content_inline(ui, &panel_title, panel_icon.as_deref(), widgets, renderer);
                });
        }
        crate::plugin_core::PanelLocation::Bottom => {
            egui::TopBottomPanel::bottom(panel_id)
                .min_height(min_size[1])
                .show(ctx, |ui| {
                    render_panel_content_inline(ui, &panel_title, panel_icon.as_deref(), widgets, renderer);
                });
        }
        crate::plugin_core::PanelLocation::Floating | crate::plugin_core::PanelLocation::Center => {
            egui::Window::new(&panel_title)
                .id(egui::Id::new(&panel_id))
                .min_size(min_size)
                .show(ctx, |ui| {
                    if let Some(widgets) = widgets {
                        for widget in widgets {
                            renderer.render(ui, widget);
                        }
                    } else {
                        ui.label(RichText::new("No content").color(Color32::GRAY));
                    }
                });
        }
    }
}

fn render_panel_content_inline(
    ui: &mut egui::Ui,
    title: &str,
    icon: Option<&str>,
    widgets: Option<&Vec<Widget>>,
    renderer: &mut UiRenderer,
) {
    // Panel header
    ui.horizontal(|ui| {
        if let Some(icon) = icon {
            ui.label(RichText::new(icon).size(16.0));
        }
        ui.heading(title);
    });
    ui.separator();

    // Panel content
    egui::ScrollArea::vertical().show(ui, |ui| {
        if let Some(widgets) = widgets {
            for widget in widgets {
                renderer.render(ui, widget);
            }
        } else {
            ui.label(RichText::new("No content").color(Color32::GRAY));
        }
    });
}

/// Render plugin toolbar items
pub fn render_plugin_toolbar(ui: &mut egui::Ui, plugin_host: &PluginHost) -> Vec<UiEvent> {
    let mut events = Vec::new();
    let api = plugin_host.api();

    if api.toolbar_items.is_empty() {
        return events;
    }

    // Add separator before plugin items
    ui.separator();

    for item in &api.toolbar_items {
        let button = egui::Button::new(&item.icon);
        let response = ui.add(button).on_hover_text(&item.tooltip);

        if response.clicked() {
            events.push(UiEvent::ButtonClicked(convert_ui_id(item.id)));
        }
    }

    events
}
