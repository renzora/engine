//! The Generate tool — draw a rectangle on the terrain and fill it with
//! procedural mountains.
//!
//! The generator itself is in [`renzora_terrain::generate`]; everything here is
//! the part you can point at. That split matters: the maths is testable without
//! a `World`, and this file is only ever about turning a cursor into a rectangle
//! and a button into one committed edit.
//!
//! # Why there is a gizmo at all
//!
//! A generator driven purely from a panel is a guessing game. You set six
//! numbers, press a button, and find out. The numbers that matter most —
//! *where*, *how big*, and *how tall relative to the terrain you already have* —
//! are exactly the ones that are hopeless to picture from a field.
//!
//! So the region is a rectangle you drag in the scene, and above it sits a
//! wireframe of the surface the current settings would produce. Nothing is
//! written until you press Generate; until then, moving a slider re-draws the
//! preview and that is all it does. The preview and the apply pass call the same
//! [`renzora_terrain::generate::blended_height`], so what you see is the result
//! and not an approximation of it.
//!
//! # Picking versus drawing
//!
//! Handles are *picked* on the terrain's flat ground plane — the cursor ray
//! meets a plane, not the sculpted surface — because a grab point that slid
//! along a hillside as you dragged would make the rectangle impossible to place.
//! They are therefore also *drawn* on that plane, with a vertical post running up
//! to the preview surface at each corner so the region still reads as a volume
//! rather than as a flat outline floating under the mountains.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use renzora::core::viewport_types::ViewportState;
use renzora::core::EditorCamera;
use renzora_editor_framework::{ActiveTool, EditorSelection};
use renzora_terrain::data::{TerrainChunkData, TerrainChunkOf, TerrainData};
use renzora_terrain::generate::{
    apply_to_chunk, blended_height, region_weight, GenRegion, TerrainGenSettings,
};

use crate::region_tool::ray_to_terrain_plane;
use crate::systems::viewport_cursor_ray;

/// Phosphor icon for the mode button on the viewport's top strip. A const so
/// the test below can check it actually resolves — an unknown name doesn't
/// fail, it renders as the literal string "magic-wand" sitting in the button.
pub const TOOL_ICON: &str = "magic-wand";

/// How far above the terrain's ground plane the outline and handles are drawn,
/// in world units — enough to clear a flat terrain without floating free of it.
const GROUND_LIFT: f32 = 0.05;

/// Cells per axis in the preview wireframe. 24 is the point where the ridge
/// lines are legible without the grid becoming the thing you look at; the mesh
/// it stands for is one to two orders of magnitude denser.
const PREVIEW_CELLS: usize = 24;

/// Handle hit radius as a fraction of the region's longest side, then clamped to
/// this metre range. Proportional so the handles stay grabbable on a 2 km
/// terrain, clamped so they don't swallow a small region whole.
const HANDLE_FRACTION: f32 = 0.05;
const HANDLE_MIN: f32 = 1.5;
const HANDLE_MAX: f32 = 12.0;

/// A region narrower than this many vertex spacings has nothing to generate
/// into, and lets a careless drag collapse the rectangle to a line you then
/// can't grab.
const MIN_REGION_VERTS: f32 = 4.0;

/// Which part of the rectangle the cursor is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenHandle {
    /// 0 = (min x, min z), then clockwise: 1 = (max x, min z), 2 = (max, max),
    /// 3 = (min x, max z).
    Corner(usize),
    /// 0 = −X, 1 = +X, 2 = −Z, 3 = +Z.
    Edge(usize),
    /// Anywhere inside that isn't a handle — drags the whole rectangle.
    Body,
}

/// An in-progress drag. The rectangle is recomputed from `start` each frame
/// rather than accumulated, so a dropped frame or a clamp against the terrain
/// edge can't make the region creep away from the cursor.
#[derive(Clone, Copy, Debug)]
struct GenDrag {
    handle: GenHandle,
    grab: Vec2,
    start: GenRegion,
}

