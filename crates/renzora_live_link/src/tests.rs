//! Two worlds kept in step through the real pipeline: change one, scan it,
//! apply the messages to the other, and check they agree. The worlds stand in
//! for the editor and the game, which share one type registry because they are
//! one executable.

use std::collections::HashSet;

use bevy::prelude::*;
use renzora_scene_api::SceneEntityId;

use crate::apply::{apply, Quiet};
use crate::tracker::{holds_entity, Tracker};
use crate::LinkMessage;

fn world() -> World {
    let mut world = World::new();
    let registry = AppTypeRegistry::default();
    {
        let mut r = registry.write();
        r.register::<Transform>();
        r.register::<GlobalTransform>();
        r.register::<Name>();
        r.register::<SceneEntityId>();
        r.register::<ChildOf>();
        r.register::<Visibility>();
    }
    world.insert_resource(registry);
    world
}

/// A world with one scene entity, id 1, as both sides have after Play.
fn scene() -> (World, Entity) {
    let mut world = world();
    let e = world
        .spawn((Name::new("crate"), SceneEntityId(1), Transform::default()))
        .id();
    (world, e)
}

/// What a frame does between scans: move the change tick on, so edits made
/// now read as newer than the last scan.
fn next_frame(world: &mut World) {
    world.increment_change_tick();
}

fn by_id(world: &mut World, id: u64) -> Option<Entity> {
    let mut q = world.query::<(Entity, &SceneEntityId)>();
    q.iter(world).find(|(_, s)| s.0 == id).map(|(e, _)| e)
}

#[test]
fn an_edit_on_one_side_lands_on_the_other_and_is_not_echoed_back() {
    let (mut a, crate_a) = scene();
    let (mut b, crate_b) = scene();
    let mut ta = Tracker::baseline(&mut a);
    let mut tb = Tracker::baseline(&mut b);

    next_frame(&mut a);
    a.get_mut::<Transform>(crate_a).unwrap().translation = Vec3::new(4.0, 5.0, 6.0);
    let sent = ta.scan(&mut a);
    assert!(
        sent.iter().any(|m| matches!(m, LinkMessage::Set { id: 1, ty, .. } if ty == Transform::type_path())),
        "{sent:?}"
    );

    next_frame(&mut b);
    apply(&mut b, sent, &mut tb, &mut Quiet, &mut HashSet::new());
    assert_eq!(b.get::<Transform>(crate_b).unwrap().translation, Vec3::new(4.0, 5.0, 6.0));

    next_frame(&mut b);
    let echo = tb.scan(&mut b);
    assert!(echo.is_empty(), "an applied edit came straight back: {echo:?}");
}

#[test]
fn nothing_is_sent_when_nothing_changed() {
    let (mut a, crate_a) = scene();
    let mut ta = Tracker::baseline(&mut a);
    next_frame(&mut a);
    // Written but not changed: change detection fires, the comparison does not.
    a.get_mut::<Transform>(crate_a).unwrap().set_changed();
    assert!(ta.scan(&mut a).is_empty());
}

#[test]
fn a_new_child_is_spawned_under_its_parent_without_sending_the_parent_link() {
    let (mut a, crate_a) = scene();
    let (mut b, crate_b) = scene();
    let mut ta = Tracker::baseline(&mut a);
    let mut tb = Tracker::baseline(&mut b);

    next_frame(&mut a);
    a.spawn((Name::new("lid"), Transform::from_xyz(0.0, 1.0, 0.0), ChildOf(crate_a)));
    let sent = ta.scan(&mut a);

    assert!(sent.iter().any(|m| matches!(m, LinkMessage::Spawn { parent: Some(1), name, .. } if name == "lid")));
    assert!(
        !sent.iter().any(|m| matches!(m, LinkMessage::Set { ty, .. } if ty == ChildOf::type_path())),
        "an entity reference crossed between worlds: {sent:?}"
    );

    next_frame(&mut b);
    apply(&mut b, sent, &mut tb, &mut Quiet, &mut HashSet::new());
    let mut lids = b.query::<(&Name, &ChildOf, &Transform)>();
    let (_, parent, transform) = lids.iter(&b).find(|(n, ..)| n.as_str() == "lid").unwrap();
    assert_eq!(parent.parent(), crate_b);
    assert_eq!(transform.translation, Vec3::Y);
}

