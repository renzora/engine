//! Finds `renzora_plugin` cdylibs on disk and initialises them.
//!
//! Deliberately symbol-dispatched: a library is treated as a C-ABI plugin only
//! if it exports [`sys::INIT_SYMBOL`]. Anything else is skipped silently, which
//! is what lets these live in the same `plugins/` directory as the older
//! `dynamic_plugin_loader` dylibs during the migration — each loader recognises
//! its own and ignores the rest.

use super::init_plugin;
use bevy::prelude::*;
use libloading::{Library, Symbol};
use crate::sys;
use std::path::{Path, PathBuf};

/// Loaded libraries, kept alive for the process lifetime.
///
/// **Never drop these.** Every function pointer a plugin registered — system
/// entry points, component destructors — points into its library, and the
/// schedule holds those pointers for as long as the app runs. Unloading safely
/// needs a registration ledger and a teardown pass that strips the plugin's
/// systems and components first; until that exists, leaking is the correct
/// behaviour rather than a shortcut.
#[derive(Resource, Default)]
pub struct LoadedPlugins(pub Vec<LoadedPlugin>);

pub struct LoadedPlugin {
    pub path: PathBuf,
    /// Held purely to keep the library mapped. Never dropped.
    _library: Library,
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

    match init_plugin(world, init) {
        sys::InitResult::Ok => {
            world
                .get_resource_or_insert_with(LoadedPlugins::default)
                .0
                .push(LoadedPlugin {
                    path: path.to_path_buf(),
                    _library: library,
                });
            LoadOutcome::Loaded
        }
        sys::InitResult::VersionTooOld => LoadOutcome::VersionTooOld,
        sys::InitResult::Failed => {
            LoadOutcome::Failed("plugin init returned Failed".to_string())
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
