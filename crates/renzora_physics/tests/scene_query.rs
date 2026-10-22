//! The scene query service (`renzora::scene_query`) against real Avian
//! colliders: a ray and a sphere sweep stop on a static box with the face's
//! normal, a sensor is passed straight through, and a 2D ray asks the 2D world.
//! This is what the particle system's Collide Scene node stands on.
#![cfg(any(feature = "avian3d", feature = "avian2d"))]

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use renzora::scene_query::{SceneQueries, SceneQueryBatch, SceneRay};
use renzora_physics::{CollisionShapeData, CollisionShapeType, PhysicsPlugin};

/// The app shape `avian2d_collision.rs` uses: manual fixed time so the physics
/// schedule (and with it the collider trees) runs on `app.update()`.
fn physics_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::diagnostic::DiagnosticsPlugin,
        bevy::asset::AssetPlugin::default(),
        TransformPlugin,
    ));
    app.init_asset::<Mesh>();
    app.add_plugins(PhysicsPlugin);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 60.0)));
    app.finish();
    app.cleanup();
    app
}

fn floor(sensor: bool, at: Vec3) -> (renzora_physics::auto_fit::SkipAutoFit, CollisionShapeData, Transform) {
    (
        renzora_physics::auto_fit::SkipAutoFit,
        CollisionShapeData {
            shape_type: CollisionShapeType::Box,
            half_extents: Vec3::new(5.0, 0.5, 5.0),
            is_sensor: sensor,
            ..Default::default()
        },
        Transform::from_translation(at),
    )
}

fn cast(app: &mut App, rays: Vec<SceneRay>) -> SceneQueryBatch {
    let service = app.world().get_resource::<SceneQueries>().expect("PhysicsPlugin installs the service").clone();
    let mut batch = SceneQueryBatch { rays, hits: Vec::new() };
    service.cast(app.world_mut(), &mut batch);
    batch
}

fn down(from: Vec3, radius: f32, planar: bool) -> SceneRay {
    SceneRay { origin: from, direction: Vec3::NEG_Y, max_distance: 10.0, radius, planar }
}

#[cfg(feature = "avian3d")]
#[test]
fn a_ray_and_a_sphere_stop_on_a_box_and_a_sensor_is_not_solid() {
    let mut app = physics_app();
    // A box whose top face is at y = 0, and a sensor slab above it.
    let solid = app.world_mut().spawn(floor(false, Vec3::new(0.0, -0.5, 0.0))).id();
    app.world_mut().spawn(floor(true, Vec3::new(0.0, 2.0, 0.0)));
    for _ in 0..5 {
        app.update();
    }
    let batch = cast(
        &mut app,
        vec![down(Vec3::new(0.0, 4.0, 0.0), 0.0, false), down(Vec3::new(1.0, 4.0, 0.0), 0.25, false), down(Vec3::new(50.0, 4.0, 0.0), 0.0, false)],
    );
    assert_eq!(batch.hits.len(), 3);
    let ray = batch.hits[0].expect("the ray hits the box, through the sensor");
    assert!((ray.distance - 4.0).abs() < 1e-3, "{ray:?}");
    assert!(ray.point.y.abs() < 1e-3 && ray.normal.abs_diff_eq(Vec3::Y, 1e-3));
    assert_eq!(ray.entity, solid);
    let sphere = batch.hits[1].expect("the sphere hits the box");
    assert!((sphere.distance - 3.75).abs() < 1e-2, "the sphere's surface touches, not its centre: {sphere:?}");
    assert!(batch.hits[2].is_none(), "nothing out there");
}

#[cfg(feature = "avian2d")]
#[test]
fn a_planar_ray_asks_the_2d_world() {
    let mut app = physics_app();
    app.world_mut().spawn((renzora_physics::Physics2d, floor(false, Vec3::new(0.0, -0.5, 0.0))));
    for _ in 0..5 {
        app.update();
    }
    let batch = cast(&mut app, vec![down(Vec3::new(0.0, 3.0, 0.0), 0.0, true)]);
    let hit = batch.hits[0].expect("the 2D ray hits the 2D box");
    assert!((hit.distance - 3.0).abs() < 1e-3, "{hit:?}");
    assert!(hit.normal.abs_diff_eq(Vec3::Y, 1e-3));
}
