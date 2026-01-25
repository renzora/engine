use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, FontId, Pos2, Rect, Stroke, TextureId, Vec2};

use crate::core::{ViewportMode, ViewportState, AssetBrowserState, OrbitCameraState};
use crate::viewport::Camera2DState;

/// Height of the viewport mode tabs bar
const VIEWPORT_TABS_HEIGHT: f32 = 24.0;
/// Width of rulers in 2D mode
const RULER_SIZE: f32 = 20.0;

pub fn render_viewport(
    ctx: &egui::Context,
    viewport: &mut ViewportState,
    assets: &mut AssetBrowserState,
    orbit: &OrbitCameraState,
    camera2d_state: &Camera2DState,
    left_panel_width: f32,
    right_panel_width: f32,
    content_start_y: f32,
    _window_size: [f32; 2],
    content_height: f32,
    viewport_texture_id: Option<TextureId>,
) {
    let screen_rect = ctx.screen_rect();

    // Calculate the full viewport area
    let full_viewport_rect = Rect::from_min_size(
        Pos2::new(left_panel_width, content_start_y),
        Vec2::new(
            screen_rect.width() - left_panel_width - right_panel_width,
            content_height,
        ),
    );

    // Render viewport mode tabs at the top
    render_viewport_tabs(ctx, viewport, full_viewport_rect);

    // Calculate content rect (below tabs, accounting for rulers in 2D mode)
    let tabs_offset = VIEWPORT_TABS_HEIGHT;
    let ruler_offset = if viewport.viewport_mode == ViewportMode::Mode2D { RULER_SIZE } else { 0.0 };

    let content_rect = Rect::from_min_size(
        Pos2::new(full_viewport_rect.min.x + ruler_offset, full_viewport_rect.min.y + tabs_offset + ruler_offset),
        Vec2::new(
            full_viewport_rect.width() - ruler_offset,
            full_viewport_rect.height() - tabs_offset - ruler_offset,
        ),
    );

    // Render rulers in 2D mode
    if viewport.viewport_mode == ViewportMode::Mode2D {
        render_rulers(ctx, viewport, camera2d_state, full_viewport_rect, tabs_offset);
    }

    // Use an Area to render the viewport content
    egui::Area::new(egui::Id::new("viewport_area"))
        .fixed_pos(content_rect.min)
        .order(egui::Order::Background)
        .show(ctx, |ui| {
            ui.set_clip_rect(content_rect);
            render_viewport_content(ui, viewport, assets, orbit, viewport_texture_id, content_rect);
        });
}

/// Render the 3D/2D mode tabs at the top of the viewport
fn render_viewport_tabs(ctx: &egui::Context, viewport: &mut ViewportState, full_rect: Rect) {
    let tabs_rect = Rect::from_min_size(
        full_rect.min,
        Vec2::new(full_rect.width(), VIEWPORT_TABS_HEIGHT),
    );

    egui::Area::new(egui::Id::new("viewport_tabs"))
        .fixed_pos(tabs_rect.min)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            ui.set_clip_rect(tabs_rect);

            // Background
            ui.painter().rect_filled(tabs_rect, 0.0, Color32::from_rgb(35, 35, 40));

            // Tab buttons
            let tab_width = 50.0;
            let tab_height = VIEWPORT_TABS_HEIGHT - 4.0;
            let tab_y = tabs_rect.min.y + 2.0;

            // 3D tab
            let tab_3d_rect = Rect::from_min_size(
                Pos2::new(tabs_rect.min.x + 4.0, tab_y),
                Vec2::new(tab_width, tab_height),
            );
            let is_3d_active = viewport.viewport_mode == ViewportMode::Mode3D;
            let tab_3d_color = if is_3d_active {
                Color32::from_rgb(60, 60, 70)
            } else {
                Color32::from_rgb(45, 45, 50)
            };
            ui.painter().rect_filled(tab_3d_rect, 3.0, tab_3d_color);
            ui.painter().text(
                tab_3d_rect.center(),
                egui::Align2::CENTER_CENTER,
                "3D",
                FontId::proportional(12.0),
                if is_3d_active { Color32::WHITE } else { Color32::from_rgb(150, 150, 160) },
            );

            // 2D tab
            let tab_2d_rect = Rect::from_min_size(
                Pos2::new(tabs_rect.min.x + 4.0 + tab_width + 4.0, tab_y),
                Vec2::new(tab_width, tab_height),
            );
            let is_2d_active = viewport.viewport_mode == ViewportMode::Mode2D;
            let tab_2d_color = if is_2d_active {
                Color32::from_rgb(60, 60, 70)
            } else {
                Color32::from_rgb(45, 45, 50)
            };
            ui.painter().rect_filled(tab_2d_rect, 3.0, tab_2d_color);
            ui.painter().text(
                tab_2d_rect.center(),
                egui::Align2::CENTER_CENTER,
                "2D",
                FontId::proportional(12.0),
                if is_2d_active { Color32::WHITE } else { Color32::from_rgb(150, 150, 160) },
            );

            // Handle clicks
            if ui.ctx().input(|i| i.pointer.any_click()) {
                if let Some(pos) = ui.ctx().pointer_hover_pos() {
                    if tab_3d_rect.contains(pos) {
                        viewport.viewport_mode = ViewportMode::Mode3D;
                    } else if tab_2d_rect.contains(pos) {
                        viewport.viewport_mode = ViewportMode::Mode2D;
                    }
                }
            }
        });
}

