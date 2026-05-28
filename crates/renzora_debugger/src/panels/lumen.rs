//! Lumen diagnostics panel content.
//!
//! Two layers:
//!   - **CPU bake** — `bake_mesh_samples` runs each frame on the main
//!     world, throttled to `MAX_BAKES_PER_FRAME` entities. We capture
//!     last/avg/max wall-clock plus sample-emission counts so the user
//!     can tell when the throttle is the bottleneck on first-time scene
//!     load (or after a large mesh change).
//!   - **Per-camera state** — `VoxelCacheView` flags (`inject_active`,
//!     `debug_active`) tell you which camera is actually driving the
//!     GI passes, and the count of `MeshVoxelSamples` entities tells
//!     you how much geometry the cache currently has coverage for.
//!
//! GPU pass timings (geometry inject, voxel resolve, SSR) live in the
//! render-world sub-app and need a bridge resource to surface here —
//! not done in v1; use the Render Stats panel or Tracy for those.

use std::time::Duration;

use bevy_egui::egui::{self, Color32, RichText};
use renzora_lumen::LumenBakeStats;
use renzora_theme::Theme;

/// Per-frame snapshot updated by `update_lumen_diag_state`. Holds
/// everything renderable in the panel so the UI side stays a pure
/// reader.
#[derive(bevy::prelude::Resource, Default, Clone)]
pub struct LumenDiagState {
    pub cameras: Vec<LumenCameraEntry>,
    pub mesh_voxel_samples_entities: usize,
    pub has_sky_cubemap: bool,
    pub bake: LumenBakeStats,
}

#[derive(Clone)]
pub struct LumenCameraEntry {
    pub camera_name: String,
    pub inject_active: bool,
    pub debug_active: bool,
}

pub fn render_lumen_content(ui: &mut egui::Ui, state: &LumenDiagState, theme: &Theme) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());

                render_bake_section(ui, &state.bake, theme);
                ui.add_space(10.0);
                render_coverage_section(ui, state, theme);
                ui.add_space(10.0);
                render_cameras_section(ui, &state.cameras, theme);
            });
        });
}

fn render_bake_section(ui: &mut egui::Ui, bake: &LumenBakeStats, theme: &Theme) {
    let primary = theme.text.primary.to_color32();
    let muted = theme.text.muted.to_color32();
    let secondary = theme.text.secondary.to_color32();

    ui.label(
        RichText::new("CPU bake")
            .size(12.0)
            .color(muted),
    );
    ui.add_space(2.0);

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format_duration(bake.last_bake_dur))
                .size(22.0)
                .color(duration_color(bake.last_bake_dur, primary))
                .strong(),
        );
        ui.label(
            RichText::new(format!(
                "last frame  ·  avg {}  ·  max {}",
                format_duration(bake.avg_bake_dur),
                format_duration(bake.max_bake_dur),
            ))
            .size(11.0)
            .color(secondary),
        );
    });

    let saturated = bake.bake_budget_per_frame > 0
        && bake.bakes_last_frame >= bake.bake_budget_per_frame;
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!(
                "{}/{} bakes this frame",
                bake.bakes_last_frame, bake.bake_budget_per_frame.max(1),
            ))
            .size(11.0)
            .color(if saturated {
                Color32::from_rgb(230, 180, 80)
            } else {
                secondary
            }),
        );
        if saturated {
            ui.label(
                RichText::new("⚠ throttle saturated (more meshes queued)")
                    .size(11.0)
                    .color(Color32::from_rgb(230, 180, 80)),
            );
        }
    });

    stat(ui, theme, "Lifetime bakes", &format_count(bake.total_bakes));
    stat(
        ui,
        theme,
        "Lifetime samples emitted",
        &format_count(bake.total_samples_baked),
    );
}

fn render_coverage_section(ui: &mut egui::Ui, state: &LumenDiagState, theme: &Theme) {
    let muted = theme.text.muted.to_color32();
    ui.label(RichText::new("Coverage").size(12.0).color(muted));
    ui.add_space(2.0);
    stat(
        ui,
        theme,
        "Entities w/ MeshVoxelSamples",
        &state.mesh_voxel_samples_entities.to_string(),
    );
    stat(
        ui,
        theme,
        "Sky cubemap source bound",
        yesno(state.has_sky_cubemap),
    );
}

fn render_cameras_section(
    ui: &mut egui::Ui,
    cams: &[LumenCameraEntry],
    theme: &Theme,
) {
    let muted = theme.text.muted.to_color32();
    let primary = theme.text.primary.to_color32();
    let secondary = theme.text.secondary.to_color32();
    ui.label(RichText::new("Voxel cache views").size(12.0).color(muted));
    ui.add_space(4.0);
    if cams.is_empty() {
        ui.label(
            RichText::new("(no cameras with VoxelCacheView)")
                .size(11.0)
                .color(muted)
                .italics(),
        );
        return;
    }
    for cam in cams {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(&cam.camera_name)
                    .strong()
                    .color(primary)
                    .size(11.0),
            );
        });
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label(
                RichText::new(format!(
                    "inject: {}  ·  debug: {}",
                    yesno(cam.inject_active),
                    yesno(cam.debug_active),
                ))
                .color(secondary)
                .size(11.0),
            );
        });
        ui.add_space(3.0);
    }
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

fn duration_color(d: Duration, base: Color32) -> Color32 {
    let ms = d.as_micros() as f64 / 1_000.0;
    if ms >= 5.0 {
        Color32::from_rgb(230, 110, 110)
    } else if ms >= 1.0 {
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

fn yesno(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}
