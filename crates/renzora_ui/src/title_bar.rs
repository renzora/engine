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
}

const TITLE_BAR_HEIGHT: f32 = 32.0;
const WINDOW_CTRL_WIDTH: f32 = 120.0; // 3 buttons × 40px
const TAB_PADDING: f32 = 16.0;
const TAB_FONT_SIZE: f32 = 11.5;
const TAB_CORNER_RADIUS: f32 = 3.0;
const UNDERLINE_HEIGHT: f32 = 2.0;
const UNDERLINE_INSET: f32 = 3.0;

/// Play mode state passed into the title bar for rendering play/stop controls.
pub struct PlayModeInfo {
    pub is_playing: bool,
    pub is_paused: bool,
    pub is_scripts_only: bool,
}

impl Default for PlayModeInfo {
    fn default() -> Self {
        Self { is_playing: false, is_paused: false, is_scripts_only: false }
    }
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
                    if ui.button("New Project").clicked() {
                        action = TitleBarAction::NewProject;
                        ui.close();
                    }
                    if ui.button("Open Project...").clicked() {
                        action = TitleBarAction::OpenProject;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("New Scene").clicked() {
                        action = TitleBarAction::NewScene;
                        ui.close();
                    }
                    if ui.button("Open Scene...").clicked() {
                        action = TitleBarAction::OpenScene;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Save").clicked() {
                        action = TitleBarAction::Save;
                        ui.close();
                    }
                    if ui.button("Save As...").clicked() {
                        action = TitleBarAction::SaveAs;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Export Project...").clicked() {
                        action = TitleBarAction::Export;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Settings").clicked() {
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
                    if ui.add_enabled(can_undo, egui::Button::new("Undo")).clicked() {
                        action = TitleBarAction::Undo;
                        ui.close();
                    }
                    if ui.add_enabled(can_redo, egui::Button::new("Redo")).clicked() {
                        action = TitleBarAction::Redo;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Reset Layout").clicked() {
                        action = TitleBarAction::ResetLayout;
                        ui.close();
                    }
                });
                if edit_menu.response.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    any_widget_hovered = true;
                }
                switch_top_menu_on_hover(ui.ctx(), &edit_menu.response);

                let help_menu = ui.menu_button("Help", |ui| {
                    if ui.button("Getting Started Tutorial").clicked() {
                        action = TitleBarAction::StartTutorial;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("About Renzora").clicked() {
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
                let total_tabs_width: f32 = tab_widths.iter().sum::<f32>()
                    + tab_spacing * (tab_widths.len().saturating_sub(1)) as f32;

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

                    let tab_rect = Rect::from_min_size(
                        Pos2::new(ui.cursor().left(), tab_y),
                        Vec2::new(tw, tab_h),
                    );

                    let tab_id = ui.id().with(("layout_tab", i));
                    let response = ui.interact(tab_rect, tab_id, Sense::click());

                    // Background
                    let bg = if is_active {
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

                    // Text
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

                    // Active underline
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
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        any_widget_hovered = true;
                    }

                    if response.clicked() {
                        action = TitleBarAction::SwitchLayout(i);
                    }

                    // Advance cursor past this tab + spacing
                    ui.add_space(tw + tab_spacing);
                }

                // --- Right: play controls + sign-in + settings gear ---
                let btn_size = 20.0;
                let gear_size = 20.0;
                let sign_in_width = 80.0;
                // Reserve room on the right for the window min/max/close buttons.
                let right_margin = WINDOW_CTRL_WIDTH + 8.0;
                let in_any_play = play_mode.is_playing || play_mode.is_paused || play_mode.is_scripts_only;
                // Play & Scripts entry points live in the viewport. Only pause+stop
                // show here, and only while a play mode is active.
                let play_controls_width = if in_any_play {
                    btn_size * 2.0 + 4.0 // pause + stop
                } else {
                    0.0
                };
                let remaining = ui.available_width() - play_controls_width - 8.0 - sign_in_width - 8.0 - gear_size - right_margin;
                if remaining > 0.0 {
                    ui.add_space(remaining);
                }

                // Play/Stop/Pause buttons
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
                            if ui
                                .button(format!(
                                    "{} My Library",
                                    egui_phosphor::regular::BOOKS
                                ))
                                .clicked()
                            {
                                action = TitleBarAction::OpenUserLibrary;
                                ui.close();
                            }
                            if ui
                                .button(format!(
                                    "{} Settings",
                                    egui_phosphor::regular::GEAR
                                ))
                                .clicked()
                            {
                                action = TitleBarAction::OpenUserSettings;
                                ui.close();
                            }
                            ui.separator();
                            if ui
                                .button(format!(
                                    "{} Sign Out",
                                    egui_phosphor::regular::SIGN_OUT
                                ))
                                .clicked()
                            {
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
                    let sign_in_resp =
                        ui.interact(sign_in_rect, sign_in_id, Sense::click());

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
                    ui.painter().rect_filled(
                        sign_in_rect,
                        egui::CornerRadius::same(3),
                        bg,
                    );

                    ui.painter().text(
                        Pos2::new(
                            sign_in_rect.left() + 14.0,
                            sign_in_rect.center().y,
                        ),
                        egui::Align2::CENTER_CENTER,
                        egui_phosphor::regular::USER,
                        egui::FontId::proportional(12.0),
                        theme.text.secondary.to_color32(),
                    );

                    ui.painter().text(
                        Pos2::new(
                            sign_in_rect.left() + 28.0,
                            sign_in_rect.center().y,
                        ),
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

/// Add a fixed brightness delta to each RGB channel of a color.
fn brighten(c: Color32, delta: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(
        c.r().saturating_add(delta),
        c.g().saturating_add(delta),
        c.b().saturating_add(delta),
        c.a(),
    )
}
