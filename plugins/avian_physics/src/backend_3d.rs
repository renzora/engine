//! The 3D world on avian3d.

use std::collections::HashMap;
use std::sync::Mutex;

use avian3d::prelude::*;
use bevy::ecs::system::SystemState;
use bevy::prelude::*;
use renzora::physics::{
    physics_backend_selected, AppPhysicsBackendExt, BodyVelocity, Caps, CastBatch, CastHit,
    CastShape, CollisionShapeData, Contact, JointKind, OverlapBatch, PhysicsBackend, PhysicsBodyData, PhysicsBodyType,
    PhysicsCommand, PhysicsCommands, PhysicsContacts, PhysicsDim, PhysicsJoint, PhysicsSet,
    PhysicsSettings, ResolvedCollider, ResolvedShape, RuntimePhysics, RuntimePhysics2d,
};

use crate::{has_body_ancestor, CommandForce};

/// The name this backend registers under, and the one project settings use.
pub const NAME: &str = "avian3d";

pub struct Avian3dBackend;

impl PhysicsBackend for Avian3dBackend {
    fn name(&self) -> &str {
        NAME
    }

    fn caps(&self) -> Caps {
        Caps::DIM_3D
            .union(Caps::TRIMESH)
            .union(Caps::JOINTS)
            .union(Caps::CCD)
    }

    fn cast(&self, world: &World, batch: &mut CastBatch) {
        with_queries(world, |q| cast(q, batch));
    }

    fn overlap(&self, world: &World, batch: &mut OverlapBatch) {
        with_queries(world, |q| overlap(q, batch));
    }

    fn runtime_type_prefixes(&self) -> &[&'static str] {
        &["avian3d::"]
    }

    /// Strip every 3D entity of its avian parts and stop avian's clock, so an
    /// unselected avian neither steps nor collides with the new backend's bodies.
    /// Selecting it again marks the settings changed, which unpauses it.
    fn detach(&self, world: &mut World, dim: PhysicsDim) {
        if dim != PhysicsDim::D3 {
            return;
        }
        let entities: Vec<Entity> = world
            .query_filtered::<Entity, (With<RuntimePhysics>, Without<RuntimePhysics2d>)>()
            .iter(world)
            .collect();
        for entity in entities {
            world
                .entity_mut(entity)
                .remove::<BodyParts>()
                .remove::<ColliderParts>();
        }
        let joints: Vec<Entity> = world
            .query_filtered::<Entity, With<PhysicsJoint>>()
            .iter(world)
            .collect();
        for entity in joints {
            world.entity_mut(entity).remove::<JointParts>();
        }
        if let Some(mut time) = world.get_resource_mut::<Time<Physics>>() {
            time.pause();
        }
    }
}

pub struct Avian3dPlugin;

impl Plugin for Avian3dPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] Avian3dPlugin");
        app.add_plugins(PhysicsPlugins::default());
        app.add_physics_backend(Avian3dBackend);
        // Built here, with the world in hand, so a cast later needs only `&World`.
        let state = SystemState::<QueryParams>::new(app.world_mut());
        app.insert_resource(QueryState(Mutex::new(state)));

        let active = physics_backend_selected(NAME, PhysicsDim::D3);
        // Before the fixed-step loop, so a pause written this frame stops the
        // very next step.
        app.add_systems(PreUpdate, apply_settings.run_if(active.clone()));
        app.add_systems(
            Update,
            (remove_dropped_bodies, remove_dropped_colliders, sync_bodies, sync_colliders, sync_joints)
                .chain()
                .in_set(PhysicsSet::Sync)
                .run_if(active.clone()),
        );
        app.add_systems(
            Update,
            (write_velocities, write_contacts)
                .in_set(PhysicsSet::Writeback)
                .run_if(active.clone()),
        );
        app.add_systems(
            PostUpdate,
            apply_commands.in_set(PhysicsSet::Commands).run_if(active),
        );
    }
}

