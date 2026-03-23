//! Top title/menu bar with centered layout tabs.

use bevy_egui::egui::{self, Color32, CursorIcon, Pos2, Rect, Sense, Vec2};
use renzora_theme::Theme;

use crate::layouts::LayoutManager;
use crate::panel::PanelRegistry;

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
}

const TITLE_BAR_HEIGHT: f32 = 28.0;
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
) -> TitleBarAction {
    let mut action = TitleBarAction::None;

    egui::TopBottomPanel::top("renzora_title_bar")
        .exact_height(TITLE_BAR_HEIGHT)
        .show(ctx, |ui| {
            let panel_rect = ui.available_rect_before_wrap();

            // Reduce vertical padding so menu items center in the 28px bar
            ui.style_mut().spacing.button_padding = Vec2::new(6.0, 2.0);
            ui.add_space(4.0);

            egui::MenuBar::new().ui(ui, |ui| {
                // --- Left: menus ---
                ui.add_space(4.0);
                ui.menu_button("File", |ui| {
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

                ui.menu_button("Edit", |ui| {
                    if ui.button("Undo").clicked() {
                        ui.close();
                    }
                    if ui.button("Redo").clicked() {
                        ui.close();
                    }
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("Getting Started Tutorial").clicked() {
                        action = TitleBarAction::StartTutorial;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("About Renzora").clicked() {
                        ui.close();
                    }
                });

                // --- Center: layout tabs ---
                let font = egui::FontId::proportional(TAB_FONT_SIZE);

                // Measure total tabs width for centering
                let tab_widths: Vec<f32> = layout_manager
                    .layouts
                    .iter()
                    .map(|l| {
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

                for (i, layout) in layout_manager.layouts.iter().enumerate() {
                    let is_active = i == layout_manager.active_index;
                    let tw = tab_widths[i];

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
                let right_margin = 8.0;
                let in_any_play = play_mode.is_playing || play_mode.is_paused || play_mode.is_scripts_only;
                let play_controls_width = if in_any_play {
                    btn_size * 2.0 + 4.0 // pause + stop
                } else {
                    btn_size * 2.0 + 4.0 // play + scripts
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
                    }
                    if stop_resp.clicked() {
                        action = TitleBarAction::Stop;
                    }
                    ui.add_space(btn_size);
                } else {
                    // Play button
                    let play_rect = Rect::from_min_size(
                        Pos2::new(ui.cursor().left(), tab_y + (tab_h - btn_size) / 2.0),
                        Vec2::splat(btn_size),
                    );
                    let play_id = ui.id().with("play_btn");
                    let play_resp = ui.interact(play_rect, play_id, Sense::click());
                    let play_color = if play_resp.hovered() {
                        Color32::from_rgb(100, 255, 100)
                    } else {
                        theme.text.muted.to_color32()
                    };
                    ui.painter().text(
                        play_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        egui_phosphor::regular::PLAY,
                        egui::FontId::proportional(14.0),
                        play_color,
                    );
                    if play_resp.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    if play_resp.clicked() {
                        action = TitleBarAction::Play;
                    }
                    ui.add_space(btn_size + 4.0);

                    // Scripts button (run scripts in editor)
                    let scripts_rect = Rect::from_min_size(
                        Pos2::new(ui.cursor().left(), tab_y + (tab_h - btn_size) / 2.0),
                        Vec2::splat(btn_size),
                    );
                    let scripts_id = ui.id().with("scripts_btn");
                    let scripts_resp = ui.interact(scripts_rect, scripts_id, Sense::click());
                    let scripts_color = if scripts_resp.hovered() {
                        Color32::from_rgb(100, 180, 255)
                    } else {
                        theme.text.muted.to_color32()
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
                    }
                    if scripts_resp.clicked() {
                        action = TitleBarAction::ScriptsOnly;
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
                }
                if gear_resp.clicked() {
                    action = TitleBarAction::ToggleSettings;
                }

                ui.add_space(gear_size + 8.0);

                // User button — sign-in or dropdown menu when signed in
                if let Some(username) = signed_in_username {
                    // Signed in — show username button with dropdown
                    let _user_btn_resp = ui.menu_button(
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
        });

    action
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
