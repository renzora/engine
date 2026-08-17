//! The Region tool — grow or shrink a terrain by clicking tiles in the scene.
//!
//! Sizing a terrain used to mean typing two numbers and hoping. You couldn't see
//! which way it would grow, and because the chunk grid is centred on its parent,
//! it grew *both* ways at once: bump `chunks_x` and everything you'd already
//! sculpted slid half a chunk sideways.
//!
//! This is the direct version, borrowed from how Terrain3D allocates space. Ghost
//! tiles ring the terrain; click one and the terrain grows to meet it. Ctrl+click
//! an edge tile and that row goes. The arithmetic that keeps the existing terrain
//! nailed in place while the grid re-centres lives in
//! [`renzora_terrain::grid`], which is where it's tested.
//!
//! The grid stays a **dense rectangle**. Terrain3D's regions are sparse and
//! non-contiguous, which is a nicer authoring model, but it is a rewrite of
//! `TerrainData`, the chunk addressing and the scene format — not something to
//! smuggle in behind a tool.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use renzora::core::viewport_types::ViewportState;
use renzora::core::EditorCamera;
use renzora_editor_framework::{ActiveTool, EditorCommands, EditorSelection};
use renzora_terrain::data::TerrainData;
use renzora_terrain::grid::{apply_grid_resize, plan_grid_resize, GridEdge};

use crate::systems::viewport_cursor_ray;

/// How high above the terrain plane the ghosts are drawn, in world units. Enough
/// to clear a flat terrain's surface without floating free of it.
const GHOST_LIFT: f32 = 0.05;

/// What the cursor is currently over, recomputed each frame by
/// [`region_hover_system`] and consumed by the gizmo and the click handler.
#[derive(Resource, Default)]
pub struct RegionHover {
    /// The terrain being edited.
    pub terrain: Option<Entity>,
    /// The ghost tile under the cursor, if any: which edge it extends, and
    /// whether it's an *add* ghost (outside the grid) or an existing edge tile
    /// armed for removal (Ctrl held).
    pub target: Option<(GridEdge, bool)>,
}

/// Which terrain the tool acts on: the selected one, else the first in the scene
/// — matching how the toolbar buttons pick a terrain.
fn active_terrain(
    selection: &EditorSelection,
    terrains: &Query<(Entity, &TerrainData, &GlobalTransform)>,
) -> Option<Entity> {
    if let Some(sel) = selection.get() {
        if terrains.get(sel).is_ok() {
            return Some(sel);
        }
    }
    terrains.iter().next().map(|(e, ..)| e)
}

/// Intersect the cursor ray with the terrain's ground plane and work out which
/// ghost tile — if any — it lands on.
pub fn region_hover_system(
    viewport: Res<ViewportState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    terrains: Query<(Entity, &TerrainData, &GlobalTransform)>,
    selection: Res<EditorSelection>,
    keys: Res<ButtonInput<KeyCode>>,
    mut hover: ResMut<RegionHover>,
) {
    hover.target = None;
    hover.terrain = active_terrain(&selection, &terrains);

    let Some(terrain) = hover.terrain else {
        return;
    };
    let Ok((_, data, xform)) = terrains.get(terrain) else {
        return;
    };
    let Some(ray) = viewport_cursor_ray(&viewport, &window_query, &camera_query) else {
        return;
    };

    let Some(local) = ray_to_terrain_plane(ray, xform) else {
        return;
    };

    // Grid space, in chunks, with the origin at the grid's min corner. Values
    // outside `0..chunks_*` are the ghost ring.
    let gx = ((local.x + data.total_width() * 0.5) / data.chunk_size).floor() as i32;
    let gz = ((local.z + data.total_depth() * 0.5) / data.chunk_size).floor() as i32;
    let (nx, nz) = (data.chunks_x as i32, data.chunks_z as i32);

    let removing = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);

    let edge = if removing {
        // Removal targets a tile that's actually there, on whichever edge it
        // sits. Corner tiles belong to two edges; X wins, arbitrarily but
        // consistently, so a corner click never does nothing.
        if gx < 0 || gz < 0 || gx >= nx || gz >= nz {
            None
        } else if gx == 0 {
            Some(GridEdge::MinX)
        } else if gx == nx - 1 {
            Some(GridEdge::MaxX)
        } else if gz == 0 {
            Some(GridEdge::MinZ)
        } else if gz == nz - 1 {
            Some(GridEdge::MaxZ)
        } else {
            None // interior tile: nothing to remove from here
        }
    } else {
        // Adding targets the ring one tile outside the grid. Diagonal ghosts
        // (outside on both axes) are excluded — "add a corner" has no meaning
        // for a rectangle, and offering it would grow two rows per click.
        let out_x = if gx == -1 {
            Some(GridEdge::MinX)
        } else if gx == nx {
            Some(GridEdge::MaxX)
        } else {
            None
        };
        let out_z = if gz == -1 {
            Some(GridEdge::MinZ)
        } else if gz == nz {
            Some(GridEdge::MaxZ)
        } else {
            None
        };
        let inside_x = (0..nx).contains(&gx);
        let inside_z = (0..nz).contains(&gz);
        match (out_x, out_z) {
            (Some(e), None) if inside_z => Some(e),
            (None, Some(e)) if inside_x => Some(e),
            _ => None,
        }
    };

    // Refuse to highlight a move that would be rejected anyway — a ghost you can
    // click but that does nothing is worse than no ghost.
    hover.target = edge
        .filter(|e| plan_grid_resize(data, *e, if removing { -1 } else { 1 }).is_some())
        .map(|e| (e, !removing));
}

