//! Tile grid widget — responsive wrapping grid of card tiles.
//!
//! # Usage
//! ```ignore
//! let grid = TileGrid::new(&theme)
//!     .zoom(zoom_factor)
//!     .available_width(ui.available_width());
//!
//! grid.show(ui, items.len(), |ui, index, tile| {
//!     let item = &items[index];
//!     let is_selected = selection.contains(&item.id);
//!
//!     // Draw icon or thumbnail into tile.icon_rect
//!     ui.painter().text(
//!         tile.icon_rect.center(), Align2::CENTER_CENTER,
//!         FOLDER, FontId::proportional(tile.icon_size), Color32::WHITE,
//!     );
//!
//!     // Draw label into tile.label_rect
//!     ui.painter().text(
//!         tile.label_pos(), Align2::CENTER_CENTER,
//!         &item.name, FontId::proportional(tile.font_size), Color32::WHITE,
//!     );
//!
//!     // Return state for the grid to apply styling
//!     TileState { is_selected, is_hovered: tile.response.hovered(), color: item.color }
//! });
//! ```

use bevy_egui::egui::{self, Color32, CornerRadius, Pos2, Sense, Stroke, StrokeKind, Vec2};
use renzora_theme::Theme;

/// Responsive tile grid configuration.
pub struct TileGrid<'a> {
    theme: &'a Theme,
    zoom: f32,
    available_width: f32,
}

impl<'a> TileGrid<'a> {
    pub fn new(theme: &'a Theme) -> Self {
        Self {
            theme,
            zoom: 1.0,
            available_width: 400.0,
        }
    }

    /// Zoom multiplier applied to base tile size (default 1.0).
    pub fn zoom(mut self, zoom: f32) -> Self {
        self.zoom = zoom.clamp(0.5, 3.0);
        self
    }

    /// Set available width for responsive sizing (call with `ui.available_width()`).
    pub fn available_width(mut self, w: f32) -> Self {
        self.available_width = w;
        self
    }

    /// Computed base tile size (before zoom) based on available width.
    fn base_tile_size(&self) -> f32 {
        if self.available_width < 150.0 {
            70.0
        } else if self.available_width < 250.0 {
            80.0
        } else if self.available_width < 400.0 {
            90.0
        } else {
            100.0
        }
    }

    /// Final tile size after zoom.
    pub fn tile_size(&self) -> f32 {
        self.base_tile_size() * self.zoom
    }

    /// Spacing between tiles.
    fn spacing(&self) -> f32 {
        if self.available_width < 200.0 { 4.0 } else { 8.0 }
    }

    /// Render the grid. `render_tile` is called once per tile with:
    /// - `ui` — painter/interaction handle
    /// - `index` — item index (0..count)
    /// - `TileLayout` — pre-computed rects for this tile
    ///
    /// The closure should return a [`TileState`] so the grid can draw backgrounds/borders.
    pub fn show(
        &self,
        ui: &mut egui::Ui,
        count: usize,
        mut render_tile: impl FnMut(&mut egui::Ui, usize, &TileLayout) -> TileState,
    ) {
        let tile_size = self.tile_size();
        let icon_size = (tile_size * 0.55).max(28.0);
        let thumbnail_size = tile_size - 16.0;
        let spacing = self.spacing();
        let label_area_height = 36.0;
        let separator_height = 3.0;
        let total_height = tile_size + separator_height + label_area_height;
        let font_size = if tile_size < 75.0 { 10.0 } else { 11.0 };

        let theme = self.theme;
        let item_bg = theme.panels.item_bg.to_color32();
        let item_hover = theme.panels.item_hover.to_color32();
        let selection_bg = theme.semantic.selection.to_color32();
        let selection_stroke = theme.semantic.selection_stroke.to_color32();
        let surface_faint = theme.surfaces.faint.to_color32();

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(spacing, spacing);

            for index in 0..count {
                let (rect, response) = ui.allocate_exact_size(
                    Vec2::new(tile_size, total_height),
                    Sense::click_and_drag(),
                );

                let icon_center = Pos2::new(rect.center().x, rect.min.y + tile_size / 2.0);
                let icon_rect =
                    egui::Rect::from_center_size(icon_center, Vec2::splat(icon_size));
                let thumbnail_rect =
                    egui::Rect::from_center_size(icon_center, Vec2::splat(thumbnail_size));
                let icon_area_rect =
                    egui::Rect::from_min_size(rect.min, Vec2::new(tile_size, tile_size));

                let separator_y = rect.min.y + tile_size;
                let separator_rect = egui::Rect::from_min_size(
                    Pos2::new(rect.min.x + 6.0, separator_y),
                    Vec2::new(tile_size - 12.0, separator_height),
                );

                let label_rect = egui::Rect::from_min_size(
                    Pos2::new(rect.min.x + 4.0, separator_y + separator_height + 2.0),
                    Vec2::new(tile_size - 8.0, label_area_height - 4.0),
                );

                let layout = TileLayout {
                    rect,
                    response: response.clone(),
                    icon_rect,
                    icon_area_rect,
                    thumbnail_rect,
                    separator_rect,
                    label_rect,
                    icon_size,
                    thumbnail_size,
                    font_size,
                    tile_size,
                };

                // --- Card background (paint before content) ---
                let is_hovered = response.hovered();
                {
                    let painter = ui.painter();

                    // Base card fill
                    let bg = if is_hovered { item_hover } else { item_bg };
                    painter.rect_filled(rect, CornerRadius::same(8), bg);

                    // Darker icon area when not highlighted
                    if !is_hovered {
                        painter.rect_filled(
                            icon_area_rect,
                            CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 },
                            surface_faint,
                        );
                    }
                }

                // Let the caller render content and report state
                let state = render_tile(ui, index, &layout);

                // --- Post-content painting (selected state, borders, separator) ---
                let painter = ui.painter();

                // Semi-transparent selection tint (preserves content visibility)
                if state.is_selected {
                    let [r, g, b, _] = selection_bg.to_array();
                    painter.rect_filled(
                        rect,
                        CornerRadius::same(8),
                        Color32::from_rgba_unmultiplied(r, g, b, 80),
                    );
                }

                // Borders
                if state.is_selected {
                    painter.rect_stroke(
                        rect,
                        8.0,
                        Stroke::new(2.0, selection_stroke),
                        StrokeKind::Inside,
                    );
                } else if state.is_hovered {
                    if let Some(c) = state.color {
                        painter.rect_stroke(
                            rect,
                            8.0,
                            Stroke::new(
                                1.0,
                                Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 60),
                            ),
                            StrokeKind::Inside,
                        );
                    }
                }