/// Per-frame tool state: what the cursor is over and what it is dragging.
#[derive(Resource, Default)]
pub struct GenerateHover {
    /// The terrain being edited.
    pub terrain: Option<Entity>,
    /// Cursor position on the terrain's ground plane, in terrain-local metres
    /// with the origin at the grid's minimum corner.
    pub cursor: Option<Vec2>,
    /// The handle under the cursor, if any.
    pub handle: Option<GenHandle>,
    drag: Option<GenDrag>,
}

impl GenerateHover {
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }
}

/// True while the Generate tool is the active one.
pub fn generate_tool_active(tool: Option<Res<ActiveTool>>) -> bool {
    tool.is_some_and(|t| *t == ActiveTool::TerrainGenerate)
}

/// Which terrain the tool acts on: the selected one, else the first in the
/// scene — matching how the other terrain tools pick a terrain.
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

/// Terrain-local *centred* space (what the transform is in) → the min-corner
/// space the generator and the chunk grid use.
fn to_corner_space(data: &TerrainData, centred: Vec3) -> Vec2 {
    Vec2::new(
        centred.x + data.total_width() * 0.5,
        centred.z + data.total_depth() * 0.5,
    )
}

/// A point in min-corner space at normalized height `h` → world space.
fn to_world(data: &TerrainData, xform: &GlobalTransform, p: Vec2, y: f32) -> Vec3 {
    xform.transform_point(Vec3::new(
        p.x - data.total_width() * 0.5,
        y,
        p.y - data.total_depth() * 0.5,
    ))
}

fn handle_radius(region: &GenRegion) -> f32 {
    (region.size().max_element() * HANDLE_FRACTION).clamp(HANDLE_MIN, HANDLE_MAX)
}

/// The eight handle positions, corners first so a corner wins over the edge it
/// sits on — grabbing a corner and getting an edge is the more annoying mistake
/// of the two.
fn handle_points(region: &GenRegion) -> [(GenHandle, Vec2); 8] {
    let (mn, mx) = (region.min, region.max);
    let c = region.center();
    [
        (GenHandle::Corner(0), Vec2::new(mn.x, mn.y)),
        (GenHandle::Corner(1), Vec2::new(mx.x, mn.y)),
        (GenHandle::Corner(2), Vec2::new(mx.x, mx.y)),
        (GenHandle::Corner(3), Vec2::new(mn.x, mx.y)),
        (GenHandle::Edge(0), Vec2::new(mn.x, c.y)),
        (GenHandle::Edge(1), Vec2::new(mx.x, c.y)),
        (GenHandle::Edge(2), Vec2::new(c.x, mn.y)),
        (GenHandle::Edge(3), Vec2::new(c.x, mx.y)),
    ]
}

/// The handle at `cursor`, or `Body` if the cursor is inside the rectangle, or
/// `None` if it's outside it entirely.
pub fn pick_handle(region: &GenRegion, cursor: Vec2) -> Option<GenHandle> {
    let r = handle_radius(region);
    for (handle, p) in handle_points(region) {
        if (cursor - p).abs().max_element() <= r {
            return Some(handle);
        }
    }
    region.contains(cursor).then_some(GenHandle::Body)
}

/// Apply a drag to the rectangle it started from.
///
/// Kept as a free function over plain values so the drag arithmetic — the part
/// that has to survive dragging a corner past its opposite, and has to keep the
/// rectangle on the terrain — is testable without a cursor.
pub fn drag_region(
    start: &GenRegion,
    handle: GenHandle,
    delta: Vec2,
    terrain: &TerrainData,
) -> GenRegion {
    let mut min = start.min;
    let mut max = start.max;
    match handle {
        GenHandle::Corner(i) => {
            if i == 0 || i == 3 {
                min.x += delta.x;
            } else {
                max.x += delta.x;
            }
            if i == 0 || i == 1 {
                min.y += delta.y;
            } else {
                max.y += delta.y;
            }
        }
        GenHandle::Edge(0) => min.x += delta.x,
        GenHandle::Edge(1) => max.x += delta.x,
        GenHandle::Edge(2) => min.y += delta.y,
        GenHandle::Edge(_) => max.y += delta.y,
        GenHandle::Body => {
            // Moving must not also resize, so the whole rectangle is shifted and
            // then pushed back inside the terrain — clamping the corners
            // independently would squash it against the edge instead.
            let size = start.size();
            let limit = Vec2::new(terrain.total_width(), terrain.total_depth()) - size;
            min = (start.min + delta).clamp(Vec2::ZERO, limit.max(Vec2::ZERO));
            max = min + size;
            return GenRegion { min, max };
        }
    }
    let region = GenRegion::new(min, max).clamped_to(terrain);
    // Enforce a floor on the size *after* clamping: a rectangle dragged to
    // nothing is one you can no longer grab a handle on.
    let floor = terrain.vertex_spacing() * MIN_REGION_VERTS;
    let size = region.size();
    if size.x >= floor && size.y >= floor {
        return region;
    }
    let c = region.center();
    let half = Vec2::new(size.x.max(floor), size.y.max(floor)) * 0.5;
    GenRegion::new(c - half, c + half).clamped_to(terrain)
}

