//! Studio Preview panel - displays an isolated 3D preview with studio lighting
//!
//! This panel renders a separate camera view with professional lighting,
//! perfect for previewing animations, models, and materials.

use bevy_egui::egui::{self, Color32, Pos2, Rect, Sense, Stroke, TextureId, Vec2};

use renzora_theme::Theme;

/// Render the studio preview panel content
pub fn render_studio_preview_content(
    ui: &mut egui::Ui,
    texture_id: TextureId,
    size: (u32, u32),
    theme: &Theme,
) {
    let available = ui.available_rect_before_wrap();
    let text_muted = theme.text.muted.to_color32();
    let bg_color = theme.surfaces.panel.to_color32();

    // Draw background
    ui.painter().rect_filled(available, 0.0, bg_color);

    // Calculate size to fit while maintaining aspect ratio
    let texture_aspect = size.0 as f32 / size.1 as f32;
    let panel_aspect = available.width() / available.height();

    let (display_width, display_height) = if texture_aspect > panel_aspect {
        // Texture is wider - fit to width
        (available.width(), available.width() / texture_aspect)
    } else {
        // Texture is taller - fit to height
        (available.height() * texture_aspect, available.height())
    };

    let display_rect = Rect::from_center_size(
        available.center(),
        Vec2::new(display_width, display_height),
    );

    // Draw the preview image
    ui.painter().image(
        texture_id,
        display_rect,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );

    // Draw border around the preview
    ui.painter().rect_stroke(
        display_rect,
        0.0,
        Stroke::new(1.0, theme.widgets.border.to_color32()),
        egui::StrokeKind::Inside,
    );

    // Label in corner
    ui.painter().text(
        Pos2::new(available.min.x + 8.0, available.min.y + 8.0),
        egui::Align2::LEFT_TOP,
        "Studio Preview",
        egui::FontId::proportional(10.0),
        text_muted,
    );
}
