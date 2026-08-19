//! End-to-end checks of the traversal controller against real avian geometry.
//!
//! These run the actual app shape the runtime has (minus the renderer): the
//! same `renzora_physics` auto-init that turns a `CollisionShapeData` into an
//! avian collider, the same spatial queries the probe casts against, and the
//! same `Update` schedule. What they are guarding is the classification — a
//! rail must be vaulted *over* and a platform climbed *onto*, and that split is
//! decided entirely by geometry the probe reads, so it is exactly the thing
//! that breaks silently.

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use renzora_parkour::{
    ParkourBlocker, ParkourController, ParkourInput, ParkourLadder, ParkourPlugin, ParkourReadState,
};
use renzora_physics::{
    CollisionShapeData, CollisionShapeType, PhysicsBodyData, PhysicsPlugin,
};

/// The runtime's app shape, minus render. Manual time so `app.update()`
/// advances a deterministic 1/60 s per call, and `finish`/`cleanup` because
/// avian registers its diagnostics resources in `Plugin::finish`, which a bare
/// update loop never calls.
fn parkour_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::diagnostic::DiagnosticsPlugin,
        bevy::asset::AssetPlugin::default(),
        TransformPlugin,
    ));
    app.init_asset::<Mesh>();
    app.add_plugins((PhysicsPlugin, ParkourPlugin));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / 60.0,
    )));
    app.finish();
    app.cleanup();
    app
}

/// A static box of `half_extents`, centred at `centre`.
fn block(app: &mut App, centre: Vec3, half_extents: Vec3) -> Entity {
    app.world_mut()
        .spawn((
            PhysicsBodyData::static_body(),
            renzora_physics::auto_fit::SkipAutoFit,
            CollisionShapeData {
                shape_type: CollisionShapeType::Box,
                half_extents,
                ..Default::default()
            },
            Transform::from_translation(centre),
        ))
        .id()
}

/// Floor spanning the play area, with its top surface at y = 0.
fn floor(app: &mut App) {
    block(app, Vec3::new(0.0, -0.5, 0.0), Vec3::new(40.0, 0.5, 40.0));
}

fn character(app: &mut App, at: Vec3) -> Entity {
    character_with(app, at, ParkourController::default())
}

fn character_with(app: &mut App, at: Vec3, controller: ParkourController) -> Entity {
    app.world_mut()
        .spawn((controller, Transform::from_translation(at)))
        .id()
}

/// Press one of the one-shot inputs for the next frame.
fn press(app: &mut App, who: Entity, set: impl Fn(&mut ParkourInput)) {
    if let Some(mut input) = app.world_mut().get_mut::<ParkourInput>(who) {
        set(&mut input);
    }
}

/// Run `frames` updates, re-stating `move_dir` each frame — the controller
/// consumes it, exactly as a script calling `parkour_move()` every frame does.
fn run(app: &mut App, who: Entity, move_dir: Vec3, frames: usize) {
    for _ in 0..frames {
        if let Some(mut input) = app.world_mut().get_mut::<ParkourInput>(who) {
            input.move_dir = move_dir;
        }
        app.update();
    }
}

fn position(app: &App, who: Entity) -> Vec3 {
    app.world().get::<Transform>(who).unwrap().translation
}

fn read(app: &App, who: Entity) -> ParkourReadState {
    app.world().get::<ParkourReadState>(who).cloned().unwrap()
}

/// Watch every state the character passes through, so a test can assert that a
/// traversal actually played rather than only checking where they ended up —
/// sliding up a ramp and mantling look the same from the final position alone.
fn run_watching(app: &mut App, who: Entity, move_dir: Vec3, frames: usize) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for _ in 0..frames {
        if let Some(mut input) = app.world_mut().get_mut::<ParkourInput>(who) {
            input.move_dir = move_dir;
        }
        app.update();
        let state = read(app, who).state;
        // Skip the very first tick: the scratch components are inserted by
        // that frame's commands, so the drive system has not written a state
        // yet and the mirror still holds its spawn default.
        if !state.is_empty() && seen.last() != Some(&state) {
            seen.push(state);
        }
    }
    seen
}

