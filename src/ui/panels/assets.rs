#![allow(dead_code)]

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, RichText, Sense, Vec2};
use std::path::PathBuf;

use crate::core::{
    AssetBrowserState, AssetViewMode, BottomPanelTab, ColliderImportType, ConsoleState,
    ConvertAxes, ImportFileResult, LogLevel, MeshHandling, ModelImportSettings,
    NormalImportMethod, TangentImportMethod, ViewportState, SceneManagerState,
    ThumbnailCache, supports_thumbnail, supports_model_preview, supports_shader_thumbnail,
};
use crate::viewport::ModelPreviewCache;
use crate::shader_thumbnail::ShaderThumbnailCache;
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
    STACK, NOTE, MUSIC_NOTES, VIDEO, BLUEPRINT, SPARKLE, MOUNTAINS, GAME_CONTROLLER,
    GRAPHICS_CARD, INFO, WARNING, CHECK_CIRCLE, X_CIRCLE,
    ARROWS_OUT_CARDINAL,
};
use super::inspector::render_category;

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
    shader_thumbnail_cache: &mut ShaderThumbnailCache,
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
                        render_assets_content(ui, current_project, assets, scene_state, thumbnail_cache, model_preview_cache, shader_thumbnail_cache, theme);
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
    render_create_material_blueprint_dialog(ctx, assets);
    render_create_script_blueprint_dialog(ctx, assets);
    render_create_shader_dialog(ctx, assets);
    render_import_dialog(ctx, assets, theme);
    render_import_results(ctx, assets, theme);
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
    render_create_video_dialog(ctx, assets);
    render_create_audio_dialog(ctx, assets);
    render_create_animation_dialog(ctx, assets);
    render_create_texture_dialog(ctx, assets);
    render_create_particle_dialog(ctx, assets);
    render_create_level_dialog(ctx, assets);
    render_create_terrain_dialog(ctx, assets);
    render_create_material_blueprint_dialog(ctx, assets);
    render_create_script_blueprint_dialog(ctx, assets);
    render_create_shader_dialog(ctx, assets);
    render_import_dialog(ctx, assets, theme);
    render_import_results(ctx, assets, theme);
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
    shader_thumbnail_cache: &mut ShaderThumbnailCache,
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
                // When grid is hidden, show files in tree
                let show_files_in_tree = !show_grid;
                ui.allocate_ui_with_layout(
                    Vec2::new(tree_width, available_height.max(50.0)),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("assets_tree_scroll")
                            .max_height(available_height.max(50.0))
                            .show(ui, |ui| {
                                ui.set_min_width(tree_width - 8.0);
                                render_tree_navigation(ui, &ctx, assets, scene_state, project, thumbnail_cache, theme, show_files_in_tree);
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
                                    render_grid_view(ui, &ctx, assets, scene_state, &filtered_items, thumbnail_cache, model_preview_cache, shader_thumbnail_cache, theme);

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
                    assets.context_submenu = None;
                }
            }
        }

        // Close menu on Escape
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            assets.context_menu_pos = None;
            assets.context_submenu = None;
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

            // Read submenu rect from egui memory (set by render_context_menu)
            let submenu_rect: Option<egui::Rect> = ctx.memory(|mem| {
                mem.data.get_temp(egui::Id::new("assets_submenu_rect"))
            });

            // Close menu when clicking outside of both menu and submenu
            if ctx.input(|i| i.pointer.primary_clicked()) {
                if let Some(click_pos) = ctx.input(|i| i.pointer.interact_pos()) {
                    let in_menu = area_response.response.rect.contains(click_pos);
                    let in_submenu = submenu_rect.map_or(false, |r| r.contains(click_pos));
                    if !in_menu && !in_submenu {
                        assets.context_menu_pos = None;
                        assets.context_submenu = None;
                    }
                }
            }
        }
    }

    // Process pending filesystem operations
    process_pending_operations(assets);
}

