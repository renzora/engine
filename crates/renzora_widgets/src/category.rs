//! Collapsible category sections — themed, foldable groups with icon headers.

use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, RichText, Sense};
use egui_phosphor::regular::{CARET_DOWN, CARET_RIGHT, TRASH};
use renzora_theme::Theme;

use crate::colors::dim_color;
use crate::toggle::toggle_switch;

/// Result of interacting with a removable category header.
pub struct CategoryHeaderAction {
    pub remove_clicked: bool,
    pub toggle_clicked: bool,
    pub drag_started: bool,
}

/// Render a collapsible category section with an icon, label, and themed header.
///
/// The header is clickable to toggle collapse state. Content is only rendered
/// when the section is open.
pub fn collapsible_section(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    category: &str,
    theme: &Theme,
    id_source: &str,
    default_open: bool,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let id = ui.make_persistent_id(id_source);
    let mut state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, default_open);

    let (accent_color, header_bg) = category_colors(theme, category);
    let frame_bg = theme.panels.category_frame_bg.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let text_primary = theme.text.primary.to_color32();

    ui.scope(|ui| {
        egui::Frame::new()
            .fill(frame_bg)
            .corner_radius(CornerRadius::same(6))
            .show(ui, |ui| {
                // Header bar
                let header_rect = ui
                    .scope(|ui| {
                        egui::Frame::new()
                            .fill(header_bg)
                            .corner_radius(CornerRadius {
                                nw: 6,
                                ne: 6,
                                sw: if state.is_open() { 0 } else { 6 },
                                se: if state.is_open() { 0 } else { 6 },
                            })
                            .inner_margin(egui::Margin::symmetric(8, 6))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let caret = if state.is_open() { CARET_DOWN } else { CARET_RIGHT };
                                    ui.label(RichText::new(caret).size(12.0).color(text_muted));
                                    ui.label(RichText::new(icon).size(15.0).color(accent_color));
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(label)
                                            .size(13.0)
                                            .strong()
                                            .color(text_primary),
                                    );
                                    ui.allocate_space(ui.available_size());
                                });
                            });
                    })
                    .response
                    .rect;

                let resp = ui.interact(header_rect, id.with("header"), Sense::click());
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    state.toggle(ui);
                }

                if state.is_open() {
                    ui.add_space(4.0);
                    egui::Frame::new()
                        .inner_margin(egui::Margin {
                            left: 4,
                            right: 4,
                            top: 0,
                            bottom: 4,
                        })
                        .show(ui, |ui| {
                            add_contents(ui);
                        });
                }
            });
    });

    state.store(ui.ctx());
    ui.add_space(6.0);
}

