//! Scripting diagnostics panel content.
//!
//! Surfaces per-script execution timing collected by `renzora_scripting`'s
//! `ScriptPerfStats` resource. Each row is one script (identified by
//! its filesystem path) showing last/avg/max on_update durations, call
//! count, and the most recent error if any. Rows are sorted by last
//! on_update descending so the most expensive scripts surface at the
//! top.
//!
//! The totals header gives a per-frame budget view: sum of all
//! scripts' last on_update + a count of how many threw this session.

use std::path::PathBuf;
use std::time::Duration;

use bevy_egui::egui::{self, Color32, RichText};
use renzora_scripting::perf::{ScriptPerf, ScriptPerfTotals};
use renzora_theme::Theme;

/// Per-frame snapshot the panel renders from. Updated by
/// `update_scripting_diag_state` in the debugger plugin.
#[derive(bevy::prelude::Resource, Default, Clone)]
pub struct ScriptingDiagState {
    pub entities_with_script: usize,
    pub total_script_attachments: usize,
    pub backend_count: usize,
    pub scripts_folder: Option<String>,
    pub totals: ScriptPerfTotals,
    /// `(path, perf)` sorted by last on_update descending.
    pub per_script: Vec<(PathBuf, ScriptPerf)>,
    pub current_frame: u64,
}

pub fn render_scripting_content(ui: &mut egui::Ui, state: &ScriptingDiagState, theme: &Theme) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());

                render_header(ui, state, theme);
                ui.add_space(8.0);
                render_table(ui, state, theme);
            });
        });
}

fn render_header(ui: &mut egui::Ui, state: &ScriptingDiagState, theme: &Theme) {
    let primary = theme.text.primary.to_color32();
    let muted = theme.text.muted.to_color32();
    let secondary = theme.text.secondary.to_color32();

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format_duration(state.totals.total_last_update))
                .size(24.0)
                .color(primary)
                .strong(),
        );
        ui.label(
            RichText::new("last frame")
                .size(11.0)
                .color(muted),
        );
    });
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!(
                "avg {} · {} script{} · {} calls",
                format_duration(state.totals.total_avg_update),
                state.totals.script_count,
                if state.totals.script_count == 1 { "" } else { "s" },
                state.totals.total_calls,
            ))
            .size(11.0)
            .color(secondary),
        );
    });
    if state.totals.scripts_with_errors > 0 {
        ui.add_space(2.0);
        ui.label(
            RichText::new(format!(
                "⚠ {} script{} threw errors ({} total)",
                state.totals.scripts_with_errors,
                if state.totals.scripts_with_errors == 1 {
                    ""
                } else {
                    "s"
                },
                state.totals.total_errors,
            ))
            .size(11.0)
            .color(Color32::from_rgb(230, 110, 110))
            .strong(),
        );
    }
    ui.add_space(4.0);
    stat(ui, theme, "Entities w/ ScriptComponent", &state.entities_with_script.to_string());
    stat(ui, theme, "Total attachments", &state.total_script_attachments.to_string());
    stat(ui, theme, "Backends registered", &state.backend_count.to_string());
    stat(
        ui,
        theme,
        "Scripts folder",
        state.scripts_folder.as_deref().unwrap_or("<unset>"),
    );
}

fn render_table(ui: &mut egui::Ui, state: &ScriptingDiagState, theme: &Theme) {
    let muted = theme.text.muted.to_color32();
    let primary = theme.text.primary.to_color32();
    let secondary = theme.text.secondary.to_color32();

    ui.label(
        RichText::new("Per-script timing")
            .size(12.0)
            .color(muted),
    );
    ui.add_space(4.0);

    if state.per_script.is_empty() {
        ui.label(
            RichText::new("(no scripts have ticked yet)")
                .size(11.0)
                .color(muted)
                .italics(),
        );
        return;
    }

    // Column header row.
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        column_label(ui, "Script", 220.0, muted);
        column_label(ui, "Last", 60.0, muted);
        column_label(ui, "Avg", 60.0, muted);
        column_label(ui, "Max", 60.0, muted);
        column_label(ui, "Calls", 60.0, muted);
    });
    let stroke = egui::Stroke::new(1.0, theme.widgets.border.to_color32());
    let (_, sep) = ui.allocate_space(egui::vec2(ui.available_width(), 1.0));
    ui.painter().line_segment([sep.left_top(), sep.right_top()], stroke);
    ui.add_space(2.0);

    for (path, perf) in &state.per_script {
        // Grey out scripts that didn't tick this frame — usually means
        // their entity moved to a tab the user isn't viewing or the
        // script was disabled.
        let stale = state.current_frame.saturating_sub(perf.last_seen_frame) > 2;
        let name_color = if stale { muted } else { primary };
        let value_color = if stale { muted } else { secondary };
        let label_color = if perf.error_count > 0 {
            Color32::from_rgb(230, 130, 110)
        } else {
            name_color
        };

        ui.horizontal(|ui| {
            ui.add_space(4.0);
            // Path: show file name + parent directory for context.
            let display = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());
            column_value(ui, &display, 220.0, label_color, perf.error_count > 0);
            column_value(
                ui,
                &format_duration(perf.last_on_update),
                60.0,
                duration_color(perf.last_on_update, value_color),
                false,
            );
            column_value(
                ui,
                &format_duration(perf.avg_on_update),
                60.0,
                value_color,
                false,
            );
            column_value(
                ui,
                &format_duration(perf.max_on_update),
                60.0,
                value_color,
                false,
            );
            column_value(
                ui,
                &format_count(perf.on_update_calls),
                60.0,
                value_color,
                false,
            );
        });

        // Sub-line: extra hook timings if any, plus the most recent
        // error message in red so a perpetually-failing script is
        // impossible to miss.
        if perf.last_on_ready > Duration::ZERO
            || perf.last_on_rpc > Duration::ZERO
            || perf.last_on_ui > Duration::ZERO
        {
            ui.horizontal(|ui| {
                ui.add_space(18.0);
                let mut bits: Vec<String> = Vec::new();
                if perf.last_on_ready > Duration::ZERO {
                    bits.push(format!("ready: {}", format_duration(perf.last_on_ready)));
                }
                if perf.last_on_rpc > Duration::ZERO {
                    bits.push(format!("rpc: {}", format_duration(perf.last_on_rpc)));
                }
                if perf.last_on_ui > Duration::ZERO {
                    bits.push(format!("ui: {}", format_duration(perf.last_on_ui)));
                }
                ui.label(
                    RichText::new(bits.join("  ·  "))
                        .size(10.0)
                        .color(muted),
                );
            });
        }
        if let Some(err) = &perf.last_error {
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

fn duration_color(d: Duration, base: Color32) -> Color32 {
    // Lightly tint anything > 1ms (per frame budget at 60fps is ~16ms;
    // a single script ≥ 1ms is unusual). Hard red ≥ 5ms.
    let micros = d.as_micros();
    if micros >= 5_000 {
        Color32::from_rgb(230, 110, 110)
    } else if micros >= 1_000 {
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

fn column_label(ui: &mut egui::Ui, label: &str, width: f32, color: Color32) {
    ui.allocate_ui_with_layout(
        egui::vec2(width, 14.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.label(RichText::new(label).size(10.0).color(color));
        },
    );
}

fn column_value(
    ui: &mut egui::Ui,
    text: &str,
    width: f32,
    color: Color32,
    bold: bool,
) {
    ui.allocate_ui_with_layout(
        egui::vec2(width, 14.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            let mut rt = RichText::new(text).size(11.0).color(color).monospace();
            if bold {
                rt = rt.strong();
            }
            ui.label(rt);
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