/// Process pending filesystem operations (rename, move)
fn process_pending_operations(assets: &mut AssetBrowserState) {
    // Handle rename
    if let Some((old_path, new_name)) = assets.pending_rename.take() {
        if let Some(parent) = old_path.parent() {
            let new_path = parent.join(&new_name);
            match std::fs::rename(&old_path, &new_path) {
                Ok(()) => {
                    // Update selection to point to new path
                    assets.selected_assets.remove(&old_path);
                    assets.selected_assets.insert(new_path.clone());
                    if assets.selected_asset.as_ref() == Some(&old_path) {
                        assets.selected_asset = Some(new_path.clone());
                    }
                    if assets.selection_anchor.as_ref() == Some(&old_path) {
                        assets.selection_anchor = Some(new_path);
                    }
                }
                Err(e) => {
                    assets.last_error = Some(format!("Rename failed: {}", e));
                    assets.error_timeout = 5.0;
                }
            }
        }
    }

    // Handle move
    if let Some((sources, target)) = assets.pending_move.take() {
        let mut had_error = false;
        for src in sources {
            if let Some(name) = src.file_name() {
                let dest = target.join(name);
                // Prevent moving a folder into itself
                if target.starts_with(&src) {
                    assets.last_error = Some("Cannot move folder into itself".to_string());
                    assets.error_timeout = 5.0;
                    had_error = true;
                    continue;
                }
                // Don't move to same location
                if src.parent() == Some(target.as_path()) {
                    continue;
                }
                if let Err(e) = std::fs::rename(&src, &dest) {
                    assets.last_error = Some(format!("Move failed: {}", e));
                    assets.error_timeout = 5.0;
                    had_error = true;
                } else {
                    // Update selection
                    assets.selected_assets.remove(&src);
                    if assets.selected_asset.as_ref() == Some(&src) {
                        assets.selected_asset = None;
                    }
                    if assets.selection_anchor.as_ref() == Some(&src) {
                        assets.selection_anchor = None;
                    }
                }
            }
        }
        // Clear all selection state after move
        assets.selected_assets.clear();
        assets.selected_asset = None;
        assets.selection_anchor = None;
        // Ensure drag state is fully cleared
        assets.dragging_assets.clear();
        assets.dragging_asset = None;
        assets.drop_target_folder = None;
        assets.item_rects.clear();
    }

    // Decrease error timeout
    if assets.error_timeout > 0.0 {
        assets.error_timeout -= 0.016; // Assume ~60 FPS
        if assets.error_timeout <= 0.0 {
            assets.last_error = None;
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
    shader_thumbnail_cache: &mut ShaderThumbnailCache,
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
    let drop_target_color = theme.semantic.accent.to_color32();

    // Build visible_item_order for range selection
    assets.visible_item_order.clear();
    for item in items {
        assets.visible_item_order.push(item.path.clone());
    }

    // Clear item rects for marquee hit testing
    assets.item_rects.clear();

    // F2 to start rename (exactly one item selected)
    if ctx.input(|i| i.key_pressed(egui::Key::F2)) && assets.renaming_asset.is_none() {
        if assets.selected_assets.len() == 1 {
            if let Some(path) = assets.selected_assets.iter().next() {
                assets.renaming_asset = Some(path.clone());
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    assets.rename_buffer = name.to_string();
                }
                assets.rename_focus_set = false;
            }
        }
    }

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
            let has_shader_thumbnail = !item.is_folder && supports_shader_thumbnail(&item.name);

            // All items can be dragged for folder moves (folders and files)
            let (rect, response) = ui.allocate_exact_size(
                Vec2::new(tile_size, total_height),
                Sense::click_and_drag(),
            );

            // Track item rect for marquee selection
            assets.item_rects.push((item.path.clone(), rect));

            let is_hovered = response.hovered();
            if is_hovered && assets.dragging_assets.is_empty() {
                ctx.set_cursor_icon(CursorIcon::PointingHand);
            }
            let is_selected = assets.selected_assets.contains(&item.path);
            let is_drop_target = assets.drop_target_folder.as_ref() == Some(&item.path);

            // Card background with subtle gradient effect
            let bg_color = if is_drop_target {
                // Highlight folder as drop target
                Color32::from_rgba_unmultiplied(
                    drop_target_color.r(),
                    drop_target_color.g(),
                    drop_target_color.b(),
                    60,
                )
            } else if is_selected {
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

            if !is_selected && !is_hovered && !is_drop_target {
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

            // Drop target border
            if is_drop_target {
                ui.painter().rect_stroke(
                    rect,
                    8.0,
                    egui::Stroke::new(2.0, drop_target_color),
                    egui::StrokeKind::Inside,
                );
            }
            // Selection border
            else if is_selected {
                ui.painter().rect_stroke(
                    rect,
                    8.0,
                    egui::Stroke::new(2.0, selection_stroke),
                    egui::StrokeKind::Inside,
                );
            }
            // Hover glow effect
            else if is_hovered && assets.dragging_assets.is_empty() {
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
            // Try shader thumbnails
            else if has_shader_thumbnail {
                if let Some(texture_id) = shader_thumbnail_cache.get_texture_id(&item.path) {
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
                } else if !shader_thumbnail_cache.is_loading(&item.path) && !shader_thumbnail_cache.has_failed(&item.path) {
                    shader_thumbnail_cache.request_thumbnail(item.path.clone());
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
                    || (has_model_preview && model_preview_cache.is_loading(&item.path))
                    || (has_shader_thumbnail && shader_thumbnail_cache.is_loading(&item.path));
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

            // Two-line label or rename text edit
            let font_size = if tile_size < 75.0 { 10.0 } else { 11.0 };
            let line_height = font_size + 2.0;

            let label_rect = egui::Rect::from_min_size(
                egui::pos2(rect.min.x + 4.0, separator_y + separator_height + 2.0),
                Vec2::new(tile_size - 8.0, label_area_height - 4.0),
            );

            // Check if this item is being renamed
            let is_renaming = assets.renaming_asset.as_ref() == Some(&item.path);

            if is_renaming {
                // Show TextEdit instead of label for renaming
                let mut child_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(label_rect)
                        .layout(egui::Layout::centered_and_justified(egui::Direction::TopDown))
                );
                let text_edit = egui::TextEdit::singleline(&mut assets.rename_buffer)
                    .desired_width(tile_size - 12.0)
                    .font(egui::FontId::proportional(font_size));
                let te_response = child_ui.add(text_edit);

                if !assets.rename_focus_set {
                    te_response.request_focus();
                    assets.rename_focus_set = true;
                }

                let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
                let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));

                if enter || (te_response.lost_focus() && !escape) {
                    // Queue filesystem rename
                    let new_name = assets.rename_buffer.trim().to_string();
                    if !new_name.is_empty() && new_name != item.name {
                        assets.pending_rename = Some((item.path.clone(), new_name));
                    }
                    assets.renaming_asset = None;
                    assets.rename_focus_set = false;
                } else if escape {
                    assets.renaming_asset = None;
                    assets.rename_focus_set = false;
                }
            } else {
                // Normal label rendering - Split name into two lines
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
            }

            // Detect drop target when dragging over folders
            if !assets.dragging_assets.is_empty() && item.is_folder {
                if response.hovered() && !assets.dragging_assets.contains(&item.path) {
                    assets.drop_target_folder = Some(item.path.clone());
                }
            }

            // Handle interactions
            handle_item_interaction(ctx, assets, scene_state, item, &response, is_draggable);

            // Tooltip with full name
            if is_hovered && assets.dragging_asset.is_none() && assets.dragging_assets.is_empty() {
                response.on_hover_text(&item.name);
            }
        }
    });

    // Get the full available rect for marquee selection (not just rendered content)
    let grid_full_rect = ui.max_rect();

    // Handle marquee selection - detect drag start in empty space via ctx.input()
    let primary_down = ctx.input(|i| i.pointer.primary_down());
    let primary_pressed = ctx.input(|i| i.pointer.primary_pressed());
    let primary_clicked = ctx.input(|i| i.pointer.primary_clicked());

    // Safety reset: if pointer is not down, clear drag state to prevent stuck state
    if !primary_down {
        if !assets.dragging_assets.is_empty() && assets.pending_move.is_none() {
            assets.dragging_assets.clear();
        }
        if assets.dragging_asset.is_some() && assets.pending_asset_drop.is_none() {
            assets.dragging_asset = None;
        }
    }

    // Check if click/press is on empty space (not over an item)
    let press_on_empty = if let Some(press_pos) = ctx.pointer_interact_pos() {
        if grid_full_rect.contains(press_pos) {
            let mut on_item = false;
            for (_, item_rect) in &assets.item_rects {
                if item_rect.contains(press_pos) {
                    on_item = true;
                    break;
                }
            }
            !on_item
        } else {
            false
        }
    } else {
        false
    };

    // Click on empty space to deselect (without drag)
    if primary_clicked && press_on_empty && assets.marquee_start.is_none() {
        let ctrl_held = ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
        let shift_held = ctx.input(|i| i.modifiers.shift);
        if !ctrl_held && !shift_held {
            assets.selected_assets.clear();
            assets.selected_asset = None;
            assets.selection_anchor = None;
        }
    }

    // Start marquee on primary press in empty space
    if primary_pressed && assets.dragging_assets.is_empty() && assets.marquee_start.is_none() && press_on_empty {
        assets.marquee_start = ctx.pointer_interact_pos();
        let ctrl_held = ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
        let shift_held = ctx.input(|i| i.modifiers.shift);
        if !ctrl_held && !shift_held {
            assets.selected_assets.clear();
        }
    }

    // Update marquee during drag
    if primary_down && assets.marquee_start.is_some() {
        assets.marquee_current = ctx.pointer_interact_pos();
    }

    // Draw marquee rectangle
    if let (Some(start), Some(current)) = (assets.marquee_start, assets.marquee_current) {
        let marquee_rect = egui::Rect::from_two_pos(start, current);

        // Semi-transparent fill
        ui.painter().rect_filled(
            marquee_rect,
            0.0,
            Color32::from_rgba_unmultiplied(100, 150, 255, 40),
        );
        // Border
        ui.painter().rect_stroke(
            marquee_rect,
            0.0,
            egui::Stroke::new(1.0, accent_color),
            egui::StrokeKind::Inside,
        );

        // Select items that intersect marquee
        for (path, item_rect) in &assets.item_rects {
            if marquee_rect.intersects(*item_rect) {
                assets.selected_assets.insert(path.clone());
            }
        }
    }

    // End marquee and drag operations on pointer release
    if ctx.input(|i| i.pointer.any_released()) {
        // Handle drop on folder
        if !assets.dragging_assets.is_empty() {
            if let Some(target) = assets.drop_target_folder.take() {
                assets.pending_move = Some((assets.dragging_assets.clone(), target));
            }
            assets.dragging_assets.clear();
            assets.dragging_asset = None;
        }

        // End marquee
        if assets.marquee_start.is_some() {
            assets.marquee_start = None;
            assets.marquee_current = None;
        }
    }

    // Clear drop target when not over any folder
    if assets.drop_target_folder.is_some() {
        let mut over_folder = false;
        if let Some(pos) = ctx.pointer_hover_pos() {
            for (path, item_rect) in &assets.item_rects {
                if item_rect.contains(pos) {
                    // Check if this path is a folder and not being dragged
                    let is_folder = path.is_dir();
                    if is_folder && !assets.dragging_assets.contains(path) {
                        over_folder = true;
                        break;
                    }
                }
            }
        }
        if !over_folder {
            assets.drop_target_folder = None;
        }
    }
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
    show_files: bool,
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
        render_nav_tree_node(ui, ctx, assets, scene_state, &project.path, 1, thumbnail_cache, theme, show_files);
    }
}

