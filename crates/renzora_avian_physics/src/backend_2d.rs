//! The 2D world on avian2d, the sprite-world sibling of [`crate::backend_3d`].
//!
//! The engine resolves every 2D collider into a 2D shape before it gets here
//! (a box becomes a rectangle from `half_extents.xy`, a sphere or cylinder a
//! circle), so this file only maps those onto avian2d.

use std::collections::HashMap;
use std::sync::Mutex;

use avian2d::prelude::*;
use bevy::ecs::system::SystemState;
use bevy::prelude::*;
use renzora::physics::{
    physics_backend_selected, AppPhysicsBackendExt, BodyVelocity, Caps, CastBatch, CastHit,
    CastShape, CollisionShapeData, Contact, JointKind, OverlapBatch, PhysicsBackend,
    PhysicsBodyData, PhysicsBodyType, PhysicsCommand, PhysicsCommands, PhysicsContacts,
    PhysicsDim, PhysicsJoint, PhysicsSet, PhysicsSettings, ResolvedCollider, ResolvedShape,
    RuntimePhysics, RuntimePhysics2d,
};

use crate::{has_body_ancestor, CommandForce};

/// The name this backend registers under, and the one project settings use.
pub const NAME: &str = "avian2d";

pub struct Avian2dBackend;

impl PhysicsBackend for Avian2dBackend {
    fn name(&self) -> &str {
        NAME
    }

    fn caps(&self) -> Caps {
        Caps::DIM_2D.union(Caps::JOINTS).union(Caps::CCD)
    }

    fn cast(&self, world: &World, batch: &mut CastBatch) {
        with_queries(world, |q| cast(q, batch));
    }

    fn overlap(&self, world: &World, batch: &mut OverlapBatch) {
        with_queries(world, |q| overlap(q, batch));
    }

    fn runtime_type_prefixes(&self) -> &[&'static str] {
        &["avian2d::"]
    }

    /// The 2D twin of `backend_3d`'s detach.
    fn detach(&self, world: &mut World, dim: PhysicsDim) {
        if dim != PhysicsDim::D2 {
            return;
        }
        let entities: Vec<Entity> = world
            .query_filtered::<Entity, (With<RuntimePhysics>, With<RuntimePhysics2d>)>()
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

pub struct Avian2dPlugin;

impl Plugin for Avian2dPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] Avian2dPlugin");
        // Interpolation disabled here, and ONLY here.
        //
        // avian2d and avian3d are separate crates with distinct types
        // throughout, so they coexist happily, except that both depend on the
        // one `bevy_transform_interpolation`, and each one's
        // `PhysicsInterpolationPlugin` adds `TransformInterpolationPlugin` from
        // it. That plugin is the *same type* from both backends, so the second
        // `add_plugins` panics with "plugin was already added". The vendored
        // fork carried a patch to skip the re-add; taking avian from crates.io
        // means solving it on this side instead.
        //
        // Dropping the 2D copy costs nothing measurable: every auto-registration
        // flag on `PhysicsInterpolationPlugin` defaults to false, so it opts no
        // entity into anything, and the engine never adds a
        // `TranslationInterpolation` / `Extrapolation` / `HermiteEasing`
        // component itself. `TransformInterpolationPlugin` still arrives via the
        // 3D backend and its easing is component-driven rather than
        // backend-specific, so it serves 2D entities too. What genuinely goes is
        // 2D extrapolation and Hermite easing, whose plugins are generic over
        // avian2d-private velocity sources and so cannot be re-added from out
        // here. If a 2D game ever needs them, that is the point to upstream a
        // guard rather than work around it.
        //
        // This relies on `Avian3dPlugin` being added first, which `lib.rs` does.
        app.add_plugins(
            PhysicsPlugins::default()
                .build()
                .disable::<PhysicsInterpolationPlugin>(),
        );

        app.add_physics_backend(Avian2dBackend);
        let state = SystemState::<QueryParams>::new(app.world_mut());
        app.insert_resource(QueryState(Mutex::new(state)));

