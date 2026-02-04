#![allow(dead_code)]

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, RichText, Sense, Vec2};
use std::path::PathBuf;

use crate::core::{
    AssetBrowserState, AssetViewMode, BottomPanelTab, ColliderImportType, ConsoleState,
    ConvertAxes, LogLevel, MeshHandling, NormalImportMethod, TangentImportMethod,
    ViewportState, SceneManagerState, ThumbnailCache, supports_thumbnail, supports_model_preview,
};
use crate::viewport::ModelPreviewCache;
use crate::plugin_core::{PluginHost, TabLocation};
use crate::project::CurrentProject;
use crate::ui_api::{UiEvent, renderer::UiRenderer};
use crate::theming::Theme;
use super::console::render_console_content;

// Icon constants from phosphor
use egui_phosphor::regular::{
    FOLDER, FILE, IMAGE, CUBE, SPEAKER_HIGH, FILE_RS, FILE_TEXT,
    GEAR, FILM_SCRIPT, FILE_CODE, DOWNLOAD, SCROLL, FOLDER_PLUS, CARET_RIGHT,
    MAGNIFYING_GLASS, LIST, SQUARES_FOUR, ARROW_LEFT, HOUSE, FOLDER_OPEN, TERMINAL,
    PLUS, X, CHECK, CARET_UP, CARET_DOWN, SUN, PALETTE, CODE, ATOM, PAINT_BRUSH,
    STACK, NOTE, MUSIC_NOTES, VIDEO, BLUEPRINT,
};

const MIN_TILE_SIZE: f32 = 64.0;
const MAX_TILE_SIZE: f32 = 128.0;
const DEFAULT_TILE_SIZE: f32 = 80.0;
const LIST_ROW_HEIGHT: f32 = 20.0;

/// Render the bottom panel with tabs (Assets + Console + Plugin tabs)
pub fn render_assets(
    ctx: &egui::Context,
    current_project: Option<&CurrentProject>,
    viewport: &mut ViewportState,
    assets: &mut AssetBrowserState,
    scene_state: &mut SceneManagerState,
    console: &mut ConsoleState,
    _left_panel_width: f32,
    _right_panel_width: f32,
    _bottom_panel_height: f32,
    plugin_host: &PluginHost,
    ui_renderer: &mut UiRenderer,
    thumbnail_cache: &mut ThumbnailCache,
    model_preview_cache: &mut ModelPreviewCache,
    theme: &Theme,
) -> Vec<UiEvent> {
    let mut ui_events = Vec::new();
    let screen_height = ctx.content_rect().height();
    // Ensure bottom panel doesn't exceed safe limits (max 50% of screen or 400px)
    let max_height = (screen_height * 0.5).min(400.0).max(100.0);
    let bar_height = 24.0;

    // Determine actual panel height based on minimized state
    let panel_height = if viewport.bottom_panel_minimized {
        bar_height
    } else {
        viewport.assets_height
    };

    // Get plugin tabs for bottom panel
    let api = plugin_host.api();
    let plugin_tabs = api.get_tabs_for_location(TabLocation::Bottom);
    let active_plugin_tab = api.get_active_tab(TabLocation::Bottom);

    // Theme colors for panel
    let panel_bg = theme.surfaces.panel.to_color32();
    let bar_bg = theme.widgets.noninteractive_bg.to_color32();
    let border_color = theme.widgets.border.to_color32();
    let tab_selected = theme.panels.tab_active.to_color32();
    let tab_hover = theme.panels.tab_hover.to_color32();
    let text_active = theme.text.primary.to_color32();
    let text_inactive = theme.text.muted.to_color32();
    let error_color = theme.semantic.error.to_color32();
    let warning_color = theme.semantic.warning.to_color32();
    let toggle_hover_bg = theme.widgets.hovered_bg.to_color32();

    let panel_response = egui::TopBottomPanel::bottom("bottom_panel")
        .exact_height(panel_height)
        .show_separator_line(false)
        .frame(egui::Frame::new().fill(panel_bg).inner_margin(egui::Margin::ZERO))
        .show(ctx, |ui| {
            // Panel bar with tabs (resize handle integrated at top edge)
            let resize_zone = 4.0; // Invisible resize zone at top of bar
            let available_width = ui.available_width();
            let (bar_rect, _) = ui.allocate_exact_size(
                egui::Vec2::new(available_width, bar_height),
                Sense::hover(),
            );

            // Resize handle - always available (expands panel if minimized)
            let resize_rect = egui::Rect::from_min_size(
                bar_rect.min,
                egui::Vec2::new(available_width, resize_zone),
            );
            let resize_response = ui.interact(resize_rect, ui.id().with("bottom_resize"), Sense::drag());

            if resize_response.hovered() || resize_response.dragged() {
                ctx.set_cursor_icon(CursorIcon::ResizeVertical);
            }

            // Use pointer position for smooth resizing
            if resize_response.dragged() {
                if let Some(pointer_pos) = ctx.pointer_interact_pos() {
                    let new_height = screen_height - pointer_pos.y;
                    viewport.assets_height = new_height.clamp(bar_height, max_height);
                    // Auto-expand if dragging while minimized
                    if viewport.bottom_panel_minimized && new_height > bar_height + 10.0 {
                        viewport.bottom_panel_minimized = false;
                    }
                }
            }

            // Draw bar background
            ui.painter().rect_filled(
                bar_rect,
                CornerRadius::ZERO,
                bar_bg,
            );

            // Draw bottom border
            ui.painter().line_segment(
                [
                    egui::pos2(bar_rect.min.x, bar_rect.max.y),
                    egui::pos2(bar_rect.max.x, bar_rect.max.y),
                ],
                egui::Stroke::new(1.0, border_color),
            );

            // Draw tabs inside the bar
            let mut tab_x = bar_rect.min.x + 8.0;
            let tab_y = bar_rect.min.y;
            let tab_height = bar_height;

            // Assets tab
            let assets_selected = active_plugin_tab.is_none() && viewport.bottom_panel_tab == BottomPanelTab::Assets;
            let assets_text = format!("{} Assets", FOLDER_OPEN);
            let assets_width = 70.0;
            let assets_rect = egui::Rect::from_min_size(
                egui::pos2(tab_x, tab_y),
                egui::Vec2::new(assets_width, tab_height),
            );

            let assets_response = ui.interact(assets_rect, ui.id().with("assets_tab"), Sense::click());
            if assets_response.hovered() {
                ctx.set_cursor_icon(CursorIcon::PointingHand);
            }
            if assets_response.clicked() {
                viewport.bottom_panel_tab = BottomPanelTab::Assets;
                ui_events.push(UiEvent::PanelTabSelected { location: 2, tab_id: String::new() });
            }

            let assets_bg = if assets_selected {
                tab_selected
            } else if assets_response.hovered() {
                tab_hover
            } else {
                Color32::TRANSPARENT
            };
            if assets_bg != Color32::TRANSPARENT {
                ui.painter().rect_filled(assets_rect, 0.0, assets_bg);
            }
            ui.painter().text(
                assets_rect.center(),
                egui::Align2::CENTER_CENTER,
                &assets_text,
                egui::FontId::proportional(12.0),
                if assets_selected { text_active } else { text_inactive },
            );

            tab_x += assets_width + 4.0;

            // Console tab
            let console_selected = active_plugin_tab.is_none() && viewport.bottom_panel_tab == BottomPanelTab::Console;
            let error_count = console.entries.iter().filter(|e| e.level == LogLevel::Error).count();
            let warning_count = console.entries.iter().filter(|e| e.level == LogLevel::Warning).count();

            let console_text = if error_count > 0 {
                format!("{} Console ({})", TERMINAL, error_count)
            } else if warning_count > 0 {
                format!("{} Console ({})", TERMINAL, warning_count)
            } else {
                format!("{} Console", TERMINAL)
            };
            let console_width = if error_count > 0 || warning_count > 0 { 95.0 } else { 75.0 };
            let console_rect = egui::Rect::from_min_size(
                egui::pos2(tab_x, tab_y),
                egui::Vec2::new(console_width, tab_height),
            );

            let console_response = ui.interact(console_rect, ui.id().with("console_tab"), Sense::click());
            if console_response.hovered() {
                ctx.set_cursor_icon(CursorIcon::PointingHand);
            }
            if console_response.clicked() {
                viewport.bottom_panel_tab = BottomPanelTab::Console;
                ui_events.push(UiEvent::PanelTabSelected { location: 2, tab_id: String::new() });
            }

            let console_bg = if console_selected {
                tab_selected
            } else if console_response.hovered() {
                tab_hover
            } else {
                Color32::TRANSPARENT
            };
            if console_bg != Color32::TRANSPARENT {
                ui.painter().rect_filled(console_rect, 0.0, console_bg);
            }

            let console_color = if error_count > 0 {
                error_color
            } else if warning_count > 0 {
                warning_color
            } else if console_selected {
                text_active
            } else {
                text_inactive
            };
            ui.painter().text(
                console_rect.center(),
                egui::Align2::CENTER_CENTER,
                &console_text,
                egui::FontId::proportional(12.0),
                console_color,
            );

            tab_x += console_width + 4.0;

            // Plugin tabs
            for tab in &plugin_tabs {
                let is_selected = active_plugin_tab == Some(tab.id.as_str());
                let tab_icon = tab.icon.as_deref().unwrap_or("");
                let tab_text = format!("{} {}", tab_icon, tab.title);
                let plugin_tab_width = 80.0;
                let plugin_rect = egui::Rect::from_min_size(
                    egui::pos2(tab_x, tab_y),
                    egui::Vec2::new(plugin_tab_width, tab_height),
                );

                let plugin_response = ui.interact(plugin_rect, ui.id().with(&tab.id), Sense::click());
                if plugin_response.hovered() {
                    ctx.set_cursor_icon(CursorIcon::PointingHand);
                }
                if plugin_response.clicked() {
                    ui_events.push(UiEvent::PanelTabSelected { location: 2, tab_id: tab.id.clone() });
                }

                let plugin_bg = if is_selected {
                    tab_selected
                } else if plugin_response.hovered() {
                    tab_hover
                } else {
                    Color32::TRANSPARENT
                };
                if plugin_bg != Color32::TRANSPARENT {
                    ui.painter().rect_filled(plugin_rect, 0.0, plugin_bg);
                }
                ui.painter().text(
                    plugin_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &tab_text,
                    egui::FontId::proportional(12.0),
                    if is_selected { text_active } else { text_inactive },
                );

                tab_x += plugin_tab_width + 4.0;
            }
            let _ = tab_x; // Suppress unused warning

            // Toggle button on the right side of the bar
            let toggle_size = 20.0;
            let toggle_rect = egui::Rect::from_center_size(
                egui::pos2(bar_rect.max.x - 16.0, bar_rect.center().y),
                egui::Vec2::splat(toggle_size),
            );

            let toggle_response = ui.interact(toggle_rect, ui.id().with("bottom_toggle"), Sense::click());
            let toggle_hovered = toggle_response.hovered();

            // Show pointer cursor on hover
            if toggle_hovered {
                ctx.set_cursor_icon(CursorIcon::PointingHand);
            }

            // Draw toggle button background on hover
            if toggle_hovered {
                ui.painter().rect_filled(
                    toggle_rect,
                    4.0,
                    toggle_hover_bg,
                );
            }

            // Draw toggle icon (up arrow when expanded, down arrow when minimized)
            let toggle_icon = if viewport.bottom_panel_minimized { CARET_UP } else { CARET_DOWN };
            ui.painter().text(
                toggle_rect.center(),
                egui::Align2::CENTER_CENTER,
                toggle_icon,
                egui::FontId::proportional(14.0),
                if toggle_hovered { text_active } else { text_inactive },
            );

            // Handle toggle click
            if toggle_response.clicked() {
                if viewport.bottom_panel_minimized {
                    // Restore previous height
                    viewport.bottom_panel_minimized = false;
                    viewport.assets_height = viewport.bottom_panel_prev_height;
                } else {
                    // Save current height and minimize
                    viewport.bottom_panel_prev_height = viewport.assets_height;
                    viewport.bottom_panel_minimized = true;
                }
            }

            // Only show content when not minimized
            if !viewport.bottom_panel_minimized {
                // Content area with padding
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(4, 4))
                    .show(ui, |ui| {

                // Tab content
                if let Some(tab_id) = active_plugin_tab {
                // Render plugin tab content
                if let Some(widgets) = api.get_tab_content(tab_id) {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for widget in widgets {
                            ui_renderer.render(ui, widget);
                        }
                    });
                } else {
                    ui.label(RichText::new("No content").color(theme.text.muted.to_color32()));
                }
            } else {
                // Render built-in tabs
                match viewport.bottom_panel_tab {
                    BottomPanelTab::Assets => {
                        render_assets_content(ui, current_project, assets, scene_state, thumbnail_cache, model_preview_cache, theme);
                    }
                    BottomPanelTab::Console => {
                        render_console_content(ui, console, theme);
                    }
                    BottomPanelTab::Animation => {
                        // Animation tab placeholder - content is rendered separately
                        ui.centered_and_justified(|ui| {
                            ui.label(RichText::new("Animation timeline coming soon").color(theme.text.muted.to_color32()));
                        });
                    }
                }
            }

                }); // End content frame
            } // End if !minimized
        });

    // Store the panel bounds for global file drop detection (in mod.rs)
    let panel_rect = panel_response.response.rect;
    assets.panel_bounds = [panel_rect.min.x, panel_rect.min.y, panel_rect.width(), panel_rect.height()];

    // Dialogs (only for assets tab)
    render_create_script_dialog(ctx, assets);
    render_create_folder_dialog(ctx, assets);
    render_import_dialog(ctx, assets, theme);
    handle_import_request(assets);

    // Process any pending file imports (files dropped into assets panel)
    process_pending_file_imports(assets);

    ui_events
}

