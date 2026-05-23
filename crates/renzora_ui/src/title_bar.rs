#![allow(unused_variables)]

//! Top title/menu bar with centered layout tabs.

use bevy_egui::egui::{self, Color32, CursorIcon, Pos2, Rect, Sense, Vec2};
use renzora_theme::Theme;

use crate::layouts::{LayoutManager, WorkspaceLayout};
use crate::panel::PanelRegistry;
use crate::window_chrome::{self, WindowActionQueue};

/// Actions returned from the title bar that the caller should handle.
pub enum TitleBarAction {
    None,
    SwitchLayout(usize),
    NewProject,
    OpenProject,
    NewScene,
    OpenScene,
    Save,
    SaveAs,
    Export,
    ToggleSettings,
    ToggleSignIn,
    OpenUserSettings,
    OpenUserLibrary,
    SignOut,
    Play,
    Stop,
    Pause,
    ScriptsOnly,
    StartTutorial,
    Undo,
    Redo,
    ResetLayout,
    ZoomIn,
    ZoomOut,
    ResetZoom,
    FrameAll,
    ToggleIsolation,
    CreateLayout(String),
    ReorderLayout { from: usize, to: usize },
    RenameLayout { index: usize, new_name: String },
    DeleteLayout(usize),
    ToggleCommandPalette,
}

const TITLE_BAR_HEIGHT: f32 = 32.0;
const WINDOW_CTRL_WIDTH: f32 = 120.0; // 3 buttons × 40px
const TAB_PADDING: f32 = 16.0;
const TAB_FONT_SIZE: f32 = 11.5;
const TAB_CORNER_RADIUS: f32 = 3.0;
const UNDERLINE_HEIGHT: f32 = 2.0;
const UNDERLINE_INSET: f32 = 3.0;

/// Play mode state passed into the title bar for rendering play/stop controls.
#[derive(Default)]
pub struct PlayModeInfo {
    pub is_playing: bool,
    pub is_paused: bool,
    pub is_scripts_only: bool,
    /// Whether the scene contains at least one `SceneCamera`. Drives the
    /// disabled state of the title bar's Play button.
    pub has_scene_camera: bool,
}


