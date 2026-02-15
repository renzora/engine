//! ECS Statistics panel for entity/component/archetype monitoring

use bevy_egui::egui::{self, Color32, CursorIcon, RichText, Stroke, Vec2};

use crate::core::EcsStatsState;
use renzora_theme::Theme;

/// Render the ECS statistics panel content
pub fn render_ecs_stats_content(
    ui: &mut egui::Ui,
    state: &mut EcsStatsState,
    theme: &Theme,
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());

                // Entity Count Section
                render_entity_count_section(ui, state, theme);

                ui.add_space(16.0);

                // Archetype Section (collapsible)
                render_archetype_section(ui, state, theme);

                ui.add_space(12.0);

                // Component Types Section (collapsible)
                render_component_section(ui, state, theme);

                ui.add_space(12.0);

                // Resources Section (collapsible)
                render_resources_section(ui, state, theme);
            });
        });
}

fn render_entity_count_section(ui: &mut egui::Ui, state: &EcsStatsState, theme: &Theme) {
    ui.label(RichText::new("Entities").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    // Large entity count display
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{}", state.entity_count))
                .size(32.0)
                .color(theme.text.primary.to_color32())
                .strong(),
        );
        ui.label(RichText::new("entities").size(12.0).color(theme.text.muted.to_color32()));
    });

    ui.add_space(4.0);

    // Archetype count
    ui.horizontal(|ui| {
        ui.label(RichText::new("Archetypes:").size(11.0).color(theme.text.secondary.to_color32()));
        ui.label(
            RichText::new(format!("{}", state.archetype_count))
                .size(11.0)
                .color(theme.text.primary.to_color32()),
        );
        ui.add_space(12.0);
        ui.label(RichText::new("Component Types:").size(11.0).color(theme.text.secondary.to_color32()));
        ui.label(
            RichText::new(format!("{}", state.component_stats.len()))
                .size(11.0)
                .color(theme.text.primary.to_color32()),
        );
    });

    ui.add_space(8.0);

    // Entity count graph
    render_entity_graph(ui, state, theme);
}

fn render_entity_graph(ui: &mut egui::Ui, state: &EcsStatsState, _theme: &Theme) {
    let height = 50.0;
    let available_width = ui.available_width();
    let size = Vec2::new(available_width, height);
    let (rect, _response) = ui.allocate_exact_size(size, egui::Sense::hover());

    if !ui.is_rect_visible(rect) {
        return;
    }

    let painter = ui.painter();

    // Background
    painter.rect_filled(rect, 2.0, Color32::from_rgb(30, 32, 36));
    painter.rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_rgb(50, 52, 58)), egui::StrokeKind::Inside);

    let data: Vec<f32> = state.entity_count_history.iter().copied().collect();
    if data.is_empty() {
        return;
    }

    let max_val = data.iter().copied().fold(100.0_f32, f32::max) * 1.2;
    let min_val = 0.0_f32;
    let range = max_val - min_val;
    if range <= 0.0 {
        return;
    }

    // Data line
    let step = rect.width() / data.len().max(1) as f32;
    let line_color = Color32::from_rgb(100, 180, 220);

    let points: Vec<egui::Pos2> = data
        .iter()
        .enumerate()
        .map(|(i, &val)| {
            let x = rect.min.x + i as f32 * step;
            let normalized = ((val - min_val) / range).clamp(0.0, 1.0);
            let y = rect.max.y - normalized * rect.height();
            egui::pos2(x, y)
        })
        .collect();

    if points.len() >= 2 {
        // Filled area
        let mut fill_points = points.clone();
        fill_points.push(egui::pos2(rect.max.x, rect.max.y));
        fill_points.push(egui::pos2(rect.min.x, rect.max.y));
        let fill_color = Color32::from_rgba_unmultiplied(100, 180, 220, 30);
        painter.add(egui::Shape::convex_polygon(fill_points, fill_color, Stroke::NONE));

        // Line
        painter.add(egui::Shape::line(points, Stroke::new(1.5, line_color)));
    }
}