/// Render a navigation tree node (folders only, or folders+files when show_files is true)
fn render_nav_tree_node(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    assets: &mut AssetBrowserState,
    scene_state: &mut SceneManagerState,
    folder_path: &PathBuf,
    depth: usize,
    thumbnail_cache: &mut ThumbnailCache,
    theme: &Theme,
    show_files: bool,
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
    let mut files: Vec<PathBuf> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        // Skip hidden files/folders
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') || name == "target" || name == "Cargo.lock" {
                continue;
            }
        }

        if path.is_dir() {
            folders.push(path);
        } else if show_files {
            files.push(path);
        }
    }

    folders.sort();
    files.sort();

    // Render folders
    for child_path in &folders {
        let Some(name) = child_path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        // Apply search filter
        if !assets.search.is_empty() {
            if !name.to_lowercase().contains(&assets.search.to_lowercase()) {
                // Check if any children match
                if !folder_contains_match(child_path, &assets.search) {
                    continue;
                }
            }
        }

        let is_expanded = assets.expanded_folders.contains(child_path);
        let is_current = assets.current_folder.as_ref() == Some(child_path);

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
        let arrow_response = ui.interact(arrow_rect, ui.id().with(("nav_arrow", child_path)), Sense::click());
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
                assets.expanded_folders.remove(child_path);
            } else {
                assets.expanded_folders.insert(child_path.clone());
            }
            assets.current_folder = Some(child_path.clone());
        }

        // Render children if expanded
        if is_expanded {
            render_nav_tree_node(ui, ctx, assets, scene_state, child_path, depth + 1, thumbnail_cache, theme, show_files);
        }
    }

    // Render files (when show_files is true)
    if show_files {
        for file_path in &files {
            let Some(name) = file_path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };

            // Apply search filter
            if !assets.search.is_empty() {
                if !name.to_lowercase().contains(&assets.search.to_lowercase()) {
                    continue;
                }
            }

            let (icon, color) = get_file_icon_and_color(name);
            let is_selected = assets.selected_assets.contains(file_path);
            let is_draggable = is_draggable_asset(name);

            let (rect, response) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), LIST_ROW_HEIGHT),
                if is_draggable { Sense::click_and_drag() } else { Sense::click() },
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

            // File icon (no arrow, aligned with folder names)
            let icon_x = rect.min.x + indent + 20.0;
            ui.painter().text(
                egui::pos2(icon_x, rect.center().y),
                egui::Align2::LEFT_CENTER,
                icon,
                egui::FontId::proportional(12.0),
                color,
            );

            // File name
            ui.painter().text(
                egui::pos2(icon_x + 16.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                name,
                egui::FontId::proportional(11.0),
                text_secondary,
            );

            // Handle file interactions
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
    let ctrl_held = ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
    let shift_held = ctx.input(|i| i.modifiers.shift);

    // Single click: SELECT (not navigate)
    if response.clicked() {
        if ctrl_held {
            // Toggle selection
            if assets.selected_assets.contains(&item.path) {
                assets.selected_assets.remove(&item.path);
                // Update selected_asset for compatibility
                assets.selected_asset = assets.selected_assets.iter().next().cloned();
            } else {
                assets.selected_assets.insert(item.path.clone());
                assets.selected_asset = Some(item.path.clone());
            }
        } else if shift_held {
            // Range selection using visible_item_order
            if let Some(ref anchor) = assets.selection_anchor.clone() {
                let anchor_idx = assets.visible_item_order.iter().position(|p| p == anchor);
                let current_idx = assets.visible_item_order.iter().position(|p| p == &item.path);

                if let (Some(start), Some(end)) = (anchor_idx, current_idx) {
                    let (start, end) = if start <= end { (start, end) } else { (end, start) };
                    // Clear existing selection and select range
                    assets.selected_assets.clear();
                    for idx in start..=end {
                        if let Some(path) = assets.visible_item_order.get(idx) {
                            assets.selected_assets.insert(path.clone());
                        }
                    }
                    assets.selected_asset = Some(item.path.clone());
                }
            } else {
                // No anchor, just select this item
                assets.selected_assets.clear();
                assets.selected_assets.insert(item.path.clone());
                assets.selection_anchor = Some(item.path.clone());
                assets.selected_asset = Some(item.path.clone());
            }
        } else {
            // Single select - clear others
            assets.selected_assets.clear();
            assets.selected_assets.insert(item.path.clone());
            assets.selection_anchor = Some(item.path.clone());
            assets.selected_asset = Some(item.path.clone());
        }
    }

    // Double click: NAVIGATE folders, OPEN files
    if response.double_clicked() {
        if item.is_folder {
            assets.current_folder = Some(item.path.clone());
            assets.selected_assets.clear();
            assets.selected_asset = None;
        } else if item.name.to_lowercase().ends_with(".blueprint") {
            // Open behavior blueprint files in blueprint editor
            assets.pending_blueprint_open = Some(item.path.clone());
            assets.requested_layout = Some("Blueprints".to_string());
        } else if item.path.extension().map(|e| e == "wgsl").unwrap_or(false) {
            // Open WGSL shader files in code editor and switch to Shaders layout
            super::code_editor::open_script(scene_state, item.path.clone());
            assets.requested_layout = Some("Shaders".to_string());
        } else if is_script_file(&item.path) {
            // Open script files in the editor and switch to Scripting layout
            super::code_editor::open_script(scene_state, item.path.clone());
            assets.requested_layout = Some("Scripting".to_string());
        } else if is_image_file(&item.name) {
            // Open image in preview tab and switch to Image Preview layout
            super::image_preview::open_image(scene_state, item.path.clone());
            assets.requested_layout = Some("Image Preview".to_string());
        } else if is_blueprint_material_file(&item.name) {
            // Open material files in blueprint editor and switch to Blueprints layout
            assets.pending_blueprint_open = Some(item.path.clone());
            assets.requested_layout = Some("Blueprints".to_string());
        } else if is_video_file(&item.name) {
            // Open video project in video editor layout
            open_video_project(scene_state, item.path.clone());
            assets.requested_layout = Some("Video Editor".to_string());
        } else if is_audio_project_file(&item.name) {
            // Open audio project in DAW layout
            open_audio_project(scene_state, item.path.clone());
            assets.requested_layout = Some("DAW".to_string());
        } else if is_animation_file(&item.name) {
            // Open animation in Animation layout
            open_animation_file(scene_state, item.path.clone());
            assets.requested_layout = Some("Animation".to_string());
        } else if is_texture_project_file(&item.name) {
            // Open texture project
            open_texture_project(scene_state, item.path.clone());
            assets.requested_layout = Some("Scene".to_string()); // Use scene layout with inspector
        } else if is_particle_file(&item.name) {
            // Open particle FX in particle editor layout
            open_particle_fx(scene_state, item.path.clone());
            assets.requested_layout = Some("Particle FX".to_string());
        } else if is_level_file(&item.name) {
            // Open level in level design layout
            open_level_file(scene_state, item.path.clone());
            assets.requested_layout = Some("Level Design".to_string());
        } else if is_terrain_file(&item.name) {
            // Open terrain in terrain layout
            open_terrain_file(scene_state, item.path.clone());
            assets.requested_layout = Some("Terrain".to_string());
        } else if is_scene_file(&item.name) {
            // Open scene file and switch to Scene layout
            open_scene_file(scene_state, item.path.clone());
            assets.requested_layout = Some("Scene".to_string());
        }
    }

    // Drag support - for moving items or dragging to viewport
    if response.drag_started() {
        // For viewport drops (models, images, etc.)
        if is_draggable {
            assets.dragging_asset = Some(item.path.clone());
        }

        // For folder moves - if item is selected and part of multi-selection, drag all selected
        if assets.selected_assets.contains(&item.path) && assets.selected_assets.len() > 1 {
            assets.dragging_assets = assets.selected_assets.iter().cloned().collect();
        } else {
            // Otherwise, just drag this item
            assets.dragging_assets = vec![item.path.clone()];
        }
    }

    // Show drag tooltip for viewport-draggable items
    if is_draggable && assets.dragging_asset.as_ref() == Some(&item.path) {
        if let Some(pos) = ctx.pointer_hover_pos() {
            egui::Area::new(egui::Id::new("drag_tooltip"))
                .fixed_pos(pos + Vec2::new(10.0, 10.0))
                .interactable(false)
                .order(egui::Order::Tooltip)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let lower_name = item.name.to_lowercase();
                            let (icon, color, hint) = if is_scene_file(&item.name) {
                                (FILM_SCRIPT, Color32::from_rgb(115, 191, 242), "Drop in hierarchy")
                            } else if lower_name.ends_with(".rhai") || lower_name.ends_with(".blueprint") {
                                (FILM_SCRIPT, Color32::from_rgb(130, 230, 180), "Drop on Script component")
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

fn render_context_menu(ui: &mut egui::Ui, assets: &mut AssetBrowserState, theme: &Theme) {
    let menu_width = 160.0;
    ui.set_min_width(menu_width);
    ui.set_max_width(menu_width);

    // Colors
    let material_color = Color32::from_rgb(0, 200, 83);
    let scene_color = Color32::from_rgb(115, 191, 242);
    let folder_color = Color32::from_rgb(255, 196, 0);
    let script_color = Color32::from_rgb(255, 128, 0);
    let shader_color = Color32::from_rgb(220, 120, 255);
    let blueprint_color = Color32::from_rgb(100, 160, 255);
    let media_color = Color32::from_rgb(180, 100, 220);
    let world_color = Color32::from_rgb(100, 200, 180);
    let text_primary = theme.text.primary.to_color32();
    let text_secondary = theme.text.secondary.to_color32();

    let item_height = 20.0;
    let item_font = 11.0;

    // Helper for compact menu items
    let menu_item = |ui: &mut egui::Ui, icon: &str, label: &str, color: Color32| -> bool {
        let desired_size = Vec2::new(menu_width, item_height);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

        if response.hovered() {
            ui.painter().rect_filled(rect, 3.0, theme.panels.item_hover.to_color32());
        }

        ui.painter().text(
            egui::pos2(rect.min.x + 14.0, rect.center().y),
            egui::Align2::CENTER_CENTER, icon,
            egui::FontId::proportional(item_font), color,
        );
        ui.painter().text(
            egui::pos2(rect.min.x + 28.0, rect.center().y),
            egui::Align2::LEFT_CENTER, label,
            egui::FontId::proportional(item_font), text_primary,
        );

        response.clicked()
    };

    // Helper for submenu trigger items (with arrow indicator)
    let submenu_trigger = |ui: &mut egui::Ui, icon: &str, label: &str, color: Color32, submenu_id: &str, current_submenu: &Option<String>| -> (bool, egui::Rect) {
        let desired_size = Vec2::new(menu_width, item_height);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

        let is_open = current_submenu.as_deref() == Some(submenu_id);
        if response.hovered() || is_open {
            ui.painter().rect_filled(rect, 3.0, theme.panels.item_hover.to_color32());
        }

        ui.painter().text(
            egui::pos2(rect.min.x + 14.0, rect.center().y),
            egui::Align2::CENTER_CENTER, icon,
            egui::FontId::proportional(item_font), color,
        );
        ui.painter().text(
            egui::pos2(rect.min.x + 28.0, rect.center().y),
            egui::Align2::LEFT_CENTER, label,
            egui::FontId::proportional(item_font), text_primary,
        );
        // Arrow indicator
        ui.painter().text(
            egui::pos2(rect.max.x - 10.0, rect.center().y),
            egui::Align2::CENTER_CENTER, CARET_RIGHT,
            egui::FontId::proportional(10.0), text_secondary,
        );

        (response.hovered(), rect)
    };

    // Section header
    let section_header = |ui: &mut egui::Ui, label: &str| {
        let desired_size = Vec2::new(menu_width, 14.0);
        let (rect, _) = ui.allocate_exact_size(desired_size, Sense::hover());
        ui.painter().text(
            egui::pos2(rect.min.x + 8.0, rect.center().y),
            egui::Align2::LEFT_CENTER, label,
            egui::FontId::proportional(9.0), text_secondary,
        );
    };

    ui.add_space(2.0);

    // === Create section ===
    section_header(ui, "CREATE");
    ui.add_space(1.0);

    if menu_item(ui, FOLDER_PLUS, "New Folder", folder_color) {
        assets.show_create_folder_dialog = true;
        assets.new_folder_name = "New Folder".to_string();
        assets.context_menu_pos = None;
        assets.context_submenu = None;
    }

    if menu_item(ui, PALETTE, "Material", material_color) {
        assets.show_create_material_dialog = true;
        assets.new_material_name = "NewMaterial".to_string();
        assets.context_menu_pos = None;
        assets.context_submenu = None;
    }

    if menu_item(ui, FILM_SCRIPT, "Scene", scene_color) {
        assets.show_create_scene_dialog = true;
        assets.new_scene_name = "NewScene".to_string();
        assets.context_menu_pos = None;
        assets.context_submenu = None;
    }

    if menu_item(ui, SCROLL, "Script", script_color) {
        assets.show_create_script_dialog = true;
        assets.new_script_name = "new_script".to_string();
        assets.context_menu_pos = None;
        assets.context_submenu = None;
    }

    if menu_item(ui, GRAPHICS_CARD, "Shader", shader_color) {
        assets.show_create_shader_dialog = true;
        assets.new_shader_name = "new_shader".to_string();
        assets.context_menu_pos = None;
        assets.context_submenu = None;
    }

    ui.add_space(2.0);
    ui.separator();
    ui.add_space(2.0);

    // === Blueprint submenu ===
    let (bp_hovered, bp_rect) = submenu_trigger(ui, BLUEPRINT, "Blueprint", blueprint_color, "blueprint", &assets.context_submenu);

    // === Media submenu ===
    let (media_hovered, _media_rect) = submenu_trigger(ui, MUSIC_NOTES, "Media", media_color, "media", &assets.context_submenu);

    // === World submenu ===
    let (world_hovered, _world_rect) = submenu_trigger(ui, MOUNTAINS, "World", world_color, "world", &assets.context_submenu);

    ui.add_space(2.0);
    ui.separator();
    ui.add_space(2.0);

    // === Import ===
    if menu_item(ui, DOWNLOAD, "Import", text_primary) {
        assets.import_asset_requested = true;
        assets.context_menu_pos = None;
        assets.context_submenu = None;
    }

    ui.add_space(2.0);

    // Track which submenu to open based on hover
    if bp_hovered {
        assets.context_submenu = Some("blueprint".to_string());
    } else if media_hovered {
        assets.context_submenu = Some("media".to_string());
    } else if world_hovered {
        assets.context_submenu = Some("world".to_string());
    }

    // Render active submenu as a side popup
    let menu_rect = ui.min_rect();
    if let Some(ref submenu) = assets.context_submenu.clone() {
        let submenu_x = menu_rect.max.x + 1.0;
        let submenu_y = match submenu.as_str() {
            "blueprint" => bp_rect.min.y,
            _ => bp_rect.min.y, // fallback
        };

        // Position submenu at the hovered trigger item
        let trigger_y = match submenu.as_str() {
            "blueprint" => bp_rect.min.y,
            "media" => bp_rect.min.y + item_height,
            "world" => bp_rect.min.y + item_height * 2.0,
            _ => bp_rect.min.y,
        };

        let submenu_response = egui::Area::new(egui::Id::new(format!("assets_submenu_{}", submenu)))
            .fixed_pos(egui::pos2(submenu_x, trigger_y))
            .order(egui::Order::Foreground)
            .constrain(true)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style())
                    .show(ui, |ui| {
                        let sub_width = 160.0;
                        ui.set_min_width(sub_width);
                        ui.set_max_width(sub_width);

                        let sub_item = |ui: &mut egui::Ui, icon: &str, label: &str, color: Color32| -> bool {
                            let desired_size = Vec2::new(sub_width, item_height);
                            let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());
                            if response.hovered() {
                                ui.painter().rect_filled(rect, 3.0, theme.panels.item_hover.to_color32());
                            }
                            ui.painter().text(
                                egui::pos2(rect.min.x + 14.0, rect.center().y),
                                egui::Align2::CENTER_CENTER, icon,
                                egui::FontId::proportional(item_font), color,
                            );
                            ui.painter().text(
                                egui::pos2(rect.min.x + 28.0, rect.center().y),
                                egui::Align2::LEFT_CENTER, label,
                                egui::FontId::proportional(item_font), text_primary,
                            );
                            response.clicked()
                        };

                        ui.add_space(2.0);

                        match submenu.as_str() {
                            "blueprint" => {
                                if sub_item(ui, PALETTE, "Material Blueprint", material_color) {
                                    assets.show_create_material_blueprint_dialog = true;
                                    assets.new_material_blueprint_name = "NewMaterial".to_string();
                                    assets.context_menu_pos = None;
                                    assets.context_submenu = None;
                                }
                                if sub_item(ui, SCROLL, "Script Blueprint", script_color) {
                                    assets.show_create_script_blueprint_dialog = true;
                                    assets.new_script_blueprint_name = "NewScript".to_string();
                                    assets.context_menu_pos = None;
                                    assets.context_submenu = None;
                                }
                            }
                            "media" => {
                                let video_color = Color32::from_rgb(220, 80, 80);
                                let audio_color = Color32::from_rgb(180, 100, 220);
                                let animation_color = Color32::from_rgb(100, 180, 220);
                                let texture_color = Color32::from_rgb(120, 200, 120);
                                let particle_color = Color32::from_rgb(255, 180, 50);

                                if sub_item(ui, VIDEO, "Video Project", video_color) {
                                    assets.show_create_video_dialog = true;
                                    assets.new_video_name = "NewVideo".to_string();
                                    assets.context_menu_pos = None;
                                    assets.context_submenu = None;
                                }
                                if sub_item(ui, MUSIC_NOTES, "Audio Project", audio_color) {
                                    assets.show_create_audio_dialog = true;
                                    assets.new_audio_name = "NewAudio".to_string();
                                    assets.context_menu_pos = None;
                                    assets.context_submenu = None;
                                }
                                if sub_item(ui, FILM_SCRIPT, "Animation", animation_color) {
                                    assets.show_create_animation_dialog = true;
                                    assets.new_animation_name = "NewAnimation".to_string();
                                    assets.context_menu_pos = None;
                                    assets.context_submenu = None;
                                }
                                if sub_item(ui, PAINT_BRUSH, "Texture", texture_color) {
                                    assets.show_create_texture_dialog = true;
                                    assets.new_texture_name = "NewTexture".to_string();
                                    assets.context_menu_pos = None;
                                    assets.context_submenu = None;
                                }
                                if sub_item(ui, SPARKLE, "Particle FX", particle_color) {
                                    assets.show_create_particle_dialog = true;
                                    assets.new_particle_name = "NewParticle".to_string();
                                    assets.context_menu_pos = None;
                                    assets.context_submenu = None;
                                }
                            }
                            "world" => {
                                let level_color = Color32::from_rgb(100, 200, 180);
                                let terrain_color = Color32::from_rgb(140, 180, 100);

                                if sub_item(ui, GAME_CONTROLLER, "Level", level_color) {
                                    assets.show_create_level_dialog = true;
                                    assets.new_level_name = "NewLevel".to_string();
                                    assets.context_menu_pos = None;
                                    assets.context_submenu = None;
                                }
                                if sub_item(ui, MOUNTAINS, "Terrain", terrain_color) {
                                    assets.show_create_terrain_dialog = true;
                                    assets.new_terrain_name = "NewTerrain".to_string();
                                    assets.context_menu_pos = None;
                                    assets.context_submenu = None;
                                }
                            }
                            _ => {}
                        }

                        ui.add_space(2.0);
                    });
            });

        // Store submenu rect in memory for click-outside detection
        ui.ctx().memory_mut(|mem| {
            mem.data.insert_temp(egui::Id::new("assets_submenu_rect"), submenu_response.response.rect);
        });

        // Close submenu if pointer moves away from both menu and submenu
        if let Some(pointer_pos) = ui.ctx().input(|i| i.pointer.hover_pos()) {
            let expanded_menu = menu_rect.expand(2.0);
            let expanded_submenu = submenu_response.response.rect.expand(2.0);
            if !expanded_menu.contains(pointer_pos) && !expanded_submenu.contains(pointer_pos) {
                assets.context_submenu = None;
            }
        }
    } else {
        // Clear stored submenu rect when no submenu is open
        ui.ctx().memory_mut(|mem| {
            mem.data.remove::<egui::Rect>(egui::Id::new("assets_submenu_rect"));
        });
    }
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

fn render_create_video_dialog(ctx: &egui::Context, assets: &mut AssetBrowserState) {
    if !assets.show_create_video_dialog {
        return;
    }

    let mut open = true;
    egui::Window::new("New Video Project")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut assets.new_video_name);
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button("Create").clicked() {
                    if let Some(ref current_folder) = assets.current_folder {
                        let name = if assets.new_video_name.ends_with(".video") {
                            assets.new_video_name.clone()
                        } else {
                            format!("{}.video", assets.new_video_name)
                        };

                        let path = current_folder.join(&name);
                        let template = create_video_template(&assets.new_video_name);

                        if let Err(e) = std::fs::write(&path, template) {
                            error!("Failed to create video project: {}", e);
                        } else {
                            info!("Created video project: {}", path.display());
                        }
                    }

                    assets.show_create_video_dialog = false;
                    assets.new_video_name.clear();
                }

                if ui.button("Cancel").clicked() {
                    assets.show_create_video_dialog = false;
                    assets.new_video_name.clear();
                }
            });
        });

    if !open {
        assets.show_create_video_dialog = false;
    }
}

