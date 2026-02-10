use bevy::prelude::*;
use bevy::window::{WindowMode, WindowPosition};
use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, Id, Pos2, Sense, Stroke, Vec2};

use crate::commands::{CommandHistory, DeleteEntityCommand, queue_command};
use crate::core::{AssetBrowserState, DockingState, ExportState, SelectionState, ViewportState, WindowState, SceneManagerState, EditorSettings, ResizeEdge, PlayModeState, PlayState};
use crate::gizmo::{GizmoState, EditorTool};
use crate::plugin_core::{MenuLocation, MenuItem, PluginHost};
use crate::theming::Theme;
use crate::ui::docking::{builtin_layouts, PanelId};
use crate::ui_api::UiEvent;

use egui_phosphor::regular::{MINUS, SQUARE, X, SQUARES_FOUR, USER, PLAY, PAUSE, STOP, GEAR, CARET_DOWN};

/// Height of the custom title bar
pub const TITLE_BAR_HEIGHT: f32 = 28.0;

pub fn render_title_bar(
    ctx: &egui::Context,
    window_state: &mut WindowState,
    selection: &mut SelectionState,
    scene_state: &mut SceneManagerState,
    settings: &mut EditorSettings,
    export_state: &mut ExportState,
    assets: &mut AssetBrowserState,
    plugin_host: &PluginHost,
    command_history: &mut CommandHistory,
    docking_state: &mut DockingState,
    viewport_state: &mut ViewportState,
    gizmo: &mut GizmoState,
    play_mode: &mut PlayModeState,
    theme: &Theme,
) -> Vec<UiEvent> {
    let mut ui_events = Vec::new();
    let is_maximized = window_state.is_maximized;

    egui::TopBottomPanel::top("title_bar")
        .exact_height(TITLE_BAR_HEIGHT)
        .frame(egui::Frame::NONE.fill(theme.surfaces.window.to_color32()))
        .show(ctx, |ui| {
            let panel_rect = ui.max_rect();
            let painter = ui.painter();

            // Window button dimensions
            let button_width = 40.0;
            let window_buttons_width = button_width * 3.0;

            // Draw bottom border
            painter.line_segment(
                [
                    Pos2::new(panel_rect.left(), panel_rect.bottom() - 1.0),
                    Pos2::new(panel_rect.right(), panel_rect.bottom() - 1.0),
                ],
                Stroke::new(1.0, theme.surfaces.extreme.to_color32()),
            );

            // Drag area (everything except window buttons) - interact with it FIRST
            // This puts the drag area at the bottom of the interaction stack
            let drag_rect = egui::Rect::from_min_max(
                panel_rect.min,
                Pos2::new(panel_rect.max.x - window_buttons_width, panel_rect.max.y),
            );

            // Create an interactive drag area that sits behind other elements
            let drag_response = ui.interact(drag_rect, Id::new("title_bar_drag"), Sense::click_and_drag());

            // Handle double-click to maximize
            if drag_response.double_clicked() {
                window_state.request_toggle_maximize = true;
            }

            // Handle drag - set flag when drag starts
            if drag_response.drag_started() {
                window_state.start_drag = true;
            }

            // Stop manual drag when mouse released
            if drag_response.drag_stopped() || !drag_response.dragged() && !drag_response.drag_started() {
                if window_state.is_being_dragged {
                    window_state.is_being_dragged = false;
                    window_state.drag_offset = None;
                }
            }

            // Layout menus on the left (offset down slightly for visual balance)
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.add_space(8.0);

                // Menu bar items
                ui_events = render_menu_items(ui, selection, scene_state, settings, export_state, assets, plugin_host, command_history, docking_state, viewport_state, theme);

                // Center layout tabs in the title bar
                let tabs_width_id = ui.id().with("layout_tabs_width");
                let last_tabs_width: f32 = ui.ctx().data_mut(|d| d.get_temp(tabs_width_id).unwrap_or(0.0));
                let panel_center_x = panel_rect.center().x;
                let cursor_x = ui.cursor().left();
                let desired_start = panel_center_x - last_tabs_width / 2.0;
                let leading = (desired_start - cursor_x).max(12.0);
                ui.add_space(leading);

                let tabs_start_x = ui.cursor().left();
                render_layout_tabs(ui, docking_state, gizmo, theme);
                let tabs_end_x = ui.cursor().left();
                ui.ctx().data_mut(|d| d.insert_temp(tabs_width_id, tabs_end_x - tabs_start_x));

                // Right-aligned section: Play controls, Settings, Sign In, Window buttons
                let play_section_width = 34.0 + 24.0 + 24.0;
                let settings_btn_width = 24.0;
                let sign_in_width = 80.0;
                let right_total = play_section_width + 12.0 + settings_btn_width + 8.0 + sign_in_width + 8.0 + window_buttons_width;
                ui.add_space(ui.available_width() - right_total);

                // === Play Controls ===
                let play_color = theme.semantic.success.to_color32();
                let is_playing = play_mode.state == PlayState::Playing;
                let is_paused = play_mode.state == PlayState::Paused;
                let is_scripts_only = play_mode.is_scripts_only();
                let is_scripts_paused = play_mode.state == PlayState::ScriptsPaused;
                let is_in_play_mode = play_mode.is_in_play_mode();
                let scripts_color = theme.semantic.accent.to_color32();
                let current_play_color = if is_scripts_only { scripts_color } else { play_color };
                let play_active = is_playing || is_scripts_only;

                title_play_dropdown(ui, play_active, current_play_color, play_mode, theme);

                let any_paused = is_paused || is_scripts_paused;
                let pause_resp = title_icon_button(ui, PAUSE, any_paused, theme.semantic.accent.to_color32(), theme);
                if pause_resp.clicked() {
                    if is_playing {
                        play_mode.state = PlayState::Paused;
                    } else if play_mode.state == PlayState::ScriptsOnly {
                        play_mode.state = PlayState::ScriptsPaused;
                    }
                }
                pause_resp.on_hover_text("Pause (F6)");

                let stop_color = if is_in_play_mode { theme.semantic.error.to_color32() } else { theme.text.disabled.to_color32() };
                let stop_resp = title_icon_button(ui, STOP, false, stop_color, theme);
                if stop_resp.clicked() && is_in_play_mode {
                    play_mode.request_stop = true;
                }
                stop_resp.on_hover_text("Stop (Escape)");

                ui.add_space(12.0);

                // === Settings ===
                let settings_panel = PanelId::Settings;
                let settings_visible = docking_state.is_panel_visible(&settings_panel);
                let settings_resp = title_icon_button(ui, GEAR, settings_visible, theme.semantic.accent.to_color32(), theme);
                if settings_resp.clicked() {
                    if settings_visible {
                        docking_state.close_panel(&settings_panel);
                    } else {
                        docking_state.open_panel(settings_panel);
                    }
                }
                settings_resp.on_hover_text("Settings");

                ui.add_space(8.0);

                // === Sign In ===

                let auth_open_id = Id::new("auth_window_open");
                let auth_open: bool = ui.ctx().data_mut(|d| d.get_temp(auth_open_id).unwrap_or(false));

                let sign_in_size = Vec2::new(sign_in_width, 20.0);
                let (sign_in_rect, sign_in_resp) = ui.allocate_exact_size(sign_in_size, Sense::click());
                if sign_in_resp.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                if ui.is_rect_visible(sign_in_rect) {
                    let bg = if auth_open {
                        theme.widgets.active_bg.to_color32()
                    } else if sign_in_resp.hovered() {
                        theme.widgets.hovered_bg.to_color32()
                    } else {
                        Color32::TRANSPARENT
                    };
                    ui.painter().rect_filled(sign_in_rect, CornerRadius::same(3), bg);

                    // Person icon
                    ui.painter().text(
                        Pos2::new(sign_in_rect.left() + 14.0, sign_in_rect.center().y),
                        egui::Align2::CENTER_CENTER,
                        USER,
                        egui::FontId::proportional(12.0),
                        theme.text.secondary.to_color32(),
                    );

                    // "Sign In" text
                    ui.painter().text(
                        Pos2::new(sign_in_rect.left() + 28.0, sign_in_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        "Sign In",
                        egui::FontId::proportional(11.0),
                        theme.text.secondary.to_color32(),
                    );
                }
                if sign_in_resp.clicked() {
                    ui.ctx().data_mut(|d| d.insert_temp(auth_open_id, !auth_open));
                }

                ui.add_space(8.0);

                // Window buttons on the right
                ui.spacing_mut().item_spacing.x = 0.0;

                // Minimize button
                let min_resp = window_button(ui, MINUS, theme.widgets.hovered_bg.to_color32(), button_width);
                if min_resp.clicked() {
                    window_state.request_minimize = true;
                }

                // Maximize/Restore button
                let max_icon = if is_maximized { SQUARES_FOUR } else { SQUARE };
                let max_resp = window_button(ui, max_icon, theme.widgets.hovered_bg.to_color32(), button_width);
                if max_resp.clicked() {
                    window_state.request_toggle_maximize = true;
                }

                // Close button (red on hover)
                let close_resp = window_button(ui, X, theme.panels.close_hover.to_color32(), button_width);
                if close_resp.clicked() {
                    window_state.request_close = true;
                }
            });
        });

    // Render auth window (outside the title bar panel)
    render_auth_window(ctx, theme);

    ui_events
}

