//! Culling debug panel for frustum and distance culling inspection

use bevy_egui::egui::{self, Color32, RichText};

use crate::core::resources::culling_debug::CullingDebugState;
use crate::theming::Theme;

/// Render the culling debug panel content
pub fn render_culling_debug_content(
    ui: &mut egui::Ui,
    state: &mut CullingDebugState,
    theme: &Theme,
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());

                render_overview_header(ui, state, theme);
                ui.add_space(12.0);

                render_culling_breakdown(ui, state, theme);
                ui.add_space(16.0);

                render_distance_distribution(ui, state, theme);
                ui.add_space(16.0);

                render_settings(ui, state, theme);
            });
        });
}

fn render_overview_header(ui: &mut egui::Ui, state: &CullingDebugState, theme: &Theme) {
    let visible = state.frustum_visible.saturating_sub(state.distance_culled);

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{}", visible))
                .size(28.0)
                .color(theme.text.primary.to_color32())
                .strong(),
        );
        ui.label(
            RichText::new(format!("/ {} visible", state.total_entities))
                .size(12.0)
                .color(theme.text.muted.to_color32()),
        );
    });

    if state.total_entities > 0 {
        let pct = (visible as f32 / state.total_entities as f32 * 100.0) as u32;
        ui.label(
            RichText::new(format!("{}% of mesh entities visible", pct))
                .size(10.0)
                .color(theme.text.muted.to_color32()),
        );
    }
}

fn render_culling_breakdown(ui: &mut egui::Ui, state: &CullingDebugState, theme: &Theme) {
    ui.label(RichText::new("Culling Breakdown").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    egui::Frame::NONE
        .fill(Color32::from_rgb(35, 37, 42))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            let total = state.total_entities.max(1) as f32;

            let visible_after_distance = state.frustum_visible.saturating_sub(state.distance_culled);

            render_bar_row(
                ui, "Visible", visible_after_distance, total,
                Color32::from_rgb(100, 200, 100), theme,
            );
            render_bar_row(
                ui, "Frustum Culled", state.frustum_culled, total,
                Color32::from_rgb(200, 150, 80), theme,
            );
            render_bar_row(
                ui, "Distance Culled", state.distance_culled, total,
                Color32::from_rgb(200, 100, 100), theme,
            );

            if state.distance_faded > 0 {
                render_bar_row(
                    ui, "Fading", state.distance_faded, total,
                    Color32::from_rgb(180, 180, 100), theme,
                );
            }
        });
}

fn render_bar_row(
    ui: &mut egui::Ui,
    label: &str,
    count: u32,
    total: f32,
    color: Color32,
    theme: &Theme,
) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .size(10.0)
                .color(theme.text.secondary.to_color32()),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format!("{}", count))
                    .size(10.0)
                    .color(theme.text.primary.to_color32())
                    .monospace(),
            );
        });
    });

    let fraction = (count as f32 / total).clamp(0.0, 1.0);
    let available_width = ui.available_width();
    let bar_height = 6.0;

    let (rect, _) = ui.allocate_exact_size(
        egui::Vec2::new(available_width, bar_height),
        egui::Sense::hover(),
    );

    // Background
    ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(50, 52, 58));
    // Filled portion
    let filled_rect = egui::Rect::from_min_size(
        rect.min,
        egui::Vec2::new(available_width * fraction, bar_height),
    );
    ui.painter().rect_filled(filled_rect, 2.0, color);

    ui.add_space(4.0);
}

fn render_distance_distribution(ui: &mut egui::Ui, state: &CullingDebugState, theme: &Theme) {
    ui.label(RichText::new("Distance Distribution").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    let bucket_labels = ["0-50m", "50-100m", "100-200m", "200-500m", "500m+"];
    let max_bucket = state.distance_buckets.iter().copied().max().unwrap_or(1).max(1) as f32;

    egui::Frame::NONE
        .fill(Color32::from_rgb(35, 37, 42))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            for (i, (count, label)) in state.distance_buckets.iter().zip(bucket_labels.iter()).enumerate() {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(*label)
                            .size(10.0)
                            .color(theme.text.secondary.to_color32())
                            .monospace(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{}", count))
                                .size(10.0)
                                .color(theme.text.primary.to_color32())
                                .monospace(),
                        );
                    });
                });

                let fraction = (*count as f32 / max_bucket).clamp(0.0, 1.0);
                let available_width = ui.available_width();
                let bar_height = 4.0;

                let (rect, _) = ui.allocate_exact_size(
                    egui::Vec2::new(available_width, bar_height),
                    egui::Sense::hover(),
                );

                ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(50, 52, 58));

                let color_intensity = (i as f32 / 4.0 * 0.6 + 0.4).min(1.0);
                let bar_color = Color32::from_rgb(
                    (80.0 + 120.0 * color_intensity) as u8,
                    (160.0 - 60.0 * color_intensity) as u8,
                    200,
                );

                let filled_rect = egui::Rect::from_min_size(
                    rect.min,
                    egui::Vec2::new(available_width * fraction, bar_height),
                );
                ui.painter().rect_filled(filled_rect, 2.0, bar_color);

                if i < 4 {
                    ui.add_space(2.0);
                }
            }
        });
}

fn render_settings(ui: &mut egui::Ui, state: &mut CullingDebugState, theme: &Theme) {
    ui.label(RichText::new("Settings").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    egui::Frame::NONE
        .fill(Color32::from_rgb(35, 37, 42))
        .corner_radius(4.0)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.checkbox(&mut state.enabled, "Enable Distance Culling");

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Max Distance:")
                        .size(10.0)
                        .color(theme.text.secondary.to_color32()),
                );
                ui.add(egui::Slider::new(&mut state.max_distance, 10.0..=2000.0).suffix("m"));
            });

            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Fade Start:")
                        .size(10.0)
                        .color(theme.text.secondary.to_color32()),
                );
                ui.add(egui::Slider::new(&mut state.fade_start_fraction, 0.5..=1.0).fixed_decimals(2));
            });
        });
}
