//! VR Settings panel
//!
//! Provides UI for configuring VR settings: render scale, locomotion mode,
//! comfort options, hand tracking, and headset status indicators.

use bevy::prelude::*;
use bevy_egui::egui;
use renzora_theme::Theme;

/// VR Settings panel state (mirrors VrConfig for UI editing)
#[derive(Resource, Default)]
pub struct VrSettingsState {
    pub render_scale: f32,
    pub comfort_vignette: f32,
    pub snap_turn_angle: f32,
    pub locomotion_mode: usize, // 0=Teleport, 1=Smooth, 2=Both
    pub move_speed: f32,
    pub hand_tracking_enabled: bool,
    pub seated_mode: bool,
    pub locomotion_hand: usize, // 0=Left, 1=Right
    pub thumbstick_deadzone: f32,
    // Status (read-only display)
    pub status_text: String,
    pub headset_name: String,
    pub refresh_rate: f32,
    pub available_refresh_rates: Vec<f32>,
    /// Index into available_refresh_rates for the dropdown (0 = current/default)
    pub selected_refresh_rate_idx: usize,
    pub left_battery: f32,
    pub right_battery: f32,
    // Mixed Reality
    pub passthrough_enabled: bool,
    pub blend_mode: usize, // 0=Opaque, 1=Additive, 2=AlphaBlend
    // Capabilities (read-only)
    pub hand_tracking_supported: bool,
    pub passthrough_supported: bool,
    pub eye_tracking_supported: bool,
    pub foveation_supported: bool,
    pub foveated_rendering: bool,
    // Reference space
    pub reference_space: String, // "stage" or "local"
    /// Dirty flag — when true, sync back to VrConfig resource
    pub dirty: bool,
    /// Whether the state has been initialized from VrConfig
    pub initialized: bool,
}