#[test]
fn character_falls_and_lands_on_the_floor() {
    let mut app = parkour_app();
    floor(&mut app);
    let who = character(&mut app, Vec3::new(0.0, 2.0, 0.0));

    run(&mut app, who, Vec3::ZERO, 90);

    let p = position(&app, who);
    assert!(
        p.y.abs() < 0.05,
        "expected to settle on the floor at y≈0, got {p:?}"
    );
    assert!(read(&app, who).grounded, "should report grounded once landed");
}

#[test]
fn runs_at_a_rail_and_vaults_over_it() {
    let mut app = parkour_app();
    floor(&mut app);
    // A 0.9 m rail, 0.5 m thick: low enough to vault, and the floor on the far
    // side drops back to y = 0, which is what makes it a rail and not a wall.
    block(&mut app, Vec3::new(3.0, 0.45, 0.0), Vec3::new(0.25, 0.45, 3.0));
    let who = character(&mut app, Vec3::new(0.0, 0.0, 0.0));

    let states = run_watching(&mut app, who, Vec3::X, 180);

    assert!(
        states.iter().any(|s| s == "vaulting"),
        "expected a vault; states were {states:?}"
    );
    let p = position(&app, who);
    assert!(p.x > 3.4, "expected to end up past the rail, got {p:?}");
    assert!(p.y.abs() < 0.1, "expected to land back on the floor, got {p:?}");
}

#[test]
fn runs_at_a_platform_and_mantles_onto_it() {
    let mut app = parkour_app();
    floor(&mut app);
    // 1.5 m tall and 6 m deep: too tall to vault, and there is no far side to
    // drop to, so the only way up is onto it.
    block(&mut app, Vec3::new(5.0, 0.75, 0.0), Vec3::new(3.0, 0.75, 3.0));
    let who = character(&mut app, Vec3::new(0.0, 0.0, 0.0));

    let states = run_watching(&mut app, who, Vec3::X, 90);
    assert!(
        states.iter().any(|s| s == "mantling"),
        "expected a mantle; states were {states:?}"
    );

    // Let go of the stick and let the mantle finish — holding forward would
    // just walk the character across the platform and off the far side, which
    // is correct behaviour but tells us nothing about the mantle.
    run(&mut app, who, Vec3::ZERO, 90);

    let p = position(&app, who);
    assert!(
        (p.y - 1.5).abs() < 0.1,
        "expected to end up standing on the 1.5 m platform, got {p:?}"
    );
    assert!(p.x > 2.2, "expected to end up on top of it, got {p:?}");
}

#[test]
fn a_blocker_stops_the_rail_being_vaulted() {
    let mut app = parkour_app();
    floor(&mut app);
    let rail = block(&mut app, Vec3::new(3.0, 0.45, 0.0), Vec3::new(0.25, 0.45, 3.0));
    app.world_mut().entity_mut(rail).insert(ParkourBlocker {});
    let who = character(&mut app, Vec3::new(0.0, 0.0, 0.0));

    let states = run_watching(&mut app, who, Vec3::X, 180);

    assert!(
        !states.iter().any(|s| s == "vaulting"),
        "blocked geometry must not be vaulted; states were {states:?}"
    );
    let p = position(&app, who);
    assert!(
        p.x < 2.8,
        "expected to be stopped in front of the rail, got {p:?}"
    );
}

#[test]
fn climbs_a_ladder_and_stays_on_it() {
    let mut app = parkour_app();
    floor(&mut app);
    // A 4 m wall — too tall to mantle, so nothing but the ladder marker can
    // get the character up it.
    let wall = block(&mut app, Vec3::new(2.0, 2.0, 0.0), Vec3::new(0.25, 2.0, 1.0));
    app.world_mut()
        .entity_mut(wall)
        .insert(ParkourLadder::default());
    let who = character(&mut app, Vec3::new(0.0, 0.0, 0.0));

    // Walk into it: `auto_attach` latches on without an explicit action.
    run(&mut app, who, Vec3::X, 60);
    assert_eq!(
        read(&app, who).state,
        "climbing",
        "walking into an auto-attach ladder should mount it"
    );

    // Then climb. `y` is the climb axis; the horizontal part is ignored.
    run(&mut app, who, Vec3::Y, 60);
    let p = position(&app, who);
    assert!(
        p.y > 1.0,
        "a second of climbing at 2.2 m/s should gain real height, got {p:?}"
    );
}

