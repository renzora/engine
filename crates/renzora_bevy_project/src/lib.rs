//! Open a hand-written Bevy crate as a project, and show the world its code
//! builds.
//!
//! This is the answer to the thing people kept asking for: not "port your game
//! to the engine's scene format", but "write Bevy the way Bevy is meant to be
//! written, and see it in an editor". The project keeps its `main.rs`, keeps
//! `cargo run`, keeps its `assets/` folder, and gains a hierarchy, an inspector,
//! gizmos and a viewport.
//!
//! ```text
//! renzora.exe --project C:/games/starfall-grove
//!   │
//!   ├─ bevy_project::inspect        Cargo.toml says: bevy 0.19, edition 2024, src/main.rs
//!   ├─ entry::stage                 .renzora/bevy/src/lib.rs: the project as a plugin
//!   ├─ Sdk::compile                 rustc, against the SDK beside the editor
//!   └─ add_plugins                  the game's own plugins, in the editor's App
//! ```
//!
//! # A game crate is a native plugin
//!
//! Not "like one". The generated root exports `renzora_native_plugin_ctor`, is
//! built by the same `rustc` invocation against the same staged SDK, and links
//! the same shared `bevy_dylib`, which is the whole reason the game's
//! `Transform` and the editor's are the same type and its entities show up in
//! the hierarchy at all. Everything `crates/renzora_native_plugin` says about
//! the boundary applies here unchanged, including the parts about never
//! unloading an image.
//!
//! # Why it loads at boot and not when a project is opened
//!
//! Bevy plugins are installed while the `App` is being built and there is no
//! `&mut App` after `run()`, so "load the game when the user picks it from the
//! splash" is not a thing Bevy can do. The editor therefore loads the project
//! named by `--project`, which is the same argument the splash already honours,
//! and opening a *different* Bevy project offers a restart: one that carries
//! the new path on the command line.
//!
//! That is a real limitation and not a temporary one. It is also barely felt:
//! the editor is being pointed at a crate it has to compile anyway, and a
//! compile is the thing you were going to wait for regardless.
//!
//! # What this does to the rest of the editor
//!
//! Three things, each of which is one line somewhere else and each of which the
//! game would otherwise break on:
//!
//! - [`renzora::ProjectKind::Bevy`] makes the unnamed-entity guard **name**
//!   rather than despawn (`renzora_engine::named_entities`). Bevy code does not
//!   call `Name::new`, and the guard would otherwise despawn the camera, the
//!   player and the entire level on the second frame.
//! - `ProjectConfig::asset_root` becomes `assets`, because that is where Bevy's
//!   `AssetPlugin` looks and the engine otherwise resolves against the project
//!   root.
//! - [`adopt_game_cameras`] tags the game's `Camera3d` as a `SceneCamera`, which
//!   is what play mode looks for and what a raw Bevy camera has no reason to
//!   carry.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use renzora::core::bevy_project::{self, BevyCrate};
use renzora::core::console_log::{console_error, console_info, console_success};
use renzora::core::{
    DefaultCamera, EditorCamera, EditorCamera2d, HideInHierarchy, IsolatedCamera, SceneCamera,
    ViewportCamera, ViewportCamera2d,
};
use renzora::CurrentProject;

pub mod cache;
pub mod entry;
pub mod scan;
pub mod schedules;
pub mod sync;
pub mod writeback;

/// Loads a Bevy crate as the project, when the editor was pointed at one.
///
/// `Default` because the generated plugin list constructs every plugin that way.
#[derive(Default)]
pub struct BevyProjectPlugin;

renzora::add!(BevyProjectPlugin, Runtime);

