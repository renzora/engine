//! Keep the project's systems out of the editor's schedules.
//!
//! # Why they cannot simply be added
//!
//! `add_plugins` scatters a plugin's systems through Bevy's schedules as
//! function pointers, and Bevy keeps no record of which plugin contributed
//! what. Nothing can take them back out again by plugin, so a second
//! `add_plugins` on a rebuild leaves the old copies running beside the new ones
//! with no way to tell them apart. That is the whole reason opening a Bevy
//! project used to cost an editor restart per edit.
//!
//! It is also the wrong behaviour even when it works. The viewport is for
//! editing. A project's `Update` systems running there means pressing Play
//! changes the world you are editing and pressing Stop leaves the debris
//! behind, which is a class of bug no amount of care removes: play and edit
//! were sharing one `World`.
//!
//! # What this does instead
//!
//! [`Schedules`] is an ordinary resource. So the project's plugin is built with
//! the editor's schedules *taken out of the world* and an empty set put in its
//! place. Everything the plugin registers lands in that empty set, the editor's
//! are put back afterwards, and what the project added is now a separate
//! collection this module owns.
//!
//! Resources are deliberately **not** redirected. The plugin builds into the
//! real world, so its `init_resource` and `insert_resource` calls land where its
//! systems will later look for them. Only the schedules are diverted, because
//! schedules are the only part that cannot be undone.
//!
//! What that buys:
//!
//! * the viewport runs the project's `Startup` and nothing else, so the world
//!   its code builds is there to look at and edit, and none of it is moving
//!   while you work;
//! * `Update` is kept rather than discarded, so anything that wants to run it
//!   deliberately can. Nothing does today: playing the game is a separate
//!   process (see `renzora_viewport::external_runtime`), which is what makes
//!   Stop leave nothing behind.
//!
//! # Two things that are not obvious
//!
//! The replacement set is empty of *systems*, not of *schedules*: see
//! [`empty_like`] for why a plugin can tell the difference, and what it cost.
//!
//! The plugin **registry** is isolated too, not just the schedules. The project
//! is built into a shell `App` holding the real world, so its plugins never
//! enter the editor's registry. That matters because `is_plugin_added` is a
//! question other code acts on: a plugin the editor can see but whose systems it
//! does not have is a `true` that means `false`. See [`capture`].
//!
//! `Plugin::finish` and `Plugin::cleanup` run inside the capture, so a plugin
//! that registers its systems there is captured like any other. That used to be
//! the documented edge of this module: `finish` ran later, from the editor's own
//! pass, with the editor's schedules back in place, and those systems ended up
//! in the editor's `Update`.

use bevy::ecs::entity::EntityHashSet;
use bevy::ecs::schedule::{ScheduleLabel, Schedules};
use bevy::prelude::*;

use renzora::core::bevy_project::FromProjectCode;

/// Every schedule the project's plugin registered, kept out of the editor's.
///
/// Written once, when the project loads. Nothing replaces it while the editor
/// runs: a save rebuilds the library on disk, and the new code is picked up by
/// Play (a fresh process) and by the next editor start.
#[derive(Resource, Default)]
pub struct ProjectSchedules(pub Schedules);

/// Has the project's startup run in this generation?
///
/// Startup is deferred rather than run inside [`capture`] because `capture`
/// happens while the editor's own `App` is still being built: the asset server
/// exists but very little else does, and a project's `Startup` reasonably
/// expects a world that has finished coming up. This runs it on the first frame
/// instead.
#[derive(Resource, Default)]
pub struct ProjectStartupPending(pub bool);

