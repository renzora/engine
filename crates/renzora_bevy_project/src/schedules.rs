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
//! # The edge worth knowing
//!
//! A plugin that registers systems from [`Plugin::finish`] rather than
//! [`Plugin::build`] escapes this, because `finish` runs later, after the
//! editor's schedules are back. That is rare (it exists for plugins that need
//! another plugin's resources) and the failure is the old behaviour rather than
//! a crash: those systems run in the editor's `Update`.

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
    app.world_mut().insert_resource(Schedules::default());

    install(app);

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
