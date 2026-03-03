//! VR Session panel
//!
//! Shows session lifecycle state, extension availability, and diagnostics.
//! Includes a Start/Stop button for manual VR session control.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, RichText};
use renzora_theme::Theme;

/// VR Session panel state
#[derive(Resource)]
pub struct VrSessionPanelState {
    pub show_extensions: bool,
    // Populated externally from VrSessionState / VrCapabilities
    pub status: String,
    pub headset_name: String,
    pub runtime_name: String,
    pub reference_space: String,
    // Capabilities
    pub hand_tracking: bool,
    pub passthrough: bool,
    pub eye_tracking: bool,
    pub foveation: bool,
    pub overlay: bool,
    pub spatial_anchors: bool,
    // Frame timing
    pub should_render: bool,
    pub target_fps: f32,
    pub actual_fps: f32,
    // Extension list
    pub enabled_extensions: Vec<String>,
    // Session control
    pub start_requested: bool,
    pub stop_requested: bool,
}

impl Default for VrSessionPanelState {
    fn default() -> Self {
        Self {
            show_extensions: false,
            status: "Disconnected".to_string(),
            headset_name: String::new(),
            runtime_name: String::new(),
            reference_space: "stage".to_string(),
            hand_tracking: false,
            passthrough: false,
            eye_tracking: false,
            foveation: false,
            overlay: false,
            spatial_anchors: false,
            should_render: false,
            target_fps: 90.0,
            actual_fps: 0.0,
            enabled_extensions: Vec::new(),
            start_requested: false,
            stop_requested: false,
        }
    }
}

pub fn render_vr_session_content(
    ui: &mut egui::Ui,
    state: &mut VrSessionPanelState,
    theme: &Theme,
) {
    let muted = theme.text.muted.to_color32();
    let green = Color32::from_rgb(60, 200, 100);
    let yellow = Color32::from_rgb(200, 200, 60);
    let red = Color32::from_rgb(200, 60, 60);
    let gray = Color32::from_gray(120);

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.set_width(ui.available_width());

        // ---- Start/Stop Button ----
        let is_active = matches!(state.status.as_str(), "focused" | "visible" | "ready" | "initializing" | "stopping");

        if is_active {
            let button = egui::Button::new(
                RichText::new("\u{23F9}  Stop VR").size(14.0).color(Color32::WHITE).strong()
            )
                .fill(Color32::from_rgb(180, 50, 50))
                .min_size(egui::Vec2::new(ui.available_width(), 32.0));
            if ui.add(button).clicked() {
                state.stop_requested = true;
            }
        } else {
            let button = egui::Button::new(
                RichText::new("\u{25B6}  Start VR").size(14.0).color(Color32::WHITE).strong()
            )
                .fill(Color32::from_rgb(50, 160, 80))
                .min_size(egui::Vec2::new(ui.available_width(), 32.0));
            if ui.add(button).clicked() {
                state.start_requested = true;
            }
        }

        ui.add_space(8.0);

        // ---- Session State ----
        ui.label(RichText::new("Session State").size(13.0).color(muted));
        ui.separator();

        let (status_color, status_label) = match state.status.as_str() {
            "focused" => (green, "Focused"),
            "visible" => (yellow, "Visible"),
            "ready" => (Color32::from_rgb(100, 160, 255), "Ready"),
            "initializing" => (gray, "Initializing"),
            "stopping" => (Color32::from_rgb(255, 160, 60), "Stopping"),
            "stopped" => (red, "Stopped"),
            _ => (red, "Disconnected"),
        };

        ui.horizontal(|ui| {
            ui.label("Status:");
            ui.colored_label(status_color, RichText::new(status_label).strong());
        });

        ui.horizontal(|ui| {
            ui.label("Rendering:");
            if state.should_render {
                ui.colored_label(green, "Active");
            } else {
                ui.colored_label(gray, "Inactive");
            }
        });

        ui.add_space(8.0);

        // ---- Headset Info ----
        ui.label(RichText::new("Headset Info").size(13.0).color(muted));
        ui.separator();

        if !state.headset_name.is_empty() {
            ui.horizontal(|ui| {
                ui.label("Headset:");
                ui.label(&state.headset_name);
            });
        }

        if !state.runtime_name.is_empty() {
            ui.horizontal(|ui| {
                ui.label("Runtime:");
                ui.label(&state.runtime_name);
            });
        }

        ui.horizontal(|ui| {
            ui.label("Reference Space:");
            ui.label(&state.reference_space);
        });

        ui.add_space(8.0);

        // ---- Capabilities ----
        ui.label(RichText::new("Capabilities").size(13.0).color(muted));
        ui.separator();

        let cap_row = |ui: &mut egui::Ui, name: &str, supported: bool| {
            ui.horizontal(|ui| {
                let (icon, color) = if supported {
                    ("\u{2714}", green)  // checkmark
                } else {
                    ("\u{2718}", red)    // cross
                };
                ui.colored_label(color, icon);
                ui.label(name);
            });
        };

        cap_row(ui, "Hand Tracking", state.hand_tracking);
        cap_row(ui, "Passthrough", state.passthrough);
        cap_row(ui, "Eye Tracking", state.eye_tracking);
        cap_row(ui, "Foveated Rendering", state.foveation);
        cap_row(ui, "Overlay", state.overlay);
        cap_row(ui, "Spatial Anchors", state.spatial_anchors);

        ui.add_space(8.0);

        // ---- Frame Timing ----
        ui.label(RichText::new("Frame Timing").size(13.0).color(muted));
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Target:");
            ui.label(format!("{:.0} Hz ({:.1} ms)", state.target_fps, 1000.0 / state.target_fps.max(1.0)));
        });

        ui.horizontal(|ui| {
            ui.label("Actual:");
            let color = if state.actual_fps >= state.target_fps * 0.95 { green }
                else if state.actual_fps >= state.target_fps * 0.8 { yellow }
                else { red };
            ui.colored_label(color, format!("{:.0} FPS", state.actual_fps));
        });

        ui.add_space(8.0);

        // ---- Extensions ----
        if ui.collapsing("Enabled Extensions", |ui| {
            if state.enabled_extensions.is_empty() {
                ui.label(RichText::new("No extensions detected").color(muted));
            } else {
                for ext in &state.enabled_extensions {
                    ui.label(ext);
                }
            }
        }).header_response.clicked() {
            state.show_extensions = !state.show_extensions;
        }
    });
}
