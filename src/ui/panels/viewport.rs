use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, TextureId};

use crate::core::{ViewportState, AssetBrowserState, OrbitCameraState};

pub fn render_viewport(
    ctx: &egui::Context,
    viewport: &mut ViewportState,
    assets: &mut AssetBrowserState,
    orbit: &OrbitCameraState,
    _left_panel_width: f32,
    _right_panel_width: f32,
    _content_start_y: f32,
    _window_size: [f32; 2],
    _content_height: f32,
    viewport_texture_id: Option<TextureId>,
) {
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(Color32::from_rgb(20, 20, 26)))
        .show(ctx, |ui| {
            render_viewport_content(ui, viewport, assets, orbit, viewport_texture_id);
        });
}

/// Render viewport content (for use in docking)
pub fn render_viewport_content(
    ui: &mut egui::Ui,
    viewport: &mut ViewportState,
    assets: &mut AssetBrowserState,
    orbit: &OrbitCameraState,
    viewport_texture_id: Option<TextureId>,
) {
    let ctx = ui.ctx().clone();
    let content_rect = ui.available_rect_before_wrap();
    viewport.position = [content_rect.min.x, content_rect.min.y];
    viewport.size = [content_rect.width(), content_rect.height()];
    viewport.hovered = ui.rect_contains_pointer(content_rect);

    // Display the viewport texture if available
    if let Some(texture_id) = viewport_texture_id {
        let image = egui::Image::new(egui::load::SizedTexture::new(
            texture_id,
            [content_rect.width(), content_rect.height()],
        ));
        ui.add(image);
    }

    // Handle asset drag and drop from assets panel
    let pointer_pos = ctx.pointer_hover_pos();
    let in_viewport = pointer_pos.map_or(false, |p| content_rect.contains(p));

    if assets.dragging_asset.is_some() && ctx.input(|i| i.pointer.any_released()) {
        if in_viewport {
            if let Some(asset_path) = assets.dragging_asset.take() {
                if let Some(mouse_pos) = pointer_pos {
                    let local_x = mouse_pos.x - content_rect.min.x;
                    let local_y = mouse_pos.y - content_rect.min.y;

                    let norm_x = local_x / content_rect.width();
                    let norm_y = local_y / content_rect.height();

                    // Calculate ground plane intersection
                    let camera_pos = calculate_camera_position(
                        orbit.focus,
                        orbit.distance,
                        orbit.yaw,
                        orbit.pitch,
                    );

                    let fov = std::f32::consts::FRAC_PI_4;
                    let aspect = content_rect.width() / content_rect.height();

                    let ndc_x = norm_x * 2.0 - 1.0;
                    let ndc_y = 1.0 - norm_y * 2.0;

                    let tan_fov = (fov / 2.0).tan();
                    let ray_view = Vec3::new(
                        ndc_x * tan_fov * aspect,
                        ndc_y * tan_fov,
                        -1.0,
                    )
                    .normalize();

                    let camera_forward = (orbit.focus - camera_pos).normalize();
                    let camera_right = camera_forward.cross(Vec3::Y).normalize();
                    let camera_up = camera_right.cross(camera_forward).normalize();

                    let ray_world = (camera_right * ray_view.x
                        + camera_up * ray_view.y
                        - camera_forward * ray_view.z)
                        .normalize();

                    let ground_point = if ray_world.y.abs() > 0.0001 {
                        let t = -camera_pos.y / ray_world.y;
                        if t > 0.0 {
                            camera_pos + ray_world * t
                        } else {
                            Vec3::ZERO
                        }
                    } else {
                        Vec3::ZERO
                    };

                    assets.pending_asset_drop = Some((asset_path, ground_point));
                }
            }
        } else {
            // Released outside viewport, cancel the drag
            assets.dragging_asset = None;
        }
    }
}

/// Calculate camera position from orbit parameters
fn calculate_camera_position(focus: Vec3, distance: f32, yaw: f32, pitch: f32) -> Vec3 {
    let x = focus.x + distance * pitch.cos() * yaw.sin();
    let y = focus.y + distance * pitch.sin();
    let z = focus.z + distance * pitch.cos() * yaw.cos();
    Vec3::new(x, y, z)
}
