//! Godot-style search overlay — backdrop + centered popup with search bar and categorized item list.

use bevy_egui::egui::{self, Align2, CornerRadius, CursorIcon, Margin, Order, RichText, Sense, Stroke, Vec2};
use egui_phosphor::regular::MAGNIFYING_GLASS;
use renzora_theme::Theme;

use super::category::category_colors;

/// A single entry displayed in the search overlay.
pub struct OverlayEntry<'a> {
    pub id: &'a str,
    pub label: &'a str,
    pub icon: &'a str,
    pub category: &'a str,
}

/// Action returned by the search overlay each frame.
pub enum OverlayAction {
    /// Nothing happened this frame.
    None,
    /// User selected an item (its id).
    Selected(String),
    /// User closed the overlay (Escape or click-outside).
    Closed,
}

/// Render a search overlay with backdrop, search bar, and categorized item list.
///
/// Returns [`OverlayAction`] indicating user interaction.
pub fn search_overlay(
    ctx: &egui::Context,
    id_salt: &str,
    title: &str,
    entries: &[OverlayEntry],
    search: &mut String,
    theme: &Theme,
) -> OverlayAction {
    let mut action = OverlayAction::None;

    // Filter entries by search term
    let lower = search.to_lowercase();
    let filtered: Vec<&OverlayEntry> = if lower.trim().is_empty() {
        entries.iter().collect()
    } else {
        entries
            .iter()
            .filter(|e| e.label.to_lowercase().contains(&lower))
            .collect()
    };

    // Track first visible item id for Enter-to-select
    let mut first_id: Option<String> = None;

    // 1. Semi-transparent backdrop (click to close)
    let backdrop_id = egui::Id::new(id_salt).with("_backdrop");
    egui::Area::new(backdrop_id)
        .order(Order::Foreground)
        .anchor(Align2::LEFT_TOP, Vec2::ZERO)
        .show(ctx, |ui| {
            let screen = ctx.input(|i| i.viewport_rect());
            let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter()
                .rect_filled(rect, 0.0, theme.surfaces.overlay.to_color32());
            if resp.clicked() {
                action = OverlayAction::Closed;
            }
        });

    // 2. Centered popup
    let popup_id = egui::Id::new(id_salt).with("_popup");
    egui::Area::new(popup_id)
        .order(Order::Tooltip)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(theme.surfaces.popup.to_color32())
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::same(16))
                .stroke(Stroke::new(1.0, theme.widgets.border.to_color32()))
                .show(ui, |ui| {
                    ui.set_width(450.0);
                    ui.set_max_height(500.0);

                    // Title
                    ui.label(
                        RichText::new(title)
                            .size(16.0)
                            .strong()
                            .color(theme.text.heading.to_color32()),
                    );
                    ui.add_space(8.0);

                    // Search bar with auto-focus
                    let search_resp = ui.add(
                        egui::TextEdit::singleline(search)
                            .desired_width(ui.available_width())
                            .hint_text(format!("{} Search...", MAGNIFYING_GLASS)),
                    );
                    search_resp.request_focus();
                    ui.add_space(8.0);

                    // Group entries by category (preserve order)
                    let mut categories: Vec<&str> = Vec::new();
                    for entry in &filtered {
                        if !categories.contains(&entry.category) {
                            categories.push(entry.category);
                        }
                    }

                    if filtered.is_empty() {
                        ui.add_space(16.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new("No results found.")
                                    .size(12.0)
                                    .color(theme.text.disabled.to_color32()),
                            );
                        });
                        ui.add_space(16.0);
                    } else {
                        // Scrollable categorized list
                        egui::ScrollArea::vertical()
                            .max_height(400.0)
                            .id_salt(egui::Id::new(id_salt).with("_scroll"))
                            .show(ui, |ui| {
                                for cat in &categories {
                                    let (accent, header_bg) = category_colors(theme, cat);
                                    // Category header
                                    egui::Frame::new()
                                        .fill(header_bg)
                                        .corner_radius(CornerRadius::same(4))
                                        .inner_margin(Margin::symmetric(8, 4))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new(capitalize(cat))
                                                    .size(11.0)
                                                    .strong()
                                                    .color(accent),
                                            );
                                        });
                                    ui.add_space(2.0);

                                    // Items in this category
                                    let items: Vec<&&OverlayEntry> = filtered
                                        .iter()
                                        .filter(|e| e.category == *cat)
                                        .collect();

                                    for entry in items {
                                        if first_id.is_none() {
                                            first_id = Some(entry.id.to_string());
                                        }

                                        let row_id = egui::Id::new(id_salt)
                                            .with("_item")
                                            .with(entry.id);

                                        let resp = ui
                                            .scope(|ui| {
                                                let (rect, resp) = ui.allocate_exact_size(
                                                    Vec2::new(ui.available_width(), 28.0),
                                                    Sense::click(),
                                                );

                                                // Hover highlight
                                                if resp.hovered() {
                                                    ui.painter().rect_filled(
                                                        rect,
                                                        4.0,
                                                        theme.panels.item_hover.to_color32(),
                                                    );
                                                    ui.ctx()
                                                        .set_cursor_icon(CursorIcon::PointingHand);
                                                }

                                                // Icon
                                                let icon_pos = rect.left_center()
                                                    + Vec2::new(8.0, 0.0);
                                                ui.painter().text(
                                                    icon_pos,
                                                    egui::Align2::LEFT_CENTER,
                                                    entry.icon,
                                                    egui::FontId::proportional(15.0),
                                                    accent,
                                                );

                                                // Label
                                                let label_pos = rect.left_center()
                                                    + Vec2::new(30.0, 0.0);
                                                ui.painter().text(
                                                    label_pos,
                                                    egui::Align2::LEFT_CENTER,
                                                    entry.label,
                                                    egui::FontId::proportional(13.0),
                                                    theme.text.primary.to_color32(),
                                                );

                                                resp
                                            })
                                            .inner;

                                        let _ = row_id; // used for id scoping
                                        if resp.clicked() {
                                            action = OverlayAction::Selected(
                                                entry.id.to_string(),
                                            );
                                        }
                                    }

                                    ui.add_space(4.0);
                                }
                            });
                    }
                });
        });

    // Keyboard: Escape closes, Enter selects first match
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        action = OverlayAction::Closed;
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
        if let Some(id) = first_id {
            action = OverlayAction::Selected(id);
        }
    }

    action
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().to_string() + c.as_str(),
    }
}
