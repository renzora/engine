//! Memory profiler panel for monitoring memory usage

use bevy_egui::egui::{self, Color32, RichText, Stroke, Vec2};

use crate::core::{MemoryProfilerState, MemoryTrend};
use crate::theming::Theme;

/// Render the memory profiler panel content
pub fn render_memory_profiler_content(
    ui: &mut egui::Ui,
    state: &MemoryProfilerState,
    theme: &Theme,
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());

                if !state.available {
                    render_unavailable(ui, theme);
                    return;
                }

                // Process Memory Section
                render_process_memory_section(ui, state, theme);

                ui.add_space(16.0);

                // Memory Trend
                render_memory_trend_section(ui, state, theme);

                ui.add_space(16.0);

                // Asset Memory Breakdown
                render_asset_memory_section(ui, state, theme);

                ui.add_space(16.0);

                // Allocation Rate
                render_allocation_rate_section(ui, state, theme);
            });
        });
}

fn render_unavailable(ui: &mut egui::Ui, theme: &Theme) {
    ui.vertical_centered(|ui| {
        ui.add_space(40.0);
        ui.label(
            RichText::new("Memory profiling unavailable")
                .size(14.0)
                .color(theme.text.muted.to_color32()),
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new("Enable 'memory-profiling' feature")
                .size(12.0)
                .color(theme.text.muted.to_color32()),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new("cargo build --features memory-profiling")
                .size(10.0)
                .color(Color32::from_gray(100))
                .monospace(),
        );
    });
}

fn render_process_memory_section(ui: &mut egui::Ui, state: &MemoryProfilerState, theme: &Theme) {
    ui.label(RichText::new("Process Memory").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    // Current memory with color coding
    let memory_mb = state.process_memory as f64 / (1024.0 * 1024.0);
    let memory_color = memory_to_color(memory_mb);

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format_memory(state.process_memory))
                .size(28.0)
                .color(memory_color)
                .strong(),
        );
    });

    ui.add_space(4.0);

    // Peak memory
    ui.horizontal(|ui| {
        ui.label(RichText::new("Peak:").size(11.0).color(theme.text.secondary.to_color32()));
        ui.label(
            RichText::new(format_memory(state.peak_memory))
                .size(11.0)
                .color(theme.text.primary.to_color32()),
        );
    });

    ui.add_space(8.0);

    // Memory graph
    render_memory_graph(ui, state);
}

fn render_memory_trend_section(ui: &mut egui::Ui, state: &MemoryProfilerState, theme: &Theme) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Trend:").size(11.0).color(theme.text.secondary.to_color32()));

        let (icon, color, text) = match state.memory_trend {
            MemoryTrend::Increasing => ("\u{f062}", Color32::from_rgb(220, 100, 100), "Increasing"),
            MemoryTrend::Decreasing => ("\u{f063}", Color32::from_rgb(100, 200, 100), "Decreasing"),
            MemoryTrend::Stable => ("\u{f068}", Color32::from_gray(150), "Stable"),
        };

        ui.label(RichText::new(icon).size(11.0).color(color));
        ui.label(RichText::new(text).size(11.0).color(color));
    });
}

fn render_asset_memory_section(ui: &mut egui::Ui, state: &MemoryProfilerState, theme: &Theme) {
    ui.label(RichText::new("Asset Memory (Estimated)").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    let assets = &state.asset_memory;
    let total = assets.meshes_bytes + assets.textures_bytes + assets.materials_bytes;

    // Total
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format_memory(total))
                .size(20.0)
                .color(theme.text.primary.to_color32())
                .strong(),
        );
        ui.label(RichText::new("total").size(11.0).color(theme.text.muted.to_color32()));
    });

    ui.add_space(8.0);

    // Breakdown bars
    let max_bytes = assets.meshes_bytes.max(assets.textures_bytes).max(assets.materials_bytes).max(1);

    render_asset_bar(ui, "Meshes", assets.mesh_count, assets.meshes_bytes, max_bytes, Color32::from_rgb(100, 180, 220), theme);
    ui.add_space(4.0);
    render_asset_bar(ui, "Textures", assets.texture_count, assets.textures_bytes, max_bytes, Color32::from_rgb(180, 140, 200), theme);
    ui.add_space(4.0);
    render_asset_bar(ui, "Materials", assets.material_count, assets.materials_bytes, max_bytes, Color32::from_rgb(200, 160, 100), theme);
}

