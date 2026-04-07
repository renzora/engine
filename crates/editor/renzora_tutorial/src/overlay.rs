//! Overlay rendering — animated arrows, pulsing highlights, tooltip cards, and backdrop.

use renzora::bevy_egui::egui::{self, Color32, FontId, Pos2, Rect, Stroke, StrokeKind, Vec2};
use renzora::theme::Theme;

use crate::steps::{ArrowDirection, TutorialStep, TutorialTarget};
use crate::TutorialState;

// ── Constants ──────────────────────────────────────────────────────────────────

const CARD_WIDTH: f32 = 360.0;
const CARD_PADDING: i8 = 20;
const CARD_CORNER: u8 = 10;
const BACKDROP_ALPHA: u8 = 120;
const HIGHLIGHT_CORNER: f32 = 6.0;
const ARROW_SIZE: f32 = 14.0;
const ARROW_GAP: f32 = 12.0;

// ── Public API ─────────────────────────────────────────────────────────────────

/// Render the full tutorial overlay for the current step.
/// Returns `TutorialAction` indicating what the user wants to do.
pub fn render_overlay(
    ctx: &egui::Context,
    state: &TutorialState,
    step: &TutorialStep,
    theme: &Theme,
    panel_rects: &std::collections::HashMap<String, Rect>,
) -> TutorialAction {
    let time = state.elapsed;
    let screen = ctx.input(|i| i.viewport_rect());
    let accent = theme.semantic.accent.to_color32();

    // Compute target rect (the area to highlight)
    let target_rect = resolve_target_rect(&step.target, panel_rects, screen);

    // 1) Semi-transparent backdrop with cutout
    draw_backdrop(ctx, screen, target_rect, time);

    // 2) Pulsing highlight border around target
    if let Some(rect) = target_rect {
        draw_highlight(ctx, rect, accent, time);
    }

    // 3) Position the card near the target
    let card_pos = compute_card_position(screen, target_rect, &step.arrow);

    // 4) Animated arrow from card toward target
    if let Some(rect) = target_rect {
        if !matches!(step.arrow, ArrowDirection::None) {
            draw_arrow(ctx, card_pos, rect, &step.arrow, accent, time);
        }
    }

    // 5) Tooltip card with title, description, step counter, buttons
    draw_card(ctx, state, step, card_pos, accent, theme)
}

/// What the user chose after interacting with the card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TutorialAction {
    None,
    Next,
    Back,
    Skip,
}

// ── Target resolution ──────────────────────────────────────────────────────────

fn resolve_target_rect(
    target: &TutorialTarget,
    panel_rects: &std::collections::HashMap<String, Rect>,
    screen: Rect,
) -> Option<Rect> {
    match target {
        TutorialTarget::Panel(id) => panel_rects.get(*id).copied(),
        TutorialTarget::PanelTopRight(id) => panel_rects.get(*id).map(|r| {
            Rect::from_min_size(
                Pos2::new(r.max.x - 48.0, r.min.y),
                Vec2::new(48.0, 32.0),
            )
        }),
        TutorialTarget::PanelTop(id) => panel_rects.get(*id).map(|r| {
            Rect::from_min_size(r.min, Vec2::new(r.width(), 32.0))
        }),
        TutorialTarget::PanelBottom(id) => panel_rects.get(*id).map(|r| {
            Rect::from_min_size(
                Pos2::new(r.min.x, r.max.y - 36.0),
                Vec2::new(r.width(), 36.0),
            )
        }),
        TutorialTarget::TitleBar => {
            Some(Rect::from_min_size(screen.min, Vec2::new(screen.width(), 56.0)))
        }
        TutorialTarget::Center => None,
    }
}

// ── Backdrop ───────────────────────────────────────────────────────────────────

