//! 2D viewport grid — a faint line mesh spaced at the editor's configured
//! tile size, drawn *behind* the sprites — plus the amber camera boundary.
//!
//! The grid used to be drawn with Bevy gizmos, on the assumption that gizmos
//! render into the offscreen image under the sprites. They don't: 2D gizmo
//! phase items are queued with `sort_key: FloatOrd(f32::INFINITY)`
//! (bevy_gizmos_render `pipeline_2d.rs`), so they sort last in `Transparent2d`
//! and paint over every sprite no matter what. The grid is therefore a real
//! `Mesh2d` — a line-list with vertex colors under the shared white
//! `ColorMaterial` — parked at [`GRID_Z`], far below the z range scene sprites
//! occupy, so the normal transparent-phase z sort puts it behind them.
//!
//! Spacing comes from `ViewportSettings.snap.translate_snap` (the toolbar's
//! "Grid Snap" pill) so the grid you see matches the snap step. Visibility is
//! the 2D-only `ViewportSettings.show_grid_2d` toolbar switch — off by
//! default, and deliberately independent of the 3D `show_grid` so hiding one
//! view's grid doesn't blank the other's. The camera boundary stays a gizmo
//! (drawing over sprites is *desired* for a frame marker) and shows whenever
//! the 2D view is active, grid on or off. Both are edit-mode only.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Mesh, PrimitiveTopology};
use bevy::prelude::*;
use bevy::sprite_render::{AlphaMode2d, ColorMaterial};

use renzora::core::viewport_types::{ViewportSettings, ViewportView};
use renzora::core::PlayModeState;

/// Grid depth: far behind the z=0 plane sprites spawn on, but inside the 2D
/// camera's default clip range (orthographic near is -1000).
const GRID_Z: f32 = -900.0;

/// Marker for the singleton grid mesh entity (editor-owned; `HideInHierarchy`
/// keeps it out of the hierarchy panel, scene saves, and scene clears).
#[derive(Component)]
pub struct Grid2dMesh;