#[test]
fn a_tall_wall_is_neither_a_ledge_nor_a_traversal() {
    let mut app = parkour_app();
    floor(&mut app);
    block(&mut app, Vec3::new(2.0, 2.0, 0.0), Vec3::new(0.25, 2.0, 3.0));
    let who = character(&mut app, Vec3::new(0.0, 0.0, 0.0));

    let states = run_watching(&mut app, who, Vec3::X, 120);

    // "airborne" is the spawn default the mirror carries until the drive
    // system's first tick; what matters is that no traversal state ever shows.
    assert!(
        states.iter().all(|s| s == "airborne" || s == "grounded"),
        "a 4 m wall should just stop the character; states were {states:?}"
    );
    let r = read(&app, who);
    assert!(
        !r.can_mantle && !r.can_vault && r.ledge_height == 0.0,
        "an unreachable wall must not be reported as a ledge: {r:?}"
    );
}

#[test]
fn script_actions_become_movement_intent() {
    use renzora::{ScriptAction, ScriptActionValue};

    let mut app = parkour_app();
    floor(&mut app);
    let who = character(&mut app, Vec3::ZERO);
    app.update();

    let mut args = std::collections::HashMap::new();
    args.insert("x".to_string(), ScriptActionValue::Float(1.0));
    args.insert("y".to_string(), ScriptActionValue::Float(0.0));
    args.insert("z".to_string(), ScriptActionValue::Float(0.0));
    app.world_mut().trigger(ScriptAction {
        name: "parkour_move".to_string(),
        entity: who,
        target_entity: None,
        args,
    });
    app.world_mut().trigger(ScriptAction {
        name: "parkour_jump".to_string(),
        entity: who,
        target_entity: None,
        args: std::collections::HashMap::new(),
    });

    let input = app.world().get::<ParkourInput>(who).unwrap();
    assert_eq!(input.move_dir, Vec3::X);
    assert!(input.jump_pressed);
}


#[test]
fn jumps_at_a_high_ledge_and_hangs_off_it() {
    let mut app = parkour_app();
    floor(&mut app);
    // 2.6 m: out of reach standing (`mantle_max_height` is 2.3), but well
    // inside the grab band once a jump has raised the character's feet.
    block(&mut app, Vec3::new(2.0, 1.3, 0.0), Vec3::new(0.25, 1.3, 3.0));
    let who = character(
        &mut app,
        Vec3::new(0.0, 0.0, 0.0),
    );

    // Run up to the wall, then jump into it.
    run(&mut app, who, Vec3::X, 30);
    press(&mut app, who, |i| i.jump_pressed = true);
    run(&mut app, who, Vec3::X, 30);

    assert_eq!(
        read(&app, who).state,
        "hanging",
        "jumping at a 2.6 m ledge should catch it"
    );
    let hanging_at = position(&app, who);
    assert!(
        hanging_at.y > 0.4 && hanging_at.y < 2.6,
        "should be hanging below the lip, not standing on it: {hanging_at:?}"
    );

    // A hang holds position — it must not sag under gravity.
    run(&mut app, who, Vec3::ZERO, 30);
    let still = position(&app, who);
    assert!(
        (still.y - hanging_at.y).abs() < 0.01,
        "a hang should hold its height: {hanging_at:?} -> {still:?}"
    );

    // Climbing up from the hang mantles onto the top.
    press(&mut app, who, |i| i.action_pressed = true);
    run(&mut app, who, Vec3::ZERO, 90);
    let p = position(&app, who);
    assert!(
        (p.y - 2.6).abs() < 0.15,
        "climbing up should end standing on the 2.6 m ledge, got {p:?}"
    );
}

