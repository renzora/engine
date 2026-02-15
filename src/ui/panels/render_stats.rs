//! Render statistics panel for GPU/CPU profiling

use bevy_egui::egui::{self, Color32, RichText, Stroke, Vec2};

use crate::core::resources::diagnostics::RenderStats;
use renzora_theme::Theme;

/// Render the render statistics panel content
pub fn render_render_stats_content(
    ui: &mut egui::Ui,
    render_stats: &RenderStats,
    theme: &Theme,
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());

                if !render_stats.enabled {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(RichText::new("Render diagnostics disabled").size(14.0).color(theme.text.muted.to_color32()));
                        ui.add_space(8.0);
                        ui.label(RichText::new("Enable RenderDiagnosticsPlugin").size(12.0).color(theme.text.disabled.to_color32()));
                    });
                    return;
                }

                // GPU Time Section
                render_gpu_section(ui, render_stats, theme);

                ui.add_space(16.0);

                // Pipeline Statistics Section
                render_pipeline_stats_section(ui, render_stats, theme);

                ui.add_space(16.0);

                // Render Passes Section
                render_passes_section(ui, render_stats, theme);
            });
        });
}

fn render_gpu_section(ui: &mut egui::Ui, stats: &RenderStats, theme: &Theme) {
    ui.label(RichText::new("GPU Timing").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    // Total GPU time
    let gpu_time_color = gpu_time_to_color(stats.gpu_time_ms);
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{:.2}", stats.gpu_time_ms)).size(28.0).color(gpu_time_color).strong());
        ui.label(RichText::new("ms GPU").size(12.0).color(theme.text.muted.to_color32()));
    });

    ui.add_space(4.0);

    // CPU render time
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{:.2}", stats.cpu_render_time_ms)).size(16.0).color(theme.text.primary.to_color32()));
        ui.label(RichText::new("ms CPU (render)").size(11.0).color(theme.text.muted.to_color32()));
    });

    ui.add_space(8.0);

    // GPU time graph
    render_time_graph(
        ui,
        &stats.gpu_time_history,
        0.0,
        20.0,
        16.67,
        Color32::from_rgb(150, 100, 200),
        theme,
    );
}

fn render_pipeline_stats_section(ui: &mut egui::Ui, stats: &RenderStats, theme: &Theme) {
    ui.label(RichText::new("Pipeline Statistics").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    // Stats grid
    egui::Grid::new("pipeline_stats_grid")
        .num_columns(2)
        .spacing([20.0, 4.0])
        .show(ui, |ui| {
            stat_row(ui, "Draw Calls", stats.draw_calls, theme);
            stat_row(ui, "Triangles", stats.triangles, theme);
            stat_row(ui, "Vertices", stats.vertices, theme);
            stat_row(ui, "Vertex Invocations", stats.vertex_shader_invocations, theme);
            stat_row(ui, "Fragment Invocations", stats.fragment_shader_invocations, theme);
            stat_row(ui, "Compute Invocations", stats.compute_shader_invocations, theme);
        });
}

fn render_passes_section(ui: &mut egui::Ui, stats: &RenderStats, theme: &Theme) {
    if stats.render_passes.is_empty() {
        return;
    }

    ui.label(RichText::new("Render Passes").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    let max_time = stats.render_passes.iter().map(|p| p.gpu_time_ms).fold(0.0_f32, f32::max);

    for pass in &stats.render_passes {
        ui.horizontal(|ui| {
            // Pass name
            ui.label(RichText::new(&pass.name).size(10.0).color(theme.text.secondary.to_color32()));

            // Time bar
            let bar_width = if max_time > 0.0 {
                (pass.gpu_time_ms / max_time * 100.0).min(100.0)
            } else {
                0.0
            };

            let bar_color = gpu_time_to_color(pass.gpu_time_ms);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(100.0, 12.0), egui::Sense::hover());

            // Background
            ui.painter().rect_filled(rect, 2.0, theme.surfaces.extreme.to_color32());

            // Fill
            let fill_rect = egui::Rect::from_min_size(
                rect.min,
                Vec2::new(rect.width() * bar_width / 100.0, rect.height()),
            );
            ui.painter().rect_filled(fill_rect, 2.0, bar_color);

            // Time label
            ui.label(RichText::new(format!("{:.2}ms", pass.gpu_time_ms)).size(10.0).color(theme.text.muted.to_color32()));
        });
    }
}

fn stat_row(ui: &mut egui::Ui, label: &str, value: u64, theme: &Theme) {
    ui.label(RichText::new(label).size(11.0).color(theme.text.secondary.to_color32()));
    ui.label(RichText::new(format_number(value)).size(11.0).color(theme.text.primary.to_color32()));
    ui.end_row();
}

fn format_number(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}

fn gpu_time_to_color(ms: f32) -> Color32 {
    if ms <= 8.0 {
        Color32::from_rgb(100, 200, 100) // Green (120+ fps budget)
    } else if ms <= 16.67 {
        Color32::from_rgb(200, 200, 100) // Yellow (60 fps budget)
    } else {
        Color32::from_rgb(200, 100, 100) // Red (below 60 fps)
    }
}

fn render_time_graph(
    ui: &mut egui::Ui,
    data: &[f32],
    min_val: f32,
    max_val: f32,
    target_line: f32,
    line_color: Color32,
    theme: &Theme,
) {
    let height = 40.0;
    let available_width = ui.available_width();
    let size = Vec2::new(available_width, height);
    let (rect, _response) = ui.allocate_exact_size(size, egui::Sense::hover());

    if !ui.is_rect_visible(rect) {
        return;
    }

    let painter = ui.painter();

    // Background
    painter.rect_filled(rect, 2.0, theme.surfaces.extreme.to_color32());
    painter.rect_stroke(rect, 2.0, Stroke::new(1.0, theme.widgets.border.to_color32()), egui::StrokeKind::Inside);

    if data.is_empty() {
        return;
    }

    let range = max_val - min_val;
    if range <= 0.0 {
        return;
    }

    // Target line
    if target_line > min_val && target_line < max_val {
        let y = rect.max.y - ((target_line - min_val) / range * rect.height());
        painter.line_segment(
            [egui::pos2(rect.min.x, y), egui::pos2(rect.max.x, y)],
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(100, 100, 100, 100)),
        );
    }

    // Data line
    let step = rect.width() / data.len().max(1) as f32;
    let points: Vec<egui::Pos2> = data
        .iter()
        .enumerate()
        .map(|(i, &val)| {
            let x = rect.min.x + i as f32 * step;
            let normalized = ((val - min_val) / range).clamp(0.0, 1.0);
            let y = rect.max.y - normalized * rect.height();
            egui::pos2(x, y)
        })
        .collect();

    if points.len() >= 2 {
        let fill_color = Color32::from_rgba_unmultiplied(line_color.r(), line_color.g(), line_color.b(), 30);
        let mut fill_points = points.clone();
        fill_points.push(egui::pos2(rect.max.x, rect.max.y));
        fill_points.push(egui::pos2(rect.min.x, rect.max.y));
        painter.add(egui::Shape::convex_polygon(fill_points, fill_color, Stroke::NONE));
        painter.add(egui::Shape::line(points, Stroke::new(1.5, line_color)));
    }
}