/// One resolved grid pass: line spacing, snapped centre, cell counts, alpha.
/// The full frame state is two of these + the line colour — cheap to compare,
/// so the mesh is only rebuilt on pan/zoom/setting changes, not every frame.
#[derive(Clone, Copy, PartialEq, Default)]
pub(crate) struct GridPass {
    center: Vec2,
    spacing: f32,
    cells: UVec2,
    alpha: u8,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub(crate) struct GridKey {
    minor: GridPass,
    major: GridPass,
    color: [u8; 3],
}

/// Bevy system: maintain the 2D editor grid mesh + draw the camera boundary.
///
/// Registered WITHOUT an `in_two_view` run condition on purpose: it must keep
/// running after the user leaves the 2D view (or enters play mode) to hide the
/// grid entity — a gated system would simply stop and leave the mesh visible.
#[allow(clippy::too_many_arguments)]
pub(crate) fn update_grid_2d(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut gizmos: Gizmos,
    settings: Option<Res<ViewportSettings>>,
    play_mode: Option<Res<PlayModeState>>,
    project: Option<Res<renzora::core::CurrentProject>>,
    cameras_2d: Query<(&Camera, &GlobalTransform), With<renzora::core::EditorCamera2d>>,
    mut grid: Query<(&Mesh2d, &mut Visibility), With<Grid2dMesh>>,
    mut cache: Local<Option<GridKey>>,
) {
    let hide = |grid: &mut Query<(&Mesh2d, &mut Visibility), With<Grid2dMesh>>| {
        if let Ok((_, mut vis)) = grid.single_mut() {
            *vis = Visibility::Hidden;
        }
    };

    let Some(settings) = settings else { return };
    let in_play = play_mode.is_some_and(|pm| pm.is_in_play_mode());
    if settings.viewport_view != ViewportView::Two || in_play {
        hide(&mut grid);
        return;
    }

    // Camera / project boundary — the game window area at world (0,0)..(W,-H)
    // (Godot top-left convention), drawn as a bright amber gizmo frame so the
    // user can see the game screen edges. Independent of the grid switch.
    if let Some(project) = project {
        let w = project.config.viewport.width.max(1) as f32;
        let h = project.config.viewport.height.max(1) as f32;
        gizmos.rect_2d(
            Isometry2d::from_translation(Vec2::new(w * 0.5, -h * 0.5)),
            Vec2::new(w, h),
            Color::srgba(1.0, 0.78, 0.25, 0.85),
        );
    }

    let tile = settings.snap.translate_snap;
    if !settings.show_grid_2d || tile <= 0.0 {
        hide(&mut grid);
        return;
    }

    let Ok((camera, cam_gt)) = cameras_2d.single() else {
        hide(&mut grid);
        return;
    };

    // Cover the VISIBLE world rect, centred on the view. The camera's
    // translation is the *top-left corner* of the view (viewport_origin is
    // top-left), so centring the grid there would push three quarters of it
    // off-screen. Derive the visible rect from the camera's own projection.
    let Some(size) = camera.logical_target_size() else {
        hide(&mut grid);
        return;
    };
    let (Ok(a), Ok(b)) = (
        camera.viewport_to_world_2d(cam_gt, Vec2::ZERO),
        camera.viewport_to_world_2d(cam_gt, size),
    ) else {
        hide(&mut grid);
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
        hide(&mut grid);
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

    // Each grid must be centred on a multiple of ITS OWN spacing. Lines sit at
    // `centre + n*spacing`, so a centre that snaps in steps smaller than the
    // spacing makes the lines shift as the camera pans — that was the "section
    // divider jumping". Snap each centre to its own span.
    let snap = |v: f32, step: f32| (v / step).round() * step;
    // Enough cells to cover the visible extent + a margin, capped so an extreme
    // zoom-out can't ask for a runaway line count.
    let cells_for = |span: f32| -> UVec2 {
        let cx = ((extent.x / span).ceil() as u32 + 2).clamp(1, 1024);
        let cy = ((extent.y / span).ceil() as u32 + 2).clamp(1, 1024);
        UVec2::new(cx, cy)
    };

    // Fade each grid out as its cells shrink on screen, so a zoomed-out view
    // doesn't collapse into a solid gray wash (Blender/Godot-style):
    // invisible below ~6px cells, full by ~18px.
    let fade = |cell_world: f32| ((cell_world * px_per_world - 6.0) / 12.0).clamp(0.0, 1.0);

    let [r, g, b, a_base] = settings.grid_color_2d;
    let minor_alpha = (a_base as f32 * fade(tile)) as u8;
    // Section lines are a touch brighter (2×, kept subtle — the grid should
    // whisper behind the art) and, being 8× coarser, stay visible longer.
    let major_alpha = ((a_base as u16 * 2).min(255) as f32 * fade(major_span)) as u8;

    let key = GridKey {
        minor: GridPass {
            center: Vec2::new(snap(center.x, tile), snap(center.y, tile)),
            spacing: tile,
            cells: cells_for(tile),
            alpha: if settings.show_subgrid { minor_alpha } else { 0 },
        },
        major: GridPass {
            center: Vec2::new(snap(center.x, major_span), snap(center.y, major_span)),
            spacing: major_span,
            cells: cells_for(major_span),
            alpha: major_alpha,
        },
        color: [r, g, b],
    };

    if key.minor.alpha == 0 && key.major.alpha == 0 {
        hide(&mut grid);
        return;
    }

    if let Ok((mesh2d, mut vis)) = grid.single_mut() {
        *vis = Visibility::Visible;
        if *cache != Some(key) {
            // Insert-by-handle only fails if the handle's generation died —
            // it can't here (the entity holds a strong handle). If it ever
            // does, the grid just keeps last frame's lines.
            let _ = meshes.insert(&mesh2d.0, build_grid_mesh(&key));
            *cache = Some(key);
        }
    } else {
        let mesh = meshes.add(build_grid_mesh(&key));
        *cache = Some(key);
        commands.spawn((
            Grid2dMesh,
            Mesh2d(mesh),
            // Own material, NOT `Handle::<ColorMaterial>::default()` — Bevy
            // initializes the default 2D material magenta. White + Blend lets
            // the per-vertex colors (and their faint alpha) read through.
            MeshMaterial2d(materials.add(ColorMaterial {
                color: Color::WHITE,
                alpha_mode: AlphaMode2d::Blend,
                ..Default::default()
            })),
            Transform::from_xyz(0.0, 0.0, GRID_Z),
            Name::new("2D Editor Grid"),
            // Editor chrome: out of the hierarchy panel, scene saves, and
            // scene-clear despawns.
            renzora::HideInHierarchy,
        ));
    }
}

/// Build the grid line-list: both passes in one mesh, per-vertex colors
/// carrying each pass's alpha. Vertices are in world space (the entity sits at
/// the origin, only offset in z), matching how the view-rect maths above
/// derived them.
fn build_grid_mesh(key: &GridKey) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();

    for pass in [&key.minor, &key.major] {
        if pass.alpha == 0 {
            continue;
        }
        let color = Color::srgba_u8(key.color[0], key.color[1], key.color[2], pass.alpha)
            .to_linear()
            .to_f32_array();
        let half = Vec2::new(
            pass.cells.x as f32 * pass.spacing * 0.5,
            pass.cells.y as f32 * pass.spacing * 0.5,
        );
        for i in 0..=pass.cells.x {
            let x = pass.center.x - half.x + i as f32 * pass.spacing;
            positions.push([x, pass.center.y - half.y, 0.0]);
            positions.push([x, pass.center.y + half.y, 0.0]);
            colors.push(color);
            colors.push(color);
        }
        for j in 0..=pass.cells.y {
            let y = pass.center.y - half.y + j as f32 * pass.spacing;
            positions.push([pass.center.x - half.x, y, 0.0]);
            positions.push([pass.center.x + half.x, y, 0.0]);
            colors.push(color);
            colors.push(color);
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh
}
