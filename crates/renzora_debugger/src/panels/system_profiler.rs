//! System profiler panel — frame time breakdown and schedule timing

use bevy_egui::egui::{self, Color32, RichText, Vec2};
use renzora_theme::Theme;

use crate::state::{DiagnosticsState, SystemTimingState, RenderStats};

pub fn render_system_profiler_content(
    ui: &mut egui::Ui,
    diagnostics: &DiagnosticsState,
    timing_state: &SystemTimingState,
    render_stats: &RenderStats,
    theme: &Theme,
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());

                render_frame_time_section(ui, diagnostics, theme);
                ui.add_space(16.0);

                render_fps_stats(ui, diagnostics, theme);
                ui.add_space(16.0);

                render_render_stats_section(ui, render_stats, theme);
                ui.add_space(16.0);

                render_schedule_breakdown(ui, timing_state, theme);
                ui.add_space(16.0);

                render_limitations_section(ui, timing_state, theme);
                ui.add_space(16.0);

                render_profiler_links(ui, theme);
            });
        });
}

fn render_frame_time_section(ui: &mut egui::Ui, diagnostics: &DiagnosticsState, theme: &Theme) {
    ui.label(RichText::new("Frame Time").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

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

    // Frame time graph
    ui.add_space(8.0);
    render_graph(ui, &diagnostics.frame_time_history, theme, Color32::from_rgb(100, 180, 220));
}

fn render_fps_stats(ui: &mut egui::Ui, diagnostics: &DiagnosticsState, theme: &Theme) {
    ui.label(RichText::new("FPS Statistics").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{:.0}", diagnostics.fps))
                .size(24.0)
                .color(fps_to_color(diagnostics.fps as f32))
                .strong(),
        );
        ui.label(RichText::new("fps").size(11.0).color(theme.text.muted.to_color32()));
    });

    egui::Frame::NONE
        .fill(theme.surfaces.faint.to_color32())
        .corner_radius(4.0)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::Grid::new("fps_stats_grid")
                .num_columns(2)
                .spacing([12.0, 2.0])
                .show(ui, |ui| {
                    grid_row(ui, "Avg", &format!("{:.0}", diagnostics.avg_fps()), theme);
                    grid_row(ui, "Min", &format!("{:.0}", diagnostics.min_fps()), theme);
                    grid_row(ui, "Max", &format!("{:.0}", diagnostics.max_fps()), theme);
                    grid_row(ui, "Entities", &format!("{}", diagnostics.entity_count), theme);
                });
        });
}

fn render_render_stats_section(ui: &mut egui::Ui, stats: &RenderStats, theme: &Theme) {
    if !stats.enabled { return; }

    ui.label(RichText::new("Render Stats").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    egui::Frame::NONE
        .fill(theme.surfaces.faint.to_color32())
        .corner_radius(4.0)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::Grid::new("render_stats_grid")
                .num_columns(2)
                .spacing([12.0, 2.0])
                .show(ui, |ui| {
                    grid_row(ui, "Draw Calls", &format!("{}", stats.draw_calls), theme);
                    grid_row(ui, "Triangles", &format_count(stats.triangles), theme);
                    grid_row(ui, "Vertices", &format_count(stats.vertices), theme);
                    grid_row(ui, "GPU Time", &format!("{:.2}ms", stats.gpu_time_ms), theme);
                });
        });
}

fn render_schedule_breakdown(ui: &mut egui::Ui, timing_state: &SystemTimingState, theme: &Theme) {
    ui.label(RichText::new("Schedule Overview (Estimated)").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    if timing_state.schedule_timings.is_empty() {
        ui.label(RichText::new("No timing data available").size(11.0).color(theme.text.muted.to_color32()));
        return;
    }

    let total_time: f32 = timing_state.schedule_timings.iter().map(|s| s.time_ms).sum();

    egui::Frame::NONE
        .fill(theme.surfaces.faint.to_color32())
        .corner_radius(4.0)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            for schedule in &timing_state.schedule_timings {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&schedule.name).size(11.0).color(theme.text.primary.to_color32()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format!("{:.2}ms", schedule.time_ms)).size(10.0).color(theme.text.secondary.to_color32()).monospace());
                        ui.label(RichText::new(format!("{:.0}%", schedule.percentage)).size(10.0).color(theme.text.muted.to_color32()));
                    });
                });

                let bar_width = ui.available_width();
                let ratio = if total_time > 0.0 { schedule.time_ms / total_time } else { 0.0 };
                let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_width, 8.0), egui::Sense::hover());
                let painter = ui.painter();
                painter.rect_filled(rect, 2.0, theme.surfaces.extreme.to_color32());
                let fill_rect = egui::Rect::from_min_size(rect.min, Vec2::new(rect.width() * ratio, rect.height()));
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
                ui.label(RichText::new("Limitations").size(11.0).color(Color32::from_rgb(220, 180, 80)).strong());
            });
            ui.add_space(4.0);
            ui.label(RichText::new(&timing_state.limitation_note).size(10.0).color(theme.text.secondary.to_color32()));
        });
}

