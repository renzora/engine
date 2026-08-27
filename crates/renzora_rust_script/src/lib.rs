//! Rust scripts: per-entity native code, compiled from the project, with the
//! same `&mut World` an exclusive system gets.
//!
//! ```ignore
//! // <project>/scripts/spin.rs
//! use bevy::prelude::*;
//!
//! fn update(world: &mut World, me: Entity) {
//!     let dt = world.resource::<Time>().delta_secs();
//!     if let Some(mut t) = world.get_mut::<Transform>(me) {
//!         t.rotate_y(dt);
//!     }
//! }
//!
//! renzora::script!(update);
//! ```
//!
//! Attached exactly like a Lua script: drop it into the entity's **Scripts**
//! component. Routing is by file extension, the same way `.lua`, `.blueprint`
//! and `.bp` already route.
//!
//! Everything Bevy allows is allowed: spawn hierarchies, build UI, insert
//! components, swap materials, reach other entities by [`Entity`]. There is no
//! vocabulary in the way, because the script and the engine share one Bevy.
//!
//! # How this fits the scripting layer, and where it does not
//!
//! A backend normally returns [`ScriptCommand`]s for a queue to apply — safe,
//! interchangeable, and exactly what a Rust script does not want. So this crate
//! splits the two halves a backend usually does together:
//!
//! * [`backend::RustScriptBackend`] **claims** `.rs`, so the Scripts component
//!   accepts one and the execution loop does not flag it as broken.
//! * [`dispatch`] **runs** it, from an exclusive system with the real world.
//!
//! # A script IS a native plugin
//!
//! Not "like one" — the same thing, built by the same compiler driver against
//! the same SDK. The only difference is the convention: a plugin exports a
//! `Plugin` and installs once, a script exports a per-entity function called for
//! each entity carrying it. So the limits are the plugin limits, not new ones —
//! see `crates/renzora_native_plugin`.
//!
//! # Reloading
//!
//! Saving a script rebuilds it, off the main thread, and swaps the function
//! pointer — see [`watch`]. Every reload leaks the old image, because a schedule
//! or a captured closure may still hold pointers into it; a restart reclaims it.
//!
//! # What is not solved yet
//!
//! **Nothing in a lean export.** A static build links no shared images, so a
//! script library has nothing to bind to. The answer is to compile scripts INTO
//! the export, which the lean exporter is already shaped for.
//!
//! **No props.** Lua declares tunables in a table the backend parses; the Rust
//! equivalent is reading attributes off the source. Until then, a script's
//! tunables are ordinary components on the entity, which the inspector already
//! edits.

pub mod backend;
pub mod watch;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use libloading::{Library, Symbol};
use renzora::core::console_log::{console_error, console_success};
use renzora::{CurrentProject, SplashState};
use renzora_plugin_build::Sdk;
use renzora_scripting::{scripts_should_run, ScriptComponent};

/// The symbol a script exports, written by [`renzora::script!`].
pub const SCRIPT_SYMBOL: &[u8] = b"renzora_script_update\0";

/// Its signature.
///
/// A plain Rust `fn` taking `&mut World`, sound only because the script and the
/// engine link one shared `bevy_dylib` — the precondition everything here rests
/// on.
type ScriptFn = fn(&mut World, Entity);

renzora::add!(RustScriptPlugin, Runtime);

#[derive(Default)]
pub struct RustScriptPlugin;

impl Plugin for RustScriptPlugin {
    fn build(&self, app: &mut App) {
        if !cfg!(feature = "dynamic_linking") {
            debug!("rust scripts unavailable: this build links no shared engine image");
            return;
        }
        app.init_resource::<LoadedScripts>()
            .init_resource::<watch::ScriptWatcher>()
            // Recompile on save. Unlike `dispatch` these are NOT gated on play
            // mode: a script should build when you save it, so the error is in
            // front of you while you are still looking at the code — not the next
            // time you press play.
            .add_systems(Update, (watch::watch, watch::finish))
            // Claims `.rs` with the engine. Not done in `build` because the
            // engine is a resource another plugin creates, and plugin build
            // order is not something to depend on.
            .add_systems(PreUpdate, register_backend)
            // Compiling is separate from dispatching so one script failing to
            // build leaves the others running, and so the compile can later move
            // off the main thread without touching the dispatcher.
            .add_systems(OnEnter(SplashState::Editor), compile_and_load)
            // Gated exactly like the Lua path. Without this a script starts
            // running the moment it is dropped on an entity, in edit mode, which
            // is both surprising and destructive — a script that spawns or
            // despawns would do so while you are still arranging the scene.
            //
            // Ordered after `ScriptingSet::PreScript` because that is where
            // `ScriptsActive` — the resource the run condition reads — is filled
            // for the frame. Unordered, this would see last frame's answer
            // whenever the scheduler happened to run it first, so toggling a
            // script's preview button would take effect a frame later here than
            // in the Lua path for no reason anyone could see.
            .add_systems(
                Update,
                dispatch
                    .after(renzora_scripting::ScriptingSet::PreScript)
                    .run_if(scripts_should_run),
            );
    }

