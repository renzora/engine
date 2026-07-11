//! Terrain mesh generation — heightmap to triangle mesh with normals via central differences.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use renzora::console_log::console_info;
use std::collections::HashMap;

use renzora_physics::{CollisionShapeData, PhysicsBodyData};

use crate::data::{TerrainChunkData, TerrainChunkOf, TerrainData};
use crate::material::TerrainCheckerboardMaterial;
use renzora::MaterialRef;

/// Depth of the flat floor plate appended under each chunk, in chunk-local
/// units below `min_height`. Small gap avoids Z-fighting with terrain that's
/// been sculpted all the way down to `min_height`.
const TERRAIN_FLOOR_GAP: f32 = 0.5;

/// Append a perimeter-wall strip to the mesh. `edge_top_indices` lists the
/// surface vertex indices along one chunk edge in CCW order (interior on
/// left); `outward` is the wall's outward-facing normal. For each segment we
/// emit two new verts (a wall-normal copy of the top vert and a floor-level
/// vert) and the two triangles bridging this pair to the previous pair.
fn add_perimeter_wall(
    positions: &mut Vec<Vec3>,
    normals: &mut Vec<Vec3>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    edge_top_indices: &[u32],
    floor_y: f32,
    outward: Vec3,
) {
    let strip_len = edge_top_indices.len();
    if strip_len < 2 {
        return;
    }
    let base = positions.len() as u32;
    for (i, &src) in edge_top_indices.iter().enumerate() {
        // Read before we push so the borrow doesn't outlive itself.
        let p = positions[src as usize];
        let u = i as f32 / (strip_len - 1) as f32;
        positions.push(p);
        normals.push(outward);
        uvs.push([u, 1.0]);
        positions.push(Vec3::new(p.x, floor_y, p.z));
        normals.push(outward);
        uvs.push([u, 0.0]);
    }
    for i in 0..(strip_len as u32 - 1) {
        let t0 = base + 2 * i;
        let b0 = t0 + 1;
        let t1 = base + 2 * (i + 1);
        let b1 = t1 + 1;
        // CCW from outside: (T0, T1, B1) + (T0, B1, B0). Works for all four
        // walls because each `edge_top_indices` is walked CCW around the chunk.
        indices.push(t0);
        indices.push(t1);
        indices.push(b1);
        indices.push(t0);
        indices.push(b1);
        indices.push(b0);
    }
}

