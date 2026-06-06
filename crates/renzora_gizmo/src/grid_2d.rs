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
//! painted on top — sprites visually sit on top of the grid.
//! Gated on play-mode so the grid never appears in the runtime view.

use bevy::prelude::*;

use renzora::core::viewport_types::{ViewportSettings, ViewportView};
use renzora::core::PlayModeState;

/// Bevy system: draw the 2D editor grid via gizmos so it renders into
/// the offscreen image *under* sprites (instead of on top). Used in 2D
/// view + edit mode only — never runs in play mode, so it can't bleed
/// into the runtime view.
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