// ── Hover + drag ─────────────────────────────────────────────────────────────

/// Track the cursor on the terrain plane and drive any active drag.
#[allow(clippy::too_many_arguments)]
pub fn generate_hover_system(
    viewport: Res<ViewportState>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    terrains: Query<(Entity, &TerrainData, &GlobalTransform)>,
    selection: Res<EditorSelection>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut settings: ResMut<TerrainGenSettings>,
    mut hover: ResMut<GenerateHover>,
) {
    hover.terrain = active_terrain(&selection, &terrains);
    hover.cursor = None;

    let Some(terrain_entity) = hover.terrain else {
        hover.handle = None;
        hover.drag = None;
        return;
    };
    let Ok((_, data, xform)) = terrains.get(terrain_entity) else {
        return;
    };

    let cursor = viewport_cursor_ray(&viewport, &window_query, &camera_query)
        .and_then(|ray| ray_to_terrain_plane(ray, xform))
        .map(|local| to_corner_space(data, local));
    hover.cursor = cursor;

    let region = settings.region(data);

    // A drag survives the cursor leaving the viewport — releasing the button
    // outside it still ends the drag, rather than leaving the rectangle stuck to
    // the pointer when it comes back.
    if let Some(drag) = hover.drag {
        if !mouse.pressed(MouseButton::Left) {
            hover.drag = None;
        } else if let Some(c) = cursor {
            let next = drag_region(&drag.start, drag.handle, c - drag.grab, data);
            settings.set_region(next);
        }
        return;
    }

    hover.handle = cursor.and_then(|c| pick_handle(&region, c));

    if mouse.just_pressed(MouseButton::Left) {
        if let (Some(c), Some(handle)) = (cursor, hover.handle) {
            // Materialise the rectangle on the first drag: until now it may have
            // been the whole-terrain one, which is derived rather than stored.
            hover.drag = Some(GenDrag {
                handle,
                grab: c,
                start: region,
            });
            settings.set_region(region);
        }
    }
}

// ── Gizmo ────────────────────────────────────────────────────────────────────