fn draw_backdrop(ctx: &egui::Context, screen: Rect, cutout: Option<Rect>, time: f64) {
    // Fade in over 0.3s
    let fade = (time / 0.3).min(1.0) as f32;
    let alpha = (BACKDROP_ALPHA as f32 * fade) as u8;
    let color = Color32::from_rgba_unmultiplied(0, 0, 0, alpha);

    egui::Area::new(egui::Id::new("tutorial_backdrop"))
        .fixed_pos(screen.min)
        .order(egui::Order::Tooltip)
        .interactable(false)
        .show(ctx, |ui| {
            let painter = ui.painter();

            if let Some(cut) = cutout {
                // Draw four rectangles around the cutout to form the backdrop
                let margin = 4.0;
                let cut = cut.expand(margin);

                // Top strip
                if cut.min.y > screen.min.y {
                    painter.rect_filled(
                        Rect::from_min_max(screen.min, Pos2::new(screen.max.x, cut.min.y)),
                        0.0,
                        color,
                    );
                }
                // Bottom strip
                if cut.max.y < screen.max.y {
                    painter.rect_filled(
                        Rect::from_min_max(Pos2::new(screen.min.x, cut.max.y), screen.max),
                        0.0,
                        color,
                    );
                }
                // Left strip (between top and bottom strips)
                if cut.min.x > screen.min.x {
                    painter.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(screen.min.x, cut.min.y),
                            Pos2::new(cut.min.x, cut.max.y),
                        ),
                        0.0,
                        color,
                    );
                }
                // Right strip
                if cut.max.x < screen.max.x {
                    painter.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(cut.max.x, cut.min.y),
                            Pos2::new(screen.max.x, cut.max.y),
                        ),
                        0.0,
                        color,
                    );
                }
            } else {
                // No cutout — full backdrop
                painter.rect_filled(screen, 0.0, color);
            }
        });
}

// ── Pulsing highlight ──────────────────────────────────────────────────────────

fn draw_highlight(ctx: &egui::Context, rect: Rect, accent: Color32, time: f64) {
    let pulse = ((time * 2.5).sin() * 0.5 + 0.5) as f32; // 0..1 pulsing
    let alpha = (120.0 + pulse * 135.0) as u8;
    let glow_color = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), alpha);

    let margin = 4.0;
    let expanded = rect.expand(margin);

    egui::Area::new(egui::Id::new("tutorial_highlight"))
        .fixed_pos(expanded.min)
        .order(egui::Order::Tooltip)
        .interactable(false)
        .show(ctx, |ui| {
            let painter = ui.painter();

            // Outer glow
            let glow_expand = 2.0 + pulse * 3.0;
            let glow_rect = expanded.expand(glow_expand);
            let glow_alpha = (40.0 + pulse * 50.0) as u8;
            let outer_glow =
                Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), glow_alpha);
            painter.rect_stroke(
                glow_rect,
                (HIGHLIGHT_CORNER + glow_expand) as u8,
                Stroke::new(2.0, outer_glow),
                StrokeKind::Outside,
            );

            // Main border
            painter.rect_stroke(
                expanded,
                HIGHLIGHT_CORNER as u8,
                Stroke::new(2.0, glow_color),
                StrokeKind::Outside,
            );
        });
}

// ── Card positioning ───────────────────────────────────────────────────────────

fn compute_card_position(screen: Rect, target: Option<Rect>, arrow: &ArrowDirection) -> Pos2 {
    let card_h_estimate = 220.0; // approximate card height

    if let Some(rect) = target {
        match arrow {
            ArrowDirection::Left => {
                // Card to the right of target
                let x = (rect.max.x + ARROW_GAP + ARROW_SIZE + 8.0).min(screen.max.x - CARD_WIDTH - 16.0);
                let y = (rect.center().y - card_h_estimate / 2.0).clamp(screen.min.y + 8.0, screen.max.y - card_h_estimate - 8.0);
                Pos2::new(x, y)
            }
            ArrowDirection::Right => {
                // Card to the left of target
                let x = (rect.min.x - ARROW_GAP - ARROW_SIZE - CARD_WIDTH - 8.0).max(screen.min.x + 16.0);
                let y = (rect.center().y - card_h_estimate / 2.0).clamp(screen.min.y + 8.0, screen.max.y - card_h_estimate - 8.0);
                Pos2::new(x, y)
            }
            ArrowDirection::Up => {
                // Card below target
                let x = (rect.center().x - CARD_WIDTH / 2.0).clamp(screen.min.x + 16.0, screen.max.x - CARD_WIDTH - 16.0);
                let y = (rect.max.y + ARROW_GAP + ARROW_SIZE + 8.0).min(screen.max.y - card_h_estimate - 8.0);
                Pos2::new(x, y)
            }
            ArrowDirection::Down => {
                // Card above target
                let x = (rect.center().x - CARD_WIDTH / 2.0).clamp(screen.min.x + 16.0, screen.max.x - CARD_WIDTH - 16.0);
                let y = (rect.min.y - ARROW_GAP - ARROW_SIZE - card_h_estimate - 8.0).max(screen.min.y + 8.0);
                Pos2::new(x, y)
            }
            ArrowDirection::None => center_card(screen, card_h_estimate),
        }
    } else {
        center_card(screen, card_h_estimate)
    }
}

