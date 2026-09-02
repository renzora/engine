//! Where the gizmo sits on a selection.

use bevy::prelude::*;

/// Return the world-space pivot to anchor the gizmo on for `entity`.
///
/// Many GLBs (e.g. scenes exported from Blender or assembled in DCCs) author
/// every mesh node with its origin at world (0,0,0) and bake the actual
/// position into the vertex data. Anchoring on `GlobalTransform.translation`
/// would put the gizmo at the world origin instead of on top of the mesh —
/// which is what users hit when dropping large scene GLBs into the editor.
///
/// We instead compute the world-space AABB over the entity's mesh and every
/// descendant mesh, falling back to the entity's transform if no AABBs are
/// available yet (e.g. just-spawned entities before mesh load).
///
/// `bottom` anchors on the base of those bounds instead of their middle
/// (`ViewportSettings::gizmo_pivot_bottom`, on by default). The middle is where
/// the handles float in mid-air on anything that stands on the ground; the base
/// is where the object meets the floor, which is where you want to grab it. It
/// is also the drag pivot, so rotate and scale turn about the base and the
/// object stays standing rather than sinking through the surface.
pub(crate) fn compute_gizmo_pivot(
    entity: Entity,
    aabbs: &Query<(Option<&bevy::camera::primitives::Aabb>, &GlobalTransform), With<Mesh3d>>,
    children: &Query<&Children>,
    fallback_gt: &GlobalTransform,
    bottom: bool,
) -> Vec3 {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    collect_pivot_aabb(entity, aabbs, children, &mut min, &mut max);
    if min.x <= max.x {
        let centre = (min + max) * 0.5;
        if bottom {
            Vec3::new(centre.x, min.y, centre.z)
        } else {
            centre
        }
    } else {
        // No bounds to sit on the base of — the origin is all there is.
        fallback_gt.translation()
    }
}

