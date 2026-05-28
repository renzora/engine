//! Material Resolver diagnostics panel content.
//!
//! Pulls together two views:
//!
//!   - `MaterialCache` — how many compiled materials of each kind are
//!     currently resident.
//!   - `MaterialPerfStats` — per-path compile timing, cache hit rate,
//!     and a short list of recent compile failures. Failures and slow
//!     compiles are what you watch when something renders blank or
//!     scenes load slowly.

use std::time::Duration;

use bevy_egui::egui::{self, Color32, RichText};
use renzora_shader::material::perf::{MaterialPerf, MaterialPerfStats};
use renzora_shader::material::resolver::MaterialCache;
use renzora_theme::Theme;

pub fn render_material_resolver_content(
    ui: &mut egui::Ui,
    cache: &MaterialCache,
    perf: &MaterialPerfStats,
    theme: &Theme,
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());

                render_header(ui, cache, perf, theme);
                ui.add_space(10.0);
                render_perf_table(ui, perf, theme);

                if !perf.recent_failures.is_empty() {
                    ui.add_space(10.0);
                    render_failures(ui, perf, theme);
                }
            });
        });
}

fn render_header(
    ui: &mut egui::Ui,
    cache: &MaterialCache,
    perf: &MaterialPerfStats,
    theme: &Theme,
) {
    let primary = theme.text.primary.to_color32();
    let muted = theme.text.muted.to_color32();
    let secondary = theme.text.secondary.to_color32();

    let total_cached =
        cache.standard_count() + cache.graph_count() + cache.code_count();

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{total_cached}"))
                .size(28.0)
                .color(primary)
                .strong(),
        );
        ui.label(
            RichText::new("materials cached")
                .size(11.0)
                .color(muted),
        );
    });
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!(
                "{} std · {} graph · {} code  ·  {} master-meta entries",
                cache.standard_count(),
                cache.graph_count(),
                cache.code_count(),
                cache.master_meta_count(),
            ))
            .size(11.0)
            .color(secondary),
        );
    });

    ui.add_space(6.0);
    stat(ui, theme, "Cache hits (lifetime)", &format_count(perf.total_cache_hits));
    stat(ui, theme, "Compiles ran", &format_count(perf.total_compiles));
    stat(
        ui,
        theme,
        "Total compile time",
        &format_duration(perf.total_compile_time),
    );
    if perf.total_failures > 0 {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "⚠ {} compile failure{}",
                    perf.total_failures,
                    if perf.total_failures == 1 { "" } else { "s" },
                ))
                .size(11.0)
                .color(Color32::from_rgb(230, 110, 110))
                .strong(),
            );
        });
    }
}

fn render_perf_table(ui: &mut egui::Ui, perf: &MaterialPerfStats, theme: &Theme) {
    let muted = theme.text.muted.to_color32();
    let primary = theme.text.primary.to_color32();
    let secondary = theme.text.secondary.to_color32();
    let stroke = egui::Stroke::new(1.0, theme.widgets.border.to_color32());

    ui.label(
        RichText::new("Per-material compile timing (sorted by last compile)")
            .size(12.0)
            .color(muted),
    );
    ui.add_space(4.0);

    let snapshot = perf.snapshot();
    if snapshot.is_empty() {
        ui.label(
            RichText::new("(no materials resolved yet)")
                .size(11.0)
                .color(muted)
                .italics(),
        );
        return;
    }

    ui.horizontal(|ui| {
        ui.add_space(4.0);
        column(ui, "Material", 200.0, muted);
        column(ui, "Kind", 40.0, muted);
        column(ui, "Last", 60.0, muted);
        column(ui, "Max", 60.0, muted);
        column(ui, "Compiles", 60.0, muted);
        column(ui, "Hits", 50.0, muted);
    });
    let (_, sep) = ui.allocate_space(egui::vec2(ui.available_width(), 1.0));
    ui.painter().line_segment([sep.left_top(), sep.right_top()], stroke);
    ui.add_space(2.0);

    for (path, p) in &snapshot {
        let name_color = if p.fail_count > 0 {
            Color32::from_rgb(230, 130, 110)
        } else {
            primary
        };
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            let short = short_path(path);
            column(ui, &short, 200.0, name_color);
            column(ui, p.kind.label(), 40.0, secondary);
            column(
                ui,
                &format_duration(p.last_compile),
                60.0,
                duration_color(p.last_compile, secondary),
            );
            column(
                ui,
                &format_duration(p.max_compile),
                60.0,
                secondary,
            );
            column(ui, &format_count(p.compile_count), 60.0, secondary);
            column(ui, &format_count(p.cache_hits), 50.0, secondary);
        });
        if let Some(err) = &p.last_error {
            ui.horizontal(|ui| {
                ui.add_space(18.0);
                ui.label(
                    RichText::new(format!("✗ {}", truncate(err, 200)))
                        .size(10.0)
                        .color(Color32::from_rgb(220, 80, 80))
                        .monospace(),
                );
            });
        }
        ui.add_space(2.0);
    }
}

