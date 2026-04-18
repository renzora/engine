//! Tree widget — indented rows with connecting lines, expand/collapse, and selection.
//!
//! The widget handles rendering; the caller drives recursion and provides data.
//!
//! # Usage
//! ```ignore
//! fn render_node(ui, depth, is_last, parent_lines, row_index, theme) {
//!     let result = tree_row(ui, &TreeRowConfig {
//!         depth,
//!         is_last,
//!         parent_lines,
//!         row_index: *row_index,
//!         has_children: !children.is_empty(),
//!         is_expanded: expanded.contains(&id),
//!         is_selected: selection.contains(&id),
//!         icon: Some(CUBE),
//!         label: &name,
//!         theme,
//!         ..Default::default()
//!     });
//!
//!     if result.expand_toggled { expanded.toggle(id); }
//!     if result.clicked { selection.select(id); }
//!     *row_index += 1;
//!
//!     if expanded.contains(&id) {
//!         for (i, child) in children.iter().enumerate() {
//!             parent_lines.push(!is_last);
//!             render_node(ui, depth + 1, i == children.len() - 1, parent_lines, row_index, theme);
//!             parent_lines.pop();
//!         }
//!     }
//! }
//! ```

use bevy_egui::egui::{self, Color32, CursorIcon, Pos2, Sense, Stroke, Vec2};
use egui_phosphor::regular::{CARET_DOWN, CARET_RIGHT};
use renzora_theme::Theme;

/// Indentation per depth level in logical pixels.
pub const INDENT_SIZE: f32 = 18.0;

/// Height of a single tree row.
pub const ROW_HEIGHT: f32 = 24.0;

/// How many extra pixels the vertical lines extend beyond the row to connect seamlessly.
const LINE_OVERLAP: f32 = 3.0;

/// Configuration for a single tree row.
pub struct TreeRowConfig<'a> {
    /// Stable identity for this row (used as the egui interaction ID).
    /// When set, the row's response ID is deterministic regardless of rendering order.
    pub stable_id: Option<egui::Id>,
    /// Nesting depth (0 = root).
    pub depth: usize,
    /// Is this the last sibling at its depth?
    pub is_last: bool,
    /// One entry per ancestor depth: `true` = that ancestor has more siblings below
    /// (draw a vertical continuation line). Caller pushes/pops around recursion.
    pub parent_lines: &'a [bool],
    /// Sequential row counter for alternating backgrounds.
    pub row_index: usize,
    /// Does this node have children?
    pub has_children: bool,
    /// Is the node currently expanded?
    pub is_expanded: bool,
    /// Is the node currently selected?
    pub is_selected: bool,
    /// Optional icon glyph (Phosphor).
    pub icon: Option<&'a str>,
    /// Icon color (defaults to theme muted).
    pub icon_color: Option<Color32>,
    /// Display label.
    pub label: &'a str,
    /// Optional left-edge color stripe `[r, g, b]`.
    pub label_color: Option<[u8; 3]>,
    /// Theme reference.
    pub theme: &'a Theme,
    /// Extra horizontal space reserved after the caret (before the icon) for caller-drawn content.
    /// The caller can use `TreeRowResult::prefix_rect` to draw into this area.
    pub prefix_width: f32,
}

/// Result of rendering a tree row.
pub struct TreeRowResult {
    /// The row's `egui::Response` (for drag-and-drop, context menus, etc.).
    pub response: egui::Response,
    /// The expand/collapse caret was clicked.
    pub expand_toggled: bool,
    /// The row itself was clicked (not the caret).
    pub clicked: bool,
    /// Ctrl was held during click.
    pub ctrl_click: bool,
    /// Shift was held during click.
    pub shift_click: bool,
    /// The allocated rect for this row.
    pub rect: egui::Rect,
    /// Rect reserved for caller-drawn prefix content (between caret and icon).
    /// Only meaningful when `TreeRowConfig::prefix_width > 0`.
    pub prefix_rect: egui::Rect,
}