fn collect_pivot_aabb(
    entity: Entity,
    aabbs: &Query<(Option<&bevy::camera::primitives::Aabb>, &GlobalTransform), With<Mesh3d>>,
    children: &Query<&Children>,
    min: &mut Vec3,
    max: &mut Vec3,
) {
    if let Ok((Some(aabb), gt)) = aabbs.get(entity) {
        let c = Vec3::from(aabb.center);
        let h = Vec3::from(aabb.half_extents);
        for sx in [-1.0_f32, 1.0] {
            for sy in [-1.0_f32, 1.0] {
                for sz in [-1.0_f32, 1.0] {
                    let corner = gt.transform_point(c + h * Vec3::new(sx, sy, sz));
                    *min = min.min(corner);
                    *max = max.max(corner);
                }
            }
        }
    }
    if let Ok(kids) = children.get(entity) {
        for child in kids.iter() {
            collect_pivot_aabb(child, aabbs, children, min, max);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::primitives::Aabb;
    use bevy::ecs::system::RunSystemOnce;

    fn run_compute_pivot(
        world: &mut World,
        entity: Entity,
        fallback: GlobalTransform,
        bottom: bool,
    ) -> Vec3 {
        world
            .run_system_once(
                move |aabbs: Query<(Option<&Aabb>, &GlobalTransform), With<Mesh3d>>,
                      children: Query<&Children>| {
                    compute_gizmo_pivot(entity, &aabbs, &children, &fallback, bottom)
                },
            )
            .unwrap()
    }

    #[test]
    fn compute_gizmo_pivot_uses_world_aabb_center() {
        let mut meshes = Assets::<Mesh>::default();
        let mesh = meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)));

        let mut world = World::new();
        let entity = world
            .spawn((
                Mesh3d(mesh),
                Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0)),
                GlobalTransform::from_translation(Vec3::new(10.0, 2.0, 0.0)),
            ))
            .id();
        // Pivot anchors on the mesh AABB, not the (bogus) fallback transform.
        let fallback = GlobalTransform::from_translation(Vec3::splat(99.0));
        let pivot = run_compute_pivot(&mut world, entity, fallback, false);
        assert!((pivot - Vec3::new(10.0, 2.0, 0.0)).length() < 1e-4, "got {pivot}");
    }

    #[test]
    fn compute_gizmo_pivot_includes_descendant_meshes() {
        let mut meshes = Assets::<Mesh>::default();
        let mesh = meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)));

        let mut world = World::new();
        // Parent has no mesh of its own — pivot must come from the child,
        // matching the scene-GLB case where the root sits at the origin.
        let parent = world.spawn(Name::new("Root")).id();
        world.spawn((
            Mesh3d(mesh),
            Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0)),
            GlobalTransform::from_translation(Vec3::new(0.0, 0.0, -6.0)),
            ChildOf(parent),
        ));
        let fallback = GlobalTransform::IDENTITY;
        let pivot = run_compute_pivot(&mut world, parent, fallback, false);
        assert!((pivot - Vec3::new(0.0, 0.0, -6.0)).length() < 1e-4, "got {pivot}");
    }

    #[test]
    fn compute_gizmo_pivot_falls_back_without_aabbs() {
        let mut world = World::new();
        let entity = world.spawn(Name::new("JustSpawned")).id();
        let fallback = GlobalTransform::from_translation(Vec3::new(1.0, 2.0, 3.0));
        let pivot = run_compute_pivot(&mut world, entity, fallback, false);
        assert_eq!(pivot, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn compute_gizmo_pivot_bottom_anchors_on_the_aabb_base() {
        let mut meshes = Assets::<Mesh>::default();
        let mesh = meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)));

        let mut world = World::new();
        // Unit-radius AABB centred at y = 2 → base sits at y = 1, and X/Z stay
        // on the centre so the handles keep straddling the object.
        let entity = world
            .spawn((
                Mesh3d(mesh),
                Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0)),
                GlobalTransform::from_translation(Vec3::new(10.0, 2.0, -3.0)),
            ))
            .id();
        let fallback = GlobalTransform::from_translation(Vec3::splat(99.0));
        let pivot = run_compute_pivot(&mut world, entity, fallback, true);
        assert!((pivot - Vec3::new(10.0, 1.0, -3.0)).length() < 1e-4, "got {pivot}");
    }

    #[test]
    fn compute_gizmo_pivot_bottom_uses_the_lowest_descendant() {
        let mut meshes = Assets::<Mesh>::default();
        let mesh = meshes.add(Mesh::from(Cuboid::new(1.0, 1.0, 1.0)));

        let mut world = World::new();
        // Two children at different heights: the base must come from the lower
        // one, not from whichever mesh is visited first.
        let parent = world.spawn(Name::new("Root")).id();
        world.spawn((
            Mesh3d(mesh.clone()),
            Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0)),
            GlobalTransform::from_translation(Vec3::new(0.0, 8.0, 0.0)),
            ChildOf(parent),
        ));
        world.spawn((
            Mesh3d(mesh),
            Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0)),
            GlobalTransform::from_translation(Vec3::new(0.0, 3.0, 0.0)),
            ChildOf(parent),
        ));
        let fallback = GlobalTransform::IDENTITY;
        let pivot = run_compute_pivot(&mut world, parent, fallback, true);
        // Combined bounds span y = 2..9 → centre y = 5.5, base y = 2.
        assert!((pivot.y - 2.0).abs() < 1e-4, "got {pivot}");
        let centre = run_compute_pivot(&mut world, parent, fallback, false);
        assert!((centre.y - 5.5).abs() < 1e-4, "got {centre}");
    }

    #[test]
    fn compute_gizmo_pivot_bottom_falls_back_without_aabbs() {
        // No bounds to sit on the base of — `bottom` must not change the
        // fallback, or a just-spawned entity would snap to y = 0.
        let mut world = World::new();
        let entity = world.spawn(Name::new("JustSpawned")).id();
        let fallback = GlobalTransform::from_translation(Vec3::new(1.0, 2.0, 3.0));
        let pivot = run_compute_pivot(&mut world, entity, fallback, true);
        assert_eq!(pivot, Vec3::new(1.0, 2.0, 3.0));
    }
}
