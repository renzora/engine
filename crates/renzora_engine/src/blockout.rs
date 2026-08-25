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
use renzora::core::{GridTexture, MeshPrimitive};

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

/// The object scale currently baked into an entity's mesh UVs by
/// [`retile_blockout_grid`].
///
/// Not registered for reflection on purpose: it describes the state of a mesh
/// asset, not the scene, and a saved scene rebuilds its meshes from the shape
/// registry on load — pristine, so the absence of this component correctly
/// means "nothing baked in yet".
#[derive(Component)]
pub struct BlockoutTiling(Vec3);

/// Keep the blockout grid a constant size in world units as an object is
/// scaled, instead of stretching four cells across whatever the object has
/// become.
///
/// A primitive's UVs run 0..1 across a face, so scaling a cube into a wall
/// stretched its four cells into four tall rectangles — the grid stopped
/// reading as a measure of anything, which is the entire reason to have it.
/// Scaling the UVs to match puts the tiles back to square and keeps them the
/// same size as the tiles on everything else in the scene.
///
/// The stretch factor has to be per-UV-axis, so this needs to know which way
/// through the model `+u` and `+v` actually run — see [`uv_axes`]. On a cube
/// that comes out exact per face; on a sphere the axes swing around, so the
/// tiling varies a little under a non-uniform scale, which is a far smaller
/// artefact than the stretching it replaces.
///
/// Entities with a [`MaterialRef`](renzora::core::MaterialRef) are left alone —
/// once a real material is on the mesh, its UVs are the author's business.
pub fn retile_blockout_grid(
    mut commands: Commands,
    query: Query<
        (
            Entity,
            Ref<Mesh3d>,
            Ref<GlobalTransform>,
            Option<&BlockoutTiling>,
        ),
        (
            With<MeshPrimitive>,
            Without<renzora::core::MaterialRef>,
            Without<renzora::core::EditedMesh>,
        ),
    >,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    for (entity, mesh3d, transform, tiling) in &query {
        if !mesh3d.is_changed() && !transform.is_changed() {
            continue;
        }
        // A fresh `Mesh3d` is a fresh mesh asset straight from the shape
        // registry, so whatever was baked into the last one is gone with it.
        let baked = if mesh3d.is_changed() {
            Vec3::ONE
        } else {
            tiling.map_or(Vec3::ONE, |t| t.0)
        };
        let scale = transform.scale().abs().max(Vec3::splat(1e-4));
        if scale.abs_diff_eq(baked, 1e-4) {
            continue;
        }

        let Some(mut mesh) = meshes.get_mut(&mesh3d.0) else {
            continue;
        };
        let Some((u_axes, v_axes)) = uv_axes(&mesh) else {
            continue;
        };
        let Some(VertexAttributeValues::Float32x2(uvs)) =
            mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0)
        else {
            continue;
        };
        if uvs.len() != u_axes.len() {
            continue;
        }
        for (i, uv) in uvs.iter_mut().enumerate() {
            // How far one unit of UV stretches in the world, before and after.
            // The ratio is what has to be applied, because the UVs already
            // carry the previous scale.
            uv[0] *= stretch(u_axes[i], scale) / stretch(u_axes[i], baked);
            uv[1] *= stretch(v_axes[i], scale) / stretch(v_axes[i], baked);
        }
        commands.entity(entity).try_insert(BlockoutTiling(scale));
    }
}

/// How much a unit-length object-space direction grows under `scale`.
fn stretch(axis: Vec3, scale: Vec3) -> f32 {
    let s = (axis * scale).length();
    if s > 1e-4 {
        s
    } else {
        // A degenerate axis (a vertex the triangle walk found nothing for)
        // falls back to the average, which at least keeps tiles square.
        (scale.x + scale.y + scale.z) / 3.0
    }
}

