#![allow(dead_code)]

use bevy_egui::egui::{self, Color32, CornerRadius, CursorIcon, RichText, Sense, Vec2};

use crate::commands::CommandHistory;
use crate::theming::Theme;

use egui_phosphor::regular::{CLOCK_COUNTER_CLOCKWISE, CARET_DOWN, CARET_RIGHT, IMAGE, TRASH};

/// Default background colors for alternating rows (used by legacy functions without theme access)
const ROW_BG_EVEN: Color32 = Color32::from_rgb(32, 34, 38);
const ROW_BG_ODD: Color32 = Color32::from_rgb(38, 40, 44);

/// Cached theme colors for inspector components (stored in egui context data)
#[derive(Clone)]
pub struct InspectorThemeColors {
    pub row_even: Color32,
    pub row_odd: Color32,
    pub surface_faint: Color32,
    pub surface_panel: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub text_disabled: Color32,
    pub widget_inactive_bg: Color32,
    pub widget_hovered_bg: Color32,
    pub widget_border: Color32,
    pub semantic_accent: Color32,
    pub semantic_success: Color32,
    pub semantic_warning: Color32,
    pub semantic_error: Color32,
}

impl Default for InspectorThemeColors {
    fn default() -> Self {
        Self {
            row_even: ROW_BG_EVEN,
            row_odd: ROW_BG_ODD,
            surface_faint: Color32::from_rgb(20, 20, 24),
            surface_panel: Color32::from_rgb(30, 32, 36),
            text_primary: Color32::from_rgb(220, 222, 228),
            text_secondary: Color32::from_rgb(180, 180, 190),
            text_muted: Color32::from_rgb(140, 142, 148),
            text_disabled: Color32::from_rgb(100, 100, 110),
            widget_inactive_bg: Color32::from_rgb(45, 48, 55),
            widget_hovered_bg: Color32::from_rgb(50, 53, 60),
            widget_border: Color32::from_rgb(55, 58, 65),
            semantic_accent: Color32::from_rgb(66, 150, 250),
            semantic_success: Color32::from_rgb(89, 191, 115),
            semantic_warning: Color32::from_rgb(242, 166, 64),
            semantic_error: Color32::from_rgb(230, 89, 89),
        }
    }
}

impl InspectorThemeColors {
    /// Create from a Theme
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            row_even: theme.panels.inspector_row_even.to_color32(),
            row_odd: theme.panels.inspector_row_odd.to_color32(),
            surface_faint: theme.surfaces.faint.to_color32(),
            surface_panel: theme.panels.category_frame_bg.to_color32(),
            text_primary: theme.text.primary.to_color32(),
            text_secondary: theme.text.secondary.to_color32(),
            text_muted: theme.text.muted.to_color32(),
            text_disabled: theme.text.disabled.to_color32(),
            widget_inactive_bg: theme.widgets.inactive_bg.to_color32(),
            widget_hovered_bg: theme.widgets.hovered_bg.to_color32(),
            widget_border: theme.widgets.border.to_color32(),
            semantic_accent: theme.semantic.accent.to_color32(),
            semantic_success: theme.semantic.success.to_color32(),
            semantic_warning: theme.semantic.warning.to_color32(),
            semantic_error: theme.semantic.error.to_color32(),
        }
    }
}

/// Store inspector theme colors in egui context (call at start of render_inspector_content)
pub fn set_inspector_theme(ctx: &egui::Context, theme: &Theme) {
    let colors = InspectorThemeColors::from_theme(theme);
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("inspector_theme"), colors));
}

/// Get inspector theme colors from egui context (call from inspector components)
pub fn get_inspector_theme(ctx: &egui::Context) -> InspectorThemeColors {
    ctx.data(|d| d.get_temp::<InspectorThemeColors>(egui::Id::new("inspector_theme")))
        .unwrap_or_default()
}

/// Background colors for alternating rows - uses theme colors
fn get_row_colors(theme: &Theme) -> (Color32, Color32) {
    (
        theme.panels.inspector_row_even.to_color32(),
        theme.panels.inspector_row_odd.to_color32(),
    )
}