/// Render asset dialogs (create script, create folder, material, scene, import)
/// Call this after render_assets_content to ensure dialogs work
pub fn render_assets_dialogs(ctx: &egui::Context, assets: &mut AssetBrowserState, theme: &Theme) {
    render_create_script_dialog(ctx, assets);
    render_create_folder_dialog(ctx, assets);
    render_create_material_dialog(ctx, assets);
    render_create_scene_dialog(ctx, assets);
    render_import_dialog(ctx, assets, theme);
    handle_import_request(assets);
    process_pending_file_imports(assets);
}

/// Render assets content (for use in docking)
pub fn render_assets_content(
    ui: &mut egui::Ui,
    current_project: Option<&CurrentProject>,
    assets: &mut AssetBrowserState,
    scene_state: &mut SceneManagerState,
    thumbnail_cache: &mut ThumbnailCache,
    model_preview_cache: &mut ModelPreviewCache,
    theme: &Theme,
) {
    let ctx = ui.ctx().clone();
    let available_width = ui.available_width();
    let is_compact = available_width < 250.0;

    // Theme colors
    let text_primary = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let text_disabled = theme.text.disabled.to_color32();
    let _accent_color = theme.semantic.accent.to_color32();

    // Toolbar with breadcrumb and import
    render_toolbar(ui, &ctx, assets, current_project, theme);

    ui.add_space(2.0);

    // Search bar
    ui.horizontal(|ui| {
        let search_width = (available_width - 8.0).min(300.0).max(60.0);
        ui.add_sized(
            [search_width, 20.0],
            egui::TextEdit::singleline(&mut assets.search)
                .hint_text(format!("{} Search...", MAGNIFYING_GLASS))
        );
    });

    ui.add_space(2.0);
    ui.separator();
    ui.add_space(2.0);

    // Update thumbnail cache folder tracking
    thumbnail_cache.clear_for_folder_change(assets.current_folder.clone());

    let available_height = ui.available_height();

    // Threshold for showing grid alongside tree
    let show_grid = available_width >= 450.0;
    let tree_width = if show_grid {
        assets.tree_panel_width.clamp(120.0, available_width * 0.5)
    } else {
        available_width
    };

    // Main content area
    let content_rect = ui.allocate_ui_with_layout(
        Vec2::new(available_width, available_height.max(50.0)),
        egui::Layout::left_to_right(egui::Align::TOP),
        |ui| {
            if let Some(project) = current_project {
                let items = collect_items(assets, project);
                let filtered_items = filter_items(&items, &assets.search);

                // Tree view panel (always shown)
                ui.allocate_ui_with_layout(
                    Vec2::new(tree_width, available_height.max(50.0)),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("assets_tree_scroll")
                            .max_height(available_height.max(50.0))
                            .show(ui, |ui| {
                                ui.set_min_width(tree_width - 8.0);
                                render_tree_navigation(ui, &ctx, assets, scene_state, project, thumbnail_cache, theme);
                            });
                    },
                );

                // Resize handle between tree and grid
                if show_grid {
                    let separator_response = ui.separator();
                    let sep_rect = separator_response.rect;
                    let resize_rect = egui::Rect::from_center_size(
                        sep_rect.center(),
                        Vec2::new(8.0, sep_rect.height()),
                    );
                    let resize_response = ui.interact(resize_rect, ui.id().with("tree_resize"), Sense::drag());

                    if resize_response.hovered() || resize_response.dragged() {
                        ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
                    }

                    if resize_response.dragged() {
                        if let Some(pointer_pos) = ctx.pointer_interact_pos() {
                            let new_width = pointer_pos.x - ui.min_rect().min.x;
                            assets.tree_panel_width = new_width.clamp(120.0, available_width * 0.5);
                        }
                    }

                    // Grid view panel
                    let grid_width = available_width - tree_width - 8.0;
                    ui.allocate_ui_with_layout(
                        Vec2::new(grid_width, available_height.max(50.0)),
                        egui::Layout::top_down(egui::Align::LEFT),
                        |ui| {
                            egui::ScrollArea::vertical()
                                .id_salt("assets_grid_scroll")
                                .max_height(available_height.max(50.0))
                                .show(ui, |ui| {
                                    render_grid_view(ui, &ctx, assets, scene_state, &filtered_items, thumbnail_cache, model_preview_cache, theme);

                                    // Fill remaining space to ensure context menu works in empty areas
                                    let remaining = ui.available_size();
                                    if remaining.y > 0.0 {
                                        ui.allocate_space(remaining);
                                    }
                                });
                        },
                    );
                }
            } else {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(FOLDER).size(32.0).color(text_disabled));
                    ui.add_space(4.0);
                    ui.label(RichText::new("No project loaded").size(11.0).color(text_muted));
                });
            }
        },
    ).response.rect;

    // Handle context menu for entire content area
    if current_project.is_some() {
        // Detect right-click to open/move context menu
        if ctx.input(|i| i.pointer.secondary_clicked()) {
            if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                if content_rect.contains(pos) {
                    assets.context_menu_pos = Some(bevy::math::Vec2::new(pos.x, pos.y));
                }
            }
        }

        // Close menu on Escape
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            assets.context_menu_pos = None;
        }

        // Show context menu popup
        if let Some(pos) = assets.context_menu_pos {
            // Approximate menu height for positioning
            let menu_height = 170.0;
            let screen_rect = ctx.screen_rect();

            // If menu would go off bottom, show it above the cursor
            let menu_y = if pos.y + menu_height > screen_rect.max.y {
                pos.y - menu_height
            } else {
                pos.y
            };

            let area_response = egui::Area::new(egui::Id::new("assets_context_menu"))
                .fixed_pos(egui::pos2(pos.x, menu_y))
                .order(egui::Order::Foreground)
                .constrain(true)
                .show(&ctx, |ui| {
                    egui::Frame::popup(ui.style())
                        .show(ui, |ui| {
                            render_context_menu(ui, assets, theme);
                        });
                });

            // Close menu when clicking outside of it
            if ctx.input(|i| i.pointer.primary_clicked()) {
                if let Some(click_pos) = ctx.input(|i| i.pointer.interact_pos()) {
                    if !area_response.response.rect.contains(click_pos) {
                        assets.context_menu_pos = None;
                    }
                }
            }
        }
    }

}