/// Render VR settings panel content
pub fn render_vr_settings_content(
    ui: &mut egui::Ui,
    state: &mut VrSettingsState,
    theme: &Theme,
) {
    if !state.initialized {
        // Set defaults until synced from VrConfig resource
        state.render_scale = 1.0;
        state.comfort_vignette = 0.3;
        state.snap_turn_angle = 45.0;
        state.locomotion_mode = 0;
        state.move_speed = 2.0;
        state.hand_tracking_enabled = true;
        state.seated_mode = false;
        state.locomotion_hand = 0;
        state.thumbstick_deadzone = 0.2;
        state.status_text = "Not connected".to_string();
        state.initialized = true;
    }

    let _text_color = theme.text.primary.to_color32();
    let muted_color = theme.text.muted.to_color32();

    egui::ScrollArea::vertical().show(ui, |ui| {
        // ---- Status Section ----
        ui.label(egui::RichText::new("Headset Status").size(13.0).color(muted_color));
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Status:");
            ui.label(&state.status_text);
        });

        if !state.headset_name.is_empty() {
            ui.horizontal(|ui| {
                ui.label("Headset:");
                ui.label(&state.headset_name);
            });
        }

        ui.horizontal(|ui| {
            ui.label("Refresh Rate:");
            if state.available_refresh_rates.len() > 1 {
                let rates = &state.available_refresh_rates;
                // Find the index matching the current actual rate
                if state.selected_refresh_rate_idx >= rates.len() {
                    state.selected_refresh_rate_idx = rates.iter()
                        .position(|r| (*r - state.refresh_rate).abs() < 1.0)
                        .unwrap_or(0);
                }
                if egui::ComboBox::from_id_salt("vr_refresh_rate")
                    .width(80.0)
                    .show_index(ui, &mut state.selected_refresh_rate_idx, rates.len(), |i| {
                        format!("{:.0} Hz", rates[i])
                    })
                    .changed()
                {
                    state.dirty = true;
                }
            } else {
                ui.label(format!("{:.0} Hz", state.refresh_rate));
            }
        });

        if state.left_battery >= 0.0 || state.right_battery >= 0.0 {
            ui.horizontal(|ui| {
                if state.left_battery >= 0.0 {
                    ui.label(format!("L: {:.0}%", state.left_battery * 100.0));
                }
                if state.right_battery >= 0.0 {
                    ui.label(format!("R: {:.0}%", state.right_battery * 100.0));
                }
            });
        }

        ui.add_space(8.0);

        // ---- Rendering Section ----
        ui.label(egui::RichText::new("Rendering").size(13.0).color(muted_color));
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Render Scale:");
            if ui.add(egui::Slider::new(&mut state.render_scale, 0.5..=2.0).step_by(0.1)).changed() {
                state.dirty = true;
            }
        });

        ui.add_space(8.0);

        // ---- Locomotion Section ----
        ui.label(egui::RichText::new("Locomotion").size(13.0).color(muted_color));
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Mode:");
            if egui::ComboBox::from_id_salt("vr_loco_mode")
                .show_index(ui, &mut state.locomotion_mode, 3, |i| match i {
                    0 => "Teleport",
                    1 => "Smooth",
                    _ => "Both",
                })
                .changed()
            {
                state.dirty = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Speed:");
            if ui.add(egui::Slider::new(&mut state.move_speed, 0.5..=10.0).step_by(0.5).suffix(" m/s")).changed() {
                state.dirty = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Hand:");
            if egui::ComboBox::from_id_salt("vr_loco_hand")
                .show_index(ui, &mut state.locomotion_hand, 2, |i| match i {
                    0 => "Left",
                    _ => "Right",
                })
                .changed()
            {
                state.dirty = true;
            }
        });

        ui.add_space(8.0);

        // ---- Turning Section ----
        ui.label(egui::RichText::new("Turning").size(13.0).color(muted_color));
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Snap Angle:");
            if ui.add(egui::Slider::new(&mut state.snap_turn_angle, 0.0..=90.0).step_by(15.0).suffix("°")).changed() {
                state.dirty = true;
            }
        });
        if state.snap_turn_angle == 0.0 {
            ui.label(egui::RichText::new("  (smooth turning)").size(10.0).color(muted_color));
        }

        ui.add_space(8.0);

        // ---- Comfort Section ----
        ui.label(egui::RichText::new("Comfort").size(13.0).color(muted_color));
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Vignette:");
            if ui.add(egui::Slider::new(&mut state.comfort_vignette, 0.0..=1.0).step_by(0.1)).changed() {
                state.dirty = true;
            }
        });

        if ui.checkbox(&mut state.seated_mode, "Seated Mode").changed() {
            state.dirty = true;
        }

        ui.add_space(8.0);

        // ---- Input Section ----
        ui.label(egui::RichText::new("Input").size(13.0).color(muted_color));
        ui.separator();

        if ui.checkbox(&mut state.hand_tracking_enabled, "Hand Tracking").changed() {
            state.dirty = true;
        }

        ui.horizontal(|ui| {
            ui.label("Deadzone:");
            if ui.add(egui::Slider::new(&mut state.thumbstick_deadzone, 0.05..=0.5).step_by(0.05)).changed() {
                state.dirty = true;
            }
        });

        ui.add_space(8.0);

        // ---- Mixed Reality Section ----
        ui.label(egui::RichText::new("Mixed Reality").size(13.0).color(muted_color));
        ui.separator();

        if ui.checkbox(&mut state.passthrough_enabled, "Passthrough").changed() {
            state.dirty = true;
        }

        ui.horizontal(|ui| {
            ui.label("Blend Mode:");
            if egui::ComboBox::from_id_salt("vr_blend_mode")
                .show_index(ui, &mut state.blend_mode, 3, |i| match i {
                    0 => "Opaque",
                    1 => "Additive",
                    _ => "Alpha Blend",
                })
                .changed()
            {
                state.dirty = true;
            }
        });

        if state.foveation_supported {
            if ui.checkbox(&mut state.foveated_rendering, "Foveated Rendering").changed() {
                state.dirty = true;
            }
        }

        ui.add_space(8.0);

        // ---- Capabilities Section ----
        ui.label(egui::RichText::new("Capabilities").size(13.0).color(muted_color));
        ui.separator();

        let check = |supported: bool| if supported { "Supported" } else { "Not Available" };
        ui.horizontal(|ui| {
            ui.label("Hand Tracking:");
            ui.label(check(state.hand_tracking_supported));
        });
        ui.horizontal(|ui| {
            ui.label("Passthrough:");
            ui.label(check(state.passthrough_supported));
        });
        ui.horizontal(|ui| {
            ui.label("Eye Tracking:");
            ui.label(check(state.eye_tracking_supported));
        });
        ui.horizontal(|ui| {
            ui.label("Foveated Rendering:");
            ui.label(check(state.foveation_supported));
        });
        ui.horizontal(|ui| {
            ui.label("Reference Space:");
            ui.label(if state.reference_space.is_empty() { "N/A" } else { &state.reference_space });
        });
    });
}
