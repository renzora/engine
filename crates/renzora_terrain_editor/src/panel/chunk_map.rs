//! The Manage mode's chunk map: the terrain's chunk grid seen from above, each
//! cell coloured by whether it is at full detail or streamed out to a
//! stand-in, the camera's cell outlined, and a ring of empty cells around it.
//!
//! It replaces the Resize scene tool and the Edit Terrain dialog's grid
//! picker. Click an empty cell beside the grid to add that row or column;
//! Ctrl+click an edge chunk to remove its row or column. Removal asks for Ctrl
//! because it deletes sculpting and paint and is not an undo step, and a plain
//! click on the map should never be able to do that. The arithmetic that
//! keeps the existing terrain where it is while the grid re-centres is
//! `renzora_terrain::grid`'s, as it was for the scene tool.
//!
//! The map is one image redrawn when [`ChunkMap`] changes; see
//! [`super::canvas`] for why.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora::core::EditorCamera;
use renzora_editor_framework::EditorSelection;
use renzora_ember::theme::*;
use renzora_ember::UiInteraction;

use renzora_terrain::data::{TerrainChunkData, TerrainChunkOf, TerrainChunkStreamedOut, TerrainData};
use renzora_terrain::grid::{apply_grid_resize, plan_grid_resize, GridEdge};

use super::canvas::{rgba8, write_pixels, Pixels};

/// Cell edge and gap in image pixels. The image is scaled to the panel width,
/// so these set its proportions, not its size on screen.
const CELL: i32 = 8;
const GAP: i32 = 1;

/// What one existing chunk is doing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum CellState {
    /// No chunk entity in this cell (it is being rebuilt).
    Missing,
    Full,
    StandIn,
}

/// The selected terrain's chunk grid as the map draws it. Rewritten only when
/// something on it changed, so the reactive readouts bound to it stay quiet.
#[derive(Resource, Default, Clone, PartialEq, Debug)]
pub(crate) struct ChunkMap {
    pub terrain: Option<Entity>,
    pub nx: u32,
    pub nz: u32,
    /// Row-major, `nx * nz`.
    pub cells: Vec<CellState>,
    /// The editor camera's cell, when it is over the grid or its ring.
    pub camera: Option<(i32, i32)>,
    /// The cell under the pointer, in grid coordinates (the ring is -1 and n).
    pub hover: Option<(i32, i32)>,
    pub ctrl: bool,
}

impl ChunkMap {
    pub fn resident(&self) -> usize {
        self.cells.iter().filter(|c| **c == CellState::Full).count()
    }

    pub fn total(&self) -> usize {
        (self.nx * self.nz) as usize
    }
}

/// Marker on the map's image node.
#[derive(Component, Default)]
pub(crate) struct ChunkMapView {
    drawn: Option<(ChunkMap, [u8; 4])>,
}

/// What a click on grid cell (`gx`, `gz`) does to an `nx` x `nz` grid: which
/// edge grows or shrinks, and by how much. `removing` is Ctrl held.
///
/// Adding takes a ring cell beside an edge (never a diagonal corner, which
/// would mean growing two rows at once). Removing takes an existing edge
/// chunk; a corner chunk belongs to two edges and X wins, so a corner click
/// always does something predictable.
pub(crate) fn map_click_action(nx: u32, nz: u32, gx: i32, gz: i32, removing: bool) -> Option<(GridEdge, i32)> {
    let (nx, nz) = (nx as i32, nz as i32);
    let inside_x = (0..nx).contains(&gx);
    let inside_z = (0..nz).contains(&gz);
    if removing {
        if !(inside_x && inside_z) {
            return None;
        }
        let edge = if gx == 0 {
            GridEdge::MinX
        } else if gx == nx - 1 {
            GridEdge::MaxX
        } else if gz == 0 {
            GridEdge::MinZ
        } else if gz == nz - 1 {
            GridEdge::MaxZ
        } else {
            return None;
        };
        return Some((edge, -1));
    }
    let edge = match (gx, gz) {
        (-1, _) if inside_z => GridEdge::MinX,
        (x, _) if x == nx && inside_z => GridEdge::MaxX,
        (_, -1) if inside_x => GridEdge::MinZ,
        (_, z) if z == nz && inside_x => GridEdge::MaxZ,
        _ => return None,
    };
    Some((edge, 1))
}

/// The cells a click would add or remove, for the hover preview.
pub(crate) fn affected_cells(nx: u32, nz: u32, edge: GridEdge, delta: i32) -> Vec<(i32, i32)> {
    let (nx, nz) = (nx as i32, nz as i32);
    let (x, z): (Option<i32>, Option<i32>) = match (edge, delta > 0) {
        (GridEdge::MinX, true) => (Some(-1), None),
        (GridEdge::MaxX, true) => (Some(nx), None),
        (GridEdge::MinZ, true) => (None, Some(-1)),
        (GridEdge::MaxZ, true) => (None, Some(nz)),
        (GridEdge::MinX, false) => (Some(0), None),
        (GridEdge::MaxX, false) => (Some(nx - 1), None),
        (GridEdge::MinZ, false) => (None, Some(0)),
        (GridEdge::MaxZ, false) => (None, Some(nz - 1)),
    };
    match (x, z) {
        (Some(x), _) => (0..nz).map(|z| (x, z)).collect(),
        (_, Some(z)) => (0..nx).map(|x| (x, z)).collect(),
        _ => Vec::new(),
    }
}