        let active = physics_backend_selected(NAME, PhysicsDim::D2);
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
            apply_commands.in_set(PhysicsSet::Commands).run_if(active.clone()),
        );
        app.add_systems(
            FixedUpdate,
            apply_fixed_commands.in_set(PhysicsSet::Commands).run_if(active),
        );
    }
}

/// One set of settings drives both worlds; the plane takes gravity's XY.
fn apply_settings(
    settings: Res<PhysicsSettings>,
    mut gravity: ResMut<Gravity>,
    mut time: ResMut<Time<Physics>>,
    mut substeps: ResMut<SubstepCount>,
) {
    if !settings.is_changed() {
        return;
    }
    gravity.0 = settings.gravity.truncate();
    time.set_relative_speed(settings.time_scale);
    substeps.0 = settings.substeps;
    if settings.paused {
        time.pause();
    } else {
        time.unpause();
    }
}

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

type ColliderParts = (Collider, ColliderTransform, Friction, Restitution, Sensor, CollisionLayers);

/// Avian components a 2D [`PhysicsJoint`] becomes. No spherical joint: a ball
/// and socket has nothing to turn about in a plane but Z, which is a hinge.
type JointParts = (FixedJoint, RevoluteJoint, PrismaticJoint, DistanceJoint, JointCollisionDisabled);

fn body_parts(body: &PhysicsBodyData) -> impl Bundle {
    let rigid_body = match body.body_type {
        PhysicsBodyType::RigidBody => RigidBody::Dynamic,
        PhysicsBodyType::StaticBody => RigidBody::Static,
        PhysicsBodyType::KinematicBody => RigidBody::Kinematic,
    };
    // 2D has a single rotation axis; the shared data's Z-rotation lock is the
    // one that maps onto it (X/Y rotation locks don't exist in the plane).
    let mut locked = LockedAxes::new();
    if body.lock_rotation_z {
        locked = locked.lock_rotation();
    }
    if body.lock_translation_x {
        locked = locked.lock_translation_x();
    }
    if body.lock_translation_y {
        locked = locked.lock_translation_y();
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

/// The avian2d collider for a resolved shape. `None` for a 3D shape, which
/// never reaches this world.
fn collider_for(shape: &ResolvedShape) -> Option<Collider> {
    Some(match shape {
        ResolvedShape::Rectangle { half_extents } => {
            Collider::rectangle(half_extents.x * 2.0, half_extents.y * 2.0)
        }
        ResolvedShape::Circle { radius } => Collider::circle(*radius),
        ResolvedShape::Capsule2d { radius, half_height } => Collider::capsule(*radius, half_height * 2.0),
        ResolvedShape::Cuboid { .. }
        | ResolvedShape::Sphere { .. }
        | ResolvedShape::Capsule { .. }
        | ResolvedShape::Cylinder { .. }
        | ResolvedShape::TriMesh(_)
        | ResolvedShape::Heightfield(_) => return None,
    })
}

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

/// The 2D twin of `backend_3d::remove_dropped_bodies`. A collider left without
/// its body gets the explicit static body every bodiless 2D collider has (see
/// `sync_bodies`), or avian2d would stop reporting its contacts.
fn remove_dropped_bodies(
    mut commands: Commands,
    mut removed_runtime: RemovedComponents<RuntimePhysics>,
    mut removed_bodies: RemovedComponents<PhysicsBodyData>,
    wired: Query<(), With<RuntimePhysics>>,
    has_body: Query<(), With<PhysicsBodyData>>,
    colliders: Query<(), (With<CollisionShapeData>, With<RuntimePhysics2d>)>,
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
                if colliders.contains(entity) && !has_body_ancestor(entity, &has_body, &parents) {
                    e.try_insert(RigidBody::Static);
                }
            }
        }
    }
}