fn render_toolbar(
    ui: &mut egui::Ui,
    _ctx: &egui::Context,
    assets: &mut AssetBrowserState,
    current_project: Option<&CurrentProject>,
    theme: &Theme,
) {
    let available_width = ui.available_width();

    // Responsive breakpoints
    let is_compact = available_width < 300.0;
    let is_medium = available_width < 450.0;
    let is_narrow = available_width < 200.0;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = if is_compact { 2.0 } else { 4.0 };

        // Back button
        let can_go_back = assets.current_folder.is_some();
        ui.add_enabled_ui(can_go_back, |ui| {
            if ui.small_button(RichText::new(ARROW_LEFT).size(14.0)).clicked() {
                if let Some(ref current) = assets.current_folder {
                    if let Some(project) = current_project {
                        if current == &project.path {
                            // Already at project root, can't go back
                        } else if let Some(parent) = current.parent() {
                            if parent >= project.path.as_path() {
                                assets.current_folder = Some(parent.to_path_buf());
                            }
                        }
                    }
                }
            }
        });

        // Home button
        if ui.small_button(RichText::new(HOUSE).size(14.0)).on_hover_text("Go to project root").clicked() {
            if let Some(project) = current_project {
                assets.current_folder = Some(project.path.clone());
            }
        }

        // Only show breadcrumb if not too narrow
        if !is_narrow {
            ui.separator();

            // Breadcrumb path (simplified for narrow widths)
            if let Some(project) = current_project {
                let project_name = project.path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Project");

                if let Some(ref current_folder) = assets.current_folder {
                    // Build breadcrumb parts
                    let mut parts: Vec<(String, PathBuf)> = vec![];
                    parts.push((project_name.to_string(), project.path.clone()));

                    if let Ok(relative) = current_folder.strip_prefix(&project.path) {
                        let mut accumulated = project.path.clone();
                        for component in relative.components() {
                            if let std::path::Component::Normal(name) = component {
                                accumulated = accumulated.join(name);
                                if let Some(name_str) = name.to_str() {
                                    parts.push((name_str.to_string(), accumulated.clone()));
                                }
                            }
                        }
                    }

                    // For compact view, only show last 1-2 parts
                    let display_parts = if is_compact && parts.len() > 2 {
                        let last = parts.last().cloned();
                        vec![("...".to_string(), project.path.clone())]
                            .into_iter()
                            .chain(last)
                            .collect::<Vec<_>>()
                    } else if is_medium && parts.len() > 3 {
                        let first = parts.first().cloned();
                        let last = parts.last().cloned();
                        vec![first, Some(("...".to_string(), project.path.clone())), last]
                            .into_iter()
                            .flatten()
                            .collect::<Vec<_>>()
                    } else {
                        parts
                    };

                    let breadcrumb_muted = theme.text.disabled.to_color32();
                    let breadcrumb_normal = theme.text.muted.to_color32();
                    let breadcrumb_active = theme.text.primary.to_color32();

                    for (i, (name, path)) in display_parts.iter().enumerate() {
                        if i > 0 {
                            ui.label(RichText::new(CARET_RIGHT).size(10.0).color(breadcrumb_muted));
                        }

                        if name == "..." {
                            ui.label(RichText::new("...").size(11.0).color(breadcrumb_muted));
                        } else {
                            let is_current = Some(path) == assets.current_folder.as_ref();
                            let text_color = if is_current {
                                breadcrumb_active
                            } else {
                                breadcrumb_normal
                            };

                            let display_name = if is_compact && name.len() > 8 {
                                format!("{}...", &name[..6])
                            } else if is_medium && name.len() > 12 {
                                format!("{}...", &name[..10])
                            } else {
                                name.clone()
                            };

                            if ui.link(RichText::new(&display_name).color(text_color).size(11.0)).clicked() {
                                assets.current_folder = Some(path.clone());
                            }
                        }
                    }
                } else {
                    ui.label(RichText::new(project_name).size(11.0).color(theme.text.muted.to_color32()));
                }
            }
        }

        // Right-aligned controls: zoom slider + import button
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;

            // Import button
            let import_enabled = assets.current_folder.is_some();
            ui.add_enabled_ui(import_enabled, |ui| {
                if ui.small_button(RichText::new(PLUS).size(14.0))
                    .on_hover_text("Import assets")
                    .clicked()
                {
                    open_import_file_dialog(assets);
                }
            });

            // Zoom slider (only when wide enough for grid)
            if !is_compact {
                ui.separator();
                ui.spacing_mut().slider_width = 50.0;
                ui.add(egui::Slider::new(&mut assets.zoom, 0.5..=1.5).show_value(false))
                    .on_hover_text("Grid size");
            }
        });
    });
}

fn collect_items(assets: &AssetBrowserState, project: &CurrentProject) -> Vec<AssetItem> {
    let mut items = Vec::new();

    let folder_to_read = assets.current_folder.as_ref()
        .unwrap_or(&project.path);

    if let Ok(entries) = std::fs::read_dir(folder_to_read) {
        let mut folders: Vec<PathBuf> = Vec::new();
        let mut files: Vec<PathBuf> = Vec::new();

        for entry in entries.flatten() {
            let path = entry.path();
            // Skip hidden files/folders
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with('.') {
                    continue;
                }
            }
            // Skip target directory and other build artifacts
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name == "target" || name == "Cargo.lock" {
                    continue;
                }
            }

            if path.is_dir() {
                folders.push(path);
            } else {
                files.push(path);
            }
        }

        folders.sort();
        files.sort();

        // Add folders first
        for folder in folders {
            if let Some(name) = folder.file_name().and_then(|n| n.to_str()) {
                let (icon, color) = get_folder_icon_and_color(name);
                items.push(AssetItem {
                    name: name.to_string(),
                    path: folder,
                    is_folder: true,
                    icon,
                    color,
                });
            }
        }

        // Add files
        for file in files {
            if let Some(name) = file.file_name().and_then(|n| n.to_str()) {
                let (icon, color) = get_file_icon_and_color(name);
                items.push(AssetItem {
                    name: name.to_string(),
                    path: file,
                    is_folder: false,
                    icon,
                    color,
                });
            }
        }
    }

    items
}

fn filter_items<'a>(items: &'a [AssetItem], search: &str) -> Vec<&'a AssetItem> {
    if search.is_empty() {
        items.iter().collect()
    } else {
        let search_lower = search.to_lowercase();
        items.iter()
            .filter(|item| item.name.to_lowercase().contains(&search_lower))
            .collect()
    }
}

fn render_grid_view(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    assets: &mut AssetBrowserState,
    scene_state: &mut SceneManagerState,
    items: &[&AssetItem],
    thumbnail_cache: &mut ThumbnailCache,
    model_preview_cache: &mut ModelPreviewCache,
    theme: &Theme,
) {
    let available_width = ui.available_width();

    // Theme colors for grid items
    let item_bg = theme.panels.item_bg.to_color32();
    let item_hover = theme.panels.item_hover.to_color32();
    let selection_bg = theme.semantic.selection.to_color32();
    let selection_stroke = theme.semantic.selection_stroke.to_color32();
    let text_primary = theme.text.primary.to_color32();
    let text_secondary = theme.text.secondary.to_color32();
    let accent_color = theme.semantic.accent.to_color32();
    let surface_faint = theme.surfaces.faint.to_color32();

    // Responsive tile sizing - larger base for new design
    let base_tile_size = if available_width < 150.0 {
        70.0  // Compact tiles
    } else if available_width < 250.0 {
        80.0  // Small panel
    } else if available_width < 400.0 {
        90.0  // Medium panel
    } else {
        100.0  // Normal size
    };

    let tile_size = base_tile_size * assets.zoom;
    let icon_size = (tile_size * 0.55).max(28.0);  // Bigger icons
    let thumbnail_size = tile_size - 16.0;
    let spacing = if available_width < 200.0 { 4.0 } else { 8.0 };

    // Label area for 2 lines
    let label_area_height = 36.0;
    let separator_height = 3.0;
    let total_height = tile_size + separator_height + label_area_height;

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(spacing, spacing);

        for item in items {
            let is_draggable = !item.is_folder && is_draggable_asset(&item.name);
            let has_image_thumbnail = !item.is_folder && supports_thumbnail(&item.name);
            let has_model_preview = !item.is_folder && supports_model_preview(&item.name);

            let (rect, response) = ui.allocate_exact_size(
                Vec2::new(tile_size, total_height),
                if is_draggable { Sense::click_and_drag() } else { Sense::click() },
            );

            let is_hovered = response.hovered();
            if is_hovered {
                ctx.set_cursor_icon(CursorIcon::PointingHand);
            }
            let is_selected = assets.selected_asset.as_ref() == Some(&item.path);

            // Card background with subtle gradient effect
            let bg_color = if is_selected {
                selection_bg
            } else if is_hovered {
                item_hover
            } else {
                item_bg
            };

            // Main card background with rounded corners
            ui.painter().rect_filled(
                rect,
                CornerRadius::same(8),
                bg_color,
            );

            // Icon area background (slightly darker)
            let icon_area_rect = egui::Rect::from_min_size(
                rect.min,
                Vec2::new(tile_size, tile_size),
            );

            if !is_selected && !is_hovered {
                ui.painter().rect_filled(
                    icon_area_rect,
                    CornerRadius {
                        nw: 8,
                        ne: 8,
                        sw: 0,
                        se: 0,
                    },
                    surface_faint,
                );
            }

            // Selection border
            if is_selected {
                ui.painter().rect_stroke(
                    rect,
                    8.0,
                    egui::Stroke::new(2.0, selection_stroke),
                    egui::StrokeKind::Inside,
                );
            }

            // Hover glow effect
            if is_hovered && !is_selected {
                ui.painter().rect_stroke(
                    rect,
                    8.0,
                    egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(
                        item.color.r(),
                        item.color.g(),
                        item.color.b(),
                        60,
                    )),
                    egui::StrokeKind::Inside,
                );
            }

            // Icon or Thumbnail - centered in icon area
            let icon_center = egui::pos2(rect.center().x, rect.min.y + tile_size / 2.0);
            let icon_rect = egui::Rect::from_center_size(
                icon_center,
                Vec2::splat(icon_size),
            );

            let mut thumbnail_shown = false;

            // Try image thumbnails first
            if has_image_thumbnail {
                if let Some(texture_id) = thumbnail_cache.get_texture_id(&item.path) {
                    let thumb_rect = egui::Rect::from_center_size(
                        icon_center,
                        Vec2::splat(thumbnail_size),
                    );

                    // Draw checkerboard background for images with transparency
                    draw_checkerboard(ui.painter(), thumb_rect, theme);

                    // Rounded thumbnail
                    ui.painter().image(
                        texture_id,
                        thumb_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    thumbnail_shown = true;
                } else if !thumbnail_cache.is_loading(&item.path) && !thumbnail_cache.has_failed(&item.path) {
                    thumbnail_cache.request_load(item.path.clone());
                }
            }
            // Try 3D model previews
            else if has_model_preview {
                if let Some(texture_id) = model_preview_cache.get_texture_id(&item.path) {
                    let thumb_rect = egui::Rect::from_center_size(
                        icon_center,
                        Vec2::splat(thumbnail_size),
                    );

                    ui.painter().image(
                        texture_id,
                        thumb_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    thumbnail_shown = true;
                } else if !model_preview_cache.is_loading(&item.path) && !model_preview_cache.has_failed(&item.path) {
                    model_preview_cache.request_preview(item.path.clone());
                }
            }

            // Fall back to icon if no thumbnail
            if !thumbnail_shown {
                // Icon shadow for depth
                ui.painter().text(
                    egui::pos2(icon_rect.center().x + 1.0, icon_rect.center().y + 1.0),
                    egui::Align2::CENTER_CENTER,
                    item.icon,
                    egui::FontId::proportional(icon_size),
                    Color32::from_rgba_unmultiplied(0, 0, 0, 30),
                );

                // Main icon
                ui.painter().text(
                    icon_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    item.icon,
                    egui::FontId::proportional(icon_size),
                    item.color,
                );

                // Show loading indicator
                let is_loading = (has_image_thumbnail && thumbnail_cache.is_loading(&item.path))
                    || (has_model_preview && model_preview_cache.is_loading(&item.path));
                if is_loading {
                    let spinner_pos = egui::pos2(rect.max.x - 14.0, rect.min.y + 14.0);
                    ui.painter().circle_filled(spinner_pos, 6.0, Color32::from_rgba_unmultiplied(0, 0, 0, 100));
                    ui.painter().circle_stroke(
                        spinner_pos,
                        4.0,
                        egui::Stroke::new(2.0, accent_color),
                    );
                }
            }

            // Color-coded separator line
            let separator_y = rect.min.y + tile_size;
            let separator_rect = egui::Rect::from_min_size(
                egui::pos2(rect.min.x + 6.0, separator_y),
                Vec2::new(tile_size - 12.0, separator_height),
            );
            ui.painter().rect_filled(
                separator_rect,
                CornerRadius::same(1),
                Color32::from_rgba_unmultiplied(
                    item.color.r(),
                    item.color.g(),
                    item.color.b(),
                    if is_selected || is_hovered { 200 } else { 120 },
                ),
            );

            // Two-line label
            let font_size = if tile_size < 75.0 { 10.0 } else { 11.0 };
            let line_height = font_size + 2.0;

            let label_rect = egui::Rect::from_min_size(
                egui::pos2(rect.min.x + 4.0, separator_y + separator_height + 2.0),
                Vec2::new(tile_size - 8.0, label_area_height - 4.0),
            );

            // Split name into two lines
            let (line1, line2) = split_name_two_lines(&item.name, tile_size, font_size);

            // First line - filename (primary color, slightly bolder)
            ui.painter().text(
                egui::pos2(label_rect.center().x, label_rect.min.y + line_height / 2.0),
                egui::Align2::CENTER_CENTER,
                &line1,
                egui::FontId::proportional(font_size),
                text_primary,
            );

            // Second line - extension or continuation (secondary color)
            if !line2.is_empty() {
                ui.painter().text(
                    egui::pos2(label_rect.center().x, label_rect.min.y + line_height + line_height / 2.0),
                    egui::Align2::CENTER_CENTER,
                    &line2,
                    egui::FontId::proportional(font_size - 1.0),
                    text_secondary,
                );
            }

            // Handle interactions
            handle_item_interaction(ctx, assets, scene_state, item, &response, is_draggable);

            // Tooltip with full name
            if is_hovered && assets.dragging_asset.is_none() {
                response.on_hover_text(&item.name);
            }
        }
    });
}