/// The selected terrain, if a terrain is selected.
pub(crate) fn selected_terrain(selection: Option<&EditorSelection>, terrains: &Query<(), With<TerrainData>>) -> Option<Entity> {
    selection.and_then(|s| s.get()).filter(|e| terrains.contains(*e))
}

/// Rebuild [`ChunkMap`] from the selected terrain, and track the pointer.
#[allow(clippy::too_many_arguments)]
pub(crate) fn refresh_chunk_map(
    selection: Option<Res<EditorSelection>>,
    is_terrain: Query<(), With<TerrainData>>,
    terrains: Query<(&TerrainData, &GlobalTransform)>,
    chunks: Query<(&TerrainChunkData, &TerrainChunkOf, Has<TerrainChunkStreamedOut>)>,
    camera: Query<&GlobalTransform, With<EditorCamera>>,
    views: Query<&RelativeCursorPosition, With<ChunkMapView>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut map: ResMut<ChunkMap>,
) {
    let mut next = ChunkMap::default();
    if let Some(terrain) = selected_terrain(selection.as_deref(), &is_terrain) {
        if let Ok((data, xform)) = terrains.get(terrain) {
            next.terrain = Some(terrain);
            next.nx = data.chunks_x;
            next.nz = data.chunks_z;
            next.cells = vec![CellState::Missing; (data.chunks_x * data.chunks_z) as usize];
            for (chunk, of, out) in &chunks {
                if of.0 != terrain || chunk.chunk_x >= data.chunks_x || chunk.chunk_z >= data.chunks_z {
                    continue;
                }
                next.cells[(chunk.chunk_z * data.chunks_x + chunk.chunk_x) as usize] =
                    if out { CellState::StandIn } else { CellState::Full };
            }
            let cell_of = |local: Vec3| {
                (
                    ((local.x + data.total_width() * 0.5) / data.chunk_size).floor() as i32,
                    ((local.z + data.total_depth() * 0.5) / data.chunk_size).floor() as i32,
                )
            };
            next.camera = camera.iter().next().map(|cam| cell_of(xform.affine().inverse().transform_point3(cam.translation())));
            next.hover = views.iter().find_map(|rcp| {
                let n = rcp.normalized.filter(|_| rcp.cursor_over)? + Vec2::splat(0.5);
                let gx = (n.x * (data.chunks_x + 2) as f32).floor() as i32 - 1;
                let gz = (n.y * (data.chunks_z + 2) as f32).floor() as i32 - 1;
                Some((gx, gz))
            });
            next.ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
        }
    }
    if *map != next {
        *map = next;
    }
}

/// Click on the map: grow or shrink the grid. Deferred, because a resize
/// despawns and respawns chunks.
pub(crate) fn chunk_map_click(
    views: Query<&UiInteraction, (With<ChunkMapView>, Changed<UiInteraction>)>,
    map: Res<ChunkMap>,
    mut commands: Commands,
) {
    if !views.iter().any(|i| *i == UiInteraction::Pressed) {
        return;
    }
    let (Some(terrain), Some((gx, gz))) = (map.terrain, map.hover) else { return };
    let Some((edge, delta)) = map_click_action(map.nx, map.nz, gx, gz, map.ctrl) else { return };
    commands.queue(move |world: &mut World| {
        apply_grid_resize(world, terrain, edge, delta);
    });
}

/// The map's pixels for `map`, using the theme's colours.
pub(crate) fn map_pixels(map: &ChunkMap, legal: impl Fn(GridEdge, i32) -> bool) -> Pixels {
    let cols = map.nx as i32 + 2;
    let rows = map.nz as i32 + 2;
    let mut px = Pixels::new((cols * (CELL + GAP) + GAP) as u32, (rows * (CELL + GAP) + GAP) as u32);
    let full = rgba8(rgb(accent()), 1.0);
    let stand_in = rgba8(rgb(accent()), 0.45);
    let missing = rgba8(rgb(text_muted()), 0.35);
    let ring = rgba8(rgb(border()), 0.55);
    let add = rgba8(rgb(accent()), 0.7);
    let remove = rgba8(rgb(close_red()), 0.9);
    let camera = rgba8(rgb(text_primary()), 1.0);

    let at = |gx: i32, gz: i32| ((gx + 1) * (CELL + GAP) + GAP, (gz + 1) * (CELL + GAP) + GAP);
    for gz in -1..=map.nz as i32 {
        for gx in -1..=map.nx as i32 {
            let inside = gx >= 0 && gz >= 0 && gx < map.nx as i32 && gz < map.nz as i32;
            let c = if inside {
                match map.cells.get((gz * map.nx as i32 + gx) as usize) {
                    Some(CellState::Full) => full,
                    Some(CellState::StandIn) => stand_in,
                    _ => missing,
                }
            } else if map_click_action(map.nx, map.nz, gx, gz, false).is_some() {
                ring
            } else {
                continue;
            };
            let (x, y) = at(gx, gz);
            px.rect(x, y, CELL, CELL, c);
        }
    }
    // What a click here would do, drawn over the cells it would touch.
    if let Some((gx, gz)) = map.hover {
        if let Some((edge, delta)) = map_click_action(map.nx, map.nz, gx, gz, map.ctrl).filter(|(e, d)| legal(*e, *d)) {
            for (cx, cz) in affected_cells(map.nx, map.nz, edge, delta) {
                let (x, y) = at(cx, cz);
                px.rect(x, y, CELL, CELL, if delta > 0 { add } else { remove });
            }
        }
    }
    if let Some((gx, gz)) = map.camera {
        if (-1..=map.nx as i32).contains(&gx) && (-1..=map.nz as i32).contains(&gz) {
            let (x, y) = at(gx, gz);
            px.frame(x - 1, y - 1, CELL + 2, CELL + 2, camera);
        }
    }
    px
}

