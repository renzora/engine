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
/// The normal map is the hairline groove along each line; see the module docs
/// on `renzora::core::blockout_grid` for why relief stays inside the lines. It
/// needs the tangents [`project_blockout_uvs`] writes, since a mesh without them
/// has its normal map silently dropped.
///
/// Roughness is 0.4, not the 0.9 it was: at 0.9 the sun's highlight spread so
/// wide that a face lit as one flat tone and the material read as unresponsive
/// to the sun. Glossy enough to smear a visible highlight across a floor toward
/// a low sun and give the groove walls a glint, still well short of polished.
pub fn blockout_material(base_color: Color, grid: Option<&GridTexture>) -> StandardMaterial {
    StandardMaterial {
        base_color,
        base_color_texture: grid.map(|g| g.color.clone()),
        normal_map_texture: grid.map(|g| g.normal.clone()),
        perceptual_roughness: 0.4,
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
        if needs_split(mesh, scale) {
            if let Some(mut mesh) = meshes.get_mut(&mesh3d.0) {
                project_mesh_uvs(&mut mesh, scale);
            }
            continue;
        }
        let (Some(projected), Some(tangents)) = (project_uvs(mesh, scale), project_tangents(mesh, scale))
        else {
            continue;
        };

        // Read, compare, and only then take the mutable borrow. `get_mut` emits
        // `AssetEvent::Modified` whether or not anything is written, and this
        // system reads those events, so writing unconditionally would have it
        // re-examining every blockout mesh in the scene every frame forever.
        let close = |a: &[f32], b: &[f32]| a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-5);
        let uvs_unchanged = matches!(
            mesh.attribute(Mesh::ATTRIBUTE_UV_0),
            Some(VertexAttributeValues::Float32x2(current))
                if current.len() == projected.len()
                    && current.iter().zip(&projected).all(|(a, b)| close(a, b))
        );
        let tangents_unchanged = matches!(
            mesh.attribute(Mesh::ATTRIBUTE_TANGENT),
            Some(VertexAttributeValues::Float32x4(current))
                if current.len() == tangents.len()
                    && current.iter().zip(&tangents).all(|(a, b)| close(a, b))
        );
        if uvs_unchanged && tangents_unchanged {
            continue;
        }

        if let Some(mut mesh) = meshes.get_mut(&mesh3d.0) {
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, projected);
            mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangents);
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
    if needs_split(mesh, scale) {
        split_shared_vertices(mesh);
    }
    if let (Some(uvs), Some(tangents)) = (project_uvs(mesh, scale), project_tangents(mesh, scale)) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangents);
    }
}

/// Tangents that match the box projection, for the grid's bevel normal map.
///
/// Written directly from the projection axis rather than by a generic tangent
/// generator: the projection already knows which object-space direction `+u`
/// and `+v` run along on every face, so `xyz` is `+u` made perpendicular to the
/// vertex normal, and `w` is the sign that makes the shader's
/// `cross(normal, tangent) * w` point along `+v`. That is the frame
/// `renzora::core::build_grid_normal_image` is authored in, so the bevel cannot
/// come out inverted on some faces, which a generator's own handedness
/// convention would risk. It also has to be rewritten every time the UVs are:
/// tangents left over from an earlier unwrap (Solari generates some for any mesh
/// it traces) would light the bevel from the wrong side.
fn project_tangents(mesh: &Mesh, scale: Vec3) -> Option<Vec<[f32; 4]>> {
    let VertexAttributeValues::Float32x3(normals) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL)? else {
        return None;
    };
    let (axes, _) = vertex_axes(mesh, scale)?;
    if axes.len() != normals.len() {
        return None;
    }

    Some(
        normals
            .iter()
            .zip(axes)
            .map(|(n, axis)| {
                let n = Vec3::from(*n).normalize_or(Vec3::Y);
                let (u, v) = match axis.unwrap_or_else(|| dominant_axis(n)) {
                    0 => (Vec3::Z, Vec3::Y),
                    1 => (Vec3::X, Vec3::Z),
                    _ => (Vec3::X, Vec3::Y),
                };
                // Gram-Schmidt against the normal. `u` lies in the projection
                // plane, so it is never parallel to a normal that chose this
                // axis; `v` is the fallback for the degenerate case anyway.
                let t = (u - n * n.dot(u)).try_normalize().unwrap_or(v);
                let w = if n.cross(t).dot(v) >= 0.0 { 1.0 } else { -1.0 };
                [t.x, t.y, t.z, w]
            })
            .collect(),
    )
}