impl Plugin for BevyProjectPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BevyProjectStatus>()
            .init_resource::<sync::ProjectSync>()
            // Registered here as well as by `renzora_project_watch`, and
            // `add_message` is idempotent so the second call is free. Without it
            // an exported game panics on its first frame: this plugin is added
            // unconditionally by the generated list, the watcher is Editor-scope
            // and absent, and a reader for a message nobody registered has no
            // resource to read. Same shape, same fix, as `renzora_rust_script`.
            .add_message::<renzora::core::project_files::ProjectFileChanged>()
            // **`PreUpdate`, and before anything reads the world.** The
            // project's `Startup` is what builds its world, so it has to have
            // run before the first system that expects those entities to exist:
            // the naming pass, the camera adoption and the hierarchy all look
            // for them. Running it here rather than inside `load` gives it a
            // world that has finished coming up.
            .add_systems(PreUpdate, schedules::run_project_startup)
            .add_systems(
                Update,
                (
                    offer_restart_for_a_different_project,
                    write_back_on_save,
                    // Ordered: notice the change, start a build once it settles,
                    // collect it. Chained so a save cannot wait a whole frame at
                    // each step for no reason.
                    (
                        sync::note_source_changes,
                        sync::start_rebuild,
                        sync::finish_rebuild,
                        sync::apply_reload,
                        sync::show_sync_status,
                    )
                        .chain(),
                ),
            )
            // **Not `Update`**, and the schedule is the fix.
            //
            // An adopted camera is deactivated, and bevy_pbr turns an
            // `AtmosphereEnvironmentMapLight` into an `AtmosphereEnvironmentMap`
            // from a system that also runs in `Update`. Two systems in one
            // schedule with no ordering between them is a coin flip, and losing
            // it means the probe exists on a camera that never renders, which
            // panics in `prepare_atmosphere_probe_bind_groups`. Ordering against
            // bevy's system is not possible: `bevy_pbr::atmosphere` is a private
            // module, so the system cannot be named.
            //
            // `RunFixedMainLoop` is the one schedule that sits **after**
            // `StateTransition` (where an `OnEnter` spawns a camera) and
            // **before** `Update` (where the probe would be created). So the
            // camera is always seen on the frame it appears, and always before
            // anything can turn it into a probe.
            .add_systems(bevy::app::RunFixedMainLoop, adopt_game_cameras)
            // After the game's systems, which run in `Update`.
            .add_systems(PostUpdate, keep_cursor_for_the_editor);

        // Same gate, same reason, as the native plugin loader: without the
        // shared images the game crate would link its own Bevy, and handing the
        // engine's `World` to a plugin that has a different `World` type is
        // memory corruption rather than an error. A lean export has no shared
        // images and compiles its plugins in instead.
        if !cfg!(feature = "dynamic_linking") {
            debug!(
                "[bevy-project] this build links no shared engine image, so no project code can \
                 be loaded"
            );
            return;
        }

        // Every path out of here says why.
        //
        // The first version returned silently when there was no project to load,
        // on the reasoning that not having one is the normal case and normal
        // cases should be quiet. That reasoning is wrong the moment the feature
        // does not work: an import that did nothing produced a log with no
        // mention of this crate anywhere in it, and no way to tell "there was no
        // project" from "there was one and something rejected it". One `debug!`
        // per boot is not a cost worth that.
        let root = match boot_project() {
            BootProject::None => {
                debug!("[bevy-project] no --project argument, so no project code to load");
                return;
            }
            // Deliberately louder: somebody asked for this folder by name and it
            // was turned down.
            BootProject::NotBevy(path) => {
                warn!(
                    "[bevy-project] --project {} is not a Bevy crate (no Cargo.toml with a \
                     `bevy` dependency, or its project.toml says it is an ordinary project)",
                    path.display()
                );
                return;
            }
            BootProject::Found(path) => path,
        };

        // Recorded before anything is built, and deliberately so: this is what
        // the launcher reads to decide "restart, or just open it", and a project
        // that fails to compile must still count as the one this process is for.
        // See `LaunchedBevyProject` for the restart loop that comes of getting
        // that wrong.
        app.insert_resource(renzora::core::bevy_project::LaunchedBevyProject(Some(
            root.clone(),
        )));

        let Some(krate) = bevy_project::inspect(&root) else {
            // Reachable when `project.toml` records `kind = "bevy"` but the crate
            // beside it has gone: a renamed member, a deleted `Cargo.toml`.
            error!(
                "[bevy-project] {} is recorded as a Bevy project but has no readable Cargo.toml \
                 with a `bevy` dependency",
                root.display()
            );
            return;
        };
        info!(
            "[bevy-project] loading {} (edition {}, root {})",
            krate.package,
            krate.edition,
            krate.root.display()
        );

        // The asset root, NOW, before the game's `Startup` systems can run.
        //
        // `sync_project_asset_path` does this too, from `Update`, and that is a
        // frame too late: a Bevy project loads its whole world in `Startup`, and
        // every `asset_server.load(...)` it issued there resolved against a
        // reader with no project path. The result was 23 `Path not found:
        // models/nature/tree_oak.glb` errors and a grove with nothing in it,
        // followed one second later by the path being set correctly for the
        // handles nobody was waiting on any more.
        //
        // The same reasoning, and the same fix, as the note in
        // `renzora_runtime` about sprites rendering invisibly.
        let asset_root = renzora::core::bevy_project::config_for(&root)
            .filter(|config| !config.asset_root.is_empty())
            .map(|config| root.join(&config.asset_root))
            .unwrap_or_else(|| root.clone());
        if let Some(path) = app.world().get_resource::<renzora_engine::ProjectAssetPath>() {
            info!("[bevy-project] asset root {}", asset_root.display());
            path.set(asset_root);
        }

        match load(app, &krate, &root) {
            Ok(notes) => {
                app.world_mut().resource_mut::<BevyProjectStatus>().loaded = Some(root.clone());
                for note in notes {
                    info!("[bevy-project] {note}");
                }
            }
            Err(report) => {
                // Held in a resource as well as logged. The console scrolls and
                // the Problems panel is a tracing sink, but a project that did
                // not compile is the whole state of the editor, and the user
                // needs to be able to go and look at it rather than catch it.
                error!("[bevy-project] {report}");
                app.world_mut().resource_mut::<BevyProjectStatus>().failure = Some(report);
            }
        }
    }
}

