//! Physics Playground panel — stress-test spawner

use bevy_egui::egui::{self, RichText};

use crate::core::resources::physics_playground::{
    PlaygroundCommand, PlaygroundShape, PlaygroundState, SpawnPattern,
};
use renzora_theme::Theme;

/// Render the physics playground panel content
pub fn render_physics_playground_content(
    ui: &mut egui::Ui,
    state: &mut PlaygroundState,
    theme: &Theme,
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());

                // Stats header
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{}", state.alive_count))
                            .size(20.0)
                            .color(theme.text.primary.to_color32())
                            .strong(),
                    );
                    ui.label(
                        RichText::new("playground entities")
                            .size(11.0)
                            .color(theme.text.muted.to_color32()),
                    );
                });

                ui.add_space(12.0);

                // Shape selection
                render_shape_section(ui, state, theme);

                ui.add_space(12.0);

                // Pattern selection
                render_pattern_section(ui, state, theme);

                ui.add_space(12.0);

                // Parameters
                render_params_section(ui, state, theme);

                ui.add_space(12.0);

                // Spawn / Clear buttons
                render_actions_section(ui, state, theme);
            });
        });
}

fn render_shape_section(ui: &mut egui::Ui, state: &mut PlaygroundState, theme: &Theme) {
    ui.label(
        RichText::new("Shape")
            .size(12.0)
            .color(theme.text.muted.to_color32()),
    );
    ui.add_space(4.0);

    ui.horizontal_wrapped(|ui| {
        for shape in PlaygroundShape::ALL {
            let selected = state.shape == *shape;
            let text = RichText::new(shape.label()).size(10.0);
            let btn = if selected {
                egui::Button::new(text).fill(theme.semantic.accent.to_color32())
            } else {
                egui::Button::new(text)
            };
            if ui.add(btn).clicked() {
                state.shape = *shape;
            }
        }
    });
}

fn render_pattern_section(ui: &mut egui::Ui, state: &mut PlaygroundState, theme: &Theme) {
    ui.label(
        RichText::new("Pattern")
            .size(12.0)
            .color(theme.text.muted.to_color32()),
    );
    ui.add_space(4.0);

    ui.horizontal_wrapped(|ui| {
        for pattern in SpawnPattern::ALL {
            let selected = state.pattern == *pattern;
            let text = RichText::new(pattern.label()).size(10.0);
            let btn = if selected {
                egui::Button::new(text).fill(theme.semantic.accent.to_color32())
            } else {
                egui::Button::new(text)
            };
            if ui.add(btn).clicked() {
                state.pattern = *pattern;
            }
        }
    });
}

fn render_params_section(ui: &mut egui::Ui, state: &mut PlaygroundState, theme: &Theme) {
    ui.label(
        RichText::new("Parameters")
            .size(12.0)
            .color(theme.text.muted.to_color32()),
    );
    ui.add_space(4.0);

    egui::Grid::new("playground_params")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label(
                RichText::new("Count")
                    .size(10.0)
                    .color(theme.text.secondary.to_color32()),
            );
            ui.add(
                egui::DragValue::new(&mut state.count)
                    .speed(1)
                    .range(1..=500),
            );
            ui.end_row();

            ui.label(
                RichText::new("Mass")
                    .size(10.0)
                    .color(theme.text.secondary.to_color32()),
            );
            ui.add(
                egui::DragValue::new(&mut state.mass)
                    .speed(0.1)
                    .range(0.01..=1000.0)
                    .suffix(" kg"),
            );
            ui.end_row();

            ui.label(
                RichText::new("Restitution")
                    .size(10.0)
                    .color(theme.text.secondary.to_color32()),
            );
            ui.add(
                egui::DragValue::new(&mut state.restitution)
                    .speed(0.01)
                    .range(0.0..=1.0),
            );
            ui.end_row();

            ui.label(
                RichText::new("Friction")
                    .size(10.0)
                    .color(theme.text.secondary.to_color32()),
            );
            ui.add(
                egui::DragValue::new(&mut state.friction)
                    .speed(0.01)
                    .range(0.0..=2.0),
            );
            ui.end_row();

            ui.label(
                RichText::new("Spawn Height")
                    .size(10.0)
                    .color(theme.text.secondary.to_color32()),
            );
            ui.add(
                egui::DragValue::new(&mut state.spawn_height)
                    .speed(0.5)
                    .range(0.0..=100.0)
                    .suffix(" m"),
            );
            ui.end_row();
        });
}

fn render_actions_section(ui: &mut egui::Ui, state: &mut PlaygroundState, theme: &Theme) {
    ui.horizontal(|ui| {
        let spawn_btn = egui::Button::new(
            RichText::new("\u{f067} Spawn").size(12.0), // plus icon
        )
        .fill(theme.semantic.success.to_color32());

        if ui.add(spawn_btn).clicked() {
            state.commands.push(PlaygroundCommand::Spawn);
        }

        ui.add_space(8.0);

        if state.alive_count > 0 {
            let clear_btn = egui::Button::new(
                RichText::new("\u{f1f8} Clear All").size(12.0), // trash icon
            )
            .fill(theme.semantic.error.to_color32());

            if ui.add(clear_btn).clicked() {
                state.commands.push(PlaygroundCommand::ClearAll);
            }
        }
    });
}