/// Generate a triangle mesh for a single terrain chunk from its heightmap.
///
/// Output mesh is the heightmap surface on top, a flat floor plate underneath
/// at `min_height - TERRAIN_FLOOR_GAP`, and vertical side walls on every
/// edge that faces open space (no neighbouring chunk). Interior boundaries
/// between adjacent chunks deliberately get no wall so the chunks meet flush.
/// Closed solid means players can't fall through holes or off the edges.
pub fn generate_chunk_mesh(terrain: &TerrainData, chunk: &TerrainChunkData) -> Mesh {
    let resolution = terrain.chunk_resolution;
    let spacing = terrain.vertex_spacing();
    let height_range = terrain.height_range();

    let surface_vert_count = (resolution * resolution) as usize;
    // Hint: surface verts + floor 4 + worst-case 4 walls × 2 × resolution.
    let total_vert_hint = surface_vert_count + 4 + 8 * resolution as usize;
    let mut positions = Vec::with_capacity(total_vert_hint);
    // Normals for the surface get computed by central differences below; size
    // the buffer to match `positions` so indices line up, then push for floor
    // and walls.
    let mut normals = vec![Vec3::Y; surface_vert_count];
    let mut uvs = Vec::with_capacity(total_vert_hint);

    // Generate vertices
    for z in 0..resolution {
        for x in 0..resolution {
            let h = terrain.min_height + chunk.get_height(x, z, resolution) * height_range;
            positions.push(Vec3::new(x as f32 * spacing, h, z as f32 * spacing));
            uvs.push([
                x as f32 / (resolution - 1) as f32,
                z as f32 / (resolution - 1) as f32,
            ]);
        }
    }

    // Calculate normals using central differences
    for z in 0..resolution {
        for x in 0..resolution {
            let h_left = chunk.get_height(x.saturating_sub(1), z, resolution);
            let h_right = chunk.get_height((x + 1).min(resolution - 1), z, resolution);
            let h_down = chunk.get_height(x, z.saturating_sub(1), resolution);
            let h_up = chunk.get_height(x, (z + 1).min(resolution - 1), resolution);

            let hl = terrain.min_height + h_left * height_range;
            let hr = terrain.min_height + h_right * height_range;
            let hd = terrain.min_height + h_down * height_range;
            let hu = terrain.min_height + h_up * height_range;

            let dx = (hr - hl) / (2.0 * spacing);
            let dz = (hu - hd) / (2.0 * spacing);

            normals[(z * resolution + x) as usize] = Vec3::new(-dx, 1.0, -dz).normalize();
        }
    }

    // Generate indices (two triangles per quad)
    let quad_count = ((resolution - 1) * (resolution - 1)) as usize;
    // +6 indices for the floor plate (two triangles)
    let mut indices = Vec::with_capacity(quad_count * 6 + 6);

    for z in 0..(resolution - 1) {
        for x in 0..(resolution - 1) {
            let tl = z * resolution + x;
            let tr = tl + 1;
            let bl = tl + resolution;
            let br = bl + 1;

            indices.push(tl);
            indices.push(bl);
            indices.push(tr);

            indices.push(tr);
            indices.push(bl);
            indices.push(br);
        }
    }

    // Floor plate: 4 corner verts at the chunk's lowest sculpted height minus
    // a small gap, facing up so anyone falling through a hole sees the
    // basement floor rather than its back side. Tracking the chunk's actual
    // minimum (rather than `terrain.min_height`) keeps the side walls thin on
    // flat areas and only drops them where the user has carved down — saved
    // scenes don't get a 10-unit slab hanging under default-flat terrain.
    // `heights` is the composed buffer (base + layers). It's `#[serde(skip)]`,
    // so on rehydrate it can briefly be empty before the composition system
    // populates it — fall back to `base_heights` so the floor stays sane.
    let height_source = if chunk.heights.is_empty() {
        &chunk.base_heights
    } else {
        &chunk.heights
    };
    let chunk_min_h_norm = height_source
        .iter()
        .copied()
        .fold(f32::INFINITY, f32::min)
        .clamp(0.0, 1.0);
    let chunk_min_world_y = terrain.min_height + chunk_min_h_norm * height_range;
    let floor_y = chunk_min_world_y - TERRAIN_FLOOR_GAP;
    let chunk_extent = (resolution - 1) as f32 * spacing;
    let floor_base = positions.len() as u32;
    let floor_corners = [
        Vec3::new(0.0, floor_y, 0.0),
        Vec3::new(chunk_extent, floor_y, 0.0),
        Vec3::new(chunk_extent, floor_y, chunk_extent),
        Vec3::new(0.0, floor_y, chunk_extent),
    ];
    let floor_uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    for (corner, uv) in floor_corners.iter().zip(floor_uvs.iter()) {
        positions.push(*corner);
        normals.push(Vec3::Y);
        uvs.push(*uv);
    }
    // Winding chosen so the surface normal points +Y (visible from above).
    indices.push(floor_base);
    indices.push(floor_base + 3);
    indices.push(floor_base + 2);
    indices.push(floor_base);
    indices.push(floor_base + 2);
    indices.push(floor_base + 1);

    // Side walls along external perimeter edges only. Internal edges between
    // neighbouring chunks would double-wall; suppress them. Each wall is
    // walked CCW around the chunk (interior on the left) so the standardized
    // quad winding in `add_perimeter_wall` faces outward.
    let is_external_south = chunk.chunk_z == 0;
    let is_external_east = chunk.chunk_x + 1 == terrain.chunks_x;
    let is_external_north = chunk.chunk_z + 1 == terrain.chunks_z;
    let is_external_west = chunk.chunk_x == 0;

    if is_external_south {
        // z=0, walk +X.
        let strip: Vec<u32> = (0..resolution).collect();
        add_perimeter_wall(
            &mut positions,
            &mut normals,
            &mut uvs,
            &mut indices,
            &strip,
            floor_y,
            Vec3::NEG_Z,
        );
    }
    if is_external_east {
        // x=resolution-1, walk +Z.
        let strip: Vec<u32> = (0..resolution)
            .map(|z| z * resolution + (resolution - 1))
            .collect();
        add_perimeter_wall(
            &mut positions,
            &mut normals,
            &mut uvs,
            &mut indices,
            &strip,
            floor_y,
            Vec3::X,
        );
    }
    if is_external_north {
        // z=resolution-1, walk -X.
        let strip: Vec<u32> = (0..resolution)
            .rev()
            .map(|x| (resolution - 1) * resolution + x)
            .collect();
        add_perimeter_wall(
            &mut positions,
            &mut normals,
            &mut uvs,
            &mut indices,
            &strip,
            floor_y,
            Vec3::Z,
        );
    }
    if is_external_west {
        // x=0, walk -Z.
        let strip: Vec<u32> = (0..resolution).rev().map(|z| z * resolution).collect();
        add_perimeter_wall(
            &mut positions,
            &mut normals,
            &mut uvs,
            &mut indices,
            &strip,
            floor_y,
            Vec3::NEG_X,
        );
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        positions
            .iter()
            .map(|p| [p.x, p.y, p.z])
            .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        normals.iter().map(|n| [n.x, n.y, n.z]).collect::<Vec<_>>(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));

    mesh
}