fn render_create_audio_dialog(ctx: &egui::Context, assets: &mut AssetBrowserState) {
    if !assets.show_create_audio_dialog {
        return;
    }

    let mut open = true;
    egui::Window::new("New Audio Project")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut assets.new_audio_name);
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button("Create").clicked() {
                    if let Some(ref current_folder) = assets.current_folder {
                        let name = if assets.new_audio_name.ends_with(".audio") {
                            assets.new_audio_name.clone()
                        } else {
                            format!("{}.audio", assets.new_audio_name)
                        };

                        let path = current_folder.join(&name);
                        let template = create_audio_template(&assets.new_audio_name);

                        if let Err(e) = std::fs::write(&path, template) {
                            error!("Failed to create audio project: {}", e);
                        } else {
                            info!("Created audio project: {}", path.display());
                        }
                    }

                    assets.show_create_audio_dialog = false;
                    assets.new_audio_name.clear();
                }

                if ui.button("Cancel").clicked() {
                    assets.show_create_audio_dialog = false;
                    assets.new_audio_name.clear();
                }
            });
        });

    if !open {
        assets.show_create_audio_dialog = false;
    }
}

fn render_create_animation_dialog(ctx: &egui::Context, assets: &mut AssetBrowserState) {
    if !assets.show_create_animation_dialog {
        return;
    }

    let mut open = true;
    egui::Window::new("New Animation")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut assets.new_animation_name);
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button("Create").clicked() {
                    if let Some(ref current_folder) = assets.current_folder {
                        let name = if assets.new_animation_name.ends_with(".anim") {
                            assets.new_animation_name.clone()
                        } else {
                            format!("{}.anim", assets.new_animation_name)
                        };

                        let path = current_folder.join(&name);
                        let template = create_animation_template(&assets.new_animation_name);

                        if let Err(e) = std::fs::write(&path, template) {
                            error!("Failed to create animation: {}", e);
                        } else {
                            info!("Created animation: {}", path.display());
                        }
                    }

                    assets.show_create_animation_dialog = false;
                    assets.new_animation_name.clear();
                }

                if ui.button("Cancel").clicked() {
                    assets.show_create_animation_dialog = false;
                    assets.new_animation_name.clear();
                }
            });
        });

    if !open {
        assets.show_create_animation_dialog = false;
    }
}

