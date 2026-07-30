//! Finds `renzora_plugin` cdylibs on disk and initialises them.
//!
//! Deliberately symbol-dispatched: a library is treated as a C-ABI plugin only
//! if it exports [`sys::INIT_SYMBOL`]. Anything else is skipped silently, which
//! is what lets these live in the same `plugins/` directory as the older
//! `dynamic_plugin_loader` dylibs during the migration — each loader recognises
//! its own and ignores the rest.

use bevy::prelude::*;
use libloading::{Library, Symbol};
use crate::sys;
use std::path::{Path, PathBuf};

/// One plugin path, across every load of it.
///
/// A slot's index in [`LoadedPlugins`] is its permanent identity: entries are
/// never removed, so an index stamped on a registration stays valid for the life
/// of the process. That is what the ownership tags on panels, render passes and
/// component schemas refer to.
pub struct PluginSlot {
    pub path: PathBuf,
    /// Shared with every system this slot's plugins registered. Bumping it
    /// retires the previous load's systems — see `host::GenGate`.
    pub generation: super::PluginGeneration,
    /// The generation of the newest load that succeeded.
    pub loaded_at: u32,
    /// **Every** library ever loaded for this path, and none of them is ever
    /// dropped.
    ///
    /// Deliberate. Every function pointer a plugin registered — system entries,
    /// panel action thunks, render callbacks — points into its library, and a
    /// retired system is still *in* the schedule, merely returning early. Freeing
    /// the library would turn those into dangling pointers. Dropping a
    /// `libloading::Library` has also deadlocked in `FreeLibrary` here before.
    ///
    /// So a reload leaks one library image. A few MB per reload across a dev
    /// session is a fair price for never unmapping code that something might
    /// still call, and a restart reclaims all of it.
    _libraries: Vec<Library>,
}

/// Every plugin path the loader has seen, indexed by slot.
#[derive(Resource, Default)]
pub struct LoadedPlugins(pub Vec<PluginSlot>);

impl LoadedPlugins {
    /// The slot for `path`, creating one if this is the first sighting.
    fn slot_for(&mut self, path: &Path) -> usize {
        if let Some(i) = self.0.iter().position(|s| s.path.as_path() == path) {
            return i;
        }
        self.0.push(PluginSlot {
            path: path.to_path_buf(),
            generation: super::PluginGeneration::default(),
            loaded_at: 0,
            _libraries: Vec::new(),
        });
        self.0.len() - 1
    }
}

/// Outcome for one candidate file, for logging and the editor's plugin panel.
#[derive(Debug)]
pub enum LoadOutcome {
    Loaded,
    /// Not a C-ABI plugin — no init symbol. Expected for older dylib plugins.
    NotAPlugin,
    /// The plugin needs a newer host than this one.
    VersionTooOld,
    /// The plugin's own init returned a failure, or the library would not open.
    Failed(String),
    /// The plugin belongs in the other binary. Not an error — an editor plugin
    /// sitting in a game's `plugins/` directory is expected when both were
    /// staged from one build.
    WrongScope(sys::PluginScope),
}

/// Load every `renzora_plugin` cdylib in `dir`.
///
/// Missing or unreadable directories are not an error — a build with no plugins
/// is normal.
pub fn load_dir(world: &mut World, dir: &Path, is_editor: bool) -> Vec<(PathBuf, LoadOutcome)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let ext = std::env::consts::DLL_EXTENSION;
    let mut results = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some(ext) {
            continue;
        }
        let outcome = load_one(world, &path, is_editor);
        results.push((path, outcome));
    }
    results
}

/// True if the file is a Rust **proc-macro** dylib.
///
/// These compile to a dylib for *rustc* to load, not for us, and `dlopen`ing one
/// into a process that is not the compiler crashes hard — before the splash,
/// with no panic and no crash report, because it is the OS loader failing rather
/// than Rust. We cannot detect that after `Library::new`, so detect it before:
/// every proc-macro dylib exports a `__rustc_proc_macro_decls_*` symbol, and the
/// name appears verbatim in the export table, so a plain byte search finds it
/// with no PE/ELF parsing.
///
/// Staging already filters these out (see xtask's `is_not_a_plugin`), but a dll
/// dropped into `plugins/` by hand must not be able to take the editor down.
fn is_proc_macro_dylib(path: &Path) -> bool {
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    const NEEDLE: &[u8] = b"__rustc_proc_macro_decls";
    bytes.windows(NEEDLE.len()).any(|w| w == NEEDLE)
}