/// Spawn a complete terrain entity with chunk children.
///
/// Returns the root terrain entity. Each chunk is spawned as a child with
/// the checkerboard material and a trimesh collider.
pub fn spawn_terrain(world: &mut World) -> Entity {
    // Start with a single tile; users grow the terrain via the Size tab's
    // Add Neighbor buttons (Unity-style). `TerrainData::default()` is kept at
    // 4×4 so saved scenes load unchanged.
    let terrain_data = TerrainData {
        chunks_x: 1,
        chunks_z: 1,
        ..TerrainData::default()
    };

    let material = {
        let mut mats = world.resource_mut::<Assets<TerrainCheckerboardMaterial>>();
        mats.add(TerrainCheckerboardMaterial::default())
    };

    // Build chunk data + meshes
    let mut chunks: Vec<(TerrainChunkData, Handle<Mesh>, Vec3)> = Vec::new();
    {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        for cz in 0..terrain_data.chunks_z {
            for cx in 0..terrain_data.chunks_x {
                let chunk = TerrainChunkData::new(cx, cz, terrain_data.chunk_resolution, 0.2);
                let mesh = generate_chunk_mesh(&terrain_data, &chunk);
                let mesh_handle = meshes.add(mesh);
                let origin = terrain_data.chunk_world_origin(cx, cz);
                chunks.push((chunk, mesh_handle, origin));
            }
        }
    }

    // Parent at origin — defaults are tuned so the initial flat heightmap
    // (20% × range-50 + min=-10 = 0) lands on the editor grid plane.
    let terrain_entity = world
        .spawn((
            Name::new("Terrain"),
            Transform::IDENTITY,
            Visibility::default(),
            terrain_data,
            // Paintable from frame one (`ensure_painter_system` covers
            // scene-loaded terrains, but only on the next Update).
            crate::painter::Painter::default(),
            renzora::SelectionStop,
        ))
        .id();

    console_info(
        "Terrain",
        format!("Spawning terrain with {} chunks", chunks.len()),
    );

    for (mut chunk_data, mesh_handle, origin) in chunks {
        chunk_data.dirty = false;
        let cx = chunk_data.chunk_x;
        let cz = chunk_data.chunk_z;
        let chunk_entity = world
            .spawn((
                Name::new(format!("Chunk ({},{})", cx, cz)),
                Mesh3d(mesh_handle),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(origin),
                PhysicsBodyData::static_body(),
                CollisionShapeData::mesh(),
                chunk_data,
                TerrainChunkOf(terrain_entity),
            ))
            .id();
        // Insert ChildOf separately to trigger Bevy's hierarchy hooks
        // (on_insert hooks don't fire when ChildOf is part of a spawn bundle)
        world
            .entity_mut(chunk_entity)
            .insert(ChildOf(terrain_entity));
    }

    terrain_entity
}

