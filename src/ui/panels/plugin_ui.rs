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
        .filter(|(loc, _, _)| *loc == MenuLocation::File)
        .map(|(_, item, _)| item)
        .collect();

    let edit_items: Vec<_> = api
        .menu_items
        .iter()
        .filter(|(loc, _, _)| *loc == MenuLocation::Edit)
        .map(|(_, item, _)| item)
        .collect();

    let view_items: Vec<_> = api
        .menu_items
        .iter()
        .filter(|(loc, _, _)| *loc == MenuLocation::View)
        .map(|(_, item, _)| item)
        .collect();

    let tools_items: Vec<_> = api
        .menu_items
        .iter()
        .filter(|(loc, _, _)| *loc == MenuLocation::Tools)
        .map(|(_, item, _)| item)
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

    for (panel, _plugin_id) in &api.panels {
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

    for (item, _plugin_id) in &api.toolbar_items {
        let button = egui::Button::new(&item.icon);
        let response = ui.add(button).on_hover_text(&item.tooltip);

        if response.clicked() {
            events.push(UiEvent::ButtonClicked(convert_ui_id(item.id)));
        }
    }

    events
}

/// Render the status bar at the bottom of the screen
pub fn render_status_bar(ctx: &egui::Context, plugin_host: &PluginHost) {
    use crate::plugin_core::StatusBarAlign;

    let api = plugin_host.api();

    // Don't render if no status items
    if api.status_bar_items.is_empty() {
        return;
    }

    // Collect and sort items by alignment and priority
    let mut left_items: Vec<_> = api.status_bar_items.values()
        .map(|(item, _plugin_id)| item)
        .filter(|item| item.align == StatusBarAlign::Left)
        .collect();
    let mut right_items: Vec<_> = api.status_bar_items.values()
        .map(|(item, _plugin_id)| item)
        .filter(|item| item.align == StatusBarAlign::Right)
        .collect();

    // Sort by priority (lower priority = closer to edge)
    left_items.sort_by_key(|item| item.priority);
    right_items.sort_by_key(|item| -item.priority); // Reverse for right side

    egui::TopBottomPanel::bottom("status_bar")
        .exact_height(22.0)
        .frame(egui::Frame::NONE
            .fill(Color32::from_rgb(30, 30, 36))
            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(50, 50, 58))))
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;

                // Left-aligned items
                for item in &left_items {
                    render_status_item(ui, item);
                }

                // Spacer to push right items to the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;

                    // Right-aligned items (reversed order for right-to-left layout)
                    for item in right_items.iter().rev() {
                        render_status_item(ui, item);
                    }
                });
            });
        });

    fn render_status_item(ui: &mut egui::Ui, item: &crate::plugin_core::StatusBarItem) {
        let text_color = Color32::from_rgb(180, 180, 190);

        // Build display text with icon if present
        let display_text = if let Some(icon) = &item.icon {
            format!("{} {}", icon, item.text)
        } else {
            item.text.clone()
        };

        let label = ui.label(RichText::new(&display_text).size(11.0).color(text_color));
        if let Some(tooltip) = &item.tooltip {
            label.on_hover_text(tooltip);
        }
    }
}
