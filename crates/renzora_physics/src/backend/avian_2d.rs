//! Avian **2D** backend — the sprite-world sibling of `backend::avian`.
//!
//! avian2d and avian3d are separate crates compiled from the same source, so
//! their `RigidBody` / `Collider` / `Time<Physics>` types are all distinct and
//! the two simulations coexist in one app without touching each other.
//! `auto_init_physics` (lib.rs) decides per entity which backend to spawn:
//! anything that is a sprite, sits under a `Node2d`, or carries the explicit
//! [`crate::Physics2d`] marker gets this backend; everything else keeps 3D.
//!
//! The same serializable `PhysicsBodyData` / `CollisionShapeData` components
//! drive both backends — the 2D mapping just reads the XY of the Vec3 fields
//! (Box → rectangle from `half_extents.xy`, Sphere → circle, Capsule →
//! capsule; Cylinder and Mesh have no 2D meaning and fall back to a circle /
//! rectangle respectively). That keeps the inspector physics section, scene
//! save format and auto-fit shared instead of forked per dimension.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::data::*;

/// Adds Avian 2D physics plugins.
pub struct Avian2dBackendPlugin {
    pub start_paused: bool,
}

impl Plugin for Avian2dBackendPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] Avian2dBackendPlugin");
        app.add_plugins(PhysicsPlugins::default());

        if self.start_paused {
            app.add_systems(Startup, pause_physics);
        }

        // The Physics panel's gravity / time-scale / substep commands are
        // drained by the 3D backend into `PhysicsPropertiesState`; mirror the
        // resulting values into the 2D resources so one panel drives both
        // simulations. (In an avian2d-only lean build the 3D drain is gone and
        // the 2D world keeps avian's defaults — acceptable until 2D grows its
        // own properties UI.)
        #[cfg(feature = "avian")]
        app.add_systems(Update, mirror_physics_properties_2d);
    }
}

fn pause_physics(mut time: ResMut<Time<Physics>>) {
    time.pause();
}

#[cfg(feature = "avian")]
fn mirror_physics_properties_2d(
    state: Res<crate::properties::PhysicsPropertiesState>,
    mut gravity: ResMut<Gravity>,
    mut time: ResMut<Time<Physics>>,
    mut substeps: ResMut<SubstepCount>,
) {
    let g = state.gravity.truncate();
    if gravity.0 != g {
        gravity.0 = g;
    }
    if time.relative_speed() != state.time_scale {
        time.set_relative_speed(state.time_scale);
    }
    if substeps.0 != state.substeps {
        substeps.0 = state.substeps;
    }
}

/// Spawn Avian 2D rigid body components from PhysicsBodyData.
pub fn spawn_physics_body(commands: &mut Commands, entity: Entity, body_data: &PhysicsBodyData) {
    let rigid_body = match body_data.body_type {
        PhysicsBodyType::RigidBody => RigidBody::Dynamic,
        PhysicsBodyType::StaticBody => RigidBody::Static,
        PhysicsBodyType::KinematicBody => RigidBody::Kinematic,
    };

    // 2D has a single rotation axis; the shared data's Z-rotation lock is the
    // one that maps onto it (X/Y rotation locks don't exist in the plane).
    let mut locked = LockedAxes::new();
    if body_data.lock_rotation_z {
        locked = locked.lock_rotation();
    }
    if body_data.lock_translation_x {
        locked = locked.lock_translation_x();
    }
    if body_data.lock_translation_y {
        locked = locked.lock_translation_y();
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

/// Spawn Avian 2D collider components from CollisionShapeData.
pub fn spawn_collision_shape(
    commands: &mut Commands,
    entity: Entity,
    shape_data: &CollisionShapeData,
) {
    let collider = match shape_data.shape_type {
        CollisionShapeType::Box => Collider::rectangle(
            shape_data.half_extents.x * 2.0,
            shape_data.half_extents.y * 2.0,
        ),
        CollisionShapeType::Sphere => Collider::circle(shape_data.radius),
        CollisionShapeType::Capsule => {
            Collider::capsule(shape_data.radius, shape_data.half_height * 2.0)
        }
        // No cylinder in a plane — its silhouette is a circle of the same radius.
        CollisionShapeType::Cylinder => Collider::circle(shape_data.radius),
        // Trimesh-from-Mesh3d has no 2D source geometry; use the fitted box.
        CollisionShapeType::Mesh => Collider::rectangle(
            shape_data.half_extents.x * 2.0,
            shape_data.half_extents.y * 2.0,
        ),
    };

    // Bake a non-zero offset into the collider itself as a one-shape compound.
    // Inserting `ColliderTransform` manually LOOKS like the way to offset a
    // shape, but avian recomputes that component every step (identity for a
    // collider on the body's own entity), so the offset was silently discarded
    // — proven by `tests/avian2d_collision.rs::offset_2d_collider_collides_at_offset`.
    let collider = if shape_data.offset.truncate() != Vec2::ZERO {
        Collider::compound(vec![(
            shape_data.offset.truncate(),
            Rotation::default(),
            collider,
        )])
    } else {
        collider
    };

    let mut entity_commands = commands.entity(entity);
    entity_commands.try_insert((
        collider,
        Friction::new(shape_data.friction),
        Restitution::new(shape_data.restitution),
    ));

    if shape_data.is_sensor {
        entity_commands.try_insert(Sensor);
    }
}

/// Remove all Avian 2D physics components from an entity.
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
