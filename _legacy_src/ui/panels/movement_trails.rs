//! Movement Trails panel — bulk trail controls

use bevy_egui::egui::{self, RichText};

use crate::core::resources::movement_trails::{
    MovementTrailsState, TrailColorMode, TrailCommand,
};
use renzora_theme::Theme;

/// Render the movement trails panel content
pub fn render_movement_trails_content(
    ui: &mut egui::Ui,
    state: &mut MovementTrailsState,
    theme: &Theme,
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());

                // Header with master toggle
                ui.horizontal(|ui| {
                    ui.checkbox(&mut state.show_all, "");
                    ui.label(
                        RichText::new("Movement Trails")
                            .size(13.0)
                            .color(theme.text.primary.to_color32())
                            .strong(),
                    );
                });

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{} entities with trails", state.trail_entity_count))
                            .size(10.0)
                            .color(theme.text.muted.to_color32()),
                    );
                });

                ui.add_space(12.0);

                // Add / Remove buttons
                ui.horizontal(|ui| {
                    let add_btn = egui::Button::new(
                        RichText::new("Add Trail to Selected").size(10.0),
                    )
                    .fill(theme.semantic.success.to_color32());

                    if ui.add(add_btn).clicked() {
                        state.commands.push(TrailCommand::AddToSelected);
                    }

                    let remove_btn = egui::Button::new(
                        RichText::new("Remove from Selected").size(10.0),
                    );

                    if ui.add(remove_btn).clicked() {
                        state.commands.push(TrailCommand::RemoveFromSelected);
                    }
                });

                ui.add_space(8.0);

                // Clear all button
                let clear_btn = egui::Button::new(
                    RichText::new("Clear All Trail Points").size(10.0),
                )
                .fill(theme.semantic.warning.to_color32());

                if ui.add(clear_btn).clicked() {
                    state.commands.push(TrailCommand::ClearAll);
                }

                ui.add_space(12.0);

                // Global color mode override
                ui.label(
                    RichText::new("Global Color Override")
                        .size(12.0)
                        .color(theme.text.muted.to_color32()),
                );
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    let none_selected = state.global_color_mode.is_none();
                    let none_btn = if none_selected {
                        egui::Button::new(RichText::new("None").size(10.0))
                            .fill(theme.semantic.accent.to_color32())
                    } else {
                        egui::Button::new(RichText::new("None").size(10.0))
                    };
                    if ui.add(none_btn).clicked() {
                        state.global_color_mode = None;
                    }

                    for mode in TrailColorMode::ALL {
                        let selected = state.global_color_mode == Some(*mode);
                        let text = RichText::new(mode.label()).size(10.0);
                        let btn = if selected {
                            egui::Button::new(text).fill(theme.semantic.accent.to_color32())
                        } else {
                            egui::Button::new(text)
                        };
                        if ui.add(btn).clicked() {
                            state.global_color_mode = Some(*mode);
                        }
                    }
                });

                ui.add_space(12.0);
                ui.label(
                    RichText::new("Add the Movement Trail component to individual entities for per-entity settings")
                        .size(9.0)
                        .color(theme.text.disabled.to_color32()),
                );
            });
        });
}