/// Redraw the map when it or the theme changed.
pub(crate) fn draw_chunk_map(
    mut views: Query<(&mut ChunkMapView, &ImageNode, &mut Node)>,
    map: Res<ChunkMap>,
    terrains: Query<&TerrainData>,
    mut images: ResMut<Assets<Image>>,
) {
    let accent_px = rgba8(rgb(accent()), 1.0);
    for (mut view, image, mut node) in &mut views {
        if view.drawn.as_ref().is_some_and(|(m, a)| m == &*map && *a == accent_px) {
            continue;
        }
        let data = map.terrain.and_then(|t| terrains.get(t).ok());
        let px = map_pixels(&map, |edge, delta| data.is_some_and(|d| plan_grid_resize(d, edge, delta).is_some()));
        let ratio = px.w as f32 / px.h.max(1) as f32;
        if node.aspect_ratio != Some(ratio) {
            node.aspect_ratio = Some(ratio);
        }
        write_pixels(&mut images, image, px.w, px.h, px.data);
        view.drawn = Some((map.clone(), accent_px));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ring_cell_beside_an_edge_adds_that_edge() {
        assert_eq!(map_click_action(4, 3, -1, 1, false), Some((GridEdge::MinX, 1)));
        assert_eq!(map_click_action(4, 3, 4, 0, false), Some((GridEdge::MaxX, 1)));
        assert_eq!(map_click_action(4, 3, 2, -1, false), Some((GridEdge::MinZ, 1)));
        assert_eq!(map_click_action(4, 3, 2, 3, false), Some((GridEdge::MaxZ, 1)));
    }

    #[test]
    fn diagonal_ring_cells_and_inner_chunks_do_nothing_on_a_plain_click() {
        assert_eq!(map_click_action(4, 3, -1, -1, false), None);
        assert_eq!(map_click_action(4, 3, 4, 3, false), None);
        assert_eq!(map_click_action(4, 3, 1, 1, false), None);
    }

    /// Removal needs Ctrl and an edge chunk; an inner chunk has no row to drop.
    #[test]
    fn ctrl_click_on_an_edge_chunk_removes_its_row() {
        assert_eq!(map_click_action(4, 3, 0, 1, true), Some((GridEdge::MinX, -1)));
        assert_eq!(map_click_action(4, 3, 3, 1, true), Some((GridEdge::MaxX, -1)));
        assert_eq!(map_click_action(4, 3, 1, 0, true), Some((GridEdge::MinZ, -1)));
        assert_eq!(map_click_action(4, 3, 1, 2, true), Some((GridEdge::MaxZ, -1)));
        assert_eq!(map_click_action(4, 3, 0, 0, true), Some((GridEdge::MinX, -1)), "corners: X wins");
        assert_eq!(map_click_action(4, 3, 1, 1, true), None);
        assert_eq!(map_click_action(4, 3, -1, 1, true), None, "nothing to remove outside the grid");
    }

    #[test]
    fn the_preview_covers_the_whole_row_a_click_touches() {
        assert_eq!(affected_cells(4, 3, GridEdge::MinX, 1), vec![(-1, 0), (-1, 1), (-1, 2)]);
        assert_eq!(affected_cells(4, 3, GridEdge::MaxZ, -1).len(), 4);
        assert!(affected_cells(4, 3, GridEdge::MaxZ, -1).iter().all(|(_, z)| *z == 2));
    }

    #[test]
    fn counts_and_image_size_follow_the_grid() {
        let map = ChunkMap {
            nx: 3,
            nz: 2,
            cells: vec![CellState::Full, CellState::StandIn, CellState::Full, CellState::Full, CellState::Missing, CellState::StandIn],
            ..default()
        };
        assert_eq!((map.resident(), map.total()), (3, 6));
        let px = map_pixels(&map, |_, _| true);
        assert_eq!(px.w as i32, 5 * (CELL + GAP) + GAP);
        assert_eq!(px.h as i32, 4 * (CELL + GAP) + GAP);
    }
}
