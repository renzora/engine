//! Physics debug panel

use bevy_egui::egui::{self, Color32, CursorIcon, RichText, Stroke, Vec2};
use renzora_theme::Theme;

use crate::state::{ColliderShapeType, PhysicsDebugState};

pub fn render_physics_debug_content(
    ui: &mut egui::Ui,
    state: &mut PhysicsDebugState,
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
                        ui.label(RichText::new("No physics bodies detected").size(14.0).color(theme.text.muted.to_color32()));
                        ui.add_space(8.0);
                        ui.label(RichText::new("Add RigidBody components to see physics stats").size(11.0).color(theme.text.muted.to_color32()));
                    });
                    return;
                }

                render_status_section(ui, state, theme);
                ui.add_space(16.0);
                render_body_counts_section(ui, state, theme);
                ui.add_space(16.0);
                render_collider_section(ui, state, theme);
                ui.add_space(16.0);
                render_step_time_section(ui, state, theme);
                ui.add_space(16.0);
                render_collision_pairs_section(ui, state, theme);
                ui.add_space(16.0);
                render_debug_toggles_section(ui, state, theme);
            });
        });
}

fn render_status_section(ui: &mut egui::Ui, state: &PhysicsDebugState, theme: &Theme) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Physics").size(12.0).color(theme.text.muted.to_color32()));
        ui.add_space(8.0);
        let (color, text) = if state.simulation_running {
            (theme.semantic.success.to_color32(), "Running")
        } else {
            (theme.semantic.warning.to_color32(), "Paused")
        };
        ui.label(RichText::new("\u{25cf}").size(11.0).color(color));
        ui.label(RichText::new(text).size(11.0).color(color));
    });
}

fn render_body_counts_section(ui: &mut egui::Ui, state: &PhysicsDebugState, theme: &Theme) {
    ui.label(RichText::new("Rigid Bodies").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{}", state.total_body_count())).size(24.0).color(theme.text.primary.to_color32()).strong());
        ui.label(RichText::new("total").size(11.0).color(theme.text.muted.to_color32()));
    });

    ui.add_space(8.0);

    let dynamic_color = Color32::from_rgb(100, 180, 220);
    let kinematic_color = Color32::from_rgb(200, 180, 100);
    let static_color = Color32::from_rgb(150, 150, 160);

    ui.horizontal(|ui| {
        ui.label(RichText::new("\u{25cf}").size(10.0).color(dynamic_color));
        ui.label(RichText::new(format!("Dynamic: {}", state.dynamic_body_count)).size(10.0).color(theme.text.secondary.to_color32()));
        ui.add_space(12.0);
        ui.label(RichText::new("\u{25cf}").size(10.0).color(kinematic_color));
        ui.label(RichText::new(format!("Kinematic: {}", state.kinematic_body_count)).size(10.0).color(theme.text.secondary.to_color32()));
        ui.add_space(12.0);
        ui.label(RichText::new("\u{25cf}").size(10.0).color(static_color));
        ui.label(RichText::new(format!("Static: {}", state.static_body_count)).size(10.0).color(theme.text.secondary.to_color32()));
    });

    ui.add_space(6.0);

    // Stacked bar
    let total = state.total_body_count().max(1);
    let bar_width = ui.available_width().min(250.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_width, 16.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 3.0, theme.surfaces.extreme.to_color32());

    if total > 0 {
        let dw = rect.width() * state.dynamic_body_count as f32 / total as f32;
        let kw = rect.width() * state.kinematic_body_count as f32 / total as f32;

        if dw > 0.0 {
            painter.rect_filled(egui::Rect::from_min_size(rect.min, Vec2::new(dw, rect.height())), 3.0, dynamic_color);
        }
        if kw > 0.0 {
            painter.rect_filled(egui::Rect::from_min_size(egui::pos2(rect.min.x + dw, rect.min.y), Vec2::new(kw, rect.height())), 0.0, kinematic_color);
        }
        let sw = rect.width() - dw - kw;
        if sw > 0.0 {
            painter.rect_filled(egui::Rect::from_min_size(egui::pos2(rect.min.x + dw + kw, rect.min.y), Vec2::new(sw, rect.height())), 0.0, static_color);
        }
    }
}

fn render_collider_section(ui: &mut egui::Ui, state: &PhysicsDebugState, theme: &Theme) {
    ui.label(RichText::new("Colliders").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{}", state.collider_count)).size(20.0).color(theme.text.primary.to_color32()).strong());
        ui.label(RichText::new("total").size(11.0).color(theme.text.muted.to_color32()));
    });

    if !state.colliders_by_type.is_empty() {
        ui.add_space(6.0);
        let mut sorted: Vec<_> = state.colliders_by_type.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));

        for (shape_type, count) in sorted.iter().take(6) {
            ui.horizontal(|ui| {
                ui.label(RichText::new("\u{25a0}").size(10.0).color(shape_type_color(**shape_type)));
                ui.label(RichText::new(format!("{}: {}", shape_type, count)).size(10.0).color(theme.text.secondary.to_color32()));
            });
        }
    }
}

