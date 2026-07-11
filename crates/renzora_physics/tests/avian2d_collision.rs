//! End-to-end check of the avian2d backend path: `auto_init_physics` routes a
//! `Physics2d`-marked body + collider into the 2D simulation, and a dynamic
//! body driven by `LinearVelocity` is BLOCKED by a static collider-only entity
//! (the shape a tilemap's merged tile colliders / stamped object colliders
//! take). Guards the exact flow the editor's play mode exercises, minus the
//! renderer — a regression here is "the player walks through walls".

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use renzora_physics::{
    CollisionShapeData, CollisionShapeType, Physics2d, PhysicsBodyData, PhysicsBodyType,
    PhysicsPlugin,
};

/// Build the same app shape the runtime has (minus render): fixed manual time
/// so the fixed-timestep physics schedule actually runs during `app.update()`.
/// `Assets<Mesh>` exists only for the 3D backend's mesh-collider system —
/// MinimalPlugins carries no assets and the system's `Res` would fail
/// validation otherwise.
fn physics_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        // Avian's tree/spatial-query systems take its diagnostics resources,
        // which it only inserts when bevy's diagnostics are present (they
        // always are in the real app via DefaultPlugins).
        bevy::diagnostic::DiagnosticsPlugin,
        bevy::asset::AssetPlugin::default(),
        TransformPlugin,
    ));
    app.init_asset::<Mesh>();
    app.add_plugins(PhysicsPlugin);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / 60.0,
    )));
    // Avian registers its diagnostics resources in `Plugin::finish`, which a
    // real app's runner calls but a bare `app.update()` loop never does.
    app.finish();
    app.cleanup();
    app
}

/// A dynamic 2D body sliding +X into a static wall must stop at the wall, not
/// pass through it.
#[test]
fn dynamic_2d_body_blocked_by_static_collider() {
    let mut app = physics_app();

    // Wall: collider-only (static), like a merged tile collider — a 10×100
    // box centred at x = 30.
    let wall = app
        .world_mut()
        .spawn((
            Physics2d,
            renzora_physics::auto_fit::SkipAutoFit,
            CollisionShapeData {
                shape_type: CollisionShapeType::Box,
                half_extents: Vec3::new(5.0, 50.0, 5.0),
                ..Default::default()
            },
            Transform::from_xyz(30.0, 0.0, 0.0),
        ))
        .id();

    // Player: dynamic circle, no gravity, locked rotation — the documented
    // top-down character setup.
    let player = app
        .world_mut()
        .spawn((
            Physics2d,
            renzora_physics::auto_fit::SkipAutoFit,
            PhysicsBodyData {
                body_type: PhysicsBodyType::RigidBody,
                gravity_scale: 0.0,
                lock_rotation_z: true,
                ..Default::default()
            },
            CollisionShapeData {
                shape_type: CollisionShapeType::Sphere,
                radius: 5.0,
                ..Default::default()
            },
            Transform::from_xyz(0.0, 0.0, 0.0),
        ))
        .id();

    // Let auto_init spawn the avian components before driving the body.
    app.update();
    app.update();
    assert!(
        app.world().get::<avian2d::prelude::RigidBody>(player).is_some(),
        "auto_init_physics never routed the Physics2d body to the avian2d backend"
    );
    assert!(
        app.world().get::<avian2d::prelude::Collider>(wall).is_some(),
        "the collider-only wall never received an avian2d Collider"
    );
    // The sim must see the wall at its authored place — a Position stuck at
    // the origin means the Transform→Position sync skipped body-less colliders.
    let wall_pos = app
        .world()
        .get::<avian2d::prelude::Position>(wall)
        .map(|p| p.0);
    assert_eq!(
        wall_pos,
        Some(Vec2::new(30.0, 0.0)),
        "wall Position not synced from its Transform"
    );

    // Walk +X at 100 u/s for 2 simulated seconds — the way the character
    // script drives movement (velocity re-asserted every frame).
    for _ in 0..120 {
        app.world_mut()
            .entity_mut(player)
            .insert(avian2d::prelude::LinearVelocity(Vec2::new(100.0, 0.0)));
        app.update();
    }

    let x = app.world().get::<Transform>(player).unwrap().translation.x;
    // Free travel would be 200; contact is at 30 - 5 (wall half) - 5 (radius) = 20.
    assert!(
        x < 26.0,
        "body passed through the static collider (x = {x}, contact expected near 20)"
    );
    assert!(
        x > 5.0,
        "body never moved (x = {x}) — velocity isn't driving the 2D simulation"
    );
}

