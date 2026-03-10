use avian3d::prelude::*;
use bevy::prelude::*;

use crate::data::*;
use crate::properties::*;

/// Adds Avian 3D physics plugins and backend-specific systems.
pub struct AvianBackendPlugin {
    pub start_paused: bool,
}

impl Plugin for AvianBackendPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] AvianBackendPlugin");
        app.add_plugins(PhysicsPlugins::default());

        if self.start_paused {
            app.add_systems(Startup, pause_physics);
        }

        app.init_resource::<PhysicsPropertiesState>()
            .add_systems(Update, sync_physics_properties);
    }
}

fn pause_physics(mut time: ResMut<Time<Physics>>) {
    time.pause();
}

pub fn physics_pause(time: &mut ResMut<Time<Physics>>) {
    time.pause();
}

pub fn physics_unpause(time: &mut ResMut<Time<Physics>>) {
    time.unpause();
}

pub fn physics_is_paused(time: &Res<Time<Physics>>) -> bool {
    time.is_paused()
}

/// Spawn Avian rigid body components from PhysicsBodyData.
pub fn spawn_physics_body(commands: &mut Commands, entity: Entity, body_data: &PhysicsBodyData) {
    let rigid_body = match body_data.body_type {
        PhysicsBodyType::RigidBody => RigidBody::Dynamic,
        PhysicsBodyType::StaticBody => RigidBody::Static,
        PhysicsBodyType::KinematicBody => RigidBody::Kinematic,
    };

    let mut locked = LockedAxes::new();
    if body_data.lock_rotation_x {
        locked = locked.lock_rotation_x();
    }
    if body_data.lock_rotation_y {
        locked = locked.lock_rotation_y();
    }
    if body_data.lock_rotation_z {
        locked = locked.lock_rotation_z();
    }
    if body_data.lock_translation_x {
        locked = locked.lock_translation_x();
    }
    if body_data.lock_translation_y {
        locked = locked.lock_translation_y();
    }
    if body_data.lock_translation_z {
        locked = locked.lock_translation_z();
    }

    commands.entity(entity).insert((
        rigid_body,
        Mass(body_data.mass),
        GravityScale(body_data.gravity_scale),
        LinearDamping(body_data.linear_damping),
        AngularDamping(body_data.angular_damping),
        locked,
    ));
}

/// Spawn Avian collider components from CollisionShapeData.
pub fn spawn_collision_shape(commands: &mut Commands, entity: Entity, shape_data: &CollisionShapeData) {
    let collider = match shape_data.shape_type {
        CollisionShapeType::Box => Collider::cuboid(
            shape_data.half_extents.x * 2.0,
            shape_data.half_extents.y * 2.0,
            shape_data.half_extents.z * 2.0,
        ),
        CollisionShapeType::Sphere => Collider::sphere(shape_data.radius),
        CollisionShapeType::Capsule => {
            Collider::capsule(shape_data.radius, shape_data.half_height * 2.0)
        }
        CollisionShapeType::Cylinder => {
            Collider::cylinder(shape_data.radius, shape_data.half_height * 2.0)
        }
    };

    let mut entity_commands = commands.entity(entity);

    if shape_data.offset != Vec3::ZERO {
        entity_commands.insert((
            collider,
            ColliderTransform::from(Transform::from_translation(shape_data.offset)),
            Friction::new(shape_data.friction),
            Restitution::new(shape_data.restitution),
        ));
    } else {
        entity_commands.insert((
            collider,
            Friction::new(shape_data.friction),
            Restitution::new(shape_data.restitution),
        ));
    }

    if shape_data.is_sensor {
        entity_commands.insert(Sensor);
    }
}

/// Remove all Avian physics components from an entity.
pub fn despawn_physics_components(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .remove::<RigidBody>()
        .remove::<Mass>()
        .remove::<GravityScale>()
        .remove::<LinearDamping>()
        .remove::<AngularDamping>()
        .remove::<LockedAxes>()
        .remove::<Collider>()
        .remove::<ColliderTransform>()
        .remove::<Friction>()
        .remove::<Restitution>()
        .remove::<Sensor>()
        .remove::<LinearVelocity>()
        .remove::<AngularVelocity>()
        .remove::<ConstantForce>()
        .remove::<ConstantTorque>();
}

fn sync_physics_properties(
    mut state: ResMut<PhysicsPropertiesState>,
    mut gravity: ResMut<Gravity>,
    mut time: ResMut<Time<Physics>>,
    mut substeps: ResMut<SubstepCount>,
) {
    let cmds: Vec<PhysicsPropertyCommand> = state.commands.drain(..).collect();
    for cmd in cmds {
        match cmd {
            PhysicsPropertyCommand::SetGravity(g) => {
                gravity.0 = g;
                state.gravity = g;
                state.gravity_preset = GravityPreset::Custom;
            }
            PhysicsPropertyCommand::SetGravityPreset(preset) => {
                let g = preset.gravity_vec();
                gravity.0 = g;
                state.gravity = g;
                state.gravity_preset = preset;
            }
            PhysicsPropertyCommand::SetTimeScale(scale) => {
                time.set_relative_speed(scale);
                state.time_scale = scale;
            }
            PhysicsPropertyCommand::SetSubsteps(count) => {
                substeps.0 = count;
                state.substeps = count;
            }
            PhysicsPropertyCommand::ResetAll => {
                let default_g = GravityPreset::Earth.gravity_vec();
                gravity.0 = default_g;
                state.gravity = default_g;
                state.gravity_preset = GravityPreset::Earth;
                time.set_relative_speed(1.0);
                state.time_scale = 1.0;
                substeps.0 = 6;
                state.substeps = 6;
            }
        }
    }

    if state.commands.is_empty() {
        state.gravity = gravity.0;
        state.substeps = substeps.0;
    }
}