/// System to sync window state and apply pending window actions
pub fn handle_window_actions(
    mut window_state: ResMut<WindowState>,
    mut windows: Query<&mut Window>,
) {
    let Ok(mut window) = windows.single_mut() else { return };

    // Sync window state
    window_state.is_maximized = window.mode == WindowMode::BorderlessFullscreen(MonitorSelection::Current);

    // Apply pending actions
    if window_state.request_close {
        std::process::exit(0);
    }

    if window_state.request_minimize {
        window.set_minimized(true);
        window_state.request_minimize = false;
    }

    if window_state.request_toggle_maximize {
        if window.mode == WindowMode::BorderlessFullscreen(MonitorSelection::Current) {
            window.mode = WindowMode::Windowed;
        } else {
            window.mode = WindowMode::BorderlessFullscreen(MonitorSelection::Current);
        }
        window_state.request_toggle_maximize = false;
    }

    // Handle window drag start - use manual drag (native winit drag has timing issues)
    if window_state.start_drag && window.mode == WindowMode::Windowed {
        // Get current cursor position relative to window
        if let Some(cursor_pos) = window.cursor_position() {
            // Store the offset from window corner to cursor
            window_state.drag_offset = Some((cursor_pos.x, cursor_pos.y));
            window_state.is_being_dragged = true;
        }

        window_state.start_drag = false;
    }

    // Manual window dragging fallback
    if window_state.is_being_dragged && window.mode == WindowMode::Windowed {
        if let Some(screen_cursor) = get_screen_cursor_pos(&window) {
            if let Some((offset_x, offset_y)) = window_state.drag_offset {
                let new_x = screen_cursor.0 as f32 - offset_x;
                let new_y = screen_cursor.1 as f32 - offset_y;

                window.position = WindowPosition::At(IVec2::new(new_x as i32, new_y as i32));
            }
        }
    }

    // Handle window resizing
    if window_state.is_resizing && window.mode == WindowMode::Windowed {
        if let Some(screen_cursor) = get_screen_cursor_pos(&window) {
            // Initialize resize start state if not set
            if window_state.resize_start_rect.is_none() {
                let pos = match window.position {
                    WindowPosition::At(p) => p,
                    _ => IVec2::ZERO,
                };
                window_state.resize_start_rect = Some((
                    pos.x,
                    pos.y,
                    window.resolution.width() as u32,
                    window.resolution.height() as u32,
                ));
                window_state.resize_start_cursor = Some(screen_cursor);
            }

            if let (Some((start_x, start_y, start_w, start_h)), Some((cursor_start_x, cursor_start_y))) =
                (window_state.resize_start_rect, window_state.resize_start_cursor)
            {
                let dx = screen_cursor.0 - cursor_start_x;
                let dy = screen_cursor.1 - cursor_start_y;

                let min_w = 800i32;
                let min_h = 600i32;

                let (new_x, new_y, new_w, new_h) = match window_state.resize_edge {
                    ResizeEdge::Right => {
                        let new_w = (start_w as i32 + dx).max(min_w) as u32;
                        (start_x, start_y, new_w, start_h)
                    }
                    ResizeEdge::Bottom => {
                        let new_h = (start_h as i32 + dy).max(min_h) as u32;
                        (start_x, start_y, start_w, new_h)
                    }
                    ResizeEdge::Left => {
                        let new_w = (start_w as i32 - dx).max(min_w) as u32;
                        let new_x = start_x + (start_w as i32 - new_w as i32);
                        (new_x, start_y, new_w, start_h)
                    }
                    ResizeEdge::Top => {
                        let new_h = (start_h as i32 - dy).max(min_h) as u32;
                        let new_y = start_y + (start_h as i32 - new_h as i32);
                        (start_x, new_y, start_w, new_h)
                    }
                    ResizeEdge::BottomRight => {
                        let new_w = (start_w as i32 + dx).max(min_w) as u32;
                        let new_h = (start_h as i32 + dy).max(min_h) as u32;
                        (start_x, start_y, new_w, new_h)
                    }
                    ResizeEdge::BottomLeft => {
                        let new_w = (start_w as i32 - dx).max(min_w) as u32;
                        let new_h = (start_h as i32 + dy).max(min_h) as u32;
                        let new_x = start_x + (start_w as i32 - new_w as i32);
                        (new_x, start_y, new_w, new_h)
                    }
                    ResizeEdge::TopRight => {
                        let new_w = (start_w as i32 + dx).max(min_w) as u32;
                        let new_h = (start_h as i32 - dy).max(min_h) as u32;
                        let new_y = start_y + (start_h as i32 - new_h as i32);
                        (start_x, new_y, new_w, new_h)
                    }
                    ResizeEdge::TopLeft => {
                        let new_w = (start_w as i32 - dx).max(min_w) as u32;
                        let new_h = (start_h as i32 - dy).max(min_h) as u32;
                        let new_x = start_x + (start_w as i32 - new_w as i32);
                        let new_y = start_y + (start_h as i32 - new_h as i32);
                        (new_x, new_y, new_w, new_h)
                    }
                    ResizeEdge::None => (start_x, start_y, start_w, start_h),
                };

                window.position = WindowPosition::At(IVec2::new(new_x, new_y));
                window.resolution.set(new_w as f32, new_h as f32);
            }
        }
    }
}

