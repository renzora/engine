//! Turning a flat mesh into a terrain.
//!
//! Before this, a terrain could only start life as a terrain: the Add menu's
//! Terrain preset spawned a `TerrainData` root with its own chunk grid, and the
//! terrain toolbar appeared only once such a root existed somewhere in the
//! scene. A plane you had already dropped in, sized and placed — the usual first
//! thing in a scene — had no route to the brushes at all. You had to delete it,
//! add a terrain, and put the terrain back where the plane was.
//!
//! So: select any flat mesh and the terrain strip offers **Make Terrain**. The
//! entity keeps its identity — same `Entity`, name, place in the hierarchy,
//! script components, project material — and swaps its single flat mesh for the
//! chunk grid the sculpt and paint brushes work on.
//!
//! ### What "flat" means here
//!
//! Not "was spawned from the Plane primitive". A mesh qualifies on its *shape*
//! (see [`plane_footprint`]) — its bounds are thin in Y next to their X/Z
//! footprint — so a subdivided grid, an imported ground plane, or a scaled
//! quad all count, and a cube does not. That is the reading of "any plane" that
//! matches what someone is actually looking at when they click.
//!
//! The new heightmap starts flat at the plane's level rather than sampling
//! whatever sub-tolerance relief the mesh had. Anything inside the flatness
//! tolerance is under a hundredth of the footprint — finer than a chunk's
//! vertex spacing resolves — so a mesh-to-heightmap resample would cost a pass
//! over every vertex to reproduce a difference nobody can see.
//!
//! ### Why the transform is flattened
//!
//! Conversion bakes the entity's scale into the terrain's chunk size and resets
//! rotation to identity, keeping only the translation. This is not tidiness:
//! the sculpt and paint systems map a world-space brush hit into heightmap
//! coordinates using the terrain root's `translation()` alone (see
//! `systems::terrain_sculpt_hover_system`), so a rotated or scaled terrain root
//! would put every brush stroke somewhere other than under the cursor. Baking
//! the scale keeps the terrain covering exactly the ground the plane covered.

use bevy::camera::primitives::Aabb;
use bevy::prelude::*;

use renzora::console_log::console_info;
use renzora::core::viewport_types::{ViewportMode, ViewportSettings};
use renzora::core::{EditedMesh, EditedMeshApplied, MaterialRef, MeshInstanceData, MeshPrimitive};
use renzora_editor_framework::{ActiveTool, EditorSelection};
use renzora_terrain::data::{TerrainChunkData, TerrainData};
use renzora_terrain::painter::Painter;

use crate::terrain_inspector::TerrainInspectorTab;

/// The strip button's icon. Deliberately *not* the `mountains` that Sculpt
/// wears: a scene can hold a terrain and have a plane selected at the same
/// time, which puts both buttons in the row at once, and two identical
/// mountains would read as one button drawn twice. Not the wand either — that
/// one is [`crate::generate_tool::TOOL_ICON`].
pub const TOOL_ICON: &str = "shovel";

/// How thin a mesh has to be, as a fraction of its widest horizontal span, to
/// read as a plane. Two percent keeps a genuinely flat mesh in (including one
/// with a little sculpted relief already on it) and a wall, a slab or a cube
/// out.
const FLATNESS_RATIO: f32 = 0.02;

/// The chunk size a conversion aims for, in world units — the same 64 m a
/// terrain spawned from the Add menu uses, so a converted plane of the usual
/// size ends up with a grid that behaves identically to a hand-added one.
const TARGET_CHUNK_SIZE: f32 = 64.0;

/// The vertex spacing a conversion aims for, in world units. Picking the
/// resolution from this rather than always using the 129 default is what stops
/// a 2 m plane from becoming a 16k-vertex chunk with 1.5 cm triangles.
const TARGET_VERTEX_SPACING: f32 = 0.5;

/// Chunk resolutions to choose between, smallest first. Terrain resolutions are
/// `2^n + 1` so a chunk's edge vertices line up with its neighbour's.
const RESOLUTION_LADDER: [u32; 3] = [33, 65, 129];

/// Most chunks a conversion will produce. A ground plane scaled out to
/// kilometres would otherwise ask for thousands of chunks and hang the editor
/// building them; past this the chunk size grows instead, trading resolution
/// for a terrain that actually appears.
const MAX_CHUNKS: u32 = 64;

// ── Detection ───────────────────────────────────────────────────────────────

