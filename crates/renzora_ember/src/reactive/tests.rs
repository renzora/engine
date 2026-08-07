//! Driver-level tests for the dependency gate.
//!
//! [`super::rx`] unit-tests the tick comparisons in isolation; these go through
//! the real registry and the real [`super::run_reactions`], because the two
//! things most likely to break are integration-shaped: a reaction being skipped
//! when it should not be (stale UI), and a legacy binding being skipped at all
//! (the same, workspace-wide).

use super::*;

#[derive(Resource, Default)]
struct Counter(u32);

#[derive(Resource, Default)]
struct Unrelated(u32);

fn test_world() -> World {
    let mut world = World::new();
    world.init_resource::<ReactionRegistry>();
    world.init_resource::<ReactiveStats>();
    world.insert_resource(Counter(0));
    world.insert_resource(Unrelated(0));
    world
}

/// Register a binding through the same deferred path real callers use.
fn register(world: &mut World, f: impl FnOnce(&mut Commands)) {
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, world);
    f(&mut commands);
    queue.apply(world);
}

fn text_of(world: &World, e: Entity) -> String {
    world.get::<Text>(e).map(|t| t.0.clone()).unwrap_or_default()
}

fn skipped(world: &World) -> usize {
    world.resource::<ReactiveStats>().skipped_this_frame
}

fn active(world: &World) -> usize {
    world.resource::<ReactiveStats>().bindings_total
}

fn parked(world: &World) -> usize {
    world.resource::<ReactiveStats>().parked_total
}

/// A parent node whose `Display` we can flip, with a bound child under it.
fn collapsible(world: &mut World) -> (Entity, Entity) {
    let parent = world.spawn(Node::default()).id();
    let child = world.spawn(Text::new(String::new())).id();
    world.entity_mut(parent).add_child(child);
    register(world, |c| {
        tracked::bind_text(c, child, |rx| rx.resource::<Counter>().0.to_string());
    });
    (parent, child)
}

fn set_display(world: &mut World, e: Entity, d: Display) {
    world.entity_mut(e).get_mut::<Node>().unwrap().display = d;
}

/// Collapsing an ancestor moves the bindings underneath it out of the walked
/// list entirely, and reopening brings them back — still working.
#[test]
fn collapsing_parks_bindings_and_reopening_restores_them() {
    let mut world = test_world();
    let (parent, child) = collapsible(&mut world);

    run_reactions(&mut world);
    assert_eq!(active(&world), 1);
    assert_eq!(parked(&world), 0);
    assert_eq!(text_of(&world, child), "0");

    // Collapse the parent.
    set_display(&mut world, parent, Display::None);
    run_reactions(&mut world);
    assert_eq!(active(&world), 0, "a hidden binding is still being walked");
    assert_eq!(parked(&world), 1);

    // While collapsed, state moves — the binding must not run.
    world.increment_change_tick();
    world.resource_mut::<Counter>().0 = 5;
    run_reactions(&mut world);
    assert_eq!(active(&world), 0);
    assert_eq!(text_of(&world, child), "0", "a parked binding ran anyway");

    // Reopen: it comes back AND catches up. This is the half that makes
    // parking correct where dropping would not be — nothing rebuilds a
    // collapsed subtree, so a dropped closure would never return.
    set_display(&mut world, parent, Display::Flex);
    run_reactions(&mut world);
    assert_eq!(active(&world), 1, "reopening did not restore the binding");
    assert_eq!(parked(&world), 0);
    assert_eq!(
        text_of(&world, child),
        "5",
        "the restored binding did not catch up on what it missed"
    );
}