/// Raw avian2d, no renzora translation layer: dynamic body vs a collider-ONLY
/// wall (standalone tree). Bisects whether a pass-through is our backend's
/// fault or vendored avian2d's.
///
/// IGNORED because it FAILS: the vendored avian2d 0.7-dev's standalone
/// collider tree produces no contacts. That's exactly why the backend gives
/// body-less 2D colliders an explicit `RigidBody::Static` (see
/// `auto_init_physics`). If a future avian update makes this pass, that
/// workaround can go.
#[test]
#[ignore = "documents the vendored-avian2d standalone-collider bug the backend works around"]
fn raw_avian2d_standalone_wall_blocks() {
    let mut app = physics_app();
    app.world_mut().spawn((
        avian2d::prelude::Collider::rectangle(10.0, 100.0),
        Transform::from_xyz(30.0, 0.0, 0.0),
    ));
    let player = app
        .world_mut()
        .spawn((
            avian2d::prelude::RigidBody::Dynamic,
            avian2d::prelude::Collider::circle(5.0),
            avian2d::prelude::GravityScale(0.0),
            avian2d::prelude::LinearVelocity(Vec2::new(100.0, 0.0)),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ))
        .id();
    for _ in 0..120 {
        app.update();
    }
    let x = app.world().get::<Transform>(player).unwrap().translation.x;
    assert!(
        x < 26.0,
        "raw avian2d: body passed through a standalone collider (x = {x})"
    );
}

/// Raw avian2d with an explicit `RigidBody::Static` wall (static tree instead
/// of standalone). If this passes while the standalone variant fails, the
/// standalone-collider path is what's broken.
#[test]
fn raw_avian2d_static_body_wall_blocks() {
    let mut app = physics_app();
    app.world_mut().spawn((
        avian2d::prelude::RigidBody::Static,
        avian2d::prelude::Collider::rectangle(10.0, 100.0),
        Transform::from_xyz(30.0, 0.0, 0.0),
    ));
    let player = app
        .world_mut()
        .spawn((
            avian2d::prelude::RigidBody::Dynamic,
            avian2d::prelude::Collider::circle(5.0),
            avian2d::prelude::GravityScale(0.0),
            avian2d::prelude::LinearVelocity(Vec2::new(100.0, 0.0)),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ))
        .id();
    for _ in 0..120 {
        app.update();
    }
    let x = app.world().get::<Transform>(player).unwrap().translation.x;
    assert!(
        x < 26.0,
        "raw avian2d: body passed through a static-body collider (x = {x})"
    );
}

/// A collider whose `CollisionShapeData.offset` is non-zero must collide at
/// the OFFSET position, not the entity origin — the trunk-collider case (the
/// palette collision box and the viewport collider editor both author this).
#[test]
fn offset_2d_collider_collides_at_offset() {
    let mut app = physics_app();

    // "Tree": entity at x = 60, collider offset -30 → collider centred at 30.
    app.world_mut().spawn((
        Physics2d,
        renzora_physics::auto_fit::SkipAutoFit,
        CollisionShapeData {
            shape_type: CollisionShapeType::Box,
            offset: Vec3::new(-30.0, 0.0, 0.0),
            half_extents: Vec3::new(5.0, 50.0, 5.0),
            ..Default::default()
        },
        Transform::from_xyz(60.0, 0.0, 0.0),
    ));

    let player = app
        .world_mut()
        .spawn((
            Physics2d,
            renzora_physics::auto_fit::SkipAutoFit,
            PhysicsBodyData {
                body_type: PhysicsBodyType::RigidBody,
                gravity_scale: 0.0,
                lock_rotation_z: true,
                ..Default::default()
            },
            CollisionShapeData {
                shape_type: CollisionShapeType::Sphere,
                radius: 5.0,
                ..Default::default()
            },
            Transform::from_xyz(0.0, 0.0, 0.0),
        ))
        .id();

    app.update();
    for _ in 0..120 {
        app.world_mut()
            .entity_mut(player)
            .insert(avian2d::prelude::LinearVelocity(Vec2::new(100.0, 0.0)));
        app.update();
    }

    let x = app.world().get::<Transform>(player).unwrap().translation.x;
    assert!(
        x < 26.0,
        "offset collider ignored — body reached x = {x}, expected to stop near 20 \
         (offset collider centre 30 minus halves)"
    );
}