fn render_create_texture_dialog(ctx: &egui::Context, assets: &mut AssetBrowserState) {
    if !assets.show_create_texture_dialog {
        return;
    }

    let mut open = true;
    egui::Window::new("New Texture")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut assets.new_texture_name);
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button("Create").clicked() {
                    if let Some(ref current_folder) = assets.current_folder {
                        let name = if assets.new_texture_name.ends_with(".texture") {
                            assets.new_texture_name.clone()
                        } else {
                            format!("{}.texture", assets.new_texture_name)
                        };

                        let path = current_folder.join(&name);
                        let template = create_texture_template(&assets.new_texture_name);

                        if let Err(e) = std::fs::write(&path, template) {
                            error!("Failed to create texture: {}", e);
                        } else {
                            info!("Created texture: {}", path.display());
                        }
                    }

                    assets.show_create_texture_dialog = false;
                    assets.new_texture_name.clear();
                }

                if ui.button("Cancel").clicked() {
                    assets.show_create_texture_dialog = false;
                    assets.new_texture_name.clear();
                }
            });
        });

    if !open {
        assets.show_create_texture_dialog = false;
    }
}

fn render_create_particle_dialog(ctx: &egui::Context, assets: &mut AssetBrowserState) {
    if !assets.show_create_particle_dialog {
        return;
    }

    let mut open = true;
    egui::Window::new("New Particle FX")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut assets.new_particle_name);
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button("Create").clicked() {
                    if let Some(ref current_folder) = assets.current_folder {
                        let name = if assets.new_particle_name.ends_with(".particle") {
                            assets.new_particle_name.clone()
                        } else {
                            format!("{}.particle", assets.new_particle_name)
                        };

                        let path = current_folder.join(&name);
                        let template = create_particle_template(&assets.new_particle_name);

                        if let Err(e) = std::fs::write(&path, template) {
                            error!("Failed to create particle FX: {}", e);
                        } else {
                            info!("Created particle FX: {}", path.display());
                        }
                    }

                    assets.show_create_particle_dialog = false;
                    assets.new_particle_name.clear();
                }

                if ui.button("Cancel").clicked() {
                    assets.show_create_particle_dialog = false;
                    assets.new_particle_name.clear();
                }
            });
        });

    if !open {
        assets.show_create_particle_dialog = false;
    }
}

fn render_create_level_dialog(ctx: &egui::Context, assets: &mut AssetBrowserState) {
    if !assets.show_create_level_dialog {
        return;
    }

    let mut open = true;
    egui::Window::new("New Level")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut assets.new_level_name);
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button("Create").clicked() {
                    if let Some(ref current_folder) = assets.current_folder {
                        let name = if assets.new_level_name.ends_with(".level") {
                            assets.new_level_name.clone()
                        } else {
                            format!("{}.level", assets.new_level_name)
                        };

                        let path = current_folder.join(&name);
                        let template = create_level_template(&assets.new_level_name);

                        if let Err(e) = std::fs::write(&path, template) {
                            error!("Failed to create level: {}", e);
                        } else {
                            info!("Created level: {}", path.display());
                        }
                    }

                    assets.show_create_level_dialog = false;
                    assets.new_level_name.clear();
                }

                if ui.button("Cancel").clicked() {
                    assets.show_create_level_dialog = false;
                    assets.new_level_name.clear();
                }
            });
        });

    if !open {
        assets.show_create_level_dialog = false;
    }
}

fn render_create_terrain_dialog(ctx: &egui::Context, assets: &mut AssetBrowserState) {
    if !assets.show_create_terrain_dialog {
        return;
    }

    let mut open = true;
    egui::Window::new("New Terrain")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut assets.new_terrain_name);
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button("Create").clicked() {
                    if let Some(ref current_folder) = assets.current_folder {
                        let name = if assets.new_terrain_name.ends_with(".terrain") {
                            assets.new_terrain_name.clone()
                        } else {
                            format!("{}.terrain", assets.new_terrain_name)
                        };

                        let path = current_folder.join(&name);
                        let template = create_terrain_template(&assets.new_terrain_name);

                        if let Err(e) = std::fs::write(&path, template) {
                            error!("Failed to create terrain: {}", e);
                        } else {
                            info!("Created terrain: {}", path.display());
                        }
                    }

                    assets.show_create_terrain_dialog = false;
                    assets.new_terrain_name.clear();
                }

                if ui.button("Cancel").clicked() {
                    assets.show_create_terrain_dialog = false;
                    assets.new_terrain_name.clear();
                }
            });
        });

    if !open {
        assets.show_create_terrain_dialog = false;
    }
}

fn render_create_material_blueprint_dialog(ctx: &egui::Context, assets: &mut AssetBrowserState) {
    if !assets.show_create_material_blueprint_dialog {
        return;
    }

    let mut open = true;
    egui::Window::new("Create Material Blueprint")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut assets.new_material_blueprint_name);
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button("Create").clicked() {
                    if let Some(ref current_folder) = assets.current_folder {
                        let name = if assets.new_material_blueprint_name.ends_with(".material_bp") {
                            assets.new_material_blueprint_name.clone()
                        } else {
                            format!("{}.material_bp", assets.new_material_blueprint_name)
                        };

                        let path = current_folder.join(&name);
                        let template = create_material_blueprint_template(&assets.new_material_blueprint_name);

                        if let Err(e) = std::fs::write(&path, template) {
                            error!("Failed to create material blueprint: {}", e);
                        } else {
                            info!("Created material blueprint: {}", path.display());
                            assets.pending_blueprint_open = Some(path);
                        }
                    }

                    assets.show_create_material_blueprint_dialog = false;
                    assets.new_material_blueprint_name.clear();
                }

                if ui.button("Cancel").clicked() {
                    assets.show_create_material_blueprint_dialog = false;
                    assets.new_material_blueprint_name.clear();
                }
            });
        });

    if !open {
        assets.show_create_material_blueprint_dialog = false;
    }
}

fn render_create_script_blueprint_dialog(ctx: &egui::Context, assets: &mut AssetBrowserState) {
    if !assets.show_create_script_blueprint_dialog {
        return;
    }

    let mut open = true;
    egui::Window::new("Create Script Blueprint")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut assets.new_script_blueprint_name);
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button("Create").clicked() {
                    if let Some(ref current_folder) = assets.current_folder {
                        let name = if assets.new_script_blueprint_name.ends_with(".blueprint") {
                            assets.new_script_blueprint_name.clone()
                        } else {
                            format!("{}.blueprint", assets.new_script_blueprint_name)
                        };

                        let path = current_folder.join(&name);
                        let template = create_script_blueprint_template(&assets.new_script_blueprint_name);

                        if let Err(e) = std::fs::write(&path, template) {
                            error!("Failed to create script blueprint: {}", e);
                        } else {
                            info!("Created script blueprint: {}", path.display());
                            assets.pending_blueprint_open = Some(path);
                        }
                    }

                    assets.show_create_script_blueprint_dialog = false;
                    assets.new_script_blueprint_name.clear();
                }

                if ui.button("Cancel").clicked() {
                    assets.show_create_script_blueprint_dialog = false;
                    assets.new_script_blueprint_name.clear();
                }
            });
        });

    if !open {
        assets.show_create_script_blueprint_dialog = false;
    }
}

