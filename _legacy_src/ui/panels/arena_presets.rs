//! Arena Presets panel — spawn pre-built arena environments

use bevy_egui::egui::{self, RichText};

use crate::core::resources::arena_presets::{ArenaCommand, ArenaPresetsState, ArenaType};
use renzora_theme::Theme;

/// Render the arena presets panel content
pub fn render_arena_presets_content(
    ui: &mut egui::Ui,
    state: &mut ArenaPresetsState,
    theme: &Theme,
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());

                // Title
                ui.label(
                    RichText::new("Arena Presets")
                        .size(13.0)
                        .color(theme.text.primary.to_color32())
                        .strong(),
                );

                ui.add_space(8.0);

                // Environment selection
                render_environment_section(ui, state, theme);

                ui.add_space(8.0);

                // Description of selected arena
                ui.label(
                    RichText::new(state.arena_type.description())
                        .size(9.0)
                        .color(theme.text.secondary.to_color32()),
                );

                ui.add_space(12.0);

                // Scale slider
                render_scale_section(ui, state, theme);

                ui.add_space(12.0);

                // Action buttons
                render_actions_section(ui, state, theme);

                ui.add_space(8.0);

                // Active arena entity count
                if state.has_active_arena {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{}", state.arena_entity_count))
                                .size(20.0)
                                .color(theme.text.primary.to_color32())
                                .strong(),
                        );
                        ui.label(
                            RichText::new("arena entities")
                                .size(11.0)
                                .color(theme.text.muted.to_color32()),
                        );
                    });

                    ui.add_space(8.0);
                }

                // Tip
                ui.label(
                    RichText::new(
                        "Arenas work with any physics panel \u{2014} spawn objects with Playground, Scenarios, or Stress Test",
                    )
                    .size(9.0)
                    .color(theme.text.muted.to_color32())
                    .italics(),
                );
            });
        });
}

fn render_environment_section(
    ui: &mut egui::Ui,
    state: &mut ArenaPresetsState,
    theme: &Theme,
) {
    ui.label(
        RichText::new("Environment")
            .size(12.0)
            .color(theme.text.muted.to_color32()),
    );
    ui.add_space(4.0);

    ui.horizontal_wrapped(|ui| {
        for arena in ArenaType::ALL {
            let selected = state.arena_type == *arena;
            let text = RichText::new(arena.label()).size(10.0);
            let btn = if selected {
                egui::Button::new(text).fill(theme.semantic.accent.to_color32())
            } else {
                egui::Button::new(text)
            };
            if ui.add(btn).clicked() {
                state.arena_type = *arena;
            }
        }
    });
}

fn render_scale_section(
    ui: &mut egui::Ui,
    state: &mut ArenaPresetsState,
    theme: &Theme,
) {
    ui.label(
        RichText::new("Scale")
            .size(11.0)
            .color(theme.text.muted.to_color32()),
    );
    ui.add_space(4.0);
    ui.add(egui::Slider::new(&mut state.scale, 0.5..=5.0));
}

fn render_actions_section(
    ui: &mut egui::Ui,
    state: &mut ArenaPresetsState,
    theme: &Theme,
) {
    ui.horizontal(|ui| {
        let spawn_btn = egui::Button::new(
            RichText::new("Spawn Arena").size(12.0),
        )
        .fill(theme.semantic.success.to_color32());

        if ui.add(spawn_btn).clicked() {
            state.commands.push(ArenaCommand::Spawn);
        }

        if state.has_active_arena {
            ui.add_space(8.0);
            let clear_btn = egui::Button::new(
                RichText::new("Clear Arena").size(12.0),
            );

            if ui.add(clear_btn).clicked() {
                state.commands.push(ArenaCommand::Clear);
            }
        }
    });
}