/// Render the title bar at the top of the editor window. Returns an action to handle.
pub fn render_title_bar(
    ctx: &egui::Context,
    theme: &Theme,
    registry: &PanelRegistry,
    layout_manager: &LayoutManager,
    play_mode: &PlayModeInfo,
    sign_in_open: bool,
    signed_in_username: Option<&str>,
    window_queue: &mut WindowActionQueue,
    can_undo: bool,
    can_redo: bool,
    isolation_active: bool,
) -> TitleBarAction {
    let mut action = TitleBarAction::None;
    let mut any_widget_hovered = false;

    egui::TopBottomPanel::top("renzora_title_bar")
        .exact_height(TITLE_BAR_HEIGHT)
        .show(ctx, |ui| {
            let panel_rect = ui.available_rect_before_wrap();

            // Render the three OS-style window buttons right-aligned; this
            // also reserves the rightmost strip so other widgets don't overlap.
            window_chrome::render_window_controls(
                ui,
                panel_rect,
                window_queue.maximized,
                window_queue,
            );

            // Reduce vertical padding so menu items center in the 28px bar
            ui.style_mut().spacing.button_padding = Vec2::new(6.0, 2.0);
            ui.add_space(4.0);

            egui::MenuBar::new().ui(ui, |ui| {
                // --- Left: menus ---
                ui.add_space(4.0);

                // About overlay state lives in egui memory; the Help → About
                // menu item below toggles it on click.
                let about_open_id = ui.id().with("about_overlay_open");
                let about_open =
                    ui.memory(|m| m.data.get_temp::<bool>(about_open_id).unwrap_or(false));
                if about_open {
                    let close_requested = render_about_overlay(ui.ctx(), theme);
                    if close_requested {
                        ui.memory_mut(|m| m.data.insert_temp(about_open_id, false));
                    }
                }

                // Match menu buttons' idle/hover/active look to the layout tabs
                // (transparent idle, brightened bg on hover/open, rounded corners).
                let window_bg_top = theme.surfaces.window.to_color32();
                {
                    let v = ui.visuals_mut();
                    let idle = Color32::TRANSPARENT;
                    let hover = brighten(window_bg_top, 10);
                    let open = brighten(window_bg_top, 18);
                    v.widgets.inactive.weak_bg_fill = idle;
                    v.widgets.inactive.bg_fill = idle;
                    v.widgets.hovered.weak_bg_fill = hover;
                    v.widgets.hovered.bg_fill = hover;
                    v.widgets.active.weak_bg_fill = open;
                    v.widgets.active.bg_fill = open;
                    v.widgets.open.weak_bg_fill = open;
                    v.widgets.open.bg_fill = open;
                    let r = egui::CornerRadius::same(TAB_CORNER_RADIUS as u8);
                    v.widgets.inactive.corner_radius = r;
                    v.widgets.hovered.corner_radius = r;
                    v.widgets.active.corner_radius = r;
                    v.widgets.open.corner_radius = r;
                    v.widgets.inactive.fg_stroke.color = theme.text.muted.to_color32();
                    v.widgets.hovered.fg_stroke.color = theme.text.secondary.to_color32();
                    v.widgets.active.fg_stroke.color = Color32::WHITE;
                    v.widgets.open.fg_stroke.color = Color32::WHITE;
                }

                let file_menu = ui.menu_button("File", |ui| {
                    if menu_item(ui, egui_phosphor::regular::FOLDER_PLUS, "New Project") {
                        action = TitleBarAction::NewProject;
                        ui.close();
                    }
                    if menu_item(ui, egui_phosphor::regular::FOLDER_OPEN, "Open Project...") {
                        action = TitleBarAction::OpenProject;
                        ui.close();
                    }
                    ui.separator();
                    if menu_item(ui, egui_phosphor::regular::FILE_PLUS, "New Scene") {
                        action = TitleBarAction::NewScene;
                        ui.close();
                    }
                    if menu_item(ui, egui_phosphor::regular::FILE, "Open Scene...") {
                        action = TitleBarAction::OpenScene;
                        ui.close();
                    }
                    ui.separator();
                    if menu_item(ui, egui_phosphor::regular::FLOPPY_DISK, "Save") {
                        action = TitleBarAction::Save;
                        ui.close();
                    }
                    if menu_item(ui, egui_phosphor::regular::FLOPPY_DISK_BACK, "Save As...") {
                        action = TitleBarAction::SaveAs;
                        ui.close();
                    }
                    ui.separator();
                    if menu_item(ui, egui_phosphor::regular::PACKAGE, "Export Project...") {
                        action = TitleBarAction::Export;
                        ui.close();
                    }
                    ui.separator();
                    if menu_item(ui, egui_phosphor::regular::GEAR, "Settings") {
                        action = TitleBarAction::ToggleSettings;
                        ui.close();
                    }
                });
                if file_menu.response.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    any_widget_hovered = true;
                }
                switch_top_menu_on_hover(ui.ctx(), &file_menu.response);

                let edit_menu = ui.menu_button("Edit", |ui| {
                    if menu_item_enabled(
                        ui,
                        egui_phosphor::regular::ARROW_U_UP_LEFT,
                        "Undo",
                        can_undo,
                    ) {
                        action = TitleBarAction::Undo;
                        ui.close();
                    }
                    if menu_item_enabled(
                        ui,
                        egui_phosphor::regular::ARROW_U_UP_RIGHT,
                        "Redo",
                        can_redo,
                    ) {
                        action = TitleBarAction::Redo;
                        ui.close();
                    }
                    ui.separator();
                    if menu_item(ui, egui_phosphor::regular::LAYOUT, "Reset Layout") {
                        action = TitleBarAction::ResetLayout;
                        ui.close();
                    }
                });
                if edit_menu.response.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    any_widget_hovered = true;
                }
                switch_top_menu_on_hover(ui.ctx(), &edit_menu.response);

                let view_menu = ui.menu_button("View", |ui| {
                    let (zoom_resp, _) = egui::containers::menu::SubMenuButton::from_button(
                        egui::Button::new(format!(
                            "{}  Zoom",
                            egui_phosphor::regular::MAGNIFYING_GLASS
                        ))
                        .right_text(egui_phosphor::regular::CARET_RIGHT),
                    )
                    .ui(ui, |ui| {
                        if menu_item(ui, egui_phosphor::regular::MAGNIFYING_GLASS_PLUS, "Zoom In") {
                            action = TitleBarAction::ZoomIn;
                            ui.close();
                        }
                        if menu_item(
                            ui,
                            egui_phosphor::regular::MAGNIFYING_GLASS_MINUS,
                            "Zoom Out",
                        ) {
                            action = TitleBarAction::ZoomOut;
                            ui.close();
                        }
                        ui.separator();
                        if menu_item(ui, egui_phosphor::regular::MAGNIFYING_GLASS, "Reset Zoom") {
                            action = TitleBarAction::ResetZoom;
                            ui.close();
                        }
                    });
                    if zoom_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if menu_item(ui, egui_phosphor::regular::CORNERS_OUT, "Fit All") {
                        action = TitleBarAction::FrameAll;
                        ui.close();
                    }
                    let iso_icon = if isolation_active {
                        egui_phosphor::regular::CHECK
                    } else {
                        egui_phosphor::regular::EYE
                    };
                    if menu_item(ui, iso_icon, "Isolation Mode") {
                        action = TitleBarAction::ToggleIsolation;
                        ui.close();
                    }
                    ui.separator();
                    let (layouts_resp, _) = egui::containers::menu::SubMenuButton::from_button(
                        egui::Button::new(format!(
                            "{}  Layouts",
                            egui_phosphor::regular::SQUARES_FOUR
                        ))
                        .right_text(egui_phosphor::regular::CARET_RIGHT),
                    )
                    .ui(ui, |ui| {
                        for (i, layout) in layout_manager.visible_layouts() {
                            let is_active = i == layout_manager.active_index
                                || (layout_manager
                                    .layouts
                                    .get(layout_manager.active_index)
                                    .map(|l| l.hidden)
                                    .unwrap_or(false)
                                    && i == layout_manager.last_scene_index);
                            let icon = if is_active {
                                egui_phosphor::regular::CHECK
                            } else {
                                " "
                            };
                            if menu_item(ui, icon, &layout.name) {
                                action = TitleBarAction::SwitchLayout(i);
                                ui.close();
                            }
                        }
                    });
                    if layouts_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                });
                if view_menu.response.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    any_widget_hovered = true;
                }
                switch_top_menu_on_hover(ui.ctx(), &view_menu.response);

                let help_menu = ui.menu_button("Help", |ui| {
                    if menu_item(
                        ui,
                        egui_phosphor::regular::GRADUATION_CAP,
                        "Getting Started Tutorial",
                    ) {
                        action = TitleBarAction::StartTutorial;
                        ui.close();
                    }
                    ui.separator();
                    if menu_item(ui, egui_phosphor::regular::BOOK_OPEN, "Documentation") {
                        open_url("https://renzora.com/docs");
                        ui.close();
                    }
                    if menu_item(ui, egui_phosphor::regular::YOUTUBE_LOGO, "YouTube") {
                        open_url("https://youtube.com/@renzoragame");
                        ui.close();
                    }
                    if menu_item(ui, egui_phosphor::regular::DISCORD_LOGO, "Discord") {
                        open_url("https://discord.gg/9UHUGUyDJv");
                        ui.close();
                    }
                    if menu_item(ui, egui_phosphor::regular::GITHUB_LOGO, "GitHub") {
                        open_url("https://github.com/renzora/engine");
                        ui.close();
                    }
                    ui.separator();
                    if menu_item(ui, egui_phosphor::regular::INFO, "About Renzora") {
                        ui.memory_mut(|m| m.data.insert_temp(about_open_id, true));
                        ui.close();
                    }
                });
                if help_menu.response.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    any_widget_hovered = true;
                }
                switch_top_menu_on_hover(ui.ctx(), &help_menu.response);

                // --- Center: layout tabs ---
                let font = egui::FontId::proportional(TAB_FONT_SIZE);

                // Visible (non-hidden) layouts paired with their original
                // index in `layout_manager.layouts`. Asset-mode layouts have
                // `hidden = true` and are filtered out — the editor switches
                // to them automatically when an asset tab is activated.
                let visible_layouts: Vec<(usize, &WorkspaceLayout)> =
                    layout_manager.visible_layouts().collect();

                // Measure total tabs width for centering
                let tab_widths: Vec<f32> = visible_layouts
                    .iter()
                    .map(|(_, l)| {
                        let galley = ui.painter().layout_no_wrap(
                            l.name.clone(),
                            font.clone(),
                            Color32::WHITE,
                        );
                        galley.rect.width() + TAB_PADDING
                    })
                    .collect();
                let tab_spacing = 2.0;
                let plus_width = 24.0;
                let search_width = 24.0;
                let total_tabs_width: f32 = tab_widths.iter().sum::<f32>()
                    + tab_spacing * (tab_widths.len().saturating_sub(1)) as f32
                    + tab_spacing * 2.0
                    + plus_width
                    + search_width;

                // Center the tabs in the panel
                let cursor_x = ui.cursor().left();
                let center_x = panel_rect.center().x;
                let desired_start = center_x - total_tabs_width / 2.0;
                let leading = (desired_start - cursor_x).max(12.0);
                ui.add_space(leading);

                let window_bg = theme.surfaces.window.to_color32();
                let accent = theme.semantic.accent.to_color32();
                let tab_y = panel_rect.min.y;
                let tab_h = panel_rect.height();

                // Search button — opens the global command palette.
                let search_rect = Rect::from_min_size(
                    Pos2::new(ui.cursor().left(), tab_y),
                    Vec2::new(search_width, tab_h),
                );
                let search_id = ui.id().with("title_search_btn");
                let search_resp = ui.interact(search_rect, search_id, Sense::click());
                let search_bg = if search_resp.hovered() {
                    brighten(window_bg, 12)
                } else {
                    Color32::TRANSPARENT
                };
                ui.painter().rect_filled(
                    search_rect,
                    egui::CornerRadius::same(TAB_CORNER_RADIUS as u8),
                    search_bg,
                );
                ui.painter().text(
                    search_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    egui_phosphor::regular::MAGNIFYING_GLASS,
                    egui::FontId::proportional(13.0),
                    if search_resp.hovered() {
                        Color32::WHITE
                    } else {
                        theme.text.muted.to_color32()
                    },
                );
                if search_resp.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    any_widget_hovered = true;
                }
                search_resp.clone().on_hover_text("Search (Ctrl+P)");
                if search_resp.clicked() {
                    action = TitleBarAction::ToggleCommandPalette;
                }
                ui.add_space(search_width + tab_spacing);

                // Pre-compute all tab rects so the drag hit-test can iterate
                // them after the render pass.
                let start_x = ui.cursor().left();
                let mut tab_rects: Vec<Rect> = Vec::with_capacity(visible_layouts.len());
                {
                    let mut x = start_x;
                    for (visible_idx, _) in visible_layouts.iter().enumerate() {
                        let tw = tab_widths[visible_idx];
                        tab_rects.push(Rect::from_min_size(
                            Pos2::new(x, tab_y),
                            Vec2::new(tw, tab_h),
                        ));
                        x += tw + tab_spacing;
                    }
                }

                let drag_id = ui.id().with("layout_tab_drag");
                let dragging_tab: Option<usize> = ui.memory(|m| m.data.get_temp::<usize>(drag_id));

                // Rename state: which tab is in rename mode + buffer for the
                // edited name. Stored in egui memory keyed off the title bar.
                let rename_id = ui.id().with("layout_tab_rename");
                let renaming: Option<(usize, String)> =
                    ui.memory(|m| m.data.get_temp::<(usize, String)>(rename_id));

                for (visible_idx, (i, layout)) in visible_layouts.iter().enumerate() {
                    let i = *i;
                    // A title-bar tab is "active" when either its own layout
                    // is the live one, OR the editor is currently in a hidden
                    // asset-mode layout that derives from this scene-mode one
                    // (i.e. `last_scene_index` points here). That way, opening
                    // a `.material` doesn't visually deselect "Materials" in
                    // the title bar.
                    let active_visible = i == layout_manager.active_index
                        || (layout_manager
                            .layouts
                            .get(layout_manager.active_index)
                            .map(|l| l.hidden)
                            .unwrap_or(false)
                            && i == layout_manager.last_scene_index);
                    let is_active = active_visible;
                    let tw = tab_widths[visible_idx];
                    let tab_rect = tab_rects[visible_idx];
                    let is_dragging_this = dragging_tab == Some(i);

                    let tab_id = ui.id().with(("layout_tab", i));
                    let is_renaming_this = renaming.as_ref().map(|(j, _)| *j == i).unwrap_or(false);

                    if !is_renaming_this {
                        let response = ui.interact(tab_rect, tab_id, Sense::click_and_drag());

                        if response.drag_started() {
                            ui.memory_mut(|m| m.data.insert_temp(drag_id, i));
                        }
                        if response.drag_stopped() {
                            ui.memory_mut(|m| m.data.remove::<usize>(drag_id));
                        }

                        // Background
                        let bg = if is_dragging_this {
                            brighten(window_bg, 26)
                        } else if is_active {
                            brighten(window_bg, 18)
                        } else if response.hovered() {
                            brighten(window_bg, 10)
                        } else {
                            Color32::TRANSPARENT
                        };
                        ui.painter().rect_filled(
                            tab_rect,
                            egui::CornerRadius::same(TAB_CORNER_RADIUS as u8),
                            bg,
                        );

                        let text_color = if is_active {
                            Color32::WHITE
                        } else if response.hovered() {
                            theme.text.secondary.to_color32()
                        } else {
                            theme.text.muted.to_color32()
                        };
                        ui.painter().text(
                            tab_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            &layout.name,
                            font.clone(),
                            text_color,
                        );

                        if is_active {
                            let underline_rect = Rect::from_min_size(
                                Pos2::new(
                                    tab_rect.min.x + UNDERLINE_INSET,
                                    tab_rect.max.y - UNDERLINE_HEIGHT,
                                ),
                                Vec2::new(
                                    tab_rect.width() - UNDERLINE_INSET * 2.0,
                                    UNDERLINE_HEIGHT,
                                ),
                            );
                            ui.painter().rect_filled(
                                underline_rect,
                                egui::CornerRadius::same(1),
                                accent,
                            );
                        }

                        if response.hovered() {
                            ui.ctx().set_cursor_icon(if dragging_tab.is_some() {
                                CursorIcon::Grabbing
                            } else {
                                CursorIcon::PointingHand
                            });
                            any_widget_hovered = true;
                        }

                        // Double-click → enter rename mode.
                        if response.double_clicked() {
                            ui.memory_mut(|m| {
                                m.data.insert_temp(rename_id, (i, layout.name.clone()));
                            });
                        } else if response.clicked() && !response.dragged() {
                            action = TitleBarAction::SwitchLayout(i);
                        }

                        // Right-click context menu.
                        let visible_count =
                            layout_manager.layouts.iter().filter(|l| !l.hidden).count();
                        let can_delete = visible_count > 1;
                        response.context_menu(|ui| {
                            if ui
                                .button(format!(
                                    "{}  Rename",
                                    egui_phosphor::regular::PENCIL_SIMPLE
                                ))
                                .clicked()
                            {
                                ui.memory_mut(|m| {
                                    m.data.insert_temp(rename_id, (i, layout.name.clone()));
                                });
                                ui.close();
                            }
                            if ui
                                .add_enabled(
                                    can_delete,
                                    egui::Button::new(format!(
                                        "{}  Delete",
                                        egui_phosphor::regular::TRASH
                                    )),
                                )
                                .clicked()
                            {
                                action = TitleBarAction::DeleteLayout(i);
                                ui.close();
                            }
                        });
                    } else {
                        // Inline rename: render a TextEdit in place of the label.
                        let mut buffer = renaming
                            .as_ref()
                            .map(|(_, s)| s.clone())
                            .unwrap_or_default();
                        let pad = 4.0;
                        let edit_rect = Rect::from_min_size(
                            Pos2::new(tab_rect.min.x + pad, tab_rect.min.y + 2.0),
                            Vec2::new(tab_rect.width() - pad * 2.0, tab_rect.height() - 4.0),
                        );
                        let edit_resp = ui
                            .scope_builder(egui::UiBuilder::new().max_rect(edit_rect), |ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut buffer)
                                        .frame(true)
                                        .desired_width(f32::INFINITY)
                                        .font(font.clone()),
                                )
                            })
                            .inner;
                        let focus_id = rename_id.with("focused");
                        let focused =
                            ui.memory(|m| m.data.get_temp::<bool>(focus_id).unwrap_or(false));
                        if !focused {
                            edit_resp.request_focus();
                            ui.memory_mut(|m| m.data.insert_temp(focus_id, true));
                        }

                        let trimmed = buffer.trim().to_string();
                        let original_name = layout.name.clone();
                        let conflict = !trimmed.is_empty()
                            && !trimmed.eq_ignore_ascii_case(&original_name)
                            && layout_manager
                                .layouts
                                .iter()
                                .any(|l| l.name.eq_ignore_ascii_case(&trimmed));
                        let valid = !trimmed.is_empty() && !conflict;

                        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                        let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));

                        // Persist the buffer between frames.
                        ui.memory_mut(|m| {
                            m.data.insert_temp(rename_id, (i, buffer.clone()));
                        });

                        if escape {
                            ui.memory_mut(|m| {
                                m.data.remove::<(usize, String)>(rename_id);
                                m.data.remove::<bool>(focus_id);
                            });
                        } else if (enter || edit_resp.lost_focus()) && valid {
                            if trimmed != original_name {
                                action = TitleBarAction::RenameLayout {
                                    index: i,
                                    new_name: trimmed,
                                };
                            }
                            ui.memory_mut(|m| {
                                m.data.remove::<(usize, String)>(rename_id);
                                m.data.remove::<bool>(focus_id);
                            });
                        }
                    }

                    // Advance cursor past this tab + spacing
                    ui.add_space(tw + tab_spacing);
                }

                // Deferred drag-reorder: while a tab is being dragged, draw
                // a vertical accent indicator at the drop slot the cursor is
                // pointing at. Reorder fires only on drag release so the
                // tabs don't shuffle live under the cursor.
                if let Some(drag_i) = dragging_tab {
                    if let Some(pos) = ui.ctx().pointer_interact_pos() {
                        // Visible-index of the slot the dragged tab will
                        // land at if dropped now. 0..=N where N is the
                        // visible-tab count.
                        let mut target_visible = visible_layouts.len();
                        for (visible_idx, rect) in tab_rects.iter().enumerate() {
                            if pos.x < rect.center().x {
                                target_visible = visible_idx;
                                break;
                            }
                        }

                        // Convert visible target → absolute "to" for
                        // `move_layout` (which removes-then-inserts at the
                        // post-remove index).
                        let dragged_visible =
                            visible_layouts.iter().position(|(i, _)| *i == drag_i);
                        let suppress_indicator = dragged_visible
                            .map(|dv| target_visible == dv || target_visible == dv + 1)
                            .unwrap_or(false);

                        // Draw the indicator at the slot boundary unless
                        // the cursor is hovering the dragged tab's own
                        // slot (no-op case).
                        if !suppress_indicator {
                            let line_x = if target_visible == 0 {
                                tab_rects[0].min.x - tab_spacing * 0.5
                            } else if target_visible >= tab_rects.len() {
                                tab_rects[tab_rects.len() - 1].max.x + tab_spacing * 0.5
                            } else {
                                tab_rects[target_visible].min.x - tab_spacing * 0.5
                            };
                            let indicator_rect = Rect::from_min_size(
                                Pos2::new(line_x - 1.5, tab_y + 2.0),
                                Vec2::new(3.0, tab_h - 4.0),
                            );
                            crate::drag_drop::draw_tab_insert_marker(ui, indicator_rect, theme);
                        }

                        // Dispatch reorder + clear drag state on release.
                        let pointer_released = ui.input(|i| {
                            i.pointer.any_released()
                                && i.pointer.button_released(egui::PointerButton::Primary)
                        });
                        if pointer_released {
                            if !suppress_indicator {
                                // Pre-remove insert position in absolute
                                // index space.
                                let pre_to = if target_visible >= visible_layouts.len() {
                                    layout_manager.layouts.len()
                                } else {
                                    visible_layouts[target_visible].0
                                };
                                let post_to = if drag_i < pre_to {
                                    pre_to.saturating_sub(1)
                                } else {
                                    pre_to
                                };
                                if post_to != drag_i {
                                    action = TitleBarAction::ReorderLayout {
                                        from: drag_i,
                                        to: post_to,
                                    };
                                }
                            }
                            ui.memory_mut(|m| m.data.remove::<usize>(drag_id));
                        }
                    }
                }

                // Plus button — opens a popup to name and create a new layout.
                let plus_rect = Rect::from_min_size(
                    Pos2::new(ui.cursor().left(), tab_y),
                    Vec2::new(plus_width, tab_h),
                );
                let plus_id = ui.id().with("layout_plus_btn");
                let plus_resp = ui.interact(plus_rect, plus_id, Sense::click());
                let plus_bg = if plus_resp.hovered() {
                    brighten(window_bg, 12)
                } else {
                    Color32::TRANSPARENT
                };
                ui.painter().rect_filled(
                    plus_rect,
                    egui::CornerRadius::same(TAB_CORNER_RADIUS as u8),
                    plus_bg,
                );
                ui.painter().text(
                    plus_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    egui_phosphor::regular::PLUS,
                    egui::FontId::proportional(13.0),
                    if plus_resp.hovered() {
                        Color32::WHITE
                    } else {
                        theme.text.muted.to_color32()
                    },
                );
                if plus_resp.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    any_widget_hovered = true;
                }
                let popup_open_id = plus_id.with("popup_open");
                let popup_open: bool =
                    ui.memory(|m| m.data.get_temp::<bool>(popup_open_id).unwrap_or(false));
                if plus_resp.clicked() {
                    let new = !popup_open;
                    ui.memory_mut(|m| m.data.insert_temp(popup_open_id, new));
                }

                if popup_open {
                    let new_layout_name_id = plus_id.with("name");
                    let opened_id = plus_id.with("popup_opened");
                    let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));

                    // deprecated upstream egui API; screen_rect()->content_rect() has
                    // different semantics, migrate later
                    #[allow(deprecated)]
                    let screen = ui.ctx().screen_rect();
                    let popup_w = 320.0_f32.min(screen.width() - 40.0);
                    // Centered horizontally on the + button, just below it,
                    // clamped inside the screen so it doesn't run off-edge.
                    let mut popup_x = plus_rect.center().x - popup_w * 0.5;
                    popup_x = popup_x.max(8.0).min(screen.width() - popup_w - 8.0);
                    let popup_pos = Pos2::new(popup_x, plus_rect.max.y + 4.0);

                    // Backdrop — full-screen click-to-close layer.
                    let mut close_requested = false;
                    egui::Area::new(egui::Id::new("layout_create_backdrop"))
                        .order(egui::Order::Foreground)
                        .fixed_pos(Pos2::ZERO)
                        .show(ui.ctx(), |ui| {
                            let resp = ui.allocate_rect(screen, Sense::click());
                            ui.painter().rect_filled(
                                screen,
                                0.0,
                                Color32::from_rgba_unmultiplied(0, 0, 0, 100),
                            );
                            if resp.clicked() {
                                close_requested = true;
                            }
                        });

                    let mut submit_requested = false;
                    let mut submitted_name: Option<String> = None;

                    egui::Area::new(egui::Id::new("layout_create_popup"))
                        .order(egui::Order::Tooltip)
                        .fixed_pos(popup_pos)
                        .show(ui.ctx(), |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                ui.set_width(popup_w);
                                ui.label(
                                    egui::RichText::new("New Workspace")
                                        .strong()
                                        .color(theme.text.heading.to_color32()),
                                );
                                ui.add_space(8.0);

                                let mut name: String = ui.memory(|m| {
                                    m.data
                                        .get_temp::<String>(new_layout_name_id)
                                        .unwrap_or_default()
                                });

                                let trimmed = name.trim().to_string();
                                let name_taken = layout_manager
                                    .layouts
                                    .iter()
                                    .any(|l| l.name.eq_ignore_ascii_case(&trimmed));
                                let valid = !trimmed.is_empty() && !name_taken;

                                let (changed, edit_lost_focus, enter_in_edit) = ui
                                    .horizontal(|ui| {
                                        let edit = ui.add(
                                            egui::TextEdit::singleline(&mut name)
                                                .hint_text("Workspace name")
                                                .desired_width(popup_w - 50.0),
                                        );
                                        let was_open = ui.memory(|m| {
                                            m.data.get_temp::<bool>(opened_id).unwrap_or(false)
                                        });
                                        if !was_open {
                                            edit.request_focus();
                                            ui.memory_mut(|m| m.data.insert_temp(opened_id, true));
                                        }
                                        let enter_pressed = edit.lost_focus()
                                            && ui.input(|i| i.key_pressed(egui::Key::Enter));

                                        let confirm = ui.add_enabled(
                                            valid,
                                            egui::Button::new(
                                                egui::RichText::new(egui_phosphor::regular::CHECK)
                                                    .size(14.0),
                                            )
                                            .min_size(Vec2::new(28.0, 24.0)),
                                        );
                                        let submit = confirm.clicked() || (enter_pressed && valid);
                                        if submit {
                                            submit_requested = true;
                                            submitted_name = Some(trimmed.clone());
                                        }
                                        (edit.changed(), edit.lost_focus(), enter_pressed)
                                    })
                                    .inner;

                                let _ = (changed, edit_lost_focus, enter_in_edit);
                                ui.memory_mut(|m| {
                                    m.data.insert_temp(new_layout_name_id, name.clone())
                                });

                                if name_taken {
                                    ui.add_space(4.0);
                                    ui.label(
                                        egui::RichText::new("Name already exists")
                                            .small()
                                            .color(theme.semantic.error.to_color32()),
                                    );
                                }
                            });
                        });

                    if submit_requested {
                        if let Some(n) = submitted_name {
                            action = TitleBarAction::CreateLayout(n);
                        }
                        ui.memory_mut(|m| {
                            m.data.remove::<String>(new_layout_name_id);
                            m.data.remove::<bool>(opened_id);
                            m.data.insert_temp(popup_open_id, false);
                        });
                    } else if close_requested || escape {
                        ui.memory_mut(|m| {
                            m.data.remove::<String>(new_layout_name_id);
                            m.data.remove::<bool>(opened_id);
                            m.data.insert_temp(popup_open_id, false);
                        });
                    }
                }

                ui.add_space(plus_width);

                // --- Right: play controls + sign-in + settings gear ---
                let btn_size = 20.0;
                let gear_size = 20.0;
                let sign_in_width = 80.0;
                // Reserve room on the right for the window min/max/close buttons.
                let right_margin = WINDOW_CTRL_WIDTH + 8.0;
                let in_any_play =
                    play_mode.is_playing || play_mode.is_paused || play_mode.is_scripts_only;
                // In edit mode show Play + Scripts; while playing/scripting
                // show Pause + Stop. Both flavours occupy 2 buttons so the
                // layout reserve is the same either way.
                let play_controls_width = btn_size * 2.0 + 4.0;
                let remaining = ui.available_width()
                    - play_controls_width
                    - 8.0
                    - sign_in_width
                    - 8.0
                    - gear_size
                    - right_margin;
                if remaining > 0.0 {
                    ui.add_space(remaining);
                }

                // ── Edit-mode controls: Play + Scripts ────────────────────
                if !in_any_play {
                    let play_clickable = play_mode.has_scene_camera;
                    let play_rect = Rect::from_min_size(
                        Pos2::new(ui.cursor().left(), tab_y + (tab_h - btn_size) / 2.0),
                        Vec2::splat(btn_size),
                    );
                    let play_id = ui.id().with("title_play");
                    let play_resp = ui.interact(
                        play_rect,
                        play_id,
                        if play_clickable {
                            Sense::click()
                        } else {
                            Sense::hover()
                        },
                    );
                    let play_color = if !play_clickable {
                        theme.text.muted.to_color32()
                    } else if play_resp.hovered() {
                        Color32::from_rgb(150, 230, 150)
                    } else {
                        Color32::from_rgb(110, 200, 110)
                    };
                    ui.painter().text(
                        play_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        egui_phosphor::regular::PLAY,
                        egui::FontId::proportional(14.0),
                        play_color,
                    );
                    if play_clickable && play_resp.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        any_widget_hovered = true;
                    }
                    let play_tip = if play_clickable {
                        "Play (F5)"
                    } else {
                        "Scene has no camera — add one to play"
                    };
                    play_resp.clone().on_hover_text(play_tip);
                    if play_clickable && play_resp.clicked() {
                        action = TitleBarAction::Play;
                    }
                    ui.add_space(btn_size + 4.0);

                    let scripts_rect = Rect::from_min_size(
                        Pos2::new(ui.cursor().left(), tab_y + (tab_h - btn_size) / 2.0),
                        Vec2::splat(btn_size),
                    );
                    let scripts_id = ui.id().with("title_scripts");
                    let scripts_resp = ui.interact(scripts_rect, scripts_id, Sense::click());
                    let scripts_color = if scripts_resp.hovered() {
                        Color32::WHITE
                    } else {
                        accent
                    };
                    ui.painter().text(
                        scripts_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        egui_phosphor::regular::CODE,
                        egui::FontId::proportional(14.0),
                        scripts_color,
                    );
                    if scripts_resp.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        any_widget_hovered = true;
                    }
                    scripts_resp.clone().on_hover_text("Run Scripts (Shift+F5)");
                    if scripts_resp.clicked() {
                        action = TitleBarAction::ScriptsOnly;
                    }
                    ui.add_space(btn_size);
                }

                // Play/Stop/Pause buttons (active during play / scripts modes)
                if in_any_play {
                    // Pause button
                    let pause_rect = Rect::from_min_size(
                        Pos2::new(ui.cursor().left(), tab_y + (tab_h - btn_size) / 2.0),
                        Vec2::splat(btn_size),
                    );
                    let pause_id = ui.id().with("play_pause");
                    let pause_resp = ui.interact(pause_rect, pause_id, Sense::click());
                    let pause_icon = if play_mode.is_paused {
                        egui_phosphor::regular::PLAY
                    } else {
                        egui_phosphor::regular::PAUSE
                    };
                    let pause_color = if pause_resp.hovered() {
                        Color32::WHITE
                    } else {
                        accent
                    };
                    ui.painter().text(
                        pause_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        pause_icon,
                        egui::FontId::proportional(14.0),
                        pause_color,
                    );
                    if pause_resp.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        any_widget_hovered = true;
                    }
                    if pause_resp.clicked() {
                        action = TitleBarAction::Pause;
                    }
                    ui.add_space(btn_size + 4.0);

                    // Stop button
                    let stop_rect = Rect::from_min_size(
                        Pos2::new(ui.cursor().left(), tab_y + (tab_h - btn_size) / 2.0),
                        Vec2::splat(btn_size),
                    );
                    let stop_id = ui.id().with("play_stop");
                    let stop_resp = ui.interact(stop_rect, stop_id, Sense::click());
                    let stop_color = if stop_resp.hovered() {
                        Color32::from_rgb(255, 100, 100)
                    } else {
                        Color32::from_rgb(220, 60, 60)
                    };
                    ui.painter().text(
                        stop_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        egui_phosphor::regular::STOP,
                        egui::FontId::proportional(14.0),
                        stop_color,
                    );
                    if stop_resp.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        any_widget_hovered = true;
                    }
                    if stop_resp.clicked() {
                        action = TitleBarAction::Stop;
                    }
                    ui.add_space(btn_size);
                }

                ui.add_space(8.0);

                let gear_rect = Rect::from_min_size(
                    Pos2::new(ui.cursor().left(), tab_y + (tab_h - gear_size) / 2.0),
                    Vec2::splat(gear_size),
                );
                let gear_id = ui.id().with("settings_gear");
                let gear_resp = ui.interact(gear_rect, gear_id, Sense::click());

                let gear_color = if gear_resp.hovered() {
                    accent
                } else {
                    theme.text.muted.to_color32()
                };
                ui.painter().text(
                    gear_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    egui_phosphor::regular::GEAR,
                    egui::FontId::proportional(15.0),
                    gear_color,
                );

                if gear_resp.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    any_widget_hovered = true;
                }
                if gear_resp.clicked() {
                    action = TitleBarAction::ToggleSettings;
                }

                ui.add_space(gear_size + 8.0);

                // User button — sign-in or dropdown menu when signed in
                if let Some(username) = signed_in_username {
                    // Signed in — show username button with dropdown
                    let user_menu = ui.menu_button(
                        egui::RichText::new(format!(
                            "{} {}",
                            egui_phosphor::regular::USER_CIRCLE,
                            username
                        ))
                        .size(11.0),
                        |ui| {
                            ui.set_min_width(140.0);
                            if menu_item(ui, egui_phosphor::regular::BOOKS, "My Library") {
                                action = TitleBarAction::OpenUserLibrary;
                                ui.close();
                            }
                            if menu_item(ui, egui_phosphor::regular::GEAR, "Settings") {
                                action = TitleBarAction::OpenUserSettings;
                                ui.close();
                            }
                            ui.separator();
                            if menu_item(ui, egui_phosphor::regular::SIGN_OUT, "Sign Out") {
                                action = TitleBarAction::SignOut;
                                ui.close();
                            }
                        },
                    );
                    if user_menu.response.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        any_widget_hovered = true;
                    }
                } else {
                    // Not signed in — simple sign-in button
                    let sign_in_size = Vec2::new(sign_in_width, 20.0);
                    let sign_in_rect = Rect::from_min_size(
                        Pos2::new(ui.cursor().left(), tab_y + (tab_h - 20.0) / 2.0),
                        sign_in_size,
                    );
                    let sign_in_id = ui.id().with("sign_in_btn");
                    let sign_in_resp = ui.interact(sign_in_rect, sign_in_id, Sense::click());

                    if sign_in_resp.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        any_widget_hovered = true;
                    }

                    let bg = if sign_in_open {
                        brighten(theme.surfaces.window.to_color32(), 25)
                    } else if sign_in_resp.hovered() {
                        brighten(theme.surfaces.window.to_color32(), 15)
                    } else {
                        Color32::TRANSPARENT
                    };
                    ui.painter()
                        .rect_filled(sign_in_rect, egui::CornerRadius::same(3), bg);

                    ui.painter().text(
                        Pos2::new(sign_in_rect.left() + 14.0, sign_in_rect.center().y),
                        egui::Align2::CENTER_CENTER,
                        egui_phosphor::regular::USER,
                        egui::FontId::proportional(12.0),
                        theme.text.secondary.to_color32(),
                    );

                    ui.painter().text(
                        Pos2::new(sign_in_rect.left() + 28.0, sign_in_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        "Sign In",
                        egui::FontId::proportional(11.0),
                        theme.text.secondary.to_color32(),
                    );

                    if sign_in_resp.clicked() {
                        action = TitleBarAction::ToggleSignIn;
                    }

                    ui.add_space(sign_in_width);
                }
            });

            // Drag-handle check runs LAST so all interactive widgets have
            // already registered — avoids competing with menu-button hover.
            let drag_rect = Rect::from_min_size(
                panel_rect.min,
                Vec2::new(panel_rect.width() - WINDOW_CTRL_WIDTH, panel_rect.height()),
            );
            window_chrome::render_drag_handle(ui, drag_rect, window_queue, any_widget_hovered);
        });

    action
}