/// Render a collapsible category with toggle switch, remove button, and drag handle.
///
/// Returns a [`CategoryHeaderAction`] indicating which buttons were clicked.
pub fn collapsible_section_removable(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    category: &str,
    theme: &Theme,
    id_source: &str,
    default_open: bool,
    can_remove: bool,
    is_disabled: bool,
    add_contents: impl FnOnce(&mut egui::Ui),
) -> CategoryHeaderAction {
    let id = ui.make_persistent_id(id_source);
    let mut state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, default_open);

    let mut remove_clicked = false;
    let mut toggle_clicked_flag = false;
    let mut drag_started = false;

    let (accent_color, header_bg) = category_colors(theme, category);
    let frame_bg = theme.panels.category_frame_bg.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let text_primary = theme.text.primary.to_color32();

    let _eff_accent = if is_disabled { dim_color(accent_color, 0.5) } else { accent_color };
    let eff_header = if is_disabled { dim_color(header_bg, 0.6) } else { header_bg };
    let eff_text = if is_disabled { dim_color(text_primary, 0.5) } else { text_primary };
    let eff_icon = if is_disabled { dim_color(accent_color, 0.4) } else { accent_color };

    ui.scope(|ui| {
        egui::Frame::new()
            .fill(frame_bg)
            .corner_radius(CornerRadius::same(6))
            .show(ui, |ui| {
                // Header
                let header_response = ui
                    .scope(|ui| {
                        egui::Frame::new()
                            .fill(eff_header)
                            .corner_radius(CornerRadius {
                                nw: 6,
                                ne: 6,
                                sw: if state.is_open() { 0 } else { 6 },
                                se: if state.is_open() { 0 } else { 6 },
                            })
                            .inner_margin(egui::Margin::symmetric(8, 6))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    // Left: grip + caret + icon + label
                                    ui.scope(|ui| {
                                        ui.set_max_width(
                                            (ui.available_width() - 58.0).max(20.0),
                                        );

                                        ui.label(
                                            RichText::new("⠿")
                                                .size(10.0)
                                                .color(dim_color(text_muted, 0.5)),
                                        );

                                        let caret =
                                            if state.is_open() { "▾" } else { "▸" };
                                        ui.label(
                                            RichText::new(caret).size(12.0).color(text_muted),
                                        );

                                        ui.label(
                                            RichText::new(icon).size(15.0).color(eff_icon),
                                        );
                                        ui.add_space(4.0);

                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(label)
                                                    .size(13.0)
                                                    .strong()
                                                    .color(eff_text),
                                            )
                                            .truncate(),
                                        );
                                    });

                                    // Right: toggle + trash
                                    if can_remove {
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                // Trash
                                                let trash = ui.add(
                                                    egui::Button::new(
                                                        RichText::new(TRASH)
                                                            .size(13.0)
                                                            .color(text_muted),
                                                    )
                                                    .frame(false),
                                                );
                                                if trash.hovered() {
                                                    let r = trash.rect.expand(2.0);
                                                    ui.painter().rect_filled(
                                                        r,
                                                        3.0,
                                                        Color32::from_rgba_premultiplied(
                                                            230, 89, 89, 30,
                                                        ),
                                                    );
                                                }
                                                if trash.clicked() {
                                                    remove_clicked = true;
                                                }

                                                ui.add_space(4.0);

                                                // Toggle
                                                if toggle_switch(ui, id.with("toggle"), !is_disabled)
                                                {
                                                    toggle_clicked_flag = true;
                                                }
                                            },
                                        );
                                    } else {
                                        ui.allocate_space(ui.available_size());
                                    }
                                });
                            });
                    })
                    .response;

                let click = header_response.interact(Sense::click_and_drag());
                if click.drag_started() {
                    drag_started = true;
                }
                if click.hovered() && !toggle_clicked_flag && !remove_clicked {
                    if click.dragged() {
                        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
                    } else {
                        ui.ctx().set_cursor_icon(CursorIcon::Grab);
                    }
                }
                if click.clicked() && !toggle_clicked_flag && !remove_clicked {
                    state.toggle(ui);
                }

                // Content
                if state.is_open() {
                    ui.add_space(4.0);
                    egui::Frame::new()
                        .inner_margin(egui::Margin {
                            left: 4,
                            right: 4,
                            top: 0,
                            bottom: 4,
                        })
                        .show(ui, |ui| {
                            if is_disabled {
                                ui.disable();
                            }
                            add_contents(ui);
                        });
                }
            });
    });

    state.store(ui.ctx());
    ui.add_space(6.0);

    CategoryHeaderAction {
        remove_clicked,
        toggle_clicked: toggle_clicked_flag,
        drag_started,
    }
}

/// Map a category name to its accent + header background colors from the theme.
fn category_colors(theme: &Theme, category: &str) -> (Color32, Color32) {
    let style = match category {
        "transform" => &theme.categories.transform,
        "environment" => &theme.categories.environment,
        "light" | "lighting" => &theme.categories.lighting,
        "camera" => &theme.categories.camera,
        "script" | "scripting" => &theme.categories.scripting,
        "physics" => &theme.categories.physics,
        "plugin" => &theme.categories.plugin,
        "nodes2d" | "nodes_2d" => &theme.categories.nodes_2d,
        "ui" => &theme.categories.ui,
        "rendering" => &theme.categories.rendering,
        "effects" | "particles" => &theme.categories.effects,
        _ => &theme.categories.transform,
    };
    (style.accent.to_color32(), style.header_bg.to_color32())
}