fn apply_settings(
    settings: Res<PhysicsSettings>,
    mut gravity: ResMut<Gravity>,
    mut time: ResMut<Time<Physics>>,
    mut substeps: ResMut<SubstepCount>,
) {
    if !settings.is_changed() {
        return;
    }
    gravity.0 = settings.gravity;
    time.set_relative_speed(settings.time_scale);
    substeps.0 = settings.substeps;
    if settings.paused {
        time.pause();
    } else {
        time.unpause();
    }
}

/// Avian components that make up a body, removed by `Strip`. Split from
/// [`ColliderParts`] because a bundle tuple stops at fifteen.
type BodyParts = (
    RigidBody,
    Mass,
    GravityScale,
    LinearDamping,
    AngularDamping,
    LockedAxes,
    LinearVelocity,
    AngularVelocity,
    ConstantForce,
    ConstantTorque,
    CommandForce,
    SweptCcd,
);

/// Avian components that make up a collider.
type ColliderParts = (Collider, ColliderTransform, Friction, Restitution, Sensor, CollisionLayers);

/// Avian components a [`PhysicsJoint`] becomes.
type JointParts = (
    FixedJoint,
    SphericalJoint,
    RevoluteJoint,
    PrismaticJoint,
    DistanceJoint,
    JointCollisionDisabled,
);

fn body_parts(body: &PhysicsBodyData) -> impl Bundle {
    let rigid_body = match body.body_type {
        PhysicsBodyType::RigidBody => RigidBody::Dynamic,
        PhysicsBodyType::StaticBody => RigidBody::Static,
        PhysicsBodyType::KinematicBody => RigidBody::Kinematic,
    };
    let mut locked = LockedAxes::new();
    if body.lock_rotation_x {
        locked = locked.lock_rotation_x();
    }
    if body.lock_rotation_y {
        locked = locked.lock_rotation_y();
    }
    if body.lock_rotation_z {
        locked = locked.lock_rotation_z();
    }
    if body.lock_translation_x {
        locked = locked.lock_translation_x();
    }
    if body.lock_translation_y {
        locked = locked.lock_translation_y();
    }
    if body.lock_translation_z {
        locked = locked.lock_translation_z();
    }
    (
        rigid_body,
        Mass(body.mass),
        GravityScale(body.gravity_scale),
        LinearDamping(body.linear_damping),
        AngularDamping(body.angular_damping),
        locked,
    )
}

/// The avian collider for a resolved shape. `None` for a 2D shape, which never
/// reaches this world.
fn collider_for(shape: &ResolvedShape) -> Option<Collider> {
    Some(match shape {
        ResolvedShape::Cuboid { half_extents } => {
            let size = *half_extents * 2.0;
            Collider::cuboid(size.x, size.y, size.z)
        }
        ResolvedShape::Sphere { radius } => Collider::sphere(*radius),
        ResolvedShape::Capsule { radius, half_height } => Collider::capsule(*radius, half_height * 2.0),
        ResolvedShape::Cylinder { radius, half_height } => {
            Collider::cylinder(*radius, half_height * 2.0)
        }
        ResolvedShape::TriMesh(mesh) => Collider::trimesh(mesh.vertices.clone(), mesh.indices.clone()),
        ResolvedShape::Rectangle { .. } | ResolvedShape::Circle { .. } | ResolvedShape::Capsule2d { .. } => {
            return None
        }
    })
}

/// The entity's collider with its offset and turn baked in.
///
/// Baked as a one-shape compound rather than set as a `ColliderTransform`:
/// avian recomputes that component every step, to identity for a collider on
/// its body's own entity, so an offset put there is silently discarded (the 2D
/// backend found this first; see `offset_2d_collider_collides_at_offset`).
/// Baking it also keeps a `Collider` cast honest: the shape a cast sweeps is
/// the shape the body collides with, offset included.
fn placed_collider(resolved: &ResolvedCollider) -> Option<Collider> {
    let collider = collider_for(&resolved.shape)?;
    if resolved.offset == Vec3::ZERO && resolved.rotation == Quat::IDENTITY {
        return Some(collider);
    }
    Some(Collider::compound(vec![(resolved.offset, resolved.rotation, collider)]))
}