fn render_step_time_section(ui: &mut egui::Ui, state: &PhysicsDebugState, theme: &Theme) {
    ui.label(RichText::new("Step Time").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    let step_color = step_time_to_color(state.step_time_ms);
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{:.2}", state.step_time_ms)).size(20.0).color(step_color).strong());
        ui.label(RichText::new("ms").size(11.0).color(theme.text.muted.to_color32()));
        ui.add_space(12.0);
        ui.label(RichText::new(format!("avg: {:.2}ms", state.avg_step_time_ms)).size(10.0).color(theme.text.secondary.to_color32()));
    });

    ui.add_space(6.0);

    // Step time graph
    let height = 40.0;
    let available_width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(available_width, height), egui::Sense::hover());
    if !ui.is_rect_visible(rect) { return; }

    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, theme.surfaces.extreme.to_color32());
    painter.rect_stroke(rect, 2.0, Stroke::new(1.0, theme.widgets.border.to_color32()), egui::StrokeKind::Inside);

    let data: Vec<f32> = state.step_time_history.iter().copied().collect();
    if data.is_empty() { return; }

    let max_val = data.iter().copied().fold(1.0_f32, f32::max) * 1.2;
    if max_val <= 0.0 { return; }

    let step = rect.width() / data.len().max(1) as f32;
    let line_color = theme.semantic.success.to_color32();

    let points: Vec<egui::Pos2> = data.iter().enumerate().map(|(i, &val)| {
        let x = rect.min.x + i as f32 * step;
        let normalized = (val / max_val).clamp(0.0, 1.0);
        let y = rect.max.y - normalized * rect.height();
        egui::pos2(x, y)
    }).collect();

    if points.len() >= 2 {
        let success = theme.semantic.success.to_color32();
        let fill_color = Color32::from_rgba_unmultiplied(success.r(), success.g(), success.b(), 30);
        let mut fill_points = points.clone();
        fill_points.push(egui::pos2(rect.max.x, rect.max.y));
        fill_points.push(egui::pos2(rect.min.x, rect.max.y));
        painter.add(egui::Shape::convex_polygon(fill_points, fill_color, Stroke::NONE));
        painter.add(egui::Shape::line(points, Stroke::new(1.5, line_color)));
    }
}

fn render_collision_pairs_section(ui: &mut egui::Ui, state: &mut PhysicsDebugState, theme: &Theme) {
    let header = ui.horizontal(|ui| {
        let icon = if state.show_collision_pairs { "\u{25be}" } else { "\u{25b8}" };
        ui.label(RichText::new(icon).size(12.0).color(theme.text.secondary.to_color32()));
        ui.label(RichText::new(format!("Collision Pairs ({})", state.collision_pair_count)).size(12.0).color(theme.text.primary.to_color32()));
    });

    let interact = header.response.interact(egui::Sense::click());
    if interact.hovered() { ui.ctx().set_cursor_icon(CursorIcon::PointingHand); }
    if interact.clicked() { state.show_collision_pairs = !state.show_collision_pairs; }

    if state.show_collision_pairs {
        ui.add_space(4.0);
        egui::Frame::NONE
            .fill(theme.surfaces.faint.to_color32())
            .corner_radius(4.0)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                if state.collision_pairs.is_empty() {
                    ui.label(RichText::new("No active collisions").size(10.0).color(theme.text.muted.to_color32()));
                } else {
                    for pair in state.collision_pairs.iter().take(10) {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{:?}", pair.entity_a)).size(9.0).color(theme.text.secondary.to_color32()).monospace());
                            ui.label(RichText::new("\u{2194}").size(9.0).color(theme.text.muted.to_color32()));
                            ui.label(RichText::new(format!("{:?}", pair.entity_b)).size(9.0).color(theme.text.secondary.to_color32()).monospace());
                            ui.label(RichText::new(format!("({} contacts)", pair.contact_count)).size(9.0).color(theme.text.muted.to_color32()));
                        });
                    }
                    if state.collision_pair_count > 10 {
                        ui.label(RichText::new(format!("... and {} more", state.collision_pair_count - 10)).size(9.0).color(theme.text.muted.to_color32()));
                    }
                }
            });
    }
}

fn render_debug_toggles_section(ui: &mut egui::Ui, state: &mut PhysicsDebugState, theme: &Theme) {
    ui.label(RichText::new("Debug Visualization").size(12.0).color(theme.text.muted.to_color32()));
    ui.add_space(4.0);

    egui::Frame::NONE
        .fill(theme.surfaces.faint.to_color32())
        .corner_radius(4.0)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.checkbox(&mut state.debug_toggles.show_colliders, "Show Colliders");
            ui.checkbox(&mut state.debug_toggles.show_contacts, "Show Contacts");
            ui.checkbox(&mut state.debug_toggles.show_aabbs, "Show AABBs");
            ui.checkbox(&mut state.debug_toggles.show_velocities, "Show Velocities");
            ui.checkbox(&mut state.debug_toggles.show_center_of_mass, "Show Center of Mass");
            ui.checkbox(&mut state.debug_toggles.show_joints, "Show Joints");
        });
}

fn shape_type_color(shape: ColliderShapeType) -> Color32 {
    match shape {
        ColliderShapeType::Sphere => Color32::from_rgb(100, 180, 220),
        ColliderShapeType::Box => Color32::from_rgb(180, 140, 200),
        ColliderShapeType::Capsule => Color32::from_rgb(140, 200, 140),
        ColliderShapeType::Cylinder => Color32::from_rgb(200, 160, 100),
        ColliderShapeType::Cone => Color32::from_rgb(200, 120, 120),
        ColliderShapeType::ConvexHull => Color32::from_rgb(120, 180, 180),
        ColliderShapeType::TriMesh => Color32::from_rgb(180, 180, 120),
        ColliderShapeType::HeightField => Color32::from_rgb(120, 140, 180),
        ColliderShapeType::Compound => Color32::from_rgb(180, 140, 160),
        ColliderShapeType::Unknown => Color32::from_gray(120),
    }
}

fn step_time_to_color(ms: f32) -> Color32 {
    if ms <= 2.0 { Color32::from_rgb(100, 200, 100) }
    else if ms <= 5.0 { Color32::from_rgb(200, 200, 100) }
    else if ms <= 10.0 { Color32::from_rgb(200, 150, 80) }
    else { Color32::from_rgb(200, 100, 100) }
}