/// Get the screen-space cursor position as `(x, y)` in pixels.
/// On Windows this calls `GetCursorPos` (absolute screen coords).
/// On other platforms it approximates from Bevy's window-relative cursor + window position.
fn get_screen_cursor_pos(window: &Window) -> Option<(i32, i32)> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
        use windows_sys::Win32::Foundation::POINT;

        let mut point = POINT { x: 0, y: 0 };
        unsafe {
            if GetCursorPos(&mut point) != 0 {
                return Some((point.x, point.y));
            }
        }
        None
    }

    #[cfg(not(target_os = "windows"))]
    {
        let cursor = window.cursor_position()?;
        let win_pos = match window.position {
            WindowPosition::At(p) => p,
            _ => IVec2::ZERO,
        };
        Some((win_pos.x + cursor.x as i32, win_pos.y + cursor.y as i32))
    }
}

fn render_layout_tabs(
    ui: &mut egui::Ui,
    docking_state: &mut DockingState,
    gizmo: &mut GizmoState,
    theme: &Theme,
) {
    let layouts = builtin_layouts();
    let active_layout = docking_state.active_layout.clone();

    ui.spacing_mut().item_spacing.x = 2.0;

    for layout in &layouts {
        let is_active = active_layout == layout.name;
        let tab_id = ui.make_persistent_id(format!("layout_tab_{}", layout.name));

        let text = &layout.name;
        let font = egui::FontId::proportional(11.5);
        let text_galley = ui.painter().layout_no_wrap(
            text.to_string(),
            font.clone(),
            Color32::WHITE, // color doesn't matter for measuring
        );
        let text_width = text_galley.size().x;
        let tab_width = text_width + 16.0; // padding
        let tab_height = ui.available_height();

        let size = Vec2::new(tab_width, tab_height);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());

        if response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }

        if ui.is_rect_visible(rect) {
            let accent = theme.semantic.accent.to_color32();

            // Background
            let bg = if is_active {
                // Active tab: subtle highlight
                let [r, g, b, _] = theme.surfaces.window.to_color32().to_array();
                Color32::from_rgb(r.saturating_add(18), g.saturating_add(18), b.saturating_add(22))
            } else if response.hovered() {
                let [r, g, b, _] = theme.surfaces.window.to_color32().to_array();
                Color32::from_rgb(r.saturating_add(10), g.saturating_add(10), b.saturating_add(14))
            } else {
                Color32::TRANSPARENT
            };

            ui.painter().rect_filled(rect, CornerRadius::same(3), bg);

            // Active underline
            if is_active {
                let underline_rect = egui::Rect::from_min_size(
                    Pos2::new(rect.left() + 3.0, rect.bottom() - 2.0),
                    Vec2::new(rect.width() - 6.0, 2.0),
                );
                ui.painter().rect_filled(underline_rect, CornerRadius::same(1), accent);
            }

            // Text
            let text_color = if is_active {
                Color32::WHITE
            } else if response.hovered() {
                theme.text.secondary.to_color32()
            } else {
                theme.text.muted.to_color32()
            };

            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                font,
                text_color,
            );
        }

        if response.clicked() {
            docking_state.switch_layout(&layout.name);
            // Switch tool based on layout
            if layout.name == "Terrain" {
                gizmo.tool = EditorTool::TerrainSculpt;
            } else if gizmo.tool == EditorTool::TerrainSculpt {
                gizmo.tool = EditorTool::Select;
            }
        }

        let _ = tab_id; // suppress unused warning
    }
}