    /// Take ownership of `ScriptsActive` when nothing else has.
    ///
    /// `ScriptingPlugin` normally fills that resource once per frame, and the
    /// run condition above reads it. But that plugin sits behind the runtime's
    /// strippable `scripting` feature — a shipped game with no Lua drops the
    /// whole host layer — while this plugin is added unconditionally by the
    /// generated plugin list. In that build the run condition would ask for a
    /// resource with no owner and panic on the first frame, which is the worst
    /// possible place to find out: an exported game, not the editor.
    ///
    /// `.rs` scripts do not need the scripting host to run (they are dispatched
    /// from here against `&mut World`), so the right answer is to keep gating
    /// them rather than to silently stop. Adding `renzora_scripting`'s own
    /// system re-uses the rule instead of restating it.
    ///
    /// In `finish` rather than `build` because it has to observe whether
    /// `ScriptingPlugin` was added, and plugin build order is not something to
    /// depend on — `finish` runs after every `build`.
    fn finish(&self, app: &mut App) {
        if app.is_plugin_added::<renzora_scripting::ScriptingPlugin>() {
            return;
        }
        app.init_resource::<renzora_scripting::ScriptsActive>()
            .configure_sets(Update, renzora_scripting::ScriptingSet::PreScript)
            .add_systems(
                Update,
                renzora_scripting::update_scripts_active
                    .in_set(renzora_scripting::ScriptingSet::PreScript),
            );
    }
}

/// Register the `.rs` backend the first time a [`ScriptEngine`] exists.
///
/// A polled `Local` rather than a one-shot at startup, because the engine may be
/// created after this plugin is built — and if it is not registered, `.rs` is an
/// extension nothing claims: the Scripts component refuses the drop, the picker
/// does not list it, and the execution loop reports `No backend for Some("rs")`.
///
/// The steady-state cost is one `bool` test per frame.
fn register_backend(
    engine: Option<ResMut<renzora_scripting::ScriptEngine>>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let Some(mut engine) = engine else { return };
    engine.add_backend(Box::new(backend::RustScriptBackend::default()));
    *done = true;
}

/// Every script image loaded this session, and each one's entry point.
///
/// `ManuallyDrop` because a resource is dropped with the World on every clean
/// shutdown, and unmapping code something may still call has crashed the runtime
/// here before. See `renzora_plugin`'s loader.
#[derive(Resource, Default)]
pub struct LoadedScripts {
    entries: HashMap<String, ScriptFn>,
    _images: Vec<std::mem::ManuallyDrop<Library>>,
}

impl LoadedScripts {
    pub fn is_loaded(&self, file_name: &str) -> bool {
        self.entries.contains_key(file_name)
    }

    /// Point `file_name` at a newly loaded image.
    ///
    /// Replacing the entry retires the previous function pointer, but the image
    /// it lived in is kept — see [`crate::watch`] for why unmapping it is not an
    /// option.
    pub fn insert(&mut self, file_name: String, f: ScriptFn, lib: Library) {
        self.entries.insert(file_name, f);
        self._images.push(std::mem::ManuallyDrop::new(lib));
    }
}