fn render_failures(ui: &mut egui::Ui, perf: &MaterialPerfStats, theme: &Theme) {
    let muted = theme.text.muted.to_color32();
    ui.label(
        RichText::new(format!(
            "Recent failures ({} of {} kept)",
            perf.recent_failures.len(),
            renzora_shader::material::perf::MAX_RECENT_FAILURES,
        ))
        .size(12.0)
        .color(muted),
    );
    ui.add_space(4.0);
    for (path, err) in &perf.recent_failures {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(
                RichText::new(short_path(path))
                    .strong()
                    .size(11.0)
                    .color(Color32::from_rgb(220, 80, 80)),
            );
        });
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(
                RichText::new(truncate(err, 200))
                    .size(10.0)
                    .color(theme.text.secondary.to_color32())
                    .monospace(),
            );
        });
        ui.add_space(2.0);
    }
}

fn duration_color(d: Duration, base: Color32) -> Color32 {
    let ms = d.as_micros() as f64 / 1_000.0;
    if ms >= 100.0 {
        Color32::from_rgb(230, 110, 110)
    } else if ms >= 20.0 {
        Color32::from_rgb(230, 180, 80)
    } else {
        base
    }
}

fn format_duration(d: Duration) -> String {
    let us = d.as_micros();
    if us == 0 {
        "—".to_string()
    } else if us < 1_000 {
        format!("{} µs", us)
    } else if us < 1_000_000 {
        format!("{:.2} ms", us as f64 / 1_000.0)
    } else {
        format!("{:.2} s", us as f64 / 1_000_000.0)
    }
}

fn format_count(n: u64) -> String {
    if n < 1_000 {
        n.to_string()
    } else if n < 1_000_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max).collect();
        out.push('…');
        out
    }
}

fn short_path(path: &str) -> String {
    // Keep just the file name + immediate parent dir to fit the column.
    let p = std::path::Path::new(path);
    if let (Some(parent), Some(name)) = (
        p.parent().and_then(|p| p.file_name()),
        p.file_name(),
    ) {
        format!(
            "{}/{}",
            parent.to_string_lossy(),
            name.to_string_lossy()
        )
    } else {
        path.to_string()
    }
}

fn column(ui: &mut egui::Ui, text: &str, width: f32, color: Color32) {
    ui.allocate_ui_with_layout(
        egui::vec2(width, 14.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.label(RichText::new(text).size(11.0).color(color).monospace());
        },
    );
}

fn stat(ui: &mut egui::Ui, theme: &Theme, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .size(11.0)
                .color(theme.text.secondary.to_color32()),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(value)
                    .monospace()
                    .color(theme.text.primary.to_color32()),
            );
        });
    });
}

#[allow(dead_code)]
fn unused(_: MaterialPerf) {}