/// What happened to the project this editor was pointed at.
#[derive(Resource, Default)]
pub struct BevyProjectStatus {
    /// The project whose code is loaded into this process, if any. Fixed for the
    /// life of the process: a loaded image is never unloaded (see
    /// `renzora_native_plugin` on why), so this cannot change without a restart.
    pub loaded: Option<PathBuf>,
    /// Why it did not load. Written for a person: it names the file, the remedy,
    /// and nothing else.
    pub failure: Option<String>,
}

/// What [`boot_project`] found, kept apart so the caller can say which it was.
///
/// Three outcomes and not an `Option`, because two of the three are worth
/// different words: nobody asked for a project, and somebody asked for one that
/// this cannot load. Collapsing them was how an import that silently did nothing
/// looked exactly like an editor started with no arguments.
enum BootProject {
    None,
    NotBevy(PathBuf),
    Found(PathBuf),
}

/// The project the editor should load code from, at the moment the `App` is
/// being built.
///
/// `--project <path>`, which is the same argument the splash already honours, so
/// the successor of an import opens the project *and* loads its code from one
/// piece of information. See [`renzora::core::bevy_project::restart_into`] for
/// why this is an argument rather than an inherited environment variable.
fn boot_project() -> BootProject {
    #[cfg(target_arch = "wasm32")]
    {
        // No `dlopen`, no `rustc`, no arguments. The gate above has already
        // returned, but the function still has to compile.
        BootProject::None
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        // `skip_while` leaves `--project` itself at the head, so the value is
        // `nth(1)`. An argument list with no `--project` is consumed entirely and
        // yields nothing, which is the common case.
        let Some(path) = std::env::args()
            .skip_while(|a| a != "--project")
            .nth(1)
            .map(PathBuf::from)
        else {
            return BootProject::None;
        };
        if bevy_project::is_bevy_project(&path) {
            BootProject::Found(path)
        } else {
            BootProject::NotBevy(path)
        }
    }
}

