//! Forces & Impulses panel — interactive force application

use bevy_egui::egui::{self, RichText};
use renzora_theme::Theme;

use crate::state::{DirectionPreset, ForceCommand, ForceMode, ForcesState};

pub fn render_forces_content(ui: &mut egui::Ui, state: &mut ForcesState, theme: &Theme) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());

                // Selection info
                if let Some(entity) = state.selected_entity {
                    ui.horizontal(|ui| {
                        let (icon, color) = if state.selected_has_rigidbody {
                            ("\u{f00c}", theme.semantic.success.to_color32())
                        } else {
                            ("\u{f071}", theme.semantic.warning.to_color32())
                        };
                        ui.label(RichText::new(icon).size(11.0).color(color));
                        ui.label(RichText::new(format!("Entity {:?}", entity)).size(10.0).color(theme.text.secondary.to_color32()).monospace());
                        if !state.selected_has_rigidbody {
                            ui.label(RichText::new("(no rigid body)").size(9.0).color(theme.semantic.warning.to_color32()));
                        }
                    });

                    if state.selected_has_rigidbody {
                        ui.horizontal(|ui| {
                            let v = state.selected_linear_velocity;
                            let w = state.selected_angular_velocity;
                            ui.label(RichText::new(format!("v: [{:.1}, {:.1}, {:.1}]", v.x, v.y, v.z)).size(9.0).color(theme.text.muted.to_color32()).monospace());
                            ui.label(RichText::new(format!("\u{03c9}: [{:.1}, {:.1}, {:.1}]", w.x, w.y, w.z)).size(9.0).color(theme.text.muted.to_color32()).monospace());
                        });
                    }
                } else {
                    ui.label(RichText::new("No entity selected").size(11.0).color(theme.text.muted.to_color32()));
                    ui.label(RichText::new("Select a rigid body entity to apply forces").size(9.0).color(theme.text.disabled.to_color32()));
                }

                ui.add_space(12.0);

                // Mode
                ui.label(RichText::new("Mode").size(12.0).color(theme.text.muted.to_color32()));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    for mode in ForceMode::ALL {
                        let selected = state.mode == *mode;
                        let text = RichText::new(mode.label()).size(10.0);
                        let btn = if selected {
                            egui::Button::new(text).fill(theme.semantic.accent.to_color32())
                        } else {
                            egui::Button::new(text)
                        };
                        if ui.add(btn).clicked() {
                            state.mode = *mode;
                        }
                    }
                });

                ui.add_space(12.0);

                // Direction
                ui.label(RichText::new("Direction").size(12.0).color(theme.text.muted.to_color32()));
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    let presets = [
                        DirectionPreset::Up, DirectionPreset::Down,
                        DirectionPreset::Left, DirectionPreset::Right,
                        DirectionPreset::Forward, DirectionPreset::Back,
                        DirectionPreset::Custom,
                    ];
                    for preset in &presets {
                        let selected = state.direction_preset == *preset;
                        let text = RichText::new(preset.label()).size(10.0);
                        let btn = if selected {
                            egui::Button::new(text).fill(theme.semantic.accent.to_color32())
                        } else {
                            egui::Button::new(text)
                        };
                        if ui.add(btn).clicked() {
                            state.direction_preset = *preset;
                            if *preset != DirectionPreset::Custom {
                                state.custom_direction = preset.to_vec3();
                            }
                        }
                    }
                });

                if state.direction_preset == DirectionPreset::Custom {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("X").size(10.0).color(theme.text.secondary.to_color32()));
                        ui.add(egui::DragValue::new(&mut state.custom_direction.x).speed(0.1));
                        ui.label(RichText::new("Y").size(10.0).color(theme.text.secondary.to_color32()));
                        ui.add(egui::DragValue::new(&mut state.custom_direction.y).speed(0.1));
                        ui.label(RichText::new("Z").size(10.0).color(theme.text.secondary.to_color32()));
                        ui.add(egui::DragValue::new(&mut state.custom_direction.z).speed(0.1));
                    });
                }

                ui.add_space(8.0);

                // Magnitude
                ui.label(RichText::new("Magnitude").size(12.0).color(theme.text.muted.to_color32()));
                ui.add_space(4.0);
                ui.add(egui::Slider::new(&mut state.magnitude, 0.1..=1000.0).logarithmic(true).clamping(egui::SliderClamping::Always));

                ui.add_space(12.0);

                // Apply
                let can_apply = state.selected_entity.is_some() && state.selected_has_rigidbody;
                ui.add_enabled_ui(can_apply, |ui| {
                    ui.horizontal(|ui| {
                        let apply_text = match state.mode {
                            ForceMode::Force => "Apply Force",
                            ForceMode::Impulse => "Apply Impulse",
                            ForceMode::Torque => "Apply Torque",
                            ForceMode::VelocityOverride => "Set Velocity",
                        };
                        let apply_btn = egui::Button::new(RichText::new(apply_text).size(12.0))
                            .fill(theme.semantic.success.to_color32());
                        if ui.add(apply_btn).clicked() {
                            if let Some(entity) = state.selected_entity {
                                let direction = if state.direction_preset == DirectionPreset::Custom {
                                    state.custom_direction
                                } else {
                                    state.direction_preset.to_vec3()
                                };
                                state.commands.push(ForceCommand::Apply {
                                    entity, mode: state.mode, direction, magnitude: state.magnitude,
                                });
                            }
                        }

                        let zero_btn = egui::Button::new(RichText::new("Zero Motion").size(10.0));
                        if ui.add(zero_btn).clicked() {
                            if let Some(entity) = state.selected_entity {
                                state.commands.push(ForceCommand::ZeroMotion { entity });
                            }
                        }
                    });
                });

                ui.add_space(16.0);

                // Explosion
                ui.label(RichText::new("Explosion").size(12.0).color(theme.text.muted.to_color32()));
                ui.add_space(4.0);
                egui::Grid::new("explosion_params")
                    .num_columns(2)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Radius").size(10.0).color(theme.text.secondary.to_color32()));
                        ui.add(egui::DragValue::new(&mut state.explosion_radius).speed(0.5).range(0.1..=100.0).suffix(" m"));
                        ui.end_row();

                        ui.label(RichText::new("Force").size(10.0).color(theme.text.secondary.to_color32()));
                        ui.add(egui::DragValue::new(&mut state.explosion_magnitude).speed(1.0).range(1.0..=1000.0));
                        ui.end_row();
                    });

                ui.add_space(4.0);
                let explode_btn = egui::Button::new(RichText::new("\u{f1e2} Explode at Origin").size(11.0))
                    .fill(theme.semantic.warning.to_color32());
                if ui.add(explode_btn).clicked() {
                    state.commands.push(ForceCommand::Explosion {
                        origin: bevy::math::Vec3::ZERO,
                        radius: state.explosion_radius,
                        magnitude: state.explosion_magnitude,
                    });
                }

                ui.add_space(16.0);

                // Velocity override
                ui.label(RichText::new("Velocity Override").size(12.0).color(theme.text.muted.to_color32()));
                ui.add_space(4.0);
                let can_set = state.selected_entity.is_some() && state.selected_has_rigidbody;
                ui.add_enabled_ui(can_set, |ui| {
                    egui::Grid::new("velocity_override")
                        .num_columns(4)
                        .spacing([4.0, 4.0])
                        .show(ui, |ui| {
                            ui.label(RichText::new("Linear").size(10.0).color(theme.text.secondary.to_color32()));
                            ui.add(egui::DragValue::new(&mut state.velocity_linear.x).speed(0.5).prefix("x:"));
                            ui.add(egui::DragValue::new(&mut state.velocity_linear.y).speed(0.5).prefix("y:"));
                            ui.add(egui::DragValue::new(&mut state.velocity_linear.z).speed(0.5).prefix("z:"));
                            ui.end_row();

                            ui.label(RichText::new("Angular").size(10.0).color(theme.text.secondary.to_color32()));
                            ui.add(egui::DragValue::new(&mut state.velocity_angular.x).speed(0.5).prefix("x:"));
                            ui.add(egui::DragValue::new(&mut state.velocity_angular.y).speed(0.5).prefix("y:"));
                            ui.add(egui::DragValue::new(&mut state.velocity_angular.z).speed(0.5).prefix("z:"));
                            ui.end_row();
                        });

                    ui.add_space(4.0);
                    if ui.button(RichText::new("Set Velocity").size(10.0)).clicked() {
                        if let Some(entity) = state.selected_entity {
                            state.commands.push(ForceCommand::SetVelocity {
                                entity, linear: state.velocity_linear, angular: state.velocity_angular,
                            });
                        }
                    }
                });
            });
        });
}