/// Rehydrate terrain chunks after scene load — spawns mesh, material, collider,
/// and `TerrainChunkOf` for chunks that have `TerrainChunkData` but no `Mesh3d`.
pub fn rehydrate_terrain_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TerrainCheckerboardMaterial>>,
    terrain_query: Query<&TerrainData>,
    chunk_query: Query<
        (
            Entity,
            &TerrainChunkData,
            Option<&TerrainChunkOf>,
            Option<&ChildOf>,
            Option<&MaterialRef>,
        ),
        // `TerrainChunkStreamedOut` chunks are meshless on purpose — the
        // streaming system removed the marker's mesh and will hand the chunk
        // back to this system by removing the marker (see `streaming.rs`).
        (Without<Mesh3d>, Without<super::data::TerrainChunkStreamedOut>),
    >,
) {
    if chunk_query.is_empty() {
        return;
    }

    let material = materials.add(TerrainCheckerboardMaterial::default());

    for (entity, chunk_data, chunk_of, child_of, mat_ref) in chunk_query.iter() {
        // Resolve parent terrain: prefer TerrainChunkOf, fall back to ChildOf parent
        let parent = chunk_of.map(|c| c.0).or_else(|| child_of.map(|c| c.0));
        let Some(parent_entity) = parent else {
            continue;
        };
        let Ok(terrain_data) = terrain_query.get(parent_entity) else {
            continue;
        };

        // Ensure parent has required hierarchy components (scene loader may omit them)
        commands
            .entity(parent_entity)
            .insert((GlobalTransform::default(), InheritedVisibility::default()));

        let mesh = generate_chunk_mesh(terrain_data, chunk_data);
        let mesh_handle = meshes.add(mesh);

        let mut ec = commands.entity(entity);
        ec.insert((
            Mesh3d(mesh_handle),
            PhysicsBodyData::static_body(),
            CollisionShapeData::mesh(),
        ));

        // Only apply default checkerboard if chunk has no custom material assigned
        if mat_ref.is_none() {
            ec.insert(MeshMaterial3d(material.clone()));
        }

        // Restore TerrainChunkOf if missing (scene load doesn't serialize it)
        if chunk_of.is_none() {
            ec.insert(TerrainChunkOf(parent_entity));
        }
    }
}

/// Recover terrains whose chunk children are missing entirely. Scenes saved
/// while the grid-regrow path spawned nameless chunks lost every chunk on
/// save (the scene saver only serializes named entities), so they load back
/// as a bare `TerrainData` root that renders nothing and can't be sculpted.
/// The heightmap data in those files is gone; spawning a fresh flat grid at
/// the authored dimensions at least brings the terrain back to a usable
/// state. Safe to run every frame: scene loads apply atomically (single
/// `write_to_world`), and the regrow path despawns + respawns within one
/// command batch, so a terrain is never legitimately chunkless.
pub fn backfill_missing_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TerrainCheckerboardMaterial>>,
    terrains: Query<(Entity, &TerrainData)>,
    chunks: Query<(Option<&TerrainChunkOf>, Option<&ChildOf>), With<TerrainChunkData>>,
) {
    if terrains.is_empty() {
        return;
    }

    let mut parents = std::collections::HashSet::new();
    for (chunk_of, child_of) in chunks.iter() {
        if let Some(parent) = chunk_of.map(|c| c.0).or_else(|| child_of.map(|c| c.parent())) {
            parents.insert(parent);
        }
    }

    for (terrain_entity, terrain_data) in terrains.iter() {
        if parents.contains(&terrain_entity) {
            continue;
        }
        console_info(
            "Terrain",
            format!(
                "Terrain {:?} has no chunks — regenerating a flat {}x{} grid",
                terrain_entity, terrain_data.chunks_x, terrain_data.chunks_z
            ),
        );
        let material = materials.add(TerrainCheckerboardMaterial::default());
        for cz in 0..terrain_data.chunks_z {
            for cx in 0..terrain_data.chunks_x {
                let mut chunk_data =
                    TerrainChunkData::new(cx, cz, terrain_data.chunk_resolution, 0.2);
                let mesh = generate_chunk_mesh(terrain_data, &chunk_data);
                chunk_data.dirty = false;
                let mesh_handle = meshes.add(mesh);
                let origin = terrain_data.chunk_world_origin(cx, cz);
                commands.entity(terrain_entity).with_child((
                    Name::new(format!("Chunk ({},{})", cx, cz)),
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(material.clone()),
                    Transform::from_translation(origin),
                    Visibility::default(),
                    PhysicsBodyData::static_body(),
                    CollisionShapeData::mesh(),
                    chunk_data,
                    TerrainChunkOf(terrain_entity),
                ));
            }
        }
    }
}