                // Color separator
                let sep_alpha = if state.is_selected || state.is_hovered { 200 } else { 120 };
                if let Some(c) = state.color {
                    painter.rect_filled(
                        separator_rect,
                        CornerRadius::same(1),
                        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), sep_alpha),
                    );
                }
            }
        });
    }
}

/// Pre-computed layout rects for a single tile, passed to the render closure.
pub struct TileLayout {
    /// Full tile rect.
    pub rect: egui::Rect,
    /// Interaction response for the tile.
    pub response: egui::Response,
    /// Rect for the icon (centered, `icon_size × icon_size`).
    pub icon_rect: egui::Rect,
    /// Full upper area of the tile (for background fill).
    pub icon_area_rect: egui::Rect,
    /// Rect for a thumbnail (centered, slightly smaller than icon area).
    pub thumbnail_rect: egui::Rect,
    /// Color separator strip between icon and label areas.
    pub separator_rect: egui::Rect,
    /// Rect for the label text (below separator).
    pub label_rect: egui::Rect,
    /// Computed icon font size.
    pub icon_size: f32,
    /// Computed thumbnail dimension.
    pub thumbnail_size: f32,
    /// Computed label font size.
    pub font_size: f32,
    /// Final tile width/height (square icon area).
    pub tile_size: f32,
}

impl TileLayout {
    /// Center position for the first line of label text.
    pub fn label_line1_pos(&self) -> Pos2 {
        let line_h = self.font_size + 2.0;
        Pos2::new(self.label_rect.center().x, self.label_rect.min.y + line_h / 2.0)
    }

    /// Center position for the second line of label text.
    pub fn label_line2_pos(&self) -> Pos2 {
        let line_h = self.font_size + 2.0;
        Pos2::new(
            self.label_rect.center().x,
            self.label_rect.min.y + line_h + line_h / 2.0,
        )
    }
}

/// State returned by the tile render closure so the grid can apply styling.
pub struct TileState {
    /// Is this tile currently selected?
    pub is_selected: bool,
    /// Is the pointer hovering this tile?
    pub is_hovered: bool,
    /// Optional accent color for border and separator.
    pub color: Option<Color32>,
}

/// Split a filename into two display lines that fit within `tile_width`.
///
/// Tries to break at the file extension dot. Truncates with `…` if needed.
pub fn split_label_two_lines(name: &str, tile_width: f32, font_size: f32) -> (String, String) {
    let char_w = font_size * 0.55;
    let max_chars = ((tile_width - 8.0) / char_w) as usize;
    let max_chars = max_chars.max(6);

    if name.len() <= max_chars {
        return (name.to_string(), String::new());
    }

    // Try splitting at extension
    if let Some(dot) = name.rfind('.') {
        let base = &name[..dot];
        let ext = &name[dot..];

        if base.len() <= max_chars && ext.len() <= max_chars {
            return (base.to_string(), ext.to_string());
        }
        if base.len() > max_chars {
            let trunc = format!("{}…", &base[..max_chars.saturating_sub(1)]);
            return (trunc, ext.to_string());
        }
    }

    // Split in the middle
    let mid = max_chars.min(name.len());
    let line1 = name[..mid].to_string();
    let rest = &name[mid..];
    let line2 = if rest.len() > max_chars {
        format!("{}…", &rest[..max_chars.saturating_sub(1)])
    } else {
        rest.to_string()
    };

    (line1, line2)
}
