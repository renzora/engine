//! Color utilities and theme color caching.

use bevy_egui::egui::{self, Color32, Vec2};
use renzora_theme::Theme;

// ── Color manipulation ──────────────────────────────────────────────────────

/// Dim a color by a brightness factor (0.0 = black, 1.0 = unchanged).
pub fn dim_color(color: Color32, factor: f32) -> Color32 {
    Color32::from_rgb(
        (color.r() as f32 * factor) as u8,
        (color.g() as f32 * factor) as u8,
        (color.b() as f32 * factor) as u8,
    )
}

/// Draw a checkerboard pattern (useful behind transparent images).
pub fn checkerboard(painter: &egui::Painter, rect: egui::Rect) {
    let size = 10.0;
    let light = Color32::from_rgb(55, 55, 60);
    let dark = Color32::from_rgb(40, 40, 45);

    let cols = (rect.width() / size).ceil() as i32;
    let rows = (rect.height() / size).ceil() as i32;

    for row in 0..rows {
        for col in 0..cols {
            let color = if (row + col) % 2 == 0 { light } else { dark };
            let cell = egui::Rect::from_min_size(
                egui::pos2(rect.min.x + col as f32 * size, rect.min.y + row as f32 * size),
                Vec2::splat(size),
            )
            .intersect(rect);
            painter.rect_filled(cell, 0.0, color);
        }
    }
}

// ── Theme color cache ────────────────────────────────────────────────────────

/// Flattened, pre-converted theme colors for fast per-frame access.
///
/// Store once via [`set_theme_colors`], retrieve anywhere via [`get_theme_colors`].
#[derive(Clone)]
pub struct ThemeColors {
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

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            row_even: Color32::from_rgb(32, 34, 38),
            row_odd: Color32::from_rgb(38, 40, 44),
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

impl ThemeColors {
    /// Build from a full `Theme`.
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

const CACHE_ID: &str = "renzora_theme_colors";

/// Store theme colors in the egui context (call once per frame).
pub fn set_theme_colors(ctx: &egui::Context, theme: &Theme) {
    let colors = ThemeColors::from_theme(theme);
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(CACHE_ID), colors));
}

/// Retrieve cached theme colors from the egui context.
pub fn get_theme_colors(ctx: &egui::Context) -> ThemeColors {
    ctx.data(|d| d.get_temp::<ThemeColors>(egui::Id::new(CACHE_ID)))
        .unwrap_or_default()
}
