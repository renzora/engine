//! The default material for an untextured primitive, and keeping its grid the
//! same size in the world however the object is scaled.
//!
//! The image itself is generated in `renzora::core::blockout_grid`; this is the
//! one place that turns it into a `StandardMaterial`. It lives here rather than
//! in the contract crate because `StandardMaterial` comes from `bevy_pbr`,
//! which a 2D-only export strips out entirely — `renzora` has to keep compiling
//! without it.
//!
//! Every path that can put a fresh primitive in the world goes through
//! [`blockout_material`]: spawning one, undoing a delete, rehydrating a scene on
//! load, and the ghost that follows the cursor during a shape drag. They used to
//! build the material inline and had drifted apart — the drag ghost was a
//! different colour from the shape it dropped, and a reloaded scene came back
//! glossier than the same shapes still sitting in it, because only `MeshColor`
//! is serialized and the rehydration path had never been given the roughness the
//! spawn path used.

use bevy::prelude::*;
use bevy::render::mesh::VertexAttributeValues;
use std::collections::HashSet;
use renzora::core::{GridTexture, MeshColor, MeshPrimitive};

/// The material a primitive wears until it is given one of its own: the
/// blockout grid, tinted by `base_color`.
///
/// `grid` is optional because a headless/server build has no `Assets<Image>`
/// and so never generates the image; without it this is a plain matte fill in
/// the shape's colour, which is all a server needs.
///
/// Deliberately flat — no normal map, no occlusion map. See the module docs on
/// `renzora::core::blockout_grid` for why relief was tried and taken back out.
pub fn blockout_material(base_color: Color, grid: Option<&GridTexture>) -> StandardMaterial {
    StandardMaterial {
        base_color,
        base_color_texture: grid.map(|g| g.0.clone()),
        perceptual_roughness: 0.9,
        ..default()
    }
}

/// Push a changed [`MeshColor`] into the material the entity is already
/// wearing.
///
/// Until this existed, `MeshColor` was write-once: every path that *creates* a
/// primitive read it (spawn, undo of a delete, scene load, the drag ghost) and
/// baked it into a fresh `StandardMaterial`, and nothing at all watched it
/// afterwards. So the component was in the scene file, on the entity, and
/// registered for reflection, and yet setting it did nothing until the next
/// reload. That caught the inspector's new colour row, but it caught scripts
/// too: `ScriptCommand::Spawn` inserts a `MeshColor`, and a script that changed
/// one later was silently ignored.
///
/// Skips anything with a [`MaterialRef`](renzora::core::MaterialRef): there the
/// resolver owns the material, and writing a base colour over a compiled graph
/// would fight it every frame the graph recompiled.
///
/// Alpha is honoured by switching the alpha mode with it. `blockout_material`
/// leaves the mode at `Opaque`, so a colour picked with alpha below 1 would
/// otherwise look exactly like the opaque one and read as the alpha slider
/// being broken.
#[cfg(feature = "render_3d")]
pub fn apply_mesh_color(
    changed: Query<
        (&MeshColor, &MeshMaterial3d<StandardMaterial>),
        (
            Changed<MeshColor>,
            Without<renzora::core::MaterialRef>,
        ),
    >,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let Some(mut materials) = materials else { return };
    for (color, handle) in &changed {
        let Some(mut material) = materials.get_mut(&handle.0) else {
            continue;
        };
        material.base_color = color.0;
        material.alpha_mode = if color.0.alpha() < 1.0 {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        };
    }
}

/// World units spanned by one tile of the blockout grid image.
///
/// One, so the grid measures metres directly: at the default scale a unit cube
/// wears exactly one tile per face, which is what the shape registry's authored
/// UVs already produced and so keeps every existing scene looking the same.
const TILE: f32 = 1.0;

