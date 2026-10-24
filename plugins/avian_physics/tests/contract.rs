//! The physics contract end to end on the avian backends: what scripts push
//! (`set_velocity`, `apply_force`, `move_controller`), what they read back
//! (`PhysicsReadState`, `CollisionReadState`), pausing, terrain-style stripping
//! and mesh colliders. These are the paths a second backend has to reproduce,
//! so they are written against renzora's components and actions, never avian's,
//! except where a test checks avian itself was told.

use std::collections::HashMap;
use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
#[allow(unused_imports)]
use renzora::physics::{
    BodyVelocity, CollisionStarted, JointKind, Overlap, OverlapBatch, PhysicsBackends,
    PhysicsCommand, PhysicsCommands, PhysicsContacts, PhysicsDim, PhysicsJoint, ResolvedShape,
};
use renzora::{ScriptAction, ScriptActionValue};
use renzora_physics::auto_fit::SkipAutoFit;
use renzora_physics::read_state::CollisionReadState;
use renzora_physics::{
    CollisionShapeData, Physics2d, PhysicsBodyData, PhysicsPlugin, PhysicsReadState,
};
use avian_physics::AvianPhysicsPlugin;

fn physics_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::diagnostic::DiagnosticsPlugin,
        bevy::asset::AssetPlugin::default(),
        TransformPlugin,
    ));
    app.init_asset::<Mesh>();
    app.add_plugins((PhysicsPlugin, AvianPhysicsPlugin));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / 60.0,
    )));
    app.finish();
    app.cleanup();
    app
}