fn center_card(screen: Rect, card_h: f32) -> Pos2 {
    Pos2::new(
        screen.center().x - CARD_WIDTH / 2.0,
        screen.center().y - card_h / 2.0,
    )
}

// ── Arrow drawing ──────────────────────────────────────────────────────────────

fn draw_arrow(
    ctx: &egui::Context,
    card_pos: Pos2,
    target_rect: Rect,
    direction: &ArrowDirection,
    accent: Color32,
    time: f64,
) {
    // Bouncing offset along the arrow axis
    let bounce = ((time * 3.0).sin() * 4.0) as f32;

    egui::Area::new(egui::Id::new("tutorial_arrow"))
        .fixed_pos(Pos2::ZERO)
        .order(egui::Order::Tooltip)
        .interactable(false)
        .show(ctx, |ui| {
            let painter = ui.painter();
            let card_rect = Rect::from_min_size(card_pos, Vec2::new(CARD_WIDTH, 200.0));

            let (start, end) = match direction {
                ArrowDirection::Left => {
                    // Arrow goes from card left edge → target right edge
                    let start = Pos2::new(card_pos.x - 4.0, card_rect.center().y);
                    let end = Pos2::new(target_rect.max.x + 6.0 + bounce, target_rect.center().y);
                    (start, end)
                }
                ArrowDirection::Right => {
                    let start = Pos2::new(card_pos.x + CARD_WIDTH + 4.0, card_rect.center().y);
                    let end = Pos2::new(target_rect.min.x - 6.0 - bounce, target_rect.center().y);
                    (start, end)
                }
                ArrowDirection::Up => {
                    let start = Pos2::new(card_rect.center().x, card_pos.y - 4.0);
                    let end = Pos2::new(target_rect.center().x, target_rect.max.y + 6.0 + bounce);
                    (start, end)
                }
                ArrowDirection::Down => {
                    let start = Pos2::new(card_rect.center().x, card_pos.y + 200.0 + 4.0);
                    let end = Pos2::new(target_rect.center().x, target_rect.min.y - 6.0 - bounce);
                    (start, end)
                }
                ArrowDirection::None => return,
            };

            // Draw line
            painter.line_segment([start, end], Stroke::new(2.0, accent));

            // Draw arrowhead at the end
            let dir = (end - start).normalized();
            let perp = Vec2::new(-dir.y, dir.x);
            let head_len = ARROW_SIZE;
            let head_width = 6.0;

            let tip = end;
            let left = tip - dir * head_len + perp * head_width;
            let right = tip - dir * head_len - perp * head_width;

            painter.add(egui::Shape::convex_polygon(
                vec![tip, left, right],
                accent,
                Stroke::NONE,
            ));
        });
}

// ── Tooltip card ───────────────────────────────────────────────────────────────

