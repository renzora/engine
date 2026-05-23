//! 2D viewport grid — thin world-space lines spaced at the editor's
//! configured tile size. Pairs with `picker_2d`'s drag-snap so the
//! sprites you place visually land on the grid intersections.
//!
//! Spacing comes from `ViewportSettings.snap.translate_snap` — same
//! value as the toolbar's "Grid Snap" pill, so the grid you see
//! matches the snap step. Visibility gated on
//! `ViewportSettings.show_grid`. The grid renders via Bevy gizmos
//! into the editor's 2D camera, which means it goes *into* the
//! offscreen image alongside the rendered sprites instead of being
//! painted on top by egui — sprites visually sit on top of the grid.
//! Gated on play-mode so the grid never appears in the runtime view.

use bevy::prelude::*;
use bevy_egui::egui;

use renzora::core::viewport_types::{ViewportSettings, ViewportState, ViewportView};
use renzora::core::PlayModeState;

fn find_editor_camera_2d(world: &World) -> Option<(&Camera, &GlobalTransform)> {
    let entity = world
        .get_resource::<crate::light_gizmo::SceneIconCache>()?
        .editor_camera_2d?;
    let camera = world.get::<Camera>(entity)?;
    let cam_gt = world.get::<GlobalTransform>(entity)?;
    Some((camera, cam_gt))
}

/// Don't render more than this many grid lines on a single frame —
/// guards against the user typing tile size 0.001 with the camera
/// zoomed out, which would try to paint millions of lines.
const MAX_LINES_PER_AXIS: usize = 600;

