//! Strand generation: sample root points across the source mesh's surface, grow
//! a tapered strand from each, and spawn the hidden render entity that the ribbon
//! mesh is rebuilt into every frame.
//!
//! Runs as a poll (the `Mesh3d` asset loads asynchronously, so the mesh often
//! isn't in `Assets<Mesh>` for the first few frames) and re-runs whenever a
//! shape field changes (`Hair::shape_signature`).

use crate::Hair;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use std::collections::HashMap;

/// Hard cap so a fat-fingered strand count can't try to allocate millions of
/// verts and hang the frame.
pub const MAX_STRANDS: usize = 50_000;

/// Frames to wait for the source mesh to finish loading before giving up.
const MESH_WAIT_FRAMES: u32 = 600;

/// One grown strand. Positions are stored in the source entity's **local**
/// space (`rest_local`); the sim projects them to world space each frame via the
/// entity's `GlobalTransform`, so the groom follows the model even when static.
pub(crate) struct Strand {
    /// Grown rest shape, root → tip, in source-mesh local space.
    pub rest_local: Vec<Vec3>,
    /// Current world-space positions (filled by the sim).
    pub world: Vec<Vec3>,
    /// Previous world positions (verlet history).
    pub prev: Vec<Vec3>,
    /// Root half-width of this strand's ribbon, world units.
    pub half_width: f32,
    /// Per-strand shade multiplier (0..1) for subtle color variation.
    pub shade: f32,
}

/// Runtime groom state, inserted alongside `Hair` once the strands exist. Not
/// reflected/serialized — the groom is regenerated from `Hair` on load.
#[derive(Component)]
pub struct HairGroomData {
    pub(crate) strands: Vec<Strand>,
    /// Hidden child entity holding the rebuilt ribbon `Mesh3d`.
    pub(crate) render_entity: Entity,
    pub(crate) mesh: Handle<Mesh>,
    pub(crate) material: Handle<StandardMaterial>,
    /// `Hair::shape_signature` the current strands were built from.
    pub(crate) signature: u64,
    /// False until the sim has seeded world/prev from the rest pose.
    pub(crate) sim_seeded: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn build_grooms(
    mut commands: Commands,
    mut query: Query<(Entity, &Hair, &Mesh3d, &GlobalTransform, Option<&mut HairGroomData>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut waited: Local<HashMap<Entity, u32>>,
) {
    for (entity, hair, mesh3d, gtransform, data) in &mut query {
        let signature = hair.shape_signature();

        // Already built and shape unchanged — nothing to do.
        if let Some(ref d) = data {
            if d.signature == signature {
                continue;
            }
        }

        let Some(source) = meshes.get(&mesh3d.0) else {
            // Mesh still loading — poll, but bound the wait.
            if data.is_none() {
                let n = waited.entry(entity).or_insert(0);
                *n += 1;
                if *n >= MESH_WAIT_FRAMES {
                    warn!("Hair on {entity:?}: source Mesh3d never loaded after {MESH_WAIT_FRAMES} frames, giving up");
                    waited.remove(&entity);
                }
            }
            continue;
        };
        waited.remove(&entity);

        let strands = generate_strands(source, hair, gtransform);
        info!("Hair on {entity:?}: grew {} strand(s)", strands.len());

        if let Some(mut d) = data {
            // Regenerate in place, keeping the render entity + handles.
            d.strands = strands;
            d.signature = signature;
            d.sim_seeded = false;
        } else {
            // First build — spawn the hidden render entity. Its ribbon mesh is
            // written in world space, so it sits at the world origin with an
            // identity transform (not parented to the model). `HideInHierarchy`
            // keeps the generated geometry out of the outliner and the saved
            // scene — only `Hair` persists; the groom rebuilds on load.
            let mesh_handle = meshes.add(empty_ribbon_mesh());
            let material = materials.add(hair_material(hair));
            let render_entity = commands
                .spawn((
                    Mesh3d(mesh_handle.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::default(),
                    Visibility::Visible,
                    renzora::HideInHierarchy,
                ))
                .id();

            commands.entity(entity).insert(HairGroomData {
                strands,
                render_entity,
                mesh: mesh_handle,
                material,
                signature,
                sim_seeded: false,
            });
        }
    }
}

/// Despawn the render entity when its `Hair` component is removed, so a deleted
/// groom doesn't leave orphaned geometry floating in the world.
pub fn cleanup_grooms(
    mut commands: Commands,
    orphaned: Query<(Entity, &HairGroomData), Without<Hair>>,
) {
    for (entity, data) in &orphaned {
        commands.entity(data.render_entity).despawn();
        commands.entity(entity).remove::<HairGroomData>();
    }
}

/// An empty `TriangleList` mesh kept in both worlds; the sim rewrites its
/// attributes every frame (`MAIN_WORLD` so it stays mutable via `Assets`).
fn empty_ribbon_mesh() -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}

/// Strands are double-sided (a ribbon is one quad-strip seen from both faces)
/// and use vertex colors for per-strand shade variation.
pub(crate) fn hair_material(hair: &Hair) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgb(hair.color.x, hair.color.y, hair.color.z),
        perceptual_roughness: 0.75,
        metallic: 0.0,
        double_sided: true,
        cull_mode: None,
        ..default()
    }
}