/// If another top-level menu popup is already open and the user hovers this
/// menu button, switch to this one. egui 0.33's `MenuButton` only auto-switches
/// for submenus, not top-level bar menus, so we do it manually.
fn switch_top_menu_on_hover(ctx: &egui::Context, response: &egui::Response) {
    if !response.hovered() {
        return;
    }
    let popup_id = egui::Popup::default_response_id(response);
    if egui::Popup::is_any_open(ctx) && !egui::Popup::is_id_open(ctx, popup_id) {
        egui::Popup::open_id(ctx, popup_id);
    }
}

/// PNG bytes for the Renzora app icon, embedded at compile time so the title
/// bar doesn't need filesystem access. Decoded once per egui context and
/// cached as a texture handle in egui memory.
const RENZORA_ICON_PNG: &[u8] = include_bytes!("../../../icon.png");

const ENGINE_VERSION: &str = "r1-alpha5";

/// Lazily decode and upload the Renzora icon as an egui texture, caching
/// the handle in egui memory. Returns `None` if the PNG fails to decode.
fn renzora_icon_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let id = egui::Id::new("renzora_title_icon");
    if let Some(handle) = ctx.memory(|m| m.data.get_temp::<egui::TextureHandle>(id)) {
        return Some(handle);
    }
    let img = image::load_from_memory(RENZORA_ICON_PNG).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let color_image =
        egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
    let handle = ctx.load_texture(
        "renzora_title_icon",
        color_image,
        egui::TextureOptions::LINEAR,
    );
    ctx.memory_mut(|m| m.data.insert_temp(id, handle.clone()));
    Some(handle)
}