/// The world-space X/Z footprint of `entity` if it is a flat mesh that could
/// become a terrain, or `None` if it is anything else.
///
/// Reads the `Aabb` Bevy computes for every `Mesh3d` rather than the mesh asset
/// itself, so this stays cheap enough for a per-frame toolbar predicate. That
/// costs one frame of latency on a freshly spawned mesh (`compute_aabb_system`
/// runs in `PostUpdate`), which is invisible on a button that appears when you
/// click something.
pub fn plane_footprint(world: &World, entity: Entity) -> Option<Vec2> {
    // Already a terrain, or part of one.
    if world.get::<TerrainData>(entity).is_some() || world.get::<TerrainChunkData>(entity).is_some()
    {
        return None;
    }
    world.get::<Mesh3d>(entity)?;
    // A flat mesh inside an imported model is one of the model's own parts: the
    // glTF instance respawns its children from the source file, so converting
    // one would leave a terrain sitting next to a plane that came straight back.
    if inside_model_instance(world, entity) {
        return None;
    }

    let half = Vec3::from(world.get::<Aabb>(entity)?.half_extents);
    let footprint = Vec2::new(half.x, half.z) * 2.0;
    let span = footprint.max_element();
    if span <= f32::EPSILON {
        return None;
    }
    if half.y * 2.0 > span * FLATNESS_RATIO {
        return None;
    }

    // The `Aabb` is in local space; the terrain replaces the mesh on the ground
    // it actually covers, so the entity's world scale belongs in the footprint.
    let scale = world
        .get::<GlobalTransform>(entity)
        .map(|gt| gt.scale())
        .unwrap_or(Vec3::ONE);
    Some(footprint * Vec2::new(scale.x.abs(), scale.z.abs()))
}

/// The scale and rotation `entity` inherits from its parent chain, which
/// [`convert`] cancels out locally. `(ONE, IDENTITY)` for an unparented entity.
fn inherited_scale_rotation(world: &World, entity: Entity) -> (Vec3, Quat) {
    let parent = match world.get::<ChildOf>(entity) {
        Some(child_of) => child_of.parent(),
        None => return (Vec3::ONE, Quat::IDENTITY),
    };
    match world.get::<GlobalTransform>(parent) {
        Some(gt) => {
            let (scale, rotation, _) = gt.to_scale_rotation_translation();
            (scale, rotation)
        }
        None => (Vec3::ONE, Quat::IDENTITY),
    }
}

/// `1 / v`, holding a degenerate (zero) scale axis at 1 rather than letting an
/// infinity through into a transform.
fn safe_recip(v: f32) -> f32 {
    if v.abs() < 1e-6 {
        1.0
    } else {
        1.0 / v
    }
}

/// Whether `entity` sits under an imported model root.
fn inside_model_instance(world: &World, entity: Entity) -> bool {
    let mut cursor = entity;
    loop {
        if world.get::<MeshInstanceData>(cursor).is_some() {
            return true;
        }
        match world.get::<ChildOf>(cursor) {
            Some(parent) => cursor = parent.parent(),
            None => return false,
        }
    }
}

/// Toolbar predicate: the selection is a flat mesh, and the viewport is in
/// Scene mode.
///
/// The mode check mirrors `crate::terrain_exists_in_scene` — the mesh Edit and
/// Sculpt modes have their own toolset, and offering to replace the mesh you
/// are editing with a terrain reads as the wrong tool in the wrong place.
pub fn plane_selected(world: &World) -> bool {
    let scene_mode = world
        .get_resource::<ViewportSettings>()
        .map(|s| s.viewport_mode == ViewportMode::Scene)
        .unwrap_or(true);
    scene_mode && selected_plane(world).is_some()
}

fn selected_plane(world: &World) -> Option<Entity> {
    let entity = world.get_resource::<EditorSelection>()?.get()?;
    plane_footprint(world, entity).map(|_| entity)
}

// ── Grid sizing ─────────────────────────────────────────────────────────────

/// Pick the chunk grid that best covers a `footprint`-sized patch of ground.
///
/// The chunk size comes from the *shorter* side, and the longer side is then
/// filled with however many of those chunks it takes. Terrain chunks are
/// square and every chunk in a terrain shares one size, so driving the size
/// from the long side instead would round a 100 × 20 plane up to a 100 × 50
/// terrain — five times the ground the user pointed at.
///
/// Height range is left at [`TerrainData::default`]'s −10 → 40, which is the
/// range that puts a flat chunk's initial 0.2 normalized height at local y = 0:
/// the new terrain's surface lands exactly where the plane's surface was.
///
/// Note that a *spawned* terrain is parked a few centimetres above y = 0 to
/// keep it off the editor grid plane, which the two fight over. A conversion
/// deliberately does not do that — the terrain replaces a mesh the user has
/// already placed, and silently shifting their ground up to dodge a grid
/// artefact is worse than the artefact.
fn terrain_grid_for(footprint: Vec2) -> TerrainData {
    let short = footprint.min_element().max(0.01);
    let long = footprint.max_element().max(0.01);

    // Grow the chunk size until the grid fits in the chunk budget. Doubling
    // quarters the count, so this runs a handful of times at most.
    let mut chunk_size = (short / (short / TARGET_CHUNK_SIZE).round().max(1.0)).max(0.01);
    let (mut n_short, mut n_long);
    loop {
        n_short = (short / chunk_size).round().max(1.0) as u32;
        n_long = (long / chunk_size).round().max(1.0) as u32;
        if n_short * n_long <= MAX_CHUNKS {
            break;
        }
        chunk_size *= 2.0;
    }

    let (chunks_x, chunks_z) = if footprint.x >= footprint.y {
        (n_long, n_short)
    } else {
        (n_short, n_long)
    };

    let ideal = chunk_size / TARGET_VERTEX_SPACING + 1.0;
    let chunk_resolution = RESOLUTION_LADDER
        .iter()
        .copied()
        .find(|r| *r as f32 >= ideal)
        .unwrap_or(RESOLUTION_LADDER[RESOLUTION_LADDER.len() - 1]);

    TerrainData {
        chunks_x,
        chunks_z,
        chunk_size,
        chunk_resolution,
        ..TerrainData::default()
    }
}

