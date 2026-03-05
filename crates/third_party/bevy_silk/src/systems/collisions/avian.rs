#![allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::option_if_let_else,
    clippy::suboptimal_flops
)]
use crate::components::{cloth::Cloth, collider::ClothCollider};
use avian3d::prelude::*;
use bevy::{camera::primitives::Aabb, log, prelude::*};

fn get_collider(aabb: &Aabb, collider: &ClothCollider) -> Collider {
    let extents = aabb.half_extents * 2.0 + collider.offset;
    Collider::compound(vec![(
        Position(aabb.center.into()),
        Quat::IDENTITY,
        Collider::cuboid(extents.x, extents.y, extents.z),
    )])
}

pub fn handle_collisions(
    mut cloth_query: Query<(Entity, &mut Cloth, &Aabb, &ClothCollider, &mut Collider)>,
    collisions: Collisions,
    mut colliders_query: Query<
        (
            &Collider,
            &GlobalTransform,
            Option<&mut LinearVelocity>,
            Option<&mut AngularVelocity>,
        ),
        Without<Cloth>,
    >,
    time: Res<Time>,
) {
    let delta_time = time.delta_secs();
    for (entity, mut cloth, aabb, collider, mut avian_collider) in &mut cloth_query {
        for contact_pair in collisions.collisions_with(entity) {
            let other_entity = if contact_pair.collider1 == entity {
                contact_pair.collider2
            } else {
                contact_pair.collider1
            };
            let Ok((
                other_collider,
                other_transform,
                other_linear_velocity,
                other_angular_velocity,
            )) = colliders_query.get_mut(other_entity)
            else {
                log::error!("Couldn't find collider on entity {:?}", entity);
                continue;
            };
            let vel = other_linear_velocity.as_ref().map_or(0.0, |velocity| {
                velocity.length_squared() * delta_time * delta_time * collider.velocity_coefficient
            });
            cloth.solve_collisions(|point| {
                let other_transform = other_transform.compute_transform();
                // TODO: Remove Nalgebra type conversions once avian has
                //       a `Collider::project_point` method that uses Glam.
                let projection = other_collider.shape_scaled().project_point(
                    &avian3d::parry::math::Isometry::new(
                        other_transform.translation.into(),
                        other_transform.rotation.to_scaled_axis().into(),
                    ),
                    &(*point).into(),
                    false,
                );
                let projected_point = Vec3::from(projection.point);
                let normal: Vec3 = (projected_point - *point)
                    .try_normalize()
                    .unwrap_or(Vec3::Y);

                if projection.is_inside {
                    Some(projected_point + (normal * collider.offset) + (normal * vel))
                } else if point.distance_squared(projected_point)
                    < collider.offset * collider.offset
                {
                    Some(projected_point - (normal * collider.offset))
                } else {
                    None
                }
            });
            if let Some(((ref mut lin_vel, ref mut ang_vel), dampen_coef)) = other_linear_velocity
                .zip(other_angular_velocity)
                .zip(collider.dampen_others)
            {
                let damp = 1.0 - dampen_coef;
                lin_vel.0 *= damp;
                ang_vel.0 *= damp;
            }
        }
        *avian_collider = get_collider(aabb, collider);
    }
}

pub fn init_cloth_collider(
    mut commands: Commands,
    cloth_query: Query<(Entity, &Aabb, &ClothCollider), (With<Cloth>, Without<Collider>)>,
) {
    for (entity, aabb, collider) in cloth_query.iter() {
        log::debug!("Initializing Cloth collisions for {:?}", entity);
        commands.entity(entity).insert((
            RigidBody::Kinematic,
            Sensor,
            get_collider(aabb, collider),
        ));
    }
}
