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
}

/// Load every `renzora_plugin` cdylib in `dir`.
///
/// Missing or unreadable directories are not an error — a build with no plugins
/// is normal.
pub fn load_dir(world: &mut World, dir: &Path) -> Vec<(PathBuf, LoadOutcome)> {
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
        let outcome = load_one(world, &path);
        results.push((path, outcome));
    }
    results
}

fn load_one(world: &mut World, path: &Path) -> LoadOutcome {
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
pub struct RenzoraPluginHostPlugin;

impl Plugin for RenzoraPluginHostPlugin {
    fn build(&self, app: &mut App) {
        let dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("plugins")))
            .unwrap_or_else(|| PathBuf::from("plugins"));

        register_exposed_components(app.world_mut());

        for (path, outcome) in load_dir(app.world_mut(), &dir) {
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
                LoadOutcome::Failed(why) => error!("[plugin] {name} failed: {why}"),
            }
        }
    }
}