fn load_one(world: &mut World, path: &Path, is_editor: bool) -> LoadOutcome {
    if is_proc_macro_dylib(path) {
        return LoadOutcome::NotAPlugin;
    }

    // SAFETY: loading arbitrary native code is inherently unsafe — a plugin can
    // do anything the process can. That is the same trust model as the existing
    // dylib loader; the C ABI buys build-environment independence, not sandboxing.
    let library = match unsafe { Library::new(path) } {
        Ok(l) => l,
        Err(e) => return LoadOutcome::Failed(format!("could not open: {e}")),
    };

    let init: Symbol<sys::ExtensionInit> =
        match unsafe { library.get(sys::INIT_SYMBOL.as_bytes()) } {
            Ok(s) => s,
            Err(_) => return LoadOutcome::NotAPlugin,
        };
    let init = *init;

    // Read the scope BEFORE calling init, so a plugin for the other binary never
    // gets the chance to register a system, a component or a panel. Checking
    // afterwards would mean unwinding registrations that already happened.
    let scope = match unsafe { library.get::<sys::ScopeEntry>(sys::SCOPE_SYMBOL.as_bytes()) } {
        Ok(f) => unsafe { f() },
        // No declaration means Runtime, matching `renzora::add!`'s default.
        Err(_) => sys::PluginScope::Runtime,
    };
    if !scope.is_known() {
        return LoadOutcome::Failed(format!(
            "declares scope {} which this build does not have",
            scope.0
        ));
    }
    if scope == sys::PluginScope::Editor && !is_editor {
        return LoadOutcome::WrongScope(scope);
    }

    // The counter is bumped only AFTER init succeeds, which is what makes a failed
    // reload harmless: a system is stale when the counter has moved *past* it, so
    // during init the new systems (higher generation) are already live while the
    // old ones (equal to the counter) still are too — and if init fails, the
    // counter never moves, the new systems stay permanently stale, and the
    // previous build keeps running. Bumping first would have retired the working
    // version before knowing whether a replacement existed.
    let (slot, counter, generation) = {
        let mut loaded = world.get_resource_or_insert_with(LoadedPlugins::default);
        let slot = loaded.slot_for(path);
        let s = &loaded.0[slot];
        let first = s._libraries.is_empty();
        (
            slot,
            s.generation.clone(),
            if first { 0 } else { s.loaded_at + 1 },
        )
    };

    // Take the slot's previous registrations back before the new build adds its
    // own, so a panel or a render pass is replaced rather than duplicated. Systems
    // are NOT in here — they retire themselves via the generation counter, because
    // Bevy cannot remove one from a schedule.
    if generation > 0 {
        super::retire_slot(world, slot);
    }

    match super::init_plugin_gen(world, init, counter.clone(), generation, slot) {
        sys::InitResult::Ok => {
            counter.store(generation, std::sync::atomic::Ordering::Relaxed);
            let mut loaded = world.resource_mut::<LoadedPlugins>();
            let s = &mut loaded.0[slot];
            s.loaded_at = generation;
            s._libraries.push(library);
            LoadOutcome::Loaded
        }
        sys::InitResult::VersionTooOld => LoadOutcome::VersionTooOld,
        sys::InitResult::Failed => {
            LoadOutcome::Failed("plugin init returned Failed".to_string())
        }
    }
}

/// Runs immediately before [`First`], and exists purely so a reload never mutates
/// a schedule that is mid-run.
///
/// A plugin's systems go into the five main-loop schedules, and `Schedules` hands
/// a schedule *out* while it runs — so a reload triggered from inside `Update`
/// would add the new build's systems to a fresh, empty `Update` that is discarded
/// the moment the real one is put back. The systems would vanish with no error.
/// Registration at `build` time avoided this by happening before any schedule ran;
/// this schedule is the same trick for a running app.
#[derive(bevy::ecs::schedule::ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PluginReload;

/// Plugin paths to reload at the next frame boundary.
///
/// A queue rather than an immediate call because the caller is usually a file
/// watcher or a UI button, neither of which holds `&mut World` at a safe moment.
#[derive(Resource, Default)]
pub struct PluginReloadQueue(pub Vec<PathBuf>);

/// Ask for `path` to be reloaded before the next frame.
///
/// Duplicates collapse: an editor save often produces several filesystem events
/// for one write, and rebuilding the same plugin three times in a frame would be
/// three sets of dead systems for no reason.
pub fn request_reload(world: &mut World, path: impl Into<PathBuf>) {
    let path = path.into();
    let mut queue = world.get_resource_or_insert_with(PluginReloadQueue::default);
    if !queue.0.contains(&path) {
        queue.0.push(path);
    }
}