/// Animated synthwave-style perspective grid drawn inside `rect`. Mirrors
/// the splash background so the About overlay feels of-a-piece with the
/// project launcher.
fn draw_about_grid(painter: &egui::Painter, rect: Rect, grid_timer: f32, time: f64) {
    let horizon_y = rect.min.y + rect.height() * 0.42;
    let center_x = rect.center().x;
    let w = rect.width();
    let h = rect.height();

    // Slow hue cycle so the panel breathes.
    let hue = (time * 0.05) as f32 % 1.0;
    let r = (120.0 + 80.0 * (hue * std::f32::consts::TAU).cos()) as u8;
    let g = (120.0 + 80.0 * (hue * std::f32::consts::TAU + 2.09).cos()) as u8;
    let b = (120.0 + 80.0 * (hue * std::f32::consts::TAU + 4.18).cos()) as u8;
    let base = Color32::from_rgb(r, g, b);
    let grid_color = base.gamma_multiply(0.18);
    let glow_color = base.gamma_multiply(0.10);

    // Vertical perspective lines converging at the horizon.
    let num_v_lines = 16;
    let num_v_segments = 10;
    for i in 0..=num_v_lines {
        let t = i as f32 / num_v_lines as f32;
        let x_bottom = center_x + (t - 0.5) * w * 2.6;
        let x_top = center_x + (t - 0.5) * w * 0.8;

        for s in 0..num_v_segments {
            let s_start = s as f32 / num_v_segments as f32;
            let s_end = (s + 1) as f32 / num_v_segments as f32;
            let y_start = horizon_y + s_start * (rect.max.y - horizon_y);
            let y_end = horizon_y + s_end * (rect.max.y - horizon_y);
            let x_start = x_top + s_start * (x_bottom - x_top);
            let x_end = x_top + s_end * (x_bottom - x_top);
            let alpha = (s_start * 2.5).min(1.0);

            painter.line_segment(
                [Pos2::new(x_start, y_start), Pos2::new(x_end, y_end)],
                egui::Stroke::new(3.0, glow_color.gamma_multiply(alpha)),
            );
            painter.line_segment(
                [Pos2::new(x_start, y_start), Pos2::new(x_end, y_end)],
                egui::Stroke::new(1.0, grid_color.gamma_multiply(alpha)),
            );
        }
    }

    // Animated horizontal lines that slide outward from the horizon.
    let num_h_lines = 10;
    for i in 0..num_h_lines {
        let t = ((i as f32 + grid_timer) / num_h_lines as f32) % 1.0;
        let p = t * t;
        let y = horizon_y + p * (rect.max.y - horizon_y);
        let alpha = (p * 2.5).min(1.0);
        painter.line_segment(
            [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
            egui::Stroke::new(3.0, glow_color.gamma_multiply(alpha)),
        );
        painter.line_segment(
            [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
            egui::Stroke::new(1.0, grid_color.gamma_multiply(alpha)),
        );
    }

    // Subtle horizon glow line.
    painter.line_segment(
        [
            Pos2::new(rect.min.x, horizon_y),
            Pos2::new(rect.max.x, horizon_y),
        ],
        egui::Stroke::new(1.0, base.gamma_multiply(0.35)),
    );

    // Soft top fade so text above the horizon stays readable.
    let fade_top = rect.min.y;
    let fade_bottom = horizon_y;
    let fade_h = (fade_bottom - fade_top).max(1.0);
    let steps = 12;
    for i in 0..steps {
        let band_t0 = i as f32 / steps as f32;
        let band_t1 = (i + 1) as f32 / steps as f32;
        let alpha = ((1.0 - band_t0) * 60.0) as u8;
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.min.x, fade_top + band_t0 * fade_h),
                Pos2::new(rect.max.x, fade_top + band_t1 * fade_h),
            ),
            0.0,
            Color32::from_rgba_unmultiplied(0, 0, 0, alpha),
        );
    }

    let _ = h;
}

