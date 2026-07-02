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
    project: Option<Res<renzora::core::CurrentProject>>,
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

    let Ok((camera, cam_gt)) = cameras_2d.single() else {
        return;
    };

    // Cover the VISIBLE world rect, centred on the view. The camera's
    // translation is the *top-left corner* of the view (viewport_origin is
    // top-left), so centring the grid there would push three quarters of it
    // off-screen — which is why the grid only showed in the corner. Derive the
    // visible rect from the camera's own projection instead.
    let Some(size) = camera.logical_target_size() else {
        return;
    };
    let (Ok(a), Ok(b)) = (
        camera.viewport_to_world_2d(cam_gt, Vec2::ZERO),
        camera.viewport_to_world_2d(cam_gt, size),
    ) else {
        return;
    };
    let view_min = a.min(b);
    let view_max = a.max(b);
    let center = (view_min + view_max) * 0.5;
    let extent = view_max - view_min;

    // How many render-image pixels one world unit covers at the current zoom.
    let px_per_world = size.x / extent.x.max(1e-6);

    // Adaptive spacing: the raw snap step (typically 1 world unit = 1 pixel in
    // 2D) is sub-pixel at any zoom that shows the whole camera boundary, so a
    // fixed-step grid only ever appeared when zoomed far in. Scale the DRAWN
    // step up in powers of two until a cell spans a readable number of pixels —
    // snapping still uses the raw `translate_snap`, and every adaptive line
    // remains a multiple of it, so what you see stays snappable.
    const MIN_CELL_PX: f32 = 12.0;
    let base_px = tile * px_per_world;
    if !base_px.is_finite() || base_px <= 0.0 {
        return;
    }
    let level = if base_px >= MIN_CELL_PX {
        0
    } else {
        ((MIN_CELL_PX / base_px).log2().ceil() as i32).clamp(0, 32)
    };
    let minor_span = tile * 2f32.powi(level);

    let major_step = if settings.show_subgrid { 8 } else { 1 };
    let major_span = minor_span * major_step as f32;

    // Each grid must be centred on a multiple of ITS OWN spacing. `grid_2d`
    // draws lines at `centre + n*spacing`, so a centre that snaps in steps
    // smaller than the spacing makes the lines shift as the camera pans — that
    // was the "section divider jumping". Snap each centre to its own span.
    let snap = |v: f32, step: f32| (v / step).round() * step;
    // Enough cells to cover the visible extent + a margin, capped so an extreme
    // zoom-out can't ask for a runaway line count.
    let cells_for = |span: f32| -> UVec2 {
        let cx = ((extent.x / span).ceil() as u32 + 2).clamp(1, 1024);
        let cy = ((extent.y / span).ceil() as u32 + 2).clamp(1, 1024);
        UVec2::new(cx, cy)
    };

    // Fade each grid out as its cells shrink on screen, so a zoomed-out view
    // doesn't collapse into a solid gray wash (Blender/Godot-style). `size` is
    // the render image in px, `extent` the world width it shows.
    let px_per_world = size.x / extent.x.max(1e-6);
    // Smoothstep-ish ramp: invisible below ~6px cells, full by ~18px.
    let fade = |cell_world: f32| ((cell_world * px_per_world - 6.0) / 12.0).clamp(0.0, 1.0);

    let [r, g, b, a_base] = settings.grid_color_2d;
    let minor_alpha = (a_base as f32 * fade(tile)) as u8;
    // Section lines are brighter and, being 8× coarser, stay visible longer.
    let major_alpha = ((a_base as u16 * 3).min(255) as f32 * fade(major_span)) as u8;

    if settings.show_subgrid && minor_alpha > 0 {
        let c = Vec2::new(snap(center.x, tile), snap(center.y, tile));
        gizmos.grid_2d(
            Isometry2d::from_translation(c),
            cells_for(tile),
            Vec2::splat(tile),
            Color::srgba_u8(r, g, b, minor_alpha),
        );
    }
    if major_alpha > 0 {
        let cm = Vec2::new(snap(center.x, major_span), snap(center.y, major_span));
        gizmos.grid_2d(
            Isometry2d::from_translation(cm),
            cells_for(major_span),
            Vec2::splat(major_span),
            Color::srgba_u8(r, g, b, major_alpha),
        );
    }

    // Camera / project boundary — the game window area at world (0,0)..(W,-H)
    // (Godot top-left convention), drawn as a bright amber frame so the user can
    // see where the game screen edges are.
    if let Some(project) = project {
        let w = project.config.viewport.width.max(1) as f32;
        let h = project.config.viewport.height.max(1) as f32;
        gizmos.rect_2d(
            Isometry2d::from_translation(Vec2::new(w * 0.5, -h * 0.5)),
            Vec2::new(w, h),
            Color::srgba(1.0, 0.78, 0.25, 0.85),
        );
    }
}