/// Helper to render a property row with alternating background (themed version)
pub fn property_row_themed(ui: &mut egui::Ui, row_index: usize, theme: &Theme, add_contents: impl FnOnce(&mut egui::Ui)) {
    let (row_even, row_odd) = get_row_colors(theme);
    let bg_color = if row_index % 2 == 0 { row_even } else { row_odd };
    let available_width = ui.available_width();

    egui::Frame::new()
        .fill(bg_color)
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.set_min_width(available_width - 12.0);
            add_contents(ui);
        });
}

/// Helper to render a property row with alternating background (legacy - uses default colors)
pub fn property_row(ui: &mut egui::Ui, row_index: usize, add_contents: impl FnOnce(&mut egui::Ui)) {
    let bg_color = if row_index % 2 == 0 {
        Color32::from_rgb(32, 34, 38)
    } else {
        Color32::from_rgb(38, 40, 44)
    };
    let available_width = ui.available_width();

    egui::Frame::new()
        .fill(bg_color)
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.set_min_width(available_width - 12.0);
            add_contents(ui);
        });
}

/// Width reserved for property labels
pub const LABEL_WIDTH: f32 = 80.0;

/// Minimum width for the inspector content area
pub const MIN_INSPECTOR_WIDTH: f32 = 220.0;

/// Helper to render an inline property with label on left, widget immediately after (themed version)
/// Returns whether the value changed
pub fn inline_property_themed<R>(
    ui: &mut egui::Ui,
    row_index: usize,
    label: &str,
    theme: &Theme,
    add_widget: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let (row_even, row_odd) = get_row_colors(theme);
    let bg_color = if row_index % 2 == 0 { row_even } else { row_odd };
    let available_width = ui.available_width().max(MIN_INSPECTOR_WIDTH);

    egui::Frame::new()
        .fill(bg_color)
        .inner_margin(egui::Margin::symmetric(4, 2))
        .show(ui, |ui| {
            ui.set_min_width(available_width - 8.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                // Fixed-width label with smaller text
                ui.add_sized(
                    [LABEL_WIDTH, 16.0],
                    egui::Label::new(egui::RichText::new(label).size(11.0)).truncate()
                );
                // Widget fills remaining space, left-aligned
                add_widget(ui)
            })
            .inner
        })
        .inner
}

/// Helper to render an inline property with label on left, widget immediately after (legacy)
/// Returns whether the value changed
pub fn inline_property<R>(
    ui: &mut egui::Ui,
    row_index: usize,
    label: &str,
    add_widget: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let bg_color = if row_index % 2 == 0 {
        Color32::from_rgb(32, 34, 38)
    } else {
        Color32::from_rgb(38, 40, 44)
    };
    let available_width = ui.available_width().max(MIN_INSPECTOR_WIDTH);

    egui::Frame::new()
        .fill(bg_color)
        .inner_margin(egui::Margin::symmetric(4, 2))
        .show(ui, |ui| {
            ui.set_min_width(available_width - 8.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                // Fixed-width label with smaller text
                ui.add_sized(
                    [LABEL_WIDTH, 16.0],
                    egui::Label::new(egui::RichText::new(label).size(11.0)).truncate()
                );
                // Widget fills remaining space, left-aligned
                add_widget(ui)
            })
            .inner
        })
        .inner
}

/// Get category style colors from theme
fn get_category_colors(theme: &Theme, category: &str) -> (Color32, Color32) {
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
        _ => &theme.categories.transform, // fallback
    };
    (style.accent.to_color32(), style.header_bg.to_color32())
}