/// Render the "About Renzora" modal overlay. Returns `true` if the user
/// clicked the backdrop, the close button, or pressed Escape — the caller
/// should toggle the open flag in that case.
fn render_about_overlay(ctx: &egui::Context, theme: &Theme) -> bool {
    let mut close_requested = ctx.input(|i| i.key_pressed(egui::Key::Escape));

    // deprecated upstream egui API; screen_rect()->content_rect() has different
    // semantics, migrate later
    #[allow(deprecated)]
    let screen = ctx.screen_rect();
    let popup_w = 460.0_f32.min(screen.width() - 40.0);
    let popup_h = 380.0_f32.min(screen.height() - 80.0);
    let popup_rect = Rect::from_center_size(screen.center(), Vec2::new(popup_w, popup_h));

    // Backdrop dim — full-screen click-to-close.
    egui::Area::new(egui::Id::new("about_overlay_backdrop"))
        .order(egui::Order::Foreground)
        .fixed_pos(Pos2::ZERO)
        .show(ctx, |ui| {
            let resp = ui.allocate_rect(screen, Sense::click());
            ui.painter()
                .rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 170));
            if resp.clicked() {
                close_requested = true;
            }
        });

    egui::Area::new(egui::Id::new("about_overlay_panel"))
        .order(egui::Order::Tooltip)
        .fixed_pos(popup_rect.min)
        .show(ctx, |ui| {
            // Per-frame animation timer state.
            let timer_id = egui::Id::new("about_grid_timer");
            let dt = ui.input(|i| i.unstable_dt);
            let time = ui.input(|i| i.time);
            let mut grid_timer = ui
                .ctx()
                .memory(|m| m.data.get_temp::<f32>(timer_id).unwrap_or(0.0));
            grid_timer = (grid_timer + dt * 0.35) % 1.0;
            ui.ctx()
                .memory_mut(|m| m.data.insert_temp(timer_id, grid_timer));

            // Solid panel background underneath the animated grid.
            let panel_radius = egui::CornerRadius::same(10);
            let bg = theme.surfaces.window.to_color32();
            let dark_bg = Color32::from_rgba_unmultiplied(
                (bg.r() as u16 * 70 / 100) as u8,
                (bg.g() as u16 * 70 / 100) as u8,
                (bg.b() as u16 * 70 / 100) as u8,
                250,
            );
            ui.painter().rect_filled(popup_rect, panel_radius, dark_bg);

            // Animated grid clipped to the panel rect.
            let grid_painter = ui.painter().clone().with_clip_rect(popup_rect);
            draw_about_grid(&grid_painter, popup_rect, grid_timer, time);

            // Accent border on top of the grid.
            ui.painter().rect_stroke(
                popup_rect,
                panel_radius,
                egui::Stroke::new(1.0, theme.semantic.accent.to_color32().gamma_multiply(0.6)),
                egui::StrokeKind::Inside,
            );

            // Close button at the top-right corner.
            let close_size = 22.0;
            let close_rect = Rect::from_min_size(
                Pos2::new(
                    popup_rect.max.x - close_size - 10.0,
                    popup_rect.min.y + 10.0,
                ),
                Vec2::splat(close_size),
            );
            let close_resp =
                ui.interact(close_rect, egui::Id::new("about_close_btn"), Sense::click());
            if close_resp.hovered() {
                ui.painter().rect_filled(
                    close_rect,
                    egui::CornerRadius::same(4),
                    Color32::from_rgba_unmultiplied(255, 255, 255, 30),
                );
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
            ui.painter().text(
                close_rect.center(),
                egui::Align2::CENTER_CENTER,
                egui_phosphor::regular::X,
                egui::FontId::proportional(14.0),
                theme.text.secondary.to_color32(),
            );
            if close_resp.clicked() {
                close_requested = true;
            }

            // Content area inside the animated background.
            let content_rect = popup_rect.shrink(28.0);
            let mut content_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(content_rect)
                    .layout(egui::Layout::top_down(egui::Align::Center)),
            );

            content_ui.add_space(6.0);

            // Centered Renzora icon (large).
            if let Some(tex) = renzora_icon_texture(content_ui.ctx()) {
                let img_size = Vec2::splat(78.0);
                let (rect, _) = content_ui.allocate_exact_size(img_size, Sense::hover());
                egui::Image::new(&tex)
                    .fit_to_exact_size(img_size)
                    .paint_at(&content_ui, rect);
            }

            content_ui.add_space(8.0);
            content_ui.label(
                egui::RichText::new("Renzora Engine")
                    .size(22.0)
                    .strong()
                    .color(Color32::WHITE),
            );
            content_ui.add_space(2.0);
            content_ui.label(
                egui::RichText::new(format!("Version {}", ENGINE_VERSION))
                    .size(11.0)
                    .color(theme.semantic.accent.to_color32()),
            );

            content_ui.add_space(20.0);

            // Link buttons in a horizontal row, evenly spaced.
            let links: &[(&str, &str, &str)] = &[
                (
                    egui_phosphor::regular::GLOBE,
                    "Website",
                    "https://renzora.com",
                ),
                (
                    egui_phosphor::regular::YOUTUBE_LOGO,
                    "YouTube",
                    "https://youtube.com/@renzoragame",
                ),
                (
                    egui_phosphor::regular::DISCORD_LOGO,
                    "Discord",
                    "https://discord.gg/9UHUGUyDJv",
                ),
                (
                    egui_phosphor::regular::GITHUB_LOGO,
                    "GitHub",
                    "https://github.com/renzora/engine",
                ),
            ];

            content_ui.horizontal(|ui| {
                let total_w = ui.available_width();
                let n = links.len();
                let gap = 10.0;
                let btn_w = ((total_w - gap * (n as f32 - 1.0)) / n as f32).floor();
                for (i, (icon, label, url)) in links.iter().enumerate() {
                    if i > 0 {
                        ui.add_space(gap);
                    }
                    // Stacked icon-on-top, label-below button.
                    let btn_size = Vec2::new(btn_w, 64.0);
                    let (rect, resp) = ui.allocate_exact_size(btn_size, Sense::click());
                    let hovered = resp.hovered();
                    if hovered {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    let bg = if hovered {
                        Color32::from_rgba_unmultiplied(255, 255, 255, 30)
                    } else {
                        Color32::from_rgba_unmultiplied(255, 255, 255, 12)
                    };
                    ui.painter()
                        .rect_filled(rect, egui::CornerRadius::same(6), bg);
                    let icon_color = if hovered {
                        Color32::WHITE
                    } else {
                        theme.text.primary.to_color32()
                    };
                    ui.painter().text(
                        Pos2::new(rect.center().x, rect.min.y + 22.0),
                        egui::Align2::CENTER_CENTER,
                        icon,
                        egui::FontId::proportional(22.0),
                        icon_color,
                    );
                    ui.painter().text(
                        Pos2::new(rect.center().x, rect.max.y - 14.0),
                        egui::Align2::CENTER_CENTER,
                        *label,
                        egui::FontId::proportional(11.0),
                        if hovered {
                            theme.text.primary.to_color32()
                        } else {
                            theme.text.secondary.to_color32()
                        },
                    );
                    if resp.clicked() {
                        open_url(url);
                    }
                }
            });

            // Keep redrawing so the grid animation runs smoothly.
            ui.ctx().request_repaint();
        });

    close_requested
}

/// Render a menu item with a leading phosphor icon. Sets the pointing-hand
/// cursor on hover. Returns `true` on click.
fn menu_item(ui: &mut egui::Ui, icon: &str, label: &str) -> bool {
    let resp = ui.button(format!("{}  {}", icon, label));
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// Like `menu_item` but takes an `enabled` flag — disabled items are dimmed
/// and unclickable. Cursor only changes when the item is enabled.
fn menu_item_enabled(ui: &mut egui::Ui, icon: &str, label: &str, enabled: bool) -> bool {
    let resp = ui.add_enabled(enabled, egui::Button::new(format!("{}  {}", icon, label)));
    if enabled && resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// Open a URL in the user's default browser. No-op on wasm32.
fn open_url(url: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn();
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(url).spawn();
        #[cfg(all(unix, not(target_os = "macos")))]
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

/// Add a fixed brightness delta to each RGB channel of a color.
fn brighten(c: Color32, delta: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(
        c.r().saturating_add(delta),
        c.g().saturating_add(delta),
        c.b().saturating_add(delta),
        c.a(),
    )
}