/// An entity un-wired from physics loses everything avian built for it; one
/// that only lost its `PhysicsBodyData` loses the body and keeps its collider,
/// which avian then treats as fixed scenery. See `PhysicsSet::Sync`.
fn remove_dropped_bodies(
    mut commands: Commands,
    mut removed_runtime: RemovedComponents<RuntimePhysics>,
    mut removed_bodies: RemovedComponents<PhysicsBodyData>,
    wired: Query<(), With<RuntimePhysics>>,
    has_body: Query<(), With<PhysicsBodyData>>,
    colliders: Query<(), (With<CollisionShapeData>, Without<RuntimePhysics2d>)>,
    parents: Query<&ChildOf>,
) {
    for entity in removed_runtime.read() {
        if !wired.contains(entity) {
            if let Ok(mut e) = commands.get_entity(entity) {
                e.try_remove::<BodyParts>().try_remove::<ColliderParts>();
            }
        }
    }
    for entity in removed_bodies.read() {
        if wired.contains(entity) && !has_body.contains(entity) {
            if let Ok(mut e) = commands.get_entity(entity) {
                e.try_remove::<BodyParts>();
                // A collider left behind is still solid; see `has_body_ancestor`.
                if colliders.contains(entity) && !has_body_ancestor(entity, &has_body, &parents) {
                    e.try_insert(RigidBody::Static);
                }
            }
        }
    }
}

/// A collider whose `ResolvedCollider` went away (its shape became a mesh that
/// has not loaded yet, or the engine dropped it) leaves the world too.
fn remove_dropped_colliders(
    mut commands: Commands,
    mut removed: RemovedComponents<ResolvedCollider>,
    still_has: Query<(), With<ResolvedCollider>>,
) {
    for entity in removed.read() {
        if still_has.contains(entity) {
            continue;
        }
        if let Ok(mut e) = commands.get_entity(entity) {
            e.try_remove::<ColliderParts>();
        }
    }
}

fn sync_bodies(
    mut commands: Commands,
    bodies: Query<
        (Entity, &PhysicsBodyData),
        (
            With<RuntimePhysics>,
            Without<RuntimePhysics2d>,
            Or<(Added<RuntimePhysics>, Changed<PhysicsBodyData>)>,
        ),
    >,
    new_colliders: Query<
        Entity,
        (
            Added<RuntimePhysics>,
            Without<RuntimePhysics2d>,
            With<CollisionShapeData>,
            Without<PhysicsBodyData>,
        ),
    >,
    all_bodies: Query<(), With<PhysicsBodyData>>,
    parents: Query<&ChildOf>,
) {
    for (entity, body) in &bodies {
        let mut e = commands.entity(entity);
        e.try_insert(body_parts(body));
        if body.ccd {
            e.try_insert(SweptCcd::default());
        } else {
            e.try_remove::<SweptCcd>();
        }
    }
    // The same explicit static body the 2D backend gives a body-less collider;
    // see `has_body_ancestor` for what happens without it.
    for entity in &new_colliders {
        if !has_body_ancestor(entity, &all_bodies, &parents) {
            commands.entity(entity).try_insert(RigidBody::Static);
        }
    }
}

fn sync_colliders(
    mut commands: Commands,
    colliders: Query<
        (Entity, &ResolvedCollider),
        (With<RuntimePhysics>, Without<RuntimePhysics2d>, Changed<ResolvedCollider>),
    >,
) {
    for (entity, resolved) in &colliders {
        let Some(collider) = placed_collider(resolved) else {
            continue;
        };
        let mut e = commands.entity(entity);
        e.try_insert((
            collider,
            Friction::new(resolved.friction),
            Restitution::new(resolved.restitution),
            CollisionLayers::from_bits(resolved.layers.memberships, resolved.layers.filters),
        ));
        // Left over from before offsets were baked into the collider.
        e.try_remove::<ColliderTransform>();
        if resolved.sensor {
            e.try_insert(Sensor);
        } else {
            e.try_remove::<Sensor>();
        }
    }
}