fn step(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

fn action(app: &mut App, entity: Entity, name: &str, v: Vec3) {
    let args: HashMap<String, ScriptActionValue> = [("x", v.x), ("y", v.y), ("z", v.z)]
        .into_iter()
        .map(|(k, f)| (k.to_string(), ScriptActionValue::Float(f)))
        .collect();
    app.world_mut().trigger(ScriptAction {
        name: name.to_string(),
        entity,
        target_entity: None,
        args,
    });
}

fn pos(app: &App, entity: Entity) -> Vec3 {
    app.world().get::<Transform>(entity).unwrap().translation
}

fn read(app: &App, entity: Entity) -> PhysicsReadState {
    app.world().get::<PhysicsReadState>(entity).unwrap().clone()
}

/// A static 20×1×20 slab whose top face is at y = 0.
fn floor(app: &mut App) -> Entity {
    app.world_mut()
        .spawn((
            Name::new("Floor"),
            SkipAutoFit,
            PhysicsBodyData::static_body(),
            CollisionShapeData::cuboid(Vec3::new(10.0, 0.5, 10.0)),
            Transform::from_xyz(0.0, -0.5, 0.0),
        ))
        .id()
}

/// A dynamic unit ball.
fn ball(app: &mut App, at: Vec3, gravity_scale: f32) -> Entity {
    app.world_mut()
        .spawn((
            Name::new("Ball"),
            SkipAutoFit,
            PhysicsBodyData {
                gravity_scale,
                ..Default::default()
            },
            CollisionShapeData::sphere(0.5),
            Transform::from_translation(at),
        ))
        .id()
}

#[test]
fn a_falling_body_lands_and_reports_the_contact() {
    let mut app = physics_app();
    floor(&mut app);
    let ball = ball(&mut app, Vec3::new(0.0, 3.0, 0.0), 1.0);

    let mut entered = String::new();
    for _ in 0..150 {
        app.update();
        let rs = app.world().get::<CollisionReadState>(ball).unwrap();
        if rs.entered {
            entered = rs.entered_name.clone();
        }
    }

    let y = pos(&app, ball).y;
    assert!((y - 0.5).abs() < 0.1, "the ball should rest on the floor, at y = {y}");
    assert_eq!(entered, "Floor", "landing should have reported entering the floor");
    assert!(app.world().get::<CollisionReadState>(ball).unwrap().colliding);
}

#[test]
fn set_velocity_moves_a_body_and_reads_back() {
    let mut app = physics_app();
    let ball = ball(&mut app, Vec3::ZERO, 0.0);
    step(&mut app, 2);

    action(&mut app, ball, "set_velocity", Vec3::new(2.0, 0.0, 0.0));
    step(&mut app, 60);

    let v = read(&app, ball).velocity;
    assert!((v - Vec3::new(2.0, 0.0, 0.0)).length() < 0.05, "read-back velocity was {v}");
    let x = pos(&app, ball).x;
    assert!((x - 2.0).abs() < 0.2, "one second at 2 m/s should be about 2 m, got {x}");
}

/// A steady force pushed every frame accelerates a body at F/m. Fired from an
/// `Update` system, which is when scripts fire it: before the backend contract
/// this exact setup left the body at rest, because the force was removed in
/// `PostUpdate`, before any physics step could see it.
#[test]
fn apply_force_every_frame_accelerates_a_body() {
    let mut app = physics_app();
    let ball = ball(&mut app, Vec3::ZERO, 0.0);
    step(&mut app, 2);

    app.add_systems(Update, move |mut commands: Commands| {
        let args = [("x", 1.0), ("y", 0.0), ("z", 0.0)]
            .into_iter()
            .map(|(k, f)| (k.to_string(), ScriptActionValue::Float(f)))
            .collect();
        commands.trigger(ScriptAction {
            name: "apply_force".to_string(),
            entity: ball,
            target_entity: None,
            args,
        });
    });
    step(&mut app, 60);

    let v = read(&app, ball).velocity.x;
    assert!((0.8..1.2).contains(&v), "1 N on 1 kg for 1 s should be about 1 m/s, got {v}");
}

#[test]
fn pause_freezes_the_world_and_unpause_resumes_it() {
    let mut app = physics_app();
    let ball = ball(&mut app, Vec3::new(0.0, 10.0, 0.0), 1.0);
    step(&mut app, 2);

    renzora_physics::pause(app.world_mut());
    step(&mut app, 1);
    let paused_at = pos(&app, ball).y;
    step(&mut app, 30);
    assert_eq!(pos(&app, ball).y, paused_at, "nothing moves while paused");

    renzora_physics::unpause(app.world_mut());
    step(&mut app, 30);
    assert!(pos(&app, ball).y < paused_at - 0.5, "it falls again once unpaused");
}

/// The terrain streaming path: a stripped collider leaves the world, stays out
/// while the entity keeps its data, and comes back when its shape is
/// re-inserted.
#[test]
fn a_stripped_collider_stays_out_until_its_shape_changes() {
    let mut app = physics_app();
    let floor = floor(&mut app);
    let ball = ball(&mut app, Vec3::new(0.0, 1.0, 0.0), 1.0);
    step(&mut app, 90);
    assert!((pos(&app, ball).y - 0.5).abs() < 0.1, "resting before the strip");

    let mut commands = app.world_mut().commands();
    renzora_physics::despawn_physics_components(&mut commands, floor);
    app.world_mut().flush();
    step(&mut app, 60);
    assert!(pos(&app, ball).y < -0.5, "the ball falls through a stripped floor");
    assert!(app.world().get::<avian3d::prelude::Collider>(floor).is_none());

    app.world_mut()
        .entity_mut(floor)
        .insert(CollisionShapeData::cuboid(Vec3::new(10.0, 0.5, 10.0)));
    step(&mut app, 2);
    assert!(
        app.world().get::<avian3d::prelude::Collider>(floor).is_some(),
        "re-inserting the shape brings the collider back"
    );
}

/// `move_controller` is collide-and-slide through the backend's shape cast: a
/// diagonal push into a wall keeps the along-wall part, and the capsule ends
/// standing on the floor.
#[test]
fn move_controller_slides_along_a_wall_and_stands_on_the_floor() {
    let mut app = physics_app();
    floor(&mut app);
    // Wall face at x = 1.5.
    app.world_mut().spawn((
        SkipAutoFit,
        PhysicsBodyData::static_body(),
        CollisionShapeData::cuboid(Vec3::new(0.5, 2.0, 10.0)),
        Transform::from_xyz(2.0, 2.0, 0.0),
    ));
    // Capsule half-height 0.5 + radius 0.3: its bottom is 0.8 below its centre.
    let character = app
        .world_mut()
        .spawn((
            SkipAutoFit,
            PhysicsBodyData::kinematic_body(),
            CollisionShapeData::capsule(0.3, 0.5),
            Transform::from_xyz(0.0, 0.85, 0.0),
        ))
        .id();
    step(&mut app, 3);

    for _ in 0..40 {
        action(&mut app, character, "kinematic_slide", Vec3::new(0.1, -0.05, 0.05));
        app.update();
    }

    let p = pos(&app, character);
    assert!(p.x < 1.25, "stopped by the wall, x = {}", p.x);
    assert!(p.x > 1.0, "reached the wall, x = {}", p.x);
    assert!(p.z > 1.5, "kept sliding along it, z = {}", p.z);
    assert!((p.y - 0.8).abs() < 0.05, "standing on the floor, y = {}", p.y);
    assert!(read(&app, character).grounded);
}

#[test]
fn a_mesh_collider_is_built_once_its_mesh_loads() {
    let mut app = physics_app();
    let mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Mesh::from(Cuboid::new(20.0, 1.0, 20.0)));
    app.world_mut().spawn((
        SkipAutoFit,
        Mesh3d(mesh),
        PhysicsBodyData::static_body(),
        CollisionShapeData::mesh(),
        Transform::from_xyz(0.0, -0.5, 0.0),
    ));
    let ball = ball(&mut app, Vec3::new(0.0, 2.0, 0.0), 1.0);
    step(&mut app, 120);

    let y = pos(&app, ball).y;
    assert!((y - 0.5).abs() < 0.1, "the ball rests on the trimesh, y = {y}");
}