/// Object-space directions that `+u` and `+v` run in at each vertex.
///
/// This is a tangent frame, but computed here and thrown away rather than
/// stored as a vertex attribute: nothing *renders* with it — the grid is flat
/// and normal-maps nothing — it is only needed to know which way a UV axis runs
/// through the model, so the retiling stretches the right one.
fn uv_axes(mesh: &Mesh) -> Option<(Vec<Vec3>, Vec<Vec3>)> {
    let VertexAttributeValues::Float32x3(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)?
    else {
        return None;
    };
    let VertexAttributeValues::Float32x2(uvs) = mesh.attribute(Mesh::ATTRIBUTE_UV_0)? else {
        return None;
    };
    let indices: Vec<usize> = mesh.indices()?.iter().collect();

    let mut u_axes = vec![Vec3::ZERO; positions.len()];
    let mut v_axes = vec![Vec3::ZERO; positions.len()];
    for tri in indices.chunks_exact(3) {
        let (a, b, c) = (tri[0], tri[1], tri[2]);
        if a.max(b).max(c) >= positions.len() {
            return None;
        }
        let edge1 = Vec3::from(positions[b]) - Vec3::from(positions[a]);
        let edge2 = Vec3::from(positions[c]) - Vec3::from(positions[a]);
        let duv1 = Vec2::from(uvs[b]) - Vec2::from(uvs[a]);
        let duv2 = Vec2::from(uvs[c]) - Vec2::from(uvs[a]);
        let det = duv1.x * duv2.y - duv2.x * duv1.y;
        if det.abs() < 1e-12 {
            continue; // degenerate UVs on this triangle — the others will do
        }
        let r = 1.0 / det;
        let u = (edge1 * duv2.y - edge2 * duv1.y) * r;
        let v = (edge2 * duv1.x - edge1 * duv2.x) * r;
        // Normalize per triangle before accumulating, so a large triangle
        // doesn't outvote its neighbours on a shared vertex.
        let (u, v) = (u.normalize_or_zero(), v.normalize_or_zero());
        for &i in tri {
            u_axes[i] += u;
            v_axes[i] += v;
        }
    }
    for (u, v) in u_axes.iter_mut().zip(v_axes.iter_mut()) {
        *u = u.normalize_or_zero();
        *v = v.normalize_or_zero();
    }
    Some((u_axes, v_axes))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On a cuboid every face's UV axes are two of the cardinal axes, which is
    /// what makes the per-face retiling exact: scaling a cube into a wall
    /// stretches each face's `u` and `v` by the two world dimensions that face
    /// actually spans.
    #[test]
    fn cuboid_uv_axes_are_cardinal_and_orthogonal() {
        let mesh = Mesh::from(Cuboid::new(1.0, 1.0, 1.0));
        let (u_axes, v_axes) = uv_axes(&mesh).expect("cuboid has positions, uvs and indices");

        for (u, v) in u_axes.iter().zip(v_axes.iter()) {
            assert!((u.length() - 1.0).abs() < 1e-4, "u should be unit: {u}");
            assert!((v.length() - 1.0).abs() < 1e-4, "v should be unit: {v}");
            assert!(u.dot(*v).abs() < 1e-4, "u and v should be perpendicular");
            // Cardinal means one component is ±1 and the rest are zero.
            for axis in [u, v] {
                assert!(
                    (axis.abs().max_element() - 1.0).abs() < 1e-4,
                    "axis should be cardinal: {axis}"
                );
            }
        }
    }

    /// The whole point: a wall made by scaling a cube gets tiles the same size
    /// as everything else, not four stretched rectangles. Each face's two UV
    /// axes must pick up the two scale components that face spans.
    #[test]
    fn stretch_follows_the_axis_it_runs_along() {
        let scale = Vec3::new(6.0, 3.0, 1.0);
        assert_eq!(stretch(Vec3::X, scale), 6.0);
        assert_eq!(stretch(Vec3::Y, scale), 3.0);
        assert_eq!(stretch(-Vec3::Z, scale), 1.0);
        // Every axis a cuboid's UVs can run along is covered by exactly one
        // scale component, so no face can come out non-square.
        let mesh = Mesh::from(Cuboid::new(1.0, 1.0, 1.0));
        let (u_axes, v_axes) = uv_axes(&mesh).unwrap();
        for (u, v) in u_axes.iter().zip(v_axes.iter()) {
            assert!([6.0, 3.0, 1.0].contains(&stretch(*u, scale)));
            assert!([6.0, 3.0, 1.0].contains(&stretch(*v, scale)));
        }
    }

    /// A vertex the triangle walk found nothing for must not collapse the UVs
    /// to zero — that would put the whole grid in one texel.
    #[test]
    fn degenerate_axis_falls_back_to_the_average_scale() {
        assert_eq!(stretch(Vec3::ZERO, Vec3::new(2.0, 3.0, 4.0)), 3.0);
    }
}
