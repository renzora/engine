//! VR Camera Preview panel
//!
//! Shows what the VR user sees without wearing the headset.
//! Displays left/right/center eye views with optional overlays.

use bevy::prelude::*;
use bevy_egui::egui::{self, RichText};
use renzora_theme::Theme;

/// VR Camera Preview panel state
#[derive(Resource)]
pub struct VrCameraPreviewState {
    pub view_mode: usize, // 0=Left, 1=Right, 2=Center
    pub show_grid_overlay: bool,
    pub show_tracking_bounds: bool,
    // HMD info (populated externally)
    pub resolution_per_eye: [u32; 2],
    pub refresh_rate: f32,
    pub ipd_mm: f32,
}

impl Default for VrCameraPreviewState {
    fn default() -> Self {
        Self {
            view_mode: 2,
            show_grid_overlay: false,
            show_tracking_bounds: false,
            resolution_per_eye: [0, 0],
            refresh_rate: 90.0,
            ipd_mm: 63.0,
        }
    }
}

pub fn render_vr_camera_preview_content(
    ui: &mut egui::Ui,
    state: &mut VrCameraPreviewState,
    theme: &Theme,
) {
    let muted = theme.text.muted.to_color32();

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.set_width(ui.available_width());

        // ---- View Mode Selector ----
        ui.horizontal(|ui| {
            ui.label("View:");
            ui.selectable_value(&mut state.view_mode, 0, "Left Eye");
            ui.selectable_value(&mut state.view_mode, 1, "Right Eye");
            ui.selectable_value(&mut state.view_mode, 2, "Center");
        });

        ui.separator();

        // ---- Preview Viewport ----
        let available = ui.available_size();
        let aspect = 16.0 / 9.0;
        let preview_width = available.x.min(600.0);
        let preview_height = preview_width / aspect;

        let (rect, _) = ui.allocate_exact_size(
            egui::Vec2::new(preview_width, preview_height),
            egui::Sense::hover(),
        );

        // Draw preview background
        ui.painter().rect_filled(
            rect,
            4.0,
            egui::Color32::from_gray(20),
        );

        // Placeholder text
        let view_label = match state.view_mode {
            0 => "Left Eye Preview",
            1 => "Right Eye Preview",
            _ => "Center Preview",
        };
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            view_label,
            egui::FontId::proportional(14.0),
            egui::Color32::from_gray(100),
        );

        // Grid overlay
        if state.show_grid_overlay {
            let stroke = egui::Stroke::new(0.5, egui::Color32::from_rgba_unmultiplied(100, 100, 100, 60));
            let step = 40.0;
            let mut x = rect.left() + step;
            while x < rect.right() {
                ui.painter().line_segment([egui::Pos2::new(x, rect.top()), egui::Pos2::new(x, rect.bottom())], stroke);
                x += step;
            }
            let mut y = rect.top() + step;
            while y < rect.bottom() {
                ui.painter().line_segment([egui::Pos2::new(rect.left(), y), egui::Pos2::new(rect.right(), y)], stroke);
                y += step;
            }
        }

        ui.add_space(4.0);

        // ---- Overlay Toggles ----
        ui.horizontal(|ui| {
            ui.checkbox(&mut state.show_grid_overlay, "Grid Overlay");
            ui.checkbox(&mut state.show_tracking_bounds, "Tracking Bounds");
        });

        ui.add_space(8.0);

        // ---- HMD Info Footer ----
        ui.label(RichText::new("HMD Info").size(13.0).color(muted));
        ui.separator();

        if state.resolution_per_eye[0] > 0 {
            ui.horizontal(|ui| {
                ui.label("Resolution/Eye:");
                ui.label(format!("{}x{}", state.resolution_per_eye[0], state.resolution_per_eye[1]));
            });
        }

        ui.horizontal(|ui| {
            ui.label("Refresh Rate:");
            ui.label(format!("{:.0} Hz", state.refresh_rate));
        });

        ui.horizontal(|ui| {
            ui.label("IPD:");
            ui.label(format!("{:.1} mm", state.ipd_mm));
        });
    });
}
