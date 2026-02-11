//! Scenario Presets panel — one-click spawning of classic physics test scenes

use bevy_egui::egui::{self, RichText};

use crate::core::resources::physics_scenarios::{
    PhysicsScenariosState, ScenarioCommand, ScenarioType,
};
use crate::theming::Theme;

/// Render the scenario presets panel content
pub fn render_physics_scenarios_content(
    ui: &mut egui::Ui,
    state: &mut PhysicsScenariosState,
    theme: &Theme,
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());

                // Header
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Scenario Presets")
                            .size(13.0)
                            .color(theme.text.primary.to_color32())
                            .strong(),
                    );
                    if state.alive_count > 0 {
                        ui.label(
                            RichText::new(format!("({} entities)", state.alive_count))
                                .size(10.0)
                                .color(theme.text.muted.to_color32()),
                        );
                    }
                });

                ui.add_space(8.0);

                // Scenario grid
                render_scenario_grid(ui, state, theme);

                ui.add_space(8.0);

                // Description
                ui.label(
                    RichText::new(state.selected_scenario.description())
                        .size(10.0)
                        .color(theme.text.secondary.to_color32()),
                );

                ui.add_space(12.0);

                // Scale slider
                ui.label(
                    RichText::new("Scale")
                        .size(12.0)
                        .color(theme.text.muted.to_color32()),
                );
                ui.add_space(4.0);
                ui.add(
                    egui::Slider::new(&mut state.scale, 0.5..=3.0)
                        .step_by(0.1)
                        .suffix("x"),
                );

                ui.add_space(12.0);

                // Action buttons
                ui.horizontal(|ui| {
                    let spawn_btn = egui::Button::new(
                        RichText::new("\u{f067} Spawn Scenario").size(12.0),
                    )
                    .fill(theme.semantic.success.to_color32());

                    if ui.add(spawn_btn).clicked() {
                        state.commands.push(ScenarioCommand::Spawn);
                    }

                    if state.alive_count > 0 {
                        ui.add_space(8.0);
                        let clear_btn = egui::Button::new(
                            RichText::new("\u{f1f8} Clear").size(12.0),
                        )
                        .fill(theme.semantic.error.to_color32());

                        if ui.add(clear_btn).clicked() {
                            state.commands.push(ScenarioCommand::ClearScenario);
                        }
                    }
                });

                ui.add_space(8.0);
                ui.label(
                    RichText::new("Enter play mode to see simulation")
                        .size(9.0)
                        .color(theme.text.disabled.to_color32()),
                );
            });
        });
}

fn render_scenario_grid(
    ui: &mut egui::Ui,
    state: &mut PhysicsScenariosState,
    theme: &Theme,
) {
    // 2 columns of scenario buttons
    egui::Grid::new("scenario_grid")
        .num_columns(2)
        .spacing([6.0, 6.0])
        .show(ui, |ui| {
            for (i, scenario) in ScenarioType::ALL.iter().enumerate() {
                let selected = state.selected_scenario == *scenario;
                let text = RichText::new(scenario.label()).size(10.0);
                let btn = if selected {
                    egui::Button::new(text)
                        .fill(theme.semantic.accent.to_color32())
                        .min_size(egui::Vec2::new(110.0, 24.0))
                } else {
                    egui::Button::new(text)
                        .min_size(egui::Vec2::new(110.0, 24.0))
                };
                if ui.add(btn).clicked() {
                    state.selected_scenario = *scenario;
                }
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
}