fn render_archetype_section(ui: &mut egui::Ui, state: &mut EcsStatsState, theme: &Theme) {
    let header_response = ui.horizontal(|ui| {
        let icon = if state.show_archetypes { "\u{f107}" } else { "\u{f105}" };
        ui.label(RichText::new(icon).size(12.0).color(theme.text.secondary.to_color32()));
        ui.label(
            RichText::new(format!("Archetypes ({})", state.archetype_count))
                .size(12.0)
                .color(theme.text.primary.to_color32()),
        );
    });

    let header_interact = header_response.response.interact(egui::Sense::click());
    if header_interact.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if header_interact.clicked() {
        state.show_archetypes = !state.show_archetypes;
    }

    if state.show_archetypes {
        ui.add_space(4.0);
        egui::Frame::NONE
            .fill(Color32::from_rgb(35, 37, 42))
            .corner_radius(4.0)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                if state.top_archetypes.is_empty() {
                    ui.label(RichText::new("No archetypes").size(11.0).color(theme.text.muted.to_color32()));
                } else {
                    for (i, archetype) in state.top_archetypes.iter().take(20).enumerate() {
                        ui.horizontal(|ui| {
                            // Entity count with bar
                            ui.label(
                                RichText::new(format!("{:>5}", archetype.entity_count))
                                    .size(10.0)
                                    .color(theme.text.primary.to_color32())
                                    .monospace(),
                            );

                            // Component list (truncated)
                            let components_str = if archetype.components.len() > 3 {
                                format!(
                                    "{}, ... +{}",
                                    archetype.components.iter().take(3).map(|s| short_component_name(s)).collect::<Vec<_>>().join(", "),
                                    archetype.components.len() - 3
                                )
                            } else {
                                archetype.components.iter().map(|s| short_component_name(s)).collect::<Vec<_>>().join(", ")
                            };

                            ui.label(
                                RichText::new(components_str)
                                    .size(10.0)
                                    .color(theme.text.secondary.to_color32()),
                            );
                        });

                        if i < state.top_archetypes.len() - 1 && i < 19 {
                            ui.separator();
                        }
                    }
                }
            });
    }
}

fn render_component_section(ui: &mut egui::Ui, state: &mut EcsStatsState, theme: &Theme) {
    let header_response = ui.horizontal(|ui| {
        let icon = if state.show_components { "\u{f107}" } else { "\u{f105}" };
        ui.label(RichText::new(icon).size(12.0).color(theme.text.secondary.to_color32()));
        ui.label(
            RichText::new(format!("Component Types ({})", state.component_stats.len()))
                .size(12.0)
                .color(theme.text.primary.to_color32()),
        );
    });

    let header_interact = header_response.response.interact(egui::Sense::click());
    if header_interact.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if header_interact.clicked() {
        state.show_components = !state.show_components;
    }

    if state.show_components {
        ui.add_space(4.0);
        egui::Frame::NONE
            .fill(Color32::from_rgb(35, 37, 42))
            .corner_radius(4.0)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                if state.component_stats.is_empty() {
                    ui.label(RichText::new("No components").size(11.0).color(theme.text.muted.to_color32()));
                } else {
                    // Header
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Instances").size(9.0).color(theme.text.muted.to_color32()));
                        ui.add_space(8.0);
                        ui.label(RichText::new("Component").size(9.0).color(theme.text.muted.to_color32()));
                    });
                    ui.separator();

                    for (i, comp) in state.component_stats.iter().take(30).enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{:>6}", comp.instance_count))
                                    .size(10.0)
                                    .color(theme.text.primary.to_color32())
                                    .monospace(),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(short_component_name(&comp.name))
                                    .size(10.0)
                                    .color(theme.text.secondary.to_color32()),
                            );
                        });

                        if i < 29 && i < state.component_stats.len() - 1 {
                            ui.add_space(2.0);
                        }
                    }

                    if state.component_stats.len() > 30 {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(format!("... and {} more", state.component_stats.len() - 30))
                                .size(10.0)
                                .color(theme.text.muted.to_color32()),
                        );
                    }
                }
            });
    }
}

fn render_resources_section(ui: &mut egui::Ui, state: &mut EcsStatsState, theme: &Theme) {
    let header_response = ui.horizontal(|ui| {
        let icon = if state.show_resources { "\u{f107}" } else { "\u{f105}" };
        ui.label(RichText::new(icon).size(12.0).color(theme.text.secondary.to_color32()));
        ui.label(
            RichText::new(format!("Resources ({})", state.resources.len()))
                .size(12.0)
                .color(theme.text.primary.to_color32()),
        );
    });

    let header_interact = header_response.response.interact(egui::Sense::click());
    if header_interact.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    if header_interact.clicked() {
        state.show_resources = !state.show_resources;
    }

    if state.show_resources {
        ui.add_space(4.0);
        egui::Frame::NONE
            .fill(Color32::from_rgb(35, 37, 42))
            .corner_radius(4.0)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                if state.resources.is_empty() {
                    ui.label(RichText::new("Resource tracking not enabled").size(11.0).color(theme.text.muted.to_color32()));
                    ui.label(
                        RichText::new("Use world.iter_resources() for full list")
                            .size(10.0)
                            .color(theme.text.muted.to_color32()),
                    );
                } else {
                    for resource in state.resources.iter().take(50) {
                        ui.label(
                            RichText::new(short_component_name(resource))
                                .size(10.0)
                                .color(theme.text.secondary.to_color32()),
                        );
                    }
                }
            });
    }
}

/// Shorten a fully-qualified component name to just the type name
fn short_component_name(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name)
}