/// Project the blockout grid onto a shape from its own geometry, so the grid
/// keeps a constant size in the world however the shape is scaled, extruded or
/// inset.
///
/// # Why projection and not a UV rescale
///
/// This used to take the mesh's authored UVs and stretch them by the object's
/// scale, which handled the one case it was written for (dragging a cube out
/// into a wall) and nothing else. Its assumption was that the mesh's unwrap
/// stays put and only the object's size changes. Modeling breaks that
/// assumption immediately: extrude and inset build new faces, and an operator
/// that has no idea what a blockout grid is has no UVs to give them. The new
/// geometry inherited whatever the fallback produced, and the grid smeared into
/// stripes across exactly the faces you had just made.
///
/// Deriving the UVs instead means there is nothing for an operator to get
/// wrong. Every vertex is projected along whichever axis its normal points down
/// most, in world-scaled object space, so the tile size is a property of the
/// world rather than of the mesh's history. Scale a wall, extrude a ledge,
/// inset a panel: the grid stays square and the same size as the grid on
/// everything else in the scene, because all of them are measuring the same
/// space.
///
/// It is also idempotent, which is what lets it run on whatever the mesh editor
/// last baked without any handshake between the two: re-projecting an
/// already-projected mesh computes the same UVs and writes nothing.
///
/// Entities with a [`MaterialRef`](renzora::core::MaterialRef) are left alone:
/// once a real material is on the mesh, its UVs are the author's business.
pub fn project_blockout_uvs(
    query: Query<(Ref<Mesh3d>, Ref<GlobalTransform>), (With<MeshPrimitive>, Without<renzora::core::MaterialRef>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mesh_events: MessageReader<AssetEvent<Mesh>>,
) {
    // A mesh's *contents* can change without its handle changing: the mesh
    // editor bakes in place, so `Mesh3d` never fires. Collect the assets that
    // were modified this frame and treat those as changed too.
    //
    // Writing below modifies the asset and so produces one of these events on
    // the next frame. That does not loop, because the equality check makes the
    // second pass write nothing, and a pass that writes nothing emits no event.
    let mut touched: HashSet<AssetId<Mesh>> = HashSet::new();
    for event in mesh_events.read() {
        if let AssetEvent::Modified { id } | AssetEvent::Added { id } = event {
            touched.insert(*id);
        }
    }

    for (mesh3d, transform) in &query {
        if !mesh3d.is_changed() && !transform.is_changed() && !touched.contains(&mesh3d.0.id()) {
            continue;
        }
        let scale = transform.scale().abs().max(Vec3::splat(1e-4));
        let Some(mesh) = meshes.get(&mesh3d.0) else {
            continue;
        };
        let Some(projected) = project_uvs(mesh, scale) else {
            continue;
        };

        // Read, compare, and only then take the mutable borrow. `get_mut` emits
        // `AssetEvent::Modified` whether or not anything is written, and this
        // system reads those events, so writing unconditionally would have it
        // re-examining every blockout mesh in the scene every frame forever.
        let unchanged = matches!(
            mesh.attribute(Mesh::ATTRIBUTE_UV_0),
            Some(VertexAttributeValues::Float32x2(current))
                if current.len() == projected.len()
                    && current.iter().zip(&projected).all(|(a, b)| {
                        (a[0] - b[0]).abs() < 1e-5 && (a[1] - b[1]).abs() < 1e-5
                    })
        );
        if unchanged {
            continue;
        }

        if let Some(mut mesh) = meshes.get_mut(&mesh3d.0) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, projected);
        }
    }
}

/// Project `mesh`'s blockout UVs in place, for an object at `scale`.
///
/// The system above is the normal path and reaches everything in the scene. This
/// is for a mesh that is not in the scene yet: the ghost that follows the cursor
/// during a shape drag is not a scene primitive, so nothing projects it, and it
/// would wear the shape registry's authored unwrap right up until the moment you
/// let go and the real shape landed wearing a different one.
///
/// Does nothing to a mesh it cannot measure, for the same reason
/// [`project_uvs`] returns `None` there.
pub fn project_mesh_uvs(mesh: &mut Mesh, scale: Vec3) {
    if let Some(uvs) = project_uvs(mesh, scale) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    }
}

/// Box-project every vertex of `mesh`, in object space scaled to world size.
///
/// `None` when the mesh has no positions or no normals to project along, which
/// is the honest answer for something this cannot measure rather than a guess
/// that would look like a bug.
fn project_uvs(mesh: &Mesh, scale: Vec3) -> Option<Vec<[f32; 2]>> {
    let VertexAttributeValues::Float32x3(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)?
    else {
        return None;
    };
    let VertexAttributeValues::Float32x3(normals) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL)? else {
        return None;
    };
    if normals.len() != positions.len() {
        return None;
    }

    Some(
        positions
            .iter()
            .zip(normals)
            .map(|(p, n)| {
                // World-sized offsets, deliberately without the rotation: which
                // way a wall is turned should not change how big its tiles are.
                let w = Vec3::from(*p) * scale / TILE;
                project_vertex(w, Vec3::from(*n))
            })
            .collect(),
    )
}