/// Stage, compile and install the project's own plugins.
///
/// Returns the notes worth telling the user, or one report explaining why not.
fn load(app: &mut App, krate: &BevyCrate, root: &Path) -> Result<Vec<String>, String> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (app, krate, root);
        Err("a Bevy project cannot be compiled in a browser".to_string())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let staged = entry::stage(krate, root).map_err(|e| e.to_string())?;
        let mut notes = staged.notes;
        let staged_dir = staged.dir.clone();

        // Before anything is mapped, and only here. `load` runs once while the
        // editor's `App` is being built, so every library left in the staged
        // directory is from a previous session and nothing in this process holds
        // one. See [`cache::prune_old_builds`] for why that timing is the whole
        // safety argument.
        cache::prune_old_builds(&staged_dir);

        let install_root = renzora_plugin_build::install::root()
            .ok_or_else(|| "could not find the editor's own directory".to_string())?;
        let sdk = renzora_plugin_build::Sdk::load(renzora_plugin_build::install::sdk_dir(
            &install_root,
        ))
        .map_err(|e| format!("the project cannot be compiled: {e}"))?;

        // A fresh filename per build. The previous one is mapped into this
        // process on a reload and cannot be replaced on Windows; on every
        // platform, overwriting a mapped image is a way to crash later rather
        // than fail now.
        let generation = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let out = staged.dir.join(format!(
            "{}-{generation}.{}",
            krate.crate_name(),
            sdk.manifest().lib_ext
        ));

        // Reuse the last build when the engine has not moved and the project
        // has not changed. Checked here rather than inside `compile`, because
        // the rebuild-on-save path deliberately always builds: there the user
        // just changed something.
        let out = match cache::reusable(&staged_dir, root, &sdk.stamp()) {
            Some(built) => {
                info!(
                    "[bevy-project] reusing {}: unchanged since the last build",
                    built.library.file_name().unwrap_or_default().to_string_lossy()
                );
                built.library
            }
            None => {
                console_info(
                    "Bevy",
                    format!(
                        "compiling {}: this is a cargo build, so the first one is slow",
                        krate.package
                    ),
                );
                compile(&sdk, krate, root, &staged_dir, &out, &mut notes, &mut |_| {})?;
                out
            }
        };

        let plugin = unsafe { open(&out) }?;

        // What the editor owns and the game must not take from it.
        //
        // A Bevy game sets `ClearColor` because it owns the window; this one
        // does it in `LevelPlugin`, which is the right place for the sky colour
        // of a grove. Inside the editor it is not the sky colour of anything:
        // it is what every camera in the process clears to, so the editor came
        // up as a sky-blue rectangle with its own UI nowhere to be seen.
        //
        // Snapshotted and put back rather than prevented: there is no way to
        // intercept `insert_resource` from outside the plugin doing it, and a
        // game is entitled to set this. The game's cameras want it and would
        // have it if they were rendering to their own window; the editor's want
        // theirs.
        let editor_clear_color = app.world().get_resource::<ClearColor>().cloned();
        let editor_ambient = app
            .world()
            .get_resource::<bevy::light::GlobalAmbientLight>()
            .cloned();

        // `catch_unwind` around the install, and only here. A game crate's
        // `build` is arbitrary code written against a plain Bevy app, and it may
        // panic for any reason of its own.
        //
        // It used to be mostly about one reason: `add_plugins` on something the
        // editor already had, `FrameTimeDiagnosticsPlugin` being the common one,
        // since Bevy's duplicate-plugin check is a panic rather than an error.
        // That case is gone, because `schedules::capture` isolates the plugin
        // registry as well as the schedules, so a project may add a plugin the
        // editor has. This stays for everything else.
        //
        // The `App` is left half-configured afterwards, which is not good. It is
        // considerably better than taking the editor down: the user gets a
        // message naming the plugin, and can delete one line and rebuild.
        // **Only the editor holds the project's systems back.**
        //
        // In the editor the viewport is for editing, so the project's `Update`
        // is captured rather than run (see [`schedules`]). In the runtime there
        // is no editing to protect and nothing ever reloads: this *is* the game,
        // whether it was launched by Play or shipped, so the plugin is installed
        // the ordinary way and every schedule it registers runs.
        //
        // Reading `EditorSession` rather than guessing: it is inserted before
        // any plugin builds, precisely so dual-mode crates can branch on it.
        let in_editor = app
            .world()
            .get_resource::<renzora::core::EditorSession>()
            .is_some_and(|session| session.0);

        let mut project_schedules = None;
        let installed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if in_editor {
                project_schedules = Some(schedules::capture(app, |app| {
                    app.add_plugins(Boxed(plugin));
                }));
            } else {
                app.add_plugins(Boxed(plugin));
            }
        }));

        if let Some(clear) = editor_clear_color {
            let taken = app.world().get_resource::<ClearColor>().map(|c| c.0);
            if taken != Some(clear.0) {
                notes.push(format!(
                    "the project set ClearColor to {:?}; the editor's has been put back, and the \
                     game's applies when it renders to its own window",
                    taken.unwrap_or(clear.0)
                ));
                app.insert_resource(clear);
            }
        }
        // Same treatment, same reason. A game sets the ambient fill because it
        // owns the lighting; in the editor it is what lights the preview rigs,
        // the material thumbnails and every other viewport, so a grove's warm
        // sky would relight the whole application.
        if let Some(ambient) = editor_ambient {
            let taken = app
                .world()
                .get_resource::<bevy::light::GlobalAmbientLight>()
                .map(|a| (a.color, a.brightness));
            if taken != Some((ambient.color, ambient.brightness)) {
                notes.push(
                    "the project set GlobalAmbientLight; the editor's has been put back"
                        .to_string(),
                );
                app.insert_resource(ambient);
            }
        }
        if let Err(panic) = installed {
            let what = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "the plugin panicked while installing".to_string());
            return Err(format!(
                "{} panicked while building its App: {what}\n\
                 The usual cause is adding a plugin the editor already has: the editor is a \
                 Bevy app too, and Bevy treats a duplicate plugin as a panic.",
                krate.package
            ));
        }

        // Only in the editor: the runtime added the plugin normally and has
        // nothing captured, so inserting an empty set would claim otherwise.
        if let Some(captured) = project_schedules {
            // Startup is deferred to the first frame rather than run here:
            // `load` is called while the editor's own `App` is still being
            // built, and a project's `Startup` is entitled to a world that has
            // finished coming up.
            app.insert_resource(schedules::ProjectSchedules(captured));
            app.insert_resource(schedules::ProjectStartupPending(true));
            notes.push(
                "the viewport runs the project's startup only: its Update systems are held back \
                 so editing is not fighting a running game. Press Play to run it"
                    .to_string(),
            );
        }

        console_success("Bevy", format!("loaded {}", krate.package));
        notes.insert(0, format!("loaded {} from {}", krate.package, root.display()));
        Ok(notes)
    }
}