#[test]
fn grabs_a_swing_anchor_and_swings_from_it() {
    use renzora_parkour::ParkourSwingAnchor;

    let mut app = parkour_app();
    floor(&mut app);
    let anchor = app
        .world_mut()
        .spawn((
            ParkourSwingAnchor::default(),
            Transform::from_xyz(3.0, 4.0, 0.0),
        ))
        .id();
    let who = character(&mut app, Vec3::new(0.0, 0.0, 0.0));

    // Jump toward the anchor and reach for it.
    run(&mut app, who, Vec3::X, 20);
    press(&mut app, who, |i| i.jump_pressed = true);
    run(&mut app, who, Vec3::X, 10);
    press(&mut app, who, |i| i.action_pressed = true);
    run(&mut app, who, Vec3::X, 5);

    assert_eq!(
        read(&app, who).state,
        "swinging",
        "reaching for an anchor in range should grab it"
    );

    // The rope keeps a constant length: the pendulum must not stretch.
    let pivot = position(&app, anchor);
    let hand = |app: &App| position(app, who) + Vec3::Y * 1.62;
    let rope = (hand(&app) - pivot).length();
    run(&mut app, who, Vec3::ZERO, 45);
    let rope_later = (hand(&app) - pivot).length();
    assert!(
        (rope - rope_later).abs() < 0.05,
        "rope length should hold: {rope} -> {rope_later}"
    );
    assert_eq!(read(&app, who).state, "swinging", "should still be swinging");

    // Letting go hands the character back to gravity.
    press(&mut app, who, |i| i.release_pressed = true);
    run(&mut app, who, Vec3::ZERO, 2);
    assert_eq!(read(&app, who).state, "airborne", "release should let go");
}


#[test]
fn runs_along_a_wall_and_kicks_off_it() {
    let mut app = parkour_app();
    floor(&mut app);
    // A long wall just to the character's right as they run along +X. The
    // side probe reaches `radius + 0.35` from the capsule's centre line, so
    // the face has to sit inside 0.7 for the run to start.
    block(&mut app, Vec3::new(5.0, 3.0, 0.85), Vec3::new(10.0, 3.0, 0.25));
    let who = character(&mut app, Vec3::new(0.0, 0.0, 0.0));

    // Get up to running speed, then jump into the wall run.
    if let Some(mut input) = app.world_mut().get_mut::<ParkourInput>(who) {
        input.sprint = true;
    }
    run(&mut app, who, Vec3::X, 30);
    press(&mut app, who, |i| i.jump_pressed = true);
    run(&mut app, who, Vec3::X, 5);

    assert_eq!(
        read(&app, who).state,
        "wall_running",
        "sprinting past a wall and jumping should start a wall run"
    );
    let during = position(&app, who);

    // A wall run keeps going forward, and sags far more slowly than gravity.
    run(&mut app, who, Vec3::X, 30);
    let later = position(&app, who);
    assert!(
        later.x > during.x + 2.0,
        "should carry along the wall, {during:?} -> {later:?}"
    );
    assert!(
        later.y > during.y - 1.5,
        "wall-run gravity should be much weaker than a fall, {during:?} -> {later:?}"
    );

    // Kicking off pushes away from the wall (its normal is -Z here).
    press(&mut app, who, |i| i.jump_pressed = true);
    run(&mut app, who, Vec3::X, 1);
    let r = read(&app, who);
    assert_eq!(r.state, "airborne", "a wall jump should leave the wall");
    assert!(
        r.velocity.z < -1.0 && r.velocity.y > 1.0,
        "a wall jump should go up and away from the wall, got {:?}",
        r.velocity
    );
}

#[test]
fn a_wall_run_ends_on_its_own_clock() {
    let mut app = parkour_app();
    floor(&mut app);
    block(&mut app, Vec3::new(20.0, 3.0, 0.85), Vec3::new(40.0, 3.0, 0.25));
    let who = character(&mut app, Vec3::new(0.0, 0.0, 0.0));

    if let Some(mut input) = app.world_mut().get_mut::<ParkourInput>(who) {
        input.sprint = true;
    }
    run(&mut app, who, Vec3::X, 30);
    press(&mut app, who, |i| i.jump_pressed = true);
    run(&mut app, who, Vec3::X, 5);
    assert_eq!(read(&app, who).state, "wall_running");

    // `wall_run_duration` is 1.5 s; well past it the run must have ended even
    // though the wall goes on forever.
    run(&mut app, who, Vec3::X, 150);
    assert_ne!(
        read(&app, who).state,
        "wall_running",
        "a wall run must not outlast wall_run_duration"
    );
}


