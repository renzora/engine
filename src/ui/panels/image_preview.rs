#![allow(dead_code)]

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CursorIcon, Sense, Vec2};
use std::path::PathBuf;

use crate::core::{ImagePreviewTextures, SceneManagerState, OpenImage, TabKind};
use renzora_theme::Theme;

use egui_phosphor::regular::{MAGNIFYING_GLASS_PLUS, MAGNIFYING_GLASS_MINUS, ARROWS_OUT_CARDINAL, FRAME_CORNERS};

/// Open an image in the preview panel
pub fn open_image(scene_state: &mut SceneManagerState, path: PathBuf) {
    // Check if already open
    for (idx, image) in scene_state.open_images.iter().enumerate() {
        if image.path == path {
            scene_state.set_active_document(TabKind::Image(idx));
            return;
        }
    }

    let name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let new_idx = scene_state.open_images.len();
    scene_state.open_images.push(OpenImage {
        path,
        name,
        zoom: 1.0,
        pan_offset: (0.0, 0.0),
    });

    scene_state.tab_order.push(TabKind::Image(new_idx));
    scene_state.set_active_document(TabKind::Image(new_idx));
}

/// Render the image preview content for the active image
pub fn render_image_preview_content(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    scene_state: &mut SceneManagerState,
    theme: &Theme,
    textures: &mut ImagePreviewTextures,
) {
    let Some(active_idx) = scene_state.active_image_tab else {
        // No active image
        ui.centered_and_justified(|ui| {
            ui.label(egui::RichText::new("No image selected")
                .color(theme.text.muted.to_color32()));
        });
        return;
    };

    let Some(image) = scene_state.open_images.get_mut(active_idx) else {
        return;
    };

    let path = image.path.clone();
    let zoom = image.zoom;
    let pan_offset = image.pan_offset;

    // Toolbar
    let toolbar_height = 28.0;
    let available_rect = ui.available_rect_before_wrap();

    let toolbar_rect = egui::Rect::from_min_size(
        available_rect.min,
        Vec2::new(available_rect.width(), toolbar_height),
    );

    ui.painter().rect_filled(
        toolbar_rect,
        0.0,
        theme.surfaces.panel.to_color32(),
    );

    // Toolbar buttons
    let btn_size = Vec2::new(24.0, 24.0);
    let mut btn_x = toolbar_rect.min.x + 8.0;
    let btn_y = toolbar_rect.center().y;

    // Zoom in button
    let zoom_in_rect = egui::Rect::from_center_size(
        egui::pos2(btn_x + btn_size.x / 2.0, btn_y),
        btn_size,
    );
    let zoom_in_response = ui.interact(zoom_in_rect, ui.id().with("zoom_in"), Sense::click());
    if zoom_in_response.hovered() {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
        ui.painter().rect_filled(zoom_in_rect, 4.0, theme.widgets.hovered_bg.to_color32());
    }
    ui.painter().text(
        zoom_in_rect.center(),
        egui::Align2::CENTER_CENTER,
        MAGNIFYING_GLASS_PLUS,
        egui::FontId::proportional(14.0),
        theme.text.secondary.to_color32(),
    );
    btn_x += btn_size.x + 4.0;

    // Zoom out button
    let zoom_out_rect = egui::Rect::from_center_size(
        egui::pos2(btn_x + btn_size.x / 2.0, btn_y),
        btn_size,
    );
    let zoom_out_response = ui.interact(zoom_out_rect, ui.id().with("zoom_out"), Sense::click());
    if zoom_out_response.hovered() {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
        ui.painter().rect_filled(zoom_out_rect, 4.0, theme.widgets.hovered_bg.to_color32());
    }
    ui.painter().text(
        zoom_out_rect.center(),
        egui::Align2::CENTER_CENTER,
        MAGNIFYING_GLASS_MINUS,
        egui::FontId::proportional(14.0),
        theme.text.secondary.to_color32(),
    );
    btn_x += btn_size.x + 4.0;

    // Fit to view button
    let fit_rect = egui::Rect::from_center_size(
        egui::pos2(btn_x + btn_size.x / 2.0, btn_y),
        btn_size,
    );
    let fit_response = ui.interact(fit_rect, ui.id().with("fit_view"), Sense::click());
    if fit_response.hovered() {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
        ui.painter().rect_filled(fit_rect, 4.0, theme.widgets.hovered_bg.to_color32());
    }
    ui.painter().text(
        fit_rect.center(),
        egui::Align2::CENTER_CENTER,
        FRAME_CORNERS,
        egui::FontId::proportional(14.0),
        theme.text.secondary.to_color32(),
    );
    btn_x += btn_size.x + 4.0;

    // Reset pan button
    let reset_rect = egui::Rect::from_center_size(
        egui::pos2(btn_x + btn_size.x / 2.0, btn_y),
        btn_size,
    );
    let reset_response = ui.interact(reset_rect, ui.id().with("reset_pan"), Sense::click());
    if reset_response.hovered() {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
        ui.painter().rect_filled(reset_rect, 4.0, theme.widgets.hovered_bg.to_color32());
    }
    ui.painter().text(
        reset_rect.center(),
        egui::Align2::CENTER_CENTER,
        ARROWS_OUT_CARDINAL,
        egui::FontId::proportional(14.0),
        theme.text.secondary.to_color32(),
    );

    // Zoom percentage display
    let zoom_text = format!("{:.0}%", zoom * 100.0);
    ui.painter().text(
        egui::pos2(toolbar_rect.max.x - 50.0, btn_y),
        egui::Align2::CENTER_CENTER,
        &zoom_text,
        egui::FontId::proportional(11.0),
        theme.text.muted.to_color32(),
    );

    // Draw border under toolbar
    ui.painter().line_segment(
        [
            egui::pos2(toolbar_rect.min.x, toolbar_rect.max.y),
            egui::pos2(toolbar_rect.max.x, toolbar_rect.max.y),
        ],
        egui::Stroke::new(1.0, theme.widgets.border.to_color32()),
    );

    // Image preview area
    let preview_rect = egui::Rect::from_min_max(
        egui::pos2(available_rect.min.x, toolbar_rect.max.y + 1.0),
        available_rect.max,
    );

    // Draw checkerboard background for transparency
    draw_checkerboard(ui, preview_rect, theme);

    // Load or get cached texture
    let texture_handle = if let Some(handle) = textures.textures.get(&path) {
        Some(handle.clone())
    } else {
        // Try to load the image
        if let Ok(image_data) = std::fs::read(&path) {
            if let Ok(dynamic_image) = image::load_from_memory(&image_data) {
                let rgba = dynamic_image.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let pixels = rgba.into_raw();

                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
                let handle = ctx.load_texture(
                    path.to_string_lossy(),
                    color_image,
                    egui::TextureOptions::LINEAR,
                );
                textures.textures.insert(path.clone(), handle.clone());
                Some(handle)
            } else {
                None
            }
        } else {
            None
        }
    };

    // Handle zoom and pan interactions
    let preview_response = ui.interact(preview_rect, ui.id().with("image_preview"), Sense::click_and_drag());

    // Get mutable reference to update zoom/pan
    if let Some(image) = scene_state.open_images.get_mut(active_idx) {
        // Handle button clicks
        if zoom_in_response.clicked() {
            image.zoom = (image.zoom * 1.25).min(10.0);
        }
        if zoom_out_response.clicked() {
            image.zoom = (image.zoom / 1.25).max(0.1);
        }
        if fit_response.clicked() {
            // Calculate fit zoom
            if let Some(ref handle) = texture_handle {
                let tex_size = handle.size_vec2();
                let fit_zoom_x = preview_rect.width() / tex_size.x;
                let fit_zoom_y = preview_rect.height() / tex_size.y;
                image.zoom = fit_zoom_x.min(fit_zoom_y).min(1.0);
            }
            image.pan_offset = (0.0, 0.0);
        }
        if reset_response.clicked() {
            image.zoom = 1.0;
            image.pan_offset = (0.0, 0.0);
        }

        // Scroll to zoom
        let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
        if preview_response.hovered() && scroll_delta != 0.0 {
            let zoom_factor = if scroll_delta > 0.0 { 1.1 } else { 0.9 };
            image.zoom = (image.zoom * zoom_factor).clamp(0.1, 10.0);
        }

        // Drag to pan
        if preview_response.dragged() {
            let delta = preview_response.drag_delta();
            image.pan_offset.0 += delta.x;
            image.pan_offset.1 += delta.y;
        }

        // Render the image
        if let Some(handle) = texture_handle {
            let tex_size = handle.size_vec2();
            let display_size = tex_size * image.zoom;

            let center = preview_rect.center();
            let image_rect = egui::Rect::from_center_size(
                egui::pos2(
                    center.x + image.pan_offset.0,
                    center.y + image.pan_offset.1,
                ),
                display_size,
            );

            // Clip to preview area
            ui.set_clip_rect(preview_rect);

            ui.painter().image(
                handle.id(),
                image_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );

            // Image info at bottom
            let info_text = format!(
                "{}x{} px",
                tex_size.x as u32,
                tex_size.y as u32,
            );
            ui.painter().text(
                egui::pos2(preview_rect.min.x + 8.0, preview_rect.max.y - 8.0),
                egui::Align2::LEFT_BOTTOM,
                &info_text,
                egui::FontId::proportional(10.0),
                theme.text.muted.to_color32(),
            );
        } else {
            // Show loading or error message
            ui.painter().text(
                preview_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Failed to load image",
                egui::FontId::proportional(14.0),
                theme.semantic.error.to_color32(),
            );
        }
    }
}

/// Draw a checkerboard pattern for transparency visualization
fn draw_checkerboard(ui: &mut egui::Ui, rect: egui::Rect, _theme: &Theme) {
    let check_size: f32 = 16.0;
    let color1 = Color32::from_rgb(40, 40, 45);
    let color2 = Color32::from_rgb(50, 50, 55);

    let start_x = rect.min.x;
    let start_y = rect.min.y;
    let end_x = rect.max.x;
    let end_y = rect.max.y;

    let mut y = start_y;
    let mut row = 0;
    while y < end_y {
        let mut x = start_x;
        let mut col = 0;
        while x < end_x {
            let color = if (row + col) % 2 == 0 { color1 } else { color2 };
            let check_rect = egui::Rect::from_min_size(
                egui::pos2(x, y),
                Vec2::new(
                    check_size.min(end_x - x),
                    check_size.min(end_y - y),
                ),
            );
            ui.painter().rect_filled(check_rect, 0.0, color);
            x += check_size;
            col += 1;
        }
        y += check_size;
        row += 1;
    }
}