// ── Conversion ──────────────────────────────────────────────────────────────

/// Replace `entity`'s flat mesh with a terrain chunk grid of the same
/// footprint. Returns `false` (leaving the world untouched) if it is not a
/// flat mesh.
fn convert(world: &mut World, entity: Entity) -> bool {
    let Some(footprint) = plane_footprint(world, entity) else {
        return false;
    };
    let terrain_data = terrain_grid_for(footprint);
    // A project material assigned to the plane carries over to the chunks; a
    // plane wearing only a plain `StandardMaterial` gets the terrain
    // checkerboard, the same look a freshly added terrain has.
    let material = world.get::<MaterialRef>(entity).cloned();
    let inherited = inherited_scale_rotation(world, entity);

    let mut em = world.entity_mut(entity);
    // `MeshPrimitive` and `EditedMesh` both rehydrate a `Mesh3d` for any entity
    // that has lost one (see `scene_io::rehydrate_meshes` /
    // `apply_edited_meshes`), so leaving either behind would grow the old plane
    // straight back through the middle of the new terrain.
    em.remove::<Mesh3d>();
    em.remove::<MeshPrimitive>();
    em.remove::<EditedMesh>();
    em.remove::<EditedMeshApplied>();
    em.insert((
        terrain_data.clone(),
        Painter::default(),
        renzora::SelectionStop,
    ));
    if let Some(mut transform) = em.get_mut::<Transform>() {
        // See the module docs: the brushes read this root's translation and
        // nothing else, so rotation and scale have to be spent here or every
        // stroke lands somewhere the cursor isn't. What's cancelled is the
        // *world* rotation and scale, not just the local ones — a plane
        // parented under a rotated or scaled group inherits both, and undoing
        // them locally is what leaves the terrain world-axis-aligned at the
        // size its footprint was measured in.
        let (parent_scale, parent_rotation) = inherited;
        transform.rotation = parent_rotation.inverse();
        transform.scale = Vec3::new(
            safe_recip(parent_scale.x),
            safe_recip(parent_scale.y),
            safe_recip(parent_scale.z),
        );
    }

    let chunks = renzora_terrain::mesh::spawn_terrain_chunks(world, entity, &terrain_data, material);
    console_info(
        "Terrain",
        format!(
            "Converted {:?} to terrain — {} × {} chunks of {:.1} m at resolution {}",
            entity,
            terrain_data.chunks_x,
            terrain_data.chunks_z,
            terrain_data.chunk_size,
            terrain_data.chunk_resolution
        ),
    );
    debug_assert_eq!(
        chunks.len() as u32,
        terrain_data.chunks_x * terrain_data.chunks_z
    );
    true
}

/// Undo step for [`convert`]: puts the plane's mesh back and clears the chunk
/// grid away.
///
/// Redo re-runs the conversion rather than restoring a captured terrain,
/// because undo has by then restored the exact mesh and transform the grid was
/// derived from, so it comes back identical. Sculpting done *after* the
/// conversion is not preserved across an undo of the conversion itself — the
/// strokes are on the chunks this step deletes.
struct ConvertPlaneCmd {
    entity: Entity,
    mesh: Mesh3d,
    primitive: Option<MeshPrimitive>,
    edited: Option<(EditedMesh, bool)>,
    transform: Transform,
    /// Whether the mesh already blocked child selection before it became a
    /// terrain, so undo doesn't strip a marker the conversion didn't add.
    selection_stop: bool,
}