/// Split filename into two lines for display
fn split_name_two_lines(name: &str, tile_width: f32, font_size: f32) -> (String, String) {
    let char_width = font_size * 0.55;
    let max_chars = ((tile_width - 8.0) / char_width) as usize;
    let max_chars = max_chars.max(6);

    if name.len() <= max_chars {
        return (name.to_string(), String::new());
    }

    // Try to split at extension
    if let Some(dot_pos) = name.rfind('.') {
        let base = &name[..dot_pos];
        let ext = &name[dot_pos..];

        if base.len() <= max_chars && ext.len() <= max_chars {
            return (base.to_string(), ext.to_string());
        }

        // Truncate base name if needed
        if base.len() > max_chars {
            let truncated_base = format!("{}…", &base[..max_chars.saturating_sub(1)]);
            return (truncated_base, ext.to_string());
        }
    }

    // No extension or doesn't fit nicely - split in middle
    let mid = max_chars.min(name.len());
    let line1 = &name[..mid];
    let remaining = &name[mid..];

    let line2 = if remaining.len() > max_chars {
        format!("{}…", &remaining[..max_chars.saturating_sub(1)])
    } else {
        remaining.to_string()
    };

    (line1.to_string(), line2)
}

/// Draw a checkerboard pattern for transparent image backgrounds
fn draw_checkerboard(painter: &egui::Painter, rect: egui::Rect, theme: &Theme) {
    let check_size = 8.0;
    // Use subtle variations for checkerboard based on theme
    let light = theme.surfaces.faint.to_color32();
    let dark = theme.surfaces.panel.to_color32();

    let cols = (rect.width() / check_size).ceil() as i32;
    let rows = (rect.height() / check_size).ceil() as i32;

    for row in 0..rows {
        for col in 0..cols {
            let color = if (row + col) % 2 == 0 { light } else { dark };
            let check_rect = egui::Rect::from_min_size(
                egui::pos2(rect.min.x + col as f32 * check_size, rect.min.y + row as f32 * check_size),
                Vec2::splat(check_size),
            ).intersect(rect);
            painter.rect_filled(check_rect, 0.0, color);
        }
    }
}

/// Render tree navigation panel (folders only, for navigating the project)
fn render_tree_navigation(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    assets: &mut AssetBrowserState,
    scene_state: &mut SceneManagerState,
    project: &CurrentProject,
    thumbnail_cache: &mut ThumbnailCache,
    theme: &Theme,
) {
    ui.style_mut().spacing.item_spacing.y = 0.0;

    // Auto-expand project root if not yet expanded and no folders are expanded
    if assets.expanded_folders.is_empty() {
        assets.expanded_folders.insert(project.path.clone());
    }

    // Theme colors
    let selection_bg = theme.semantic.selection.to_color32();
    let item_hover = theme.panels.item_hover.to_color32();
    let text_secondary = theme.text.secondary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let accent_color = theme.semantic.accent.to_color32();

    // Render project root as the top item
    let project_name = project.path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Project");

    let is_root_expanded = assets.expanded_folders.contains(&project.path);
    let is_root_current = assets.current_folder.as_ref() == Some(&project.path) || assets.current_folder.is_none();

    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), LIST_ROW_HEIGHT),
        Sense::click(),
    );

    let is_hovered = response.hovered();
    if is_hovered {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
    }

    // Background
    let bg_color = if is_root_current {
        selection_bg
    } else if is_hovered {
        item_hover
    } else {
        Color32::TRANSPARENT
    };

    if bg_color != Color32::TRANSPARENT {
        ui.painter().rect_filled(rect, 2.0, bg_color);
    }

    // Expand/collapse arrow
    let arrow_x = rect.min.x + 8.0;
    let arrow_icon = if is_root_expanded { CARET_DOWN } else { CARET_RIGHT };

    let arrow_rect = egui::Rect::from_center_size(
        egui::pos2(arrow_x, rect.center().y),
        Vec2::splat(14.0),
    );
    let arrow_response = ui.interact(arrow_rect, ui.id().with("project_root_arrow"), Sense::click());
    if arrow_response.hovered() {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
    }

    ui.painter().text(
        egui::pos2(arrow_x, rect.center().y),
        egui::Align2::CENTER_CENTER,
        arrow_icon,
        egui::FontId::proportional(9.0),
        if arrow_response.hovered() { text_secondary } else { text_muted },
    );

    // Project icon (house/folder)
    let icon_x = arrow_x + 12.0;
    ui.painter().text(
        egui::pos2(icon_x, rect.center().y),
        egui::Align2::LEFT_CENTER,
        if is_root_expanded { FOLDER_OPEN } else { HOUSE },
        egui::FontId::proportional(12.0),
        accent_color,
    );

    // Project name
    ui.painter().text(
        egui::pos2(icon_x + 16.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        project_name,
        egui::FontId::proportional(11.0),
        text_secondary,
    );

    // Handle interactions - single click toggles and navigates
    if arrow_response.clicked() || response.clicked() {
        if is_root_expanded {
            assets.expanded_folders.remove(&project.path);
        } else {
            assets.expanded_folders.insert(project.path.clone());
        }
        assets.current_folder = Some(project.path.clone());
    }

    // Render children if expanded
    if is_root_expanded {
        render_nav_tree_node(ui, ctx, assets, scene_state, &project.path, 1, thumbnail_cache, theme);
    }
}

/// Render a navigation tree node (folders only)
fn render_nav_tree_node(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    assets: &mut AssetBrowserState,
    _scene_state: &mut SceneManagerState,
    folder_path: &PathBuf,
    depth: usize,
    _thumbnail_cache: &mut ThumbnailCache,
    theme: &Theme,
) {
    let indent = depth as f32 * 14.0;

    // Theme colors for tree items
    let selection_bg = theme.semantic.selection.to_color32();
    let item_hover = theme.panels.item_hover.to_color32();
    let text_secondary = theme.text.secondary.to_color32();
    let text_muted = theme.text.muted.to_color32();

    // Read directory contents
    let Ok(entries) = std::fs::read_dir(folder_path) else {
        return;
    };

    let mut folders: Vec<PathBuf> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        // Skip hidden folders
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') || name == "target" {
                continue;
            }
        }

        // Apply search filter
        if !assets.search.is_empty() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if !name.to_lowercase().contains(&assets.search.to_lowercase()) {
                    // Check if any children match
                    if !folder_contains_match(&path, &assets.search) {
                        continue;
                    }
                }
            }
        }

        folders.push(path);
    }

    folders.sort();

    // Render folders
    for child_path in folders {
        let Some(name) = child_path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        let is_expanded = assets.expanded_folders.contains(&child_path);
        let is_current = assets.current_folder.as_ref() == Some(&child_path);

        let (icon, color) = get_folder_icon_and_color(name);

        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), LIST_ROW_HEIGHT),
            Sense::click(),
        );

        let is_hovered = response.hovered();
        if is_hovered {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }

        // Background
        let bg_color = if is_current {
            selection_bg
        } else if is_hovered {
            item_hover
        } else {
            Color32::TRANSPARENT
        };

        if bg_color != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, 2.0, bg_color);
        }

        // Expand/collapse arrow
        let arrow_x = rect.min.x + indent + 8.0;
        let arrow_icon = if is_expanded { CARET_DOWN } else { CARET_RIGHT };

        let arrow_rect = egui::Rect::from_center_size(
            egui::pos2(arrow_x, rect.center().y),
            Vec2::splat(14.0),
        );
        let arrow_response = ui.interact(arrow_rect, ui.id().with(("nav_arrow", &child_path)), Sense::click());
        if arrow_response.hovered() {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }

        ui.painter().text(
            egui::pos2(arrow_x, rect.center().y),
            egui::Align2::CENTER_CENTER,
            arrow_icon,
            egui::FontId::proportional(9.0),
            if arrow_response.hovered() { text_secondary } else { text_muted },
        );

        // Folder icon
        let icon_x = arrow_x + 12.0;
        ui.painter().text(
            egui::pos2(icon_x, rect.center().y),
            egui::Align2::LEFT_CENTER,
            if is_expanded { FOLDER_OPEN } else { icon },
            egui::FontId::proportional(12.0),
            color,
        );

        // Folder name
        ui.painter().text(
            egui::pos2(icon_x + 16.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            name,
            egui::FontId::proportional(11.0),
            text_secondary,
        );

        // Handle interactions - single click toggles and navigates
        if arrow_response.clicked() || response.clicked() {
            if is_expanded {
                assets.expanded_folders.remove(&child_path);
            } else {
                assets.expanded_folders.insert(child_path.clone());
            }
            assets.current_folder = Some(child_path.clone());
        }

        // Render children if expanded
        if is_expanded {
            render_nav_tree_node(ui, ctx, assets, _scene_state, &child_path, depth + 1, _thumbnail_cache, theme);
        }
    }
}

fn render_list_view(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    assets: &mut AssetBrowserState,
    scene_state: &mut SceneManagerState,
    _items: &[&AssetItem],
    thumbnail_cache: &mut ThumbnailCache,
    _model_preview_cache: &mut ModelPreviewCache,
    project: &CurrentProject,
    theme: &Theme,
) {
    // Remove vertical spacing between rows
    ui.style_mut().spacing.item_spacing.y = 0.0;

    // For tree view, start from current folder or project root
    let root_folder = assets.current_folder.clone().unwrap_or_else(|| project.path.clone());
    render_tree_node(ui, ctx, assets, scene_state, &root_folder, 0, thumbnail_cache, theme);
}

