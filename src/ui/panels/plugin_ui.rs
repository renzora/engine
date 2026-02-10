//! Plugin UI rendering
//!
//! This module renders UI elements registered by plugins, including:
//! - Menu items
//! - Panels
//! - Toolbar items

#![allow(dead_code)]

use bevy_egui::egui::{self, Color32, RichText, CornerRadius};
use egui_phosphor::regular::{CARET_UP, DOWNLOAD_SIMPLE, PALETTE, WARNING};

use crate::core::{AssetLoadingProgress, format_bytes};
use crate::plugin_core::{MenuLocation, MenuItem, PanelDefinition, PluginHost};
use crate::ui_api::{renderer::UiRenderer, types::UiId, UiEvent, Widget};
use crate::theming::ThemeManager;
use crate::update::{UpdateState, UpdateDialogState};

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

/// Render plugin-registered panels as floating windows.
/// Panels that are docked in the dock tree are skipped (they render inline).
pub fn render_plugin_panels(
    ctx: &egui::Context,
    plugin_host: &PluginHost,
    renderer: &mut UiRenderer,
    docked_panel_ids: &std::collections::HashSet<String>,
) -> Vec<UiEvent> {
    let api = plugin_host.api();

    for (panel, _plugin_id) in &api.panels {
        if docked_panel_ids.contains(&panel.id) {
            continue;
        }
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

    // For now, all plugin panels render as floating windows
    // Docked panels (Left/Right/Bottom) would conflict with existing editor panels
    // Future: integrate plugin panels into the existing panel layout system

    let title = if let Some(icon) = &panel_icon {
        format!("{} {}", icon, panel_title)
    } else {
        panel_title.clone()
    };

    egui::Window::new(&title)
        .id(egui::Id::new(&panel_id))
        .default_size(min_size)
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
pub fn render_status_bar(
    ctx: &egui::Context,
    plugin_host: &PluginHost,
    loading_progress: &AssetLoadingProgress,
    theme_manager: &mut ThemeManager,
    update_state: &UpdateState,
    update_dialog: &mut UpdateDialogState,
) {
    // Clone theme data needed for the dropup before borrowing theme immutably
    let active_theme_name = theme_manager.active_theme_name.clone();
    let available_themes = theme_manager.available_themes.clone();

    let theme = &theme_manager.active_theme;
    use crate::plugin_core::StatusBarAlign;

    let api = plugin_host.api();

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

    let text_color = theme.text.secondary.to_color32();
    let accent_color = theme.semantic.accent.to_color32();
    let panel_fill = theme.surfaces.panel.to_color32();
    let border_color = theme.widgets.border.to_color32();
    let inactive_bg = theme.widgets.inactive_bg.to_color32();
    let hover_bg = theme.widgets.hovered_bg.to_color32();
    let muted_color = theme.text.muted.to_color32();
    let success_color = theme.semantic.success.to_color32();
    let error_color = theme.semantic.error.to_color32();

    // Check if a theme was selected last frame (via egui temp data)
    let theme_selection_id = egui::Id::new("status_bar_theme_selection");
    let pending: Option<String> = ctx.data(|d| d.get_temp::<String>(theme_selection_id));
    if let Some(name) = pending {
        ctx.data_mut(|d| d.remove_temp::<String>(theme_selection_id));
        theme_manager.load_theme(&name);
    }

    egui::TopBottomPanel::bottom("status_bar")
        .exact_height(22.0)
        .frame(egui::Frame::NONE
            .fill(panel_fill)
            .stroke(egui::Stroke::new(1.0, border_color)))
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;

                // Show asset loading progress if loading
                if loading_progress.loading {
                    // Calculate progress based on bytes (total bytes vs loaded bytes)
                    let progress = if loading_progress.total_bytes > 0 {
                        loading_progress.loaded_bytes as f32 / loading_progress.total_bytes as f32
                    } else if loading_progress.total > 0 {
                        loading_progress.loaded as f32 / loading_progress.total as f32
                    } else {
                        0.0
                    };

                    // Spinner icon using egui's built-in spinner
                    ui.add(egui::Spinner::new().size(14.0).color(accent_color));

                    // Download/loading icon
                    ui.label(RichText::new("⬇").size(12.0).color(accent_color));

                    // Progress text with file count and sizes
                    let remaining = loading_progress.total.saturating_sub(loading_progress.loaded);
                    let size_text = if loading_progress.total_bytes > 0 {
                        format!(
                            "Loading {} file{} ({} / {})",
                            remaining,
                            if remaining == 1 { "" } else { "s" },
                            format_bytes(loading_progress.loaded_bytes),
                            format_bytes(loading_progress.total_bytes)
                        )
                    } else {
                        format!(
                            "Loading {} file{}...",
                            remaining,
                            if remaining == 1 { "" } else { "s" }
                        )
                    };
                    ui.label(RichText::new(size_text).size(11.0).color(text_color));

                    // Progress bar based on total bytes
                    let bar_width = 120.0;
                    let bar_height = 6.0;
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(bar_width, bar_height), egui::Sense::hover());

                    // Background with rounded corners
                    ui.painter().rect_filled(rect, 3.0, inactive_bg);

                    // Fill with rounded corners
                    if progress > 0.0 {
                        let fill_width = rect.width() * progress.clamp(0.0, 1.0);
                        let fill_rect = egui::Rect::from_min_size(
                            rect.min,
                            egui::vec2(fill_width, rect.height()),
                        );
                        // Use accent color for progress fill
                        ui.painter().rect_filled(fill_rect, 3.0, accent_color);
                    }

                    ui.add_space(8.0);
                    ui.separator();

                    // Request repaint for animation
                    ctx.request_repaint();
                }

                // Left-aligned items
                for item in &left_items {
                    render_status_item(ui, item, text_color);
                }

                // Spacer to push right items to the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;

                    // Version text (rightmost)
                    ui.label(RichText::new("Renzora r1-alpha2").size(11.0).color(text_color));

                    // Theme picker dropup
                    let theme_popup_id = ui.id().with("theme_picker_popup");
                    let is_open: bool = ui.data(|d| d.get_temp::<bool>(theme_popup_id).unwrap_or(false));
                    let theme_btn = ui.add(
                        egui::Button::new(
                            RichText::new(format!("{} {} {}", PALETTE, &active_theme_name, CARET_UP))
                                .size(11.0)
                                .color(text_color),
                        )
                        .fill(Color32::TRANSPARENT)
                        .corner_radius(CornerRadius::same(3))
                        .min_size(egui::vec2(0.0, 18.0)),
                    );
                    // Draw border on left, bottom, right only (no top)
                    let r = theme_btn.rect;
                    let stroke = egui::Stroke::new(1.0, border_color);
                    ui.painter().line_segment([r.left_top(), r.left_bottom()], stroke);
                    ui.painter().line_segment([r.left_bottom(), r.right_bottom()], stroke);
                    ui.painter().line_segment([r.right_top(), r.right_bottom()], stroke);
                    if theme_btn.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if theme_btn.clicked() {
                        ui.data_mut(|d| d.insert_temp(theme_popup_id, !is_open));
                    }

                    // Render popup above the button
                    if is_open {
                        let btn_rect = theme_btn.rect;
                        let popup_area_id = theme_popup_id.with("area");
                        let area_resp = egui::Area::new(popup_area_id)
                            .order(egui::Order::Foreground)
                            .fixed_pos(egui::pos2(btn_rect.max.x, btn_rect.min.y))
                            .pivot(egui::Align2::RIGHT_BOTTOM)
                            .show(ui.ctx(), |ui| {
                                let mut frame = egui::Frame::popup(ui.style());
                                frame.stroke = egui::Stroke::NONE;
                                frame.show(ui, |ui| {
                                    ui.set_max_height(200.0);
                                    ui.set_max_width(160.0);
                                    ui.style_mut().spacing.item_spacing.y = 2.0;

                                    egui::ScrollArea::vertical().show(ui, |ui| {
                                        for name in &available_themes {
                                            let is_active = *name == active_theme_name;
                                            let label = format!("{} {}", PALETTE, name);

                                            let btn = ui.add_enabled(
                                                !is_active,
                                                egui::Button::new(&label)
                                                    .fill(Color32::TRANSPARENT)
                                                    .corner_radius(CornerRadius::same(2))
                                                    .min_size(egui::vec2(ui.available_width(), 0.0)),
                                            );
                                            if btn.hovered() && !is_active {
                                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                            }
                                            if btn.clicked() {
                                                ui.ctx().data_mut(|d| d.insert_temp::<String>(theme_selection_id, name.clone()));
                                                ui.data_mut(|d| d.insert_temp(theme_popup_id, false));
                                            }
                                        }
                                    });
                                });
                            });

                        // Close if clicked outside (but not on the toggle button)
                        let popup_rect = area_resp.response.rect;
                        if ui.input(|i| i.pointer.any_pressed()) {
                            let pointer_pos = ui.input(|i| i.pointer.interact_pos());
                            if let Some(pos) = pointer_pos {
                                if !popup_rect.contains(pos) && !btn_rect.contains(pos) {
                                    ui.data_mut(|d| d.insert_temp(theme_popup_id, false));
                                }
                            }
                        }
                    }

                    // Update available indicator
                    if let Some(ref result) = update_state.check_result {
                        if result.update_available {
                            let btn = egui::Button::new(
                                RichText::new(format!("{} Update Available", DOWNLOAD_SIMPLE))
                                    .size(11.0)
                                    .color(Color32::WHITE)
                            )
                            .fill(success_color)
                            .corner_radius(CornerRadius::same(3))
                            .min_size(egui::vec2(0.0, 18.0));

                            if ui.add(btn).on_hover_text("Click to view update details").clicked() {
                                update_dialog.open = true;
                            }

                            ui.separator();
                        }
                    } else if update_state.checking {
                        // Show checking indicator
                        ui.add(egui::Spinner::new().size(12.0).color(accent_color));
                        ui.label(RichText::new("Checking...").size(11.0).color(text_color));
                        ui.separator();
                        ctx.request_repaint();
                    } else if let Some(ref err) = update_state.error {
                        // Show error indicator
                        ui.label(RichText::new(WARNING).size(11.0).color(error_color))
                            .on_hover_text(format!("Update check failed: {}", err));
                        ui.separator();
                    }

                    // Right-aligned items (reversed order for right-to-left layout)
                    for item in right_items.iter().rev() {
                        render_status_item(ui, item, text_color);
                    }
                });
            });
        });


    fn render_status_item(ui: &mut egui::Ui, item: &crate::plugin_core::StatusBarItem, text_color: Color32) {
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