#[derive(Clone, Copy, PartialEq, Default)]
enum AuthView {
    #[default]
    SignIn,
    Register,
    ForgotPassword,
}

fn render_auth_window(ctx: &egui::Context, theme: &Theme) {
    let open_id = Id::new("auth_window_open");
    let view_id = Id::new("auth_view");
    let email_id = Id::new("auth_email");
    let password_id = Id::new("auth_password");
    let confirm_pw_id = Id::new("auth_confirm_pw");
    let username_id = Id::new("auth_username");

    let mut open: bool = ctx.data_mut(|d| d.get_temp(open_id).unwrap_or(false));
    if !open {
        return;
    }

    let mut view: AuthView = ctx.data_mut(|d| d.get_temp(view_id).unwrap_or_default());

    let mut email: String = ctx.data_mut(|d| d.get_temp::<String>(email_id).unwrap_or_default());
    let mut password: String = ctx.data_mut(|d| d.get_temp::<String>(password_id).unwrap_or_default());
    let mut confirm_pw: String = ctx.data_mut(|d| d.get_temp::<String>(confirm_pw_id).unwrap_or_default());
    let mut username: String = ctx.data_mut(|d| d.get_temp::<String>(username_id).unwrap_or_default());

    let title = match view {
        AuthView::SignIn => "Sign In",
        AuthView::Register => "Create Account",
        AuthView::ForgotPassword => "Reset Password",
    };

    let accent = theme.semantic.accent.to_color32();
    let text_secondary = theme.text.secondary.to_color32();

    egui::Window::new(title)
        .id(Id::new("auth_window"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([320.0, 0.0])
        .frame(egui::Frame::window(&ctx.style())
            .fill(theme.surfaces.panel.to_color32())
            .stroke(Stroke::new(1.0, theme.widgets.border.to_color32()))
            .corner_radius(CornerRadius::same(8)))
        .show(ctx, |ui| {
            ui.add_space(8.0);

            match view {
                AuthView::SignIn => {
                    // Email
                    ui.label(egui::RichText::new("Email").size(11.0).color(text_secondary));
                    ui.add_space(2.0);
                    ui.add(egui::TextEdit::singleline(&mut email)
                        .desired_width(f32::INFINITY)
                        .hint_text("you@example.com"));
                    ui.add_space(8.0);

                    // Password
                    ui.label(egui::RichText::new("Password").size(11.0).color(text_secondary));
                    ui.add_space(2.0);
                    ui.add(egui::TextEdit::singleline(&mut password)
                        .desired_width(f32::INFINITY)
                        .password(true)
                        .hint_text("Password"));
                    ui.add_space(4.0);

                    // Forgot password link
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        let forgot = ui.add(egui::Label::new(
                            egui::RichText::new("Forgot password?").size(11.0).color(accent)
                        ).sense(Sense::click()));
                        if forgot.hovered() {
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        }
                        if forgot.clicked() {
                            view = AuthView::ForgotPassword;
                        }
                    });

                    ui.add_space(12.0);

                    // Sign In button
                    let btn = ui.add_sized(
                        [ui.available_width(), 32.0],
                        egui::Button::new(egui::RichText::new("Sign In").color(Color32::WHITE).size(13.0))
                            .fill(accent)
                            .corner_radius(CornerRadius::same(4)),
                    );
                    if btn.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }

                    ui.add_space(12.0);

                    // Register link
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Don't have an account?").size(11.0).color(text_secondary));
                        let reg = ui.add(egui::Label::new(
                            egui::RichText::new("Register").size(11.0).color(accent)
                        ).sense(Sense::click()));
                        if reg.hovered() {
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        }
                        if reg.clicked() {
                            view = AuthView::Register;
                        }
                    });
                }

                AuthView::Register => {
                    // Username
                    ui.label(egui::RichText::new("Username").size(11.0).color(text_secondary));
                    ui.add_space(2.0);
                    ui.add(egui::TextEdit::singleline(&mut username)
                        .desired_width(f32::INFINITY)
                        .hint_text("Username"));
                    ui.add_space(8.0);

                    // Email
                    ui.label(egui::RichText::new("Email").size(11.0).color(text_secondary));
                    ui.add_space(2.0);
                    ui.add(egui::TextEdit::singleline(&mut email)
                        .desired_width(f32::INFINITY)
                        .hint_text("you@example.com"));
                    ui.add_space(8.0);

                    // Password
                    ui.label(egui::RichText::new("Password").size(11.0).color(text_secondary));
                    ui.add_space(2.0);
                    ui.add(egui::TextEdit::singleline(&mut password)
                        .desired_width(f32::INFINITY)
                        .password(true)
                        .hint_text("Password"));
                    ui.add_space(8.0);

                    // Confirm password
                    ui.label(egui::RichText::new("Confirm Password").size(11.0).color(text_secondary));
                    ui.add_space(2.0);
                    ui.add(egui::TextEdit::singleline(&mut confirm_pw)
                        .desired_width(f32::INFINITY)
                        .password(true)
                        .hint_text("Confirm password"));

                    ui.add_space(16.0);

                    // Create Account button
                    let btn = ui.add_sized(
                        [ui.available_width(), 32.0],
                        egui::Button::new(egui::RichText::new("Create Account").color(Color32::WHITE).size(13.0))
                            .fill(accent)
                            .corner_radius(CornerRadius::same(4)),
                    );
                    if btn.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }

                    ui.add_space(12.0);

                    // Back to sign in
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Already have an account?").size(11.0).color(text_secondary));
                        let back = ui.add(egui::Label::new(
                            egui::RichText::new("Sign In").size(11.0).color(accent)
                        ).sense(Sense::click()));
                        if back.hovered() {
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        }
                        if back.clicked() {
                            view = AuthView::SignIn;
                        }
                    });
                }

                AuthView::ForgotPassword => {
                    ui.label(egui::RichText::new("Enter your email and we'll send you a link to reset your password.").size(11.0).color(text_secondary).weak());
                    ui.add_space(12.0);

                    // Email
                    ui.label(egui::RichText::new("Email").size(11.0).color(text_secondary));
                    ui.add_space(2.0);
                    ui.add(egui::TextEdit::singleline(&mut email)
                        .desired_width(f32::INFINITY)
                        .hint_text("you@example.com"));

                    ui.add_space(16.0);

                    // Send Reset Link button
                    let btn = ui.add_sized(
                        [ui.available_width(), 32.0],
                        egui::Button::new(egui::RichText::new("Send Reset Link").color(Color32::WHITE).size(13.0))
                            .fill(accent)
                            .corner_radius(CornerRadius::same(4)),
                    );
                    if btn.hovered() {
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }

                    ui.add_space(12.0);

                    // Back to sign in
                    ui.horizontal(|ui| {
                        let back = ui.add(egui::Label::new(
                            egui::RichText::new("Back to Sign In").size(11.0).color(accent)
                        ).sense(Sense::click()));
                        if back.hovered() {
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        }
                        if back.clicked() {
                            view = AuthView::SignIn;
                        }
                    });
                }
            }

            ui.add_space(4.0);
        });

    // Persist state
    ctx.data_mut(|d| {
        d.insert_temp(open_id, open);
        d.insert_temp(view_id, view);
        d.insert_temp(email_id, email);
        d.insert_temp(password_id, password);
        d.insert_temp(confirm_pw_id, confirm_pw);
        d.insert_temp(username_id, username);
    });
}

