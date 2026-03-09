//! Inspector registration for the Gauges component.
//!
//! Uses a custom UI function to render attribute bars and an "add attribute"
//! interface instead of the standard field-based inspector.

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_editor::{EditorCommands, InspectorEntry};
use renzora_theme::Theme;

use crate::{Attributes, Gauges};

/// Build the inspector entry for the `Gauges` component.
pub fn gauges_inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "gauges",
        display_name: "Gauges",
        icon: regular::GAUGE,
        category: "gameplay",
        has_fn: |world, entity| world.get::<Gauges>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.commands().entity(entity).insert(Gauges);
        }),
        remove_fn: Some(|world, entity| {
            world.commands().entity(entity).remove::<Gauges>();
            world.commands().entity(entity).remove::<Attributes>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
        custom_ui_fn: Some(gauges_custom_ui),
    }
}

/// Custom inspector UI — renders attribute bars with values and an add button.
fn gauges_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    _cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(attrs) = world.get::<Attributes>(entity) else {
        ui.label(
            egui::RichText::new("Attributes not yet initialized.")
                .size(11.0)
                .color(theme.text.muted.to_color32()),
        );
        return;
    };

    let items: Vec<(String, f32)> = attrs
        .iter()
        .map(|(id, val)| {
            // Try to get a human-readable name; fall back to debug id
            (bevy_gauge::attribute_id::Interner::global().resolve(id).to_string(), val)
        })
        .collect();

    if items.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("No attributes defined.")
                    .size(11.0)
                    .color(theme.text.muted.to_color32()),
            );
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new("Use scripts or code to add attributes.")
                    .size(10.0)
                    .color(theme.text.muted.to_color32()),
            );
            ui.add_space(4.0);
        });
        return;
    }

    for (i, (name, value)) in items.iter().enumerate() {
        let bg = if i % 2 == 0 {
            theme.panels.inspector_row_even.to_color32()
        } else {
            theme.panels.inspector_row_odd.to_color32()
        };

        egui::Frame::new()
            .fill(bg)
            .inner_margin(egui::Margin::symmetric(6, 3))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width() - 12.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;

                    // Attribute name
                    ui.add_sized(
                        [80.0, 16.0],
                        egui::Label::new(
                            egui::RichText::new(name)
                                .size(11.0)
                                .color(theme.text.primary.to_color32()),
                        )
                        .truncate(),
                    );

                    // Gauge bar
                    let available = ui.available_width() - 50.0;
                    let bar_width = available.max(40.0);
                    let (rect, _) = ui.allocate_exact_size(
                        egui::vec2(bar_width, 14.0),
                        egui::Sense::hover(),
                    );

                    // Background track
                    let rounding = egui::CornerRadius::same(3);
                    ui.painter().rect_filled(
                        rect,
                        rounding,
                        theme.surfaces.faint.to_color32(),
                    );

                    // Determine fill ratio (clamp 0..1 assuming 0..100 range,
                    // but show raw value — scripts define actual ranges)
                    let ratio = (value / 100.0).clamp(0.0, 1.0);
                    if ratio > 0.0 {
                        let fill_rect = egui::Rect::from_min_size(
                            rect.min,
                            egui::vec2(rect.width() * ratio, rect.height()),
                        );
                        let bar_color = gauge_color(ratio, theme);
                        ui.painter().rect_filled(fill_rect, rounding, bar_color);
                    }

                    // Value text centered on bar
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("{:.1}", value),
                        egui::FontId::proportional(10.0),
                        theme.text.primary.to_color32(),
                    );

                    // Numeric value to the right
                    ui.label(
                        egui::RichText::new(format!("{:.1}", value))
                            .size(10.0)
                            .color(theme.text.secondary.to_color32()),
                    );
                });
            });
    }
}

/// Pick a bar color based on the fill ratio: red < 25%, yellow < 50%, green/accent otherwise.
fn gauge_color(ratio: f32, theme: &Theme) -> egui::Color32 {
    if ratio < 0.25 {
        egui::Color32::from_rgb(220, 60, 60)
    } else if ratio < 0.5 {
        theme.semantic.warning.to_color32()
    } else {
        theme.semantic.success.to_color32()
    }
}
