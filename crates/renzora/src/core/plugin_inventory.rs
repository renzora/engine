//! What plugins this process found on disk, and what became of each.
//!
//! `renzora_native_plugin`'s loader reports here as it runs, so the Settings UI
//! can list every installed plugin without re-deriving the loader's rules.
//!
//! That last part is the reason this exists rather than the settings panel
//! doing its own `read_dir`. "Is this a plugin?" is a real question with a
//! non-obvious answer: a plugin is a directory containing `src/lib.rs` whose
//! manifest declares a `dylib`, or one holding nothing but a prebuilt `build/`,
//! and the loader skips entries for reasons — wrong scope, no shared engine
//! image, a build that failed — that a second implementation would silently
//! disagree about. A panel that lists a *different* set from the one the engine
//! loaded is worse than no panel, and the same mistake has been made here before
//! with script extensions.
//!
//! # Enabling and disabling
//!
//! [`load_disabled_plugins`](crate::load_disabled_plugins) is the persisted
//! list, keyed by [`PluginEntry::id`], and the loader consults it before
//! loading anything. Disabling **cannot** take effect until the next launch, and
//! that is structural rather than unfinished: a plugin adds systems, resources
//! and function pointers to the `App` while it is being assembled, and Bevy has
//! no way to withdraw those. Unmapping the image is worse still — a retired
//! system is still *in* the schedule, merely returning early.
//!
//! So the toggle records intent, and the loader acts on it at startup. The UI
//! says so plainly instead of pretending otherwise.
//!
//! Nothing here reads the disk: the loader runs during `App` assembly, before
//! any resource exists, so it reads the preference file directly and reports
//! the result to this inventory afterwards.

use bevy::prelude::*;

/// What happened to one plugin this launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginState {
    /// Installed into the `App`.
    Loaded,
    /// Turned off by the user; the loader did not open the file at all.
    Disabled,
    /// The loader declined it for a reason of its own — wrong scope for this
    /// process, no shared engine image to bind to, never built.
    /// Carries the reason, phrased for a person.
    Skipped(String),
    /// It should have loaded and did not. Carries the error.
    Failed(String),
}

impl PluginState {
    /// Whether the plugin is running right now.
    pub fn is_active(&self) -> bool {
        matches!(self, PluginState::Loaded)
    }
}

/// One plugin found on disk.
#[derive(Debug, Clone)]
pub struct PluginEntry {
    /// The stable key: the plugin's directory name under `plugins/`.
    ///
    /// This is what [`load_disabled_plugins`](crate::load_disabled_plugins)
    /// stores, so it has to be the same string on every platform. A directory
    /// name is, where the built library's filename is not — the same plugin
    /// compiles to `grayscale.dll` on Windows and `libgrayscale.so` on Linux,
    /// and a preference file that survives moving between them must not
    /// disagree about which plugin was turned off.
    pub id: String,
    pub state: PluginState,
}

/// Every plugin this process found, in the order the loaders reached them.
#[derive(Resource, Default)]
pub struct PluginInventory {
    pub entries: Vec<PluginEntry>,
}

impl PluginInventory {
    /// Record one plugin. Replaces any existing entry with the same id and kind,
    /// so a loader that re-scans does not double up.
    pub fn record(&mut self, id: impl Into<String>, state: PluginState) {
        let id = id.into();
        self.entries.retain(|e| e.id != id);
        self.entries.push(PluginEntry { id, state });
    }

    /// Entries sorted for display: by name, case-insensitively.
    pub fn sorted(&self) -> Vec<&PluginEntry> {
        let mut out: Vec<&PluginEntry> = self.entries.iter().collect();
        out.sort_by_key(|e| e.id.to_lowercase());
        out
    }
}

/// The live, editable mirror of the persisted disable list.
///
/// The loaders do **not** read this — they run during `App` assembly, before any
/// resource exists, and read the preference file directly. This is the copy the
/// Settings UI edits and the reactive bindings watch, saved back to disk on every
/// change. Same arrangement as `dev_mode`, which `load_dev_mode` reads off disk
/// for a plugin's benefit while `EditorSettings` carries the editable one.
///
/// Which means the resource and the file can disagree for exactly one session:
/// between toggling a plugin and restarting. That gap *is* the feature — see the
/// module doc on why disabling cannot take effect immediately.
#[derive(Resource, Default)]
pub struct DisabledPlugins(pub Vec<String>);

impl DisabledPlugins {
    pub fn contains(&self, id: &str) -> bool {
        self.0.iter().any(|d| d == id)
    }

    /// Turn a plugin on or off. Returns whether anything changed.
    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> bool {
        // Phrased as "is it already enabled" rather than negating the disabled
        // flag at the comparison: `enabled == !was` is the same test, but clippy
        // rejects it under `-D warnings` and it reads backwards anyway.
        let currently_enabled = !self.contains(id);
        if enabled == currently_enabled {
            return false;
        }
        if enabled {
            self.0.retain(|d| d != id);
        } else {
            self.0.push(id.to_string());
        }
        true
    }
}

/// Record a plugin into the world's inventory, creating it if needed.
///
/// A free function because the loader runs during `App` assembly, where the
/// resource may not exist yet.
pub fn record_plugin(world: &mut World, id: impl Into<String>, state: PluginState) {
    world
        .get_resource_or_insert_with(PluginInventory::default)
        .record(id, state);
}

/// Where a plugin's store artwork lives: `<exe>/plugins/<id>/thumbnail.jpg`.
///
/// Here rather than in either panel because two of them need it — Settings →
/// Plugins and the exporter's plugin picker — and a thumbnail that showed up in
/// one place and not the other would look like a broken image rather than a
/// disagreement about the path.
///
/// `None` when the install directory cannot be determined, which is the same
/// condition under which no plugins would have loaded either.
///
/// Searches every plugin root rather than assuming one beside the executable.
/// A macOS install has two — the plugins sealed inside the signed `.app` and
/// the ones the user installed into Application Support — and looking only at
/// the first would draw a placeholder for exactly the plugins someone chose to
/// install, which reads as a broken image rather than a missing file.
#[cfg(not(target_arch = "wasm32"))]
pub fn plugin_thumbnail_path(id: &str) -> Option<std::path::PathBuf> {
    let root = renzora_native_build::install::root()?;
    let dirs = renzora_native_build::install::plugin_dirs(&root);
    // The first that exists. Falling back to the first root when none does keeps
    // the old behaviour for the caller, which expects a path it can test rather
    // than a `None` meaning "no plugins at all".
    let candidates = dirs
        .iter()
        .map(|d| d.join(id).join("thumbnail.jpg"));
    let mut first = None;
    for path in candidates {
        if first.is_none() {
            first = Some(path.clone());
        }
        if path.is_file() {
            return Some(path);
        }
    }
    first
}

/// No install directory to search in a browser tab.
#[cfg(target_arch = "wasm32")]
pub fn plugin_thumbnail_path(_id: &str) -> Option<std::path::PathBuf> {
    None
}