#[test]
fn set_velocity_reaches_a_2d_body() {
    let mut app = physics_app();
    let body = app
        .world_mut()
        .spawn((
            Physics2d,
            SkipAutoFit,
            PhysicsBodyData {
                gravity_scale: 0.0,
                ..Default::default()
            },
            CollisionShapeData::sphere(5.0),
            Transform::default(),
        ))
        .id();
    step(&mut app, 2);

    action(&mut app, body, "set_velocity", Vec3::new(0.0, 30.0, 99.0));
    step(&mut app, 60);

    let v = read(&app, body).velocity;
    assert!((v - Vec3::new(0.0, 30.0, 0.0)).length() < 0.5, "2D velocity drops z, got {v}");
    assert!((pos(&app, body).y - 30.0).abs() < 2.0);
}

// ── The rest of the contract: joints, layers, events, impulses, queries ──

fn push(app: &mut App, command: PhysicsCommand) {
    app.world_mut().resource_mut::<PhysicsCommands>().push(command);
}

/// A landing triggers `CollisionStarted` once, carrying a contact whose normal
/// points up at the ball and whose impulse is the landing's.
#[test]
fn landing_triggers_collision_started_with_the_contact() {
    let mut app = physics_app();
    let floor = floor(&mut app);
    let ball = ball(&mut app, Vec3::new(0.0, 2.0, 0.0), 1.0);
    let started = std::sync::Arc::new(std::sync::Mutex::new(Vec::<CollisionStarted>::new()));
    let sink = started.clone();
    app.add_observer(move |t: On<CollisionStarted>| sink.lock().unwrap().push(*t.event()));
    step(&mut app, 120);

    let started = started.lock().unwrap();
    let landing: Vec<_> = started.iter().filter(|e| e.entity == ball && e.other == floor).collect();
    assert_eq!(landing.len(), 1, "one start for one landing: {started:?}");
    let contact = landing[0].contact;
    assert!(contact.normal.dot(Vec3::Y) > 0.9, "pushed up, normal {}", contact.normal);
    assert!(contact.point.y.abs() < 0.1, "touching at the floor's top, {}", contact.point);
    let touching = app.world().get::<PhysicsContacts>(ball).unwrap();
    assert!(touching.touches(floor));
}