impl ConvertPlaneCmd {
    /// Capture what `undo` will need to hand back, or `None` if `entity` isn't
    /// a flat mesh.
    fn capture(world: &World, entity: Entity) -> Option<Self> {
        plane_footprint(world, entity)?;
        Some(Self {
            entity,
            mesh: world.get::<Mesh3d>(entity)?.clone(),
            primitive: world.get::<MeshPrimitive>(entity).cloned(),
            edited: world
                .get::<EditedMesh>(entity)
                .cloned()
                .map(|m| (m, world.get::<EditedMeshApplied>(entity).is_some())),
            transform: world.get::<Transform>(entity).copied().unwrap_or_default(),
            selection_stop: world.get::<renzora::SelectionStop>(entity).is_some(),
        })
    }
}

impl renzora_undo::UndoCommand for ConvertPlaneCmd {
    fn label(&self) -> &str {
        "Make Terrain"
    }

    fn execute(&mut self, world: &mut World) {
        convert(world, self.entity);
    }

    fn undo(&mut self, world: &mut World) {
        // Every chunk child, not just the ones the conversion spawned: the
        // Resize Terrain tool can have added more since.
        let chunks: Vec<Entity> = world
            .get::<Children>(self.entity)
            .map(|children| {
                children
                    .iter()
                    .filter(|c| world.get::<TerrainChunkData>(*c).is_some())
                    .collect()
            })
            .unwrap_or_default();
        for chunk in chunks {
            if let Ok(chunk) = world.get_entity_mut(chunk) {
                chunk.despawn();
            }
        }

        let Ok(mut em) = world.get_entity_mut(self.entity) else {
            return;
        };
        em.remove::<TerrainData>();
        em.remove::<Painter>();
        if !self.selection_stop {
            em.remove::<renzora::SelectionStop>();
        }
        em.insert((self.mesh.clone(), self.transform));
        if let Some(primitive) = self.primitive.clone() {
            em.insert(primitive);
        }
        if let Some((edited, applied)) = self.edited.clone() {
            em.insert(edited);
            if applied {
                em.insert(EditedMeshApplied);
            }
        }
    }
}

/// Toolbar activator for **Make Terrain**: convert the selected plane and drop
/// straight into the sculpt brush on it.
///
/// Going straight to sculpting is the point of the button — the reason to
/// convert a plane is to start shaping it, and the terrain strip's own Sculpt
/// button has just appeared beside this one for the same terrain.
pub fn convert_selected(world: &mut World) {
    let Some(entity) = selected_plane(world) else {
        return;
    };
    let Some(cmd) = ConvertPlaneCmd::capture(world, entity) else {
        return;
    };
    renzora_undo::execute(world, renzora_undo::UndoContext::Scene, Box::new(cmd));
    renzora_undo::seal(world, &renzora_undo::UndoContext::Scene);

    if world.get::<TerrainData>(entity).is_some() {
        if let Some(selection) = world.get_resource::<EditorSelection>() {
            selection.set(Some(entity));
        }
        world.insert_resource(TerrainInspectorTab::Sculpt);
        world.insert_resource(ActiveTool::TerrainSculpt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_icon_resolves_and_is_its_own() {
        // A typo here ships as the literal name crammed into a 28px button.
        assert!(renzora_ember::font::icon_glyph(TOOL_ICON).is_some());
        // Sharing an icon with a tool that can be on screen at the same time
        // is the failure this button was given its own icon to avoid.
        assert_ne!(TOOL_ICON, crate::generate_tool::TOOL_ICON);
        assert_ne!(TOOL_ICON, "mountains");
    }

    #[test]
    fn square_plane_becomes_one_chunk_of_its_own_size() {
        let grid = terrain_grid_for(Vec2::splat(20.0));
        assert_eq!((grid.chunks_x, grid.chunks_z), (1, 1));
        assert!((grid.chunk_size - 20.0).abs() < 1e-3);
    }

    #[test]
    fn oblong_plane_tiles_the_long_side() {
        // 100 × 20 must stay 100 × 20, not round out to 100 × 50.
        let grid = terrain_grid_for(Vec2::new(100.0, 20.0));
        assert!((grid.chunk_size - 20.0).abs() < 1e-3);
        assert_eq!((grid.chunks_x, grid.chunks_z), (5, 1));

        // …and the same plane laid the other way round.
        let grid = terrain_grid_for(Vec2::new(20.0, 100.0));
        assert_eq!((grid.chunks_x, grid.chunks_z), (1, 5));
    }

    #[test]
    fn resolution_follows_chunk_size() {
        // A small plane must not get 1.5 cm triangles.
        assert_eq!(terrain_grid_for(Vec2::splat(2.0)).chunk_resolution, 33);
        assert_eq!(terrain_grid_for(Vec2::splat(64.0)).chunk_resolution, 129);
    }

    #[test]
    fn huge_plane_stays_within_the_chunk_budget() {
        let grid = terrain_grid_for(Vec2::splat(5000.0));
        assert!(grid.chunks_x * grid.chunks_z <= MAX_CHUNKS);
        // Still covers the ground it was asked to.
        assert!(grid.total_width() >= 5000.0 * 0.5);
    }
}