/// Renders a styled inspector category with header and content
pub fn render_category(
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
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, default_open);
    let (accent_color, header_bg) = get_category_colors(theme, category);
    let frame_bg = theme.panels.category_frame_bg.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let text_primary = theme.text.primary.to_color32();

    ui.scope(|ui| {
        // Outer frame for the entire category
        egui::Frame::new()
            .fill(frame_bg)
            .corner_radius(CornerRadius::same(6))
            .show(ui, |ui| {

                // Header bar
                let header_rect = ui.scope(|ui| {
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
                                // Collapse indicator
                                let caret = if state.is_open() { CARET_DOWN } else { CARET_RIGHT };
                                ui.label(RichText::new(caret).size(12.0).color(text_muted));

                                // Icon
                                ui.label(RichText::new(icon).size(15.0).color(accent_color));

                                ui.add_space(4.0);

                                // Label
                                ui.label(RichText::new(label).size(13.0).strong().color(text_primary));

                                // Fill remaining width
                                ui.allocate_space(ui.available_size());
                            });
                        });
                }).response.rect;

                // Make header clickable
                let header_response = ui.interact(header_rect, id.with("header"), egui::Sense::click());
                if header_response.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                if header_response.clicked() {
                    state.toggle(ui);
                }

                // Content area with padding
                if state.is_open() {
                    ui.add_space(4.0);
                    egui::Frame::new()
                        .inner_margin(egui::Margin { left: 4, right: 4, top: 0, bottom: 4 })
                        .show(ui, |ui| {
                            add_contents(ui);
                        });
                }
            });
    });

    state.store(ui.ctx());

    ui.add_space(6.0);
}

/// Result of a category header interaction
pub struct CategoryHeaderAction {
    pub remove_clicked: bool,
    pub toggle_clicked: bool,
}

/// Renders a styled inspector category with header, content, toggle switch, and remove button
/// Returns a CategoryHeaderAction indicating which buttons were clicked
pub fn render_category_removable(
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
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, default_open);
    let mut remove_clicked = false;
    let mut toggle_clicked = false;
    let (accent_color, header_bg) = get_category_colors(theme, category);
    let frame_bg = theme.panels.category_frame_bg.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let text_primary = theme.text.primary.to_color32();
    let error_color = theme.semantic.error.to_color32();

    // Dim colors when disabled
    let effective_accent = if is_disabled { dim_color(accent_color, 0.5) } else { accent_color };
    let effective_header_bg = if is_disabled { dim_color(header_bg, 0.6) } else { header_bg };
    let effective_text = if is_disabled { dim_color(text_primary, 0.5) } else { text_primary };
    let effective_icon = if is_disabled { dim_color(accent_color, 0.4) } else { accent_color };

    ui.scope(|ui| {
        // Outer frame for the entire category
        egui::Frame::new()
            .fill(frame_bg)
            .corner_radius(CornerRadius::same(6))
            .show(ui, |ui| {

                // Header bar
                let header_response = ui.scope(|ui| {
                    egui::Frame::new()
                        .fill(effective_header_bg)
                        .corner_radius(CornerRadius {
                            nw: 6,
                            ne: 6,
                            sw: if state.is_open() { 0 } else { 6 },
                            se: if state.is_open() { 0 } else { 6 },
                        })
                        .inner_margin(egui::Margin::symmetric(8, 6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Left side: caret + icon + label (no individual click handler)
                                ui.scope(|ui| {
                                    // Collapse indicator
                                    let caret = if state.is_open() { CARET_DOWN } else { CARET_RIGHT };
                                    ui.label(RichText::new(caret).size(12.0).color(text_muted));

                                    // Icon
                                    ui.label(RichText::new(icon).size(15.0).color(effective_icon));

                                    ui.add_space(4.0);

                                    // Label
                                    ui.label(RichText::new(label).size(13.0).strong().color(effective_text));
                                });

                                // Right-aligned: Toggle switch + Trash button
                                if can_remove {
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        // Trash icon (rightmost)
                                        let trash_response = ui.add(
                                            egui::Button::new(RichText::new(TRASH).size(13.0).color(text_muted))
                                                .frame(false)
                                        );
                                        if trash_response.hovered() {
                                            let painter = ui.painter();
                                            let rect = trash_response.rect.expand(2.0);
                                            painter.rect_filled(rect, 3.0, Color32::from_rgba_premultiplied(230, 89, 89, 30));
                                        }
                                        trash_response.clone().on_hover_text("Remove component");
                                        if trash_response.clicked() {
                                            remove_clicked = true;
                                        }

                                        ui.add_space(4.0);

                                        // Toggle switch
                                        let toggle_response = paint_toggle_switch(ui, id.with("toggle"), !is_disabled);
                                        if toggle_response.clicked {
                                            toggle_clicked = true;
                                        }
                                    });
                                } else {
                                    // Fill remaining width so header has full width
                                    ui.allocate_space(ui.available_size());
                                }
                            });
                        });
                }).response;

                // Entire header is clickable for collapse (except toggle/trash which set their own flags)
                let header_click = header_response.interact(Sense::click());
                if header_click.hovered() && !toggle_clicked && !remove_clicked {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                if header_click.clicked() && !toggle_clicked && !remove_clicked {
                    state.toggle(ui);
                }

                // Content area with padding
                if state.is_open() {
                    ui.add_space(4.0);
                    egui::Frame::new()
                        .inner_margin(egui::Margin { left: 4, right: 4, top: 0, bottom: 4 })
                        .show(ui, |ui| {
                            // Disable all content widgets when component is disabled
                            ui.set_enabled(!is_disabled);
                            add_contents(ui);
                        });
                }
            });
    });

    state.store(ui.ctx());

    ui.add_space(6.0);

    CategoryHeaderAction { remove_clicked, toggle_clicked }
}