/// Where the cursor ray meets the terrain's ground plane, in terrain-local space.
fn ray_to_terrain_plane(ray: Ray3d, xform: &GlobalTransform) -> Option<Vec3> {
    // The plane is the terrain's own XZ plane, so it tilts with the terrain
    // rather than being a fixed world-Y plane.
    let origin = xform.translation();
    let normal = xform.rotation() * Vec3::Y;
    let denom = ray.direction.dot(normal);
    if denom.abs() < 1e-6 {
        return None; // ray parallel to the plane
    }
    let t = (origin - ray.origin).dot(normal) / denom;
    if t < 0.0 {
        return None; // plane is behind the camera
    }
    let world = ray.origin + *ray.direction * t;
    Some(xform.affine().inverse().transform_point3(world))
}

/// Draw the grid outline plus the ghost ring.
pub fn region_gizmo_system(
    mut gizmos: Gizmos,
    terrains: Query<(Entity, &TerrainData, &GlobalTransform)>,
    hover: Res<RegionHover>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    let Some(terrain) = hover.terrain else {
        return;
    };
    let Ok((_, data, xform)) = terrains.get(terrain) else {
        return;
    };
    let removing = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);

    let existing = Color::srgba(1.0, 1.0, 1.0, 0.25);
    let ghost = Color::srgba(0.45, 0.75, 1.0, 0.45);
    let ghost_hot = Color::srgba(0.45, 0.85, 1.0, 1.0);
    let remove_hot = Color::srgba(1.0, 0.4, 0.35, 1.0);

    // The grid as it stands.
    for cz in 0..data.chunks_z {
        for cx in 0..data.chunks_x {
            draw_tile(&mut gizmos, data, xform, cx as i32, cz as i32, existing);
        }
    }

    if removing {
        // Highlight the row that would go, so it's clear a click removes more
        // than the single tile under the cursor.
        if let Some((edge, _)) = hover.target {
            for (gx, gz) in edge_tiles(data, edge) {
                draw_tile(&mut gizmos, data, xform, gx, gz, remove_hot);
            }
        }
        return;
    }

    // The ring of ghosts, each with a `+` through it.
    for (edge, gx, gz) in ghost_tiles(data) {
        let hot = hover.target == Some((edge, true));
        let color = if hot { ghost_hot } else { ghost };
        draw_tile(&mut gizmos, data, xform, gx, gz, color);
        draw_plus(&mut gizmos, data, xform, gx, gz, color);
    }
}

/// Every ghost position around the grid, one per edge cell.
fn ghost_tiles(data: &TerrainData) -> Vec<(GridEdge, i32, i32)> {
    let (nx, nz) = (data.chunks_x as i32, data.chunks_z as i32);
    let mut out = Vec::with_capacity((nx as usize + nz as usize) * 2);
    for gz in 0..nz {
        out.push((GridEdge::MinX, -1, gz));
        out.push((GridEdge::MaxX, nx, gz));
    }
    for gx in 0..nx {
        out.push((GridEdge::MinZ, gx, -1));
        out.push((GridEdge::MaxZ, gx, nz));
    }
    out
}

/// The existing tiles making up one edge of the grid.
fn edge_tiles(data: &TerrainData, edge: GridEdge) -> Vec<(i32, i32)> {
    let (nx, nz) = (data.chunks_x as i32, data.chunks_z as i32);
    match edge {
        GridEdge::MinX => (0..nz).map(|gz| (0, gz)).collect(),
        GridEdge::MaxX => (0..nz).map(|gz| (nx - 1, gz)).collect(),
        GridEdge::MinZ => (0..nx).map(|gx| (gx, 0)).collect(),
        GridEdge::MaxZ => (0..nx).map(|gx| (gx, nz - 1)).collect(),
    }
}

/// Grid cell → its four world-space corners, following the terrain's transform.
fn tile_corners(data: &TerrainData, xform: &GlobalTransform, gx: i32, gz: i32) -> [Vec3; 4] {
    let s = data.chunk_size;
    let x0 = gx as f32 * s - data.total_width() * 0.5;
    let z0 = gz as f32 * s - data.total_depth() * 0.5;
    let p = |x: f32, z: f32| xform.transform_point(Vec3::new(x, GHOST_LIFT, z));
    [
        p(x0, z0),
        p(x0 + s, z0),
        p(x0 + s, z0 + s),
        p(x0, z0 + s),
    ]
}