fn render_asset_bar(
    ui: &mut egui::Ui,
    name: &str,
    count: usize,
    bytes: u64,
    max_bytes: u64,
    color: Color32,
    theme: &Theme,
) {
    ui.horizontal(|ui| {
        // Name and count
        ui.label(
            RichText::new(format!("{} ({})", name, count))
                .size(10.0)
                .color(theme.text.secondary.to_color32()),
        );
    });

    ui.horizontal(|ui| {
        // Progress bar
        let bar_width = 120.0;
        let fill_ratio = if max_bytes > 0 { bytes as f32 / max_bytes as f32 } else { 0.0 };

        let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_width, 12.0), egui::Sense::hover());

        // Background
        ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(40, 42, 48));

        // Fill
        let fill_rect = egui::Rect::from_min_size(
            rect.min,
            Vec2::new(rect.width() * fill_ratio, rect.height()),
        );
        ui.painter().rect_filled(fill_rect, 2.0, color);

        ui.add_space(8.0);

        // Size label
        ui.label(
            RichText::new(format_memory(bytes))
                .size(10.0)
                .color(theme.text.primary.to_color32()),
        );
    });
}

fn render_allocation_rate_section(ui: &mut egui::Ui, state: &MemoryProfilerState, theme: &Theme) {
    ui.label(RichText::new("Allocation Rate").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    let rate = state.allocation_rate;
    let (rate_str, color) = if rate.abs() < 1024.0 {
        (format!("{:.0} B/s", rate), Color32::from_gray(150))
    } else if rate.abs() < 1024.0 * 1024.0 {
        (format!("{:.1} KB/s", rate / 1024.0), if rate > 0.0 { Color32::from_rgb(200, 180, 100) } else { Color32::from_rgb(100, 200, 100) })
    } else {
        (format!("{:.2} MB/s", rate / (1024.0 * 1024.0)), if rate > 0.0 { Color32::from_rgb(220, 100, 100) } else { Color32::from_rgb(100, 200, 100) })
    };

    ui.horizontal(|ui| {
        if rate > 0.0 {
            ui.label(RichText::new("+").size(14.0).color(color));
        } else if rate < 0.0 {
            ui.label(RichText::new("-").size(14.0).color(color));
        }
        ui.label(RichText::new(rate_str).size(14.0).color(color));
    });

    // Warning for high allocation
    if rate > 10.0 * 1024.0 * 1024.0 {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("\u{26a0}").size(12.0).color(Color32::from_rgb(220, 180, 80)));
            ui.label(
                RichText::new("High allocation rate detected")
                    .size(10.0)
                    .color(Color32::from_rgb(220, 180, 80)),
            );
        });
    }
}

fn render_memory_graph(ui: &mut egui::Ui, state: &MemoryProfilerState) {
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
    painter.rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_rgb(50, 52, 58)), egui::StrokeKind::Inside);

    let data: Vec<f32> = state.memory_history.iter().copied().collect();
    if data.is_empty() {
        return;
    }

    let max_val = data.iter().copied().fold(100.0_f32, f32::max) * 1.1;
    let min_val = data.iter().copied().fold(f32::MAX, f32::min) * 0.9;
    let range = max_val - min_val;
    if range <= 0.0 {
        return;
    }

    // Data line
    let step = rect.width() / data.len().max(1) as f32;
    let line_color = Color32::from_rgb(150, 100, 200);

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
        // Filled area
        let mut fill_points = points.clone();
        fill_points.push(egui::pos2(rect.max.x, rect.max.y));
        fill_points.push(egui::pos2(rect.min.x, rect.max.y));
        let fill_color = Color32::from_rgba_unmultiplied(150, 100, 200, 30);
        painter.add(egui::Shape::convex_polygon(fill_points, fill_color, Stroke::NONE));

        // Line
        painter.add(egui::Shape::line(points, Stroke::new(1.5, line_color)));
    }

    // Current value marker
    if let Some(&last_val) = data.last() {
        let normalized = ((last_val - min_val) / range).clamp(0.0, 1.0);
        let y = rect.max.y - normalized * rect.height();
        painter.circle_filled(egui::pos2(rect.max.x - 2.0, y), 3.0, line_color);
    }
}

fn format_memory(bytes: u64) -> String {
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

fn memory_to_color(mb: f64) -> Color32 {
    if mb < 512.0 {
        Color32::from_rgb(100, 200, 100) // Green - under 512MB
    } else if mb < 1024.0 {
        Color32::from_rgb(200, 200, 100) // Yellow - under 1GB
    } else if mb < 2048.0 {
        Color32::from_rgb(200, 150, 80) // Orange - under 2GB
    } else {
        Color32::from_rgb(200, 100, 100) // Red - over 2GB
    }
}
