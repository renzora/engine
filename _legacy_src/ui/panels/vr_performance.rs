//! VR Performance panel
//!
//! VR-specific performance metrics: frame budget, reprojection,
//! per-eye render stats.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, RichText, Vec2};
use std::collections::VecDeque;
use renzora_theme::Theme;

/// VR Performance panel state
#[derive(Resource)]
pub struct VrPerformanceState {
    pub frame_time_history: VecDeque<f32>,
    pub target_framerate: f32,
    pub dropped_frames: u32,
    pub reprojection_count: u32,
    pub total_frames: u32,
    // Resolution
    pub resolution_per_eye: [u32; 2],
    pub render_scale: f32,
    pub foveation_active: bool,
}

impl Default for VrPerformanceState {
    fn default() -> Self {
        Self {
            frame_time_history: VecDeque::with_capacity(120),
            target_framerate: 90.0,
            dropped_frames: 0,
            reprojection_count: 0,
            total_frames: 0,
            resolution_per_eye: [0, 0],
            render_scale: 1.0,
            foveation_active: false,
        }
    }
}

pub fn render_vr_performance_content(
    ui: &mut egui::Ui,
    state: &mut VrPerformanceState,
    theme: &Theme,
) {
    let muted = theme.text.muted.to_color32();
    let green = Color32::from_rgb(60, 200, 100);
    let yellow = Color32::from_rgb(200, 200, 60);
    let red = Color32::from_rgb(200, 60, 60);

    let budget_ms = if state.target_framerate > 0.0 {
        1000.0 / state.target_framerate
    } else {
        11.1
    };

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.set_width(ui.available_width());

        // ---- Frame Budget ----
        ui.label(RichText::new("Frame Budget").size(13.0).color(muted));
        ui.separator();

        let current_ms = state.frame_time_history.back().copied().unwrap_or(0.0);
        let budget_usage = current_ms / budget_ms;

        let bar_color = if budget_usage < 0.8 { green }
            else if budget_usage < 1.0 { yellow }
            else { red };

        ui.horizontal(|ui| {
            ui.label(format!("{:.1} ms", current_ms));
            ui.label(format!("/ {:.1} ms budget", budget_ms));
        });

        // Budget bar
        let bar_width = ui.available_width().min(300.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_width, 20.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 4.0, Color32::from_gray(30));

        let fill = (budget_usage.min(1.5) / 1.5) * rect.width();
        let fill_rect = egui::Rect::from_min_size(rect.min, Vec2::new(fill, rect.height()));
        ui.painter().rect_filled(fill_rect, 4.0, bar_color);

        // Budget line at 100%
        let budget_x = rect.left() + rect.width() * (1.0 / 1.5);
        ui.painter().line_segment(
            [egui::Pos2::new(budget_x, rect.top()), egui::Pos2::new(budget_x, rect.bottom())],
            egui::Stroke::new(1.0, Color32::WHITE),
        );

        ui.horizontal(|ui| {
            ui.label(format!("{:.0}% budget", budget_usage * 100.0));
            if budget_usage > 1.0 {
                ui.colored_label(red, "OVER BUDGET");
            }
        });

        ui.add_space(8.0);

        // ---- Frame Time Graph ----
        ui.label(RichText::new("Frame Time History").size(13.0).color(muted));
        ui.separator();

        let graph_width = ui.available_width().min(300.0);
        let graph_height = 80.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(graph_width, graph_height), egui::Sense::hover());
        ui.painter().rect_filled(rect, 2.0, Color32::from_gray(20));

        if !state.frame_time_history.is_empty() {
            let max_ms = budget_ms * 1.5;
            let count = state.frame_time_history.len();
            let step = rect.width() / count.max(1) as f32;

            for (i, &ms) in state.frame_time_history.iter().enumerate() {
                let x = rect.left() + i as f32 * step;
                let h = (ms / max_ms).min(1.0) * rect.height();
                let color = if ms < budget_ms * 0.8 { green }
                    else if ms < budget_ms { yellow }
                    else { red };

                ui.painter().line_segment(
                    [
                        egui::Pos2::new(x, rect.bottom()),
                        egui::Pos2::new(x, rect.bottom() - h),
                    ],
                    egui::Stroke::new(1.5, color),
                );
            }

            // Target line
            let target_y = rect.bottom() - (budget_ms / max_ms) * rect.height();
            ui.painter().line_segment(
                [egui::Pos2::new(rect.left(), target_y), egui::Pos2::new(rect.right(), target_y)],
                egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 80)),
            );
        }

        ui.add_space(8.0);

        // ---- Reprojection ----
        ui.label(RichText::new("Reprojection").size(13.0).color(muted));
        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Dropped Frames:");
            let color = if state.dropped_frames == 0 { green } else { red };
            ui.colored_label(color, format!("{}", state.dropped_frames));
        });

        ui.horizontal(|ui| {
            ui.label("Reprojection:");
            ui.label(format!("{}", state.reprojection_count));
            if state.total_frames > 0 {
                let pct = state.reprojection_count as f32 / state.total_frames as f32 * 100.0;
                ui.label(format!("({:.1}%)", pct));
            }
        });

        ui.add_space(8.0);

        // ---- Resolution ----
        ui.label(RichText::new("Resolution").size(13.0).color(muted));
        ui.separator();

        if state.resolution_per_eye[0] > 0 {
            ui.horizontal(|ui| {
                ui.label("Per Eye:");
                ui.label(format!("{}x{}", state.resolution_per_eye[0], state.resolution_per_eye[1]));
            });
        }

        ui.horizontal(|ui| {
            ui.label("Render Scale:");
            ui.label(format!("{:.0}%", state.render_scale * 100.0));
        });

        ui.horizontal(|ui| {
            ui.label("Foveation:");
            if state.foveation_active {
                ui.colored_label(green, "Active");
            } else {
                ui.label("Disabled");
            }
        });
    });
}