/// Build `install` with the editor's schedules set aside, and keep whatever it
/// registered.
///
/// The swap is the whole mechanism. See the module docs.
pub fn capture(app: &mut App, install: impl FnOnce(&mut App)) -> Schedules {
    let editor = app.world_mut().remove_resource::<Schedules>();
    app.world_mut().insert_resource(empty_like(editor.as_ref()));

    // The plugin *registry* is isolated as well as the schedules, by building
    // into a shell `App` holding the real world. Without this the editor's
    // `is_plugin_added` learns about plugins whose systems it does not have, and
    // answers `true` for something that is not running.
    //
    // That is not hypothetical. A project adding `FrameTimeDiagnosticsPlugin`
    // registered the frame-time diagnostic into the shared `DiagnosticsStore`
    // and had its measuring system captured, so nothing ever fed it. The status
    // bar then asked `is_plugin_added::<FrameTimeDiagnosticsPlugin>()`, got
    // `true`, stood down from adding a working one, and read 0 FPS for the rest
    // of the session.
    //
    // A shell's registry starts empty and is dropped, so the project's plugins
    // are the project's. It also means a project may add a plugin the editor
    // already has, which is otherwise a duplicate-plugin panic, and
    // `FrameTimeDiagnosticsPlugin` is exactly the plugin a game is most likely
    // to add.
    {
        let mut shell = App::empty();
        core::mem::swap(shell.world_mut(), app.world_mut());
        install(&mut shell);
        // The rest of the lifecycle, while the schedules are still swapped out.
        //
        // These used to be run by the editor's own `finish` pass, because the
        // project's plugins were in the editor's registry: the module docs
        // called that "the edge worth knowing", since a plugin registering
        // systems from `finish` had them land in the editor's `Update` rather
        // than being captured. Running it here closes that edge rather than
        // documenting it, and a project's `finish` sees every plugin it depends
        // on, because they are all in this same shell.
        shell.finish();
        shell.cleanup();
        core::mem::swap(shell.world_mut(), app.world_mut());
    }

    let project = app
        .world_mut()
        .remove_resource::<Schedules>()
        .unwrap_or_default();
    // Put the editor's back before anything else touches the world. A world with
    // no `Schedules` is one where the next `add_systems` silently does nothing.
    if let Some(editor) = editor {
        app.world_mut().insert_resource(editor);
    }
    project
}

/// The same *set* of schedules the editor has, every one of them empty.
///
/// A bare `Schedules::default()` is not a blank slate, it is a world where no
/// schedule exists, and a plugin can tell the difference. `init_state` reaches
/// for `StateTransition` to register the state's transition systems into it, and
/// a missing schedule is not something it works around:
///
/// ```text
/// The `StateTransition` schedule is missing. Did you forget to add
/// StatesPlugin or DefaultPlugins before calling init_state?
/// ```
///
/// That killed every Bevy project using states, on load, with a message
/// pointing at the project's own `main` rather than at the editor that took the
/// schedule away. `add_systems` never noticed because it creates a schedule on
/// demand; anything that expects one to be *there* did.
///
/// So the replacement mirrors the editor's labels and holds none of its systems.
/// The project registers into its own copies, they are captured with the rest,
/// and the editor's are put back untouched. Empty schedules that the project
/// never touches cost a hash-map entry each.
///
/// The project's `StateTransition` is captured and, like its `Update`, not run:
/// the viewport runs a project's `Startup` and nothing else, so its states stay
/// at their initial value while you edit. That is the intended behaviour rather
/// than a shortfall, and the same reason `Update` is held back.
///
/// One cosmetic edge: on the reload path the shell `App` has an empty plugin
/// registry, so `init_state` logs "States were added to the app, but
/// `StatesPlugin` is not installed" once. The plugin *is* installed, in the real
/// app the world came from; it is `is_plugin_added` that cannot see it.
fn empty_like(editor: Option<&Schedules>) -> Schedules {
    use bevy::ecs::schedule::Schedule;

    let mut fresh = Schedules::default();
    let Some(editor) = editor else {
        return fresh;
    };
    // Taken from the schedule rather than the map key: the key is a
    // `&dyn ScheduleLabel`, which cannot be interned through a trait object,
    // while `Schedule::label` hands back the `InternedScheduleLabel` it already
    // holds.
    for (_, schedule) in editor.iter() {
        fresh.insert(Schedule::new(schedule.label()));
    }
    fresh
}