/// Each 3D [`PhysicsJoint`] becomes the matching avian joint on the same entity.
fn sync_joints(
    mut commands: Commands,
    mut removed: RemovedComponents<PhysicsJoint>,
    joints: Query<(Entity, &PhysicsJoint), Changed<PhysicsJoint>>,
    dims: Query<Has<RuntimePhysics2d>>,
) {
    for entity in removed.read() {
        if let Ok(mut e) = commands.get_entity(entity) {
            e.try_remove::<JointParts>();
        }
    }
    for (entity, joint) in &joints {
        if dims.get(joint.body1) == Ok(true) {
            continue;
        }
        let mut e = commands.entity(entity);
        e.try_remove::<JointParts>();
        let (b1, b2, c) = (joint.body1, joint.body2, joint.compliance);
        match joint.kind {
            JointKind::Fixed => {
                e.try_insert(
                    FixedJoint::new(b1, b2)
                        .with_local_anchor1(joint.anchor1)
                        .with_local_anchor2(joint.anchor2)
                        .with_angle_compliance(c),
                );
            }
            JointKind::Spherical { twist_axis, swing_limit, twist_limit } => {
                let mut j = SphericalJoint::new(b1, b2)
                    .with_local_anchor1(joint.anchor1)
                    .with_local_anchor2(joint.anchor2)
                    .with_twist_axis(twist_axis.normalize_or(Vec3::Y))
                    .with_swing_compliance(c)
                    .with_twist_compliance(c);
                if let Some(swing) = swing_limit {
                    j = j.with_swing_limits(-swing, swing);
                }
                if let Some(twist) = twist_limit {
                    j = j.with_twist_limits(-twist, twist);
                }
                e.try_insert(j);
            }
            JointKind::Revolute { axis, limits } => {
                let mut j = RevoluteJoint::new(b1, b2)
                    .with_local_anchor1(joint.anchor1)
                    .with_local_anchor2(joint.anchor2)
                    .with_hinge_axis(axis.normalize_or(Vec3::Y))
                    .with_align_compliance(c)
                    .with_limit_compliance(c);
                if let Some((min, max)) = limits {
                    j = j.with_angle_limits(min, max);
                }
                e.try_insert(j);
            }
            JointKind::Prismatic { axis, limits } => {
                let mut j = PrismaticJoint::new(b1, b2)
                    .with_local_anchor1(joint.anchor1)
                    .with_local_anchor2(joint.anchor2)
                    .with_slider_axis(axis.normalize_or(Vec3::X))
                    .with_align_compliance(c)
                    .with_angle_compliance(c)
                    .with_limit_compliance(c);
                if let Some((min, max)) = limits {
                    j = j.with_limits(min, max);
                }
                e.try_insert(j);
            }
            JointKind::Distance { min, max } => {
                e.try_insert(
                    DistanceJoint::new(b1, b2)
                        .with_local_anchor1(joint.anchor1)
                        .with_local_anchor2(joint.anchor2)
                        .with_limits(min, max)
                        .with_compliance(c),
                );
            }
        }
        if !joint.collide_connected {
            e.try_insert(JointCollisionDisabled);
        }
    }
}

fn write_velocities(
    mut q: Query<
        (Option<&LinearVelocity>, Option<&AngularVelocity>, &mut BodyVelocity),
        Without<RuntimePhysics2d>,
    >,
) {
    for (linear, angular, mut velocity) in &mut q {
        velocity.set_if_neq(BodyVelocity {
            linear: linear.map(|v| v.0).unwrap_or(Vec3::ZERO),
            angular: angular.map(|v| v.0).unwrap_or(Vec3::ZERO),
        });
    }
}