fn draw_card(
    ctx: &egui::Context,
    state: &TutorialState,
    step: &TutorialStep,
    pos: Pos2,
    accent: Color32,
    theme: &Theme,
) -> TutorialAction {
    let mut action = TutorialAction::None;
    let total = state.total_steps;
    let current = state.current_step;

    egui::Area::new(egui::Id::new("tutorial_card"))
        .fixed_pos(pos)
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            let bg = theme.surfaces.popup.to_color32();
            let border = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 160);

            egui::Frame::NONE
                .fill(bg)
                .stroke(Stroke::new(1.5, border))
                .inner_margin(egui::Margin::same(CARD_PADDING))
                .corner_radius(egui::CornerRadius::same(CARD_CORNER))
                .shadow(egui::epaint::Shadow {
                    offset: [0, 4],
                    blur: 16,
                    spread: 2,
                    color: Color32::from_rgba_unmultiplied(0, 0, 0, 80),
                })
                .show(ui, |ui| {
                    ui.set_width(CARD_WIDTH - CARD_PADDING as f32 * 2.0);

                    // Step indicator: "Step 3 of 16"
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("Step {} of {}", current + 1, total))
                                .font(FontId::proportional(11.0))
                                .color(theme.text.muted.to_color32()),
                        );
                    });

                    ui.add_space(6.0);

                    // Title
                    ui.label(
                        egui::RichText::new(step.title)
                            .font(FontId::proportional(17.0))
                            .color(theme.text.heading.to_color32())
                            .strong(),
                    );

                    ui.add_space(8.0);

                    // Description
                    ui.label(
                        egui::RichText::new(step.description)
                            .font(FontId::proportional(13.0))
                            .color(theme.text.primary.to_color32()),
                    );

                    ui.add_space(14.0);

                    // Progress dots
                    ui.horizontal(|ui| {
                        let dot_radius = 3.0;
                        let dot_spacing = 10.0;
                        let painter = ui.painter();
                        let start_x = ui.cursor().min.x;
                        let y = ui.cursor().min.y + dot_radius + 2.0;

                        // Show a window of dots around the current step
                        let window = 12;
                        let half = window / 2;
                        let dot_start = if total <= window {
                            0
                        } else {
                            current.saturating_sub(half).min(total - window)
                        };
                        let dot_end = (dot_start + window).min(total);

                        for i in dot_start..dot_end {
                            let x = start_x + (i - dot_start) as f32 * dot_spacing + dot_radius;
                            if i == current {
                                painter.circle_filled(Pos2::new(x, y), dot_radius + 1.0, accent);
                            } else {
                                painter.circle_filled(
                                    Pos2::new(x, y),
                                    dot_radius,
                                    Color32::from_rgb(80, 80, 90),
                                );
                            }
                        }

                        // Reserve space
                        ui.allocate_space(Vec2::new(
                            (dot_end - dot_start) as f32 * dot_spacing,
                            dot_radius * 2.0 + 4.0,
                        ));
                    });

                    ui.add_space(10.0);

                    // Buttons
                    ui.horizontal(|ui| {
                        // Skip button (left-aligned, subtle)
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new("Skip Tutorial")
                                        .font(FontId::proportional(12.0))
                                        .color(theme.text.muted.to_color32()),
                                )
                                .frame(false),
                            )
                            .clicked()
                        {
                            action = TutorialAction::Skip;
                        }

                        // Push buttons to the right
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Next / Finish button
                            let is_last = current + 1 >= total;
                            let next_label = if is_last { "Finish" } else { "Next" };
                            let next_icon = if is_last {
                                renzora::egui_phosphor::regular::CHECK
                            } else {
                                renzora::egui_phosphor::regular::ARROW_RIGHT
                            };

                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(format!("{}  {}", next_label, next_icon))
                                            .font(FontId::proportional(13.0))
                                            .color(Color32::WHITE),
                                    )
                                    .fill(accent)
                                    .corner_radius(egui::CornerRadius::same(6))
                                    .min_size(Vec2::new(80.0, 28.0)),
                                )
                                .clicked()
                            {
                                action = TutorialAction::Next;
                            }

                            // Back button
                            if current > 0 {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(format!(
                                                "{}  Back",
                                                renzora::egui_phosphor::regular::ARROW_LEFT
                                            ))
                                            .font(FontId::proportional(13.0))
                                            .color(theme.text.primary.to_color32()),
                                        )
                                        .corner_radius(egui::CornerRadius::same(6))
                                        .min_size(Vec2::new(70.0, 28.0)),
                                    )
                                    .clicked()
                                {
                                    action = TutorialAction::Back;
                                }
                            }
                        });
                    });
                });
        });

    action
}
