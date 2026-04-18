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
            .add_systems(Update, sync_physics_properties)
            .add_systems(Update, init_avian_mesh_colliders);
    }
}

/// Marker: this entity has `CollisionShapeData { shape_type: Mesh }` but no
/// actual avian `Collider` yet — waiting for the Mesh asset to be ready.
#[derive(Component)]
pub struct PendingMeshCollider;

/// Build `Collider::trimesh_from_mesh` from the entity's `Mesh3d` once the
/// asset loads. Runs every frame; removes the marker on success.
fn init_avian_mesh_colliders(
    mut commands: Commands,
    meshes: Res<Assets<Mesh>>,
    pending: Query<(Entity, &Mesh3d, &CollisionShapeData), With<PendingMeshCollider>>,
) {
    for (entity, mesh_ref, shape) in &pending {
        let Some(mesh) = meshes.get(&mesh_ref.0) else { continue };
        let Some(collider) = trimesh_from_mesh(mesh) else { continue };

        let mut ec = commands.entity(entity);
        ec.try_insert((
            collider,
            Friction::new(shape.friction),
            Restitution::new(shape.restitution),
        ));
        if shape.is_sensor {
            ec.try_insert(Sensor);
        }
        ec.remove::<PendingMeshCollider>();
    }
}

/// Build an avian trimesh `Collider` from a Bevy `Mesh` (TriangleList only).
fn trimesh_from_mesh(mesh: &Mesh) -> Option<Collider> {
    use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
    if mesh.primitive_topology() != PrimitiveTopology::TriangleList {
        return None;
    }
    let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION)? {
        VertexAttributeValues::Float32x3(v) => v,
        _ => return None,
    };
    let vertices: Vec<Vec3> = positions.iter().map(|p| Vec3::new(p[0], p[1], p[2])).collect();
    let indices: Vec<[u32; 3]> = match mesh.indices()? {
        Indices::U32(idx) => idx.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect(),
        Indices::U16(idx) => idx.chunks_exact(3).map(|c| [c[0] as u32, c[1] as u32, c[2] as u32]).collect(),
    };
    if indices.is_empty() { return None; }
    Some(Collider::trimesh(vertices, indices))
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

    commands.entity(entity).try_insert((
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
        CollisionShapeType::Mesh => {
            // Built later by `init_avian_mesh_colliders`, which has access to Assets<Mesh>.
            commands.entity(entity).try_insert(PendingMeshCollider);
            return;
        }
    };

    let mut entity_commands = commands.entity(entity);

    if shape_data.offset != Vec3::ZERO {
        entity_commands.try_insert((
            collider,
            ColliderTransform::from(Transform::from_translation(shape_data.offset)),
            Friction::new(shape_data.friction),
            Restitution::new(shape_data.restitution),
        ));
    } else {
        entity_commands.try_insert((
            collider,
            Friction::new(shape_data.friction),
            Restitution::new(shape_data.restitution),
        ));
    }

    if shape_data.is_sensor {
        entity_commands.try_insert(Sensor);
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