/// Draw the region rectangle, its handles, and the preview of what Generate
/// would produce.
pub fn generate_gizmo_system(
    mut gizmos: Gizmos,
    terrains: Query<(Entity, &TerrainData, &GlobalTransform)>,
    chunks: Query<(&TerrainChunkData, &TerrainChunkOf)>,
    settings: Res<TerrainGenSettings>,
    hover: Res<GenerateHover>,
) {
    let Some(terrain_entity) = hover.terrain else {
        return;
    };
    let Ok((_, data, xform)) = terrains.get(terrain_entity) else {
        return;
    };
    let region = settings.region(data);

    let outline = Color::srgba(0.45, 0.85, 1.0, 0.9);
    let handle_idle = Color::srgba(0.45, 0.85, 1.0, 0.75);
    let handle_hot = Color::srgba(1.0, 0.9, 0.35, 1.0);
    let post = Color::srgba(0.45, 0.85, 1.0, 0.35);

    // The rectangle, on the ground plane.
    let corners = [
        Vec2::new(region.min.x, region.min.y),
        Vec2::new(region.max.x, region.min.y),
        Vec2::new(region.max.x, region.max.y),
        Vec2::new(region.min.x, region.max.y),
    ];
    for i in 0..4 {
        gizmos.line(
            to_world(data, xform, corners[i], GROUND_LIFT),
            to_world(data, xform, corners[(i + 1) % 4], GROUND_LIFT),
            outline,
        );
    }

    // Handles. Body isn't drawn — the outline already is the body.
    let r = handle_radius(&region);
    for (handle, p) in handle_points(&region) {
        let hot = hover.handle == Some(handle) && !hover.dragging()
            || hover.drag.map(|d| d.handle) == Some(handle);
        let color = if hot { handle_hot } else { handle_idle };
        let a = to_world(data, xform, p - Vec2::splat(r * 0.5), GROUND_LIFT);
        let b = to_world(data, xform, p + Vec2::new(r * 0.5, -r * 0.5), GROUND_LIFT);
        let c = to_world(data, xform, p + Vec2::splat(r * 0.5), GROUND_LIFT);
        let d = to_world(data, xform, p + Vec2::new(-r * 0.5, r * 0.5), GROUND_LIFT);
        gizmos.line(a, b, color);
        gizmos.line(b, c, color);
        gizmos.line(c, d, color);
        gizmos.line(d, a, color);
    }

    if !settings.preview {
        return;
    }

    // The terrain's chunks, borrowed once. The preview reads existing heights
    // at ~1200 points per frame and a query walk per point is the difference
    // between a free gizmo and a stutter.
    let owned: Vec<(u32, u32, &[f32])> = chunks
        .iter()
        .filter(|(_, owner)| owner.0 == terrain_entity)
        .map(|(chunk, _)| {
            (
                chunk.chunk_x,
                chunk.chunk_z,
                chunk.base_heights.as_slice(),
            )
        })
        .collect();
    let step = region.size() / PREVIEW_CELLS as f32;
    if step.x <= 0.0 || step.y <= 0.0 {
        return;
    }

    // The surface, as a wireframe. Alpha follows the region weight, so the
    // feather band is visible as the preview fading into the ground rather than
    // being a number you have to imagine.
    let point = |ix: usize, iz: usize| -> (Vec3, f32) {
        let p = region.min + Vec2::new(ix as f32 * step.x, iz as f32 * step.y);
        let current = sample_base(&owned, data, p);
        let h = blended_height(&settings, data, &region, current, p.x, p.y);
        let y = data.min_height + h * data.height_range();
        (
            to_world(data, xform, p, y),
            region_weight(&region, settings.feather, p.x, p.y),
        )
    };

    let line = |gizmos: &mut Gizmos, a: (Vec3, f32), b: (Vec3, f32)| {
        let w = (a.1 + b.1) * 0.5;
        gizmos.line(a.0, b.0, Color::srgba(0.55, 0.9, 1.0, 0.15 + 0.6 * w));
    };

    for iz in 0..=PREVIEW_CELLS {
        let mut prev = point(0, iz);
        for ix in 1..=PREVIEW_CELLS {
            let cur = point(ix, iz);
            line(&mut gizmos, prev, cur);
            prev = cur;
        }
    }
    for ix in 0..=PREVIEW_CELLS {
        let mut prev = point(ix, 0);
        for iz in 1..=PREVIEW_CELLS {
            let cur = point(ix, iz);
            line(&mut gizmos, prev, cur);
            prev = cur;
        }
    }

    // Corner posts from the plane up to the preview surface, so the rectangle
    // and the wireframe read as one object.
    for c in corners {
        let current = sample_base(&owned, data, c);
        let h = blended_height(&settings, data, &region, current, c.x, c.y);
        gizmos.line(
            to_world(data, xform, c, GROUND_LIFT),
            to_world(data, xform, c, data.min_height + h * data.height_range()),
            post,
        );
    }
}

