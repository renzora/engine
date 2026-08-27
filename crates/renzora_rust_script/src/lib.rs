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
//! # What is not solved yet
//!
//! **No hot reload.** Scripts compile when the project opens. Recompiling on save
//! and swapping the function pointer is the obvious next step, and the mechanism
//! is known — but every reload leaks the old image, because a schedule may still
//! hold pointers into it.
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
use renzora_scripting::ScriptComponent;

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
            .add_systems(Update, dispatch.run_if(scripts_should_run));
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
    let f: Symbol<ScriptFn> = unsafe { lib.get(SCRIPT_SYMBOL) }.map_err(|_| {
        "exports no entry point — did you forget `renzora::script!(update);`?".to_string()
    })?;
    let f = *f;
    Ok((f, lib))
}

/// The directory holding the editor, which is where `sdk/` lives.
pub fn sdk_root() -> Option<PathBuf> {
    exe_dir()
}

/// Whether scripts should run this frame — the same rule the Lua path uses.
///
/// In the editor: when play mode says scripts are running, or when at least one
/// script is being *previewed* (the inspector's per-script play button), in which
/// case [`dispatch`] runs only the previewed ones so the rest of the scene stays
/// static. In a standalone runtime there is no `PlayModeState`, so always.
///
/// Deliberately a copy of `renzora_scripting`'s condition rather than a call to
/// it: that one is private, and the two must agree — a Rust script that ran in
/// edit mode while the Lua script beside it did not would be a confusing bug to
/// chase.
fn scripts_should_run(
    play_mode: Option<Res<renzora::PlayModeState>>,
    scripts: Query<&ScriptComponent>,
) -> bool {
    match play_mode {
        Some(pm) if pm.is_scripts_running() => true,
        Some(_) => scripts
            .iter()
            .any(|sc| sc.scripts.iter().any(|e| e.enabled && e.preview)),
        None => true,
    }
}

/// Call each entity's `.rs` scripts once per frame.
///
/// Exclusive, because a script takes `&mut World` and nothing else may be
/// borrowed while it runs — which is also why the pairs are collected first.
pub fn dispatch(world: &mut World) {
    // Edit-mode preview: the run condition let us through because at least one
    // script has its inspector play button on, not because play mode started. Run
    // ONLY those, so the rest of the scene stays static — same rule as the Lua
    // executor.
    let preview_only = world
        .get_resource::<renzora::PlayModeState>()
        .map(|pm| !pm.is_scripts_running())
        .unwrap_or(false);

    let mut q = world.query::<(Entity, &ScriptComponent)>();
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

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf))
}