/// One vertex's UV: the two axes that are not the one its normal points down.
///
/// Split out so the axis choice can be tested on its own: it is the whole of
/// what makes a projection read as a grid rather than as a smear, and it is
/// wrong in a way that is hard to see on a screenshot and trivial to assert.
fn project_vertex(world: Vec3, normal: Vec3) -> [f32; 2] {
    let n = normal.abs();
    if n.x >= n.y && n.x >= n.z {
        [world.z, world.y]
    } else if n.y >= n.z {
        [world.x, world.z]
    } else {
        [world.x, world.y]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::primitives::Cuboid;

    fn uvs_of(mesh: &Mesh) -> Vec<[f32; 2]> {
        match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
            Some(VertexAttributeValues::Float32x2(v)) => v.clone(),
            _ => panic!("mesh has no UVs"),
        }
    }

    /// The grid keeps a constant size in the world as the object is scaled.
    ///
    /// Stated as the property that matters: the span of UV across a face has to
    /// grow in step with the face, so a cube dragged into a wall shows more
    /// tiles rather than four stretched ones.
    #[test]
    fn scaling_a_shape_adds_tiles_instead_of_stretching_them() {
        let mesh = Mesh::from(Cuboid::new(1.0, 1.0, 1.0));

        let unit = project_uvs(&mesh, Vec3::ONE).expect("a cuboid projects");
        let wide = project_uvs(&mesh, Vec3::new(8.0, 1.0, 1.0)).expect("a cuboid projects");

        let span = |uvs: &[[f32; 2]], axis: usize| {
            let (lo, hi) = uvs.iter().fold((f32::MAX, f32::MIN), |(lo, hi), uv| {
                (lo.min(uv[axis]), hi.max(uv[axis]))
            });
            hi - lo
        };
        // Eight times the object, eight times the tiles across it.
        assert!(
            (span(&wide, 0) - span(&unit, 0) * 8.0).abs() < 1e-4,
            "u span {} should be 8x {}",
            span(&wide, 0),
            span(&unit, 0)
        );
        // The axis that did not grow is untouched, or a non-uniform scale would
        // square up one direction by skewing the other.
        assert!((span(&wide, 1) - span(&unit, 1)).abs() < 1e-4);
    }

    /// Tiles stay square under a non-uniform scale, which is the whole point of
    /// projecting rather than stretching an authored unwrap.
    #[test]
    fn tiles_stay_square_under_a_non_uniform_scale() {
        let mesh = Mesh::from(Cuboid::new(1.0, 1.0, 1.0));
        let uvs = project_uvs(&mesh, Vec3::new(5.0, 1.0, 3.0)).expect("a cuboid projects");

        let VertexAttributeValues::Float32x3(positions) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
        else {
            panic!()
        };
        // Every edge of the cuboid must map to a UV distance equal to its world
        // length, on whichever face it belongs to. Checking the two ends of the
        // mesh's own index list is enough to catch an axis mix-up.
        let indices: Vec<usize> = mesh.indices().unwrap().iter().collect();
        for tri in indices.chunks_exact(3) {
            for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
                let world_a = Vec3::from(positions[a]) * Vec3::new(5.0, 1.0, 3.0);
                let world_b = Vec3::from(positions[b]) * Vec3::new(5.0, 1.0, 3.0);
                let uv_a = Vec2::from(uvs[a]);
                let uv_b = Vec2::from(uvs[b]);
                // The projection drops one axis, so the UV distance is the world
                // distance measured in the plane. It can never exceed it.
                assert!(
                    uv_a.distance(uv_b) <= world_a.distance(world_b) + 1e-4,
                    "a UV edge grew longer than the edge it measures"
                );
            }
        }
    }

    /// A vertex projects along the two axes its normal does *not* point down.
    /// Get this wrong and a face's UVs collapse to a line, which is exactly the
    /// stripe pattern this replaced.
    #[test]
    fn a_vertex_projects_across_its_face_not_through_it() {
        let world = Vec3::new(2.0, 3.0, 5.0);
        assert_eq!(project_vertex(world, Vec3::X), [5.0, 3.0]);
        assert_eq!(project_vertex(world, Vec3::NEG_X), [5.0, 3.0]);
        assert_eq!(project_vertex(world, Vec3::Y), [2.0, 5.0]);
        assert_eq!(project_vertex(world, Vec3::Z), [2.0, 3.0]);
    }

    /// Re-projecting a projected mesh must produce the same UVs. The system
    /// relies on this to run beside the mesh editor without a handshake, and to
    /// avoid re-triggering itself through the asset events it listens to.
    #[test]
    fn projection_is_idempotent() {
        let mut mesh = Mesh::from(Cuboid::new(1.0, 1.0, 1.0));
        let scale = Vec3::new(2.0, 0.5, 3.0);

        let once = project_uvs(&mesh, scale).expect("a cuboid projects");
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, once.clone());
        let twice = project_uvs(&mesh, scale).expect("a cuboid projects");

        assert_eq!(uvs_of(&mesh), once);
        assert_eq!(once, twice);
    }

    /// A mesh with no normals cannot be projected, and says so rather than
    /// guessing an axis and producing a smear that looks like a bug elsewhere.
    #[test]
    fn a_mesh_without_normals_is_left_alone() {
        let mut mesh = Mesh::from(Cuboid::new(1.0, 1.0, 1.0));
        mesh.remove_attribute(Mesh::ATTRIBUTE_NORMAL);
        assert!(project_uvs(&mesh, Vec3::ONE).is_none());
    }
}
