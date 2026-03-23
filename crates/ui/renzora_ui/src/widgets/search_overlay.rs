//! Godot-style search overlay — backdrop + centered popup with search bar and
//! categorized item list with tree-line connectors.

use bevy_egui::egui::{self, Align2, Color32, CornerRadius, CursorIcon, Margin, Order, Pos2, RichText, Sense, Stroke, Vec2};
use egui_phosphor::regular::{CARET_DOWN, CARET_RIGHT, MAGNIFYING_GLASS};
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

/// Render a search overlay with backdrop, search bar, and categorized tree list.
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
    let has_search = !search.is_empty();

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

    // 2. Centered popup window
    let popup_id = egui::Id::new(id_salt).with("_popup");
    egui::Area::new(popup_id)
        .order(Order::Tooltip)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(theme.surfaces.popup.to_color32())
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::same(12))
                .stroke(Stroke::new(1.0, theme.widgets.border.to_color32()))
                .show(ui, |ui| {
                    ui.set_width(560.0);
                    ui.set_max_height(620.0);

                    // Title bar
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(title)
                                .size(18.0)
                                .strong()
                                .color(theme.text.heading.to_color32()),
                        );
                    });
                    ui.add_space(6.0);

                    // Search bar with auto-focus
                    let search_resp = ui.add_sized(
                        [ui.available_width(), 32.0],
                        egui::TextEdit::singleline(search)
                            .hint_text(format!("{} Search...", MAGNIFYING_GLASS))
                            .font(egui::FontId::proportional(15.0))
                            .margin(Margin::symmetric(8, 8)),
                    );
                    search_resp.request_focus();

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(2.0);

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
                        // Scrollable categorized tree list
                        egui::ScrollArea::vertical()
                            .max_height(520.0)
                            .id_salt(egui::Id::new(id_salt).with("_scroll"))
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;

                                let tree_line_color = theme.widgets.border.to_color32();
                                let line_stroke = Stroke::new(1.5, tree_line_color);
                                let row_height = 28.0_f32;
                                let indent = 24.0_f32;
                                let row_even = theme.panels.inspector_row_even.to_color32();
                                let row_odd = theme.panels.inspector_row_odd.to_color32();

                                let mut row_index: usize = 0;

                                for cat in &categories {
                                    let (accent, _header_bg) = category_colors(theme, cat);

                                    // Category expand/collapse state
                                    let cat_id = egui::Id::new(id_salt).with("_cat").with(*cat);
                                    let is_expanded = if has_search {
                                        true
                                    } else {
                                        ui.ctx().data_mut(|d| {
                                            *d.get_persisted_mut_or_insert_with(cat_id, || true)
                                        })
                                    };

                                    // --- Category row ---
                                    let (cat_rect, cat_response) = ui.allocate_exact_size(
                                        Vec2::new(ui.available_width(), row_height),
                                        Sense::click(),
                                    );
                                    let painter = ui.painter();

                                    // Alternating row bg
                                    let bg = if row_index % 2 == 0 { row_even } else { row_odd };
                                    painter.rect_filled(cat_rect, 0.0, bg);

                                    // Hover highlight
                                    if cat_response.hovered() {
                                        let [r, g, b, _] = theme.widgets.hovered_bg.to_color32().to_array();
                                        painter.rect_filled(
                                            cat_rect,
                                            0.0,
                                            Color32::from_rgba_unmultiplied(r, g, b, 40),
                                        );
                                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                                    }
                                    row_index += 1;

                                    let base_x = cat_rect.min.x + 4.0;
                                    let center_y = cat_rect.center().y;

                                    // Caret
                                    let caret = if is_expanded { CARET_DOWN } else { CARET_RIGHT };
                                    let caret_color = Color32::from_rgb(140, 140, 150);
                                    painter.text(
                                        Pos2::new(base_x + 2.0, center_y),
                                        Align2::LEFT_CENTER,
                                        caret,
                                        egui::FontId::proportional(13.0),
                                        caret_color,
                                    );

                                    // Category label
                                    painter.text(
                                        Pos2::new(base_x + 20.0, center_y),
                                        Align2::LEFT_CENTER,
                                        capitalize(cat),
                                        egui::FontId::proportional(14.0),
                                        accent,
                                    );

                                    // Toggle expand on click
                                    if cat_response.clicked() && !has_search {
                                        let new_val = !is_expanded;
                                        ui.ctx().data_mut(|d| d.insert_persisted(cat_id, new_val));
                                    }

                                    // --- Children ---
                                    if is_expanded {
                                        let items: Vec<&&OverlayEntry> = filtered
                                            .iter()
                                            .filter(|e| e.category == *cat)
                                            .collect();
                                        let item_count = items.len();

                                        for (i_idx, entry) in items.iter().enumerate() {
                                            if first_id.is_none() {
                                                first_id = Some(entry.id.to_string());
                                            }

                                            let is_last_item = i_idx == item_count - 1;

                                            let (row_rect, row_response) = ui.allocate_exact_size(
                                                Vec2::new(ui.available_width(), row_height),
                                                Sense::click(),
                                            );
                                            let painter = ui.painter();

                                            // Alternating row bg
                                            let bg = if row_index % 2 == 0 { row_even } else { row_odd };
                                            painter.rect_filled(row_rect, 0.0, bg);

                                            // Hover highlight
                                            if row_response.hovered() {
                                                let [r, g, b, _] = theme.widgets.hovered_bg.to_color32().to_array();
                                                painter.rect_filled(
                                                    row_rect,
                                                    0.0,
                                                    Color32::from_rgba_unmultiplied(r, g, b, 40),
                                                );
                                                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                                            }
                                            row_index += 1;

                                            let child_center_y = row_rect.center().y;
                                            let line_x = base_x + indent / 2.0 - 1.0;
                                            let line_overlap = 3.0;

                                            // Vertical tree line
                                            if !is_last_item {
                                                painter.line_segment(
                                                    [
                                                        Pos2::new(line_x, row_rect.min.y - line_overlap),
                                                        Pos2::new(line_x, row_rect.max.y + line_overlap),
                                                    ],
                                                    line_stroke,
                                                );
                                            } else {
                                                painter.line_segment(
                                                    [
                                                        Pos2::new(line_x, row_rect.min.y - line_overlap),
                                                        Pos2::new(line_x, child_center_y),
                                                    ],
                                                    line_stroke,
                                                );
                                            }

                                            // Horizontal connector
                                            let h_end_x = base_x + indent - 2.0;
                                            painter.line_segment(
                                                [
                                                    Pos2::new(line_x, child_center_y),
                                                    Pos2::new(h_end_x, child_center_y),
                                                ],
                                                line_stroke,
                                            );

                                            // Item icon + label
                                            let item_x = base_x + indent + 2.0;
                                            painter.text(
                                                Pos2::new(item_x, child_center_y),
                                                Align2::LEFT_CENTER,
                                                entry.icon,
                                                egui::FontId::proportional(15.0),
                                                accent,
                                            );
                                            painter.text(
                                                Pos2::new(item_x + 22.0, child_center_y),
                                                Align2::LEFT_CENTER,
                                                entry.label,
                                                egui::FontId::proportional(13.0),
                                                theme.text.primary.to_color32(),
                                            );

                                            if row_response.clicked() {
                                                action = OverlayAction::Selected(
                                                    entry.id.to_string(),
                                                );
                                            }
                                        }
                                    }
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