fn render_tree_node(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    assets: &mut AssetBrowserState,
    scene_state: &mut SceneManagerState,
    folder_path: &PathBuf,
    depth: usize,
    thumbnail_cache: &mut ThumbnailCache,
    theme: &Theme,
) {
    let indent = depth as f32 * 14.0;
    let thumbnail_size = 14.0;

    // Theme colors for tree items
    let selection_bg = theme.semantic.selection.to_color32();
    let item_hover = theme.panels.item_hover.to_color32();
    let text_secondary = theme.text.secondary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let text_disabled = theme.text.disabled.to_color32();

    // Read directory contents
    let Ok(entries) = std::fs::read_dir(folder_path) else {
        return;
    };

    let mut folders: Vec<PathBuf> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        // Skip hidden files/folders
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') || name == "target" || name == "Cargo.lock" {
                continue;
            }
        }

        // Apply search filter
        if !assets.search.is_empty() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if !name.to_lowercase().contains(&assets.search.to_lowercase()) {
                    // For folders, check if any children match
                    if path.is_dir() && !folder_contains_match(&path, &assets.search) {
                        continue;
                    } else if !path.is_dir() {
                        continue;
                    }
                }
            }
        }

        if path.is_dir() {
            folders.push(path);
        } else {
            files.push(path);
        }
    }

    folders.sort();
    files.sort();

    // Render folders first
    for folder_path in folders {
        let Some(name) = folder_path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        let is_expanded = assets.expanded_folders.contains(&folder_path);
        let is_selected = assets.selected_asset.as_ref() == Some(&folder_path);

        let (icon, color) = get_folder_icon_and_color(name);

        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), LIST_ROW_HEIGHT),
            Sense::click(),
        );

        let is_hovered = response.hovered();
        if is_hovered {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }

        // Background
        let bg_color = if is_selected {
            selection_bg
        } else if is_hovered {
            item_hover
        } else {
            Color32::TRANSPARENT
        };

        if bg_color != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, 2.0, bg_color);
        }

        // Expand/collapse arrow
        let arrow_x = rect.min.x + indent + 8.0;
        let arrow_icon = if is_expanded { CARET_DOWN } else { CARET_RIGHT };

        let arrow_rect = egui::Rect::from_center_size(
            egui::pos2(arrow_x, rect.center().y),
            Vec2::splat(14.0),
        );
        let arrow_response = ui.interact(arrow_rect, ui.id().with(("arrow", &folder_path)), Sense::click());
        if arrow_response.hovered() {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }

        ui.painter().text(
            egui::pos2(arrow_x, rect.center().y),
            egui::Align2::CENTER_CENTER,
            arrow_icon,
            egui::FontId::proportional(9.0),
            if arrow_response.hovered() { text_secondary } else { text_muted },
        );

        // Folder icon
        let icon_x = arrow_x + 12.0;
        ui.painter().text(
            egui::pos2(icon_x, rect.center().y),
            egui::Align2::LEFT_CENTER,
            if is_expanded { FOLDER_OPEN } else { icon },
            egui::FontId::proportional(12.0),
            color,
        );

        // Folder name
        ui.painter().text(
            egui::pos2(icon_x + 16.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            name,
            egui::FontId::proportional(11.0),
            text_secondary,
        );

        // Handle interactions - single click toggles and selects
        if arrow_response.clicked() || response.clicked() {
            if is_expanded {
                assets.expanded_folders.remove(&folder_path);
            } else {
                assets.expanded_folders.insert(folder_path.clone());
            }
            assets.selected_asset = Some(folder_path.clone());
        }

        // Render children if expanded
        if is_expanded {
            render_tree_node(ui, ctx, assets, scene_state, &folder_path, depth + 1, thumbnail_cache, theme);
        }
    }

    // Render files
    for file_path in files {
        let Some(name) = file_path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        let (icon, color) = get_file_icon_and_color(name);
        let is_draggable = is_draggable_asset(name);
        let has_thumbnail = supports_thumbnail(name);
        let is_selected = assets.selected_asset.as_ref() == Some(&file_path);

        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), LIST_ROW_HEIGHT),
            if is_draggable { Sense::click_and_drag() } else { Sense::click() },
        );

        let is_hovered = response.hovered();
        if is_hovered {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }

        // Background
        let file_bg_color = if is_selected {
            selection_bg
        } else if is_hovered {
            item_hover
        } else {
            Color32::TRANSPARENT
        };

        if file_bg_color != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, 2.0, file_bg_color);
        }

        // Icon position (indented, no arrow space needed for files but align with folder names)
        let icon_x = rect.min.x + indent + 20.0;

        // Icon or small thumbnail
        let mut thumbnail_shown = false;

        if has_thumbnail {
            if let Some(texture_id) = thumbnail_cache.get_texture_id(&file_path) {
                let thumb_rect = egui::Rect::from_center_size(
                    egui::pos2(icon_x + 6.0, rect.center().y),
                    Vec2::splat(thumbnail_size),
                );
                ui.painter().image(
                    texture_id,
                    thumb_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
                thumbnail_shown = true;
            } else if !thumbnail_cache.is_loading(&file_path) && !thumbnail_cache.has_failed(&file_path) {
                thumbnail_cache.request_load(file_path.clone());
            }
        }

        if !thumbnail_shown {
            ui.painter().text(
                egui::pos2(icon_x, rect.center().y),
                egui::Align2::LEFT_CENTER,
                icon,
                egui::FontId::proportional(12.0),
                color,
            );
        }

        // File name
        ui.painter().text(
            egui::pos2(icon_x + 16.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            name,
            egui::FontId::proportional(11.0),
            text_secondary,
        );

        // File extension on the right
        if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
            ui.painter().text(
                egui::pos2(rect.max.x - 6.0, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                ext.to_uppercase(),
                egui::FontId::proportional(9.0),
                text_disabled,
            );
        }

        // Create a temporary AssetItem for handle_item_interaction
        let item = AssetItem {
            name: name.to_string(),
            path: file_path.clone(),
            is_folder: false,
            icon,
            color,
        };

        handle_item_interaction(ctx, assets, scene_state, &item, &response, is_draggable);
    }
}

/// Check if a folder contains any files matching the search
fn folder_contains_match(folder: &PathBuf, search: &str) -> bool {
    let search_lower = search.to_lowercase();

    if let Ok(entries) = std::fs::read_dir(folder) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.to_lowercase().contains(&search_lower) {
                    return true;
                }
                if path.is_dir() && folder_contains_match(&path, search) {
                    return true;
                }
            }
        }
    }
    false
}

fn handle_item_interaction(
    ctx: &egui::Context,
    assets: &mut AssetBrowserState,
    scene_state: &mut SceneManagerState,
    item: &AssetItem,
    response: &egui::Response,
    is_draggable: bool,
) {
    if response.clicked() {
        if item.is_folder {
            assets.current_folder = Some(item.path.clone());
        } else {
            assets.selected_asset = Some(item.path.clone());

            // Open script files in the editor and switch to Scripting layout
            if is_script_file(&item.path) {
                super::script_editor::open_script(scene_state, item.path.clone());
                assets.requested_layout = Some("Scripting".to_string());
            }

            // Open material files in blueprint editor and switch to Blueprints layout
            if is_blueprint_material_file(&item.name) {
                assets.pending_blueprint_open = Some(item.path.clone());
                assets.requested_layout = Some("Blueprints".to_string());
            }
        }
    }

    if response.double_clicked() {
        if item.is_folder {
            assets.current_folder = Some(item.path.clone());
        } else if is_image_file(&item.name) {
            // Open image in preview tab and switch to Image Preview layout
            super::image_preview::open_image(scene_state, item.path.clone());
            assets.requested_layout = Some("Image Preview".to_string());
        }
    }

    // Drag support for models
    if is_draggable {
        if response.drag_started() {
            assets.dragging_asset = Some(item.path.clone());
        }

        if assets.dragging_asset.as_ref() == Some(&item.path) {
            if let Some(pos) = ctx.pointer_hover_pos() {
                egui::Area::new(egui::Id::new("drag_tooltip"))
                    .fixed_pos(pos + Vec2::new(10.0, 10.0))
                    .interactable(false)
                    .order(egui::Order::Tooltip)
                    .show(ctx, |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let (icon, color, hint) = if is_scene_file(&item.name) {
                                    (FILM_SCRIPT, Color32::from_rgb(115, 191, 242), "Drop in hierarchy")
                                } else if is_image_file(&item.name) {
                                    (IMAGE, Color32::from_rgb(166, 217, 140), "Drop in viewport (3D: Plane, 2D: Sprite)")
                                } else if is_hdr_file(&item.name) {
                                    (SUN, Color32::from_rgb(255, 200, 100), "Drop on World Environment (Sky Texture)")
                                } else {
                                    (CUBE, Color32::from_rgb(242, 166, 115), "Drop in viewport")
                                };
                                ui.label(RichText::new(icon).color(color));
                                ui.label(format!("{}: {}", hint, item.name));
                            });
                        });
                    });
            }
        }
    }
}