#[test]
fn a_despawn_on_one_side_despawns_on_the_other() {
    let (mut a, crate_a) = scene();
    let (mut b, _) = scene();
    let mut ta = Tracker::baseline(&mut a);
    let mut tb = Tracker::baseline(&mut b);

    next_frame(&mut a);
    a.despawn(crate_a);
    let sent = ta.scan(&mut a);
    assert_eq!(sent, vec![LinkMessage::Despawn { id: 1 }]);

    apply(&mut b, sent, &mut tb, &mut Quiet, &mut HashSet::new());
    assert!(by_id(&mut b, 1).is_none());
}

/// The game has a global scene's entity (a loading screen) that the editor
/// never has. It must not travel: once reported, the editor's copy came back
/// to the game as a despawn and the loading screen vanished.
/// The game owns its UI: a UI root, a widget in it and a text span under the
/// widget (which has no `Node` of its own) are never sent, while an ordinary
/// object spawned beside them is.
#[test]
fn ui_is_never_sent() {
    let (mut game, _) = scene();
    let mut tracker = Tracker::baseline(&mut game);

    let root = game.spawn((Name::new("hud"), Node::default())).id();
    let widget = game.spawn((Name::new("button"), Node::default(), ChildOf(root))).id();
    game.spawn((Name::new("label_span"), Transform::default(), ChildOf(widget)));
    game.spawn((Name::new("bullet"), Transform::default()));

    let mut sent = Vec::new();
    for _ in 0..12 {
        next_frame(&mut game);
        sent.extend(tracker.scan(&mut game));
    }
    let spawned: Vec<&str> = sent
        .iter()
        .filter_map(|m| match m {
            LinkMessage::Spawn { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(spawned, ["bullet"], "sent: {sent:?}");
}

#[test]
fn a_global_scene_entity_is_not_sent() {
    let (mut game, _) = scene();
    game.spawn((
        Name::new("loading_screen"),
        SceneEntityId(2),
        Transform::default(),
        renzora_scene_api::Persistent,
    ));
    let mut tracker = Tracker::baseline(&mut game);

    for _ in 0..12 {
        next_frame(&mut game);
        let sent = tracker.scan(&mut game);
        assert!(
            !sent.iter().any(|m| matches!(
                m,
                LinkMessage::Spawn { id: 2, .. } | LinkMessage::Set { id: 2, .. } | LinkMessage::Despawn { id: 2 }
            )),
            "the global scene's entity was sent: {sent:?}"
        );
    }
}

#[test]
fn a_removed_component_is_removed_on_the_other_side() {
    let (mut a, crate_a) = scene();
    let (mut b, crate_b) = scene();
    a.entity_mut(crate_a).insert(Visibility::Hidden);
    b.entity_mut(crate_b).insert(Visibility::Hidden);
    let mut ta = Tracker::baseline(&mut a);
    let mut tb = Tracker::baseline(&mut b);

    next_frame(&mut a);
    a.entity_mut(crate_a).remove::<Visibility>();
    let sent = ta.scan(&mut a);
    assert!(sent.iter().any(|m| matches!(m, LinkMessage::Remove { id: 1, ty } if ty == Visibility::type_path())));

    apply(&mut b, sent, &mut tb, &mut Quiet, &mut HashSet::new());
    assert!(b.get::<Visibility>(crate_b).is_none());
}

#[test]
fn entity_references_are_found_wherever_they_sit() {
    let mut w = world();
    let e = w.spawn_empty().id();
    assert!(holds_entity(&ChildOf(e)));
    assert!(holds_entity(&Some(e)));
    assert!(holds_entity(&vec![(1u32, e)]));
    assert!(!holds_entity(&Transform::default()));
    assert!(!holds_entity(&Name::new("x")));
}

#[test]
fn unknown_types_are_reported_once_and_skipped() {
    let (mut b, crate_b) = scene();
    let mut tb = Tracker::baseline(&mut b);
    let mut unknown = HashSet::new();
    let bogus = || LinkMessage::Set { id: 1, ty: "not::a::Type".into(), value: "()".into() };
    apply(&mut b, vec![bogus(), bogus()], &mut tb, &mut Quiet, &mut unknown);
    assert_eq!(unknown.len(), 1);
    assert_eq!(*b.get::<Transform>(crate_b).unwrap(), Transform::default());
}

#[test]
fn messages_are_one_json_line_each() {
    let message = LinkMessage::Set { id: 5, ty: "a::B".into(), value: "(x: 1)\n".into() };
    let line = serde_json::to_string(&message).unwrap();
    assert!(!line.contains('\n'));
    assert_eq!(serde_json::from_str::<LinkMessage>(&line).unwrap(), message);
}