fn sync_bodies(
    mut commands: Commands,
    bodies: Query<
        (Entity, &PhysicsBodyData),
        (
            With<RuntimePhysics>,
            With<RuntimePhysics2d>,
            Or<(Added<RuntimePhysics>, Changed<PhysicsBodyData>)>,
        ),
    >,
    new_colliders: Query<
        Entity,
        (
            Added<RuntimePhysics>,
            With<RuntimePhysics2d>,
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
    // A collider with no body would land in avian2d's "standalone" collider
    // tree, which produces NO contacts (see tests/avian2d_collision.rs:
    // raw_avian2d_standalone_wall_blocks). An explicit static body is
    // semantically identical for world geometry (tile colliders, prop trunks)
    // and uses the static tree, which works. Skipped when a body sits on an
    // ancestor so a child collider still attaches to that body instead of
    // becoming its own.
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
        (With<RuntimePhysics>, With<RuntimePhysics2d>, Changed<ResolvedCollider>),
    >,
) {
    for (entity, resolved) in &colliders {
        let Some(collider) = collider_for(&resolved.shape) else {
            continue;
        };
        // Bake a non-zero offset (and turn) into the collider itself as a
        // one-shape compound. Inserting `ColliderTransform` manually LOOKS like
        // the way to offset a shape, but avian recomputes that component every
        // step (identity for a collider on the body's own entity), so the offset
        // was silently discarded, as `offset_2d_collider_collides_at_offset`
        // proves.
        let offset = resolved.offset.truncate();
        let angle = z_angle(resolved.rotation);
        let collider = if offset != Vec2::ZERO || angle != 0.0 {
            Collider::compound(vec![(offset, Rotation::radians(angle), collider)])
        } else {
            collider
        };
        let mut e = commands.entity(entity);
        e.try_insert((
            collider,
            Friction::new(resolved.friction),
            Restitution::new(resolved.restitution),
            CollisionLayers::from_bits(resolved.layers.memberships, resolved.layers.filters),
        ));
        if resolved.sensor {
            e.try_insert(Sensor);
        } else {
            e.try_remove::<Sensor>();
        }
    }
}

/// The plane's one rotation: the turn about Z.
fn z_angle(rotation: Quat) -> f32 {
    rotation.to_euler(EulerRot::ZYX).0
}

/// Each 2D [`PhysicsJoint`] becomes the matching avian2d joint.
fn sync_joints(
    mut commands: Commands,
    mut removed: RemovedComponents<PhysicsJoint>,
    joints: Query<(Entity, &PhysicsJoint), Changed<PhysicsJoint>>,
    dims: Query<Has<RuntimePhysics2d>>,
    mut warned: Local<bool>,
) {
    for entity in removed.read() {
        if let Ok(mut e) = commands.get_entity(entity) {
            e.try_remove::<JointParts>();
        }
    }
    for (entity, joint) in &joints {
        if dims.get(joint.body1) != Ok(true) {
            continue;
        }
        let mut e = commands.entity(entity);
        e.try_remove::<JointParts>();
        let (b1, b2, c) = (joint.body1, joint.body2, joint.compliance);
        let (a1, a2) = (joint.anchor1.truncate(), joint.anchor2.truncate());
        match joint.kind {
            JointKind::Fixed => {
                e.try_insert(
                    FixedJoint::new(b1, b2)
                        .with_local_anchor1(a1)
                        .with_local_anchor2(a2)
                        .with_angle_compliance(c),
                );
            }
            JointKind::Revolute { limits, .. } => {
                let mut j = RevoluteJoint::new(b1, b2)
                    .with_local_anchor1(a1)
                    .with_local_anchor2(a2)
                    .with_limit_compliance(c);
                if let Some((min, max)) = limits {
                    j = j.with_angle_limits(min, max);
                }
                e.try_insert(j);
            }
            JointKind::Prismatic { axis, limits } => {
                let mut j = PrismaticJoint::new(b1, b2)
                    .with_local_anchor1(a1)
                    .with_local_anchor2(a2)
                    .with_slider_axis(axis.truncate().normalize_or(Vec2::X))
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
                        .with_local_anchor1(a1)
                        .with_local_anchor2(a2)
                        .with_limits(min, max)
                        .with_compliance(c),
                );
            }
            JointKind::Spherical { .. } => {
                if !*warned {
                    *warned = true;
                    warn!("[physics] a spherical joint has no meaning in 2D; use a revolute joint there");
                }
                continue;
            }
        }
        if !joint.collide_connected {
            e.try_insert(JointCollisionDisabled);
        }
    }
}