/// Read a terrain's existing base heightmap at an arbitrary local point.
///
/// Nearest-vertex, not bilinear: the preview grid is far coarser than the
/// heightmap, so interpolating between two vertices it is already skipping past
/// buys nothing.
fn sample_base(owned: &[(u32, u32, &[f32])], data: &TerrainData, p: Vec2) -> f32 {
    let res = data.chunk_resolution;
    if res == 0 || data.chunk_size <= 0.0 {
        return 0.0;
    }
    let cx = ((p.x / data.chunk_size).floor().max(0.0) as u32).min(data.chunks_x.saturating_sub(1));
    let cz = ((p.y / data.chunk_size).floor().max(0.0) as u32).min(data.chunks_z.saturating_sub(1));
    let spacing = data.vertex_spacing();
    let vx = (((p.x - cx as f32 * data.chunk_size) / spacing).round().max(0.0) as u32).min(res - 1);
    let vz = (((p.y - cz as f32 * data.chunk_size) / spacing).round().max(0.0) as u32).min(res - 1);
    owned
        .iter()
        .find(|(chunk_x, chunk_z, _)| *chunk_x == cx && *chunk_z == cz)
        .and_then(|(_, _, heights)| heights.get((vz * res + vx) as usize).copied())
        .unwrap_or(0.0)
}

// ── Apply ────────────────────────────────────────────────────────────────────

/// One chunk's base heightmap, keyed by the chunk *entity*.
///
/// By entity and not by `(chunk_x, chunk_z)`: a scene can hold more than one
/// terrain, and two of them will happily both have a chunk (0, 0).
#[derive(Clone)]
pub struct GenerateUndo {
    chunks: Vec<(Entity, Vec<f32>)>,
}

fn restore_generated(world: &mut World, entry: &GenerateUndo) {
    for (entity, heights) in &entry.chunks {
        if let Some(mut chunk) = world.get_mut::<TerrainChunkData>(*entity) {
            chunk.base_heights = heights.clone();
            chunk.dirty = true;
        }
    }
}

/// Run the generator over `terrain` and record it as one undo step.
///
/// Exclusive, and queued through `EditorCommands` by the toolbar button, because
/// it snapshots every chunk twice and calls into `renzora_undo` — neither of
/// which belongs inside a system holding queries over the chunks.
pub fn generate_now(world: &mut World, terrain: Entity) {
    let Some(data) = world.get::<TerrainData>(terrain).cloned() else {
        return;
    };
    let settings = world
        .get_resource::<TerrainGenSettings>()
        .cloned()
        .unwrap_or_default();
    let region = settings.region(&data);

    // The chunks this terrain owns, in one pass, so the snapshot and the apply
    // agree on the set even if something else spawns a chunk in between.
    let owned: Vec<Entity> = {
        let mut q = world.query::<(Entity, &TerrainChunkOf)>();
        q.iter(world)
            .filter(|(_, owner)| owner.0 == terrain)
            .map(|(e, _)| e)
            .collect()
    };

    let snapshot = |world: &mut World, owned: &[Entity]| GenerateUndo {
        chunks: owned
            .iter()
            .filter_map(|e| {
                world
                    .get::<TerrainChunkData>(*e)
                    .map(|c| (*e, c.base_heights.clone()))
            })
            .collect(),
    };

    let before = snapshot(world, &owned);

    let mut changed = false;
    for entity in &owned {
        // Take the component out from under the borrow checker's feet: the
        // generator needs `&TerrainData` and `&mut TerrainChunkData` at once,
        // and they live on different entities.
        let Some(mut chunk) = world.get_mut::<TerrainChunkData>(*entity) else {
            continue;
        };
        changed |= apply_to_chunk(&mut chunk, &data, &settings, &region);
    }
    if !changed {
        return;
    }

    let after = snapshot(world, &owned);
    renzora_undo::record(
        world,
        renzora_undo::UndoContext::Scene,
        Box::new(renzora_undo::SnapshotCmd {
            label: "Generate Terrain".to_string(),
            before,
            after,
            restore: restore_generated,
            // One press is one step, always — nothing to merge with.
            merge_key: None,
        }),
    );
    renzora_undo::seal(world, &renzora_undo::UndoContext::Scene);
}

