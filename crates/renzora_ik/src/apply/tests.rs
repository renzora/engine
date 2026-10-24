//! The systems end to end on a hand-built skeleton, no renderer and no
//! physics backend: what the solver tests cannot show is that the pass reads
//! the right bones, writes them in the right space, and does not creep when
//! nothing re-animates a bone between frames.

use crate::components::{FootIk, LookAt, SpringBones, TwoBoneIk};
use crate::IkPlugin;
use bevy::prelude::*;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, IkPlugin));
    app
}

fn world_pos(app: &mut App, e: Entity) -> Vec3 {
    let mut q = app.world_mut().query::<(&Transform, Option<&ChildOf>)>();
    let w = app.world();
    let mut t = *q.get(w, e).unwrap().0;
    let mut cur = e;
    while let Some(p) = q.get(w, cur).unwrap().1 {
        let p = p.parent();
        t = q.get(w, p).unwrap().0.mul_transform(t);
        cur = p;
    }
    t.translation
}

/// Hips at 1 m, a leg hanging straight-ish down to an ankle at ~0.1 m.
fn leg(app: &mut App) -> (Entity, Entity, Entity, Entity, Entity) {
    let w = app.world_mut();
    let root = w.spawn((Name::new("Character"), Transform::IDENTITY)).id();
    let hips = w.spawn((Name::new("Hips"), Transform::from_xyz(0.0, 1.0, 0.0), ChildOf(root))).id();
    let thigh = w.spawn((Name::new("LeftUpLeg"), Transform::from_xyz(0.1, 0.0, 0.0), ChildOf(hips))).id();
    let knee = w.spawn((Name::new("LeftLeg"), Transform::from_xyz(0.0, -0.45, 0.03), ChildOf(thigh))).id();
    let ankle = w.spawn((Name::new("LeftFoot"), Transform::from_xyz(0.0, -0.45, -0.03), ChildOf(knee))).id();
    (root, hips, thigh, knee, ankle)
}

#[test]
fn two_bone_ik_reaches_its_target() {
    let mut app = app();
    let (_, _, _, _, ankle) = leg(&mut app);
    let target = Vec3::new(0.3, 0.4, 0.3);
    app.world_mut().entity_mut(ankle).insert(TwoBoneIk { target_position: target, ..default() });
    app.update();
    app.update();
    let got = world_pos(&mut app, ankle);
    assert!(got.distance(target) < 1e-3, "ankle at {got:?}");
}

#[test]
fn a_named_target_is_followed() {
    let mut app = app();
    let (_, _, _, _, ankle) = leg(&mut app);
    let target = Vec3::new(-0.1, 0.5, 0.35);
    app.world_mut().spawn((Name::new("HandTarget"), Transform::from_translation(target)));
    app.world_mut().entity_mut(ankle).insert(TwoBoneIk { target: "HandTarget".into(), ..default() });
    for _ in 0..3 {
        app.update();
    }
    assert!(world_pos(&mut app, ankle).distance(target) < 1e-3);
}

#[test]
fn a_partial_weight_does_not_creep_on_an_unanimated_bone() {
    let mut app = app();
    let (_, _, _, _, ankle) = leg(&mut app);
    app.world_mut()
        .entity_mut(ankle)
        .insert(TwoBoneIk { target_position: Vec3::new(0.4, 0.5, 0.2), weight: 0.5, ..default() });
    app.update();
    app.update();
    let first = world_pos(&mut app, ankle);
    for _ in 0..10 {
        app.update();
    }
    let later = world_pos(&mut app, ankle);
    assert!(first.distance(later) < 1e-5, "{first:?} crept to {later:?}");
}

#[test]
fn foot_ik_without_physics_stands_on_the_fallback_plane() {
    let mut app = app();
    let (root, hips, _, _, ankle) = leg(&mut app);
    let animated = world_pos(&mut app, ankle);
    // Ground 0.15 below the character's floor: the foot and hips go down.
    app.world_mut().entity_mut(root).insert(FootIk {
        right_foot: String::new(),
        ground_height: -0.15,
        smoothing: 0.0,
        ..default()
    });
    for _ in 0..4 {
        app.update();
    }
    let planted = world_pos(&mut app, ankle);
    assert!((planted.y - (animated.y - 0.15)).abs() < 1e-3, "ankle {planted:?}, animated {animated:?}");
    assert!((world_pos(&mut app, hips).y - 0.85).abs() < 1e-3);
}

#[test]
fn look_at_turns_the_bone() {
    let mut app = app();
    let head = app.world_mut().spawn((Name::new("Head"), Transform::from_xyz(0.0, 1.6, 0.0))).id();
    let target = Vec3::new(1.0, 1.6, 1.0);
    app.world_mut().entity_mut(head).insert(LookAt { target_position: target, max_angle: 180.0, ..default() });
    app.update();
    let rot = app.world().get::<Transform>(head).unwrap().rotation;
    let dir = rot * Vec3::Z;
    assert!(dir.distance((target - Vec3::new(0.0, 1.6, 0.0)).normalize()) < 1e-4);
}

#[test]
fn spring_bones_lag_behind_motion() {
    let mut app = app();
    let w = app.world_mut();
    let root = w.spawn(Transform::IDENTITY).id();
    let b0 = w.spawn((Transform::from_xyz(0.0, 1.0, 0.0), ChildOf(root))).id();
    let b1 = w.spawn((Transform::from_xyz(0.0, 0.2, 0.0), ChildOf(b0))).id();
    let b2 = w.spawn((Transform::from_xyz(0.0, 0.2, 0.0), ChildOf(b1))).id();
    w.entity_mut(b0).insert(SpringBones { gravity: Vec3::ZERO, ..default() });
    app.update();
    app.update();
    // Move the character sideways (below the teleport distance): the tip lags.
    app.world_mut().get_mut::<Transform>(root).unwrap().translation.x = 0.3;
    std::thread::sleep(std::time::Duration::from_millis(16));
    app.update();
    let tip = world_pos(&mut app, b2);
    assert!(tip.x < 0.3 - 1e-4, "tip did not lag: {tip:?}");
    assert!(tip.is_finite());
}
