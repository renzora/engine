//! Collision Visualizer panel — contact points, normals, penetration depth

use bevy_egui::egui::{self, RichText};

use crate::core::resources::collision_viz::{
    CollisionVizState, ContactColorMode,
};
use crate::theming::Theme;

/// Render the collision visualizer panel content
pub fn render_collision_viz_content(
    ui: &mut egui::Ui,
    state: &mut CollisionVizState,
    theme: &Theme,
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());

                ui.label(
                    RichText::new("Collision Visualizer")
                        .size(13.0)
                        .color(theme.text.primary.to_color32())
                        .strong(),
                );

                ui.add_space(8.0);

                // Toggle checkboxes
                ui.checkbox(&mut state.show_contact_points, "Show Contact Points");
                ui.checkbox(&mut state.show_normals, "Show Normals");
                ui.checkbox(&mut state.show_penetration, "Show Penetration");
                ui.checkbox(&mut state.show_impulse_flash, "Flash on Impulse");

                ui.add_space(8.0);

                // Color mode
                ui.label(
                    RichText::new("Color Mode")
                        .size(12.0)
                        .color(theme.text.muted.to_color32()),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    for mode in ContactColorMode::ALL {
                        let selected = state.color_by == *mode;
                        let text = RichText::new(mode.label()).size(10.0);
                        let btn = if selected {
                            egui::Button::new(text).fill(theme.semantic.accent.to_color32())
                        } else {
                            egui::Button::new(text)
                        };
                        if ui.add(btn).clicked() {
                            state.color_by = *mode;
                        }
                    }
                });

                ui.add_space(8.0);

                // Size sliders
                egui::Grid::new("collision_viz_params")
                    .num_columns(2)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Point Size").size(10.0).color(theme.text.secondary.to_color32()));
                        ui.add(egui::DragValue::new(&mut state.contact_point_size).speed(0.01).range(0.01..=1.0));
                        ui.end_row();

                        ui.label(RichText::new("Normal Length").size(10.0).color(theme.text.secondary.to_color32()));
                        ui.add(egui::DragValue::new(&mut state.normal_length).speed(0.1).range(0.1..=5.0));
                        ui.end_row();
                    });

                ui.add_space(12.0);

                // Live stats
                ui.label(
                    RichText::new("Live Stats")
                        .size(12.0)
                        .color(theme.text.muted.to_color32()),
                );
                ui.add_space(4.0);

                egui::Grid::new("collision_stats")
                    .num_columns(2)
                    .spacing([8.0, 2.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Active Contacts").size(10.0).color(theme.text.secondary.to_color32()));
                        ui.label(RichText::new(format!("{}", state.active_contacts)).size(10.0).color(theme.text.primary.to_color32()).monospace());
                        ui.end_row();

                        ui.label(RichText::new("Deepest Penetration").size(10.0).color(theme.text.secondary.to_color32()));
                        ui.label(RichText::new(format!("{:.3}", state.deepest_penetration)).size(10.0).color(theme.text.primary.to_color32()).monospace());
                        ui.end_row();

                        ui.label(RichText::new("Max Impulse").size(10.0).color(theme.text.secondary.to_color32()));
                        ui.label(RichText::new(format!("{:.1}", state.max_impulse)).size(10.0).color(theme.text.primary.to_color32()).monospace());
                        ui.end_row();
                    });

                ui.add_space(12.0);

                // Collision log
                ui.label(
                    RichText::new("Collision Log")
                        .size(12.0)
                        .color(theme.text.muted.to_color32()),
                );
                ui.add_space(4.0);

                if state.collision_log.is_empty() {
                    ui.label(
                        RichText::new("No collision events yet")
                            .size(9.0)
                            .color(theme.text.disabled.to_color32()),
                    );
                } else {
                    let max_show = 20;
                    for event in state.collision_log.iter().rev().take(max_show) {
                        let icon = match event.event_type {
                            crate::core::resources::collision_viz::CollisionEventType::Start => "\u{f111}",
                            crate::core::resources::collision_viz::CollisionEventType::End => "\u{f10c}",
                        };
                        ui.label(
                            RichText::new(format!(
                                "{} {:?} <-> {:?} @ {:.1}s",
                                icon, event.entity_a, event.entity_b, event.timestamp
                            ))
                            .size(9.0)
                            .color(theme.text.secondary.to_color32())
                            .monospace(),
                        );
                    }
                }
            });
        });
}