/// Flatten the terrain's base layer back to the level `base` sits at, so a
/// generate you don't like can be undone *forwards* as well as with Ctrl+Z.
pub fn reset_now(world: &mut World, terrain: Entity) {
    let Some(data) = world.get::<TerrainData>(terrain).cloned() else {
        return;
    };
    let base = world
        .get_resource::<TerrainGenSettings>()
        .map(|g| g.base)
        .unwrap_or(0.0);
    let range = data.height_range();
    if range <= 0.0 {
        return;
    }
    let level = ((base - data.min_height) / range).clamp(0.0, 1.0);

    let owned: Vec<Entity> = {
        let mut q = world.query::<(Entity, &TerrainChunkOf)>();
        q.iter(world)
            .filter(|(_, owner)| owner.0 == terrain)
            .map(|(e, _)| e)
            .collect()
    };
    let snapshot = |world: &mut World, owned: &[Entity]| GenerateUndo {
        chunks: owned
            .iter()
            .filter_map(|e| {
                world
                    .get::<TerrainChunkData>(*e)
                    .map(|c| (*e, c.base_heights.clone()))
            })
            .collect(),
    };
    let before = snapshot(world, &owned);

    let mut changed = false;
    for entity in &owned {
        let Some(mut chunk) = world.get_mut::<TerrainChunkData>(*entity) else {
            continue;
        };
        if chunk.base_heights.iter().all(|h| (*h - level).abs() < 1e-6) {
            continue;
        }
        chunk.base_heights.fill(level);
        chunk.dirty = true;
        changed = true;
    }
    if !changed {
        return;
    }

    let after = snapshot(world, &owned);
    renzora_undo::record(
        world,
        renzora_undo::UndoContext::Scene,
        Box::new(renzora_undo::SnapshotCmd {
            label: "Flatten Terrain".to_string(),
            before,
            after,
            restore: restore_generated,
            merge_key: None,
        }),
    );
    renzora_undo::seal(world, &renzora_undo::UndoContext::Scene);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> TerrainData {
        TerrainData {
            chunks_x: 4,
            chunks_z: 4,
            chunk_size: 64.0,
            chunk_resolution: 65,
            ..TerrainData::default()
        }
    }

    fn region() -> GenRegion {
        GenRegion::new(Vec2::new(50.0, 50.0), Vec2::new(200.0, 200.0))
    }

    #[test]
    fn the_tool_icon_resolves() {
        assert!(
            renzora_ember::font::icon_glyph(TOOL_ICON).is_some(),
            "unknown Phosphor icon {TOOL_ICON:?}"
        );
    }

    #[test]
    fn corners_are_picked_at_their_own_positions() {
        let r = region();
        assert_eq!(pick_handle(&r, r.min), Some(GenHandle::Corner(0)));
        assert_eq!(
            pick_handle(&r, Vec2::new(r.max.x, r.min.y)),
            Some(GenHandle::Corner(1))
        );
        assert_eq!(pick_handle(&r, r.max), Some(GenHandle::Corner(2)));
        assert_eq!(
            pick_handle(&r, Vec2::new(r.min.x, r.max.y)),
            Some(GenHandle::Corner(3))
        );
    }

    /// A corner sits on two edges. Grabbing one and getting an edge instead is
    /// the worse of the two mistakes, so corners must win the tie.
    #[test]
    fn a_corner_beats_the_edges_it_sits_on() {
        let r = region();
        assert_eq!(pick_handle(&r, r.min), Some(GenHandle::Corner(0)));
    }

    #[test]
    fn the_interior_is_the_body_and_the_outside_is_nothing() {
        let r = region();
        assert_eq!(pick_handle(&r, r.center()), Some(GenHandle::Body));
        assert_eq!(pick_handle(&r, Vec2::new(-40.0, 125.0)), None);
    }

    #[test]
    fn dragging_a_corner_leaves_the_opposite_one_fixed() {
        let d = data();
        let r = region();
        let out = drag_region(&r, GenHandle::Corner(0), Vec2::new(20.0, 30.0), &d);
        assert_eq!(out.max, r.max);
        assert_eq!(out.min, Vec2::new(70.0, 80.0));
    }

    #[test]
    fn dragging_an_edge_moves_only_that_edge() {
        let d = data();
        let r = region();
        let out = drag_region(&r, GenHandle::Edge(1), Vec2::new(25.0, 999.0), &d);
        assert_eq!(out.min, r.min);
        assert_eq!(out.max, Vec2::new(r.max.x + 25.0, r.max.y));
    }

    /// Moving must not resize. Clamping the two corners independently would
    /// squash the rectangle against the terrain edge instead of stopping it.
    #[test]
    fn dragging_the_body_preserves_the_size() {
        let d = data();
        let r = region();
        for delta in [
            Vec2::new(30.0, -20.0),
            Vec2::new(-9999.0, 0.0),
            Vec2::new(9999.0, 9999.0),
        ] {
            let out = drag_region(&r, GenHandle::Body, delta, &d);
            assert!((out.size() - r.size()).abs().max_element() < 1e-3, "{delta:?}");
        }
    }

    #[test]
    fn a_body_drag_stays_on_the_terrain() {
        let d = data();
        let out = drag_region(&region(), GenHandle::Body, Vec2::splat(9999.0), &d);
        assert!(out.max.x <= d.total_width() + 1e-3);
        assert!(out.max.y <= d.total_depth() + 1e-3);
        assert!(out.min.x >= -1e-3 && out.min.y >= -1e-3);
    }

    /// Dragging a corner past its opposite has to keep producing a rectangle —
    /// and one you can still grab a handle on next frame.
    #[test]
    fn a_corner_dragged_past_its_opposite_stays_grabbable() {
        let d = data();
        let out = drag_region(&region(), GenHandle::Corner(0), Vec2::splat(400.0), &d);
        let floor = d.vertex_spacing() * MIN_REGION_VERTS;
        assert!(out.size().x >= floor - 1e-3, "collapsed on x: {:?}", out);
        assert!(out.size().y >= floor - 1e-3, "collapsed on y: {:?}", out);
        assert!(pick_handle(&out, out.min).is_some());
    }

    /// An edge dragged onto its opposite must not collapse the rectangle to a
    /// line, or the tool becomes unrecoverable without a Reset.
    #[test]
    fn an_edge_cannot_collapse_the_region() {
        let d = data();
        let out = drag_region(&region(), GenHandle::Edge(0), Vec2::new(400.0, 0.0), &d);
        assert!(out.size().x >= d.vertex_spacing() * MIN_REGION_VERTS - 1e-3);
    }

    #[test]
    fn a_resize_drag_stays_on_the_terrain() {
        let d = data();
        let out = drag_region(&region(), GenHandle::Corner(2), Vec2::splat(9999.0), &d);
        assert!(out.max.x <= d.total_width() + 1e-3);
        assert!(out.max.y <= d.total_depth() + 1e-3);
    }

    /// Handles have to stay grabbable at both extremes: a small region must not
    /// be swallowed by its own handles, and a huge one must not have handles too
    /// small to hit.
    #[test]
    fn handle_radius_stays_within_its_clamp() {
        let tiny = GenRegion::new(Vec2::ZERO, Vec2::splat(2.0));
        let huge = GenRegion::new(Vec2::ZERO, Vec2::splat(4000.0));
        assert!((handle_radius(&tiny) - HANDLE_MIN).abs() < 1e-5);
        assert!((handle_radius(&huge) - HANDLE_MAX).abs() < 1e-5);
    }

    /// The centre of a region must be the body even when the region is small
    /// enough that the handle boxes crowd it — otherwise you cannot move a small
    /// region at all.
    #[test]
    fn a_small_region_can_still_be_moved() {
        let small = GenRegion::new(Vec2::splat(100.0), Vec2::splat(120.0));
        assert_eq!(pick_handle(&small, small.center()), Some(GenHandle::Body));
    }

    /// The gizmo converts between three spaces; a sign error in either direction
    /// puts the rectangle on the wrong side of the terrain.
    #[test]
    fn corner_space_round_trips_through_world_space() {
        let d = data();
        let x = GlobalTransform::from(Transform::from_xyz(12.0, 0.0, -34.0));
        for p in [Vec2::ZERO, Vec2::new(64.0, 192.0), Vec2::splat(256.0)] {
            let world = to_world(&d, &x, p, 0.0);
            let back = to_corner_space(&d, x.affine().inverse().transform_point3(world));
            assert!((back - p).abs().max_element() < 1e-3, "{p:?} → {back:?}");
        }
    }
}