/// Mirrors 2D velocity into the contract's Vec3 fields (`z = 0`, spin about Z).
fn write_velocities(
    mut q: Query<
        (Option<&LinearVelocity>, Option<&AngularVelocity>, &mut BodyVelocity),
        With<RuntimePhysics2d>,
    >,
) {
    for (linear, angular, mut velocity) in &mut q {
        velocity.set_if_neq(BodyVelocity {
            linear: linear.map(|v| v.0.extend(0.0)).unwrap_or(Vec3::ZERO),
            angular: angular.map(|v| Vec3::Z * v.0).unwrap_or(Vec3::ZERO),
        });
    }
}

fn write_contacts(
    mut q: Query<(Entity, &mut PhysicsContacts, Option<&GlobalTransform>), With<RuntimePhysics2d>>,
    collisions: Collisions,
) {
    for (entity, mut contacts, transform) in &mut q {
        let current: Vec<Contact> = collisions
            .collisions_with(entity)
            .map(|pair| {
                let first = pair.collider1 == entity;
                let other = if first { pair.collider2 } else { pair.collider1 };
                // See `backend_3d::write_contacts` for the normal's direction.
                let deepest = pair
                    .manifolds
                    .iter()
                    .flat_map(|m| m.points.iter().map(move |p| (m.normal, p)))
                    .max_by(|a, b| a.1.penetration.total_cmp(&b.1.penetration));
                let here = transform.map(|t| t.translation()).unwrap_or_default();
                let (point, normal) = match deepest {
                    Some((normal, p)) => {
                        let n = if first { -normal } else { normal };
                        (p.point.extend(here.z), n.extend(0.0))
                    }
                    None => (here, Vec3::ZERO),
                };
                Contact { entity: other, point, normal, impulse: pair.total_normal_impulse_magnitude() }
            })
            .collect();
        if contacts.0 != current {
            contacts.0 = current;
        }
    }
}

/// The 2D twin of `backend_3d::apply_commands`; vectors lose their `z`, and
/// spins keep only theirs.
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
    apply_queue(&queue, &dims, &mut forces_q, &mut commands);
}

/// The 2D twin of `backend_3d::apply_fixed_commands`.
fn apply_fixed_commands(
    queue: Res<PhysicsCommands>,
    dims: Query<Has<RuntimePhysics2d>, With<RuntimePhysics>>,
    mut forces_q: Query<Forces>,
    mut commands: Commands,
) {
    apply_queue(&queue, &dims, &mut forces_q, &mut commands);
}