/// Render a single tree row. Call this inside a vertical layout, once per visible node.
pub fn tree_row(ui: &mut egui::Ui, cfg: &TreeRowConfig<'_>) -> TreeRowResult {
    let theme = cfg.theme;
    let tree_line_color = theme.widgets.border.to_color32();
    let selection_color = theme.semantic.selection.to_color32();

    // Allocate row with stable ID if provided
    let (rect, response) = if let Some(stable_id) = cfg.stable_id {
        let rect = ui.allocate_space(Vec2::new(ui.available_width(), ROW_HEIGHT));
        let response = ui.interact(rect.1, stable_id, Sense::click_and_drag());
        (rect.1, response)
    } else {
        ui.allocate_exact_size(
            Vec2::new(ui.available_width(), ROW_HEIGHT),
            Sense::click_and_drag(),
        )
    };
    let painter = ui.painter();

    // --- Alternating row background ---
    if cfg.row_index % 2 == 1 {
        painter.rect_filled(rect, 0.0, theme.panels.inspector_row_odd.to_color32());
    }

    // --- Label color stripe ---
    if let Some([r, g, b]) = cfg.label_color {
        painter.rect_filled(
            rect,
            0.0,
            Color32::from_rgba_unmultiplied(r, g, b, 25),
        );
        let stripe = egui::Rect::from_min_max(rect.min, Pos2::new(rect.min.x + 3.0, rect.max.y));
        painter.rect_filled(stripe, 0.0, Color32::from_rgb(r, g, b));
    }

    // --- Selection / hover highlight ---
    if cfg.is_selected {
        let [r, g, b, _] = selection_color.to_array();
        painter.rect_filled(
            rect,
            0.0,
            Color32::from_rgba_unmultiplied(r, g, b, 160),
        );
        painter.rect_stroke(
            rect.shrink(0.5),
            0.0,
            Stroke::new(1.0, Color32::from_rgb(r, g, b)),
            egui::StrokeKind::Inside,
        );
    } else if response.hovered() {
        let [r, g, b, _] = theme.widgets.hovered_bg.to_color32().to_array();
        painter.rect_filled(
            rect,
            0.0,
            Color32::from_rgba_unmultiplied(r, g, b, 40),
        );
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    let base_x = rect.min.x + 4.0;
    let center_y = rect.center().y;
    let line_stroke = Stroke::new(1.5, tree_line_color);
    let line_x_offset = INDENT_SIZE / 2.0 - 1.0;

    // --- Vertical continuation lines for ancestors ---
    for (level, &has_more) in cfg.parent_lines.iter().enumerate() {
        if has_more {
            let x = base_x + (level as f32 * INDENT_SIZE) + line_x_offset;
            painter.line_segment(
                [
                    Pos2::new(x, rect.min.y - LINE_OVERLAP),
                    Pos2::new(x, rect.max.y + LINE_OVERLAP),
                ],
                line_stroke,
            );
        }
    }

    // --- Connector lines for this node ---
    if cfg.depth > 0 {
        let x = base_x + ((cfg.depth - 1) as f32 * INDENT_SIZE) + line_x_offset;
        let h_end = base_x + (cfg.depth as f32 * INDENT_SIZE) - 2.0;

        if cfg.is_last {
            // └ shape
            painter.line_segment(
                [Pos2::new(x, rect.min.y - LINE_OVERLAP), Pos2::new(x, center_y)],
                line_stroke,
            );
        } else {
            // ├ shape
            painter.line_segment(
                [
                    Pos2::new(x, rect.min.y - LINE_OVERLAP),
                    Pos2::new(x, rect.max.y + LINE_OVERLAP),
                ],
                line_stroke,
            );
        }
        // Horizontal arm
        painter.line_segment(
            [Pos2::new(x, center_y), Pos2::new(h_end, center_y)],
            line_stroke,
        );
    }

    // --- Expand/collapse caret ---
    let content_x = base_x + (cfg.depth as f32 * INDENT_SIZE);
    let mut expand_toggled = false;

    if cfg.has_children {
        let (icon, color) = if cfg.is_expanded {
            (CARET_DOWN, Color32::from_rgb(150, 150, 160))
        } else {
            (CARET_RIGHT, Color32::from_rgb(110, 110, 120))
        };

        let caret_rect = egui::Rect::from_min_size(
            Pos2::new(content_x, rect.min.y),
            Vec2::new(16.0, ROW_HEIGHT),
        );
        let caret_id = ui.id().with(("tree_caret", cfg.row_index));
        let caret_resp = ui.interact(caret_rect, caret_id, Sense::click());

        painter.text(
            caret_rect.center(),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(12.0),
            color,
        );

        if caret_resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if caret_resp.clicked() {
            expand_toggled = true;
        }
    }

    let after_caret_x = content_x + 16.0;

    // --- Prefix area (caller-drawn content between caret and icon) ---
    let prefix_rect = egui::Rect::from_min_size(
        Pos2::new(after_caret_x, rect.min.y),
        Vec2::new(cfg.prefix_width, ROW_HEIGHT),
    );
    let after_prefix_x = after_caret_x + cfg.prefix_width;

    // --- Icon ---
    let mut text_x = after_prefix_x;
    if let Some(icon_str) = cfg.icon {
        let ic = cfg.icon_color.unwrap_or_else(|| theme.text.muted.to_color32());
        painter.text(
            Pos2::new(text_x + 6.0, center_y),
            egui::Align2::LEFT_CENTER,
            icon_str,
            egui::FontId::proportional(14.0),
            ic,
        );
        text_x += 22.0;
    }

    // --- Label ---
    let label_color = if cfg.is_selected {
        Color32::WHITE
    } else {
        theme.text.primary.to_color32()
    };
    painter.text(
        Pos2::new(text_x + 4.0, center_y),
        egui::Align2::LEFT_CENTER,
        cfg.label,
        egui::FontId::proportional(13.0),
        label_color,
    );

    // --- Click detection (excluding caret area) ---
    let modifiers = ui.ctx().input(|i| i.modifiers);
    let row_clicked = response.clicked() && !expand_toggled;

    TreeRowResult {
        response,
        expand_toggled,
        clicked: row_clicked,
        ctrl_click: row_clicked && (modifiers.ctrl || modifiers.command),
        shift_click: row_clicked && modifiers.shift,
        rect,
        prefix_rect,
    }
}

/// Compute the drop position from a pointer's Y within a tree row rect.
///
/// Returns one of three zones, split into thirds of the row height:
/// - `Before` — top third
/// - `AsChild` — middle third
/// - `After` — bottom third
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeDropZone {
    Before,
    AsChild,
    After,
}

/// Determine which drop zone the pointer is in relative to `row_rect`.
pub fn tree_drop_zone(row_rect: egui::Rect, pointer_y: f32) -> TreeDropZone {
    let rel = pointer_y - row_rect.min.y;
    let zone = ROW_HEIGHT / 3.0;
    if rel < zone {
        TreeDropZone::Before
    } else if rel > ROW_HEIGHT - zone {
        TreeDropZone::After
    } else {
        TreeDropZone::AsChild
    }
}

/// Draw a drop indicator line (before or after a row).
pub fn draw_drop_line(ui: &mut egui::Ui, row_rect: egui::Rect, zone: TreeDropZone, theme: &Theme) {
    let color = theme.semantic.accent.to_color32();
    let y = match zone {
        TreeDropZone::Before => row_rect.min.y + 1.0,
        TreeDropZone::After => row_rect.max.y - 1.0,
        TreeDropZone::AsChild => return, // no line for child drop
    };
    let fg = ui.painter().clone().with_layer_id(egui::LayerId::new(
        egui::Order::Foreground,
        ui.id().with("drop_line"),
    ));
    fg.line_segment(
        [Pos2::new(row_rect.min.x, y), Pos2::new(row_rect.max.x, y)],
        Stroke::new(3.0, color),
    );
}
