//! Stress Test panel — automated scaling tests with performance measurement

use bevy_egui::egui::{self, RichText, Color32};

use crate::core::resources::stress_test::{
    StressTestCommand, StressTestState, StressTestType,
};
use crate::theming::Theme;

/// Render the stress test panel content
pub fn render_stress_test_content(
    ui: &mut egui::Ui,
    state: &mut StressTestState,
    theme: &Theme,
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());

                ui.label(
                    RichText::new("Stress Test")
                        .size(13.0)
                        .color(theme.text.primary.to_color32())
                        .strong(),
                );

                ui.add_space(8.0);

                // Test type selector
                ui.label(
                    RichText::new("Test Type")
                        .size(12.0)
                        .color(theme.text.muted.to_color32()),
                );
                ui.add_space(4.0);

                ui.horizontal_wrapped(|ui| {
                    for test in StressTestType::ALL {
                        let selected = state.test_type == *test;
                        let text = RichText::new(test.label()).size(10.0);
                        let btn = if selected {
                            egui::Button::new(text).fill(theme.semantic.accent.to_color32())
                        } else {
                            egui::Button::new(text)
                        };
                        if ui.add(btn).clicked() && !state.is_running {
                            state.test_type = *test;
                        }
                    }
                });

                ui.add_space(4.0);
                ui.label(
                    RichText::new(state.test_type.description())
                        .size(9.0)
                        .color(theme.text.secondary.to_color32()),
                );

                ui.add_space(8.0);

                // Max objects slider
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Max Objects")
                            .size(11.0)
                            .color(theme.text.secondary.to_color32()),
                    );
                    let slider = egui::Slider::new(&mut state.max_objects, 10..=1_000_000)
                        .logarithmic(true)
                        .clamp_to_range(true);
                    if ui.add_enabled(!state.is_running, slider).changed() {
                        state.rebuild_scaling_steps();
                    }
                });

                ui.add_space(8.0);

                // Start / Stop
                ui.horizontal(|ui| {
                    if state.is_running {
                        let stop_btn = egui::Button::new(
                            RichText::new("Stop").size(12.0),
                        )
                        .fill(theme.semantic.error.to_color32());
                        if ui.add(stop_btn).clicked() {
                            state.commands.push(StressTestCommand::Stop);
                        }
                    } else {
                        let start_btn = egui::Button::new(
                            RichText::new("Start Test").size(12.0),
                        )
                        .fill(theme.semantic.success.to_color32());
                        if ui.add(start_btn).clicked() {
                            state.commands.push(StressTestCommand::Start);
                        }
                    }

                    if !state.results.is_empty() {
                        let clear_btn = egui::Button::new(
                            RichText::new("Clear Results").size(10.0),
                        );
                        if ui.add(clear_btn).clicked() {
                            state.commands.push(StressTestCommand::ClearResults);
                        }
                    }
                });

                // Progress bar
                if state.is_running && state.total_steps > 0 {
                    ui.add_space(8.0);
                    let progress = state.current_step as f32 / state.total_steps as f32;
                    ui.add(egui::ProgressBar::new(progress).text(format!(
                        "Step {}/{}",
                        state.current_step, state.total_steps
                    )));
                }

                ui.add_space(12.0);

                // Results table
                if !state.results.is_empty() {
                    render_results_table(ui, state, theme);
                }

                // Determinism result
                if matches!(state.test_type, StressTestType::Determinism) && state.determinism_hashes.len() >= 2 {
                    ui.add_space(12.0);
                    let pass = state.determinism_hashes[0] == state.determinism_hashes[1];
                    let (label, color) = if pass {
                        ("PASS - Deterministic", Color32::from_rgb(100, 200, 100))
                    } else {
                        ("FAIL - Non-deterministic", Color32::from_rgb(200, 100, 100))
                    };
                    ui.label(RichText::new(label).size(14.0).color(color).strong());

                    for (i, hash) in state.determinism_hashes.iter().enumerate() {
                        ui.label(
                            RichText::new(format!("Run {}: {:016x}", i + 1, hash))
                                .size(9.0)
                                .color(theme.text.secondary.to_color32())
                                .monospace(),
                        );
                    }
                }
            });
        });
}

fn render_results_table(
    ui: &mut egui::Ui,
    state: &StressTestState,
    theme: &Theme,
) {
    ui.label(
        RichText::new("Results")
            .size(12.0)
            .color(theme.text.muted.to_color32()),
    );
    ui.add_space(4.0);

    egui::Grid::new("stress_results")
        .num_columns(4)
        .spacing([12.0, 2.0])
        .striped(true)
        .show(ui, |ui| {
            // Header
            ui.label(RichText::new("Count").size(10.0).color(theme.text.secondary.to_color32()).strong());
            ui.label(RichText::new("FPS").size(10.0).color(theme.text.secondary.to_color32()).strong());
            ui.label(RichText::new("Phys ms").size(10.0).color(theme.text.secondary.to_color32()).strong());
            ui.label(RichText::new("Collisions").size(10.0).color(theme.text.secondary.to_color32()).strong());
            ui.end_row();

            for result in &state.results {
                ui.label(RichText::new(format!("{}", result.object_count)).size(10.0).color(theme.text.primary.to_color32()).monospace());

                let fps_color = if result.avg_fps >= 60.0 {
                    Color32::from_rgb(100, 200, 100)
                } else if result.avg_fps >= 30.0 {
                    Color32::from_rgb(200, 200, 100)
                } else {
                    Color32::from_rgb(200, 100, 100)
                };
                ui.label(RichText::new(format!("{:.0}", result.avg_fps)).size(10.0).color(fps_color).monospace());
                ui.label(RichText::new(format!("{:.2}", result.physics_ms)).size(10.0).color(theme.text.primary.to_color32()).monospace());
                ui.label(RichText::new(format!("{}", result.collision_pairs)).size(10.0).color(theme.text.primary.to_color32()).monospace());
                ui.end_row();
            }
        });

    // Simple bar chart of FPS vs object count
    if state.results.len() > 1 {
        ui.add_space(8.0);
        ui.label(
            RichText::new("FPS vs Object Count")
                .size(10.0)
                .color(theme.text.muted.to_color32()),
        );
        ui.add_space(4.0);

        let max_fps = state.results.iter().map(|r| r.avg_fps).fold(0.0f32, f32::max).max(1.0);
        let bar_height = 80.0;
        let width = ui.available_width().min(300.0);
        let bar_width = width / state.results.len() as f32 - 2.0;

        let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(width, bar_height), egui::Sense::hover());
        let painter = ui.painter();

        painter.rect_filled(rect, 2.0, Color32::from_rgba_unmultiplied(30, 30, 40, 180));

        for (i, result) in state.results.iter().enumerate() {
            let fraction = result.avg_fps / max_fps;
            let x = rect.min.x + i as f32 * (bar_width + 2.0);
            let h = fraction * (bar_height - 4.0);
            let bar_rect = egui::Rect::from_min_size(
                egui::pos2(x + 1.0, rect.max.y - h - 2.0),
                egui::vec2(bar_width, h),
            );
            let color = if result.avg_fps >= 60.0 {
                Color32::from_rgb(100, 200, 100)
            } else if result.avg_fps >= 30.0 {
                Color32::from_rgb(200, 200, 100)
            } else {
                Color32::from_rgb(200, 100, 100)
            };
            painter.rect_filled(bar_rect, 1.0, color);
        }
    }
}