/// Dim a Color32 by a brightness factor (0.0 = black, 1.0 = unchanged)
fn dim_color(color: Color32, factor: f32) -> Color32 {
    Color32::from_rgb(
        (color.r() as f32 * factor) as u8,
        (color.g() as f32 * factor) as u8,
        (color.b() as f32 * factor) as u8,
    )
}

/// Result of a toggle switch interaction
struct ToggleSwitchResponse {
    clicked: bool,
    rect: egui::Rect,
}

/// Paint a custom toggle switch (pill-shaped) and return whether it was clicked
fn paint_toggle_switch(ui: &mut egui::Ui, id: egui::Id, enabled: bool) -> ToggleSwitchResponse {
    let desired_size = Vec2::new(28.0, 14.0);
    let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click());

    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool_with_time(id, enabled, 0.15);

        let bg_color = Color32::from_rgb(
            (80.0 + (89.0 - 80.0) * how_on) as u8,
            (80.0 + (191.0 - 80.0) * how_on) as u8,
            (85.0 + (115.0 - 85.0) * how_on) as u8,
        );

        // Draw pill background
        let radius = rect.height() / 2.0;
        ui.painter().rect_filled(rect, radius, bg_color);

        // Draw circle knob
        let knob_radius = radius - 2.0;
        let knob_x = rect.left() + radius + how_on * (rect.width() - 2.0 * radius);
        let knob_center = egui::pos2(knob_x, rect.center().y);
        ui.painter().circle_filled(knob_center, knob_radius, Color32::WHITE);
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        response.clone().on_hover_text(if enabled { "Disable component" } else { "Enable component" });
    }

    ToggleSwitchResponse {
        clicked: response.clicked(),
        rect,
    }
}