/// Compile the staged crate, dropping the optional label table if that is what
/// stands between the project and opening.
///
/// Shared by the loader and by the `stage_project` example, deliberately: the
/// example exists to tell someone why their project will not load, and an
/// example that compiled it differently from the editor would answer a question
/// nobody asked. `on_line` is where the compiler's output goes: the editor
/// discards it, the example prints it.
///
/// Diagnostics are rewritten to name the author's crate root rather than the
/// generated copy. The generated root *is* their crate root with the modules
/// redirected, so the line numbers are already theirs; only the path is wrong,
/// and a path pointing at a file that is rewritten on every build is worse than
/// no path at all.
#[cfg(not(target_arch = "wasm32"))]
pub fn compile(
    sdk: &renzora_plugin_build::Sdk,
    krate: &BevyCrate,
    project: &Path,
    staged: &Path,
    out: &Path,
    notes: &mut Vec<String>,
    on_line: &mut dyn FnMut(&str),
) -> Result<String, String> {
    let readable = |e: renzora_plugin_build::Error| {
        e.to_string().replace(
            &staged.join("src").join("lib.rs").to_string_lossy().to_string(),
            &krate.root.to_string_lossy(),
        )
    };

    let first = match sdk.compile_with(staged, out, on_line) {
        Ok(stamp) => {
            cache::record(staged, &stamp, out);
            return Ok(stamp);
        }
        Err(e) => e,
    };

    // The component-label table is the one generated thing that names the
    // project's *types* rather than copying its text, so it is the one thing
    // that can fail to compile on a crate that is otherwise fine: a `#[cfg]`-ed
    // component, a `Component` derived through a macro, a type alias. It buys
    // nicer names in the hierarchy and nothing else, so it is dropped and the
    // build retried: a project that opens with dull labels beats one that does
    // not open.
    entry::stage_with(krate, project, entry::Labels::Skip).map_err(|e| e.to_string())?;
    match sdk.compile_with(staged, out, on_line) {
        Ok(stamp) => {
            notes.push(
                "the generated component-name table did not compile, so entities are named \
                 after their Bevy components instead"
                    .to_string(),
            );
            cache::record(staged, &stamp, out);
            Ok(stamp)
        }
        // Report the FIRST error, not the second. The retry's is the same error
        // with one feature missing, and the first one is the one whose line
        // numbers match what the user wrote.
        Err(_) => Err(readable(first)),
    }
}

