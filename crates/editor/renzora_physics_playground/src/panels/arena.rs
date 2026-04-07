//! Arena Presets panel — spawn pre-built arena environments

use renzora::bevy_egui::egui::{self, CursorIcon, RichText, Vec2};
use renzora::theme::Theme;

use crate::state::{ArenaCommand, ArenaPresetsState, ArenaType};

pub fn render_arena_presets_content(
    ui: &mut egui::Ui,
    state: &mut ArenaPresetsState,
    theme: &Theme,
) {
    let accent = theme.semantic.accent.to_color32();
    let text_primary = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();

    let padding = 6.0;
    let spacing = 4.0;

    ui.add_space(padding);

    // Header
    ui.horizontal(|ui| {
        ui.add_space(padding);
        ui.label(RichText::new("Arenas").size(13.0).color(text_primary).strong());
        if state.has_active_arena {
            ui.label(RichText::new(format!("({} entities)", state.arena_entity_count)).size(10.0).color(text_muted));
        }
    });

    ui.add_space(spacing);

    // Scale + Clear row
    ui.horizontal(|ui| {
        ui.add_space(padding);
        ui.label(RichText::new("Scale").size(10.0).color(text_muted));
        ui.add(egui::Slider::new(&mut state.scale, 0.5..=5.0));
        if state.has_active_arena {
            if ui.add(egui::Button::new(RichText::new("Clear").size(10.0))
                .fill(theme.semantic.error.to_color32())).clicked()
            {
                state.commands.push(ArenaCommand::Clear);
            }
        }
    });

    ui.add_space(spacing);

    // Responsive grid
    egui::ScrollArea::vertical().show(ui, |ui| {
        let available_width = ui.available_width() - padding * 2.0;
        let min_tile = 72.0;
        let max_tile = 100.0;
        let cols = ((available_width + spacing) / (min_tile + spacing)).floor().max(1.0) as usize;
        let tile_size = ((available_width - spacing * (cols as f32 - 1.0)) / cols as f32)
            .clamp(min_tile, max_tile);
        let label_height = 16.0;

        ui.add_space(2.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = spacing;

            let arenas = ArenaType::ALL;
            let rows = (arenas.len() + cols - 1) / cols;
            for row in 0..rows {
                ui.horizontal(|ui| {
                    ui.add_space(padding);
                    ui.spacing_mut().item_spacing.x = spacing;
                    for col in 0..cols {
                        let idx = row * cols + col;
                        if idx >= arenas.len() {
                            break;
                        }
                        let arena = &arenas[idx];
                        let selected = state.arena_type == *arena;
                        let total_height = tile_size + label_height;
                        let (rect, response) = ui.allocate_exact_size(
                            Vec2::new(tile_size, total_height),
                            egui::Sense::click(),
                        );

                        let hovered = response.hovered();
                        let bg = if selected {
                            accent.linear_multiply(0.3)
                        } else if hovered {
                            theme.widgets.hovered_bg.to_color32()
                        } else {
                            theme.surfaces.faint.to_color32()
                        };

                        ui.painter().rect_filled(rect, 6.0, bg);

                        if selected {
                            ui.painter().rect_stroke(
                                rect, 6.0,
                                egui::Stroke::new(1.5, accent),
                                egui::StrokeKind::Outside,
                            );
                        } else if hovered {
                            ui.painter().rect_stroke(
                                rect, 6.0,
                                egui::Stroke::new(1.0, accent.linear_multiply(0.6)),
                                egui::StrokeKind::Outside,
                            );
                        }

                        if hovered {
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        }

                        // Icon
                        let icon_rect = egui::Rect::from_min_size(
                            rect.min,
                            Vec2::new(tile_size, tile_size),
                        );
                        ui.painter().text(
                            icon_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            arena.icon(),
                            egui::FontId::proportional(28.0),
                            if selected || hovered { accent } else { text_primary },
                        );

                        // Label
                        ui.painter().text(
                            egui::pos2(rect.center().x, rect.max.y - label_height / 2.0),
                            egui::Align2::CENTER_CENTER,
                            arena.label(),
                            egui::FontId::proportional(10.0),
                            text_muted,
                        );

                        response.clone().on_hover_text(arena.description());

                        if response.clicked() {
                            let changed = state.arena_type != *arena;
                            state.arena_type = *arena;
                            if changed || !state.has_active_arena {
                                state.commands.push(ArenaCommand::Spawn);
                            }
                        }
                    }
                });
            }
        });

        ui.add_space(padding);

        ui.label(
            RichText::new("Arenas work with Playground, Scenarios, or Stress Test")
                .size(9.0)
                .color(text_muted)
                .italics(),
        );

        ui.add_space(padding);
    });
}
