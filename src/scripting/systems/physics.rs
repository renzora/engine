//! Physics command processing system
//!
//! Processes queued physics commands from scripts using Avian components.

use bevy::prelude::*;
use bevy::ecs::system::ParamSet;

#[cfg(feature = "physics")]
use avian3d::prelude::*;

use crate::scripting::resources::{PhysicsCommand, PhysicsCommandQueue, RaycastHit, RaycastResults};

/// System to process queued physics commands
///
/// Note: Avian uses a different API than Rapier:
/// - Forces are applied via the `Forces` QueryData helper or `ConstantForce` component
/// - Linear/Angular velocities are accessed directly via their components
///
/// We use ParamSet because Forces internally accesses LinearVelocity/AngularVelocity,
/// which would conflict with our separate velocity queries.
#[cfg(feature = "physics")]
pub fn process_physics_commands(
    mut queue: ResMut<PhysicsCommandQueue>,
    mut raycast_results: ResMut<RaycastResults>,
    // ParamSet to handle conflicting queries - Forces accesses velocities internally
    mut physics_params: ParamSet<(
        Query<Forces>,                    // p0: For force/impulse/torque
        Query<&mut LinearVelocity>,       // p1: For set velocity
        Query<&mut AngularVelocity>,      // p2: For set angular velocity
    )>,
    mut gravity_scales: Query<&mut GravityScale>,
    spatial_query: SpatialQuery,
    mut commands: Commands,
) {
    if queue.is_empty() {
        return;
    }

    // Collect commands to process (drain returns Vec already)
    let cmds = queue.drain();

    for cmd in cmds {
        match cmd {
            PhysicsCommand::ApplyForce { entity, force } => {
                // Use Avian's Forces QueryData for one-time force application
                if let Ok(mut forces) = physics_params.p0().get_mut(entity) {
                    forces.apply_force(force);
                } else {
                    // Entity doesn't have physics components yet - add constant force
                    commands.entity(entity).insert(ConstantForce::new(force.x, force.y, force.z));
                }
            }

            PhysicsCommand::ApplyImpulse { entity, impulse } => {
                // Impulses are applied via Forces::apply_linear_impulse
                if let Ok(mut forces) = physics_params.p0().get_mut(entity) {
                    forces.apply_linear_impulse(impulse);
                } else {
                    // Can't apply impulse without physics - set velocity instead
                    commands.entity(entity).insert(LinearVelocity(impulse));
                }
            }

            PhysicsCommand::ApplyTorque { entity, torque } => {
                if let Ok(mut forces) = physics_params.p0().get_mut(entity) {
                    forces.apply_torque(torque);
                } else {
                    commands.entity(entity).insert(ConstantTorque::new(torque.x, torque.y, torque.z));
                }
            }

            PhysicsCommand::SetVelocity { entity, velocity } => {
                if let Ok(mut lin_vel) = physics_params.p1().get_mut(entity) {
                    lin_vel.0 = velocity;
                } else {
                    commands.entity(entity).insert(LinearVelocity(velocity));
                }
            }

            PhysicsCommand::SetAngularVelocity { entity, velocity } => {
                if let Ok(mut ang_vel) = physics_params.p2().get_mut(entity) {
                    ang_vel.0 = velocity;
                } else {
                    commands.entity(entity).insert(AngularVelocity(velocity));
                }
            }

            PhysicsCommand::SetGravityScale { entity, scale } => {
                if let Ok(mut grav_scale) = gravity_scales.get_mut(entity) {
                    grav_scale.0 = scale;
                } else {
                    commands.entity(entity).insert(GravityScale(scale));
                }
            }

            PhysicsCommand::Raycast {
                origin,
                direction,
                max_distance,
                requester,
                result_var,
            } => {
                let dir = Dir3::new(direction.normalize()).unwrap_or(Dir3::NEG_Z);

                // Perform raycast using Avian's SpatialQuery
                if let Some(hit) = spatial_query.cast_ray(
                    origin,
                    dir,
                    max_distance,
                    true, // solid
                    &SpatialQueryFilter::default(),
                ) {
                    let hit_point = origin + direction.normalize() * hit.distance;
                    raycast_results.results.insert(
                        (requester, result_var),
                        RaycastHit {
                            hit: true,
                            entity: Some(hit.entity),
                            point: hit_point,
                            normal: hit.normal,
                            distance: hit.distance,
                        },
                    );
                } else {
                    // No hit
                    raycast_results
                        .results
                        .insert((requester, result_var), RaycastHit::default());
                }
            }
        }
    }
}

/// Stub system when physics is disabled
#[cfg(not(feature = "physics"))]
pub fn process_physics_commands(
    mut queue: ResMut<PhysicsCommandQueue>,
) {
    // Just drain the queue - physics disabled
    if !queue.is_empty() {
        let _ = queue.drain();
        warn!("Physics commands ignored - physics feature not enabled");
    }
}