fn render_menu_items(
    ui: &mut egui::Ui,
    selection: &mut SelectionState,
    scene_state: &mut SceneManagerState,
    settings: &mut EditorSettings,
    export_state: &mut ExportState,
    assets: &mut AssetBrowserState,
    plugin_host: &PluginHost,
    command_history: &mut CommandHistory,
    docking_state: &mut DockingState,
    viewport_state: &mut ViewportState,
    theme: &Theme,
) -> Vec<UiEvent> {
    let mut events = Vec::new();
    let api = plugin_host.api();

    // Get plugin menu items grouped by location
    let file_items: Vec<_> = api.menu_items.iter()
        .filter(|(loc, _, _)| *loc == MenuLocation::File)
        .map(|(_, item, _)| item)
        .collect();

    let tools_items: Vec<_> = api.menu_items.iter()
        .filter(|(loc, _, _)| *loc == MenuLocation::Tools)
        .map(|(_, item, _)| item)
        .collect();

    ui.style_mut().visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    ui.style_mut().visuals.widgets.hovered.weak_bg_fill = theme.widgets.hovered_bg.to_color32();
    ui.style_mut().visuals.widgets.active.weak_bg_fill = theme.widgets.active_bg.to_color32();

    submenu(ui, "File", |ui| {
        if menu_item(ui, "New Scene") {
            scene_state.new_scene_requested = true;
            ui.close();
        }
        if menu_item(ui, "Open Scene...") {
            scene_state.open_scene_requested = true;
            ui.close();
        }
        ui.separator();
        if menu_item(ui, "Save Scene        Ctrl+S") {
            scene_state.save_scene_requested = true;
            ui.close();
        }
        if menu_item(ui, "Save Scene As...  Ctrl+Shift+S") {
            scene_state.save_scene_as_requested = true;
            ui.close();
        }

        // Plugin File menu items
        if !file_items.is_empty() {
            ui.separator();
            for item in &file_items {
                if let Some(event) = render_plugin_menu_item(ui, item) {
                    events.push(event);
                }
            }
        }

        ui.separator();

        // Import submenu
        submenu(ui, "Import", |ui| {
            if menu_item(ui, "Import Assets...") {
                assets.import_asset_requested = true;
                ui.close();
            }
            ui.separator();
            if menu_item(ui, "Import 3D Model...") {
                // Open file dialog specifically for 3D models
                if let Some(paths) = rfd::FileDialog::new()
                    .add_filter("3D Models", &["glb", "gltf", "obj", "fbx"])
                    .pick_files()
                {
                    if !paths.is_empty() {
                        assets.pending_import_files = paths;
                        assets.show_import_dialog = true;
                    }
                }
                ui.close();
            }
            if menu_item(ui, "Import Image...") {
                if let Some(paths) = rfd::FileDialog::new()
                    .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "tga", "hdr", "exr"])
                    .pick_files()
                {
                    if let Some(target_folder) = assets.current_folder.clone() {
                        for source_path in paths {
                            if let Some(file_name) = source_path.file_name() {
                                let dest_path = target_folder.join(file_name);
                                let _ = std::fs::copy(&source_path, &dest_path);
                            }
                        }
                    }
                }
                ui.close();
            }
            if menu_item(ui, "Import Audio...") {
                if let Some(paths) = rfd::FileDialog::new()
                    .add_filter("Audio", &["wav", "ogg", "mp3", "flac"])
                    .pick_files()
                {
                    if let Some(target_folder) = assets.current_folder.clone() {
                        for source_path in paths {
                            if let Some(file_name) = source_path.file_name() {
                                let dest_path = target_folder.join(file_name);
                                let _ = std::fs::copy(&source_path, &dest_path);
                            }
                        }
                    }
                }
                ui.close();
            }
        });

        // Export submenu
        submenu(ui, "Export", |ui| {
            if menu_item(ui, "Export Game...") {
                export_state.show_dialog = true;
                ui.close();
            }
        });

        ui.separator();
        if menu_item(ui, "Settings                Ctrl+,") {
            docking_state.open_panel(PanelId::Settings);
            ui.close();
        }
        ui.separator();
        if menu_item(ui, "Exit") {
            std::process::exit(0);
        }
    });

    submenu(ui, "Edit", |ui| {
        // Undo with shortcut hint and disabled state
        let can_undo = command_history.can_undo();
        if menu_item_enabled(ui, "Undo                    Ctrl+Z", can_undo) {
            command_history.pending_undo = 1;
            ui.close();
        }

        // Redo with shortcut hint and disabled state
        let can_redo = command_history.can_redo();
        if menu_item_enabled(ui, "Redo                    Ctrl+Y", can_redo) {
            command_history.pending_redo = 1;
            ui.close();
        }

        ui.separator();
        if menu_item(ui, "Cut") {
            ui.close();
        }
        if menu_item(ui, "Copy") {
            ui.close();
        }
        if menu_item(ui, "Paste") {
            ui.close();
        }
        ui.separator();
        if menu_item(ui, "Duplicate") {
            ui.close();
        }
        if menu_item(ui, "Delete") {
            if let Some(entity) = selection.selected_entity {
                queue_command(command_history, Box::new(DeleteEntityCommand::new(entity)));
            }
            ui.close();
        }
    });

    // Tools menu (for plugins)
    if !tools_items.is_empty() {
        submenu(ui, "Tools", |ui| {
            for item in &tools_items {
                if let Some(event) = render_plugin_menu_item(ui, item) {
                    events.push(event);
                }
            }
        });
    }

    // Helper to apply layout and sync viewport state
    let apply_layout = |name: &str, docking: &mut DockingState, viewport: &mut ViewportState| {
        if docking.switch_layout(name) {
            // Set default viewport state based on layout name
            match name {
                "Default" => {
                    viewport.hierarchy_width = 260.0;
                    viewport.inspector_width = 320.0;
                    viewport.assets_height = 200.0;
                }
                "Scripting" => {
                    viewport.hierarchy_width = 220.0;
                    viewport.inspector_width = 300.0;
                    viewport.assets_height = 180.0;
                }
                "Animation" => {
                    viewport.hierarchy_width = 260.0;
                    viewport.inspector_width = 320.0;
                    viewport.assets_height = 250.0;
                }
                "Debug" => {
                    viewport.hierarchy_width = 300.0;
                    viewport.inspector_width = 280.0;
                    viewport.assets_height = 200.0;
                }
                _ => {}
            }
        }
    };

    // Window menu
    submenu(ui, "Window", |ui| {
        // Save layout
        if menu_item(ui, "Save Layout As...") {
            // TODO: Show save layout dialog
            ui.close();
        }

        if menu_item(ui, "Reset Layout") {
            apply_layout("Default", docking_state, viewport_state);
            ui.close();
        }
    });

    // Dev menu (only visible when dev_mode is enabled)
    if settings.dev_mode {
        submenu(ui, "Dev", |ui| {
            if menu_item(ui, "New Plugin...") {
                // TODO: Show new plugin dialog
                ui.close();
            }
            ui.separator();
            if menu_item(ui, "Open Plugin Source...") {
                // Open file dialog to select a .rs file
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Rust Source", &["rs"])
                    .add_filter("All Files", &["*"])
                    .set_title("Open Plugin Source")
                    .pick_file()
                {
                    crate::ui::panels::open_script(scene_state, path);
                }
                ui.close();
            }
            if menu_item(ui, "Open Cargo.toml...") {
                // Open file dialog to select Cargo.toml
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Cargo.toml", &["toml"])
                    .set_title("Open Plugin Cargo.toml")
                    .pick_file()
                {
                    crate::ui::panels::open_script(scene_state, path);
                }
                ui.close();
            }
            ui.separator();
            ui.label(egui::RichText::new("Shortcuts").color(Color32::GRAY).small());
            ui.label(egui::RichText::new("  Ctrl+B - Build Plugin").color(Color32::GRAY).small());
            ui.label(egui::RichText::new("  Ctrl+S - Save File").color(Color32::GRAY).small());
        });
    }

    submenu(ui, "Help", |ui| {
        if menu_item(ui, "Documentation") {
            ui.close();
        }
        ui.separator();
        if menu_item(ui, "Discord") {
            let _ = open::that("https://discord.gg/9UHUGUyDJv");
            ui.close();
        }
        if menu_item(ui, "YouTube") {
            let _ = open::that("https://youtube.com/@renzoragame");
            ui.close();
        }
        ui.separator();
        if menu_item(ui, "About") {
            ui.close();
        }
    });

    events
}