/// Marker: this chunk's mesh changed but its collider rebuild is pending.
/// Trimesh construction over a full chunk (~33k triangles) is far too slow to
/// run once per drag frame — in a debug build it stalls sculpting by seconds —
/// so the rebuild is debounced by [`flush_stale_colliders_system`].
#[derive(Component)]
pub struct ColliderStale {
    /// `Time::elapsed_secs()` of the most recent mesh change.
    pub last_change: f32,
}

/// How long a chunk must sit unchanged before its collider rebuilds.
const COLLIDER_DEBOUNCE_SECS: f32 = 0.25;

/// System that regenerates meshes for dirty chunks. Colliders are only
/// marked stale here; the debounced flush below rebuilds them.
pub fn terrain_chunk_mesh_update_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    time: Res<Time>,
    terrain_query: Query<&TerrainData>,
    mut chunk_query: Query<(Entity, &mut TerrainChunkData, &TerrainChunkOf, &Mesh3d)>,
) {
    for (entity, mut chunk, chunk_of, mesh_handle) in chunk_query.iter_mut() {
        // Keyed on `mesh_stale` (set by composition), NOT `dirty` (set by
        // writers) — see the flag docs on `TerrainChunkData`.
        if !chunk.mesh_stale {
            continue;
        }
        let Ok(terrain) = terrain_query.get(chunk_of.0) else {
            continue;
        };
        let new_mesh = generate_chunk_mesh(terrain, &chunk);
        if let Some(mut mesh) = meshes.get_mut(&mesh_handle.0) {
            *mesh = new_mesh;
        }
        commands.entity(entity).try_insert(ColliderStale {
            last_change: time.elapsed_secs(),
        });
        chunk.mesh_stale = false;
    }
}

/// Rebuild the collider of any chunk whose mesh has been stable for
/// [`COLLIDER_DEBOUNCE_SECS`] — and never mid-stroke, so a held brush costs
/// only mesh regen per frame. Re-inserting triggers
/// `Changed<CollisionShapeData>`, which makes the physics layer rebuild the
/// avian trimesh from the updated Mesh asset.
pub fn flush_stale_colliders_system(
    mut commands: Commands,
    time: Res<Time>,
    sculpt_state: Res<crate::data::TerrainSculptState>,
    stale: Query<(Entity, &ColliderStale)>,
) {
    if sculpt_state.is_sculpting {
        return;
    }
    let now = time.elapsed_secs();
    for (entity, marker) in stale.iter() {
        if now - marker.last_change < COLLIDER_DEBOUNCE_SECS {
            continue;
        }
        commands
            .entity(entity)
            .remove::<ColliderStale>()
            .try_insert(CollisionShapeData::mesh());
    }
}

/// Bilinear resampling of heightmap from one resolution to another.
fn resample_heights(old_heights: &[f32], old_res: u32, new_res: u32) -> Vec<f32> {
    let new_size = (new_res * new_res) as usize;
    let mut out = vec![0.2f32; new_size];

    if old_heights.is_empty() || old_res < 2 || new_res < 2 {
        return out;
    }

    for nz in 0..new_res {
        for nx in 0..new_res {
            let fx = nx as f32 / (new_res - 1) as f32 * (old_res - 1) as f32;
            let fz = nz as f32 / (new_res - 1) as f32 * (old_res - 1) as f32;

            let ox0 = (fx.floor() as u32).min(old_res - 1);
            let oz0 = (fz.floor() as u32).min(old_res - 1);
            let ox1 = (ox0 + 1).min(old_res - 1);
            let oz1 = (oz0 + 1).min(old_res - 1);

            let tx = fx.fract();
            let tz = fz.fract();

            let get = |x: u32, z: u32| {
                old_heights
                    .get((z * old_res + x) as usize)
                    .copied()
                    .unwrap_or(0.2)
            };

            let h = get(ox0, oz0) * (1.0 - tx) * (1.0 - tz)
                + get(ox1, oz0) * tx * (1.0 - tz)
                + get(ox0, oz1) * (1.0 - tx) * tz
                + get(ox1, oz1) * tx * tz;

            out[(nz * new_res + nx) as usize] = h;
        }
    }

    out
}

