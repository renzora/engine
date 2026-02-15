//! Performance diagnostics panel for debugging and profiling

use bevy_egui::egui::{self, Color32, RichText, Stroke, Vec2};

use crate::core::DiagnosticsState;
use renzora_theme::Theme;

/// Render the performance diagnostics panel content
pub fn render_performance_content(
    ui: &mut egui::Ui,
    diagnostics: &DiagnosticsState,
    theme: &Theme,
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());

                // FPS Section
                render_fps_section(ui, diagnostics, theme);

                ui.add_space(16.0);

                // Frame Time Section
                render_frame_time_section(ui, diagnostics, theme);

                ui.add_space(16.0);

                // Entity Count Section
                render_entity_section(ui, diagnostics, theme);

                ui.add_space(16.0);

                // System Info Section (if available)
                render_system_info_section(ui, diagnostics, theme);
            });
        });
}

fn render_fps_section(ui: &mut egui::Ui, diagnostics: &DiagnosticsState, theme: &Theme) {
    ui.label(RichText::new("Frames Per Second").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    // Current FPS with color coding
    let fps = diagnostics.fps as f32;
    let fps_color = fps_to_color(fps);

    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{:.0}", fps)).size(28.0).color(fps_color).strong());
        ui.label(RichText::new("FPS").size(12.0).color(theme.text.muted.to_color32()));
    });

    ui.add_space(4.0);

    // Stats row
    ui.horizontal(|ui| {
        stat_label(ui, "Avg", diagnostics.avg_fps(), theme);
        ui.add_space(12.0);
        stat_label(ui, "Min", diagnostics.min_fps(), theme);
        ui.add_space(12.0);
        stat_label(ui, "Max", diagnostics.max_fps(), theme);
        ui.add_space(12.0);
        stat_label(ui, "1% Low", diagnostics.one_percent_low_fps(), theme);
    });

    ui.add_space(8.0);

    // FPS Graph
    render_graph(
        ui,
        &diagnostics.fps_history.iter().copied().collect::<Vec<_>>(),
        0.0,
        120.0,
        60.0,
        Color32::from_rgb(100, 200, 100),
        theme,
    );
}

fn render_frame_time_section(ui: &mut egui::Ui, diagnostics: &DiagnosticsState, theme: &Theme) {
    ui.label(RichText::new("Frame Time").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    let frame_time = diagnostics.frame_time_ms as f32;
    let ft_color = frame_time_to_color(frame_time);

    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{:.2}", frame_time)).size(28.0).color(ft_color).strong());
        ui.label(RichText::new("ms").size(12.0).color(theme.text.muted.to_color32()));
    });

    ui.add_space(4.0);

    ui.horizontal(|ui| {
        stat_label_ms(ui, "Avg", diagnostics.avg_frame_time(), theme);
    });

    ui.add_space(8.0);

    // Frame time graph (inverted colors - lower is better)
    render_graph(
        ui,
        &diagnostics.frame_time_history.iter().copied().collect::<Vec<_>>(),
        0.0,
        33.33, // Cap at ~30fps equivalent
        16.67, // Target line at 60fps
        Color32::from_rgb(100, 150, 200),
        theme,
    );
}

fn render_entity_section(ui: &mut egui::Ui, diagnostics: &DiagnosticsState, theme: &Theme) {
    ui.label(RichText::new("Entities").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{}", diagnostics.entity_count)).size(28.0).color(theme.text.primary.to_color32()).strong());
        ui.label(RichText::new("entities").size(12.0).color(theme.text.muted.to_color32()));
    });

    ui.add_space(8.0);

    // Entity count graph
    let max_entities = diagnostics.entity_count_history.iter().copied().fold(100.0_f32, f32::max);
    render_graph(
        ui,
        &diagnostics.entity_count_history.iter().copied().collect::<Vec<_>>(),
        0.0,
        max_entities * 1.2,
        0.0,
        Color32::from_rgb(200, 150, 100),
        theme,
    );
}

fn render_system_info_section(ui: &mut egui::Ui, diagnostics: &DiagnosticsState, theme: &Theme) {
    let has_info = diagnostics.cpu_usage.is_some() || diagnostics.memory_usage.is_some();

    if !has_info {
        return;
    }

    ui.label(RichText::new("System").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    if let Some(cpu) = diagnostics.cpu_usage {
        ui.horizontal(|ui| {
            ui.label(RichText::new("CPU:").size(11.0).color(theme.text.secondary.to_color32()));
            ui.label(RichText::new(format!("{:.1}%", cpu)).size(11.0).color(theme.text.primary.to_color32()));
        });
    }

    if let Some(mem) = diagnostics.memory_usage {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Memory:").size(11.0).color(theme.text.secondary.to_color32()));
            ui.label(RichText::new(format_bytes(mem)).size(11.0).color(theme.text.primary.to_color32()));
        });
    }
}

fn stat_label(ui: &mut egui::Ui, label: &str, value: f32, theme: &Theme) {
    ui.label(RichText::new(format!("{}: {:.0}", label, value)).size(10.0).color(theme.text.secondary.to_color32()));
}

fn stat_label_ms(ui: &mut egui::Ui, label: &str, value: f32, theme: &Theme) {
    ui.label(RichText::new(format!("{}: {:.2}ms", label, value)).size(10.0).color(theme.text.secondary.to_color32()));
}

fn fps_to_color(fps: f32) -> Color32 {
    if fps >= 60.0 {
        Color32::from_rgb(100, 200, 100) // Green
    } else if fps >= 30.0 {
        Color32::from_rgb(200, 200, 100) // Yellow
    } else {
        Color32::from_rgb(200, 100, 100) // Red
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

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn render_graph(
    ui: &mut egui::Ui,
    data: &[f32],
    min_val: f32,
    max_val: f32,
    target_line: f32,
    line_color: Color32,
    _theme: &Theme,
) {
    let height = 50.0;
    let available_width = ui.available_width();
    let size = Vec2::new(available_width, height);
    let (rect, _response) = ui.allocate_exact_size(size, egui::Sense::hover());

    if !ui.is_rect_visible(rect) {
        return;
    }

    let painter = ui.painter();

    // Background
    painter.rect_filled(rect, 2.0, Color32::from_rgb(30, 32, 36));

    // Border
    painter.rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_rgb(50, 52, 58)), egui::StrokeKind::Inside);

    if data.is_empty() {
        return;
    }

    let range = max_val - min_val;
    if range <= 0.0 {
        return;
    }

    // Target line (e.g., 60 FPS line)
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
        // Draw filled area under the line
        let mut fill_points = points.clone();
        fill_points.push(egui::pos2(rect.max.x, rect.max.y));
        fill_points.push(egui::pos2(rect.min.x, rect.max.y));

        let fill_color = Color32::from_rgba_unmultiplied(line_color.r(), line_color.g(), line_color.b(), 30);
        painter.add(egui::Shape::convex_polygon(fill_points, fill_color, Stroke::NONE));

        // Draw line
        painter.add(egui::Shape::line(points, Stroke::new(1.5, line_color)));
    }

    // Current value marker (rightmost point)
    if let Some(&last_val) = data.last() {
        let normalized = ((last_val - min_val) / range).clamp(0.0, 1.0);
        let y = rect.max.y - normalized * rect.height();
        let x = rect.max.x - 2.0;
        painter.circle_filled(egui::pos2(x, y), 3.0, line_color);
    }
}