pub fn draw_grid_2d(ui: &mut egui::Ui, world: &World, rect: egui::Rect) {
    let Some(settings) = world.get_resource::<ViewportSettings>() else {
        return;
    };
    if settings.viewport_view != ViewportView::Two {
        return;
    }
    if !settings.show_grid {
        return;
    }
    let tile = settings.snap.translate_snap;
    if tile <= 0.0 {
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

    // Helpers between panel pixels and world coords (mirrors ruler_2d).
    let panel_to_image = |p: Vec2| -> Vec2 {
        Vec2::new(
            p.x * image_size.x / rect.width(),
            p.y * image_size.y / rect.height(),
        )
    };
    let panel_to_world = |panel: Vec2| -> Option<Vec2> {
        camera
            .viewport_to_world_2d(cam_gt, panel_to_image(panel))
            .ok()
    };
    let world_x_to_panel_x = |world_x: f32| -> Option<f32> {
        let img = camera
            .world_to_viewport(cam_gt, Vec3::new(world_x, 0.0, 0.0))
            .ok()?;
        Some(img.x * rect.width() / image_size.x + rect.min.x)
    };
    let world_y_to_panel_y = |world_y: f32| -> Option<f32> {
        let img = camera
            .world_to_viewport(cam_gt, Vec3::new(0.0, world_y, 0.0))
            .ok()?;
        Some(img.y * rect.height() / image_size.y + rect.min.y)
    };

    // Visible world AABB at the panel corners.
    let (Some(tl_world), Some(br_world)) = (
        panel_to_world(Vec2::ZERO),
        panel_to_world(Vec2::new(rect.width(), rect.height())),
    ) else {
        return;
    };
    let world_min_x = tl_world.x.min(br_world.x);
    let world_max_x = tl_world.x.max(br_world.x);
    // World Y is up, screen Y is down — `tl_world.y` is the *higher* world Y.
    let world_max_y = tl_world.y.max(br_world.y);
    let world_min_y = tl_world.y.min(br_world.y);

    // Bail out if zoom-out would paint an absurd number of lines.
    let lines_x = ((world_max_x - world_min_x) / tile).ceil() as usize;
    let lines_y = ((world_max_y - world_min_y) / tile).ceil() as usize;
    if lines_x > MAX_LINES_PER_AXIS || lines_y > MAX_LINES_PER_AXIS {
        return;
    }

    let painter = ui.painter_at(rect);
    // Subgrid: faint hairline at every tile.
    // Major grid: stronger line every N tiles for orientation. N scales
    // with how zoomed-in we are so labels/lines stay readable.
    let major_every: i64 = if settings.show_subgrid { 8 } else { 1 };

    // User-configured grid colour. Minor lines render at the exact
    // alpha; major lines bump it ~3× (clamped) so the hierarchy is
    // visible without needing a second colour picker in the UI.
    let [r, g, b, a_minor] = settings.grid_color_2d;
    let a_major = (a_minor as u16 * 3).min(255) as u8;
    let minor_color = egui::Color32::from_rgba_unmultiplied(r, g, b, a_minor);
    let major_color = egui::Color32::from_rgba_unmultiplied(r, g, b, a_major);

    let start_x = (world_min_x / tile).floor() as i64;
    let end_x = (world_max_x / tile).ceil() as i64;
    for i in start_x..=end_x {
        let world_x = i as f32 * tile;
        let Some(panel_x) = world_x_to_panel_x(world_x) else {
            continue;
        };
        if panel_x < rect.min.x || panel_x > rect.max.x {
            continue;
        }
        let is_major = major_every > 1 && i.rem_euclid(major_every) == 0;
        let stroke = if is_major {
            egui::Stroke::new(1.0, major_color)
        } else if settings.show_subgrid {
            egui::Stroke::new(1.0, minor_color)
        } else {
            egui::Stroke::new(1.0, major_color)
        };
        painter.line_segment(
            [
                egui::Pos2::new(panel_x, rect.min.y),
                egui::Pos2::new(panel_x, rect.max.y),
            ],
            stroke,
        );
    }

    let start_y = (world_min_y / tile).floor() as i64;
    let end_y = (world_max_y / tile).ceil() as i64;
    for i in start_y..=end_y {
        let world_y = i as f32 * tile;
        let Some(panel_y) = world_y_to_panel_y(world_y) else {
            continue;
        };
        if panel_y < rect.min.y || panel_y > rect.max.y {
            continue;
        }
        let is_major = major_every > 1 && i.rem_euclid(major_every) == 0;
        let stroke = if is_major {
            egui::Stroke::new(1.0, major_color)
        } else if settings.show_subgrid {
            egui::Stroke::new(1.0, minor_color)
        } else {
            egui::Stroke::new(1.0, major_color)
        };
        painter.line_segment(
            [
                egui::Pos2::new(rect.min.x, panel_y),
                egui::Pos2::new(rect.max.x, panel_y),
            ],
            stroke,
        );
    }
}

/// Bevy system: draw the 2D editor grid via gizmos so it renders into
/// the offscreen image *under* sprites (instead of on top, which is
/// what the egui overlay above does). Used in 2D view + edit mode
/// only — never runs in play mode, so it can't bleed into the
/// runtime view.
///
/// Two passes: minor lines at every tile, major lines at every 8th
/// tile. The grid extends a generous distance around the editor
/// camera so panning / zooming feels infinite without the cost of
/// covering the entire world.
pub fn draw_grid_2d_gizmos(
    mut gizmos: Gizmos,
    settings: Option<Res<ViewportSettings>>,
    play_mode: Option<Res<PlayModeState>>,
    cameras_2d: Query<(&Camera, &GlobalTransform), With<renzora::core::EditorCamera2d>>,
) {
    let Some(settings) = settings else { return };
    if settings.viewport_view != ViewportView::Two || !settings.show_grid {
        return;
    }
    if play_mode.is_some_and(|pm| pm.is_in_play_mode()) {
        return;
    }
    let tile = settings.snap.translate_snap;
    if tile <= 0.0 {
        return;
    }

    // Centre the grid on the camera so panning never runs out of
    // grid. Cap the extent so stupid-low tile sizes don't try to
    // draw a billion lines.
    let Ok((_, cam_gt)) = cameras_2d.single() else {
        return;
    };
    let cam_xy = cam_gt.translation().truncate();
    let centre_tile = Vec2::new(
        (cam_xy.x / tile).round() * tile,
        (cam_xy.y / tile).round() * tile,
    );

    let major_step = if settings.show_subgrid { 8 } else { 1 };
    let target_cells: u32 = 256;
    let cells_minor = UVec2::splat(target_cells);
    let cells_major = UVec2::splat(target_cells / major_step.max(1) as u32);

    let [r, g, b, a_minor] = settings.grid_color_2d;
    let a_major = (a_minor as u16 * 3).min(255) as u8;
    let minor_color = Color::srgba_u8(r, g, b, a_minor);
    let major_color = Color::srgba_u8(r, g, b, a_major);

    let iso = Isometry2d::from_translation(centre_tile);

    if settings.show_subgrid {
        gizmos.grid_2d(iso, cells_minor, Vec2::splat(tile), minor_color);
    }
    gizmos.grid_2d(
        iso,
        cells_major,
        Vec2::splat(tile * major_step as f32),
        major_color,
    );
}
