//! Authentication UI and API client for Renzora Editor.
//!
//! Provides sign-in, register, and forgot-password modals that communicate
//! with the renzora.com API. Tokens are persisted to disk for auto-login.

pub mod api;
pub mod marketplace;
pub mod session;

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CursorIcon, Sense};
use renzora_theme::Theme;

pub use session::AuthSession;

use std::sync::{mpsc, Mutex};

/// Current view within the auth window.
#[derive(Clone, Copy, PartialEq, Default)]
pub enum AuthView {
    #[default]
    SignIn,
    Register,
    ForgotPassword,
}

/// Result from a background auth API call.
enum AuthResult {
    Success(api::AuthResponse),
    ForgotSuccess(String),
    Error(String),
}

/// Persistent authentication UI state.
#[derive(Resource)]
pub struct AuthState {
    pub window_open: bool,
    pub view: AuthView,
    pub email: String,
    pub password: String,
    pub confirm_password: String,
    pub username: String,
    /// Status message shown in the UI.
    pub status: Option<String>,
    /// Error message shown in the UI.
    pub error: Option<String>,
    /// Whether an API call is in flight.
    pub loading: bool,
    /// Set to `true` when sign-in succeeds so the editor can react (e.g. switch layout).
    pub just_signed_in: bool,
    /// Channel receiver for background API results.
    receiver: Option<Mutex<mpsc::Receiver<AuthResult>>>,
}

impl Default for AuthState {
    fn default() -> Self {
        Self {
            window_open: false,
            view: AuthView::default(),
            email: String::new(),
            password: String::new(),
            confirm_password: String::new(),
            username: String::new(),
            status: None,
            error: None,
            loading: false,
            just_signed_in: false,
            receiver: None,
        }
    }
}

