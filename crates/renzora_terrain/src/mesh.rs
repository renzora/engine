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

/// How far above y=0 a freshly added terrain is parked.
///
/// A new terrain's flat heightmap lands exactly on y=0 (20% × range-50 +
/// min=-10), which is also where the editor grid sits. The grid draws in the
/// transparent pass with `depth_compare: Greater` and no depth write, so a
/// coplanar surface leaves the comparison on a knife edge and the grid breaks up
/// into shimmering patches as the camera moves. Lifting the terrain a hair puts
/// it strictly in front, so it occludes the grid cleanly instead of fighting it.
///
/// 5 cm against a 50-unit height range is invisible while you sculpt, but big
/// enough to stay separated at the depth precision you get zoomed out over a
/// grown, many-chunk terrain. It only applies to the initial spawn — the value
/// is saved with the scene, and moving the terrain yourself overrides it.
const TERRAIN_GRID_CLEARANCE: f32 = 0.05;

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
    generate_chunk_lod_mesh(terrain, chunk, 1)
}

/// The chunks beside one, for the heights just past its edges.
///
/// A vertex normal is taken from the heights on either side of it, and at a
/// chunk's edge one side belongs to the next chunk. Without it the edge took a
/// one-sided slope, each chunk a different one, so the two chunks lit their
/// shared edge differently and every seam showed as a line on a hillside.
#[derive(Default, Clone, Copy)]
pub struct ChunkNeighbours<'a> {
    pub west: Option<&'a TerrainChunkData>,
    pub east: Option<&'a TerrainChunkData>,
    pub south: Option<&'a TerrainChunkData>,
    pub north: Option<&'a TerrainChunkData>,
}

impl<'a> ChunkNeighbours<'a> {
    /// The neighbours of the chunk at `(x, z)`, from a lookup by grid cell.
    pub fn around(x: u32, z: u32, find: impl Fn(u32, u32) -> Option<&'a TerrainChunkData>) -> Self {
        Self {
            west: x.checked_sub(1).and_then(|w| find(w, z)),
            east: find(x + 1, z),
            south: z.checked_sub(1).and_then(|s| find(x, s)),
            north: find(x, z + 1),
        }
    }
}

/// [`generate_chunk_mesh`] from every `step`th height sample: a coarser
/// version of the chunk for drawing at a distance (see `lod`). `step` must
/// divide `chunk_resolution - 1`.
///
/// Normals still come from the full-detail heights at each kept vertex, so a
/// coarse chunk lights like the fine one and switching between them changes
/// the silhouette, not the shading.
///
/// A coarse chunk keeps every full-detail vertex along its border, and its
/// outer ring of cells fans into them from a vertex at each cell's centre.
/// Every chunk therefore has the same border vertices whatever level it and
/// its neighbours are drawing, so the seams are closed exactly, on any ground.
/// Skirts were tried first and could not do this: a skirt only hangs down, so
/// it filled the slit where a coarse edge rose above its neighbour but not the
/// one where it dipped below, and eroded ground opened pinholes along seams.
pub fn generate_chunk_lod_mesh(terrain: &TerrainData, chunk: &TerrainChunkData, step: u32) -> Mesh {
    generate_chunk_lod_mesh_beside(terrain, chunk, step, ChunkNeighbours::default())
}