/// Build and load every `.rs` in the open project's `scripts/`.
///
/// On entering the editor rather than at startup, because a project — and
/// therefore a `scripts/` directory — does not exist before then.
fn compile_and_load(world: &mut World) {
    let Some(project) = world.get_resource::<CurrentProject>().map(|p| p.path.clone()) else {
        return;
    };
    let scripts_dir = project.join("scripts");
    if !scripts_dir.is_dir() {
        return;
    }

    let sources: Vec<PathBuf> = std::fs::read_dir(&scripts_dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rs"))
        .collect();
    if sources.is_empty() {
        return;
    }

    let Some(root) = exe_dir() else { return };
    let sdk = match Sdk::load(root.join("sdk")) {
        Ok(sdk) => sdk,
        Err(e) => {
            // Said once rather than per script: without an SDK nothing can be
            // built, and the reason is the same for all of them.
            warn!("rust scripts cannot be built: {e}");
            console_error("Script", format!("Rust scripts cannot be built — {e}"));
            return;
        }
    };

    for src in sources {
        let name = src.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string();
        // Claim this source's mtime for the watcher BEFORE building it. The
        // watcher decides what to rebuild by comparing against `seen`, and it
        // has never seen anything yet — so without this it noticed every script
        // half a second later and built the whole directory a second time, on
        // the task pool, while these builds were still finishing. Two rustc runs
        // per script at project open, and a leaked image for each.
        //
        // Recorded even when the build below fails, matching the watcher's own
        // rule: a script that does not compile stays quiet until it is edited
        // again rather than re-reporting the same error every poll.
        if let Ok(mtime) = std::fs::metadata(&src).and_then(|m| m.modified()) {
            world.resource_mut::<watch::ScriptWatcher>().mark_seen(name.clone(), mtime);
        }
        match build_to_path(&sdk, &project, &src).and_then(|p| load_library(&p)) {
            Ok((f, lib)) => {
                world.resource_mut::<LoadedScripts>().insert(name.clone(), f, lib);
                info!("[rust-script] loaded {name}");
                console_success("Script", format!("compiled {name}"));
            }
            Err(e) => {
                // Both, and neither is redundant. `error!` reaches stdout and the
                // Problems panel (which has a tracing layer); the Console panel
                // has none and only shows what is pushed to it explicitly. A
                // compile error is the single thing a script author most needs to
                // see, so it goes to the place they are already looking.
                error!("[rust-script] {name}: {e}");
                console_error("Script", format!("{name}\n{e}"));
            }
        }
    }
}

/// Compile one `.rs` into `<project>/.renzora/scripts/`, returning the library.
///
/// Split from [`load_library`] so the compile — the second that matters — can run
/// on a task pool while the load stays on the main thread. Nothing here touches
/// the `World`, which is what makes that possible.
///
/// The build directory is hidden inside the project because these are derived:
/// they belong with the project, but nobody should be asked to look at or commit
/// them.
pub fn build_to_path(sdk: &Sdk, project: &Path, src: &Path) -> Result<PathBuf, String> {
    let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("script");
    let build = project.join(".renzora").join("scripts").join(stem);
    std::fs::create_dir_all(build.join("src")).map_err(|e| e.to_string())?;

    // `Sdk::compile` takes a plugin-shaped DIRECTORY (`<dir>/src/lib.rs`) — it
    // generates the Cargo.toml Bevy's derives need there and takes the crate name
    // from it. A script is one loose file, so it is staged into that shape rather
    // than teaching the compiler a second layout.
    std::fs::copy(src, build.join("src").join("lib.rs")).map_err(|e| e.to_string())?;

    // A recompile writes a NEW file rather than overwriting the loaded one: on
    // Windows the previous library is still mapped and cannot be replaced, and on
    // every platform overwriting a mapped image is a way to crash later rather
    // than fail now. The generation counter is the file's own mtime, which is
    // monotonic enough and needs no state kept anywhere.
    let gen = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let out = build.join(format!("{stem}-{gen}.{}", sdk.manifest().lib_ext));

    sdk.compile(&build, &out).map_err(|e| {
        // Point at the file the author edits, not the staged copy they have never
        // seen — a diagnostic naming `.renzora/scripts/spin/src/lib.rs` sends
        // them to a derived file that is overwritten on every build.
        e.to_string()
            .replace(&build.join("src").join("lib.rs").to_string_lossy().to_string(), &src.to_string_lossy())
    })?;
    Ok(out)
}

/// `dlopen` a built script and find its entry point.
///
/// SAFETY: code compiled from the project's own source, put there by the person
/// running the editor. Same trust model as a plugin.
pub fn load_library(path: &Path) -> Result<(ScriptFn, Library), String> {
    let lib = unsafe { Library::new(path) }.map_err(|e| e.to_string())?;
    let f: Symbol<ScriptFn> = match unsafe { lib.get(SCRIPT_SYMBOL) } {
        Ok(f) => f,
        Err(_) => {
            // Leaked rather than returned to be dropped. `Library::new` already
            // ran the image's static initializers, and unmapping a warmed Rust
            // dylib runs `FreeLibrary` inside the loader lock — the deadlock
            // `renzora_plugin`'s loader hit. A script missing its entry point is
            // an author typo, so this happens while someone iterates: exactly
            // the situation where it would be hit repeatedly.
            std::mem::forget(lib);
            return Err(
                "exports no entry point — did you forget `renzora::script!(update);`?".to_string(),
            );
        }
    };
    let f = *f;
    Ok((f, lib))
}

/// The directory holding the editor, which is where `sdk/` lives.
pub fn sdk_root() -> Option<PathBuf> {
    exe_dir()
}

// Whether scripts should run this frame is `renzora_scripting`'s
// `scripts_should_run`, imported above rather than reimplemented here. This used
// to be a hand-kept copy because the original was private, with a comment noting
// that the two "must agree" — a Rust script that ran in edit mode while the Lua
// script beside it did not would be a confusing bug to chase. It is now `pub`
// and reads a resource computed once per frame, so the copy is gone and the two
// paths cannot drift apart. `finish` below covers the one build where the
// resource would otherwise have no owner.

/// Call each entity's `.rs` scripts once per frame.
///
/// Exclusive, because a script takes `&mut World` and nothing else may be
/// borrowed while it runs — which is also why the pairs are collected first.
pub fn dispatch(
    world: &mut World,
    // Cached rather than `world.query::<…>()` per call. Building a `QueryState`
    // walks every archetype in the world to work out which ones match, and this
    // runs each frame of play mode in a scene with thousands of them — paid to
    // rediscover an answer that changes only when an archetype is created.
    // `Local` keeps one across frames and `iter` updates it incrementally.
    mut q: Local<bevy::ecs::query::QueryState<(Entity, &'static ScriptComponent)>>,
) {
    // Edit-mode preview: the run condition let us through because at least one
    // script has its inspector play button on, not because play mode started. Run
    // ONLY those, so the rest of the scene stays static — same rule as the Lua
    // executor.
    let preview_only = world
        .get_resource::<renzora::PlayModeState>()
        .map(|pm| !pm.is_scripts_running())
        .unwrap_or(false);

    let calls: Vec<(Entity, String)> = q
        .iter(world)
        .flat_map(|(entity, sc)| {
            sc.scripts
                .iter()
                .filter(|e| e.enabled && (!preview_only || e.preview))
                .filter_map(|e| e.script_path.as_ref())
                .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("rs"))
                // Keyed by file name: an entry's path may be project-relative or
                // scripts-relative depending on how it was added, but the leaf is
                // the same either way and is what the loader keyed on.
                .filter_map(|p| p.file_name()?.to_str().map(|n| (entity, n.to_string())))
                .collect::<Vec<_>>()
        })
        .collect();
    if calls.is_empty() {
        return;
    }

    // Resolved before the loop so the resource is not borrowed across a call that
    // may insert, remove or despawn anything at all.
    let resolved: Vec<(Entity, ScriptFn)> = {
        let loaded = world.resource::<LoadedScripts>();
        calls
            .into_iter()
            .filter_map(|(e, name)| loaded.entries.get(&name).map(|f| (e, *f)))
            .collect()
    };

    for (entity, f) in resolved {
        // An earlier script may have despawned this entity — its own, even.
        if world.get_entity(entity).is_err() {
            continue;
        }
        // A panic crossing the boundary is undefined, so it is caught: a broken
        // script stops working rather than taking the editor with it. A segfault
        // is still fatal and nothing here can help with that.
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(world, entity))).is_err() {
            error!("[rust-script] panicked on {entity}");
            // Deliberately not per-frame-guarded: a panicking script panics every
            // frame, and a Console that says so sixty times is still better than
            // one that says it once and scrolls away. The panel coalesces repeats
            // into a count.
            console_error("Script", format!("panicked while running on {entity}"));
        }
    }
}

/// The directory holding `sdk/`, which is where scripts are compiled against.
///
/// NOT simply the executable's parent: inside a Linux AppImage that is a
/// read-only temporary mount with no SDK beside it. See
/// [`renzora_plugin_build::install`].
fn exe_dir() -> Option<PathBuf> {
    renzora_plugin_build::install::root()
}