/// `dlopen` a built game crate and construct its plugin.
///
/// # Safety
///
/// The library was compiled a moment ago, by this process, from source in the
/// project the user opened. Same trust model as a native plugin, and the same
/// rule about the handle: it is **never dropped**. Every system the plugin
/// registers is a function pointer into this image and the schedule holds those
/// for the life of the `App`; unmapping it turns them into dangling pointers,
/// and `FreeLibrary` has deadlocked this engine before.
#[cfg(not(target_arch = "wasm32"))]
unsafe fn open(path: &Path) -> Result<Box<dyn Plugin>, String> {
    let lib = unsafe { libloading::Library::new(path) }
        .map_err(|e| format!("could not load {}: {e}", path.display()))?;
    let lib = std::mem::ManuallyDrop::new(lib);
    type Ctor = unsafe fn() -> Box<dyn Plugin>;
    let ctor: libloading::Symbol<Ctor> = unsafe {
        lib.get(renzora_native_plugin::CTOR_SYMBOL)
    }
    .map_err(|e| {
        format!(
            "{} has no `renzora_native_plugin_ctor` ({e}). The generated crate root should have \
             called `renzora::plugin!`; this is an engine bug, not a project one",
            path.display()
        )
    })?;
    Ok(unsafe { ctor() })
}

/// Adapts a `Box<dyn Plugin>` into something `add_plugins` accepts.
///
/// The same wrapper `renzora_native_plugin` uses, and for the same reason: a
/// boxed plugin is not itself a `Plugin`, and Bevy names plugins by
/// `type_name`, which would make every boxed one look like the same plugin.
struct Boxed(Box<dyn Plugin>);

impl Plugin for Boxed {
    fn build(&self, app: &mut App) {
        self.0.build(app);
    }
    fn name(&self) -> &str {
        self.0.name()
    }
    fn is_unique(&self) -> bool {
        false
    }
}

/// Tell the user that the project they just opened needs a restart, and offer
/// to do it.
///
/// A Bevy project opened from the splash has its config, its asset root and its
/// files: everything except its *code*, which could only have been installed
/// while the `App` was being built. Rather than leaving an empty viewport and no
/// explanation, this says so once.
fn offer_restart_for_a_different_project(
    project: Option<Res<CurrentProject>>,
    status: Res<BevyProjectStatus>,
    mut told: Local<bool>,
) {
    let Some(project) = project else { return };
    if !project.is_changed() || *told {
        return;
    }
    if !project.config.kind.is_code_first() {
        return;
    }
    if status.loaded.as_ref() == Some(&project.path) {
        return;
    }
    *told = true;
    console_error(
        "Bevy",
        format!(
            "{} is a Bevy project, but its code is not loaded. A game crate is installed while \
             the editor's App is being built, which has already happened. Restart with \
             `--project {}` to load it.",
            project.config.name,
            project.path.display()
        ),
    );
}

