//! Gauges debug panel — live view of all entities with attributes.
//!
//! Shows every entity that has the `Gauges` component with all its
//! attribute values displayed as gauge bars, useful for debugging
//! RPG systems at runtime.

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_editor_framework::{EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;

use crate::GaugesSnapshot;

// ── State ─────────────────────────────────────────────────────────────────

#[derive(Default)]
struct PanelState {
    filter: String,
}

// ── Panel ─────────────────────────────────────────────────────────────────

pub struct GaugesPanel {
    state: RwLock<PanelState>,
}

impl Default for GaugesPanel {
    fn default() -> Self {
        Self {
            state: RwLock::new(PanelState::default()),
        }
    }
}

impl EditorPanel for GaugesPanel {
    fn id(&self) -> &str {
        "gauges"
    }

    fn title(&self) -> &str {
        "Gauges"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::GAUGE)
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Bottom
    }

    fn min_size(&self) -> [f32; 2] {
        [200.0, 120.0]
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world
            .get_resource::<ThemeManager>()
            .map(|tm| tm.active_theme.clone())
            .unwrap_or_default();

        let Some(snapshot) = world.get_resource::<GaugesSnapshot>() else {
            return;
        };

        let mut state = self.state.write().unwrap();

        // Header with filter
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(
                egui::RichText::new(regular::MAGNIFYING_GLASS)
                    .size(12.0)
                    .color(theme.text.muted.to_color32()),
            );
            let filter_response = ui.add(
                egui::TextEdit::singleline(&mut state.filter)
                    .hint_text("Filter entities...")
                    .desired_width(150.0)
                    .font(egui::FontId::proportional(11.0)),
            );
            if ui
                .add_enabled(
                    !state.filter.is_empty(),
                    egui::Button::new(egui::RichText::new(regular::X).size(11.0)),
                )
                .clicked()
            {
                state.filter.clear();
                filter_response.request_focus();
            }
        });

        ui.add_space(4.0);
        ui.separator();

        // Filter entries
        let filter_lower = state.filter.to_lowercase();
        let entries: Vec<_> = snapshot
            .entries
            .iter()
            .filter(|e| {
                if filter_lower.is_empty() {
                    return true;
                }
                let entity_str = format!("{:?}", e.entity);
                let name_str = e.name.as_deref().unwrap_or("");
                entity_str.to_lowercase().contains(&filter_lower)
                    || name_str.to_lowercase().contains(&filter_lower)
            })
            .collect();

        if entries.is_empty() {
            ui.add_space(16.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(regular::GAUGE)
                        .size(24.0)
                        .color(theme.text.muted.to_color32()),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("No entities with gauges")
                        .size(12.0)
                        .color(theme.text.muted.to_color32()),
                );
                ui.label(
                    egui::RichText::new(
                        "Add the Gauges component to an entity to see its attributes here.",
                    )
                    .size(10.0)
                    .color(theme.text.muted.to_color32()),
                );
            });
            return;
        }

        // Scrollable entity list
        egui::ScrollArea::vertical()
            .id_salt("gauges_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;

                for entry in &entries {
                    let fallback = format!("{:?}", entry.entity);
                    let label = entry.name.as_deref().unwrap_or(&fallback);

                    let id = ui.make_persistent_id(format!("gauge_entity_{:?}", entry.entity));
                    egui::collapsing_header::CollapsingState::load_with_default_open(
                        ui.ctx(),
                        id,
                        true,
                    )
                    .show_header(ui, |ui| {
                        ui.label(
                            egui::RichText::new(regular::CUBE)
                                .size(11.0)
                                .color(theme.semantic.accent.to_color32()),
                        );
                        ui.label(
                            egui::RichText::new(label)
                                .size(11.0)
                                .color(theme.text.primary.to_color32()),
                        );
                    })
                    .body(|ui| {
                        if entry.attributes.is_empty() {
                            ui.label(
                                egui::RichText::new("No attributes defined")
                                    .size(10.0)
                                    .color(theme.text.muted.to_color32()),
                            );
                            return;
                        }

                        for (i, (attr_name, value)) in entry.attributes.iter().enumerate() {
                            let bg = if i % 2 == 0 {
                                theme.panels.inspector_row_even.to_color32()
                            } else {
                                theme.panels.inspector_row_odd.to_color32()
                            };

                            egui::Frame::new()
                                .fill(bg)
                                .inner_margin(egui::Margin::symmetric(4, 2))
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width() - 8.0);
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 4.0;

                                        // Name
                                        ui.add_sized(
                                            [70.0, 14.0],
                                            egui::Label::new(
                                                egui::RichText::new(attr_name)
                                                    .size(10.0)
                                                    .color(theme.text.secondary.to_color32()),
                                            )
                                            .truncate(),
                                        );

                                        // Bar
                                        let bar_width =
                                            (ui.available_width() - 44.0).max(30.0);
                                        let (rect, _) = ui.allocate_exact_size(
                                            egui::vec2(bar_width, 12.0),
                                            egui::Sense::hover(),
                                        );

                                        let rounding = egui::CornerRadius::same(2);
                                        ui.painter().rect_filled(
                                            rect,
                                            rounding,
                                            theme.surfaces.faint.to_color32(),
                                        );

                                        let ratio = (value / 100.0).clamp(0.0, 1.0);
                                        if ratio > 0.0 {
                                            let fill = egui::Rect::from_min_size(
                                                rect.min,
                                                egui::vec2(
                                                    rect.width() * ratio,
                                                    rect.height(),
                                                ),
                                            );
                                            ui.painter().rect_filled(
                                                fill,
                                                rounding,
                                                gauge_bar_color(ratio, &theme),
                                            );
                                        }

                                        // Value on bar
                                        ui.painter().text(
                                            rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            format!("{:.1}", value),
                                            egui::FontId::proportional(9.0),
                                            theme.text.primary.to_color32(),
                                        );

                                        // Value label
                                        ui.label(
                                            egui::RichText::new(format!("{:.1}", value))
                                                .size(10.0)
                                                .color(theme.text.secondary.to_color32()),
                                        );
                                    });
                                });
                        }
                    });
                }
            });
    }
}

/// Bar color based on fill ratio.
fn gauge_bar_color(ratio: f32, theme: &renzora_theme::Theme) -> egui::Color32 {
    if ratio < 0.25 {
        egui::Color32::from_rgb(220, 60, 60)
    } else if ratio < 0.5 {
        theme.semantic.warning.to_color32()
    } else {
        theme.semantic.success.to_color32()
    }
}