/// Parking must not leak when the subtree is despawned while collapsed — the
/// entries are filed under an anchor that no longer exists.
#[test]
fn despawning_a_collapsed_subtree_drops_its_parked_bindings() {
    let mut world = test_world();
    let (parent, _child) = collapsible(&mut world);

    run_reactions(&mut world);
    set_display(&mut world, parent, Display::None);
    run_reactions(&mut world);
    assert_eq!(parked(&world), 1);

    // `sync_panes` despawns a whole pane, collapsed sections and all.
    world.entity_mut(parent).despawn();
    run_reactions(&mut world);
    assert_eq!(active(&world), 0);
    assert_eq!(
        parked(&world),
        0,
        "parked bindings survived their subtree being despawned"
    );
}

/// A binding that hides its OWN target must keep running — it is the only
/// thing that can un-hide it. Parking it would strand the node hidden forever.
#[test]
fn a_binding_that_collapses_its_own_target_is_not_parked() {
    #[derive(Resource)]
    struct Show(bool);

    let mut world = test_world();
    world.insert_resource(Show(false));
    let e = world.spawn(Node::default()).id();
    register(&mut world, |c| {
        tracked::bind_display(c, e, |rx| rx.resource::<Show>().0);
    });

    run_reactions(&mut world);
    assert_eq!(
        world.get::<Node>(e).unwrap().display,
        Display::None,
        "precondition: the binding hid its own target"
    );
    assert_eq!(active(&world), 1, "a self-hiding binding was parked");
    assert_eq!(parked(&world), 0);

    // It must still be able to bring the node back.
    world.increment_change_tick();
    world.resource_mut::<Show>().0 = true;
    run_reactions(&mut world);
    assert_eq!(
        world.get::<Node>(e).unwrap().display,
        Display::Flex,
        "a self-hiding binding could not un-hide its target"
    );
}

/// The headline claim: a tracked binding runs once, then stops running until
/// something it actually read changes — and still updates when it does.
#[test]
fn a_tracked_binding_skips_until_its_dependency_moves() {
    let mut world = test_world();
    let e = world.spawn(Text::new(String::new())).id();
    register(&mut world, |c| {
        tracked::bind_text(c, e, |rx| rx.resource::<Counter>().0.to_string());
    });

    // First pass seeds the dep set, so it must run.
    run_reactions(&mut world);
    assert_eq!(text_of(&world, e), "0");
    assert_eq!(skipped(&world), 0);

    // Nothing changed → skipped outright, closure never entered.
    run_reactions(&mut world);
    assert_eq!(
        skipped(&world),
        1,
        "a tracked binding re-ran with none of its dependencies touched"
    );

    // A write to something it never read must not wake it.
    world.increment_change_tick();
    world.resource_mut::<Unrelated>().0 += 1;
    run_reactions(&mut world);
    assert_eq!(
        skipped(&world),
        1,
        "an unrelated resource write woke a tracked binding"
    );

    // A write to what it did read must wake it, and the UI must follow.
    world.increment_change_tick();
    world.resource_mut::<Counter>().0 = 42;
    run_reactions(&mut world);
    assert_eq!(skipped(&world), 0);
    assert_eq!(text_of(&world, e), "42");
}

/// The migration-safety property, exercised through the real driver: a legacy
/// `Fn(&World)` binding records nothing, so it must keep running every frame
/// exactly as it does today.
#[test]
fn a_legacy_binding_never_gets_skipped() {
    let mut world = test_world();
    let e = world.spawn(Text::new(String::new())).id();
    register(&mut world, |c| {
        bind_text(c, e, |w: &World| w.resource::<Counter>().0.to_string());
    });

    for _ in 0..3 {
        run_reactions(&mut world);
        assert_eq!(
            skipped(&world),
            0,
            "an untracked binding was skipped — this is the staleness that the \
             empty-dep-set-is-dirty rule exists to prevent"
        );
    }

    world.increment_change_tick();
    world.resource_mut::<Counter>().0 = 7;
    run_reactions(&mut world);
    assert_eq!(text_of(&world, e), "7");
}

