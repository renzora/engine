//! Physics Properties panel — global simulation settings

use bevy_egui::egui::{self, RichText};

use crate::core::resources::physics_properties::{
    GravityPreset, PhysicsPropertiesState, PhysicsPropertyCommand,
};
use crate::theming::Theme;

/// Render the physics properties panel content
pub fn render_physics_properties_content(
    ui: &mut egui::Ui,
    state: &mut PhysicsPropertiesState,
    theme: &Theme,
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());

                if !state.physics_available {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            RichText::new("Physics not available")
                                .size(14.0)
                                .color(theme.text.muted.to_color32()),
                        );
                    });
                    return;
                }

                // Gravity section
                render_gravity_section(ui, state, theme);

                ui.add_space(16.0);

                // Time scale section
                render_time_scale_section(ui, state, theme);

                ui.add_space(16.0);

                // Substeps section
                render_substeps_section(ui, state, theme);

                ui.add_space(16.0);

                // Reset button
                if ui
                    .button(RichText::new("Reset All to Defaults").size(11.0))
                    .clicked()
                {
                    state.commands.push(PhysicsPropertyCommand::ResetAll);
                }
            });
        });
}

fn render_gravity_section(
    ui: &mut egui::Ui,
    state: &mut PhysicsPropertiesState,
    theme: &Theme,
) {
    ui.label(
        RichText::new("Gravity")
            .size(12.0)
            .color(theme.text.muted.to_color32()),
    );
    ui.add_space(4.0);

    // Preset buttons
    ui.horizontal_wrapped(|ui| {
        for preset in GravityPreset::ALL {
            let selected = state.gravity_preset == *preset;
            let label = RichText::new(preset.label()).size(10.0);
            let btn = if selected {
                egui::Button::new(label).fill(theme.semantic.accent.to_color32())
            } else {
                egui::Button::new(label)
            };
            if ui.add(btn).clicked() {
                state
                    .commands
                    .push(PhysicsPropertyCommand::SetGravityPreset(*preset));
            }
        }
    });

    ui.add_space(6.0);

    // Gravity vector
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("X")
                .size(10.0)
                .color(theme.text.secondary.to_color32()),
        );
        let mut gx = state.gravity.x;
        if ui
            .add(egui::DragValue::new(&mut gx).speed(0.1).range(-100.0..=100.0))
            .changed()
        {
            state.gravity.x = gx;
            state
                .commands
                .push(PhysicsPropertyCommand::SetGravity(state.gravity));
        }

        ui.label(
            RichText::new("Y")
                .size(10.0)
                .color(theme.text.secondary.to_color32()),
        );
        let mut gy = state.gravity.y;
        if ui
            .add(egui::DragValue::new(&mut gy).speed(0.1).range(-100.0..=100.0))
            .changed()
        {
            state.gravity.y = gy;
            state
                .commands
                .push(PhysicsPropertyCommand::SetGravity(state.gravity));
        }

        ui.label(
            RichText::new("Z")
                .size(10.0)
                .color(theme.text.secondary.to_color32()),
        );
        let mut gz = state.gravity.z;
        if ui
            .add(egui::DragValue::new(&mut gz).speed(0.1).range(-100.0..=100.0))
            .changed()
        {
            state.gravity.z = gz;
            state
                .commands
                .push(PhysicsPropertyCommand::SetGravity(state.gravity));
        }
    });

    // Display magnitude
    let mag = state.gravity.length();
    ui.label(
        RichText::new(format!("Magnitude: {:.2} m/s\u{00b2}", mag))
            .size(10.0)
            .color(theme.text.muted.to_color32()),
    );
}

fn render_time_scale_section(
    ui: &mut egui::Ui,
    state: &mut PhysicsPropertiesState,
    theme: &Theme,
) {
    ui.label(
        RichText::new("Time Scale")
            .size(12.0)
            .color(theme.text.muted.to_color32()),
    );
    ui.add_space(4.0);

    // Quick presets
    ui.horizontal(|ui| {
        for (label, value) in &[("0.1x", 0.1f32), ("0.25x", 0.25), ("0.5x", 0.5), ("1x", 1.0), ("2x", 2.0), ("4x", 4.0)] {
            let selected = (state.time_scale - value).abs() < 0.01;
            let text = RichText::new(*label).size(10.0);
            let btn = if selected {
                egui::Button::new(text).fill(theme.semantic.accent.to_color32())
            } else {
                egui::Button::new(text)
            };
            if ui.add(btn).clicked() {
                state
                    .commands
                    .push(PhysicsPropertyCommand::SetTimeScale(*value));
            }
        }
    });

    ui.add_space(4.0);

    // Slider
    let mut ts = state.time_scale;
    if ui
        .add(
            egui::Slider::new(&mut ts, 0.0..=10.0)
                .text("speed")
                .logarithmic(true)
                .clamp_to_range(true),
        )
        .changed()
    {
        state
            .commands
            .push(PhysicsPropertyCommand::SetTimeScale(ts));
    }
}

fn render_substeps_section(
    ui: &mut egui::Ui,
    state: &mut PhysicsPropertiesState,
    theme: &Theme,
) {
    ui.label(
        RichText::new("Solver Substeps")
            .size(12.0)
            .color(theme.text.muted.to_color32()),
    );
    ui.add_space(4.0);

    let mut sub = state.substeps;
    if ui
        .add(
            egui::Slider::new(&mut sub, 1..=50)
                .text("substeps")
                .clamp_to_range(true),
        )
        .changed()
    {
        state
            .commands
            .push(PhysicsPropertyCommand::SetSubsteps(sub));
    }

    ui.label(
        RichText::new("Higher = more accurate, slower")
            .size(9.0)
            .color(theme.text.muted.to_color32()),
    );
}