/// A body whose layer the floor does not collide with falls straight through
/// it, while a default one lands.
#[test]
fn collision_layers_filter_from_either_side() {
    let mut app = physics_app();
    app.world_mut().spawn((
        SkipAutoFit,
        PhysicsBodyData::static_body(),
        CollisionShapeData { collides_with: 0b01, ..CollisionShapeData::cuboid(Vec3::new(10.0, 0.5, 10.0)) },
        Transform::from_xyz(0.0, -0.5, 0.0),
    ));
    let lander = ball(&mut app, Vec3::new(-2.0, 1.0, 0.0), 1.0);
    let ghost = app
        .world_mut()
        .spawn((
            SkipAutoFit,
            PhysicsBodyData::default(),
            CollisionShapeData { collision_layers: 0b10, ..CollisionShapeData::sphere(0.5) },
            Transform::from_xyz(2.0, 1.0, 0.0),
        ))
        .id();
    step(&mut app, 90);
    assert!((pos(&app, lander).y - 0.5).abs() < 0.1, "layer 0 lands");
    assert!(pos(&app, ghost).y < -1.0, "layer 1 is ignored by the floor");
}

/// A turned collider collides where it is turned to: a tall thin slab laid flat
/// by a quarter turn about Z catches a ball that would miss it upright.
#[test]
fn a_turned_collider_collides_turned() {
    let mut app = physics_app();
    app.world_mut().spawn((
        SkipAutoFit,
        PhysicsBodyData::static_body(),
        CollisionShapeData {
            rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            ..CollisionShapeData::cuboid(Vec3::new(0.5, 10.0, 10.0))
        },
        Transform::from_xyz(0.0, -0.5, 0.0),
    ));
    let ball = ball(&mut app, Vec3::new(3.0, 1.0, 0.0), 1.0);
    step(&mut app, 90);
    let y = pos(&app, ball).y;
    assert!((y - 0.5).abs() < 0.1, "rests on the slab laid flat, y = {y}");
}

/// A collider offset from its entity collides where it is offset to. A ball
/// whose collider hangs 1 m below the entity rests with the entity 1 m higher
/// than it would without the offset. Avian resets a `ColliderTransform` on a
/// body's own entity every step, so this held only once the offset was baked
/// into the collider.
#[test]
fn a_3d_collider_offset_is_kept() {
    let mut app = physics_app();
    floor(&mut app);
    let ball = app
        .world_mut()
        .spawn((
            SkipAutoFit,
            PhysicsBodyData::default(),
            CollisionShapeData { offset: Vec3::new(0.0, -1.0, 0.0), ..CollisionShapeData::sphere(0.5) },
            Transform::from_xyz(0.0, 3.0, 0.0),
        ))
        .id();
    step(&mut app, 180);
    let y = pos(&app, ball).y;
    assert!((y - 1.5).abs() < 0.1, "entity rests 1 m above its collider, y = {y}");
}