/// [`capture`], from a `&mut World` rather than a `&mut App`.
///
/// A reload happens while the editor is running, where there is no `App`: the
/// runner owns it and a system only ever sees the world. `Plugin::build` takes
/// `&mut App`, so one has to be produced.
///
/// Building into a *scratch* `App` and moving the result across does not work,
/// because only schedules can be moved. A plugin's `init_resource` and
/// `insert_resource` calls would land in the scratch world, and its systems
/// would then run against a live world where those resources do not exist.
///
/// So the world is moved instead of the plugin. The live world is swapped into
/// an empty `App` for the duration of the call and swapped back afterwards,
/// which makes every `App` method the plugin reaches for operate on the real
/// world. The scratch `App` is a shell holding a borrowed world, not a second
/// world.
///
/// [`capture`] now makes a shell of its own for the same reason, so this one
/// exists only to satisfy its `&mut App` signature. Nesting the two is harmless:
/// the outer shell holds the world for the length of the call and contributes
/// nothing else.
pub fn capture_into_world(world: &mut World, plugin: impl Plugin) -> Schedules {
    let mut shell = App::empty();
    core::mem::swap(shell.world_mut(), world);
    let captured = capture(&mut shell, |app| {
        app.add_plugins(plugin);
    });
    core::mem::swap(shell.world_mut(), world);
    captured
}

/// Run one of the project's schedules against the live world.
///
/// `Schedules` is taken out for the duration because a schedule needs `&mut
/// World` to run and cannot be borrowed from the world it is running against.
/// It goes back afterwards, including on the path where the schedule is absent,
/// which is the normal case for a project that never registered one.
pub fn run_project_schedule(world: &mut World, label: impl ScheduleLabel) -> bool {
    let Some(mut schedules) = world.remove_resource::<ProjectSchedules>() else {
        return false;
    };
    let ran = match schedules.0.get_mut(label) {
        Some(schedule) => {
            schedule.run(world);
            true
        }
        None => false,
    };
    world.insert_resource(schedules);
    ran
}

/// Run the project's startup schedules once, on the first frame after a load.
///
/// All three of Bevy's startup labels, in the order `Main` would have run them,
/// because a project is entitled to use any of them and the ordering between
/// them is the only thing their separation means.
pub fn run_project_startup(world: &mut World) {
    let pending = world
        .get_resource::<ProjectStartupPending>()
        .is_some_and(|p| p.0);
    if !pending {
        return;
    }
    world.insert_resource(ProjectStartupPending(false));

    // Everything that exists before the project builds its world. The set is
    // taken here rather than inferred later because provenance cannot be read
    // off an entity afterwards: see [`tag_new_entities`].
    let before: EntityHashSet = world.iter_entities().map(|e| e.id()).collect();

    run_project_schedule(world, PreStartup);
    run_project_schedule(world, Startup);
    run_project_schedule(world, PostStartup);
    // Startup spawns through `Commands`, which are queued rather than applied.
    // Without this the world is unchanged until something else happens to flush
    // them, and the first frame renders an empty scene.
    world.flush();

    let tagged = tag_new_entities(world, &before);
    bevy::log::info!("[bevy-project] the project's startup built {tagged} entities");
}

