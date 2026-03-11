//! Authentication UI — sign-in, register, and forgot-password windows.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CursorIcon, Sense};
use renzora_theme::Theme;

/// Current view within the auth window.
#[derive(Clone, Copy, PartialEq, Default)]
pub enum AuthView {
    #[default]
    SignIn,
    Register,
    ForgotPassword,
}

/// Persistent authentication UI state.
#[derive(Resource, Default)]
pub struct AuthState {
    pub window_open: bool,
    pub view: AuthView,
    pub email: String,
    pub password: String,
    pub confirm_password: String,
    pub username: String,
}

/// Render the auth modal window. Call this each frame from the editor loop.
pub fn render_auth_window(ctx: &egui::Context, theme: &Theme, state: &mut AuthState) {
    if !state.window_open {
        return;
    }

    let accent = theme.semantic.accent.to_color32();
    let text_secondary = theme.text.secondary.to_color32();

    let title = match state.view {
        AuthView::SignIn => "Sign In",
        AuthView::Register => "Create Account",
        AuthView::ForgotPassword => "Reset Password",
    };

    let mut open = state.window_open;

    egui::Window::new(title)
        .id(egui::Id::new("auth_window"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([320.0, 0.0])
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(theme.surfaces.panel.to_color32())
                .stroke(egui::Stroke::new(1.0, theme.widgets.border.to_color32()))
                .corner_radius(egui::CornerRadius::same(8)),
        )
        .show(ctx, |ui| {
            ui.add_space(8.0);

            match state.view {
                AuthView::SignIn => {
                    render_sign_in_form(ui, state, accent, text_secondary);
                }
                AuthView::Register => {
                    render_register_form(ui, state, accent, text_secondary);
                }
                AuthView::ForgotPassword => {
                    render_forgot_password_form(ui, state, accent, text_secondary);
                }
            }

            ui.add_space(4.0);
        });

    state.window_open = open;
}

fn render_sign_in_form(
    ui: &mut egui::Ui,
    state: &mut AuthState,
    accent: Color32,
    text_secondary: Color32,
) {
    // Email
    ui.label(egui::RichText::new("Email").size(11.0).color(text_secondary));
    ui.add_space(2.0);
    ui.add(
        egui::TextEdit::singleline(&mut state.email)
            .desired_width(f32::INFINITY)
            .hint_text("you@example.com"),
    );
    ui.add_space(8.0);

    // Password
    ui.label(egui::RichText::new("Password").size(11.0).color(text_secondary));
    ui.add_space(2.0);
    ui.add(
        egui::TextEdit::singleline(&mut state.password)
            .desired_width(f32::INFINITY)
            .password(true)
            .hint_text("Password"),
    );
    ui.add_space(4.0);

    // Forgot password link
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
        let forgot = ui.add(
            egui::Label::new(
                egui::RichText::new("Forgot password?")
                    .size(11.0)
                    .color(accent),
            )
            .sense(Sense::click()),
        );
        if forgot.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if forgot.clicked() {
            state.view = AuthView::ForgotPassword;
        }
    });

    ui.add_space(12.0);

    // Sign In button
    let btn = ui.add_sized(
        [ui.available_width(), 32.0],
        egui::Button::new(
            egui::RichText::new("Sign In")
                .color(Color32::WHITE)
                .size(13.0),
        )
        .fill(accent)
        .corner_radius(egui::CornerRadius::same(4)),
    );
    if btn.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    ui.add_space(12.0);

    // Register link
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Don't have an account?")
                .size(11.0)
                .color(text_secondary),
        );
        let reg = ui.add(
            egui::Label::new(egui::RichText::new("Register").size(11.0).color(accent))
                .sense(Sense::click()),
        );
        if reg.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if reg.clicked() {
            state.view = AuthView::Register;
        }
    });
}

fn render_register_form(
    ui: &mut egui::Ui,
    state: &mut AuthState,
    accent: Color32,
    text_secondary: Color32,
) {
    // Username
    ui.label(egui::RichText::new("Username").size(11.0).color(text_secondary));
    ui.add_space(2.0);
    ui.add(
        egui::TextEdit::singleline(&mut state.username)
            .desired_width(f32::INFINITY)
            .hint_text("Username"),
    );
    ui.add_space(8.0);

    // Email
    ui.label(egui::RichText::new("Email").size(11.0).color(text_secondary));
    ui.add_space(2.0);
    ui.add(
        egui::TextEdit::singleline(&mut state.email)
            .desired_width(f32::INFINITY)
            .hint_text("you@example.com"),
    );
    ui.add_space(8.0);

    // Password
    ui.label(egui::RichText::new("Password").size(11.0).color(text_secondary));
    ui.add_space(2.0);
    ui.add(
        egui::TextEdit::singleline(&mut state.password)
            .desired_width(f32::INFINITY)
            .password(true)
            .hint_text("Password"),
    );
    ui.add_space(8.0);

    // Confirm password
    ui.label(
        egui::RichText::new("Confirm Password")
            .size(11.0)
            .color(text_secondary),
    );
    ui.add_space(2.0);
    ui.add(
        egui::TextEdit::singleline(&mut state.confirm_password)
            .desired_width(f32::INFINITY)
            .password(true)
            .hint_text("Confirm password"),
    );

    ui.add_space(16.0);

    // Create Account button
    let btn = ui.add_sized(
        [ui.available_width(), 32.0],
        egui::Button::new(
            egui::RichText::new("Create Account")
                .color(Color32::WHITE)
                .size(13.0),
        )
        .fill(accent)
        .corner_radius(egui::CornerRadius::same(4)),
    );
    if btn.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    ui.add_space(12.0);

    // Back to sign in
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Already have an account?")
                .size(11.0)
                .color(text_secondary),
        );
        let back = ui.add(
            egui::Label::new(egui::RichText::new("Sign In").size(11.0).color(accent))
                .sense(Sense::click()),
        );
        if back.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if back.clicked() {
            state.view = AuthView::SignIn;
        }
    });
}

fn render_forgot_password_form(
    ui: &mut egui::Ui,
    state: &mut AuthState,
    accent: Color32,
    text_secondary: Color32,
) {
    ui.label(
        egui::RichText::new("Enter your email and we'll send you a link to reset your password.")
            .size(11.0)
            .color(text_secondary)
            .weak(),
    );
    ui.add_space(12.0);

    // Email
    ui.label(egui::RichText::new("Email").size(11.0).color(text_secondary));
    ui.add_space(2.0);
    ui.add(
        egui::TextEdit::singleline(&mut state.email)
            .desired_width(f32::INFINITY)
            .hint_text("you@example.com"),
    );

    ui.add_space(16.0);

    // Send Reset Link button
    let btn = ui.add_sized(
        [ui.available_width(), 32.0],
        egui::Button::new(
            egui::RichText::new("Send Reset Link")
                .color(Color32::WHITE)
                .size(13.0),
        )
        .fill(accent)
        .corner_radius(egui::CornerRadius::same(4)),
    );
    if btn.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    ui.add_space(12.0);

    // Back to sign in
    ui.horizontal(|ui| {
        let back = ui.add(
            egui::Label::new(
                egui::RichText::new("Back to Sign In")
                    .size(11.0)
                    .color(accent),
            )
            .sense(Sense::click()),
        );
        if back.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if back.clicked() {
            state.view = AuthView::SignIn;
        }
    });
}