/// `AddImpulse` is momentum, so a 2 kg body pushed with 4 N·s moves at 2 m/s;
/// `SetAngularVelocity` spins it, and the spin reads back.
#[test]
fn impulses_divide_by_mass_and_spin_reads_back() {
    let mut app = physics_app();
    let body = app
        .world_mut()
        .spawn((
            SkipAutoFit,
            PhysicsBodyData { mass: 2.0, gravity_scale: 0.0, angular_damping: 0.0, ..Default::default() },
            CollisionShapeData::sphere(0.5),
            Transform::default(),
        ))
        .id();
    step(&mut app, 2);
    push(&mut app, PhysicsCommand::AddImpulse { entity: body, impulse: Vec3::new(4.0, 0.0, 0.0) });
    push(&mut app, PhysicsCommand::SetAngularVelocity { entity: body, velocity: Vec3::new(0.0, 3.0, 0.0) });
    step(&mut app, 10);
    let v = app.world().get::<BodyVelocity>(body).unwrap();
    assert!((v.linear.x - 2.0).abs() < 0.05, "4 N·s on 2 kg is 2 m/s, got {}", v.linear);
    assert!((v.angular.y - 3.0).abs() < 0.1, "spin reads back, got {}", v.angular);
}

/// Two bodies on a rope: the lower one falls until the rope is taut and then
/// hangs at its length.
#[test]
fn a_distance_joint_holds_a_body_at_its_length() {
    let mut app = physics_app();
    let anchor = app
        .world_mut()
        .spawn((
            SkipAutoFit,
            PhysicsBodyData::static_body(),
            CollisionShapeData::sphere(0.1),
            Transform::from_xyz(0.0, 5.0, 0.0),
        ))
        .id();
    let weight = ball(&mut app, Vec3::new(0.0, 4.5, 0.0), 1.0);
    // Spawned in the same frame as its bodies, as a ragdoll's joints are.
    app.world_mut().spawn(
        PhysicsJoint::new(anchor, weight, JointKind::Distance { min: 0.0, max: 2.0 })
            .with_collide_connected(false),
    );
    step(&mut app, 180);
    let p = pos(&app, weight);
    assert!((p.y - 3.0).abs() < 0.1, "hangs 2 m below the anchor, y = {}", p.y);

    // Despawning the joint is how it breaks.
    let joint = app.world_mut().query_filtered::<Entity, With<PhysicsJoint>>().single(app.world()).unwrap();
    app.world_mut().despawn(joint);
    step(&mut app, 30);
    assert!(pos(&app, weight).y < 2.5, "falls once the rope is gone");
}

/// A chain of ball joints built as a ragdoll's limb is (each link a child of
/// the one before) holds together while it swings: the world anchor points of
/// every joint stay on top of each other.
#[test]
fn a_spherical_chain_stays_linked() {
    let mut app = physics_app();
    let top = app
        .world_mut()
        .spawn((
            SkipAutoFit,
            PhysicsBodyData::static_body(),
            CollisionShapeData::sphere(0.1),
            Transform::from_xyz(0.0, 5.0, 0.0),
        ))
        .id();
    let mut prev = top;
    let mut links = Vec::new();
    for _ in 0..3 {
        let link = app
            .world_mut()
            .spawn((
                SkipAutoFit,
                PhysicsBodyData::default(),
                CollisionShapeData::sphere(0.1),
                Transform::from_xyz(0.5, 0.0, 0.0),
                ChildOf(prev),
            ))
            .id();
        app.world_mut().spawn(
            PhysicsJoint::new(
                prev,
                link,
                JointKind::Spherical { twist_axis: Vec3::Y, swing_limit: None, twist_limit: None },
            )
            .with_anchors(Vec3::new(0.25, 0.0, 0.0), Vec3::new(-0.25, 0.0, 0.0))
            .with_collide_connected(false),
        );
        links.push(link);
        prev = link;
    }
    let at = |app: &App, e: Entity, local: Vec3| app.world().get::<GlobalTransform>(e).unwrap().transform_point(local);
    let mut lowest = f32::MAX;
    for _ in 0..120 {
        app.update();
        let mut prev = top;
        for link in &links {
            let gap = at(&app, prev, Vec3::new(0.25, 0.0, 0.0)).distance(at(&app, *link, Vec3::new(-0.25, 0.0, 0.0)));
            // XPBD at six substeps lets a swinging chain stretch a few
            // centimetres (about 5 cm here, flat or nested alike). A joint that
            // is not solved at all comes apart by metres.
            assert!(gap < 0.1, "the joint holds, anchors {gap} apart");
            prev = *link;
        }
        lowest = lowest.min(at(&app, prev, Vec3::ZERO).y);
    }
    assert!(lowest < 3.8, "and the chain swings down, lowest end y = {lowest}");
}