/// Render a plugin menu item, returns UiEvent if clicked
fn render_plugin_menu_item(ui: &mut egui::Ui, item: &MenuItem) -> Option<UiEvent> {
    if item.children.is_empty() {
        // Leaf item
        let mut text = String::new();
        if let Some(icon) = &item.icon {
            text.push_str(icon);
            text.push(' ');
        }
        text.push_str(&item.label);
        if let Some(shortcut) = &item.shortcut {
            text.push_str("    ");
            text.push_str(shortcut);
        }

        let button = egui::Button::new(&text);
        let response = ui.add_enabled(item.enabled, button);

        if response.hovered() && item.enabled {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }

        if response.clicked() {
            ui.close();
            return Some(UiEvent::ButtonClicked(crate::ui_api::UiId(item.id.0)));
        }
    } else {
        // Submenu
        let label = if let Some(icon) = &item.icon {
            format!("{} {}", icon, item.label)
        } else {
            item.label.clone()
        };

        submenu(ui, &label, |ui| {
            for child in &item.children {
                render_plugin_menu_item(ui, child);
            }
        });
    }

    None
}

/// Helper for menu items - shows pointer cursor on hover and returns if clicked
fn menu_item(ui: &mut egui::Ui, label: &str) -> bool {
    let btn = ui.button(label);
    if btn.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    btn.clicked()
}