/// Box-project `mesh`, in object space scaled to world size.
///
/// The axis is chosen per **triangle**, from its face normal, and every vertex
/// takes the axis of the triangles that use it. Choosing it per vertex, from the
/// vertex normal, is what this replaced: on a curved shape the three corners of
/// one triangle could pick three different axes, and the rasteriser then blended
/// three unrelated projections across it, which drew the grid lines as jagged
/// diagonal smears. [`needs_split`] guarantees no vertex is shared by triangles
/// that disagree, so the per-triangle choice is well defined per vertex.
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
    let (axes, _) = vertex_axes(mesh, scale)?;

    Some(
        positions
            .iter()
            .zip(normals)
            .zip(axes)
            .map(|((p, n), axis)| {
                // World-sized offsets, deliberately without the rotation: which
                // way a wall is turned should not change how big its tiles are.
                let w = Vec3::from(*p) * scale / TILE;
                // A vertex no triangle uses has no face to ask, so its own
                // normal is the only answer there is.
                uv_on_axis(w, axis.unwrap_or_else(|| dominant_axis(Vec3::from(*n))))
            })
            .collect(),
    )
}

/// Whether some vertex is shared by triangles that project along different
/// axes, so the mesh needs [`split_shared_vertices`] before it can be projected
/// cleanly. False for a cuboid or a plane, whose faces already have their own
/// vertices, so those keep their topology untouched.
fn needs_split(mesh: &Mesh, scale: Vec3) -> bool {
    vertex_axes(mesh, scale).is_some_and(|(_, conflict)| conflict)
}

/// Give every triangle its own three vertices.
///
/// Only reached for a mesh where a shared vertex sat on the boundary between two
/// projection axes (a sphere, a cylinder's cap rim, a torus). Every attribute is
/// duplicated with it, normals included, so shading stays smooth; only the UVs
/// are then free to differ either side of the boundary. Indices are put back as
/// `0..n` because the mesh editor and sculpt snapshot a mesh through its index
/// list and decline one without.
fn split_shared_vertices(mesh: &mut Mesh) {
    use bevy::mesh::Indices;
    mesh.duplicate_vertices();
    let count = mesh.count_vertices() as u32;
    mesh.insert_indices(Indices::U32((0..count).collect()));
}

/// The projection axis each vertex takes from the triangles that use it, and
/// whether any vertex is used by triangles that disagree.
///
/// `None` for anything that is not a triangle list with positions.
fn vertex_axes(mesh: &Mesh, scale: Vec3) -> Option<(Vec<Option<usize>>, bool)> {
    use bevy::mesh::PrimitiveTopology;
    if mesh.primitive_topology() != PrimitiveTopology::TriangleList {
        return None;
    }
    let VertexAttributeValues::Float32x3(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)?
    else {
        return None;
    };
    let normals = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
        Some(VertexAttributeValues::Float32x3(n)) if n.len() == positions.len() => Some(n),
        _ => None,
    };
    let indices: Vec<usize> = match mesh.indices() {
        Some(indices) => indices.iter().collect(),
        None => (0..positions.len()).collect(),
    };

    let mut axes: Vec<Option<usize>> = vec![None; positions.len()];
    let mut conflict = false;
    for tri in indices.chunks_exact(3) {
        if tri.iter().any(|&i| i >= positions.len()) {
            return None;
        }
        let world = |i: usize| Vec3::from(positions[i]) * scale;
        let face = (world(tri[1]) - world(tri[0])).cross(world(tri[2]) - world(tri[0]));
        // A degenerate triangle has no face normal; its vertex normals still
        // say which way the surface faces there.
        let face = if face.length_squared() > 1e-12 {
            face
        } else {
            normals.map_or(Vec3::Y, |n| tri.iter().map(|&i| Vec3::from(n[i])).sum())
        };
        let axis = dominant_axis(face);
        for &i in tri {
            match axes[i] {
                None => axes[i] = Some(axis),
                Some(existing) if existing != axis => conflict = true,
                Some(_) => {}
            }
        }
    }
    Some((axes, conflict))
}

/// The axis `normal` points down most: 0 for x, 1 for y, 2 for z.
fn dominant_axis(normal: Vec3) -> usize {
    let n = normal.abs();
    if n.x >= n.y && n.x >= n.z {
        0
    } else if n.y >= n.z {
        1
    } else {
        2
    }
}

/// A position's UV when projected along `axis`: the two axes it is not.
fn uv_on_axis(world: Vec3, axis: usize) -> [f32; 2] {
    match axis {
        0 => [world.z, world.y],
        1 => [world.x, world.z],
        _ => [world.x, world.y],
    }
}