/// An imported character normally carries its collider on a child mesh, not on
/// the entity the controller sits on. If the sweeps only excluded the
/// controller's own entity the capsule would collide with that child every
/// frame and the character would stand still forever, with nothing logged
/// anywhere to say why.
#[test]
fn a_collider_on_a_child_does_not_block_its_own_character() {
    let mut app = parkour_app();
    floor(&mut app);
    let who = character(&mut app, Vec3::new(0.0, 0.0, 0.0));

    // A body-sized collider parked on a child, the shape a GLB import takes.
    let body = app
        .world_mut()
        .spawn((
            PhysicsBodyData::static_body(),
            renzora_physics::auto_fit::SkipAutoFit,
            CollisionShapeData {
                shape_type: CollisionShapeType::Box,
                half_extents: Vec3::new(0.4, 0.9, 0.4),
                offset: Vec3::new(0.0, 0.9, 0.0),
                ..Default::default()
            },
            Transform::from_xyz(0.0, 0.0, 0.0),
        ))
        .id();
    app.world_mut().entity_mut(who).add_child(body);

    run(&mut app, who, Vec3::X, 60);

    let p = position(&app, who);
    assert!(
        p.x > 2.0,
        "the character's own collider must not stop it; got {p:?}"
    );
}

/// `facing_offset` must turn the mesh and nothing else. A glTF character
/// authored facing +Z needs 180 to stop running backwards, and that must not
/// quietly reverse where they walk or which way they probe for ledges.
#[test]
fn facing_offset_turns_the_model_without_turning_the_movement() {
    let mut app = parkour_app();
    floor(&mut app);
    let plain = character(&mut app, Vec3::new(0.0, 0.0, 0.0));
    let flipped = character_with(
        &mut app,
        Vec3::new(0.0, 0.0, 10.0),
        ParkourController {
            facing_offset: 180.0,
            ..Default::default()
        },
    );

    for _ in 0..90 {
        for who in [plain, flipped] {
            if let Some(mut input) = app.world_mut().get_mut::<ParkourInput>(who) {
                input.move_dir = Vec3::X;
            }
        }
        app.update();
    }

    // Both travel the same way.
    let a = position(&app, plain);
    let b = position(&app, flipped);
    assert!(a.x > 3.0, "plain character should run +X, got {a:?}");
    assert!(
        (b.x - a.x).abs() < 0.2,
        "the offset must not change where the character goes: {a:?} vs {b:?}"
    );

    // ...but they point opposite ways.
    let fwd_plain = app.world().get::<Transform>(plain).unwrap().forward();
    let fwd_flipped = app.world().get::<Transform>(flipped).unwrap().forward();
    assert!(
        fwd_plain.dot(*fwd_flipped) < -0.95,
        "180 degrees of offset should reverse the mesh: {fwd_plain:?} vs {fwd_flipped:?}"
    );
    // The plain one faces the way it runs.
    assert!(
        fwd_plain.dot(Vec3::X) > 0.95,
        "plain character should face its own movement, got {fwd_plain:?}"
    );
}


/// The character must actually travel at `walk_speed`. It once did not: the
/// ground snap parked the capsule exactly on the contact plane, and the next
/// horizontal sweep intermittently clipped against the floor, costing about a
/// quarter of the speed. Nothing looked broken — the character just felt
/// sluggish — which is why this is measured rather than eyeballed.
#[test]
fn walks_at_the_speed_it_was_asked_for() {
    let mut app = parkour_app();
    floor(&mut app);
    let controller = ParkourController::default();
    let who = character(&mut app, Vec3::ZERO);

    let seconds = 1.5;
    let frames = (seconds * 60.0) as usize;
    run(&mut app, who, Vec3::X, frames);

    // Distance at full speed, less the ramp up to it.
    let ramp = controller.walk_speed / controller.acceleration;
    let want = controller.walk_speed * seconds - 0.5 * controller.walk_speed * ramp;
    let got = position(&app, who).x;
    assert!(
        got > want * 0.95,
        "expected roughly {want:.2} m of walking, got {got:.2} m"
    );
}