/// Helper for menu items with enabled state
fn menu_item_enabled(ui: &mut egui::Ui, label: &str, enabled: bool) -> bool {
    let btn = ui.add_enabled(enabled, egui::Button::new(label));
    if btn.hovered() && enabled {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    btn.clicked()
}


/// Helper for nested submenus - shows pointer cursor on hover
fn submenu(ui: &mut egui::Ui, label: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    let response = ui.menu_button(label, add_contents);
    if response.response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
}

fn window_button(ui: &mut egui::Ui, icon: &str, hover_color: Color32, width: f32) -> egui::Response {
    let size = Vec2::new(width, TITLE_BAR_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    if ui.is_rect_visible(rect) {
        let bg_color = if response.hovered() {
            hover_color
        } else {
            Color32::TRANSPARENT
        };

        ui.painter().rect_filled(rect, CornerRadius::ZERO, bg_color);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(12.0),
            if response.hovered() {
                Color32::WHITE
            } else {
                Color32::from_rgb(160, 160, 170)
            },
        );
    }

    response
}

fn title_icon_button(
    ui: &mut egui::Ui,
    icon: &str,
    active: bool,
    active_color: Color32,
    theme: &Theme,
) -> egui::Response {
    let size = Vec2::new(24.0, 20.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    if ui.is_rect_visible(rect) {
        let bg = if active {
            active_color
        } else if response.hovered() {
            theme.widgets.hovered_bg.to_color32()
        } else {
            Color32::TRANSPARENT
        };

        ui.painter().rect_filled(rect, CornerRadius::same(3), bg);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(12.0),
            if active || response.hovered() {
                Color32::WHITE
            } else {
                theme.text.secondary.to_color32()
            },
        );
    }

    response
}

fn title_play_dropdown(
    ui: &mut egui::Ui,
    active: bool,
    active_color: Color32,
    play_mode: &mut PlayModeState,
    theme: &Theme,
) {
    let button_id = ui.make_persistent_id("title_play_dropdown");
    let size = Vec2::new(34.0, 20.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());

    let main_rect = egui::Rect::from_min_max(rect.min, Pos2::new(rect.right() - 12.0, rect.max.y));
    let dropdown_rect = egui::Rect::from_min_max(Pos2::new(rect.right() - 12.0, rect.min.y), rect.max);

    let main_response = ui.interact(main_rect, button_id.with("main"), Sense::click());
    let dropdown_response = ui.interact(dropdown_rect, button_id.with("dropdown"), Sense::click());

    let is_hovered = response.hovered() || main_response.hovered() || dropdown_response.hovered();

    if is_hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    if ui.is_rect_visible(rect) {
        let bg = if active {
            active_color
        } else if is_hovered {
            theme.widgets.hovered_bg.to_color32()
        } else {
            Color32::TRANSPARENT
        };

        ui.painter().rect_filled(rect, CornerRadius::same(3), bg);

        // Play icon
        ui.painter().text(
            Pos2::new(rect.left() + 11.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            PLAY,
            egui::FontId::proportional(12.0),
            if active || is_hovered { Color32::WHITE } else { theme.text.secondary.to_color32() },
        );

        // Divider
        let divider_x = rect.right() - 12.0;
        ui.painter().line_segment(
            [
                Pos2::new(divider_x, rect.top() + 4.0),
                Pos2::new(divider_x, rect.bottom() - 4.0),
            ],
            egui::Stroke::new(1.0, Color32::from_white_alpha(40)),
        );

        // Caret
        ui.painter().text(
            Pos2::new(rect.right() - 6.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            CARET_DOWN,
            egui::FontId::proportional(8.0),
            Color32::from_white_alpha(180),
        );
    }

    // Main click - play/resume
    if main_response.clicked() {
        if play_mode.state == PlayState::Paused {
            play_mode.state = PlayState::Playing;
        } else if play_mode.state == PlayState::ScriptsPaused {
            play_mode.state = PlayState::ScriptsOnly;
        } else if play_mode.is_editing() {
            play_mode.request_play = true;
        }
    }

    // Dropdown click
    if dropdown_response.clicked() {
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
            ui.set_min_width(160.0);
            ui.style_mut().spacing.item_spacing.y = 2.0;

            let play_icon_color = theme.semantic.success.to_color32();
            let scripts_icon_color = theme.semantic.accent.to_color32();

            if title_play_menu_item(ui, PLAY, "Play (Fullscreen)", "F5", play_icon_color) {
                if play_mode.is_editing() {
                    play_mode.request_play = true;
                }
                ui.close();
            }

            if title_play_menu_item(ui, PLAY, "Run Scripts", "Shift+F5", scripts_icon_color) {
                if play_mode.is_editing() {
                    play_mode.request_scripts_only = true;
                }
                ui.close();
            }
        },
    );

    let tooltip = if play_mode.is_paused() || play_mode.state == PlayState::ScriptsPaused {
        "Resume"
    } else if play_mode.is_in_play_mode() {
        "Playing..."
    } else {
        "Play (click arrow for options)"
    };
    response.on_hover_text(tooltip);
}

fn title_play_menu_item(ui: &mut egui::Ui, icon: &str, label: &str, shortcut: &str, icon_color: Color32) -> bool {
    let desired_size = Vec2::new(ui.available_width().max(160.0), 24.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

    if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(2), Color32::from_white_alpha(15));
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    ui.painter().text(
        Pos2::new(rect.left() + 16.0, rect.center().y),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(13.0),
        icon_color,
    );

    ui.painter().text(
        Pos2::new(rect.left() + 32.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(12.0),
        Color32::WHITE,
    );

    ui.painter().text(
        Pos2::new(rect.right() - 8.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        shortcut,
        egui::FontId::proportional(10.0),
        Color32::from_white_alpha(100),
    );

    response.clicked()
}

/// Render a simplified title bar for the splash screen (no menu items)
pub fn render_splash_title_bar(
    ctx: &egui::Context,
    window_state: &mut WindowState,
    theme: &Theme,
) {
    let is_maximized = window_state.is_maximized;

    egui::TopBottomPanel::top("splash_title_bar")
        .exact_height(TITLE_BAR_HEIGHT)
        .frame(egui::Frame::NONE.fill(theme.surfaces.window.to_color32()))
        .show(ctx, |ui| {
            let panel_rect = ui.max_rect();
            let painter = ui.painter();

            // Window button dimensions
            let button_width = 40.0;
            let window_buttons_width = button_width * 3.0;

            // Draw centered title with icon
            let title_text = "Renzora Engine r1".to_string();
            let title_galley = painter.layout_no_wrap(
                title_text.clone(),
                egui::FontId::proportional(13.0),
                theme.text.muted.to_color32(),
            );
            let title_pos = Pos2::new(
                panel_rect.center().x - title_galley.size().x / 2.0,
                panel_rect.center().y - title_galley.size().y / 2.0,
            );
            painter.galley(title_pos, title_galley, theme.text.muted.to_color32());

            // Draw bottom border
            painter.line_segment(
                [
                    Pos2::new(panel_rect.left(), panel_rect.bottom() - 1.0),
                    Pos2::new(panel_rect.right(), panel_rect.bottom() - 1.0),
                ],
                Stroke::new(1.0, theme.surfaces.extreme.to_color32()),
            );

            // Drag area (everything except window buttons)
            let drag_rect = egui::Rect::from_min_max(
                panel_rect.min,
                Pos2::new(panel_rect.max.x - window_buttons_width, panel_rect.max.y),
            );

            let drag_response = ui.interact(drag_rect, Id::new("splash_title_bar_drag"), Sense::click_and_drag());

            // Handle double-click to maximize
            if drag_response.double_clicked() {
                window_state.request_toggle_maximize = true;
            }

            // Handle drag
            if drag_response.drag_started() {
                window_state.start_drag = true;
            }

            if drag_response.drag_stopped() || !drag_response.dragged() && !drag_response.drag_started() {
                if window_state.is_being_dragged {
                    window_state.is_being_dragged = false;
                    window_state.drag_offset = None;
                }
            }

            // Layout window buttons on the right
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                // Fill space to push buttons to the right
                ui.add_space(ui.available_width() - window_buttons_width);

                ui.spacing_mut().item_spacing.x = 0.0;

                // Minimize button
                let min_resp = window_button(ui, MINUS, theme.widgets.hovered_bg.to_color32(), button_width);
                if min_resp.clicked() {
                    window_state.request_minimize = true;
                }

                // Maximize/Restore button
                let max_icon = if is_maximized { SQUARES_FOUR } else { SQUARE };
                let max_resp = window_button(ui, max_icon, theme.widgets.hovered_bg.to_color32(), button_width);
                if max_resp.clicked() {
                    window_state.request_toggle_maximize = true;
                }

                // Close button (red on hover)
                let close_resp = window_button(ui, X, theme.panels.close_hover.to_color32(), button_width);
                if close_resp.clicked() {
                    window_state.request_close = true;
                }
            });
        });
}