/// Make the game's cameras legible to the editor.
///
/// A camera spawned by Bevy code carries `Camera3d` and nothing else. Play mode
/// looks for [`SceneCamera`] and refuses to start without one
/// (`renzora_viewport::play_mode`), so without this the game loads, renders, and
/// the Play button reports "no scene camera found" about a project whose camera
/// is right there in the hierarchy.
///
/// It also **deactivates** the camera in the editor. A raw Bevy camera renders
/// to the window at order 0, which is on top of the editor's own chrome; the
/// editor's viewport camera is driven onto this one's pose instead
/// (`renzora_camera::drive_editor_camera_in_play`), so the game is seen through
/// the editor's pipeline and nothing on the GPU changes when play is toggled.
/// In a shipped game there is no editor camera and this leaves it alone.
#[allow(clippy::type_complexity)]
fn adopt_game_cameras(
    mut commands: Commands,
    project: Option<Res<CurrentProject>>,
    editor: Option<Res<renzora::core::EditorSession>>,
    // **No `Added<Camera>` filter.** It was there to keep the query cheap and it
    // made the whole thing depend on which frame `CurrentProject` landed on: the
    // camera is spawned in the game's `Startup` and the project is inserted in
    // the splash's, and an adoption that misses its one frame never happens
    // again. Inserting `SceneCamera` is what makes the query cheap instead:
    // an adopted camera drops out of it permanently.
    mut cameras: Query<
        (Entity, &mut Camera),
        (
            Without<SceneCamera>,
            Without<EditorCamera>,
            Without<EditorCamera2d>,
            Without<ViewportCamera>,
            Without<ViewportCamera2d>,
            Without<IsolatedCamera>,
            Without<HideInHierarchy>,
            // The camera the editor's own UI renders on, excluded by what it
            // *is* rather than by name. Every other exclusion here is a marker
            // this crate has to know about in advance, and a list like that is
            // only ever as good as the last time someone remembered to add to
            // it: the editor's UI camera carries `EditorUiCamera`, which lives
            // in an editor crate a `Runtime` plugin cannot depend on, so it was
            // not on the list, was adopted as a game camera, and was switched
            // off, taking the whole editor with it.
            //
            // A game's camera is never the editor's default UI camera, so this
            // one holds however the marker lists drift.
            Without<bevy::ui::IsDefaultUiCamera>,
        ),
    >,
    existing_default: Query<(), With<DefaultCamera>>,
) {
    if !project.is_some_and(|p| p.config.kind.is_code_first()) {
        return;
    }
    // `EditorSession` is a `bool`, not a marker: it is inserted in a shipped
    // game too, holding `false`. Testing it for presence deactivated the game's
    // camera in the game, which is every camera it has.
    let in_editor = editor.is_some_and(|e| e.0);
    let mut needs_default = existing_default.is_empty();
    for (entity, mut camera) in cameras.iter_mut() {
        let mut e = commands.entity(entity);
        e.insert(SceneCamera);
        if needs_default {
            e.insert(DefaultCamera);
            needs_default = false;
        }
        if in_editor {
            // A raw Bevy camera renders to the window at order 0, which is on
            // top of the editor's own chrome, and clears the whole surface on
            // the way. The editor's viewport camera is driven onto this one's
            // pose instead, so the game is seen through the editor's pipeline.
            camera.is_active = false;

            // ...and an atmosphere probe cannot survive that.
            //
            // `AtmosphereEnvironmentMapLight` makes the camera an environment
            // probe, and `prepare_atmosphere_probe_bind_groups` iterates every
            // probe and unwraps the atmosphere uniforms. Those uniforms are
            // written by *rendering an atmosphere view*, which an inactive
            // camera never does, so the probe outlives its own inputs and
            // bevy_pbr panics on `Option::unwrap()` at
            // `atmosphere/environment.rs:116`.
            //
            // The engine has met this before from the other side: deactivating
            // the editor's own camera while it held a probe crashed the same
            // way. Here the probe belongs to the game, and in the editor it has
            // nothing to contribute anyway: the editor camera renders the view,
            // with the editor's own environment. In a shipped game there is no
            // editor camera, `in_editor` is false, and the probe is left alone.
            e.remove::<bevy::light::AtmosphereEnvironmentMapLight>();
            // Its settings go with it. They describe how a view renders an
            // atmosphere, and this view renders nothing.
            e.remove::<bevy::pbr::AtmosphereSettings>();
        }
        info!(
            "[bevy-project] adopted {entity} as a scene camera{}",
            if in_editor { " (deactivated; the editor viewport renders its view)" } else { "" }
        );
    }
}