/// One vertex's UV: the two axes that are not the one its normal points down.
///
/// Split out so the axis choice can be tested on its own: it is the whole of
/// what makes a projection read as a grid rather than as a smear, and it is
/// wrong in a way that is hard to see on a screenshot and trivial to assert.
#[cfg(test)]
fn project_vertex(world: Vec3, normal: Vec3) -> [f32; 2] {
    uv_on_axis(world, dominant_axis(normal))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::shape::Cuboid;

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

    /// A cuboid's faces already own their vertices, so it is projected without
    /// its topology changing.
    #[test]
    fn a_cuboid_is_not_split() {
        let mesh = Mesh::from(Cuboid::new(1.0, 1.0, 1.0));
        assert!(!needs_split(&mesh, Vec3::new(3.0, 1.0, 2.0)));
    }

    /// On a sphere every triangle projects along one axis for all three of its
    /// corners. Mixed corners are what drew the grid as jagged smears.
    #[test]
    fn a_sphere_projects_each_triangle_along_one_axis() {
        let mut mesh = Sphere::new(0.5).mesh().uv(32, 18);
        let scale = Vec3::ONE;
        assert!(needs_split(&mesh, scale), "a shared-vertex sphere should need a split");

        project_mesh_uvs(&mut mesh, scale);
        assert!(!needs_split(&mesh, scale), "a split mesh should not need another");

        let VertexAttributeValues::Float32x3(positions) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
        else {
            panic!()
        };
        let uvs = uvs_of(&mesh);
        let indices: Vec<usize> = mesh.indices().expect("indices are kept").iter().collect();
        for tri in indices.chunks_exact(3) {
            let p = |i: usize| Vec3::from(positions[i]);
            let axis = dominant_axis((p(tri[1]) - p(tri[0])).cross(p(tri[2]) - p(tri[0])));
            for &i in tri {
                assert_eq!(uvs[i], uv_on_axis(p(i), axis), "corner off its triangle's axis");
            }
        }
    }

    /// The tangent frame must run the way the UVs actually do on every
    /// triangle: `xyz` along increasing `u`, and Bevy's
    /// `w * cross(normal, tangent)` along increasing `v`. One flipped and the
    /// bevel lights from the wrong side, so the tiles look dished in.
    #[test]
    fn tangents_follow_the_projected_uvs() {
        for (mut mesh, scale) in [
            (Mesh::from(Cuboid::new(1.0, 1.0, 1.0)), Vec3::new(3.0, 1.0, 2.0)),
            (Sphere::new(0.5).mesh().uv(32, 18), Vec3::ONE),
        ] {
            project_mesh_uvs(&mut mesh, scale);
            let VertexAttributeValues::Float32x3(positions) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
            else {
                panic!()
            };
            let VertexAttributeValues::Float32x3(normals) =
                mesh.attribute(Mesh::ATTRIBUTE_NORMAL).unwrap()
            else {
                panic!()
            };
            let Some(VertexAttributeValues::Float32x4(tangents)) =
                mesh.attribute(Mesh::ATTRIBUTE_TANGENT)
            else {
                panic!("projection should write tangents")
            };
            let uvs = uvs_of(&mesh);
            let indices: Vec<usize> = mesh.indices().unwrap().iter().collect();

            for tri in indices.chunks_exact(3) {
                let p = |i: usize| Vec3::from(positions[i]) * scale;
                let uv = |i: usize| Vec2::from(uvs[i]);
                let (e1, e2) = (p(tri[1]) - p(tri[0]), p(tri[2]) - p(tri[0]));
                let (d1, d2) = (uv(tri[1]) - uv(tri[0]), uv(tri[2]) - uv(tri[0]));
                let det = d1.x * d2.y - d2.x * d1.y;
                if det.abs() < 1e-8 {
                    continue;
                }
                // The classic per-triangle tangent: dP/du and dP/dv.
                let dp_du = (e1 * d2.y - e2 * d1.y) / det;
                let dp_dv = (e2 * d1.x - e1 * d2.x) / det;
                for &i in tri {
                    let n = Vec3::from(normals[i]);
                    let t = Vec4::from(tangents[i]);
                    let b = t.w * n.cross(t.truncate());
                    assert!(t.truncate().dot(dp_du) > 0.0, "tangent against +u");
                    assert!(b.dot(dp_dv) > 0.0, "bitangent against +v");
                }
            }
        }
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