/// Render rulers for 2D mode
fn render_rulers(
    ctx: &egui::Context,
    viewport: &ViewportState,
    camera2d_state: &Camera2DState,
    full_rect: Rect,
    tabs_offset: f32,
) {
    let ruler_bg = Color32::from_rgb(40, 40, 45);
    let ruler_tick = Color32::from_rgb(100, 100, 110);
    let ruler_text = Color32::from_rgb(140, 140, 150);
    let ruler_major_tick = Color32::from_rgb(150, 150, 160);

    // Horizontal ruler (top)
    let h_ruler_rect = Rect::from_min_size(
        Pos2::new(full_rect.min.x + RULER_SIZE, full_rect.min.y + tabs_offset),
        Vec2::new(full_rect.width() - RULER_SIZE, RULER_SIZE),
    );

    egui::Area::new(egui::Id::new("h_ruler"))
        .fixed_pos(h_ruler_rect.min)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            ui.set_clip_rect(h_ruler_rect);
            ui.painter().rect_filled(h_ruler_rect, 0.0, ruler_bg);

            // Calculate tick spacing based on zoom
            let tick_spacing = calculate_ruler_tick_spacing(camera2d_state.zoom);
            let world_left = camera2d_state.pan_offset.x - (viewport.size[0] / camera2d_state.zoom / 2.0);
            let world_right = camera2d_state.pan_offset.x + (viewport.size[0] / camera2d_state.zoom / 2.0);

            // Draw tick marks
            let start_tick = (world_left / tick_spacing).floor() as i32;
            let end_tick = (world_right / tick_spacing).ceil() as i32;

            for i in start_tick..=end_tick {
                let world_x = i as f32 * tick_spacing;
                let screen_x = world_to_screen_x(world_x, camera2d_state, viewport);
                let local_x = screen_x - full_rect.min.x;

                if local_x < RULER_SIZE || local_x > full_rect.width() {
                    continue;
                }

                let is_major = i % 10 == 0;
                let tick_height = if is_major { RULER_SIZE * 0.6 } else { RULER_SIZE * 0.3 };
                let tick_y = h_ruler_rect.max.y - tick_height;

                ui.painter().line_segment(
                    [Pos2::new(screen_x, tick_y), Pos2::new(screen_x, h_ruler_rect.max.y)],
                    Stroke::new(1.0, if is_major { ruler_major_tick } else { ruler_tick }),
                );

                // Draw label for major ticks
                if is_major {
                    ui.painter().text(
                        Pos2::new(screen_x + 2.0, h_ruler_rect.min.y + 2.0),
                        egui::Align2::LEFT_TOP,
                        format!("{}", world_x as i32),
                        FontId::proportional(9.0),
                        ruler_text,
                    );
                }
            }
        });

    // Vertical ruler (left)
    let v_ruler_rect = Rect::from_min_size(
        Pos2::new(full_rect.min.x, full_rect.min.y + tabs_offset + RULER_SIZE),
        Vec2::new(RULER_SIZE, full_rect.height() - tabs_offset - RULER_SIZE),
    );

    egui::Area::new(egui::Id::new("v_ruler"))
        .fixed_pos(v_ruler_rect.min)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            ui.set_clip_rect(v_ruler_rect);
            ui.painter().rect_filled(v_ruler_rect, 0.0, ruler_bg);

            // Calculate tick spacing based on zoom
            let tick_spacing = calculate_ruler_tick_spacing(camera2d_state.zoom);
            let world_bottom = camera2d_state.pan_offset.y - (viewport.size[1] / camera2d_state.zoom / 2.0);
            let world_top = camera2d_state.pan_offset.y + (viewport.size[1] / camera2d_state.zoom / 2.0);

            // Draw tick marks
            let start_tick = (world_bottom / tick_spacing).floor() as i32;
            let end_tick = (world_top / tick_spacing).ceil() as i32;

            for i in start_tick..=end_tick {
                let world_y = i as f32 * tick_spacing;
                let screen_y = world_to_screen_y(world_y, camera2d_state, viewport);
                let local_y = screen_y - full_rect.min.y - tabs_offset;

                if local_y < RULER_SIZE || local_y > full_rect.height() - tabs_offset {
                    continue;
                }

                let is_major = i % 10 == 0;
                let tick_width = if is_major { RULER_SIZE * 0.6 } else { RULER_SIZE * 0.3 };
                let tick_x = v_ruler_rect.max.x - tick_width;

                ui.painter().line_segment(
                    [Pos2::new(tick_x, screen_y), Pos2::new(v_ruler_rect.max.x, screen_y)],
                    Stroke::new(1.0, if is_major { ruler_major_tick } else { ruler_tick }),
                );

                // Draw label for major ticks
                if is_major {
                    ui.painter().text(
                        Pos2::new(v_ruler_rect.min.x + 2.0, screen_y - 8.0),
                        egui::Align2::LEFT_TOP,
                        format!("{}", world_y as i32),
                        FontId::proportional(9.0),
                        ruler_text,
                    );
                }
            }
        });

    // Corner square (top-left)
    let corner_rect = Rect::from_min_size(
        Pos2::new(full_rect.min.x, full_rect.min.y + tabs_offset),
        Vec2::new(RULER_SIZE, RULER_SIZE),
    );
    egui::Area::new(egui::Id::new("ruler_corner"))
        .fixed_pos(corner_rect.min)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            ui.painter().rect_filled(corner_rect, 0.0, ruler_bg);
        });
}