fn render_create_shader_dialog(ctx: &egui::Context, assets: &mut AssetBrowserState) {
    if !assets.show_create_shader_dialog {
        return;
    }

    let mut open = true;
    egui::Window::new("Create Shader")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut assets.new_shader_name);
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button("Create").clicked() {
                    if let Some(ref current_folder) = assets.current_folder {
                        let name = if assets.new_shader_name.ends_with(".wgsl") {
                            assets.new_shader_name.clone()
                        } else {
                            format!("{}.wgsl", assets.new_shader_name)
                        };

                        let path = current_folder.join(&name);
                        let template = create_shader_template(&assets.new_shader_name);

                        if let Err(e) = std::fs::write(&path, template) {
                            error!("Failed to create shader: {}", e);
                        } else {
                            info!("Created shader: {}", path.display());
                        }
                    }

                    assets.show_create_shader_dialog = false;
                    assets.new_shader_name.clear();
                }

                if ui.button("Cancel").clicked() {
                    assets.show_create_shader_dialog = false;
                    assets.new_shader_name.clear();
                }
            });
        });

    if !open {
        assets.show_create_shader_dialog = false;
    }
}

/// Opens the file dialog to select files for import
fn open_import_file_dialog(assets: &mut AssetBrowserState) {
    if let Some(paths) = rfd::FileDialog::new()
        .add_filter("All Assets", &["glb", "gltf", "obj", "fbx", "usd", "usdz", "png", "jpg", "jpeg", "wav", "ogg", "mp3", "rhai"])
        .add_filter("3D Models", &["glb", "gltf", "obj", "fbx", "usd", "usdz"])
        .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "tga"])
        .add_filter("Audio", &["wav", "ogg", "mp3"])
        .add_filter("Scripts", &["rhai"])
        .pick_files()
    {
        // Check if any models are selected - if so, show the import settings dialog
        let has_models = paths.iter().any(|p| is_model_file(p.file_name().and_then(|n| n.to_str()).unwrap_or("")));

        if has_models {
            // Auto-detect format and apply defaults
            if let Some(ext) = paths.iter()
                .filter_map(|p| p.extension().and_then(|e| e.to_str()))
                .find(|e| is_model_file(&format!("x.{}", e)))
            {
                assets.import_settings.apply_format_defaults(ext);
            }
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

    let hint_color = theme.text.muted.to_color32();
    let model_icon_color = theme.categories.rendering.accent.to_color32();
    let file_icon_color = theme.text.muted.to_color32();

    egui::Window::new(format!("{} Import Settings", DOWNLOAD))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(460.0)
        .default_height(600.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .frame(egui::Frame::window(&ctx.style())
            .fill(theme.surfaces.panel.to_color32())
            .stroke(egui::Stroke::new(1.0, theme.widgets.border.to_color32()))
            .corner_radius(egui::CornerRadius::same(8)))
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;

            // File list header
            let file_count = assets.pending_import_files.len();
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} files selected", file_count))
                    .color(theme.text.secondary.to_color32()).small());
            });

            // File list in dark frame
            egui::Frame::new()
                .fill(Color32::from_rgb(35, 37, 42))
                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(55, 57, 62)))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::same(6))
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
                                let color = if is_model { model_icon_color } else { file_icon_color };
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(icon).color(color));
                                    ui.label(RichText::new(filename).color(Color32::from_rgb(200, 200, 210)));
                                });
                            }
                        });
                });

            // Format info banner
            {
                let first_model_ext = assets.pending_import_files.iter()
                    .filter_map(|p| p.extension().and_then(|e| e.to_str()))
                    .find(|e| is_model_file(&format!("x.{}", e)))
                    .unwrap_or("");
                if !first_model_ext.is_empty() {
                    let desc = ModelImportSettings::format_description(first_model_ext);
                    ui.add_space(2.0);
                    egui::Frame::new()
                        .fill(Color32::from_rgb(35, 45, 55))
                        .corner_radius(egui::CornerRadius::same(4))
                        .inner_margin(egui::Margin::same(6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(INFO).color(theme.semantic.accent.to_color32()));
                                ui.label(RichText::new(desc).color(theme.text.secondary.to_color32()).small());
                            });
                        });
                }
            }

            ui.add_space(4.0);

            egui::ScrollArea::vertical()
                .max_height(450.0)
                .show(ui, |ui| {
                    // === TRANSFORM ===
                    render_category(ui, ARROWS_OUT_CARDINAL, "Transform", "transform", theme, "import_transform", true, |ui| {
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

                    // === MESH ===
                    render_category(ui, CUBE, "Mesh", "rendering", theme, "import_mesh", true, |ui| {
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
                        ui.add_enabled_ui(false, |ui| {
                            ui.checkbox(&mut assets.import_settings.generate_lods, "Generate LODs");
                        });
                        ui.label(RichText::new(format!("  {} LOD generation not yet implemented", WARNING)).color(hint_color).small());
                    });

                    // === NORMALS & TANGENTS ===
                    render_category(ui, ATOM, "Normals & Tangents", "rendering", theme, "import_normals", false, |ui| {
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

                    // === MATERIALS & TEXTURES ===
                    render_category(ui, PALETTE, "Materials & Textures", "rendering", theme, "import_materials", true, |ui| {
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

                    // === ANIMATION (not yet implemented) ===
                    render_category(ui, SPARKLE, "Animation & Skeleton", "rendering", theme, "import_animation", false, |ui| {
                        ui.label(RichText::new(format!("{} Not yet implemented", WARNING)).color(hint_color).small());
                        ui.add_enabled_ui(false, |ui| {
                            ui.checkbox(&mut assets.import_settings.import_animations, "Import Animations");
                            ui.checkbox(&mut assets.import_settings.import_skeleton, "Import Skeleton/Bones");
                            ui.checkbox(&mut assets.import_settings.import_as_skeletal, "Import as Skeletal Mesh");
                        });
                    });

                    // === COMPRESSION (not yet implemented) ===
                    render_category(ui, STACK, "Compression (Draco)", "rendering", theme, "import_compression", false, |ui| {
                        ui.label(RichText::new(format!("{} Not yet implemented", WARNING)).color(hint_color).small());
                        ui.add_enabled_ui(false, |ui| {
                            ui.checkbox(&mut assets.import_settings.draco_compression, "Enable Draco Compression");
                        });
                    });

                    // === PHYSICS (not yet implemented) ===
                    render_category(ui, GAME_CONTROLLER, "Physics & Collision", "physics", theme, "import_physics", false, |ui| {
                        ui.label(RichText::new(format!("{} Not yet implemented", WARNING)).color(hint_color).small());
                        ui.add_enabled_ui(false, |ui| {
                            ui.checkbox(&mut assets.import_settings.generate_colliders, "Generate Collision Shapes");
                        });
                    });

                    // === LIGHTMAPPING (not yet implemented) ===
                    render_category(ui, SUN, "Lightmapping", "environment", theme, "import_lightmap", false, |ui| {
                        ui.label(RichText::new(format!("{} Not yet implemented", WARNING)).color(hint_color).small());
                        ui.add_enabled_ui(false, |ui| {
                            ui.checkbox(&mut assets.import_settings.generate_lightmap_uvs, "Generate Lightmap UVs");
                        });
                    });
                });

            ui.add_space(8.0);

            // Action buttons
            let model_count = assets.pending_import_files.iter()
                .filter(|p| is_model_file(p.file_name().and_then(|n| n.to_str()).unwrap_or("")))
                .count();
            let total_count = assets.pending_import_files.len();

            ui.horizontal(|ui| {
                // Import button (accent filled)
                let import_label = if model_count < total_count {
                    format!("{} Import Models ({})", CHECK, model_count)
                } else {
                    format!("{} Import ({})", CHECK, total_count)
                };
                let accent = theme.semantic.accent.to_color32();
                if ui.add(egui::Button::new(RichText::new(import_label).color(Color32::WHITE).strong())
                    .fill(accent)
                    .corner_radius(egui::CornerRadius::same(4))).clicked()
                {
                    should_import = true;
                }

                if model_count < total_count {
                    if ui.button(RichText::new(format!("{} Import All ({})", CHECK, total_count))).clicked() {
                        should_import = true;
                    }
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
        if let Some(target_folder) = assets.current_folder.clone() {
            perform_model_import(assets, &target_folder);
        }
        assets.show_import_dialog = false;
        assets.pending_import_files.clear();
    }
}

/// Perform the actual import with the configured settings
fn perform_model_import(assets: &mut AssetBrowserState, target_folder: &PathBuf) {
    let _ = std::fs::create_dir_all(target_folder);

    let pending_files = assets.pending_import_files.clone();
    let settings = assets.import_settings.clone();
    let mut results = Vec::new();

    for source_path in &pending_files {
        let filename = source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Unknown")
            .to_string();

        let file_name = match source_path.file_name() {
            Some(n) => n,
            None => {
                results.push(ImportFileResult {
                    filename,
                    success: false,
                    message: "Invalid file path".to_string(),
                    output_size: None,
                });
                continue;
            }
        };

        let dest_path = target_folder.join(file_name);

        match std::fs::copy(source_path, &dest_path) {
            Err(e) => {
                error!("Failed to import {}: {}", source_path.display(), e);
                results.push(ImportFileResult {
                    filename,
                    success: false,
                    message: format!("Copy failed: {}", e),
                    output_size: None,
                });
            }
            Ok(_) => {
                // Copy sidecar files (MTL, textures) for non-glTF formats
                let sidecars = crate::import::copy_sidecar_files(source_path, target_folder);
                let sidecar_count = sidecars.len();

                // Convert non-glTF formats to GLB
                let ext = dest_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                if crate::import::is_convertible_model(&ext) {
                    match crate::import::convert_to_glb(&dest_path, &settings) {
                        Ok(glb_path) => {
                            let output_size = std::fs::metadata(&glb_path).ok().map(|m| m.len());
                            let mut msg = format!("Converted to GLB");
                            if let Some(size) = output_size {
                                msg.push_str(&format!(" ({:.1} KB)", size as f64 / 1024.0));
                            }
                            if sidecar_count > 0 {
                                msg.push_str(&format!(", {} sidecar file(s)", sidecar_count));
                            }
                            info!("Converted to GLB: {}", glb_path.display());
                            results.push(ImportFileResult {
                                filename,
                                success: true,
                                message: msg,
                                output_size,
                            });
                        }
                        Err(e) => {
                            error!("Failed to convert {} to GLB: {}", dest_path.display(), e);
                            results.push(ImportFileResult {
                                filename,
                                success: false,
                                message: format!("Conversion failed: {}", e),
                                output_size: None,
                            });
                        }
                    }
                } else {
                    // glTF/GLB files are copied directly
                    let output_size = std::fs::metadata(&dest_path).ok().map(|m| m.len());
                    let mut msg = "Imported directly".to_string();
                    if let Some(size) = output_size {
                        msg.push_str(&format!(" ({:.1} KB)", size as f64 / 1024.0));
                    }
                    if sidecar_count > 0 {
                        msg.push_str(&format!(", {} sidecar file(s)", sidecar_count));
                    }
                    info!("Imported: {}", dest_path.display());
                    results.push(ImportFileResult {
                        filename,
                        success: true,
                        message: msg,
                        output_size,
                    });
                }
            }
        }
    }

    assets.import_status.results = results;
    assets.import_status.completed = true;
    assets.import_status.show_results = true;
}

/// Render import results overlay after import completes
fn render_import_results(ctx: &egui::Context, assets: &mut AssetBrowserState, theme: &Theme) {
    if !assets.import_status.show_results {
        return;
    }

    let mut open = true;
    let success_count = assets.import_status.results.iter().filter(|r| r.success).count();
    let fail_count = assets.import_status.results.iter().filter(|r| !r.success).count();
    let total = assets.import_status.results.len();

    let title = if fail_count == 0 {
        format!("{} Import Complete", CHECK_CIRCLE)
    } else if success_count == 0 {
        format!("{} Import Failed", X_CIRCLE)
    } else {
        format!("{} Import Complete ({} warnings)", WARNING, fail_count)
    };

    egui::Window::new(title)
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(400.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            // Summary
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} of {} files imported successfully", success_count, total)));
            });

            ui.add_space(4.0);

            // Per-file results
            egui::Frame::new()
                .fill(theme.surfaces.faint.to_color32())
                .corner_radius(4.0)
                .inner_margin(8.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .show(ui, |ui| {
                            for result in &assets.import_status.results {
                                ui.horizontal(|ui| {
                                    if result.success {
                                        ui.label(RichText::new(CHECK_CIRCLE).color(theme.semantic.success.to_color32()));
                                    } else {
                                        ui.label(RichText::new(X_CIRCLE).color(theme.semantic.error.to_color32()));
                                    }
                                    ui.label(RichText::new(&result.filename).strong());
                                });
                                ui.horizontal(|ui| {
                                    ui.add_space(22.0);
                                    ui.label(RichText::new(&result.message).color(theme.text.muted.to_color32()).small());
                                });
                                ui.add_space(2.0);
                            }
                        });
                });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Close").clicked() {
                    assets.import_status.show_results = false;
                }
            });
        });

    if !open {
        assets.import_status.show_results = false;
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

                // Copy sidecar files and convert non-glTF formats
                crate::import::copy_sidecar_files(&source_path, &target_folder);

                let ext = dest_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if crate::import::is_convertible_model(&ext) {
                    match crate::import::convert_to_glb(&dest_path, &Default::default()) {
                        Ok(glb_path) => {
                            info!("Converted to GLB: {}", glb_path.display());
                        }
                        Err(e) => {
                            error!("Failed to convert {} to GLB: {}", dest_path.display(), e);
                        }
                    }
                }
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

fn create_material_blueprint_template(name: &str) -> String {
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

fn create_script_blueprint_template(name: &str) -> String {
    let clean_name = name.trim_end_matches(".blueprint");
    format!(r#"{{
    "version": 1,
    "graph": {{
        "name": "{}",
        "graph_type": "Behavior",
        "nodes": [],
        "connections": [],
        "variables": [],
        "next_node_id": 1
    }}
}}"#, clean_name)
}

fn create_shader_template(name: &str) -> String {
    let clean_name = name.trim_end_matches(".wgsl");
    format!(r#"// Shader: {}
#import bevy_pbr::forward_io::VertexOutput

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {{
    let uv = mesh.uv;
    let color = vec3<f32>(uv.x, uv.y, 0.5);
    return vec4<f32>(color, 1.0);
}}
"#, clean_name)
}

fn create_scene_template(name: &str) -> String {
    format!(r#"{{
    "name": "{}",
    "entities": []
}}"#, name.trim_end_matches(".scene"))
}

fn create_video_template(name: &str) -> String {
    format!(r#"{{
    "name": "{}",
    "version": "1.0",
    "type": "video_project",
    "resolution": [1920, 1080],
    "framerate": 30,
    "duration": 0.0,
    "tracks": [],
    "clips": []
}}"#, name.trim_end_matches(".video"))
}

