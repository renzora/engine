//! Reaching `App` methods from a running editor, where there is no `App`.
//!
//! Bevy's `Plugin::build` takes `&mut App`. Once the editor is running, the
//! runner owns the `App` and every system only ever sees `&mut World`. That is
//! the whole reason installing a plugin used to cost a restart: not policy, just
//! that the seam fell where the `App` stops being reachable.
//!
//! # Why the world is moved rather than the plugin
//!
//! The obvious approach is a scratch `App`, build the plugin into it, and move
//! the result across. It does not work, because only *schedules* can be moved.
//! A plugin's `init_resource` and `insert_resource` calls would land in the
//! scratch world, and its systems would then run against a live world where
//! those resources do not exist: the plugin would install successfully and then
//! panic on its first frame, looking for state that went somewhere else.
//!
//! So [`with_app`] moves the world instead. The live world is swapped into an
//! empty `App` for the duration of a call and swapped back afterwards, which
//! makes every `App` method the plugin reaches for operate on the real world.
//! The temporary `App` is a shell holding a borrowed world, not a second world.
//!
//! # What the shell does not carry
//!
//! * **The shell's plugin registry is discarded**, so an installed plugin is not
//!   recorded in the editor's. Nothing downstream reads that list, but
//!   `is_plugin_added` will answer `false` for it afterwards, so a *second*
//!   plugin cannot coordinate with the first through that question.
//!   ([`install_plugin`] drives `finish` and `cleanup` off the shell's registry
//!   before it is dropped, so the lifecycle itself is not lost.)
//! * **`is_plugin_added` answers against the shell's empty registry.** Bevy's
//!   duplicate-plugin panic therefore does not fire, so a caller that might
//!   install the same plugin twice has to check for itself. Worse, a plugin
//!   that guards its own work with it (`if !app.is_plugin_added::<X>()`) will
//!   take the branch and add a second `X` over the editor's.
//! * **There are no sub-apps**, so anything reaching for `RenderApp` fails:
//!
//!   ```text
//!   ERROR bevy_render::extract_resource: Render app did not exist when trying
//!         to add `extract_resource` for <…>
//!   ```
//!
//!   A plugin that builds render-graph nodes, extract systems or a custom
//!   pipeline installs with its render half missing, and needs a restart to work.
//!   This is not fixable from here: sub-apps live on the `App`, not in the
//!   `World`, so there is nothing to lend.
//!
//! # What it still cannot do
//!
//! Unload. A loaded image is permanent: `ComponentDescriptor` stores a `drop`
//! function pointer for every component type registered through it and Bevy
//! never unregisters a component, the type registry holds `ReflectComponent`
//! function pointers, and observers and asset loaders hold more. Unmapping the
//! image leaves every one of them dangling, and the next despawn calls into
//! freed memory. So this is a one-way door, and the cost of walking through it
//! is one mapped image for the life of the process.

use bevy::prelude::*;

/// Run `f` with the live world borrowed by a temporary [`App`].
///
/// The world is put back before this returns, including if `f` panics is *not*
/// true: a panic escapes with the world still inside the shell, which drops it.
/// Callers that run untrusted plugin code should catch the panic inside `f`
/// rather than around this call.
pub fn with_app<R>(world: &mut World, f: impl FnOnce(&mut App) -> R) -> R {
    let mut shell = App::empty();
    core::mem::swap(shell.world_mut(), world);
    let result = f(&mut shell);
    core::mem::swap(shell.world_mut(), world);
    result
}