/// Calculate tick spacing for rulers based on zoom level
fn calculate_ruler_tick_spacing(zoom: f32) -> f32 {
    // Target spacing in screen pixels
    let target_spacing = 50.0;
    let ideal_world_spacing = target_spacing / zoom;

    // Round to a nice number
    let nice_numbers = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0];
    let mut best = nice_numbers[0];
    let mut best_diff = (ideal_world_spacing - best).abs();

    for &nice in &nice_numbers {
        let diff = (ideal_world_spacing - nice).abs();
        if diff < best_diff {
            best_diff = diff;
            best = nice;
        }
    }
    best
}

/// Convert world X coordinate to screen X coordinate
fn world_to_screen_x(world_x: f32, camera2d_state: &Camera2DState, viewport: &ViewportState) -> f32 {
    let relative_x = world_x - camera2d_state.pan_offset.x;
    let screen_relative = relative_x * camera2d_state.zoom;
    viewport.position[0] + RULER_SIZE + viewport.size[0] / 2.0 + screen_relative
}

/// Convert world Y coordinate to screen Y coordinate
fn world_to_screen_y(world_y: f32, camera2d_state: &Camera2DState, viewport: &ViewportState) -> f32 {
    let relative_y = world_y - camera2d_state.pan_offset.y;
    let screen_relative = -relative_y * camera2d_state.zoom; // Y is inverted in screen coords
    viewport.position[1] + VIEWPORT_TABS_HEIGHT + RULER_SIZE + viewport.size[1] / 2.0 + screen_relative
}

/// Render viewport content (for use in docking)
pub fn render_viewport_content(
    ui: &mut egui::Ui,
    viewport: &mut ViewportState,
    assets: &mut AssetBrowserState,
    orbit: &OrbitCameraState,
    viewport_texture_id: Option<TextureId>,
    content_rect: Rect,
) {
    let ctx = ui.ctx().clone();

    // Update viewport state with the actual content area
    viewport.position = [content_rect.min.x, content_rect.min.y];
    viewport.size = [content_rect.width(), content_rect.height()];
    viewport.hovered = ui.rect_contains_pointer(content_rect);

    // Display the viewport texture if available
    if let Some(texture_id) = viewport_texture_id {
        // Allocate the space
        let (_rect, _response) = ui.allocate_exact_size(
            Vec2::new(content_rect.width(), content_rect.height()),
            egui::Sense::hover(),
        );

        // Draw the image
        let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
        ui.painter().image(texture_id, content_rect, uv, Color32::WHITE);
    } else {
        // No texture yet - draw placeholder
        ui.painter().rect_filled(content_rect, 0.0, Color32::from_rgb(30, 30, 35));

        // Draw "No Viewport" text centered
        ui.painter().text(
            content_rect.center(),
            egui::Align2::CENTER_CENTER,
            "Viewport Loading...",
            egui::FontId::proportional(14.0),
            Color32::from_rgb(100, 100, 110),
        );
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