fn create_audio_template(name: &str) -> String {
    format!(r#"{{
    "name": "{}",
    "version": "1.0",
    "type": "audio_project",
    "sample_rate": 44100,
    "bit_depth": 16,
    "channels": 2,
    "bpm": 120,
    "duration": 0.0,
    "tracks": [],
    "clips": []
}}"#, name.trim_end_matches(".audio"))
}

fn create_animation_template(name: &str) -> String {
    format!(r#"{{
    "name": "{}",
    "version": "1.0",
    "type": "animation",
    "duration": 1.0,
    "framerate": 30,
    "tracks": [],
    "keyframes": []
}}"#, name.trim_end_matches(".anim"))
}

fn create_texture_template(name: &str) -> String {
    format!(r#"{{
    "name": "{}",
    "version": "1.0",
    "type": "texture",
    "width": 512,
    "height": 512,
    "format": "rgba8",
    "layers": []
}}"#, name.trim_end_matches(".texture"))
}

fn create_particle_template(name: &str) -> String {
    use crate::particles::HanabiEffectDefinition;
    let mut effect = HanabiEffectDefinition::default();
    effect.name = name.trim_end_matches(".particle").to_string();

    let pretty = ron::ser::PrettyConfig::new()
        .depth_limit(4)
        .separate_tuple_members(true);

    ron::ser::to_string_pretty(&effect, pretty).unwrap_or_else(|_| {
        // Fallback to a minimal valid RON
        format!("(name:\"{}\",capacity:1000,spawn_mode:Rate,spawn_rate:50.0,spawn_count:10,spawn_duration:0.0,spawn_cycle_count:0,spawn_starts_active:true,lifetime_min:1.0,lifetime_max:2.0,emit_shape:Point,velocity_mode:Directional,velocity_magnitude:2.0,velocity_spread:0.3,velocity_direction:(0.0,1.0,0.0),velocity_speed_min:0.0,velocity_speed_max:0.0,velocity_axis:(0.0,1.0,0.0),acceleration:(0.0,-2.0,0.0),linear_drag:0.0,radial_acceleration:0.0,tangent_acceleration:0.0,tangent_accel_axis:(0.0,1.0,0.0),conform_to_sphere:None,size_start:0.1,size_end:0.0,size_curve:[],size_start_min:0.0,size_start_max:0.0,size_non_uniform:false,size_start_x:0.1,size_start_y:0.1,size_end_x:0.0,size_end_y:0.0,screen_space_size:false,roundness:0.0,color_gradient:[(position:0.0,color:(1.0,1.0,1.0,1.0)),(position:1.0,color:(1.0,1.0,1.0,0.0))],use_flat_color:false,flat_color:(1.0,1.0,1.0,1.0),use_hdr_color:false,hdr_intensity:1.0,color_blend_mode:Modulate,blend_mode:Blend,texture_path:None,billboard_mode:FaceCamera,render_layer:0,alpha_mode:Blend,alpha_mask_threshold:0.5,orient_mode:ParallelCameraDepthPlane,rotation_speed:0.0,flipbook:None,simulation_space:Local,simulation_condition:Always,motion_integration:PostUpdate,kill_zones:[],variables:{{}})",
            name.trim_end_matches(".particle"))
    })
}

fn create_level_template(name: &str) -> String {
    format!(r#"{{
    "name": "{}",
    "version": "1.0",
    "type": "level",
    "scene_type": "3d",
    "entities": [],
    "spawn_points": [],
    "triggers": []
}}"#, name.trim_end_matches(".level"))
}

fn create_terrain_template(name: &str) -> String {
    format!(r#"{{
    "name": "{}",
    "version": "1.0",
    "type": "terrain",
    "size": [256, 256],
    "resolution": 1.0,
    "height_range": [0.0, 100.0],
    "heightmap": null,
    "layers": []
}}"#, name.trim_end_matches(".terrain"))
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
    matches!(ext.as_str(), "glb" | "gltf" | "obj" | "fbx" | "usd" | "usdz")
}