/// Removing physics data from a live entity takes it out of the simulation:
/// a floor that loses its collider stops holding the ball up, and a ball that
/// loses only its body stops moving and stays where it is as plain scenery.
#[test]
fn removing_physics_data_takes_it_out_of_the_world() {
    let mut app = physics_app();
    let floor = floor(&mut app);
    let ball = ball(&mut app, Vec3::new(0.0, 1.0, 0.0), 1.0);
    let crate_box = app
        .world_mut()
        .spawn((
            SkipAutoFit,
            PhysicsBodyData { gravity_scale: 0.0, ..Default::default() },
            CollisionShapeData::sphere(0.5),
            Transform::from_xyz(5.0, 3.0, 0.0),
        ))
        .id();
    step(&mut app, 90);
    assert!((pos(&app, ball).y - 0.5).abs() < 0.1, "resting first");

    app.world_mut().entity_mut(floor).remove::<(CollisionShapeData, PhysicsBodyData)>();
    step(&mut app, 60);
    assert!(pos(&app, ball).y < -0.5, "falls once the floor's physics is gone");

    push(&mut app, PhysicsCommand::SetLinearVelocity { entity: crate_box, velocity: Vec3::X });
    step(&mut app, 5);
    app.world_mut().entity_mut(crate_box).remove::<PhysicsBodyData>();
    step(&mut app, 2);
    let held = pos(&app, crate_box);
    step(&mut app, 30);
    assert_eq!(pos(&app, crate_box), held, "a body without its data no longer moves");
}

/// A sphere placed over a box finds it; placed in the open, nothing.
#[test]
fn an_overlap_finds_what_it_touches() {
    let mut app = physics_app();
    let floor = floor(&mut app);
    step(&mut app, 2);
    let at = |y| Overlap::new(PhysicsDim::D3, ResolvedShape::Sphere { radius: 0.5 }, Vec3::new(0.0, y, 0.0));
    let mut batch = OverlapBatch { queries: vec![at(0.2), at(5.0)], hits: Vec::new() };
    PhysicsBackends::overlap(app.world(), &mut batch);
    assert_eq!(batch.hits[0], vec![floor]);
    assert!(batch.hits[1].is_empty());
}

/// Joints, layers and the rest exist in 2D too.
#[test]
fn a_2d_revolute_joint_swings_a_body_round_its_pin() {
    let mut app = physics_app();
    let pin = app
        .world_mut()
        .spawn((
            Physics2d,
            SkipAutoFit,
            PhysicsBodyData::static_body(),
            CollisionShapeData::sphere(0.1),
            Transform::default(),
        ))
        .id();
    let bob = app
        .world_mut()
        .spawn((
            Physics2d,
            SkipAutoFit,
            PhysicsBodyData::default(),
            CollisionShapeData::sphere(0.25),
            Transform::from_xyz(2.0, 0.0, 0.0),
        ))
        .id();
    app.world_mut().spawn(
        PhysicsJoint::new(pin, bob, JointKind::Revolute { axis: Vec3::Z, limits: None })
            .with_anchors(Vec3::ZERO, Vec3::new(-2.0, 0.0, 0.0))
            .with_collide_connected(false),
    );
    let mut lowest = f32::MAX;
    for _ in 0..120 {
        app.update();
        let p = pos(&app, bob).truncate();
        assert!((p.length() - 2.0).abs() < 0.1, "stays 2 from the pin, {}", p.length());
        lowest = lowest.min(p.y);
    }
    // A pendulum released level passes straight under its pin.
    assert!(lowest < -1.9, "swings down under gravity, lowest y = {lowest}");
}
