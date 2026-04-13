#![allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::option_if_let_else,
    clippy::suboptimal_flops
)]
use crate::components::{cloth::Cloth, collider::ClothCollider};
use bevy::{camera::primitives::Aabb, log, prelude::*};
use bevy_rapier3d::prelude::*;

fn get_collider(aabb: &Aabb, collider: &ClothCollider) -> Collider {
    let extents = aabb.half_extents + collider.offset;
    Collider::compound(vec![(
        aabb.center.into(),
        Quat::IDENTITY,
        Collider::cuboid(extents.x, extents.y, extents.z),
    )])
}

pub fn handle_collisions(
    mut cloth_query: Query<(
        Entity,
        &mut Cloth,
        &Aabb,
        &ClothCollider,
        &mut Collider,
        Option<&RapierContextEntityLink>,
    )>,
    default_rapier_context: ReadRapierContext<With<DefaultRapierContext>>,
    rapier_contexts: ReadRapierContext<Without<DefaultRapierContext>>,
    mut colliders_query: Query<
        (&Collider, &GlobalTransform, Option<&mut Velocity>),
        Without<Cloth>,
    >,
    time: Res<Time>,
) {
    let Ok(default_context) = default_rapier_context.single() else {
        panic!("No default rapier context set up");
    };
    let delta_time = time.delta_secs();
    for (entity, mut cloth, aabb, collider, mut rapier_collider, context_link) in &mut cloth_query {
        let linked_context = context_link.and_then(|l| {
            rapier_contexts.rapier_context.get(l.0).ok().map(
                |(simulation, colliders, joints, rigidbody_set)| RapierContext {
                    simulation,
                    colliders,
                    joints,
                    rigidbody_set,
                },
            )
        });
        let context = match linked_context.as_ref() {
            Some(ctx) => ctx,
            None => &default_context,
        };
        for contact_pair in context.contact_pairs_with(entity) {
            let other_entity = if contact_pair.collider1() == Some(entity) {
                contact_pair.collider2()
            } else {
                contact_pair.collider1()
            };
            let Some(other_entity) = other_entity else {
                log::error!("Failed to retrieve contact pairs from rapier");
                continue;
            };
            let Ok((other_collider, other_transform, other_velocity)) =
                colliders_query.get_mut(other_entity)
            else {
                log::error!("Couldn't find collider on entity {:?}", entity);
                continue;
            };
            let vel = other_velocity.as_ref().map_or(0.0, |v| {
                v.linvel.length_squared() * delta_time * delta_time * collider.velocity_coefficient
            });
            cloth.solve_collisions(|point| {
                let other_transform = other_transform.compute_transform();
                let projected_point = other_collider.project_point(
                    other_transform.translation,
                    other_transform.rotation,
                    *point,
                    false,
                );
                let normal: Vec3 = (projected_point.point - *point)
                    .try_normalize()
                    .unwrap_or(Vec3::Y);
                if projected_point.is_inside {
                    Some(projected_point.point + (normal * collider.offset) + (normal * vel))
                } else if point.distance_squared(projected_point.point)
                    < collider.offset * collider.offset
                {
                    Some(projected_point.point - (normal * collider.offset))
                } else {
                    None
                }
            });
            if let Some((ref mut vel, dampen_coef)) = other_velocity.zip(collider.dampen_others) {
                let damp = 1.0 - dampen_coef;
                vel.linvel *= damp;
                vel.angvel *= damp;
            }
        }
        *rapier_collider = get_collider(aabb, collider);
    }
}

pub fn init_cloth_collider(
    mut commands: Commands,
    cloth_query: Query<(Entity, &Aabb, &ClothCollider), (With<Cloth>, Without<Collider>)>,
) {
    for (entity, aabb, collider) in cloth_query.iter() {
        log::debug!("Initializing Cloth collisions for {:?}", entity);
        commands.entity(entity).insert((
            RigidBody::KinematicPositionBased,
            get_collider(aabb, collider),
            SolverGroups::new(Group::NONE, Group::NONE),
        ));
    }
}
