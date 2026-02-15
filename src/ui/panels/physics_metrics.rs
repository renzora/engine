//! Physics Metrics panel — real-time energy, velocity, and momentum monitoring

use bevy_egui::egui::{self, RichText, Color32};

use crate::core::resources::physics_metrics::PhysicsMetricsState;
use renzora_theme::Theme;

/// Render the physics metrics panel content
pub fn render_physics_metrics_content(
    ui: &mut egui::Ui,
    state: &mut PhysicsMetricsState,
    theme: &Theme,
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());

                // Tracking toggle
                ui.horizontal(|ui| {
                    ui.checkbox(&mut state.tracking_enabled, "");
                    ui.label(
                        RichText::new("Physics Metrics")
                            .size(13.0)
                            .color(theme.text.primary.to_color32())
                            .strong(),
                    );
                });

                ui.add_space(8.0);

                // Energy section
                render_energy_section(ui, state, theme);

                ui.add_space(12.0);

                // Bodies section
                render_bodies_section(ui, state, theme);

                ui.add_space(12.0);

                // Velocity section
                render_velocity_section(ui, state, theme);

                ui.add_space(12.0);

                // Collisions section
                render_collisions_section(ui, state, theme);

                ui.add_space(12.0);

                // Performance section
                render_performance_section(ui, state, theme);
            });
        });
}

fn render_energy_section(ui: &mut egui::Ui, state: &PhysicsMetricsState, theme: &Theme) {
    ui.label(
        RichText::new("Energy")
            .size(12.0)
            .color(theme.text.muted.to_color32()),
    );
    ui.add_space(4.0);

    egui::Grid::new("energy_grid")
        .num_columns(2)
        .spacing([8.0, 2.0])
        .show(ui, |ui| {
            ui.label(RichText::new("Kinetic").size(10.0).color(theme.text.secondary.to_color32()));
            ui.label(RichText::new(format!("{:.1} J", state.total_kinetic_energy)).size(10.0).color(theme.text.primary.to_color32()).monospace());
            ui.end_row();

            ui.label(RichText::new("Potential").size(10.0).color(theme.text.secondary.to_color32()));
            ui.label(RichText::new(format!("{:.1} J", state.total_potential_energy)).size(10.0).color(theme.text.primary.to_color32()).monospace());
            ui.end_row();

            ui.label(RichText::new("Total").size(10.0).color(theme.text.secondary.to_color32()).strong());
            ui.label(RichText::new(format!("{:.1} J", state.total_energy)).size(10.0).color(theme.text.primary.to_color32()).monospace().strong());
            ui.end_row();
        });

    // Energy sparkline
    if state.energy_history.len() > 1 {
        ui.add_space(4.0);
        draw_sparkline(ui, &state.energy_history.iter().map(|v| *v as f32).collect::<Vec<_>>(), Color32::from_rgb(100, 200, 100), 40.0);
    }
}

fn render_bodies_section(ui: &mut egui::Ui, state: &PhysicsMetricsState, theme: &Theme) {
    ui.label(
        RichText::new("Bodies")
            .size(12.0)
            .color(theme.text.muted.to_color32()),
    );
    ui.add_space(4.0);

    let total = state.active_bodies + state.sleeping_bodies;
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{}", total)).size(16.0).color(theme.text.primary.to_color32()).strong());
        ui.label(RichText::new("total").size(10.0).color(theme.text.muted.to_color32()));
        ui.add_space(8.0);
        ui.label(RichText::new(format!("{} active", state.active_bodies)).size(10.0).color(Color32::from_rgb(100, 200, 100)));
        ui.label(RichText::new(format!("{} sleeping", state.sleeping_bodies)).size(10.0).color(Color32::from_rgb(150, 150, 150)));
    });
}

fn render_velocity_section(ui: &mut egui::Ui, state: &PhysicsMetricsState, theme: &Theme) {
    ui.label(
        RichText::new("Velocity & Momentum")
            .size(12.0)
            .color(theme.text.muted.to_color32()),
    );
    ui.add_space(4.0);

    egui::Grid::new("velocity_grid")
        .num_columns(2)
        .spacing([8.0, 2.0])
        .show(ui, |ui| {
            ui.label(RichText::new("Avg Speed").size(10.0).color(theme.text.secondary.to_color32()));
            ui.label(RichText::new(format!("{:.2} m/s", state.avg_velocity)).size(10.0).color(theme.text.primary.to_color32()).monospace());
            ui.end_row();

            ui.label(RichText::new("Max Speed").size(10.0).color(theme.text.secondary.to_color32()));
            ui.label(RichText::new(format!("{:.2} m/s", state.max_velocity)).size(10.0).color(theme.text.primary.to_color32()).monospace());
            ui.end_row();

            ui.label(RichText::new("Momentum").size(10.0).color(theme.text.secondary.to_color32()));
            ui.label(RichText::new(format!("{:.1}", state.total_momentum.length())).size(10.0).color(theme.text.primary.to_color32()).monospace());
            ui.end_row();
        });
}

fn render_collisions_section(ui: &mut egui::Ui, state: &PhysicsMetricsState, theme: &Theme) {
    ui.label(
        RichText::new("Collisions")
            .size(12.0)
            .color(theme.text.muted.to_color32()),
    );
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{}", state.collision_count)).size(16.0).color(theme.text.primary.to_color32()).strong());
        ui.label(RichText::new("active pairs").size(10.0).color(theme.text.muted.to_color32()));
    });

    if state.collision_pairs_history.len() > 1 {
        ui.add_space(4.0);
        draw_sparkline(ui, &state.collision_pairs_history.iter().map(|v| *v as f32).collect::<Vec<_>>(), Color32::from_rgb(200, 100, 100), 30.0);
    }
}

fn render_performance_section(ui: &mut egui::Ui, state: &PhysicsMetricsState, theme: &Theme) {
    ui.label(
        RichText::new("Physics Time")
            .size(12.0)
            .color(theme.text.muted.to_color32()),
    );
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{:.2} ms", state.frame_physics_time_ms)).size(14.0).color(theme.text.primary.to_color32()).monospace());
    });

    if state.physics_time_history.len() > 1 {
        ui.add_space(4.0);
        draw_sparkline(ui, &state.physics_time_history.iter().copied().collect::<Vec<_>>(), Color32::from_rgb(100, 150, 200), 30.0);
    }
}

/// Draw a simple sparkline graph using egui painter
fn draw_sparkline(ui: &mut egui::Ui, data: &[f32], color: Color32, height: f32) {
    let width = ui.available_width().min(300.0);
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(width, height), egui::Sense::hover());

    if data.len() < 2 {
        return;
    }

    let min_val = data.iter().copied().fold(f32::MAX, f32::min);
    let max_val = data.iter().copied().fold(f32::MIN, f32::max);
    let range = (max_val - min_val).max(0.001);

    let painter = ui.painter();

    // Background
    painter.rect_filled(rect, 2.0, Color32::from_rgba_unmultiplied(30, 30, 40, 180));

    // Draw line
    let points: Vec<egui::Pos2> = data
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let x = rect.min.x + (i as f32 / (data.len() - 1) as f32) * rect.width();
            let y = rect.max.y - ((v - min_val) / range) * rect.height();
            egui::pos2(x, y)
        })
        .collect();

    for pair in points.windows(2) {
        painter.line_segment([pair[0], pair[1]], egui::Stroke::new(1.5, color));
    }
}