/// Tracked and legacy bindings in one registry must not interfere: the tracked
/// one skips and the legacy one does not, on the same frame.
#[test]
fn tracked_and_legacy_bindings_coexist() {
    let mut world = test_world();
    let a = world.spawn(Text::new(String::new())).id();
    let b = world.spawn(Text::new(String::new())).id();
    register(&mut world, |c| {
        tracked::bind_text(c, a, |rx| rx.resource::<Counter>().0.to_string());
        bind_text(c, b, |w: &World| w.resource::<Counter>().0.to_string());
    });

    run_reactions(&mut world);
    run_reactions(&mut world);
    assert_eq!(world.resource::<ReactiveStats>().bindings_total, 2);
    assert_eq!(
        skipped(&world),
        1,
        "expected exactly the tracked binding to be skipped"
    );
}

/// A binding reading a component is woken by a write to that entity's
/// component, and not by a write to a sibling's.
#[test]
fn component_reads_wake_only_their_own_entity() {
    #[derive(Component)]
    struct Label(u32);

    let mut world = test_world();
    let src = world.spawn(Label(1)).id();
    let other = world.spawn(Label(1)).id();
    let e = world.spawn(Text::new(String::new())).id();
    register(&mut world, |c| {
        tracked::bind_text(c, e, move |rx| {
            rx.get::<Label>(src).map(|l| l.0).unwrap_or(0).to_string()
        });
    });

    run_reactions(&mut world);
    assert_eq!(text_of(&world, e), "1");

    world.increment_change_tick();
    world.entity_mut(other).get_mut::<Label>().unwrap().0 = 5;
    run_reactions(&mut world);
    assert_eq!(skipped(&world), 1);

    world.increment_change_tick();
    world.entity_mut(src).get_mut::<Label>().unwrap().0 = 9;
    run_reactions(&mut world);
    assert_eq!(text_of(&world, e), "9");
}

/// A closure that branches onto different data must re-subscribe as it
/// branches, or it goes stale the first time the branch flips.
#[test]
fn a_branching_closure_resubscribes() {
    #[derive(Resource)]
    struct UseB(bool);
    #[derive(Resource)]
    struct A(u32);
    #[derive(Resource)]
    struct B(u32);

    let mut world = test_world();
    world.insert_resource(UseB(false));
    world.insert_resource(A(1));
    world.insert_resource(B(100));
    let e = world.spawn(Text::new(String::new())).id();
    register(&mut world, |c| {
        tracked::bind_text(c, e, |rx| {
            if rx.resource::<UseB>().0 {
                rx.resource::<B>().0.to_string()
            } else {
                rx.resource::<A>().0.to_string()
            }
        });
    });

    run_reactions(&mut world);
    assert_eq!(text_of(&world, e), "1");

    // While reading A, a write to B is irrelevant.
    world.increment_change_tick();
    world.resource_mut::<B>().0 = 200;
    run_reactions(&mut world);
    assert_eq!(skipped(&world), 1);

    // Flip the branch; now B is the live dependency and A is not.
    world.increment_change_tick();
    world.resource_mut::<UseB>().0 = true;
    run_reactions(&mut world);
    assert_eq!(text_of(&world, e), "200");

    world.increment_change_tick();
    world.resource_mut::<A>().0 = 50;
    run_reactions(&mut world);
    assert_eq!(
        skipped(&world),
        1,
        "the closure stayed subscribed to a branch it no longer reads"
    );

    world.increment_change_tick();
    world.resource_mut::<B>().0 = 300;
    run_reactions(&mut world);
    assert_eq!(text_of(&world, e), "300");
}