fn draw_tile(
    gizmos: &mut Gizmos,
    data: &TerrainData,
    xform: &GlobalTransform,
    gx: i32,
    gz: i32,
    color: Color,
) {
    let c = tile_corners(data, xform, gx, gz);
    for i in 0..4 {
        gizmos.line(c[i], c[(i + 1) % 4], color);
    }
}

/// A `+` inscribed in the tile — the thing that makes a ghost read as "click to
/// add" rather than as a stray outline.
fn draw_plus(
    gizmos: &mut Gizmos,
    data: &TerrainData,
    xform: &GlobalTransform,
    gx: i32,
    gz: i32,
    color: Color,
) {
    let c = tile_corners(data, xform, gx, gz);
    let center = (c[0] + c[2]) * 0.5;
    // A quarter of the tile each way, so the mark stays clear of the outline.
    let across = (c[1] - c[0]) * 0.25;
    let down = (c[3] - c[0]) * 0.25;
    gizmos.line(center - across, center + across, color);
    gizmos.line(center - down, center + down, color);
}

/// Click a ghost to grow, Ctrl+click an edge tile to shrink.
pub fn region_click_system(
    mouse: Res<ButtonInput<MouseButton>>,
    hover: Res<RegionHover>,
    cmds: Option<Res<EditorCommands>>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let (Some(cmds), Some(terrain), Some((edge, adding))) =
        (cmds, hover.terrain, hover.target)
    else {
        return;
    };
    let delta = if adding { 1 } else { -1 };
    // Deferred: resizing despawns and respawns every chunk, which is not
    // something to do from inside a system that's holding queries over them.
    cmds.push(move |w: &mut World| {
        apply_grid_resize(w, terrain, edge, delta);
    });
}

/// True while the Region tool is the active one.
pub fn region_tool_active(tool: Option<Res<ActiveTool>>) -> bool {
    tool.is_some_and(|t| *t == ActiveTool::TerrainRegion)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(chunks_x: u32, chunks_z: u32) -> TerrainData {
        TerrainData {
            chunks_x,
            chunks_z,
            chunk_size: 64.0,
            ..TerrainData::default()
        }
    }

    #[test]
    fn ghost_ring_has_one_tile_per_edge_cell() {
        let d = data(3, 2);
        let ghosts = ghost_tiles(&d);
        // Two columns of 2 + two rows of 3.
        assert_eq!(ghosts.len(), 2 * 2 + 2 * 3);
    }

    /// The ring must sit strictly outside the grid — a ghost overlapping a real
    /// chunk would be un-clickable, hidden behind the terrain it's meant to
    /// extend.
    #[test]
    fn ghosts_never_overlap_existing_chunks() {
        let d = data(4, 3);
        for (_, gx, gz) in ghost_tiles(&d) {
            let inside =
                (0..d.chunks_x as i32).contains(&gx) && (0..d.chunks_z as i32).contains(&gz);
            assert!(!inside, "ghost ({gx},{gz}) lands on an existing chunk");
        }
    }

    /// Each ghost must be adjacent to the grid on exactly one axis — diagonal
    /// corner ghosts are deliberately excluded, since growing "diagonally" would
    /// mean two rows per click.
    #[test]
    fn ghost_ring_excludes_diagonals() {
        let d = data(4, 3);
        for (_, gx, gz) in ghost_tiles(&d) {
            let inside_x = (0..d.chunks_x as i32).contains(&gx);
            let inside_z = (0..d.chunks_z as i32).contains(&gz);
            assert!(
                inside_x != inside_z,
                "ghost ({gx},{gz}) is diagonal to the grid"
            );
        }
    }

    #[test]
    fn edge_tiles_run_the_full_length_of_their_side() {
        let d = data(4, 3);
        assert_eq!(edge_tiles(&d, GridEdge::MinX).len(), 3);
        assert_eq!(edge_tiles(&d, GridEdge::MaxX).len(), 3);
        assert_eq!(edge_tiles(&d, GridEdge::MinZ).len(), 4);
        assert_eq!(edge_tiles(&d, GridEdge::MaxZ).len(), 4);
        // …and index the correct row.
        assert!(edge_tiles(&d, GridEdge::MaxX).iter().all(|(gx, _)| *gx == 3));
        assert!(edge_tiles(&d, GridEdge::MaxZ).iter().all(|(_, gz)| *gz == 2));
    }

    /// Tile corners follow the terrain transform, so a moved or rotated terrain
    /// draws its ghosts in the right place rather than around the world origin.
    #[test]
    fn tile_corners_follow_the_terrain_transform() {
        let d = data(2, 2);
        let moved = GlobalTransform::from(Transform::from_xyz(100.0, 0.0, -50.0));
        let at_origin = tile_corners(&d, &GlobalTransform::IDENTITY, 0, 0);
        let shifted = tile_corners(&d, &moved, 0, 0);
        for (a, b) in at_origin.iter().zip(shifted.iter()) {
            assert!((*b - *a - Vec3::new(100.0, 0.0, -50.0)).length() < 1e-3);
        }
    }
}