/// Write the editor's placements back into the project when the project is saved.
///
/// Hooked to the same `SaveSceneRequested` that Ctrl+S raises, because to the
/// person pressing it this *is* saving the scene: a code-first project simply
/// keeps its scene in a `.rs` file rather than a `.bsn` one.
///
/// Runs before `renzora_scene`'s own save consumes the request? No: it does not
/// consume it at all. Both handlers read the marker and only the scene saver
/// removes it, so the ordering between them does not matter and neither has to
/// know the other exists.
fn write_back_on_save(
    world: &mut World,
) {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = world;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if world.get_resource::<renzora::core::SaveSceneRequested>().is_none() {
            return;
        }
        let Some(project) = world.get_resource::<CurrentProject>() else {
            return;
        };
        if !project.config.kind.is_code_first() {
            return;
        }
        let root = project.path.clone();
        let Some(krate) = bevy_project::inspect(&root) else {
            return;
        };

        let spawns = writeback::collect(world);
        let count = spawns.len();
        match writeback::write(&krate.root, &spawns) {
            Ok(true) => {
                let path = writeback::module_path(&krate.root);
                console_success(
                    "Bevy",
                    format!(
                        "wrote {count} entit{} to {}",
                        if count == 1 { "y" } else { "ies" },
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ),
                );
                info!("[bevy-project] wrote {count} authored entities to {}", path.display());
            }
            // Nothing changed. Silent on purpose: Ctrl+S on an unedited project
            // should not produce a line, and the file's mtime staying put is
            // what keeps it out of the watcher and out of `git status`.
            Ok(false) => {}
            Err(e) => {
                console_error("Bevy", format!("could not write the authored module: {e}"));
                error!("[bevy-project] write-back failed: {e}");
            }
        }
    }
}

/// Keep the window's cursor under the editor's control outside play mode.
///
/// A game owns the cursor: it locks it for mouse-look and hides it, usually on a
/// click. Inside the editor that click is just as likely to have been aimed at
/// the hierarchy, and the project this was found on locks on **any** left press
/// anywhere:
///
/// ```ignore
/// } else if buttons.just_pressed(MouseButton::Left) && cursor.grab_mode == CursorGrabMode::None {
///     cursor.grab_mode = CursorGrabMode::Locked;
///     cursor.visible = false;
/// }
/// ```
///
/// So clicking a panel froze the pointer over the whole editor.
///
/// # Why reassert rather than restore
///
/// `ClearColor` and the ambient light are set once while the plugin is built, so
/// snapshotting them and putting them back afterwards is enough. The cursor is
/// set by a *system*, every frame it decides to, and there is no moment after
/// which it stops. The only thing that works is to take it back every frame,
/// which is what this does, in `PostUpdate`, after the game's systems have had
/// their say in `Update`.
///
/// Play mode hands it over: the game is meant to own the cursor while it is
/// being played. `is_in_play_mode` deliberately excludes Simulating, where the
/// editor stays live and the pointer stays the user's.
#[allow(clippy::type_complexity)]
fn keep_cursor_for_the_editor(
    project: Option<Res<CurrentProject>>,
    editor: Option<Res<renzora::core::EditorSession>>,
    play: Option<Res<renzora::core::PlayModeState>>,
    mut cursors: Query<&mut bevy::window::CursorOptions, With<bevy::window::PrimaryWindow>>,
) {
    if !project.is_some_and(|p| p.config.kind.is_code_first()) {
        return;
    }
    if !editor.is_some_and(|e| e.0) {
        return;
    }
    if play.is_some_and(|p| p.is_in_play_mode()) {
        return;
    }
    for mut cursor in &mut cursors {
        // Guarded writes. `CursorOptions` is change-detected and winit acts on a
        // change, so writing the same values every frame would be a window
        // command per frame for the life of the editor.
        if cursor.grab_mode != bevy::window::CursorGrabMode::None {
            cursor.grab_mode = bevy::window::CursorGrabMode::None;
        }
        if !cursor.visible {
            cursor.visible = true;
        }
    }
}