fn render_context_menu(ui: &mut egui::Ui, assets: &mut AssetBrowserState, theme: &Theme) {
    ui.set_min_width(150.0);
    ui.set_max_width(150.0);

    // Colors for different categories (Unreal-style)
    let material_color = Color32::from_rgb(0, 200, 83);    // Green for materials
    let scene_color = Color32::from_rgb(115, 191, 242);    // Blue for scenes
    let folder_color = Color32::from_rgb(255, 196, 0);     // Yellow/gold for folders
    let script_color = Color32::from_rgb(255, 128, 0);     // Orange for scripts
    let text_primary = theme.text.primary.to_color32();

    // Helper to render a large menu item with icon and color
    let large_menu_item = |ui: &mut egui::Ui, icon: &str, label: &str, color: Color32| -> bool {
        let desired_size = Vec2::new(150.0, 28.0);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

        let is_hovered = response.hovered();
        if is_hovered {
            ui.painter().rect_filled(
                rect,
                4.0,
                theme.panels.item_hover.to_color32(),
            );
        }

        // Color indicator bar on left
        let indicator_rect = egui::Rect::from_min_size(
            rect.min,
            Vec2::new(3.0, rect.height()),
        );
        ui.painter().rect_filled(indicator_rect, CornerRadius::ZERO, color);

        // Icon with color
        ui.painter().text(
            egui::pos2(rect.min.x + 14.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(14.0),
            color,
        );

        // Label
        ui.painter().text(
            egui::pos2(rect.min.x + 30.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(12.0),
            text_primary,
        );

        response.clicked()
    };

    // Helper for normal menu items
    let menu_item = |ui: &mut egui::Ui, icon: &str, label: &str, color: Color32| -> bool {
        let desired_size = Vec2::new(150.0, 22.0);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

        let is_hovered = response.hovered();
        if is_hovered {
            ui.painter().rect_filled(
                rect,
                4.0,
                theme.panels.item_hover.to_color32(),
            );
        }

        // Icon with color
        ui.painter().text(
            egui::pos2(rect.min.x + 14.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(12.0),
            color,
        );

        // Label
        ui.painter().text(
            egui::pos2(rect.min.x + 30.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(11.0),
            text_primary,
        );

        response.clicked()
    };

    // === Primary Create Actions (Large Items) ===
    ui.add_space(2.0);

    if large_menu_item(ui, PALETTE, "Create Material", material_color) {
        assets.show_create_material_dialog = true;
        assets.new_material_name = "NewMaterial".to_string();
        assets.context_menu_pos = None;
    }

    if large_menu_item(ui, FILM_SCRIPT, "New Scene", scene_color) {
        assets.show_create_scene_dialog = true;
        assets.new_scene_name = "NewScene".to_string();
        assets.context_menu_pos = None;
    }

    ui.add_space(2.0);
    ui.separator();
    ui.add_space(2.0);

    // === Standard Create Actions ===
    if menu_item(ui, FOLDER_PLUS, "New Folder", folder_color) {
        assets.show_create_folder_dialog = true;
        assets.new_folder_name = "New Folder".to_string();
        assets.context_menu_pos = None;
    }

    if menu_item(ui, SCROLL, "Create Script", script_color) {
        assets.show_create_script_dialog = true;
        assets.new_script_name = "new_script".to_string();
        assets.context_menu_pos = None;
    }

    ui.add_space(2.0);
    ui.separator();
    ui.add_space(2.0);

    // === Import ===
    if menu_item(ui, DOWNLOAD, "Import", text_primary) {
        assets.import_asset_requested = true;
        assets.context_menu_pos = None;
    }

    ui.add_space(2.0);
}

fn render_create_script_dialog(ctx: &egui::Context, assets: &mut AssetBrowserState) {
    if !assets.show_create_script_dialog {
        return;
    }

    let mut open = true;
    egui::Window::new("Create Script")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut assets.new_script_name);
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button("Create").clicked() {
                    if let Some(ref target_folder) = assets.current_folder {
                        let script_name = if assets.new_script_name.ends_with(".rhai") {
                            assets.new_script_name.clone()
                        } else {
                            format!("{}.rhai", assets.new_script_name)
                        };

                        let script_path = target_folder.join(&script_name);
                        let template = create_script_template(&assets.new_script_name);

                        if let Err(e) = std::fs::write(&script_path, template) {
                            error!("Failed to create script: {}", e);
                        } else {
                            info!("Created script: {}", script_path.display());
                        }
                    }

                    assets.show_create_script_dialog = false;
                    assets.new_script_name.clear();
                }

                if ui.button("Cancel").clicked() {
                    assets.show_create_script_dialog = false;
                    assets.new_script_name.clear();
                }
            });
        });

    if !open {
        assets.show_create_script_dialog = false;
    }
}

fn render_create_folder_dialog(ctx: &egui::Context, assets: &mut AssetBrowserState) {
    if !assets.show_create_folder_dialog {
        return;
    }

    let mut open = true;
    egui::Window::new("New Folder")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut assets.new_folder_name);
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button("Create").clicked() {
                    if let Some(ref current_folder) = assets.current_folder {
                        let new_folder_path = current_folder.join(&assets.new_folder_name);

                        if let Err(e) = std::fs::create_dir_all(&new_folder_path) {
                            error!("Failed to create folder: {}", e);
                        } else {
                            info!("Created folder: {}", new_folder_path.display());
                            assets.current_folder = Some(new_folder_path);
                        }
                    }

                    assets.show_create_folder_dialog = false;
                    assets.new_folder_name.clear();
                }

                if ui.button("Cancel").clicked() {
                    assets.show_create_folder_dialog = false;
                    assets.new_folder_name.clear();
                }
            });
        });

    if !open {
        assets.show_create_folder_dialog = false;
    }
}

fn render_create_material_dialog(ctx: &egui::Context, assets: &mut AssetBrowserState) {
    if !assets.show_create_material_dialog {
        return;
    }

    let mut open = true;
    egui::Window::new("Create Material")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut assets.new_material_name);
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button("Create").clicked() {
                    if let Some(ref current_folder) = assets.current_folder {
                        let material_name = if assets.new_material_name.ends_with(".material_bp") {
                            assets.new_material_name.clone()
                        } else {
                            format!("{}.material_bp", assets.new_material_name)
                        };

                        let material_path = current_folder.join(&material_name);
                        let template = create_material_template(&assets.new_material_name);

                        if let Err(e) = std::fs::write(&material_path, template) {
                            error!("Failed to create material: {}", e);
                        } else {
                            info!("Created material: {}", material_path.display());
                        }
                    }

                    assets.show_create_material_dialog = false;
                    assets.new_material_name.clear();
                }

                if ui.button("Cancel").clicked() {
                    assets.show_create_material_dialog = false;
                    assets.new_material_name.clear();
                }
            });
        });

    if !open {
        assets.show_create_material_dialog = false;
    }
}

fn render_create_scene_dialog(ctx: &egui::Context, assets: &mut AssetBrowserState) {
    if !assets.show_create_scene_dialog {
        return;
    }

    let mut open = true;
    egui::Window::new("New Scene")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut assets.new_scene_name);
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button("Create").clicked() {
                    if let Some(ref current_folder) = assets.current_folder {
                        let scene_name = if assets.new_scene_name.ends_with(".scene") {
                            assets.new_scene_name.clone()
                        } else {
                            format!("{}.scene", assets.new_scene_name)
                        };

                        let scene_path = current_folder.join(&scene_name);
                        let template = create_scene_template(&assets.new_scene_name);

                        if let Err(e) = std::fs::write(&scene_path, template) {
                            error!("Failed to create scene: {}", e);
                        } else {
                            info!("Created scene: {}", scene_path.display());
                        }
                    }

                    assets.show_create_scene_dialog = false;
                    assets.new_scene_name.clear();
                }

                if ui.button("Cancel").clicked() {
                    assets.show_create_scene_dialog = false;
                    assets.new_scene_name.clear();
                }
            });
        });

    if !open {
        assets.show_create_scene_dialog = false;
    }
}

/// Opens the file dialog to select files for import
fn open_import_file_dialog(assets: &mut AssetBrowserState) {
    if let Some(paths) = rfd::FileDialog::new()
        .add_filter("All Assets", &["glb", "gltf", "obj", "fbx", "png", "jpg", "jpeg", "wav", "ogg", "mp3", "rhai"])
        .add_filter("3D Models", &["glb", "gltf", "obj", "fbx"])
        .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "tga"])
        .add_filter("Audio", &["wav", "ogg", "mp3"])
        .add_filter("Scripts", &["rhai"])
        .pick_files()
    {
        // Check if any models are selected - if so, show the import settings dialog
        let has_models = paths.iter().any(|p| is_model_file(p.file_name().and_then(|n| n.to_str()).unwrap_or("")));

        if has_models {
            assets.pending_import_files = paths;
            assets.show_import_dialog = true;
        } else {
            // For non-model files, import directly
            if let Some(target_folder) = assets.current_folder.clone() {
                import_files_directly(&paths, &target_folder);
            }
        }
    }
}

/// Import files directly without showing settings dialog (for non-model files)
fn import_files_directly(paths: &[PathBuf], target_folder: &PathBuf) {
    let _ = std::fs::create_dir_all(target_folder);

    for source_path in paths {
        if let Some(file_name) = source_path.file_name() {
            let dest_path = target_folder.join(file_name);
            if let Err(e) = std::fs::copy(source_path, &dest_path) {
                error!("Failed to import {}: {}", source_path.display(), e);
            } else {
                info!("Imported: {}", dest_path.display());
            }
        }
    }
}