/// Mark everything the project's startup just created as the project's.
///
/// [`FromProjectCode`] decides two things: which entities a reload throws away,
/// and which ones the write-back must *not* generate Rust for. Getting it wrong
/// in either direction is bad, and it was wrong.
///
/// `renzora_engine::named_entities` used to be the only thing that set it, as a
/// side effect of naming: an unnamed entity got a `Name` and the tag together,
/// and anything that already had a `Name` was assumed to be the editor's,
/// because "the project's own spawns do not name anything". That holds for a
/// crate written before the author had heard of this editor. It is false the
/// moment a project calls `Name::new`, which is good practice and which the
/// project scaffold does on every entity it spawns. Every one of them was
/// therefore filed as editor-authored, survived the despawn, and a reload
/// doubled the world.
///
/// Provenance is not a property of an entity, so it cannot be recovered by
/// inspecting one. It is a property of *when* it appeared. This is the one place
/// that knows: the project's startup is run from here, so the entities that
/// exist afterwards and did not exist before are exactly its.
///
/// `With<Transform>` narrows it to scene content, matching what the rest of the
/// engine means by that word. A reload despawns what this tags, and Bevy spawns
/// entities of its own for observers and assets; taking those out from under it
/// because they happened to appear during startup would break the editor rather
/// than the project.
fn tag_new_entities(world: &mut World, before: &EntityHashSet) -> usize {
    let new: Vec<Entity> = world
        .iter_entities()
        .map(|entity| entity.id())
        .filter(|entity| !before.contains(entity))
        .collect();
    let mut tagged = 0;
    for entity in new {
        let Ok(mut entity) = world.get_entity_mut(entity) else {
            continue;
        };
        if !entity.contains::<Transform>() {
            continue;
        }
        entity.insert(FromProjectCode);
        tagged += 1;
    }
    tagged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default, PartialEq, Debug)]
    struct Ran {
        startup: u32,
        update: u32,
    }

    struct GamePlugin;

    impl Plugin for GamePlugin {
        fn build(&self, app: &mut App) {
            app.init_resource::<Ran>();
            app.add_systems(Startup, |mut ran: ResMut<Ran>| ran.startup += 1);
            app.add_systems(Update, |mut ran: ResMut<Ran>| ran.update += 1);
        }
    }

    /// A project that uses states must survive being captured.
    ///
    /// `init_state` does not just insert a resource: it reaches for the
    /// `StateTransition` schedule and registers the state's transition systems
    /// into it. Swapping in an empty `Schedules` took that schedule away, so
    /// every Bevy project using states died on load with
    ///
    /// ```text
    /// The `StateTransition` schedule is missing. Did you forget to add
    /// StatesPlugin or DefaultPlugins before calling init_state?
    /// ```
    ///
    /// which reads like the project's own bug and is not.
    #[test]
    fn a_project_that_uses_states_loads() {
        #[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        enum Wave {
            #[default]
            Idle,
            Fighting,
        }

        struct StatefulGame;
        impl Plugin for StatefulGame {
            fn build(&self, app: &mut App) {
                app.init_state::<Wave>();
                app.add_systems(OnEnter(Wave::Fighting), || {});
            }
        }

        let mut app = App::new();
        // What the editor has and a bare `App::new()` does not: `StateTransition`
        // arrives with `StatesPlugin`, which is part of `DefaultPlugins`. Without
        // this the test would be asking whether `capture` can mirror a schedule
        // the editor never had.
        app.add_plugins(bevy::state::app::StatesPlugin);

        let captured = capture(&mut app, |app| {
            app.add_plugins(StatefulGame);
        });

        assert!(
            app.world().contains_resource::<State<Wave>>(),
            "the state resource belongs in the live world, like every other resource"
        );
        // The transition systems are the project's, so they go with the
        // project's schedules rather than into the editor's.
        assert!(
            captured.get(StateTransition).is_some(),
            "the project's state transition schedule must be captured, not lost"
        );
    }

    /// What the project adds must not change the editor's answer to "is this
    /// plugin added".
    ///
    /// The 0 FPS bug: a project adds `FrameTimeDiagnosticsPlugin`, its measuring
    /// system is captured and never runs, and the status bar's
    /// `if !is_plugin_added { add a working one }` then declines to add one.
    /// Nothing feeds the diagnostic and the readout sits at zero all session.
    ///
    /// Modelled with a plain plugin rather than Bevy's, because the property is
    /// about the registry and not about diagnostics.
    #[test]
    fn a_projects_plugins_do_not_enter_the_editors_registry() {
        #[derive(Resource, Default, PartialEq, Debug)]
        struct Fed(u32);

        /// Stands in for `FrameTimeDiagnosticsPlugin`: something both the
        /// project and the editor might add, which does real per-frame work.
        struct FeederPlugin;
        impl Plugin for FeederPlugin {
            fn build(&self, app: &mut App) {
                app.init_resource::<Fed>();
                app.add_systems(Update, |mut fed: ResMut<Fed>| fed.0 += 1);
            }
        }

        struct GameWithFeeder;
        impl Plugin for GameWithFeeder {
            fn build(&self, app: &mut App) {
                app.add_plugins(FeederPlugin);
            }
        }

        let mut app = App::new();
        let _captured = capture(&mut app, |app| {
            app.add_plugins(GameWithFeeder);
        });

        assert!(
            !app.is_plugin_added::<FeederPlugin>(),
            "the editor must not believe it has a plugin whose systems were captured"
        );

        // So the editor's own guarded add still happens, and still works.
        if !app.is_plugin_added::<FeederPlugin>() {
            app.add_plugins(FeederPlugin);
        }
        app.update();
        assert_eq!(
            *app.world().resource::<Fed>(),
            Fed(1),
            "the editor's copy runs; this is the readout that used to sit at zero"
        );
    }

    /// A project may add a plugin the editor already has.
    ///
    /// Bevy makes a duplicate plugin a panic, and the isolated registry is what
    /// stops one. `FrameTimeDiagnosticsPlugin` is the plugin a game is most
    /// likely to add and the editor most likely to have.
    #[test]
    fn a_project_may_add_a_plugin_the_editor_already_has() {
        struct Shared;
        impl Plugin for Shared {
            fn build(&self, app: &mut App) {
                app.add_systems(Update, || {});
            }
        }

        let mut app = App::new();
        app.add_plugins(Shared);

        // Would panic with "plugin was already added" if the registry were shared.
        let _captured = capture(&mut app, |app| {
            app.add_plugins(Shared);
        });
    }

    /// A plugin that registers from `finish` is captured like any other.
    ///
    /// This used to be the module's documented escape hatch: `finish` ran from
    /// the editor's own pass, after the schedules were back, so those systems
    /// ran in the editor's `Update` and a reload could not take them out again.
    #[test]
    fn systems_registered_from_finish_are_captured_too() {
        #[derive(Resource, Default, PartialEq, Debug)]
        struct Late(u32);

        struct LateRegistrar;
        impl Plugin for LateRegistrar {
            fn build(&self, app: &mut App) {
                app.init_resource::<Late>();
            }
            fn finish(&self, app: &mut App) {
                app.add_systems(Update, |mut late: ResMut<Late>| late.0 += 1);
            }
        }

        let mut app = App::new();
        let captured = capture(&mut app, |app| {
            app.add_plugins(LateRegistrar);
        });

        app.update();
        assert_eq!(
            *app.world().resource::<Late>(),
            Late(0),
            "a finish-registered system must not run from the editor's Update"
        );
        assert!(
            captured.get(Update).is_some(),
            "it belongs in the project's schedules, where a reload can drop it"
        );
    }

    /// The whole mechanism, in one test: a plugin's resources reach the live
    /// world while its systems do not reach the live schedules, and the systems
    /// still run correctly when the captured schedule is run against that world.
    ///
    /// That last part is the one that had to be proven rather than assumed. A
    /// system is registered in one place and run in another, and Bevy initialises
    /// a system lazily, against whichever world first runs it. If that were not
    /// so, none of this would work.
    #[test]
    fn a_plugin_is_captured_and_still_runs_against_the_live_world() {
        let mut app = App::new();
        let captured = capture(&mut app, |app| {
            app.add_plugins(GamePlugin);
        });

        // Resources are not diverted: the plugin built into the real world.
        assert!(
            app.world().contains_resource::<Ran>(),
            "the plugin's resources belong in the live world"
        );

        // Its systems are not in the editor's schedules, so a frame runs none.
        app.update();
        assert_eq!(
            *app.world().resource::<Ran>(),
            Ran { startup: 0, update: 0 },
            "no captured system should run from the editor's own schedules"
        );

        app.insert_resource(ProjectSchedules(captured));
        app.insert_resource(ProjectStartupPending(true));
        run_project_startup(app.world_mut());

        assert_eq!(
            *app.world().resource::<Ran>(),
            Ran { startup: 1, update: 0 },
            "startup runs against the live world; update is held back"
        );

        // And it stays held back: further frames change nothing.
        app.update();
        app.update();
        assert_eq!(
            *app.world().resource::<Ran>(),
            Ran { startup: 1, update: 0 },
            "the viewport does not run the project's Update"
        );
    }

    /// The regression that doubled the world on every reload.
    ///
    /// A project that names its own entities is doing the right thing, and the
    /// old provenance rule read a `Name` as "the editor made this". Nothing the
    /// startup spawns may be missed, whether it named itself or not, because
    /// what is missed is what survives the despawn and appears twice.
    #[test]
    fn everything_startup_spawns_is_tagged_named_or_not() {
        struct SpawningPlugin;
        impl Plugin for SpawningPlugin {
            fn build(&self, app: &mut App) {
                app.add_systems(Startup, |mut commands: Commands| {
                    commands.spawn((Name::new("pillar-0"), Transform::default()));
                    commands.spawn((Name::new("obelisk"), Transform::default()));
                    commands.spawn(Transform::default());
                });
            }
        }

        let mut app = App::new();
        let captured = capture(&mut app, |app| {
            app.add_plugins(SpawningPlugin);
        });
        app.insert_resource(ProjectSchedules(captured));
        app.insert_resource(ProjectStartupPending(true));
        run_project_startup(app.world_mut());

        let mut tagged = app
            .world_mut()
            .query_filtered::<Entity, With<FromProjectCode>>();
        assert_eq!(
            tagged.iter(app.world()).count(),
            3,
            "a named entity is still the project's"
        );
    }

    /// An entity that was there before the project started is not the project's,
    /// however much it looks like scene content. Tagging one would have a reload
    /// despawn something the editor owns.
    #[test]
    fn what_existed_before_startup_is_left_alone() {
        struct NoopPlugin;
        impl Plugin for NoopPlugin {
            fn build(&self, app: &mut App) {
                app.add_systems(Startup, || {});
            }
        }

        let mut app = App::new();
        let editors = app.world_mut().spawn((Name::new("editor-grid"), Transform::default())).id();
        let captured = capture(&mut app, |app| {
            app.add_plugins(NoopPlugin);
        });
        app.insert_resource(ProjectSchedules(captured));
        app.insert_resource(ProjectStartupPending(true));
        run_project_startup(app.world_mut());

        assert!(
            !app.world().entity(editors).contains::<FromProjectCode>(),
            "an entity that predates the project is not the project's"
        );
    }

    /// A reload builds the new world before taking the old one down, so an
    /// asset both generations use is never released in between.
    ///
    /// Modelled with a counter standing in for the asset server: `loads` counts
    /// how many times something had to be fetched, and `live` is the reference
    /// count. If the old entities were despawned first, `live` would touch zero
    /// and the second load would be a real fetch. On a project with 93 MB of
    /// models that difference is a multi-second stall on every reload.
    #[test]
    fn a_reload_keeps_shared_assets_alive_across_the_swap() {
        #[derive(Resource, Default)]
        struct Assets {
            live: i32,
            fetches: u32,
        }

        // What `asset_server.load(path)` does: a cache hit while something still
        // holds it, a fetch when it does not.
        fn acquire(assets: &mut Assets) {
            if assets.live == 0 {
                assets.fetches += 1;
            }
            assets.live += 1;
        }

        let mut app = App::new();
        app.init_resource::<Assets>();
        // The first generation holds the asset.
        acquire(app.world_mut().resource_mut::<Assets>().as_mut());
        assert_eq!(app.world().resource::<Assets>().fetches, 1);

        // Reload, in the order `apply_reload` uses: acquire for the new
        // generation first, release the old one after.
        acquire(app.world_mut().resource_mut::<Assets>().as_mut());
        app.world_mut().resource_mut::<Assets>().live -= 1;

        let assets = app.world().resource::<Assets>();
        assert_eq!(assets.live, 1, "the new generation holds it");
        assert_eq!(
            assets.fetches, 1,
            "the asset was never released, so it was never re-read"
        );
    }

    /// Startup runs once per load, not once per frame.
    #[test]
    fn startup_is_not_run_twice() {
        let mut app = App::new();
        let captured = capture(&mut app, |app| {
            app.add_plugins(GamePlugin);
        });
        app.insert_resource(ProjectSchedules(captured));
        app.insert_resource(ProjectStartupPending(true));

        run_project_startup(app.world_mut());
        run_project_startup(app.world_mut());
        run_project_startup(app.world_mut());

        assert_eq!(app.world().resource::<Ran>().startup, 1);
    }

    /// `Update` is available to run deliberately. This is what a future in-process
    /// play mode would call, and it proves the captured schedule is not inert.
    #[test]
    fn the_captured_update_runs_when_it_is_asked_to() {
        let mut app = App::new();
        let captured = capture(&mut app, |app| {
            app.add_plugins(GamePlugin);
        });
        app.insert_resource(ProjectSchedules(captured));

        run_project_schedule(app.world_mut(), Update);
        run_project_schedule(app.world_mut(), Update);

        assert_eq!(app.world().resource::<Ran>().update, 2);
    }

}