/// [`generate_chunk_lod_mesh`] with the chunks around it, so the normals along
/// its edges match theirs (see [`ChunkNeighbours`]).
pub fn generate_chunk_lod_mesh_beside(
    terrain: &TerrainData,
    chunk: &TerrainChunkData,
    step: u32,
    neighbours: ChunkNeighbours,
) -> Mesh {
    let full_resolution = terrain.chunk_resolution;
    let step = step.max(1);
    let resolution = (full_resolution - 1) / step + 1;
    let full_spacing = terrain.vertex_spacing();
    let height_range = terrain.height_range();
    let last = full_resolution - 1;
    let sample = |x: u32, z: u32| chunk.get_height(x, z, full_resolution);
    let world_height = |x: u32, z: u32| terrain.min_height + sample(x, z) * height_range;
    // A full-detail sample one step past the chunk's edge, read from the
    // neighbour there (whose edge is shared, so one in from it), or clamped
    // to this chunk's own edge at the outside of the terrain. Only one of x
    // and z is ever out of range: the differences below step along one axis.
    let around = |x: i32, z: i32| -> f32 {
        let (l, r) = (last as i32, full_resolution);
        let out = match (x, z) {
            (-1, z) => neighbours.west.map(|n| n.get_height(last - 1, z as u32, r)),
            (x, z) if x > l => neighbours.east.map(|n| n.get_height(1, z as u32, r)),
            (x, -1) => neighbours.south.map(|n| n.get_height(x as u32, last - 1, r)),
            (x, z) if z > l => neighbours.north.map(|n| n.get_height(x as u32, 1, r)),
            _ => None,
        };
        let h = out.unwrap_or_else(|| sample(x.clamp(0, l) as u32, z.clamp(0, l) as u32));
        terrain.min_height + h * height_range
    };

    let surface_vert_count = (resolution * resolution) as usize;
    // Hint: surface verts + border verts + ring centres + floor 4 + walls.
    let total_vert_hint = surface_vert_count + 12 * full_resolution as usize + 4;
    let mut positions = Vec::with_capacity(total_vert_hint);
    let mut normals = Vec::with_capacity(total_vert_hint);
    let mut uvs = Vec::with_capacity(total_vert_hint);

    // One surface vertex at a full-detail sample. Positions come from the
    // sample's own index, never from a coarse index times a coarse spacing, so
    // a border vertex lands on exactly the same float in every level. Normals
    // are central differences on the full-detail grid, so a coarse chunk
    // lights like the fine one and a level switch changes only the silhouette.
    let push = |positions: &mut Vec<Vec3>, normals: &mut Vec<Vec3>, uvs: &mut Vec<[f32; 2]>, fx: u32, fz: u32| {
        let index = positions.len() as u32;
        positions.push(Vec3::new(fx as f32 * full_spacing, world_height(fx, fz), fz as f32 * full_spacing));
        let (x, z) = (fx as i32, fz as i32);
        let dx = (around(x + 1, z) - around(x - 1, z)) / (2.0 * full_spacing);
        let dz = (around(x, z + 1) - around(x, z - 1)) / (2.0 * full_spacing);
        normals.push(Vec3::new(-dx, 1.0, -dz).normalize());
        uvs.push([fx as f32 / last as f32, fz as f32 / last as f32]);
        index
    };

    for z in 0..resolution {
        for x in 0..resolution {
            push(&mut positions, &mut normals, &mut uvs, x * step, z * step);
        }
    }

    // The four borders at full detail, indexed by full-detail sample along
    // the edge. Samples a coarse vertex already covers reuse it.
    let mut border = |coarse: &dyn Fn(u32) -> u32, at: &dyn Fn(u32) -> (u32, u32)| -> Vec<u32> {
        (0..full_resolution)
            .map(|k| {
                if k % step == 0 {
                    coarse(k / step)
                } else {
                    let (fx, fz) = at(k);
                    push(&mut positions, &mut normals, &mut uvs, fx, fz)
                }
            })
            .collect()
    };
    let r = resolution;
    let south = border(&|i| i, &|k| (k, 0));
    let north = border(&|i| (r - 1) * r + i, &|k| (k, last));
    let west = border(&|i| i * r, &|k| (0, k));
    let east = border(&|i| i * r + r - 1, &|k| (last, k));

    // Indices: two triangles per inner cell, a fan per border cell.
    let quad_count = ((resolution - 1) * (resolution - 1)) as usize;
    // +6 indices for the floor plate (two triangles)
    let mut indices = Vec::with_capacity(quad_count * 6 + 6);
    let mut ring: Vec<u32> = Vec::with_capacity(4 * step as usize + 4);

    for z in 0..(resolution - 1) {
        for x in 0..(resolution - 1) {
            let tl = z * resolution + x;
            let tr = tl + 1;
            let bl = tl + resolution;
            let br = bl + 1;

            let on_border = x == 0 || z == 0 || x + 2 == resolution || z + 2 == resolution;
            if step == 1 || !on_border {
                indices.push(tl);
                indices.push(bl);
                indices.push(tr);

                indices.push(tr);
                indices.push(bl);
                indices.push(br);
                continue;
            }

            // Walk the cell tl, bl, br, tr (the winding of the quads above),
            // taking every full-detail vertex on a side that lies on the
            // chunk's border. Each side contributes its start, not its end.
            ring.clear();
            let (x0, z0) = (x * step, z * step);
            if x == 0 {
                ring.extend((z0..z0 + step).map(|k| west[k as usize]));
            } else {
                ring.push(tl);
            }
            if z + 2 == resolution {
                ring.extend((x0..x0 + step).map(|k| north[k as usize]));
            } else {
                ring.push(bl);
            }
            if x + 2 == resolution {
                ring.extend((z0 + 1..=z0 + step).rev().map(|k| east[k as usize]));
            } else {
                ring.push(br);
            }
            if z == 0 {
                ring.extend((x0 + 1..=x0 + step).rev().map(|k| south[k as usize]));
            } else {
                ring.push(tr);
            }
            let centre = push(&mut positions, &mut normals, &mut uvs, x0 + step / 2, z0 + step / 2);
            for i in 0..ring.len() {
                indices.push(centre);
                indices.push(ring[i]);
                indices.push(ring[(i + 1) % ring.len()]);
            }
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
    let chunk_extent = last as f32 * full_spacing;
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
    let mut wall = |strip: Vec<u32>, outward: Vec3| {
        add_perimeter_wall(&mut positions, &mut normals, &mut uvs, &mut indices, &strip, floor_y, outward);
    };
    if chunk.chunk_z == 0 {
        // z=0, walk +X.
        wall(south, Vec3::NEG_Z);
    }
    if chunk.chunk_x + 1 == terrain.chunks_x {
        // x=last, walk +Z.
        wall(east, Vec3::X);
    }
    if chunk.chunk_z + 1 == terrain.chunks_z {
        // z=last, walk -X.
        wall(north.into_iter().rev().collect(), Vec3::Z);
    }
    if chunk.chunk_x == 0 {
        // x=0, walk -Z.
        wall(west.into_iter().rev().collect(), Vec3::NEG_X);
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

    // Parent at origin, plus `TERRAIN_GRID_CLEARANCE` — defaults are tuned so
    // the initial flat heightmap (20% × range-50 + min=-10 = 0) lands on the
    // editor grid plane, and exactly coplanar is the one place the grid glitches.
    let terrain_entity = world
        .spawn((
            Name::new("Terrain"),
            Transform::from_xyz(0.0, TERRAIN_GRID_CLEARANCE, 0.0),
            Visibility::default(),
            terrain_data.clone(),
            // No material: a new terrain shows the blockout grid until one is
            // chosen (a `TerrainMaterial`; Alpine Landscape is the engine's
            // default, see `landscape`).
            renzora::SelectionStop,
        ))
        .id();

    let chunks = spawn_terrain_chunks(world, terrain_entity, &terrain_data, None);
    console_info(
        "Terrain",
        format!("Spawning terrain with {} chunks", chunks.len()),
    );

    terrain_entity
}

/// Spawn the flat chunk grid `terrain_data` describes as children of
/// `terrain_entity`, and return the chunk entities.
///
/// Split out of [`spawn_terrain`] because a terrain is not always born from the
/// Add menu: the editor also *converts* an existing flat mesh into one (see
/// `renzora_terrain_editor`'s plane→terrain conversion). Both routes have to lay
/// chunks out identically — same local origins, same collider, same
/// `TerrainChunkOf` wiring — or a converted terrain sculpts subtly differently
/// from a spawned one.
///
/// `material` is the project material each chunk should wear. `None` gives them
/// the terrain checkerboard, which is what a fresh terrain looks like; passing
/// one carries a converted mesh's assigned material across, and mirrors what
/// [`rehydrate_terrain_chunks`] does on scene load — a chunk with a `MaterialRef`
/// is left for the material resolver rather than having the checkerboard
/// stamped on top of it.
pub fn spawn_terrain_chunks(
    world: &mut World,
    terrain_entity: Entity,
    terrain_data: &TerrainData,
    material: Option<MaterialRef>,
) -> Vec<Entity> {
    let checker = material.is_none().then(|| {
        let mut mats = world.resource_mut::<Assets<TerrainCheckerboardMaterial>>();
        mats.add(TerrainCheckerboardMaterial::default())
    });

    // Chunk data only. The meshes are built by `rehydrate_terrain_chunks` for
    // the chunks streaming keeps resident, so a large grid is never built at
    // full detail all at once (see `stream_terrain_chunks`).
    let mut spawned = Vec::new();
    for cz in 0..terrain_data.chunks_z {
        for cx in 0..terrain_data.chunks_x {
            let mut chunk_data = TerrainChunkData::new(cx, cz, terrain_data.chunk_resolution, 0.2);
            chunk_data.dirty = false;
            let origin = terrain_data.chunk_world_origin(cx, cz);
            let chunk_entity = world
                .spawn((
                    Name::new(format!("Chunk ({},{})", cx, cz)),
                    Visibility::default(),
                    Transform::from_translation(origin),
                    PhysicsBodyData::static_body(),
                    CollisionShapeData::mesh(),
                    chunk_data,
                    TerrainChunkOf(terrain_entity),
                ))
                .id();
            match (&checker, &material) {
                (Some(checker), _) => {
                    world
                        .entity_mut(chunk_entity)
                        .insert(MeshMaterial3d(checker.clone()));
                }
                (None, Some(mat)) => {
                    world.entity_mut(chunk_entity).insert(mat.clone());
                }
                (None, None) => {}
            }
            // Insert ChildOf separately to trigger Bevy's hierarchy hooks
            // (on_insert hooks don't fire when ChildOf is part of a spawn bundle)
            world
                .entity_mut(chunk_entity)
                .insert(ChildOf(terrain_entity));
            spawned.push(chunk_entity);
        }
    }

    spawned
}

/// Most chunks given a full-detail mesh in one frame. Each also costs its
/// levels of detail (`ensure_chunk_lods`, which follows the mesh) and a GPU
/// upload, and a new 2 km terrain wanted about two hundred at once: three
/// frames of two to three seconds each. A dozen a frame, built in parallel
/// and nearest the camera first, fills the ground in under the camera while
/// the editor keeps drawing.
const MAX_BUILDS_PER_FRAME: usize = 12;

/// Rehydrate terrain chunks after scene load — spawns mesh, material, collider,
/// and `TerrainChunkOf` for chunks that have `TerrainChunkData` but no `Mesh3d`.
/// At most [`MAX_BUILDS_PER_FRAME`] a frame, nearest the streaming focus first.
#[allow(clippy::too_many_arguments)]
pub fn rehydrate_terrain_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TerrainCheckerboardMaterial>>,
    focus: Option<Res<crate::streaming::StreamFocus>>,
    terrain_query: Query<&TerrainData>,
    roots: Query<&GlobalTransform>,
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
        // A chunk still waiting for its heights would be built flat and then
        // again (`starter::PendingShape`).
        (
            Without<Mesh3d>,
            Without<super::data::TerrainChunkStreamedOut>,
            Without<crate::starter::PendingShape>,
        ),
    >,
    all_chunks: Query<(&TerrainChunkData, &TerrainChunkOf)>,
    pending: Query<&TerrainChunkOf, With<crate::starter::PendingShape>>,
) {
    if chunk_query.is_empty() {
        return;
    }
    // A terrain still being shaped builds nothing yet, not even its shaped
    // chunks: a chunk's edges are lit from its neighbours' heights, so one
    // built beside a neighbour that was still flat kept a seam along that
    // edge after the neighbour got its real heights.
    let shaping: std::collections::HashSet<Entity> = pending.iter().map(|of| of.0).collect();

    // Which chunks this frame: the nearest, by the centre of the chunk on the
    // ground. Without a focus (no camera yet) the order does not matter.
    let focus = focus.and_then(|f| f.0);
    let mut queue: Vec<(f32, Entity, Entity)> = chunk_query
        .iter()
        .filter_map(|(entity, chunk, chunk_of, child_of, _)| {
            // Resolve parent terrain: prefer TerrainChunkOf, fall back to ChildOf parent
            let parent = chunk_of.map(|c| c.0).or_else(|| child_of.map(|c| c.0))?;
            if shaping.contains(&parent) {
                return None;
            }
            let terrain = terrain_query.get(parent).ok()?;
            let distance = match (focus, roots.get(parent)) {
                (Some(focus), Ok(root)) => {
                    let half = terrain.chunk_size * 0.5;
                    let centre = root.transform_point(
                        terrain.chunk_world_origin(chunk.chunk_x, chunk.chunk_z) + Vec3::new(half, 0.0, half),
                    );
                    focus.xz().distance_squared(centre.xz())
                }
                _ => 0.0,
            };
            Some((distance, entity, parent))
        })
        .collect();
    queue.sort_by(|a, b| a.0.total_cmp(&b.0));
    queue.truncate(MAX_BUILDS_PER_FRAME);

    // Every chunk by terrain and cell, for the neighbours' edge heights: a
    // chunk streamed back in beside resident ones must light its edges as they
    // do, or the seam line comes back wherever streaming has been.
    let grid: HashMap<(Entity, u32, u32), &TerrainChunkData> = all_chunks
        .iter()
        .map(|(chunk, of)| ((of.0, chunk.chunk_x, chunk.chunk_z), chunk))
        .collect();

    // The meshes are pure work on the heights, so they build in parallel.
    let jobs: Vec<(Entity, Entity, &TerrainData, &TerrainChunkData, ChunkNeighbours)> = queue
        .iter()
        .filter_map(|&(_, entity, parent)| {
            let (_, chunk, ..) = chunk_query.get(entity).ok()?;
            let terrain = terrain_query.get(parent).ok()?;
            let neighbours =
                ChunkNeighbours::around(chunk.chunk_x, chunk.chunk_z, |x, z| grid.get(&(parent, x, z)).copied());
            Some((entity, parent, terrain, chunk, neighbours))
        })
        .collect();
    let built: Vec<(Entity, Entity, Mesh)> = bevy::tasks::ComputeTaskPool::get().scope(|scope| {
        for (entity, parent, terrain, chunk, neighbours) in &jobs {
            scope.spawn(async move { (*entity, *parent, generate_chunk_lod_mesh_beside(terrain, chunk, 1, *neighbours)) });
        }
    });

    let mut material: Option<Handle<TerrainCheckerboardMaterial>> = None;
    for (entity, parent_entity, mesh) in built {
        let Ok((_, _, chunk_of, _, mat_ref)) = chunk_query.get(entity) else {
            continue;
        };

        // Ensure parent has required hierarchy components (scene loader may omit
        // them). Only when missing: a default `InheritedVisibility` is hidden,
        // and nothing recomputes it while the root's `Visibility` is unchanged,
        // so overwriting it hid the whole terrain. That ran once per scene load
        // until chunks streamed back in through here, then on every stream-in.
        commands
            .entity(parent_entity)
            .insert_if_new((GlobalTransform::default(), InheritedVisibility::default()));

        let mesh_handle = meshes.add(mesh);

        let mut ec = commands.entity(entity);
        ec.insert((
            Mesh3d(mesh_handle),
            PhysicsBodyData::static_body(),
            CollisionShapeData::mesh(),
        ));

        // Only apply default checkerboard if chunk has no custom material assigned
        if mat_ref.is_none() {
            let material = material
                .get_or_insert_with(|| materials.add(TerrainCheckerboardMaterial::default()))
                .clone();
            ec.insert(MeshMaterial3d(material));
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
                chunk_data.dirty = false;
                let origin = terrain_data.chunk_world_origin(cx, cz);
                // Meshless, like every other route that makes chunks: the
                // resident ones are built by `rehydrate_terrain_chunks`.
                commands.entity(terrain_entity).with_child((
                    Name::new(format!("Chunk ({},{})", cx, cz)),
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
    // Keyed on `mesh_stale` (set by composition), NOT `dirty` (set by
    // writers): see the flag docs on `TerrainChunkData`.
    if !chunk_query.iter().any(|(_, chunk, _, _)| chunk.mesh_stale) {
        return;
    }
    // Every chunk by terrain and grid cell, for the neighbours' edge heights.
    let grid: HashMap<(Entity, u32, u32), &TerrainChunkData> = chunk_query
        .iter()
        .map(|(_, chunk, of, _)| ((of.0, chunk.chunk_x, chunk.chunk_z), chunk))
        .collect();
    let mut rebuilt = Vec::new();
    for (entity, chunk, chunk_of, mesh_handle) in chunk_query.iter() {
        if !chunk.mesh_stale {
            continue;
        }
        let Ok(terrain) = terrain_query.get(chunk_of.0) else {
            continue;
        };
        let neighbours = ChunkNeighbours::around(chunk.chunk_x, chunk.chunk_z, |x, z| {
            grid.get(&(chunk_of.0, x, z)).copied()
        });
        let new_mesh = generate_chunk_lod_mesh_beside(terrain, chunk, 1, neighbours);
        if let Some(mut mesh) = meshes.get_mut(&mesh_handle.0) {
            *mesh = new_mesh;
        }
        rebuilt.push(entity);
    }
    drop(grid);
    for entity in rebuilt {
        commands.entity(entity).try_insert(ColliderStale {
            last_change: time.elapsed_secs(),
        });
        if let Ok((_, mut chunk, _, _)) = chunk_query.get_mut(entity) {
            chunk.mesh_stale = false;
        }
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
    // A streamed-out chunk has no collider on purpose: rebuilding one for an
    // edit made just before it left put the collider back on every chunk a
    // generate or erode had touched, and with it most of what streaming saves.
    stale: Query<(Entity, &ColliderStale), Without<crate::data::TerrainChunkStreamedOut>>,
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

/// Give every chunk whose collider is being (re)built its heights as a
/// heightfield, so physics builds that instead of a trimesh from the mesh.
///
/// A trimesh of a 129-vertex chunk is 32 thousand triangles, and the physics
/// engine works out mass properties for each one it builds: with streaming
/// bringing in dozens of chunks at once, that alone was a 57 ms frame. A
/// heightfield is the same ground, built from the grid the chunk already has.
///
/// Keyed on the collider request (`CollisionShapeData` added or reinserted,
/// which is what the debounced rebuild above does), so a held brush stroke
/// never rebuilds it.
#[allow(clippy::type_complexity)]
pub fn sync_chunk_heightfields(
    mut commands: Commands,
    terrains: Query<&TerrainData>,
    chunks: Query<
        (Entity, &TerrainChunkData, &TerrainChunkOf),
        (Changed<CollisionShapeData>, Without<crate::data::TerrainChunkStreamedOut>),
    >,
) {
    for (entity, chunk, of) in &chunks {
        let Ok(terrain) = terrains.get(of.0) else { continue };
        if let Some(field) = chunk_heightfield(terrain, chunk) {
            commands
                .entity(entity)
                .try_insert(renzora::physics::HeightfieldSource(std::sync::Arc::new(field)));
        }
    }
}

/// A chunk's composed heights as a heightfield in the chunk's local space.
pub fn chunk_heightfield(terrain: &TerrainData, chunk: &TerrainChunkData) -> Option<renzora::physics::HeightfieldData> {
    let res = terrain.chunk_resolution as usize;
    let source = if chunk.heights.len() == res * res { &chunk.heights } else { &chunk.base_heights };
    if res < 2 || source.len() != res * res {
        return None;
    }
    let range = terrain.height_range();
    Some(renzora::physics::HeightfieldData {
        heights: source.iter().map(|h| terrain.min_height + h * range).collect(),
        columns: res,
        rows: res,
        size: Vec2::splat(terrain.chunk_size),
    })
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
#[allow(clippy::too_many_arguments)]
pub fn terrain_data_changed_system(
    mut commands: Commands,
    mut terrain_materials: ResMut<Assets<TerrainCheckerboardMaterial>>,
    changed_terrain: Query<(Entity, &TerrainData), Changed<TerrainData>>,
    added: Query<Entity, Added<TerrainData>>,
    mut chunk_query: Query<(Entity, &TerrainChunkOf, &mut TerrainChunkData)>,
    // Painted layer weights, carried over to the respawned chunk in the same
    // cell (resampled on first read if the resolution changed).
    weights: Query<&crate::landscape::TerrainChunkWeights>,
    // The fields the chunk meshes are built from, per terrain, as last seen.
    mut shapes: Local<HashMap<Entity, [u32; 6]>>,
) {
    for (terrain_entity, terrain_data) in changed_terrain.iter() {
        let shape = [
            terrain_data.chunks_x,
            terrain_data.chunks_z,
            terrain_data.chunk_resolution,
            terrain_data.chunk_size.to_bits(),
            terrain_data.min_height.to_bits(),
            terrain_data.max_height.to_bits(),
        ];
        let previous = shapes.insert(terrain_entity, shape);
        // Skip first insertion — chunks are spawned by the terrain creation command
        if added.contains(terrain_entity) {
            continue;
        }
        // Only the grid, resolution, chunk size and height range change what a
        // chunk's mesh is. Anything else (streaming, levels of detail) used to
        // rebuild every chunk, its collider and its virtual geometry: toggling
        // streaming on a 256-chunk terrain stalled the editor for seconds.
        if previous == Some(shape) {
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
        let old_weights: HashMap<(u32, u32), crate::landscape::TerrainChunkWeights> = existing
            .iter()
            .filter_map(|(e, cx, cz, _)| weights.get(*e).ok().map(|w| ((*cx, *cz), w.clone())))
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

                let origin = terrain_data.chunk_world_origin(cx, cz);

                // No mesh: streaming decides which chunks get one (see
                // `stream_terrain_chunks`) and `rehydrate_terrain_chunks`
                // builds those, so a large grid never builds them all at once.
                commands.entity(terrain_entity).with_child((
                    // The scene saver only serializes named entities — a
                    // nameless chunk silently vanishes from the save and the
                    // terrain loads back as an empty root.
                    Name::new(format!("Chunk ({},{})", cx, cz)),
                    MeshMaterial3d(material.clone()),
                    Transform::from_translation(origin),
                    Visibility::default(),
                    PhysicsBodyData::static_body(),
                    CollisionShapeData::mesh(),
                    chunk_data,
                    TerrainChunkOf(terrain_entity),
                    old_weights.get(&(cx, cz)).cloned().unwrap_or_default(),
                ));
            }
        }
    }
}