fn is_scene_file(filename: &str) -> bool {
    // Check for .ron (Bevy scene format)
    filename.to_lowercase().ends_with(".ron")
}

fn is_blueprint_material_file(filename: &str) -> bool {
    filename.to_lowercase().ends_with(".material_bp")
}

fn is_draggable_asset(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    is_model_file(filename) || is_scene_file(filename) || is_image_file(filename) || is_blueprint_material_file(filename) || is_hdr_file(filename) || is_level_file(filename) || is_terrain_file(filename) || is_particle_file(filename) || lower.ends_with(".rhai") || lower.ends_with(".blueprint") || lower.ends_with(".wgsl")
}

fn is_video_file(filename: &str) -> bool {
    filename.to_lowercase().ends_with(".video")
}

/// Open a video project in the video editor
fn open_video_project(scene_state: &mut SceneManagerState, path: PathBuf) {
    use crate::core::{OpenVideo, TabKind};

    // Check if already open
    for (idx, video) in scene_state.open_videos.iter().enumerate() {
        if video.path == path {
            scene_state.set_active_document(TabKind::Video(idx));
            return;
        }
    }

    let name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let new_idx = scene_state.open_videos.len();
    scene_state.open_videos.push(OpenVideo {
        path,
        name,
        is_modified: false,
    });

    scene_state.tab_order.push(TabKind::Video(new_idx));
    scene_state.set_active_document(TabKind::Video(new_idx));
}

/// Open an audio project in the DAW
fn open_audio_project(scene_state: &mut SceneManagerState, path: PathBuf) {
    use crate::core::{OpenAudio, TabKind};

    // Check if already open
    for (idx, audio) in scene_state.open_audios.iter().enumerate() {
        if audio.path == path {
            scene_state.set_active_document(TabKind::Audio(idx));
            return;
        }
    }

    let name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let new_idx = scene_state.open_audios.len();
    scene_state.open_audios.push(OpenAudio {
        path,
        name,
        is_modified: false,
    });

    scene_state.tab_order.push(TabKind::Audio(new_idx));
    scene_state.set_active_document(TabKind::Audio(new_idx));
}

/// Open an animation file
fn open_animation_file(scene_state: &mut SceneManagerState, path: PathBuf) {
    use crate::core::{OpenAnimation, TabKind};

    // Check if already open
    for (idx, anim) in scene_state.open_animations.iter().enumerate() {
        if anim.path == path {
            scene_state.set_active_document(TabKind::Animation(idx));
            return;
        }
    }

    let name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let new_idx = scene_state.open_animations.len();
    scene_state.open_animations.push(OpenAnimation {
        path,
        name,
        is_modified: false,
    });

    scene_state.tab_order.push(TabKind::Animation(new_idx));
    scene_state.set_active_document(TabKind::Animation(new_idx));
}

/// Open a texture project
fn open_texture_project(scene_state: &mut SceneManagerState, path: PathBuf) {
    use crate::core::{OpenTexture, TabKind};

    // Check if already open
    for (idx, tex) in scene_state.open_textures.iter().enumerate() {
        if tex.path == path {
            scene_state.set_active_document(TabKind::Texture(idx));
            return;
        }
    }

    let name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let new_idx = scene_state.open_textures.len();
    scene_state.open_textures.push(OpenTexture {
        path,
        name,
        is_modified: false,
    });

    scene_state.tab_order.push(TabKind::Texture(new_idx));
    scene_state.set_active_document(TabKind::Texture(new_idx));
}

/// Open a particle FX file
fn open_particle_fx(scene_state: &mut SceneManagerState, path: PathBuf) {
    use crate::core::{OpenParticleFX, TabKind};

    // Check if already open
    for (idx, particle) in scene_state.open_particles.iter().enumerate() {
        if particle.path == path {
            scene_state.set_active_document(TabKind::ParticleFX(idx));
            return;
        }
    }

    let name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let new_idx = scene_state.open_particles.len();
    scene_state.open_particles.push(OpenParticleFX {
        path,
        name,
        is_modified: false,
    });

    scene_state.tab_order.push(TabKind::ParticleFX(new_idx));
    scene_state.set_active_document(TabKind::ParticleFX(new_idx));
}

/// Open a level file (scene variant)
fn open_level_file(scene_state: &mut SceneManagerState, path: PathBuf) {
    use crate::core::{OpenLevel, TabKind};

    // Check if already open
    for (idx, level) in scene_state.open_levels.iter().enumerate() {
        if level.path == path {
            scene_state.set_active_document(TabKind::Level(idx));
            return;
        }
    }

    let name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let new_idx = scene_state.open_levels.len();
    scene_state.open_levels.push(OpenLevel {
        path,
        name,
        is_modified: false,
    });

    scene_state.tab_order.push(TabKind::Level(new_idx));
    scene_state.set_active_document(TabKind::Level(new_idx));
}

/// Open a terrain file (scene variant)
fn open_terrain_file(scene_state: &mut SceneManagerState, path: PathBuf) {
    use crate::core::{OpenTerrain, TabKind};

    // Check if already open
    for (idx, terrain) in scene_state.open_terrains.iter().enumerate() {
        if terrain.path == path {
            scene_state.set_active_document(TabKind::Terrain(idx));
            return;
        }
    }

    let name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let new_idx = scene_state.open_terrains.len();
    scene_state.open_terrains.push(OpenTerrain {
        path,
        name,
        is_modified: false,
    });

    scene_state.tab_order.push(TabKind::Terrain(new_idx));
    scene_state.set_active_document(TabKind::Terrain(new_idx));
}

/// Open a scene file (.ron) in the scene view
fn open_scene_file(scene_state: &mut SceneManagerState, path: PathBuf) {
    use crate::core::{SceneTab, TabKind};

    // Check if already open
    for (idx, scene) in scene_state.scene_tabs.iter().enumerate() {
        if scene.path.as_ref() == Some(&path) {
            scene_state.set_active_document(TabKind::Scene(idx));
            return;
        }
    }

    let name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .trim_end_matches(".ron")
        .to_string();

    let new_idx = scene_state.scene_tabs.len();
    scene_state.scene_tabs.push(SceneTab {
        name,
        path: Some(path),
        is_modified: false,
        camera_state: None,
    });

    scene_state.tab_order.push(TabKind::Scene(new_idx));
    scene_state.set_active_document(TabKind::Scene(new_idx));
}

fn is_audio_project_file(filename: &str) -> bool {
    filename.to_lowercase().ends_with(".audio")
}

fn is_animation_file(filename: &str) -> bool {
    filename.to_lowercase().ends_with(".anim")
}

fn is_texture_project_file(filename: &str) -> bool {
    filename.to_lowercase().ends_with(".texture")
}

fn is_particle_file(filename: &str) -> bool {
    filename.to_lowercase().ends_with(".particle")
}

fn is_level_file(filename: &str) -> bool {
    filename.to_lowercase().ends_with(".level")
}

fn is_terrain_file(filename: &str) -> bool {
    filename.to_lowercase().ends_with(".terrain")
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

    // Video project files
    if lower_name.ends_with(".video") {
        return (VIDEO, Color32::from_rgb(220, 80, 80));  // Red
    }

    // Audio project files
    if lower_name.ends_with(".audio") {
        return (MUSIC_NOTES, Color32::from_rgb(180, 100, 220));  // Purple
    }

    // Animation files
    if lower_name.ends_with(".anim") {
        return (FILM_SCRIPT, Color32::from_rgb(100, 180, 220));  // Light blue
    }

    // Texture project files
    if lower_name.ends_with(".texture") {
        return (PAINT_BRUSH, Color32::from_rgb(120, 200, 120));  // Green
    }

    // Particle FX files
    if lower_name.ends_with(".particle") {
        return (SPARKLE, Color32::from_rgb(255, 180, 50));  // Orange/gold
    }

    // Level files (scene variant)
    if lower_name.ends_with(".level") {
        return (GAME_CONTROLLER, Color32::from_rgb(100, 200, 180));  // Teal
    }

    // Terrain files (scene variant)
    if lower_name.ends_with(".terrain") {
        return (MOUNTAINS, Color32::from_rgb(140, 180, 100));  // Olive
    }

    let ext = filename.rsplit('.').next().unwrap_or("");
    match ext.to_lowercase().as_str() {
        // Scripts - code icon
        "rhai" => (CODE, Color32::from_rgb(130, 230, 180)),  // Mint green
        "lua" => (CODE, Color32::from_rgb(80, 130, 230)),    // Lua blue
        "js" | "ts" => (CODE, Color32::from_rgb(240, 220, 80)),  // JavaScript yellow

        // Shaders
        "wgsl" | "glsl" | "vert" | "frag" => (GRAPHICS_CARD, Color32::from_rgb(220, 120, 255)),  // Purple for shaders

        // Rust source
        "rs" => (FILE_RS, Color32::from_rgb(255, 130, 80)),  // Rust orange

        // Materials
        "mat" | "material" => (PAINT_BRUSH, Color32::from_rgb(255, 120, 180)),  // Pink

        // Images
        "png" | "jpg" | "jpeg" | "bmp" | "tga" | "webp" => (IMAGE, Color32::from_rgb(150, 230, 130)),  // Green
        "hdr" | "exr" => (SUN, Color32::from_rgb(255, 220, 100)),  // Golden for HDR

        // 3D Models
        "gltf" | "glb" | "obj" | "fbx" | "usd" | "usdz" => (CUBE, Color32::from_rgb(255, 170, 100)),  // Orange

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