/// Render the auth modal window. Call this each frame from the editor loop.
pub fn render_auth_window(
    ctx: &egui::Context,
    theme: &Theme,
    state: &mut AuthState,
    session: &mut AuthSession,
) {
    // Poll for background API results
    poll_auth_result(state, session);

    if !state.window_open {
        return;
    }

    let accent = theme.semantic.accent.to_color32();
    let text_secondary = theme.text.secondary.to_color32();

    // If signed in, no need for the auth window — the title-bar dropdown
    // handles settings / sign-out / library.
    if session.is_signed_in() {
        state.window_open = false;
        return;
    }

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

            // Status/error messages
            if let Some(msg) = &state.status {
                ui.label(
                    egui::RichText::new(msg)
                        .size(11.0)
                        .color(Color32::from_rgb(34, 197, 94)),
                );
                ui.add_space(4.0);
            }
            if let Some(err) = &state.error {
                ui.label(
                    egui::RichText::new(err)
                        .size(11.0)
                        .color(Color32::from_rgb(239, 68, 68)),
                );
                ui.add_space(4.0);
            }

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

/// Poll for results from background auth API calls.
fn poll_auth_result(state: &mut AuthState, session: &mut AuthSession) {
    let result = state
        .receiver
        .as_ref()
        .and_then(|rx| rx.lock().ok())
        .and_then(|rx| rx.try_recv().ok());

    if let Some(result) = result {
        state.loading = false;
        match result {
            AuthResult::Success(response) => {
                session.set_from_response(&response);
                #[cfg(not(target_arch = "wasm32"))]
                session::save_session(session);
                state.error = None;
                state.status = None;
                // Clear form fields and close window — editor will handle the transition
                state.password.clear();
                state.confirm_password.clear();
                state.window_open = false;
                state.just_signed_in = true;
            }
            AuthResult::ForgotSuccess(msg) => {
                state.error = None;
                state.status = Some(msg);
            }
            AuthResult::Error(err) => {
                state.status = None;
                state.error = Some(err);
            }
        }
    }
}

/// Spawn a background thread for an auth API call.
#[cfg(not(target_arch = "wasm32"))]
fn spawn_auth_request(state: &mut AuthState, f: impl FnOnce() -> AuthResult + Send + 'static) {
    state.loading = true;
    state.error = None;
    state.status = None;

    let (tx, rx) = mpsc::channel();
    state.receiver = Some(Mutex::new(rx));

    std::thread::spawn(move || {
        let result = f();
        let _ = tx.send(result);
    });
}

#[cfg(target_arch = "wasm32")]
fn spawn_auth_request(_state: &mut AuthState, _f: impl FnOnce() -> AuthResult + Send + 'static) {
    // No-op on WASM
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
            state.error = None;
            state.status = None;
        }
    });

    ui.add_space(12.0);

    // Sign In button
    let btn_text = if state.loading { "Signing in..." } else { "Sign In" };
    let btn = ui.add_sized(
        [ui.available_width(), 32.0],
        egui::Button::new(
            egui::RichText::new(btn_text)
                .color(Color32::WHITE)
                .size(13.0),
        )
        .fill(accent)
        .corner_radius(egui::CornerRadius::same(4)),
    );
    if btn.hovered() && !state.loading {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if btn.clicked() && !state.loading {
        let email = state.email.clone();
        let password = state.password.clone();
        spawn_auth_request(state, move || match api::login(&email, &password) {
            Ok(resp) => AuthResult::Success(resp),
            Err(e) => AuthResult::Error(e),
        });
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
            state.error = None;
            state.status = None;
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
    let btn_text = if state.loading {
        "Creating account..."
    } else {
        "Create Account"
    };
    let btn = ui.add_sized(
        [ui.available_width(), 32.0],
        egui::Button::new(
            egui::RichText::new(btn_text)
                .color(Color32::WHITE)
                .size(13.0),
        )
        .fill(accent)
        .corner_radius(egui::CornerRadius::same(4)),
    );
    if btn.hovered() && !state.loading {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if btn.clicked() && !state.loading {
        // Client-side validation
        if state.password != state.confirm_password {
            state.error = Some("Passwords do not match".into());
        } else if state.password.len() < 8 {
            state.error = Some("Password must be at least 8 characters".into());
        } else if state.username.len() < 3 {
            state.error = Some("Username must be at least 3 characters".into());
        } else {
            let username = state.username.clone();
            let email = state.email.clone();
            let password = state.password.clone();
            spawn_auth_request(state, move || {
                match api::register(&username, &email, &password) {
                    Ok(resp) => AuthResult::Success(resp),
                    Err(e) => AuthResult::Error(e),
                }
            });
        }
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
            state.error = None;
            state.status = None;
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
    let btn_text = if state.loading {
        "Sending..."
    } else {
        "Send Reset Link"
    };
    let btn = ui.add_sized(
        [ui.available_width(), 32.0],
        egui::Button::new(
            egui::RichText::new(btn_text)
                .color(Color32::WHITE)
                .size(13.0),
        )
        .fill(accent)
        .corner_radius(egui::CornerRadius::same(4)),
    );
    if btn.hovered() && !state.loading {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if btn.clicked() && !state.loading {
        let email = state.email.clone();
        spawn_auth_request(state, move || match api::forgot_password(&email) {
            Ok(resp) => AuthResult::ForgotSuccess(resp.message),
            Err(e) => AuthResult::Error(e),
        });
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
            state.error = None;
            state.status = None;
        }
    });
}

/// Try to restore a previously saved session on startup.
/// Call this once during editor initialization.
#[cfg(not(target_arch = "wasm32"))]
pub fn try_restore_session() -> AuthSession {
    if let Some(mut saved) = session::load_session() {
        // Try to refresh the token in the background to verify it's still valid
        if let Some(refresh) = &saved.refresh_token {
            match api::refresh_token(refresh) {
                Ok(response) => {
                    saved.set_from_response(&response);
                    session::save_session(&saved);
                }
                Err(_) => {
                    // Token expired or invalid — clear session
                    saved.clear();
                    session::delete_session();
                }
            }
        }
        saved
    } else {
        AuthSession::default()
    }
}

#[cfg(target_arch = "wasm32")]
pub fn try_restore_session() -> AuthSession {
    AuthSession::default()
}