/// System that rebuilds terrain chunks when `TerrainData` changes in the inspector.
///
/// Handles chunk grid changes, resolution changes, height range changes.
/// Resamples heightmaps via bilinear interpolation when resolution changes.
pub fn terrain_data_changed_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut terrain_materials: ResMut<Assets<TerrainCheckerboardMaterial>>,
    changed_terrain: Query<(Entity, &TerrainData), Changed<TerrainData>>,
    added: Query<Entity, Added<TerrainData>>,
    mut chunk_query: Query<(Entity, &TerrainChunkOf, &mut TerrainChunkData)>,
) {
    for (terrain_entity, terrain_data) in changed_terrain.iter() {
        // Skip first insertion — chunks are spawned by the terrain creation command
        if added.contains(terrain_entity) {
            continue;
        }

        // Fast path: grid and resolution unchanged (e.g. a min/max height
        // drag from the inspector). Re-flag the chunks dirty so their meshes
        // rebuild in place — despawn/respawn of every chunk entity per drag
        // tick would churn the hierarchy and the physics layer.
        {
            let expected =
                (terrain_data.chunks_x * terrain_data.chunks_z) as usize;
            let expected_len =
                (terrain_data.chunk_resolution * terrain_data.chunk_resolution) as usize;
            let mut matching = 0usize;
            for (_, of, chunk) in chunk_query.iter() {
                if of.0 != terrain_entity {
                    continue;
                }
                if chunk.base_heights.len() != expected_len
                    || chunk.chunk_x >= terrain_data.chunks_x
                    || chunk.chunk_z >= terrain_data.chunks_z
                {
                    matching = usize::MAX;
                    break;
                }
                matching += 1;
            }
            if matching == expected {
                for (entity, of, mut chunk) in chunk_query.iter_mut() {
                    if of.0 == terrain_entity {
                        chunk.dirty = true;
                        // A chunk_size edit moves the chunk origins as well
                        // as the vertices; keep the child transforms honest.
                        let origin =
                            terrain_data.chunk_world_origin(chunk.chunk_x, chunk.chunk_z);
                        commands
                            .entity(entity)
                            .try_insert(Transform::from_translation(origin));
                    }
                }
                continue;
            }
        }

        // Collect existing chunks for this terrain
        let existing: Vec<(Entity, u32, u32, Vec<f32>)> = chunk_query
            .iter()
            .filter(|(_, of, _)| of.0 == terrain_entity)
            .map(|(e, _, data)| (e, data.chunk_x, data.chunk_z, data.base_heights.clone()))
            .collect();

        if existing.is_empty() {
            continue;
        }

        // Detect old resolution from existing chunk heightmap size
        let old_res = {
            let len = existing[0].3.len();
            (len as f32).sqrt().round() as u32
        };

        // Build lookup: (cx, cz) -> old heights
        let old_map: HashMap<(u32, u32), Vec<f32>> = existing
            .iter()
            .map(|(_, cx, cz, h)| ((*cx, *cz), h.clone()))
            .collect();

        // Despawn all old chunk entities
        for (chunk_entity, _, _, _) in &existing {
            commands.entity(*chunk_entity).despawn();
        }

        let material = terrain_materials.add(TerrainCheckerboardMaterial::default());

        let new_res = terrain_data.chunk_resolution;

        // Spawn new chunks with resampled or fresh heights
        for cz in 0..terrain_data.chunks_z {
            for cx in 0..terrain_data.chunks_x {
                let base_heights = if let Some(old_h) = old_map.get(&(cx, cz)) {
                    if old_res != new_res {
                        resample_heights(old_h, old_res, new_res)
                    } else {
                        old_h.clone()
                    }
                } else {
                    vec![0.2f32; (new_res * new_res) as usize]
                };

                let chunk_data = TerrainChunkData {
                    chunk_x: cx,
                    chunk_z: cz,
                    heights: base_heights.clone(),
                    base_heights,
                    dirty: false,
                    mesh_stale: false,
                };

                let mesh = generate_chunk_mesh(terrain_data, &chunk_data);
                let mesh_handle = meshes.add(mesh);
                let origin = terrain_data.chunk_world_origin(cx, cz);

                commands.entity(terrain_entity).with_child((
                    // The scene saver only serializes named entities — a
                    // nameless chunk silently vanishes from the save and the
                    // terrain loads back as an empty root.
                    Name::new(format!("Chunk ({},{})", cx, cz)),
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(material.clone()),
                    Transform::from_translation(origin),
                    Visibility::default(),
                    PhysicsBodyData::static_body(),
                    CollisionShapeData::mesh(),
                    chunk_data,
                    TerrainChunkOf(terrain_entity),
                ));
            }
        }
    }
}