fn write_contacts(
    mut q: Query<(Entity, &mut PhysicsContacts, Option<&GlobalTransform>), Without<RuntimePhysics2d>>,
    collisions: Collisions,
) {
    for (entity, mut contacts, transform) in &mut q {
        let current: Vec<Contact> = collisions
            .collisions_with(entity)
            .map(|pair| {
                let first = pair.collider1 == entity;
                let other = if first { pair.collider2 } else { pair.collider1 };
                // Manifold normals point from collider1 to collider2; ours points
                // at this body, which is the opposite way when it is collider1.
                let deepest = pair
                    .manifolds
                    .iter()
                    .flat_map(|m| m.points.iter().map(move |p| (m.normal, p)))
                    .max_by(|a, b| a.1.penetration.total_cmp(&b.1.penetration));
                let (point, normal) = match deepest {
                    Some((normal, p)) => (p.point, if first { -normal } else { normal }),
                    // A sensor overlap has no manifold.
                    None => (transform.map(|t| t.translation()).unwrap_or_default(), Vec3::ZERO),
                };
                Contact { entity: other, point, normal, impulse: pair.total_normal_impulse_magnitude() }
            })
            .collect();
        if contacts.0 != current {
            contacts.0 = current;
        }
    }
}

/// Apply this frame's commands to the 3D bodies among their targets.
///
/// Last frame's command forces are dropped first: a force acts through every
/// step until the next frame's commands land, so one pushed every frame is a
/// steady force at any frame rate, and one pushed once acts for a frame.
fn apply_commands(
    queue: Res<PhysicsCommands>,
    dims: Query<Has<RuntimePhysics2d>, With<RuntimePhysics>>,
    previous_forces: Query<Entity, With<CommandForce>>,
    mut forces_q: Query<Forces>,
    mut commands: Commands,
) {
    for entity in &previous_forces {
        commands
            .entity(entity)
            .try_remove::<(ConstantForce, ConstantTorque, CommandForce)>();
    }
    let mut forces: HashMap<Entity, (Vec3, Vec3)> = HashMap::new();
    for command in &queue.0 {
        let entity = command.entity();
        if let PhysicsCommand::Strip { entity } = *command {
            // Whatever world the entity was in: removing absent parts is a no-op.
            if let Ok(mut e) = commands.get_entity(entity) {
                e.try_remove::<BodyParts>().try_remove::<ColliderParts>();
            }
            continue;
        }
        if dims.get(entity) != Ok(false) {
            continue;
        }
        match *command {
            PhysicsCommand::AddForce { force, .. } => {
                forces.entry(entity).or_default().0 += force;
            }
            PhysicsCommand::AddTorque { torque, .. } => {
                forces.entry(entity).or_default().1 += torque;
            }
            PhysicsCommand::AddImpulse { impulse, .. } => {
                if let Ok(mut f) = forces_q.get_mut(entity) {
                    f.apply_linear_impulse(impulse);
                }
            }
            PhysicsCommand::AddAngularImpulse { impulse, .. } => {
                if let Ok(mut f) = forces_q.get_mut(entity) {
                    f.apply_angular_impulse(impulse);
                }
            }
            PhysicsCommand::SetLinearVelocity { velocity, .. } => {
                commands.entity(entity).try_insert(LinearVelocity(velocity));
            }
            PhysicsCommand::SetAngularVelocity { velocity, .. } => {
                commands.entity(entity).try_insert(AngularVelocity(velocity));
            }
            PhysicsCommand::Strip { .. } => {}
        }
    }
    for (entity, (force, torque)) in forces {
        commands
            .entity(entity)
            .try_insert((ConstantForce(force), ConstantTorque(torque), CommandForce));
    }
}

// ── Queries ──────────────────────────────────────────────────────────────

type QueryParams = (
    SpatialQuery<'static, 'static>,
    Query<'static, 'static, (), With<Sensor>>,
    Query<'static, 'static, &'static Collider>,
);