/// Install a plugin into a running world, and run its startup.
///
/// [`with_app`] alone is not enough, and the way it falls short is quiet. Bevy
/// runs `Startup` **once**, at app start. A plugin added later puts its startup
/// systems into a schedule that has already run and never will again, so they
/// are registered and never execute. The plugin appears to install: its panel
/// opens, its status item draws, and everything those things read is empty,
/// because whatever gathered the data was a startup system.
///
/// So the startup schedules are isolated for the length of the install:
///
/// 1. the live startup schedules are lifted out and set aside;
/// 2. the plugin is built, so anything it adds to `Update` and friends lands in
///    the live schedules with its ordering against editor systems intact, while
///    its startup systems land in *fresh* schedules holding only its own;
/// 3. those fresh schedules are run once, against the live world;
/// 4. the originals go back.
///
/// Only startup is isolated. Running the live `Startup` again would re-run every
/// editor startup system and spawn a second copy of the world.
/// # Panics
///
/// It does not. A plugin's `build` is arbitrary third-party code, and Bevy makes
/// a duplicate plugin a panic, so the unwind is caught **inside** [`with_app`]
/// and returned as an error. Catching it outside would be a bug rather than a
/// style choice: the shell still holds the world at that moment, so the unwind
/// would drop the editor's entire `World` on its way past.
pub fn install_plugin<P: Plugin>(world: &mut World, plugin: P) -> Result<(), String> {
    use bevy::ecs::schedule::Schedules;

    /// Bevy's startup labels, in the order `Main` runs them.
    fn startup_labels() -> [bevy::ecs::schedule::InternedScheduleLabel; 3] {
        use bevy::ecs::schedule::ScheduleLabel;
        [
            PreStartup.intern(),
            Startup.intern(),
            PostStartup.intern(),
        ]
    }

    let saved: Vec<_> = {
        let mut schedules = world.get_resource_mut::<Schedules>();
        match schedules.as_mut() {
            Some(schedules) => startup_labels()
                .into_iter()
                .filter_map(|label| schedules.remove(label))
                .collect(),
            None => Vec::new(),
        }
    };

    let built = with_app(world, |app| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            app.add_plugins(plugin);
            // The rest of the lifecycle, which a plugin added during assembly
            // gets from `App::run` and one added later would otherwise never
            // see. `finish` exists for work that needs another plugin's
            // resources to be in place, so skipping it leaves exactly the kind
            // of plugin that has real setup half-built.
            //
            // Both walk the shell's *own* registry, which holds this plugin and
            // everything it nested, so the whole tree is finished rather than
            // just the top of it.
            app.finish();
            app.cleanup();
        }))
    });

    if let Err(panic) = built {
        // The startup schedules go back before returning, or the editor is left
        // without the ones it was holding when this was called.
        if let Some(mut schedules) = world.get_resource_mut::<Schedules>() {
            for schedule in saved {
                schedules.insert(schedule);
            }
        }
        let what = panic
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "the plugin panicked while installing".to_string());
        return Err(what);
    }

    // Whatever the plugin put in a startup schedule, run now. Taken out of the
    // world first because a schedule needs `&mut World` to run and cannot be
    // borrowed from the world it runs against.
    for label in startup_labels() {
        let fresh = world
            .get_resource_mut::<Schedules>()
            .and_then(|mut s| s.remove(label));
        if let Some(mut schedule) = fresh {
            schedule.run(world);
        }
    }
    // Commands queued by startup are applied rather than left for whatever
    // happens to flush next, so the plugin is fully installed when this returns.
    world.flush();

    if let Some(mut schedules) = world.get_resource_mut::<Schedules>() {
        for schedule in saved {
            schedules.insert(schedule);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default, PartialEq, Debug)]
    struct Ran {
        built: u32,
        updated: u32,
    }

    struct LatePlugin;

    impl Plugin for LatePlugin {
        fn build(&self, app: &mut App) {
            app.init_resource::<Ran>();
            app.world_mut().resource_mut::<Ran>().built += 1;
            app.add_systems(Update, |mut ran: ResMut<Ran>| ran.updated += 1);
        }
    }

    /// The whole point: a plugin installed with only a `&mut World` in hand puts
    /// its resources in that world and its systems in that world's schedules,
    /// and the systems then run.
    ///
    /// The second assertion is the one that had to be proven rather than
    /// assumed. A system is registered during the call and run afterwards, and
    /// Bevy initialises a system lazily against whichever world first runs it.
    /// If that were not so, the system would be looking at a world that no
    /// longer exists.
    #[test]
    fn a_plugin_installs_into_a_running_world() {
        let mut app = App::new();
        app.update();

        with_app(app.world_mut(), |shell| {
            shell.add_plugins(LatePlugin);
        });

        assert_eq!(
            app.world().resource::<Ran>().built,
            1,
            "build ran against the live world"
        );

        app.update();
        app.update();
        assert_eq!(
            app.world().resource::<Ran>().updated,
            2,
            "its systems joined the live schedules and ran"
        );
    }

    /// The bug three separately-written plugins all hit: they installed, drew
    /// their UI, and showed nothing, because whatever gathered their data was a
    /// startup system and `Startup` had run hours earlier.
    #[test]
    fn a_late_plugins_startup_runs() {
        #[derive(Resource, Default, PartialEq, Debug)]
        struct Gathered {
            hardware: u32,
            frames: u32,
        }

        struct MonitorPlugin;
        impl Plugin for MonitorPlugin {
            fn build(&self, app: &mut App) {
                app.init_resource::<Gathered>();
                app.add_systems(Startup, |mut g: ResMut<Gathered>| g.hardware += 1);
                app.add_systems(Update, |mut g: ResMut<Gathered>| g.frames += 1);
            }
        }

        let mut app = App::new();
        // The editor's own startup, long since run.
        app.add_systems(Startup, || {});
        app.update();

        install_plugin(app.world_mut(), MonitorPlugin);

        assert_eq!(
            app.world().resource::<Gathered>().hardware,
            1,
            "its startup must run, or everything it feeds reads zero"
        );

        app.update();
        app.update();
        let gathered = app.world().resource::<Gathered>();
        assert_eq!(gathered.frames, 2, "its update joined the live schedule");
        assert_eq!(gathered.hardware, 1, "startup runs once, not once per frame");
    }

    /// The editor's own startup systems must not run a second time. Re-running
    /// the live `Startup` would spawn a duplicate of everything the editor
    /// built.
    #[test]
    fn the_editors_startup_is_not_re_run() {
        #[derive(Resource, Default)]
        struct EditorStartups(u32);

        struct Quiet;
        impl Plugin for Quiet {
            fn build(&self, app: &mut App) {
                app.add_systems(Startup, || {});
            }
        }

        let mut app = App::new();
        app.init_resource::<EditorStartups>();
        app.add_systems(Startup, |mut n: ResMut<EditorStartups>| n.0 += 1);
        app.update();
        assert_eq!(app.world().resource::<EditorStartups>().0, 1);

        install_plugin(app.world_mut(), Quiet);
        app.update();

        assert_eq!(
            app.world().resource::<EditorStartups>().0,
            1,
            "the editor's startup ran once, at startup, and must stay that way"
        );
    }

    /// The world comes back. Anything that was in it before is still there, and
    /// the `App` it was borrowed by is gone.
    #[test]
    fn the_world_is_returned_intact() {
        let mut app = App::new();
        let marker = app.world_mut().spawn(Name::new("editor-chrome")).id();

        with_app(app.world_mut(), |shell| {
            shell.add_plugins(LatePlugin);
        });

        assert!(
            app.world().get_entity(marker).is_ok(),
            "an entity that predates the install must survive it"
        );
        assert!(app.world().contains_resource::<Ran>());
    }
}