/// Render the model import settings dialog
fn render_import_dialog(ctx: &egui::Context, assets: &mut AssetBrowserState, theme: &Theme) {
    if !assets.show_import_dialog {
        return;
    }

    let mut open = true;
    let mut should_import = false;

    // Theme colors for dialog
    let hint_color = theme.text.muted.to_color32();
    let frame_bg = theme.surfaces.faint.to_color32();
    let model_icon_color = theme.categories.rendering.accent.to_color32();
    let file_icon_color = theme.text.muted.to_color32();

    egui::Window::new(format!("{} Import Settings", DOWNLOAD))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(500.0)
        .default_height(600.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;

            // Show files being imported
            egui::CollapsingHeader::new(RichText::new("Files to Import").strong())
                .default_open(true)
                .show(ui, |ui| {
                    egui::Frame::new()
                        .fill(frame_bg)
                        .corner_radius(4.0)
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .max_height(60.0)
                                .show(ui, |ui| {
                                    for path in &assets.pending_import_files {
                                        let filename = path.file_name()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or("Unknown");
                                        let is_model = is_model_file(filename);
                                        let icon = if is_model { CUBE } else { FILE };
                                        let color = if is_model {
                                            model_icon_color
                                        } else {
                                            file_icon_color
                                        };
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(icon).color(color));
                                            ui.label(filename);
                                        });
                                    }
                                });
                        });
                });

            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .max_height(450.0)
                .show(ui, |ui| {
                    // === TRANSFORM SECTION ===
                    egui::CollapsingHeader::new(RichText::new("Transform").strong())
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Scale:");
                                ui.add(egui::DragValue::new(&mut assets.import_settings.scale)
                                    .range(0.001..=1000.0)
                                    .speed(0.01)
                                    .prefix("x"));
                                if ui.small_button("Reset").clicked() {
                                    assets.import_settings.scale = 1.0;
                                }
                            });

                            ui.horizontal(|ui| {
                                ui.label("Rotation:");
                                ui.add(egui::DragValue::new(&mut assets.import_settings.rotation_offset.0)
                                    .range(-360.0..=360.0).speed(1.0).prefix("X: ").suffix("°"));
                                ui.add(egui::DragValue::new(&mut assets.import_settings.rotation_offset.1)
                                    .range(-360.0..=360.0).speed(1.0).prefix("Y: ").suffix("°"));
                                ui.add(egui::DragValue::new(&mut assets.import_settings.rotation_offset.2)
                                    .range(-360.0..=360.0).speed(1.0).prefix("Z: ").suffix("°"));
                            });

                            ui.horizontal(|ui| {
                                ui.label("Translation:");
                                ui.add(egui::DragValue::new(&mut assets.import_settings.translation_offset.0)
                                    .speed(0.1).prefix("X: "));
                                ui.add(egui::DragValue::new(&mut assets.import_settings.translation_offset.1)
                                    .speed(0.1).prefix("Y: "));
                                ui.add(egui::DragValue::new(&mut assets.import_settings.translation_offset.2)
                                    .speed(0.1).prefix("Z: "));
                            });

                            ui.horizontal(|ui| {
                                ui.label("Convert Axes:");
                                egui::ComboBox::from_id_salt("convert_axes")
                                    .selected_text(match assets.import_settings.convert_axes {
                                        ConvertAxes::None => "None",
                                        ConvertAxes::ZUpToYUp => "Z-Up to Y-Up (Blender)",
                                        ConvertAxes::YUpToZUp => "Y-Up to Z-Up",
                                        ConvertAxes::FlipX => "Flip X",
                                        ConvertAxes::FlipZ => "Flip Z",
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut assets.import_settings.convert_axes, ConvertAxes::None, "None");
                                        ui.selectable_value(&mut assets.import_settings.convert_axes, ConvertAxes::ZUpToYUp, "Z-Up to Y-Up (Blender)");
                                        ui.selectable_value(&mut assets.import_settings.convert_axes, ConvertAxes::YUpToZUp, "Y-Up to Z-Up");
                                        ui.selectable_value(&mut assets.import_settings.convert_axes, ConvertAxes::FlipX, "Flip X");
                                        ui.selectable_value(&mut assets.import_settings.convert_axes, ConvertAxes::FlipZ, "Flip Z");
                                    });
                            });
                        });

                    // === MESH SECTION ===
                    egui::CollapsingHeader::new(RichText::new("Mesh").strong())
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Mesh Handling:");
                                egui::ComboBox::from_id_salt("mesh_handling")
                                    .selected_text(match assets.import_settings.mesh_handling {
                                        MeshHandling::KeepHierarchy => "Keep Hierarchy",
                                        MeshHandling::ExtractMeshes => "Extract Meshes",
                                        MeshHandling::FlattenHierarchy => "Flatten Hierarchy",
                                        MeshHandling::CombineAll => "Combine All",
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut assets.import_settings.mesh_handling, MeshHandling::KeepHierarchy, "Keep Hierarchy");
                                        ui.selectable_value(&mut assets.import_settings.mesh_handling, MeshHandling::ExtractMeshes, "Extract Meshes");
                                        ui.selectable_value(&mut assets.import_settings.mesh_handling, MeshHandling::FlattenHierarchy, "Flatten Hierarchy");
                                        ui.selectable_value(&mut assets.import_settings.mesh_handling, MeshHandling::CombineAll, "Combine All");
                                    });
                            });

                            if assets.import_settings.mesh_handling == MeshHandling::ExtractMeshes {
                                ui.label(RichText::new("  Each mesh will be saved as a separate .glb file").color(hint_color).small());
                            }

                            ui.checkbox(&mut assets.import_settings.combine_meshes, "Combine meshes with same material");

                            ui.add_space(4.0);
                            ui.checkbox(&mut assets.import_settings.generate_lods, "Generate LODs");
                            ui.add_enabled_ui(assets.import_settings.generate_lods, |ui| {
                                ui.horizontal(|ui| {
                                    ui.add_space(20.0);
                                    ui.label("LOD Levels:");
                                    ui.add(egui::Slider::new(&mut assets.import_settings.lod_count, 1..=6));
                                });
                                ui.horizontal(|ui| {
                                    ui.add_space(20.0);
                                    ui.label("Reduction %:");
                                    ui.add(egui::Slider::new(&mut assets.import_settings.lod_reduction, 10.0..=90.0).suffix("%"));
                                });
                            });
                        });

                    // === NORMALS & TANGENTS SECTION ===
                    egui::CollapsingHeader::new(RichText::new("Normals & Tangents").strong())
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Normals:");
                                egui::ComboBox::from_id_salt("normal_import")
                                    .selected_text(match assets.import_settings.normal_import {
                                        NormalImportMethod::Import => "Import from File",
                                        NormalImportMethod::ComputeSmooth => "Compute (Smooth)",
                                        NormalImportMethod::ComputeFlat => "Compute (Flat)",
                                        NormalImportMethod::ImportAndRecompute => "Import & Recompute Tangents",
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut assets.import_settings.normal_import, NormalImportMethod::Import, "Import from File");
                                        ui.selectable_value(&mut assets.import_settings.normal_import, NormalImportMethod::ComputeSmooth, "Compute (Smooth)");
                                        ui.selectable_value(&mut assets.import_settings.normal_import, NormalImportMethod::ComputeFlat, "Compute (Flat)");
                                        ui.selectable_value(&mut assets.import_settings.normal_import, NormalImportMethod::ImportAndRecompute, "Import & Recompute Tangents");
                                    });
                            });

                            let show_smoothing = matches!(assets.import_settings.normal_import,
                                NormalImportMethod::ComputeSmooth | NormalImportMethod::ImportAndRecompute);
                            ui.add_enabled_ui(show_smoothing, |ui| {
                                ui.horizontal(|ui| {
                                    ui.add_space(20.0);
                                    ui.label("Smoothing Angle:");
                                    ui.add(egui::Slider::new(&mut assets.import_settings.smoothing_angle, 0.0..=180.0).suffix("°"));
                                });
                            });

                            ui.horizontal(|ui| {
                                ui.label("Tangents:");
                                egui::ComboBox::from_id_salt("tangent_import")
                                    .selected_text(match assets.import_settings.tangent_import {
                                        TangentImportMethod::Import => "Import from File",
                                        TangentImportMethod::ComputeMikkTSpace => "Compute (MikkTSpace)",
                                        TangentImportMethod::None => "Don't Import",
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut assets.import_settings.tangent_import, TangentImportMethod::Import, "Import from File");
                                        ui.selectable_value(&mut assets.import_settings.tangent_import, TangentImportMethod::ComputeMikkTSpace, "Compute (MikkTSpace)");
                                        ui.selectable_value(&mut assets.import_settings.tangent_import, TangentImportMethod::None, "Don't Import");
                                    });
                            });
                        });

                    // === MATERIALS & TEXTURES SECTION ===
                    egui::CollapsingHeader::new(RichText::new("Materials & Textures").strong())
                        .default_open(true)
                        .show(ui, |ui| {
                            ui.checkbox(&mut assets.import_settings.import_materials, "Import Materials");
                            ui.checkbox(&mut assets.import_settings.import_vertex_colors, "Import Vertex Colors");

                            ui.add_space(4.0);
                            ui.checkbox(&mut assets.import_settings.extract_textures, "Extract Textures");
                            ui.add_enabled_ui(assets.import_settings.extract_textures, |ui| {
                                ui.horizontal(|ui| {
                                    ui.add_space(20.0);
                                    ui.label("Subfolder:");
                                    ui.text_edit_singleline(&mut assets.import_settings.texture_subfolder);
                                });
                                ui.label(RichText::new("  Embedded textures will be extracted to this subfolder").color(hint_color).small());
                            });
                        });

                    // === ANIMATION SECTION ===
                    egui::CollapsingHeader::new(RichText::new("Animation & Skeleton").strong())
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.checkbox(&mut assets.import_settings.import_animations, "Import Animations");
                            ui.checkbox(&mut assets.import_settings.import_skeleton, "Import Skeleton/Bones");
                            ui.checkbox(&mut assets.import_settings.import_as_skeletal, "Import as Skeletal Mesh");

                            if assets.import_settings.import_as_skeletal {
                                ui.label(RichText::new("  Mesh will be set up for skeletal animation").color(hint_color).small());
                            }
                        });

                    // === COMPRESSION SECTION ===
                    egui::CollapsingHeader::new(RichText::new("Compression (Draco)").strong())
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.checkbox(&mut assets.import_settings.draco_compression, "Enable Draco Compression");
                            ui.label(RichText::new("Draco compresses mesh geometry for smaller file sizes (glTF/glb)").color(hint_color).small());

                            ui.add_enabled_ui(assets.import_settings.draco_compression, |ui| {
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    ui.label("Compression Level:");
                                    ui.add(egui::Slider::new(&mut assets.import_settings.draco_compression_level, 0..=10));
                                });
                                ui.label(RichText::new("  Higher = smaller file, slower encoding").color(hint_color).small());

                                ui.add_space(4.0);
                                ui.label("Quantization Bits (higher = better quality):");
                                ui.horizontal(|ui| {
                                    ui.add_space(20.0);
                                    ui.label("Positions:");
                                    ui.add(egui::Slider::new(&mut assets.import_settings.draco_position_bits, 8..=16));
                                });
                                ui.horizontal(|ui| {
                                    ui.add_space(20.0);
                                    ui.label("Normals:");
                                    ui.add(egui::Slider::new(&mut assets.import_settings.draco_normal_bits, 8..=16));
                                });
                                ui.horizontal(|ui| {
                                    ui.add_space(20.0);
                                    ui.label("UVs:");
                                    ui.add(egui::Slider::new(&mut assets.import_settings.draco_uv_bits, 8..=16));
                                });
                            });
                        });

                    // === PHYSICS / COLLISION SECTION ===
                    egui::CollapsingHeader::new(RichText::new("Physics & Collision").strong())
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.checkbox(&mut assets.import_settings.generate_colliders, "Generate Collision Shapes");

                            ui.add_enabled_ui(assets.import_settings.generate_colliders, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label("Collider Type:");
                                    egui::ComboBox::from_id_salt("collider_type")
                                        .selected_text(match assets.import_settings.collider_type {
                                            ColliderImportType::ConvexHull => "Convex Hull",
                                            ColliderImportType::Trimesh => "Triangle Mesh",
                                            ColliderImportType::AABB => "Bounding Box (AABB)",
                                            ColliderImportType::OBB => "Oriented Box (OBB)",
                                            ColliderImportType::Capsule => "Capsule (Auto-fit)",
                                            ColliderImportType::Sphere => "Sphere (Auto-fit)",
                                            ColliderImportType::Decomposed => "Decomposed (V-HACD)",
                                            ColliderImportType::SimplifiedMesh => "Simplified Mesh",
                                        })
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(&mut assets.import_settings.collider_type, ColliderImportType::ConvexHull, "Convex Hull");
                                            ui.selectable_value(&mut assets.import_settings.collider_type, ColliderImportType::Trimesh, "Triangle Mesh");
                                            ui.selectable_value(&mut assets.import_settings.collider_type, ColliderImportType::AABB, "Bounding Box (AABB)");
                                            ui.selectable_value(&mut assets.import_settings.collider_type, ColliderImportType::OBB, "Oriented Box (OBB)");
                                            ui.selectable_value(&mut assets.import_settings.collider_type, ColliderImportType::Capsule, "Capsule (Auto-fit)");
                                            ui.selectable_value(&mut assets.import_settings.collider_type, ColliderImportType::Sphere, "Sphere (Auto-fit)");
                                            ui.selectable_value(&mut assets.import_settings.collider_type, ColliderImportType::Decomposed, "Decomposed (V-HACD)");
                                            ui.selectable_value(&mut assets.import_settings.collider_type, ColliderImportType::SimplifiedMesh, "Simplified Mesh");
                                        });
                                });

                                ui.checkbox(&mut assets.import_settings.simplify_collision, "Simplify Collision Mesh");
                                ui.add_enabled_ui(assets.import_settings.simplify_collision, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.add_space(20.0);
                                        ui.label("Simplification:");
                                        ui.add(egui::Slider::new(&mut assets.import_settings.collision_simplification, 0.1..=1.0)
                                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)));
                                    });
                                });
                            });
                        });

                    // === LIGHTMAPPING SECTION ===
                    egui::CollapsingHeader::new(RichText::new("Lightmapping").strong())
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.checkbox(&mut assets.import_settings.generate_lightmap_uvs, "Generate Lightmap UVs");

                            ui.add_enabled_ui(assets.import_settings.generate_lightmap_uvs, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label("UV Channel:");
                                    ui.add(egui::DragValue::new(&mut assets.import_settings.lightmap_uv_channel)
                                        .range(0..=7));
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Min Resolution:");
                                    egui::ComboBox::from_id_salt("lightmap_res")
                                        .selected_text(format!("{}", assets.import_settings.lightmap_resolution))
                                        .show_ui(ui, |ui| {
                                            for res in [32, 64, 128, 256, 512, 1024] {
                                                ui.selectable_value(&mut assets.import_settings.lightmap_resolution, res, format!("{}", res));
                                            }
                                        });
                                });
                            });
                        });
                });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // Buttons
            ui.horizontal(|ui| {
                if ui.button(RichText::new(format!("{} Import", CHECK)).strong()).clicked() {
                    should_import = true;
                }

                if ui.button(RichText::new(format!("{} Import All", CHECK))).clicked() {
                    should_import = true;
                }

                if ui.button(RichText::new(format!("{} Cancel", X))).clicked() {
                    assets.show_import_dialog = false;
                    assets.pending_import_files.clear();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Reset to Defaults").clicked() {
                        assets.import_settings = Default::default();
                    }
                });
            });
        });

    if !open {
        assets.show_import_dialog = false;
        assets.pending_import_files.clear();
    }

    if should_import {
        // Perform the import
        if let Some(target_folder) = assets.current_folder.clone() {
            perform_model_import(assets, &target_folder);
        }
        assets.show_import_dialog = false;
        assets.pending_import_files.clear();
    }
}