/// Two-way bindings must notice the user's edit, which arrives as a write to
/// `Bound<T>` rather than to the state the `get` closure reads. If `Bound` were
/// not itself a recorded dependency the gate would skip the reaction and drop
/// every keystroke.
#[test]
fn a_tracked_2way_binding_sees_the_widget_edit() {
    let mut world = test_world();
    let w = world.spawn_empty().id();
    register(&mut world, |c| {
        tracked::bind_2way(
            c,
            w,
            |rx| rx.resource::<Counter>().0,
            |world, v| world.resource_mut::<Counter>().0 = *v,
        );
    });

    run_reactions(&mut world);
    assert_eq!(world.get::<Bound<u32>>(w).map(|b| b.0), Some(0));

    // Simulate the widget's input system writing the model.
    world.increment_change_tick();
    world.entity_mut(w).get_mut::<Bound<u32>>().unwrap().0 = 12;
    run_reactions(&mut world);
    assert_eq!(
        world.resource::<Counter>().0,
        12,
        "the widget edit was skipped instead of written back to state"
    );

    // And the other direction still works.
    world.increment_change_tick();
    world.resource_mut::<Counter>().0 = 77;
    run_reactions(&mut world);
    assert_eq!(world.get::<Bound<u32>>(w).map(|b| b.0), Some(77));
}

/// A dead target drops its reaction. The gate must not stand between a
/// despawned target and the liveness check inside the reaction.
#[test]
fn a_despawned_target_drops_its_reaction() {
    let mut world = test_world();
    let e = world.spawn(Text::new(String::new())).id();
    register(&mut world, |c| {
        tracked::bind_text(c, e, |rx| rx.resource::<Counter>().0.to_string());
    });

    run_reactions(&mut world);
    assert_eq!(world.resource::<ReactiveStats>().bindings_total, 1);

    world.despawn(e);
    world.increment_change_tick();
    world.resource_mut::<Counter>().0 += 1;
    run_reactions(&mut world);
    assert_eq!(world.resource::<ReactiveStats>().bindings_total, 0);
}

/// The same, but with the dependency **unchanged** — which is the case that
/// actually leaked.
///
/// Only the reaction closure reports `Dead`, and the dep gate returns early
/// without calling it. So a binding on a despawned target whose dependencies
/// are clean was never re-examined and stayed in the registry forever. Since
/// `dock::sync_panes` despawns every inactive pane, that is one leaked binding
/// per widget per tab switch, for the life of the session.
#[test]
fn a_despawned_target_drops_even_when_its_dependencies_are_clean() {
    let mut world = test_world();
    let e = world.spawn(Text::new(String::new())).id();
    register(&mut world, |c| {
        tracked::bind_text(c, e, |rx| rx.resource::<Counter>().0.to_string());
    });

    // Seed the dep set, then confirm the gate is actually engaged.
    run_reactions(&mut world);
    run_reactions(&mut world);
    assert_eq!(skipped(&world), 1, "precondition: the binding is being gated");

    // Despawn with NOTHING else changing — no tick bump, no resource write.
    world.despawn(e);
    run_reactions(&mut world);
    assert_eq!(
        world.resource::<ReactiveStats>().bindings_total,
        0,
        "a gated binding outlived its despawned target — the registry leaks one \
         entry per widget per dock tab switch"
    );
}

/// Panels are rebuilt on activation, so the registry must return to its
/// starting size across an open/close cycle rather than ratcheting upward.
#[test]
fn repeated_panel_cycles_do_not_grow_the_registry() {
    let mut world = test_world();

    for _ in 0..5 {
        // "Open a panel": a handful of widgets, each with a binding.
        let widgets: Vec<Entity> = (0..4)
            .map(|_| world.spawn(Text::new(String::new())).id())
            .collect();
        register(&mut world, |c| {
            for &w in &widgets {
                tracked::bind_text(c, w, |rx| rx.resource::<Counter>().0.to_string());
            }
        });
        run_reactions(&mut world);
        assert_eq!(world.resource::<ReactiveStats>().bindings_total, 4);

        // "Switch tabs": `sync_panes` despawns the inactive pane's entities.
        for w in widgets {
            world.despawn(w);
        }
        run_reactions(&mut world);
        assert_eq!(
            world.resource::<ReactiveStats>().bindings_total,
            0,
            "bindings accumulated across a panel open/close cycle"
        );
    }
}