/// Render the history panel content
pub fn render_history_content(
    ui: &mut egui::Ui,
    command_history: &mut CommandHistory,
) {
    use egui_phosphor::regular::{ARROW_U_UP_LEFT, ARROW_U_UP_RIGHT, CIRCLE};

    let theme_colors = get_inspector_theme(ui.ctx());

    // Content area with padding
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(6, 4))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let undo_descriptions = command_history.undo_descriptions();
                let redo_descriptions = command_history.redo_descriptions();
                let undo_count = undo_descriptions.len();
                let redo_count = redo_descriptions.len();

                if undo_descriptions.is_empty() && redo_descriptions.is_empty() {
                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new(CLOCK_COUNTER_CLOCKWISE).size(32.0).color(theme_colors.text_disabled));
                        ui.add_space(8.0);
                        ui.label(RichText::new("No History").weak());
                        ui.add_space(8.0);
                        ui.label(RichText::new("Actions you perform will\nappear here.").weak());
                    });
                    return;
                }

                // Show redo stack (future actions) - displayed at top, grayed out
                // Clicking on a redo item will redo all actions up to and including that item
                if !redo_descriptions.is_empty() {
                    ui.label(RichText::new("Redo Stack").size(11.0).color(theme_colors.text_disabled));
                    ui.add_space(4.0);

                    // Show redo items in reverse order (most recent undone at top)
                    // Index 0 in reversed = last item in original = furthest from current state
                    for (display_i, desc) in redo_descriptions.iter().rev().enumerate() {
                        // Number of redos needed: redo_count - display_i (to reach this state)
                        let redos_needed = redo_count - display_i;

                        let row_id = ui.id().with(("redo", display_i));
                        let available_width = ui.available_width();
                        let (row_rect, row_response) = ui.allocate_exact_size(
                            Vec2::new(available_width, 24.0),
                            Sense::click(),
                        );

                        let is_hovered = row_response.hovered();
                        if is_hovered {
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        }
                        let bg_color = if is_hovered {
                            theme_colors.widget_hovered_bg
                        } else if display_i % 2 == 0 {
                            theme_colors.row_even
                        } else {
                            theme_colors.row_odd
                        };

                        ui.painter().rect_filled(row_rect, 3.0, bg_color);

                        // Draw icon and text
                        let icon_pos = egui::pos2(row_rect.min.x + 8.0, row_rect.center().y);
                        let text_pos = egui::pos2(row_rect.min.x + 26.0, row_rect.center().y);

                        ui.painter().text(
                            icon_pos,
                            egui::Align2::LEFT_CENTER,
                            ARROW_U_UP_RIGHT,
                            egui::FontId::proportional(12.0),
                            if is_hovered { theme_colors.text_muted } else { theme_colors.text_disabled },
                        );
                        ui.painter().text(
                            text_pos,
                            egui::Align2::LEFT_CENTER,
                            desc,
                            egui::FontId::proportional(12.0),
                            if is_hovered { theme_colors.text_secondary } else { theme_colors.text_disabled },
                        );

                        if row_response.clicked() {
                            command_history.pending_redo = redos_needed;
                        }

                        if is_hovered {
                            row_response.on_hover_text(format!("Click to redo {} action(s)", redos_needed));
                        }

                        ui.add_space(1.0);
                        let _ = row_id;
                    }
                    ui.add_space(8.0);
                }

                // Current state indicator
                ui.horizontal(|ui| {
                    ui.label(RichText::new(CIRCLE).size(10.0).color(theme_colors.semantic_success));
                    ui.label(RichText::new("Current State").size(11.0).color(theme_colors.semantic_success));
                });
                ui.add_space(8.0);

                // Show undo stack (past actions) - displayed below current state
                // Clicking on an undo item will undo all actions back to that point
                if !undo_descriptions.is_empty() {
                    ui.label(RichText::new("Undo Stack").size(11.0).color(theme_colors.text_muted));
                    ui.add_space(4.0);

                    // Show undo items in reverse order (most recent at top)
                    // Index 0 in reversed = last item in original = most recent = 1 undo
                    for (display_i, desc) in undo_descriptions.iter().rev().enumerate() {
                        // Number of undos needed: display_i + 1 (to reach the state BEFORE this action)
                        let undos_needed = display_i + 1;
                        let is_next = display_i == 0;

                        let row_id = ui.id().with(("undo", display_i));
                        let available_width = ui.available_width();
                        let (row_rect, row_response) = ui.allocate_exact_size(
                            Vec2::new(available_width, 24.0),
                            Sense::click(),
                        );

                        let is_hovered = row_response.hovered();
                        if is_hovered {
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        }
                        let bg_color = if is_hovered {
                            theme_colors.widget_hovered_bg
                        } else if display_i % 2 == 0 {
                            theme_colors.row_even
                        } else {
                            theme_colors.row_odd
                        };

                        ui.painter().rect_filled(row_rect, 3.0, bg_color);

                        // Draw icon and text
                        let icon_pos = egui::pos2(row_rect.min.x + 8.0, row_rect.center().y);
                        let text_pos = egui::pos2(row_rect.min.x + 26.0, row_rect.center().y);

                        let icon_color = if is_hovered {
                            theme_colors.semantic_accent
                        } else if is_next {
                            theme_colors.semantic_accent
                        } else {
                            theme_colors.text_muted
                        };
                        let text_color = if is_hovered {
                            theme_colors.text_primary
                        } else if is_next {
                            theme_colors.text_primary
                        } else {
                            theme_colors.text_secondary
                        };

                        ui.painter().text(
                            icon_pos,
                            egui::Align2::LEFT_CENTER,
                            ARROW_U_UP_LEFT,
                            egui::FontId::proportional(12.0),
                            icon_color,
                        );
                        ui.painter().text(
                            text_pos,
                            egui::Align2::LEFT_CENTER,
                            desc,
                            egui::FontId::proportional(12.0),
                            text_color,
                        );

                        if row_response.clicked() {
                            command_history.pending_undo = undos_needed;
                        }

                        if is_hovered {
                            row_response.on_hover_text(format!("Click to undo {} action(s)", undos_needed));
                        }

                        ui.add_space(1.0);
                        let _ = row_id;
                    }
                }

                ui.add_space(16.0);

                // Stats
                ui.separator();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("Undo: {}", undo_count)).size(11.0).color(theme_colors.text_muted));
                    ui.label(RichText::new("|").size(11.0).color(theme_colors.widget_border));
                    ui.label(RichText::new(format!("Redo: {}", redo_count)).size(11.0).color(theme_colors.text_muted));
                });
            });
        });
}