/// Whether this binary is the editor, kept so a reload can apply the same scope
/// filter the initial load did.
#[derive(Resource)]
pub struct PluginHostConfig {
    pub is_editor: bool,
}

fn apply_reload_requests(world: &mut World) {
    let pending = match world.get_resource_mut::<PluginReloadQueue>() {
        Some(mut q) if !q.0.is_empty() => std::mem::take(&mut q.0),
        _ => return,
    };
    let is_editor = world
        .get_resource::<PluginHostConfig>()
        .is_some_and(|c| c.is_editor);

    for path in pending {
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        match load_one(world, &path, is_editor) {
            LoadOutcome::Loaded => {
                let generation = world
                    .get_resource::<LoadedPlugins>()
                    .and_then(|l| l.0.iter().find(|s| s.path == path))
                    .map(|s| s.loaded_at)
                    .unwrap_or(0);
                info!("[plugin] reloaded {name} (generation {generation})");
            }
            // Every failure leaves the previous build running — the generation
            // counter only moves on success — so these are warnings, not errors
            // that need the app to do anything about them.
            LoadOutcome::Failed(why) => {
                warn!("[plugin] reload of {name} failed, keeping the running build: {why}")
            }
            LoadOutcome::VersionTooOld => {
                warn!("[plugin] reload of {name} needs a newer ABI; keeping the running build")
            }
            LoadOutcome::WrongScope(scope) => {
                warn!("[plugin] reload of {name} declares {scope:?} scope, which this binary is not")
            }
            LoadOutcome::NotAPlugin => {
                warn!("[plugin] reload of {name}: no `{}` export", sys::INIT_SYMBOL)
            }
        }
    }
}

/// Host components a plugin is allowed to resolve by type path.
///
/// This exists because Bevy registers components **lazily** — adding
/// `TransformPlugin` registers the *type* for reflection but does not allocate a
/// `ComponentId` until something actually spawns or queries one. A plugin
/// loading at startup would therefore ask for a perfectly real component and get
/// nothing back, which is indistinguishable from a typo.
///
/// Registering eagerly is cheap (it allocates an id and nothing else) and has a
/// useful side effect: this list IS the public component surface. A host type
/// not named here is not reachable from a plugin, which is a decision worth
/// making deliberately rather than by accident of what happened to be spawned
/// before load.
fn register_exposed_components(world: &mut World) {
    world.register_component::<Transform>();
    world.register_component::<GlobalTransform>();
    world.register_component::<Visibility>();
    world.register_component::<Name>();
    world.register_component::<Mesh3d>();
}

/// Loads C-ABI plugins from `<exe-dir>/plugins/` during app build.
///
/// Runs at `build` time rather than in a startup system because plugins insert
/// systems into schedules, and doing that before the first frame avoids any
/// question about mutating a schedule that is mid-run.
pub struct RenzoraPluginHostPlugin {
    /// Whether this binary is the editor. Editor-scope plugins load only when
    /// it is; runtime-scope plugins load either way.
    pub is_editor: bool,
}

impl Plugin for RenzoraPluginHostPlugin {
    fn build(&self, app: &mut App) {
        let dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("plugins")))
            .unwrap_or_else(|| PathBuf::from("plugins"));

        register_exposed_components(app.world_mut());

        // Reload machinery, before the initial load so a plugin that somehow
        // requests a reload during its own init is queued rather than lost.
        app.insert_resource(PluginHostConfig { is_editor: self.is_editor })
            .init_resource::<PluginReloadQueue>()
            .init_schedule(PluginReload)
            .add_systems(PluginReload, apply_reload_requests);
        app.world_mut()
            .resource_mut::<bevy::app::MainScheduleOrder>()
            .insert_before(First, PluginReload);

        for (path, outcome) in load_dir(app.world_mut(), &dir, self.is_editor) {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            match outcome {
                LoadOutcome::Loaded => info!("[plugin] loaded {name}"),
                LoadOutcome::NotAPlugin => {}
                LoadOutcome::VersionTooOld => warn!(
                    "[plugin] {name} needs a newer renzora_plugin ABI than this build \
                     (host is {}.{})",
                    sys::VERSION_MAJOR,
                    sys::VERSION_MINOR
                ),
                // Debug, not warn: a game staged alongside the editor sees every
                // editor plugin in its `plugins/` directory, and saying so at
                // warn level once per plugin per launch is noise about something
                // working correctly.
                LoadOutcome::WrongScope(scope) => {
                    debug!("[plugin] skipping {name} — {scope:?} scope")
                }
                LoadOutcome::Failed(why) => error!("[plugin] {name} failed: {why}"),
            }
        }
    }
}
