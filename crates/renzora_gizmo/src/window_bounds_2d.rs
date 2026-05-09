//! 2D viewport "window area" outline — shows the project's exported
//! window dimensions as a rectangle anchored top-left at world origin
//! (0, 0), Godot-style. Helps users author UI/HUDs and lay out scenes
//! against the surface the player will actually see.

use bevy::prelude::*;
use bevy_egui::egui;

use renzora::core::viewport_types::{ViewportSettings, ViewportState, ViewportView};
use renzora::core::CurrentProject;

fn find_editor_camera_2d<'a>(world: &'a World) -> Option<(&'a Camera, &'a GlobalTransform)> {
    let entity = world
        .get_resource::<crate::light_gizmo::SceneIconCache>()?
        .editor_camera_2d?;
    let camera = world.get::<Camera>(entity)?;
    let cam_gt = world.get::<GlobalTransform>(entity)?;
    Some((camera, cam_gt))
}

/// Egui overlay drawer. Registered at order ~95 — over the ruler but
/// under selection outlines and gizmos.
pub fn draw_window_bounds_2d(ui: &mut egui::Ui, world: &World, rect: egui::Rect) {
    let view = world
        .get_resource::<ViewportSettings>()
        .map(|s| s.viewport_view)
        .unwrap_or_default();
    if view != ViewportView::Two {
        return;
    }

    // Project must be loaded. The outline shows the **game render
    // region** — what the user's Camera 2D will actually shoot, not
    // the OS window. With stretch mode `Disabled` those are the same.
    // With stretch mode `Viewport` the camera renders to a smaller
    // offscreen image at viewport dimensions and the runtime upscales
    // it to the OS window, so authoring must happen against the
    // viewport size.
    let Some(project) = world.get_resource::<CurrentProject>() else {
        return;
    };
    let (w, h) = match project.config.viewport.stretch_mode {
        renzora::core::StretchMode::Viewport => (
            project.config.viewport.width as f32,
            project.config.viewport.height as f32,
        ),
        renzora::core::StretchMode::Disabled => (
            project.config.window.width as f32,
            project.config.window.height as f32,
        ),
    };
    if w <= 0.0 || h <= 0.0 {
        return;
    }

    let Some(viewport) = world.get_resource::<ViewportState>() else {
        return;
    };
    let Some((camera, cam_gt)) = find_editor_camera_2d(world) else {
        return;
    };
    let image_size = viewport.current_size.as_vec2();
    if image_size.x <= 0.0 || image_size.y <= 0.0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }

    let to_screen = |w_pos: Vec3| -> Option<egui::Pos2> {
        let img = camera.world_to_viewport(cam_gt, w_pos).ok()?;
        let panel_x = img.x * rect.width() / image_size.x;
        let panel_y = img.y * rect.height() / image_size.y;
        Some(egui::Pos2::new(rect.min.x + panel_x, rect.min.y + panel_y))
    };

    // Godot-style: window top-left is world origin (0, 0) so ruler 0
    // lines coincide with the window's left and top edges. World Y is
    // up in Bevy, so the window extends in +X and -Y from origin —
    // top-left = (0, 0), bottom-right = (width, -height).
    let tl = to_screen(Vec3::new(0.0, 0.0, 0.0));
    let tr = to_screen(Vec3::new(w, 0.0, 0.0));
    let bl = to_screen(Vec3::new(0.0, -h, 0.0));
    let br = to_screen(Vec3::new(w, -h, 0.0));
    let (Some(tl), Some(tr), Some(bl), Some(br)) = (tl, tr, bl, br) else {
        return;
    };

    let painter = ui.painter_at(rect);

    // Axis lines spanning the full panel — Godot-style. Standard colour
    // convention: red = X axis (horizontal line at Y=0), green = Y axis
    // (vertical line at X=0).
    let x_axis_color = egui::Color32::from_rgba_unmultiplied(220, 80, 90, 110);
    let y_axis_color = egui::Color32::from_rgba_unmultiplied(110, 200, 110, 110);
    if let Some(origin) = to_screen(Vec3::new(0.0, 0.0, 0.0)) {
        if origin.x >= rect.min.x && origin.x <= rect.max.x {
            painter.line_segment(
                [
                    egui::Pos2::new(origin.x, rect.min.y),
                    egui::Pos2::new(origin.x, rect.max.y),
                ],
                egui::Stroke::new(1.0, y_axis_color),
            );
        }
        if origin.y >= rect.min.y && origin.y <= rect.max.y {
            painter.line_segment(
                [
                    egui::Pos2::new(rect.min.x, origin.y),
                    egui::Pos2::new(rect.max.x, origin.y),
                ],
                egui::Stroke::new(1.0, x_axis_color),
            );
        }
    }

    // Soft fill outside the window rect to dim "off-screen" content.
    // Just an outline if the bounds extend off the panel — egui's clip
    // takes care of the visible portion.
    let bounds = egui::Rect::from_two_pos(tl.min(bl), tr.max(br));
    let outline_color = egui::Color32::from_rgb(140, 170, 230);
    painter.rect_stroke(
        bounds,
        0.0,
        egui::Stroke::new(1.5, outline_color),
        egui::StrokeKind::Outside,
    );

    // Label in the top-left corner of the window rect, just inside, so
    // the user knows what the box represents at a glance.
    // Label matches the dimensions the rect was drawn with — viewport
    // size in stretch mode, window size otherwise.
    let label = format!("{}×{}", w as u32, h as u32);
    let label_pos = egui::Pos2::new(tl.x + 4.0, tl.y + 2.0);
    painter.text(
        label_pos + egui::vec2(1.0, 1.0),
        egui::Align2::LEFT_TOP,
        &label,
        egui::FontId::monospace(10.0),
        egui::Color32::from_black_alpha(160),
    );
    painter.text(
        label_pos,
        egui::Align2::LEFT_TOP,
        &label,
        egui::FontId::monospace(10.0),
        outline_color,
    );

    // Origin marker — small crosshair at world (0, 0).
    if let Some(origin) = to_screen(Vec3::ZERO) {
        let cross = 5.0_f32;
        let origin_color = egui::Color32::from_rgb(255, 200, 80);
        painter.line_segment(
            [
                egui::Pos2::new(origin.x - cross, origin.y),
                egui::Pos2::new(origin.x + cross, origin.y),
            ],
            egui::Stroke::new(1.0, origin_color),
        );
        painter.line_segment(
            [
                egui::Pos2::new(origin.x, origin.y - cross),
                egui::Pos2::new(origin.x, origin.y + cross),
            ],
            egui::Stroke::new(1.0, origin_color),
        );
    }
}
