//! Auto-fit must be rotation-invariant. A unit cube mesh under a rotated,
//! non-uniformly scaled entity already fits the default 0.5 box exactly; the old
//! fit took a world-axis AABB and mapped it back, which grew a 15 degree Y
//! platform's collider to roughly 0.70 x 0.5 x 0.81 every time the scene loaded.

use bevy::camera::primitives::Aabb;
use bevy::math::Vec3A;
use bevy::prelude::*;
use renzora_physics::auto_fit::{auto_fit_collision_shapes, mark_new_collision_shapes};
use renzora_physics::CollisionShapeData;

#[test]
fn rotated_unit_cube_keeps_default_extents() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_systems(Update, (mark_new_collision_shapes, auto_fit_collision_shapes).chain());

    let tf = Transform {
        translation: Vec3::new(7.0, 1.0, 0.0),
        rotation: Quat::from_rotation_y(15f32.to_radians()),
        scale: Vec3::new(5.0, 0.5, 4.0),
    };
    let e = app
        .world_mut()
        .spawn((
            tf,
            GlobalTransform::from(tf),
            Aabb { center: Vec3A::ZERO, half_extents: Vec3A::splat(0.5) },
            CollisionShapeData::default(),
        ))
        .id();

    for _ in 0..3 {
        app.update();
    }

    let shape = app.world().get::<CollisionShapeData>(e).unwrap();
    assert!(
        shape.half_extents.abs_diff_eq(Vec3::splat(0.5), 1e-4),
        "half extents changed to {:?}",
        shape.half_extents
    );
    assert!(shape.offset.abs_diff_eq(Vec3::ZERO, 1e-4), "offset changed to {:?}", shape.offset);
}
