//! System profiler panel for schedule/system timing overview

use bevy_egui::egui::{self, Color32, RichText, Vec2};

use crate::core::{DiagnosticsState, SystemTimingState};
use crate::theming::Theme;

/// Render the system profiler panel content
pub fn render_system_profiler_content(
    ui: &mut egui::Ui,
    diagnostics: &DiagnosticsState,
    timing_state: &SystemTimingState,
    theme: &Theme,
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());

                // Frame Time Overview
                render_frame_time_section(ui, diagnostics, theme);

                ui.add_space(16.0);

                // Schedule Breakdown
                render_schedule_breakdown(ui, timing_state, theme);

                ui.add_space(16.0);

                // Limitations Note
                render_limitations_section(ui, timing_state, theme);

                ui.add_space(16.0);

                // External Profiler Links
                render_profiler_links(ui, theme);
            });
        });
}

fn render_frame_time_section(ui: &mut egui::Ui, diagnostics: &DiagnosticsState, theme: &Theme) {
    ui.label(RichText::new("Frame Time Breakdown").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    // Total frame time
    let frame_time = diagnostics.frame_time_ms as f32;
    let frame_color = frame_time_to_color(frame_time);

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{:.2}", frame_time))
                .size(24.0)
                .color(frame_color)
                .strong(),
        );
        ui.label(RichText::new("ms / frame").size(11.0).color(theme.text.muted.to_color32()));
    });

    ui.add_space(4.0);

    // Target comparison
    ui.horizontal(|ui| {
        let target_60fps = 16.67;
        let target_30fps = 33.33;

        if frame_time <= target_60fps {
            ui.label(
                RichText::new(format!("\u{2713} Under 60fps target ({:.1}ms budget)", target_60fps - frame_time))
                    .size(10.0)
                    .color(Color32::from_rgb(100, 200, 100)),
            );
        } else if frame_time <= target_30fps {
            ui.label(
                RichText::new(format!("\u{26a0} Between 30-60fps ({:.1}ms over 60fps target)", frame_time - target_60fps))
                    .size(10.0)
                    .color(Color32::from_rgb(200, 180, 80)),
            );
        } else {
            ui.label(
                RichText::new(format!("\u{2717} Below 30fps ({:.1}ms over target)", frame_time - target_30fps))
                    .size(10.0)
                    .color(Color32::from_rgb(200, 100, 100)),
            );
        }
    });
}

fn render_schedule_breakdown(ui: &mut egui::Ui, timing_state: &SystemTimingState, theme: &Theme) {
    ui.label(RichText::new("Schedule Overview (Estimated)").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    if timing_state.schedule_timings.is_empty() {
        ui.label(
            RichText::new("No timing data available")
                .size(11.0)
                .color(theme.text.muted.to_color32()),
        );
        return;
    }

    let total_time: f32 = timing_state.schedule_timings.iter().map(|s| s.time_ms).sum();

    egui::Frame::NONE
        .fill(Color32::from_rgb(35, 37, 42))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            for schedule in &timing_state.schedule_timings {
                ui.horizontal(|ui| {
                    // Schedule name
                    ui.label(
                        RichText::new(&schedule.name)
                            .size(11.0)
                            .color(theme.text.primary.to_color32()),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Time
                        ui.label(
                            RichText::new(format!("{:.2}ms", schedule.time_ms))
                                .size(10.0)
                                .color(theme.text.secondary.to_color32())
                                .monospace(),
                        );

                        // Percentage
                        ui.label(
                            RichText::new(format!("{:.0}%", schedule.percentage))
                                .size(10.0)
                                .color(theme.text.muted.to_color32()),
                        );
                    });
                });

                // Progress bar
                let bar_width = ui.available_width();
                let ratio = if total_time > 0.0 { schedule.time_ms / total_time } else { 0.0 };
                let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_width, 8.0), egui::Sense::hover());

                let painter = ui.painter();
                painter.rect_filled(rect, 2.0, Color32::from_rgb(50, 52, 58));

                let fill_rect = egui::Rect::from_min_size(
                    rect.min,
                    Vec2::new(rect.width() * ratio, rect.height()),
                );
                painter.rect_filled(fill_rect, 2.0, schedule_color(&schedule.name));

                ui.add_space(6.0);
            }
        });

    ui.add_space(4.0);
    ui.label(
        RichText::new("Note: These are rough estimates, not actual measurements")
            .size(9.0)
            .color(theme.text.muted.to_color32())
            .italics(),
    );
}