/// Render asset information in the inspector when an asset is selected
pub fn render_asset_inspector(
    ui: &mut egui::Ui,
    asset_path: &std::path::PathBuf,
    thumbnail_cache: &mut crate::core::ThumbnailCache,
    theme: &Theme,
) {
    use egui_phosphor::regular::{FILE, INFO};

    let filename = asset_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown");

    let extension = asset_path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    // Header with asset name
    let (accent_color, _) = get_category_colors(theme, "environment");
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::new(4.0, 20.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 0.0, accent_color);

        ui.label(RichText::new(IMAGE).size(14.0).color(accent_color));
        ui.label(RichText::new(filename).size(14.0).strong());
    });

    ui.add_space(8.0);

    // Check if it's an image file
    let is_image = matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "bmp" | "tga" | "webp" | "hdr" | "exr");

    if is_image {
        // Show thumbnail preview
        let preview_size = (ui.available_width() - 16.0).min(200.0);

        ui.add_space(4.0);

        // Center the preview
        ui.horizontal(|ui| {
            let padding = (ui.available_width() - preview_size) / 2.0;
            ui.add_space(padding.max(0.0));

            // Preview frame
            let (rect, _) = ui.allocate_exact_size(
                Vec2::splat(preview_size),
                egui::Sense::hover(),
            );

            // Draw checkerboard background for transparency
            draw_checkerboard_pattern(ui.painter(), rect);

            // Draw border
            ui.painter().rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(1.0, Color32::from_rgb(60, 62, 68)),
                egui::StrokeKind::Inside,
            );

            // Try to show the thumbnail
            if let Some(texture_id) = thumbnail_cache.get_texture_id(asset_path) {
                ui.painter().image(
                    texture_id,
                    rect.shrink(2.0),
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            } else if thumbnail_cache.has_failed(asset_path) {
                // Show error message
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Preview unavailable",
                    egui::FontId::proportional(11.0),
                    Color32::from_rgb(150, 100, 100),
                );
            } else {
                // Request load if not already loading
                if !thumbnail_cache.is_loading(asset_path) {
                    thumbnail_cache.request_load(asset_path.clone());
                }
                // Show loading indicator
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Loading...",
                    egui::FontId::proportional(11.0),
                    Color32::from_rgb(140, 142, 148),
                );
            }
        });

        ui.add_space(8.0);
    }

    ui.separator();
    ui.add_space(4.0);

    // Information section
    render_category(
        ui,
        INFO,
        "Information",
        "environment",
        theme,
        "asset_info",
        true,
        |ui| {
            let mut row = 0;

            // File type
            inline_property(ui, row, "Type", |ui| {
                let type_name = match extension.as_str() {
                    "png" => "PNG Image",
                    "jpg" | "jpeg" => "JPEG Image",
                    "bmp" => "Bitmap Image",
                    "tga" => "TGA Image",
                    "webp" => "WebP Image",
                    "hdr" => "HDR Image",
                    "exr" => "OpenEXR Image",
                    "glb" => "glTF Binary",
                    "gltf" => "glTF Model",
                    "obj" => "OBJ Model",
                    "fbx" => "FBX Model",
                    "wav" => "WAV Audio",
                    "ogg" => "OGG Audio",
                    "mp3" => "MP3 Audio",
                    "rhai" => "Rhai Script",
                    "ron" => "Scene File",
                    "material_bp" => "Material Blueprint",
                    _ => "Unknown",
                };
                ui.label(type_name);
            });
            row += 1;

            // File size
            if let Ok(metadata) = std::fs::metadata(asset_path) {
                let size_bytes = metadata.len();
                let size_str = if size_bytes < 1024 {
                    format!("{} B", size_bytes)
                } else if size_bytes < 1024 * 1024 {
                    format!("{:.1} KB", size_bytes as f64 / 1024.0)
                } else {
                    format!("{:.2} MB", size_bytes as f64 / (1024.0 * 1024.0))
                };

                inline_property(ui, row, "Size", |ui| {
                    ui.label(size_str);
                });
                row += 1;
            }

            // For images, try to get dimensions
            if is_image {
                if let Some(dims) = get_image_dimensions(asset_path) {
                    inline_property(ui, row, "Dimensions", |ui| {
                        ui.label(format!("{} x {}", dims.0, dims.1));
                    });
                    row += 1;

                    // Aspect ratio
                    let aspect = dims.0 as f32 / dims.1 as f32;
                    inline_property(ui, row, "Aspect Ratio", |ui| {
                        ui.label(format!("{:.3}", aspect));
                    });
                    row += 1;

                    // Total pixels
                    let total_pixels = dims.0 as u64 * dims.1 as u64;
                    let megapixels = total_pixels as f64 / 1_000_000.0;
                    inline_property(ui, row, "Pixels", |ui| {
                        if megapixels >= 1.0 {
                            ui.label(format!("{:.2} MP", megapixels));
                        } else {
                            ui.label(format!("{}", total_pixels));
                        }
                    });
                    row += 1;
                }
            }

            // File path
            inline_property(ui, row, "Path", |ui| {
                let display_path = asset_path.to_string_lossy();
                let truncated = if display_path.len() > 40 {
                    format!("...{}", &display_path[display_path.len() - 37..])
                } else {
                    display_path.to_string()
                };
                ui.label(RichText::new(truncated).size(10.0));
            });
        },
    );
}

/// Draw a checkerboard pattern for transparent image backgrounds
fn draw_checkerboard_pattern(painter: &egui::Painter, rect: egui::Rect) {
    let check_size = 10.0;
    let light = Color32::from_rgb(55, 55, 60);
    let dark = Color32::from_rgb(40, 40, 45);

    let cols = (rect.width() / check_size).ceil() as i32;
    let rows = (rect.height() / check_size).ceil() as i32;

    for row in 0..rows {
        for col in 0..cols {
            let color = if (row + col) % 2 == 0 { light } else { dark };
            let check_rect = egui::Rect::from_min_size(
                egui::pos2(rect.min.x + col as f32 * check_size, rect.min.y + row as f32 * check_size),
                Vec2::splat(check_size),
            ).intersect(rect);
            painter.rect_filled(check_rect, 0.0, color);
        }
    }
}

/// Try to get image dimensions without fully loading the image
fn get_image_dimensions(path: &std::path::PathBuf) -> Option<(u32, u32)> {
    // Use image crate to quickly read dimensions
    match image::image_dimensions(path) {
        Ok(dims) => Some(dims),
        Err(_) => None,
    }
}
