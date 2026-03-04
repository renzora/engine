//! VR Devices panel
//!
//! Shows connected VR devices with tracking status and battery levels.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, RichText};
use renzora_theme::Theme;

/// Tracking quality for a VR device
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrackingQuality {
    #[default]
    Unknown,
    Lost,
    Degraded,
    Good,
}

impl TrackingQuality {
    #[allow(dead_code)]
    pub fn label(&self) -> &'static str {
        match self {
            TrackingQuality::Unknown => "Unknown",
            TrackingQuality::Lost => "Lost",
            TrackingQuality::Degraded => "Degraded",
            TrackingQuality::Good => "Good",
        }
    }
}

/// Status of a single VR device
#[derive(Debug, Clone, Default)]
pub struct DeviceStatus {
    pub name: String,
    pub connected: bool,
    pub tracked: bool,
    /// Battery level 0.0-1.0, negative = unknown
    pub battery: f32,
    pub tracking_quality: TrackingQuality,
}

/// VR Devices panel state
#[derive(Resource, Default)]
pub struct VrDevicesState {
    pub headset: DeviceStatus,
    pub left_controller: DeviceStatus,
    pub right_controller: DeviceStatus,
}

pub fn render_vr_devices_content(
    ui: &mut egui::Ui,
    state: &mut VrDevicesState,
    theme: &Theme,
) {
    let muted = theme.text.muted.to_color32();

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.set_width(ui.available_width());

        render_device_card(ui, "Headset", &state.headset, muted);
        ui.add_space(8.0);
        render_device_card(ui, "Left Controller", &state.left_controller, muted);
        ui.add_space(8.0);
        render_device_card(ui, "Right Controller", &state.right_controller, muted);
    });
}

fn render_device_card(
    ui: &mut egui::Ui,
    label: &str,
    device: &DeviceStatus,
    muted: Color32,
) {
    let green = Color32::from_rgb(60, 200, 100);
    let yellow = Color32::from_rgb(200, 200, 60);
    let red = Color32::from_rgb(200, 60, 60);
    let gray = Color32::from_gray(120);

    ui.label(RichText::new(label).size(13.0).color(muted));
    ui.separator();

    // Status dot + device name
    ui.horizontal(|ui| {
        let dot_color = if !device.connected {
            gray
        } else {
            match device.tracking_quality {
                TrackingQuality::Good => green,
                TrackingQuality::Degraded => yellow,
                TrackingQuality::Lost => red,
                TrackingQuality::Unknown => gray,
            }
        };
        ui.colored_label(dot_color, "\u{25CF}"); // filled circle
        if device.name.is_empty() {
            ui.label(if device.connected { "Connected" } else { "Not connected" });
        } else {
            ui.label(&device.name);
        }
    });

    // Tracking quality
    ui.horizontal(|ui| {
        ui.label("Tracking:");
        if !device.connected {
            ui.colored_label(gray, "N/A");
        } else {
            let (color, text) = match device.tracking_quality {
                TrackingQuality::Good => (green, "Good"),
                TrackingQuality::Degraded => (yellow, "Degraded"),
                TrackingQuality::Lost => (red, "Lost"),
                TrackingQuality::Unknown => (gray, "Unknown"),
            };
            ui.colored_label(color, text);
        }
    });

    // Battery bar
    if device.battery >= 0.0 {
        ui.horizontal(|ui| {
            ui.label("Battery:");
            let pct = (device.battery * 100.0).round() as u32;
            let bar_color = if pct > 50 { green } else if pct > 20 { yellow } else { red };
            let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(80.0, 12.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 3.0, Color32::from_gray(50));
            let filled = egui::Rect::from_min_size(
                rect.min,
                egui::Vec2::new(rect.width() * device.battery.clamp(0.0, 1.0), rect.height()),
            );
            ui.painter().rect_filled(filled, 3.0, bar_color);
            ui.label(format!("{}%", pct));
        });
    }
}