fn render_limitations_section(ui: &mut egui::Ui, timing_state: &SystemTimingState, theme: &Theme) {
    egui::Frame::NONE
        .fill(Color32::from_rgb(50, 45, 35))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("\u{26a0}").size(14.0).color(Color32::from_rgb(220, 180, 80)));
                ui.label(
                    RichText::new("Limitations")
                        .size(11.0)
                        .color(Color32::from_rgb(220, 180, 80))
                        .strong(),
                );
            });

            ui.add_space(4.0);

            ui.label(
                RichText::new(&timing_state.limitation_note)
                    .size(10.0)
                    .color(theme.text.secondary.to_color32()),
            );
        });
}

fn render_profiler_links(ui: &mut egui::Ui, theme: &Theme) {
    ui.label(RichText::new("External Profilers").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    egui::Frame::NONE
        .fill(Color32::from_rgb(35, 37, 42))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            // Tracy
            ui.horizontal(|ui| {
                ui.label(RichText::new("\u{1f50d}").size(12.0));
                ui.label(RichText::new("Tracy Profiler").size(11.0).color(theme.text.primary.to_color32()));
            });
            ui.label(
                RichText::new("Best for per-system timing and GPU profiling")
                    .size(9.0)
                    .color(theme.text.muted.to_color32()),
            );
            ui.label(
                RichText::new("cargo run --features bevy/trace_tracy")
                    .size(9.0)
                    .color(Color32::from_gray(100))
                    .monospace(),
            );

            ui.add_space(8.0);

            // Chrome tracing
            ui.horizontal(|ui| {
                ui.label(RichText::new("\u{1f4ca}").size(12.0));
                ui.label(RichText::new("Chrome Tracing").size(11.0).color(theme.text.primary.to_color32()));
            });
            ui.label(
                RichText::new("Export traces to chrome://tracing")
                    .size(9.0)
                    .color(theme.text.muted.to_color32()),
            );
            ui.label(
                RichText::new("cargo run --features bevy/trace_chrome")
                    .size(9.0)
                    .color(Color32::from_gray(100))
                    .monospace(),
            );

            ui.add_space(8.0);

            // Documentation link
            ui.horizontal(|ui| {
                ui.label(RichText::new("\u{1f4d6}").size(12.0));
                if ui.link(RichText::new("Bevy Profiling Guide").size(10.0)).clicked() {
                    // In a real app, we'd open the URL
                }
            });
            ui.label(
                RichText::new("github.com/bevyengine/bevy/blob/main/docs/profiling.md")
                    .size(9.0)
                    .color(theme.text.muted.to_color32()),
            );
        });
}

fn schedule_color(name: &str) -> Color32 {
    match name {
        "PreUpdate" => Color32::from_rgb(100, 180, 220),
        "Update" => Color32::from_rgb(140, 200, 140),
        "PostUpdate" => Color32::from_rgb(200, 160, 100),
        "Render" => Color32::from_rgb(180, 140, 200),
        _ => Color32::from_rgb(150, 150, 160),
    }
}

fn frame_time_to_color(ms: f32) -> Color32 {
    if ms <= 16.67 {
        Color32::from_rgb(100, 200, 100) // Green (60+ fps)
    } else if ms <= 33.33 {
        Color32::from_rgb(200, 200, 100) // Yellow (30-60 fps)
    } else {
        Color32::from_rgb(200, 100, 100) // Red (<30 fps)
    }
}