fn render_profiler_links(ui: &mut egui::Ui, theme: &Theme) {
    ui.label(RichText::new("External Profilers").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    egui::Frame::NONE
        .fill(theme.surfaces.faint.to_color32())
        .corner_radius(4.0)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS).size(12.0));
                ui.label(RichText::new("Tracy Profiler").size(11.0).color(theme.text.primary.to_color32()));
            });
            ui.label(RichText::new("Best for per-system timing and GPU profiling").size(9.0).color(theme.text.muted.to_color32()));
            ui.label(RichText::new("cargo run --features renzora_debugger/tracy").size(9.0).color(Color32::from_gray(100)).monospace());

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label(RichText::new(egui_phosphor::regular::CHART_BAR).size(12.0));
                ui.label(RichText::new("Chrome Tracing").size(11.0).color(theme.text.primary.to_color32()));
            });
            ui.label(RichText::new("Export traces to chrome://tracing").size(9.0).color(theme.text.muted.to_color32()));
            ui.label(RichText::new("cargo run --features bevy/trace_chrome").size(9.0).color(Color32::from_gray(100)).monospace());
        });
}

fn render_graph(ui: &mut egui::Ui, data: &std::collections::VecDeque<f32>, theme: &Theme, line_color: Color32) {
    let height = 40.0;
    let available_width = ui.available_width();
    let size = Vec2::new(available_width, height);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());

    if !ui.is_rect_visible(rect) { return; }

    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, theme.surfaces.extreme.to_color32());
    painter.rect_stroke(rect, 2.0, egui::Stroke::new(1.0, theme.widgets.border.to_color32()), egui::StrokeKind::Inside);

    let data: Vec<f32> = data.iter().copied().collect();
    if data.is_empty() { return; }

    let max_val = data.iter().copied().fold(1.0_f32, f32::max) * 1.2;
    if max_val <= 0.0 { return; }

    let step = rect.width() / data.len().max(1) as f32;
    let points: Vec<egui::Pos2> = data.iter().enumerate().map(|(i, &val)| {
        let x = rect.min.x + i as f32 * step;
        let normalized = (val / max_val).clamp(0.0, 1.0);
        let y = rect.max.y - normalized * rect.height();
        egui::pos2(x, y)
    }).collect();

    if points.len() >= 2 {
        let fill_color = Color32::from_rgba_unmultiplied(line_color.r(), line_color.g(), line_color.b(), 30);
        let mut fill_points = points.clone();
        fill_points.push(egui::pos2(rect.max.x, rect.max.y));
        fill_points.push(egui::pos2(rect.min.x, rect.max.y));
        painter.add(egui::Shape::convex_polygon(fill_points, fill_color, egui::Stroke::NONE));
        painter.add(egui::Shape::line(points, egui::Stroke::new(1.5, line_color)));
    }
}

fn grid_row(ui: &mut egui::Ui, label: &str, value: &str, theme: &Theme) {
    ui.label(RichText::new(label).size(10.0).color(theme.text.muted.to_color32()));
    ui.label(RichText::new(value).size(10.0).color(theme.text.primary.to_color32()).monospace());
    ui.end_row();
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
    if ms <= 16.67 { Color32::from_rgb(100, 200, 100) }
    else if ms <= 33.33 { Color32::from_rgb(200, 200, 100) }
    else { Color32::from_rgb(200, 100, 100) }
}

fn fps_to_color(fps: f32) -> Color32 {
    if fps >= 60.0 { Color32::from_rgb(100, 200, 100) }
    else if fps >= 30.0 { Color32::from_rgb(200, 200, 100) }
    else { Color32::from_rgb(200, 100, 100) }
}

fn format_count(n: u64) -> String {
    if n >= 1_000_000 { format!("{:.1}M", n as f64 / 1_000_000.0) }
    else if n >= 1_000 { format!("{:.1}K", n as f64 / 1_000.0) }
    else { format!("{}", n) }
}