/// The spatial-query parameters, built once so a cast needs only `&World`.
/// A mutex because fetching from it takes `&mut`, and casts arrive through
/// `&self`.
#[derive(Resource)]
struct QueryState(Mutex<SystemState<QueryParams>>);

struct Queries<'a, 'w, 's> {
    spatial: &'a SpatialQuery<'w, 's>,
    sensors: &'a Query<'w, 's, (), With<Sensor>>,
    colliders: &'a Query<'w, 's, &'static Collider>,
}

fn with_queries(world: &World, run: impl FnOnce(Queries)) {
    // Missing only when avian's own plugins were not added; every query is
    // then a miss, which is the same answer as an empty scene.
    let Some(state) = world.get_resource::<QueryState>() else { return };
    let Ok(mut state) = state.0.lock() else { return };
    let Ok((spatial, sensors, colliders)) = state.get(world) else { return };
    run(Queries { spatial: &spatial, sensors: &sensors, colliders: &colliders });
}

fn cast(q: Queries, batch: &mut CastBatch) {
    // Shapes are shared between consecutive casts of one shape, which a burst
    // of one emitter's particles nearly always is.
    let mut built: Option<(ResolvedShape, Collider)> = None;
    let CastBatch { casts, hits } = batch;
    for (cast, hit) in casts.iter().zip(hits.iter_mut()) {
        if cast.dim != PhysicsDim::D3 {
            continue;
        }
        let Ok(dir) = Dir3::new(cast.direction) else { continue };
        let filter = SpatialQueryFilter::from_mask(cast.collides_with)
            .with_excluded_entities(cast.exclude.iter().copied());
        let solid = |e: Entity| cast.include_sensors || !q.sensors.contains(e);
        let shape = match &cast.shape {
            CastShape::Ray => {
                if let Some(h) = q.spatial.cast_ray_predicate(
                    cast.origin,
                    dir,
                    cast.max_distance,
                    !cast.ignore_origin_penetration,
                    &filter,
                    &solid,
                ) {
                    *hit = Some(CastHit {
                        distance: h.distance,
                        point: cast.origin + *dir * h.distance,
                        normal: h.normal,
                        entity: h.entity,
                    });
                }
                continue;
            }
            CastShape::Shape(shape) => {
                if built.as_ref().is_none_or(|(s, _)| s != shape) {
                    built = collider_for(shape).map(|c| (shape.clone(), c));
                }
                match &built {
                    Some((_, c)) => c.clone(),
                    None => continue,
                }
            }
            // The entity's own collider, offset and turn included (see
            // `placed_collider`), so the cast is placed by the entity's pose.
            CastShape::Collider(entity) => match q.colliders.get(*entity) {
                Ok(c) => c.clone(),
                Err(_) => continue,
            },
        };
        let config = ShapeCastConfig {
            max_distance: cast.max_distance,
            ignore_origin_penetration: cast.ignore_origin_penetration,
            ..Default::default()
        };
        if let Some(h) =
            q.spatial.cast_shape_predicate(&shape, cast.origin, cast.rotation, dir, &config, &filter, &solid)
        {
            *hit = Some(CastHit {
                distance: h.distance,
                point: h.point1,
                normal: h.normal1,
                entity: h.entity,
            });
        }
    }
}

fn overlap(q: Queries, batch: &mut OverlapBatch) {
    let OverlapBatch { queries, hits } = batch;
    for (query, found) in queries.iter().zip(hits.iter_mut()) {
        if query.dim != PhysicsDim::D3 {
            continue;
        }
        let Some(shape) = collider_for(&query.shape) else { continue };
        let filter = SpatialQueryFilter::from_mask(query.collides_with)
            .with_excluded_entities(query.exclude.iter().copied());
        q.spatial.shape_intersections_callback(&shape, query.position, query.rotation, &filter, |e| {
            if query.include_sensors || !q.sensors.contains(e) {
                found.push(e);
            }
            true
        });
    }
}
