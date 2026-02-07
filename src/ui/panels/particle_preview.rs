//! Particle Preview panel - standalone viewport for particle effect preview
//!
//! Displays the isolated particle preview render texture in a dedicated panel.
//! Simple dark background to visualize particle effects.

use bevy_egui::egui::{self, Color32, Pos2, Rect, TextureId, Vec2};

/// Render the particle preview panel content
pub fn render_particle_preview_content(
    ui: &mut egui::Ui,
    texture_id: Option<TextureId>,
    size: (u32, u32),
) {
    let available = ui.available_rect_before_wrap();

    // Dark background for particle visualization
    ui.painter().rect_filled(available, 0.0, Color32::from_rgb(20, 20, 25));

    if let Some(tex_id) = texture_id {
        // Calculate size to fit while maintaining aspect ratio
        let texture_aspect = size.0 as f32 / size.1.max(1) as f32;
        let panel_aspect = available.width() / available.height();

        let (display_width, display_height) = if texture_aspect > panel_aspect {
            (available.width(), available.width() / texture_aspect)
        } else {
            (available.height() * texture_aspect, available.height())
        };

        let display_rect = Rect::from_center_size(
            available.center(),
            Vec2::new(display_width, display_height),
        );

        // Draw the preview image
        ui.painter().image(
            tex_id,
            display_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        // Placeholder when no texture available
        ui.painter().text(
            available.center(),
            egui::Align2::CENTER_CENTER,
            "Particle Preview\n\nNo effect loaded",
            egui::FontId::proportional(14.0),
            Color32::from_gray(100),
        );
    }
}