fn apply_queue(
    queue: &PhysicsCommands,
    dims: &Query<Has<RuntimePhysics2d>, With<RuntimePhysics>>,
    forces_q: &mut Query<Forces>,
    commands: &mut Commands,
) {
    let mut forces: HashMap<Entity, (Vec2, f32)> = HashMap::new();
    for command in &queue.0 {
        let entity = command.entity();
        if let PhysicsCommand::Strip { entity } = *command {
            if let Ok(mut e) = commands.get_entity(entity) {
                e.try_remove::<BodyParts>().try_remove::<ColliderParts>();
            }
            continue;
        }
        if dims.get(entity) != Ok(true) {
            continue;
        }
        match *command {
            PhysicsCommand::AddForce { force, .. } => {
                forces.entry(entity).or_default().0 += force.truncate();
            }
            PhysicsCommand::AddTorque { torque, .. } => {
                forces.entry(entity).or_default().1 += torque.z;
            }
            PhysicsCommand::AddImpulse { impulse, .. } => {
                if let Ok(mut f) = forces_q.get_mut(entity) {
                    f.apply_linear_impulse(impulse.truncate());
                }
            }
            PhysicsCommand::AddAngularImpulse { impulse, .. } => {
                if let Ok(mut f) = forces_q.get_mut(entity) {
                    f.apply_angular_impulse(impulse.z);
                }
            }
            PhysicsCommand::SetLinearVelocity { velocity, .. } => {
                commands.entity(entity).try_insert(LinearVelocity(velocity.truncate()));
            }
            PhysicsCommand::SetAngularVelocity { velocity, .. } => {
                commands.entity(entity).try_insert(AngularVelocity(velocity.z));
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

/// See `backend_3d::QueryState`.
#[derive(Resource)]
struct QueryState(Mutex<SystemState<QueryParams>>);

struct Queries<'a, 'w, 's> {
    spatial: &'a SpatialQuery<'w, 's>,
    sensors: &'a Query<'w, 's, (), With<Sensor>>,
    colliders: &'a Query<'w, 's, &'static Collider>,
}

fn with_queries(world: &World, run: impl FnOnce(Queries)) {
    let Some(state) = world.get_resource::<QueryState>() else { return };
    let Ok(mut state) = state.0.lock() else { return };
    let Ok((spatial, sensors, colliders)) = state.get(world) else { return };
    run(Queries { spatial: &spatial, sensors: &sensors, colliders: &colliders });
}

fn overlap(q: Queries, batch: &mut OverlapBatch) {
    let OverlapBatch { queries, hits } = batch;
    for (query, found) in queries.iter().zip(hits.iter_mut()) {
        if query.dim != PhysicsDim::D2 {
            continue;
        }
        let Some(shape) = collider_for(&query.shape) else { continue };
        let filter = SpatialQueryFilter::from_mask(query.collides_with)
            .with_excluded_entities(query.exclude.iter().copied());
        q.spatial.shape_intersections_callback(
            &shape,
            query.position.truncate(),
            Rotation::radians(z_angle(query.rotation)),
            &filter,
            |e| {
                if query.include_sensors || !q.sensors.contains(e) {
                    found.push(e);
                }
                true
            },
        );
    }
}

fn cast(q: Queries, batch: &mut CastBatch) {
    let (spatial, sensors, colliders) = (q.spatial, q.sensors, q.colliders);
    let mut built: Option<(ResolvedShape, Collider)> = None;
    let CastBatch { casts, hits } = batch;
    for (cast, hit) in casts.iter().zip(hits.iter_mut()) {
        if cast.dim != PhysicsDim::D2 {
            continue;
        }
        let Ok(dir) = Dir2::new(cast.direction.truncate()) else { continue };
        let origin = cast.origin.truncate();
        let filter = SpatialQueryFilter::from_mask(cast.collides_with)
            .with_excluded_entities(cast.exclude.iter().copied());
        let solid = |e: Entity| cast.include_sensors || !sensors.contains(e);
        let shape = match &cast.shape {
            CastShape::Ray => {
                if let Some(h) = spatial.cast_ray_predicate(
                    origin,
                    dir,
                    cast.max_distance,
                    !cast.ignore_origin_penetration,
                    &filter,
                    &solid,
                ) {
                    *hit = Some(CastHit {
                        distance: h.distance,
                        point: (origin + *dir * h.distance).extend(cast.origin.z),
                        normal: h.normal.extend(0.0),
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
            CastShape::Collider(entity) => match colliders.get(*entity) {
                Ok(c) => c.clone(),
                Err(_) => continue,
            },
        };
        let angle = z_angle(cast.rotation);
        let config = ShapeCastConfig {
            max_distance: cast.max_distance,
            ignore_origin_penetration: cast.ignore_origin_penetration,
            ..Default::default()
        };
        if let Some(h) = spatial.cast_shape_predicate(
            &shape,
            origin,
            Rot2::radians(angle),
            dir,
            &config,
            &filter,
            &solid,
        ) {
            *hit = Some(CastHit {
                distance: h.distance,
                point: h.point1.extend(cast.origin.z),
                normal: h.normal1.extend(0.0),
                entity: h.entity,
            });
        }
    }
}