/// Area-weighted surface scatter + growth. Reads POSITION (and NORMAL if present,
/// else per-triangle geometric normals) from the source mesh, in its local space.
fn generate_strands(mesh: &Mesh, hair: &Hair, gtransform: &GlobalTransform) -> Vec<Strand> {
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return Vec::new();
    };
    let normals = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
        Some(VertexAttributeValues::Float32x3(n)) => Some(n),
        _ => None,
    };

    // Triangle index triples, from the index buffer or an unindexed soup.
    let tris: Vec<[usize; 3]> = match mesh.indices() {
        Some(Indices::U16(v)) => v.chunks_exact(3).map(|c| [c[0] as usize, c[1] as usize, c[2] as usize]).collect(),
        Some(Indices::U32(v)) => v.chunks_exact(3).map(|c| [c[0] as usize, c[1] as usize, c[2] as usize]).collect(),
        None => (0..positions.len() / 3).map(|t| [t * 3, t * 3 + 1, t * 3 + 2]).collect(),
    };

    // Total surface area, to hand each triangle its fair share of strands.
    let mut total_area = 0.0f32;
    for t in &tris {
        if let (Some(&a), Some(&b), Some(&c)) =
            (positions.get(t[0]), positions.get(t[1]), positions.get(t[2]))
        {
            let (a, b, c) = (Vec3::from(a), Vec3::from(b), Vec3::from(c));
            total_area += (b - a).cross(c - a).length() * 0.5;
        }
    }
    if total_area < 1e-9 {
        return Vec::new();
    }

    let target = (hair.strands.max(0.0) as usize).min(MAX_STRANDS);
    let segments = (hair.segments as usize).clamp(1, 16);
    let seg_len = (hair.length / segments as f32).max(1e-4);
    // World "down" expressed in the mesh's local space, so droop points the way
    // gravity does regardless of how the model is oriented.
    let local_down = (gtransform.rotation().inverse() * Vec3::NEG_Y).normalize_or_zero();

    let mut strands = Vec::new();
    for (ti, t) in tris.iter().enumerate() {
        let (Some(&pa), Some(&pb), Some(&pc)) =
            (positions.get(t[0]), positions.get(t[1]), positions.get(t[2]))
        else {
            continue;
        };
        let (pa, pb, pc) = (Vec3::from(pa), Vec3::from(pb), Vec3::from(pc));
        let area = (pb - pa).cross(pc - pa).length() * 0.5;
        if area < 1e-9 {
            continue;
        }

        // Expected strands here = share of the target by area; the fractional
        // remainder is placed probabilistically so small triangles still get hair.
        let expected = target as f32 * area / total_area;
        let mut count = expected.floor() as usize;
        if rand01((ti as u32).wrapping_mul(9781).wrapping_add(1)) < expected.fract() {
            count += 1;
        }

        // Vertex normals if available, else the flat triangle normal.
        let tri_n = (pb - pa).cross(pc - pa).normalize_or_zero();
        let vn = |i: usize| normals.and_then(|n| n.get(i)).map(|&v| Vec3::from(v)).unwrap_or(tri_n);
        let (na, nb, nc) = (vn(t[0]), vn(t[1]), vn(t[2]));

        for k in 0..count {
            if strands.len() >= MAX_STRANDS {
                return strands;
            }
            let seed = (ti as u32).wrapping_mul(2_654_435_761)
                ^ (k as u32).wrapping_mul(40_503)
                ^ 0x9E37_79B9;
            // Barycentric root + interpolated normal.
            let mut u = rand01(seed);
            let mut v = rand01(seed ^ 0x68bc_21eb);
            if u + v > 1.0 {
                u = 1.0 - u;
                v = 1.0 - v;
            }
            let w = 1.0 - u - v;
            let root = pa * w + pb * u + pc * v;
            let normal = (na * w + nb * u + nc * v).normalize_or_zero();
            let normal = if normal == Vec3::ZERO { tri_n } else { normal };

            let len_factor =
                1.0 - hair.length_jitter.clamp(0.0, 1.0) * rand01(seed ^ 0x0001_2345);
            let shade = rand01(seed ^ 0x00ab_cdef);

            // Grow: step along a direction that eases from the surface normal
            // toward local-down by `droop`, so strands lift off then fall.
            let mut pts = Vec::with_capacity(segments + 1);
            pts.push(root);
            let mut p = root;
            for i in 1..=segments {
                let t01 = i as f32 / segments as f32;
                let dir = normal
                    .lerp(local_down, hair.droop.clamp(0.0, 1.0) * t01)
                    .normalize_or_zero();
                let dir = if dir == Vec3::ZERO { normal } else { dir };
                p += dir * seg_len * len_factor;
                pts.push(p);
            }

            strands.push(Strand {
                world: pts.clone(),
                prev: pts.clone(),
                rest_local: pts,
                half_width: hair.width.max(1e-4),
                shade,
            });
        }
    }
    strands
}

/// Cheap deterministic hash → `[0, 1)`. Deterministic (not `Math.random`) so a
/// groom regenerates identically on reload for the same `Hair` settings.
fn rand01(x: u32) -> f32 {
    let mut h = x.wrapping_add(0x9E37_79B9);
    h = (h ^ (h >> 16)).wrapping_mul(0x85eb_ca6b);
    h = (h ^ (h >> 13)).wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    (h & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}