/// Perform the actual import with the configured settings
fn perform_model_import(assets: &AssetBrowserState, target_folder: &PathBuf) {
    let _ = std::fs::create_dir_all(target_folder);

    for source_path in &assets.pending_import_files {
        if let Some(file_name) = source_path.file_name() {
            let dest_path = target_folder.join(file_name);

            // For now, just copy the file
            // In a full implementation, you would apply the import settings here
            // (e.g., scale, coordinate flip, etc. would be stored as metadata
            // or applied during scene loading)
            if let Err(e) = std::fs::copy(source_path, &dest_path) {
                error!("Failed to import {}: {}", source_path.display(), e);
            } else {
                info!("Imported: {} (scale: {}, colliders: {})",
                    dest_path.display(),
                    assets.import_settings.scale,
                    assets.import_settings.generate_colliders
                );

                // TODO: Save import settings as a .meta file alongside the asset
                // This would allow the settings to be reapplied when the asset is loaded
            }
        }
    }
}

fn handle_import_request(assets: &mut AssetBrowserState) {
    if !assets.import_asset_requested {
        return;
    }

    assets.import_asset_requested = false;

    // Use the new import flow
    open_import_file_dialog(assets);
}

/// Process files that were dropped into the assets panel (import without spawning)
fn process_pending_file_imports(assets: &mut AssetBrowserState) {
    if assets.pending_file_imports.is_empty() {
        return;
    }

    let Some(target_folder) = assets.current_folder.clone() else {
        // No folder selected, can't import
        assets.pending_file_imports.clear();
        return;
    };

    let files_to_import = std::mem::take(&mut assets.pending_file_imports);

    for source_path in files_to_import {
        let file_name = source_path.file_name().unwrap_or_default();
        let dest_path = target_folder.join(file_name);

        match std::fs::copy(&source_path, &dest_path) {
            Ok(_) => {
                info!("Imported to assets: {}", dest_path.display());
            }
            Err(e) => {
                error!("Failed to import {}: {}", source_path.display(), e);
            }
        }
    }
}

fn create_script_template(name: &str) -> String {
    format!(r#"// Script: {}
// Called once when the script starts
fn on_ready() {{
    // Initialize your script here
}}

// Called every frame
fn on_update() {{
    // Update logic here
    // Available variables:
    //   delta - time since last frame
    //   elapsed - total elapsed time
    //   position_x, position_y, position_z - current position
    //   rotation_x, rotation_y, rotation_z - current rotation (degrees)
    //   scale_x, scale_y, scale_z - current scale
    //   input_x, input_y - movement input (-1 to 1)
    //
    // Available functions:
    //   set_position(x, y, z) - set absolute position
    //   set_rotation(x, y, z) - set rotation in degrees
    //   translate(x, y, z) - move by delta
    //   print_log(message) - print to console
    //   lerp(a, b, t), clamp(v, min, max)
    //   sin, cos, tan, sqrt, abs, floor, ceil, round
    //   min, max, pow, deg_to_rad, rad_to_deg
}}
"#, name)
}

fn create_material_template(name: &str) -> String {
    let clean_name = name.trim_end_matches(".material_bp");
    format!(r#"{{
    "version": 1,
    "graph": {{
        "name": "{}",
        "graph_type": "Material",
        "nodes": [],
        "connections": [],
        "variables": [],
        "next_node_id": 1
    }}
}}"#, clean_name)
}

fn create_scene_template(name: &str) -> String {
    format!(r#"{{
    "name": "{}",
    "entities": []
}}"#, name.trim_end_matches(".scene"))
}

struct AssetItem {
    name: String,
    path: PathBuf,
    is_folder: bool,
    icon: &'static str,
    color: Color32,
}

fn truncate_name(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        name.to_string()
    } else {
        format!("{}...", &name[..max_len.saturating_sub(3)])
    }
}

fn is_model_file(filename: &str) -> bool {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    matches!(ext.as_str(), "glb" | "gltf" | "obj" | "fbx")
}

fn is_scene_file(filename: &str) -> bool {
    // Check for .ron (Bevy scene format)
    filename.to_lowercase().ends_with(".ron")
}

fn is_blueprint_material_file(filename: &str) -> bool {
    filename.to_lowercase().ends_with(".material_bp")
}

fn is_draggable_asset(filename: &str) -> bool {
    is_model_file(filename) || is_scene_file(filename) || is_image_file(filename) || is_blueprint_material_file(filename) || is_hdr_file(filename)
}

fn is_image_file(filename: &str) -> bool {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "bmp" | "tga" | "webp")
}

fn is_hdr_file(filename: &str) -> bool {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    matches!(ext.as_str(), "hdr" | "exr")
}

fn is_script_file(path: &PathBuf) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase() == "rhai")
        .unwrap_or(false)
}

/// Get icon and color for folders based on name
fn get_folder_icon_and_color(name: &str) -> (&'static str, Color32) {
    match name.to_lowercase().as_str() {
        "assets" => (FOLDER, Color32::from_rgb(255, 210, 100)),     // Golden yellow
        "scenes" => (FOLDER, Color32::from_rgb(100, 180, 255)),     // Sky blue
        "scripts" => (FOLDER, Color32::from_rgb(130, 230, 180)),    // Mint green
        "blueprints" => (FOLDER, Color32::from_rgb(100, 180, 255)), // Blue (same as scenes)
        "materials" => (FOLDER, Color32::from_rgb(255, 130, 200)),  // Pink
        "textures" | "images" => (FOLDER, Color32::from_rgb(150, 230, 130)),  // Green
        "models" | "meshes" => (FOLDER, Color32::from_rgb(255, 170, 100)),    // Orange
        "audio" | "sounds" | "music" => (FOLDER, Color32::from_rgb(200, 130, 230)),  // Purple
        "prefabs" => (FOLDER, Color32::from_rgb(130, 180, 255)),    // Light blue
        "src" => (FOLDER, Color32::from_rgb(255, 130, 80)),         // Rust orange
        "shaders" => (FOLDER, Color32::from_rgb(180, 130, 255)),    // Violet
        _ => (FOLDER, Color32::from_rgb(170, 175, 190)),            // Neutral gray
    }
}

/// Get icon and color for a file based on its extension
fn get_file_icon_and_color(filename: &str) -> (&'static str, Color32) {
    // Check for special file types first
    let lower_name = filename.to_lowercase();

    // Blueprint files
    if lower_name.ends_with(".bp") || lower_name.ends_with(".blueprint") {
        return (BLUEPRINT, Color32::from_rgb(100, 180, 255));  // Bright blue
    }

    // Material blueprint files
    if lower_name.ends_with(".material_bp") {
        return (ATOM, Color32::from_rgb(255, 120, 200));  // Pink/magenta
    }

    // Scene files (.ron)
    if lower_name.ends_with(".ron") {
        return (FILM_SCRIPT, Color32::from_rgb(115, 200, 255));  // Sky blue
    }

    let ext = filename.rsplit('.').next().unwrap_or("");
    match ext.to_lowercase().as_str() {
        // Scripts - code icon
        "rhai" => (CODE, Color32::from_rgb(130, 230, 180)),  // Mint green
        "lua" => (CODE, Color32::from_rgb(80, 130, 230)),    // Lua blue
        "js" | "ts" => (CODE, Color32::from_rgb(240, 220, 80)),  // JavaScript yellow

        // Rust source
        "rs" => (FILE_RS, Color32::from_rgb(255, 130, 80)),  // Rust orange

        // Materials
        "mat" | "material" => (PAINT_BRUSH, Color32::from_rgb(255, 120, 180)),  // Pink

        // Images
        "png" | "jpg" | "jpeg" | "bmp" | "tga" | "webp" => (IMAGE, Color32::from_rgb(150, 230, 130)),  // Green
        "hdr" | "exr" => (SUN, Color32::from_rgb(255, 220, 100)),  // Golden for HDR

        // 3D Models
        "gltf" | "glb" | "obj" | "fbx" => (CUBE, Color32::from_rgb(255, 170, 100)),  // Orange

        // Audio
        "wav" | "ogg" | "mp3" | "flac" => (MUSIC_NOTES, Color32::from_rgb(200, 130, 230)),  // Purple

        // Video
        "mp4" | "avi" | "mov" | "webm" => (VIDEO, Color32::from_rgb(230, 100, 100)),  // Red

        // Config files
        "json" => (STACK, Color32::from_rgb(180, 180, 200)),  // Gray-blue
        "toml" => (GEAR, Color32::from_rgb(180, 180, 200)),
        "yaml" | "yml" => (STACK, Color32::from_rgb(180, 180, 200)),

        // Text/docs
        "txt" => (FILE_TEXT, Color32::from_rgb(180, 180, 200)),
        "md" => (NOTE, Color32::from_rgb(180, 200, 220)),

        // Default
        _ => (FILE, Color32::from_rgb(150, 150, 165)),
    }
}
