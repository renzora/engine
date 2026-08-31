//! What plugins this process found on disk, and what became of each.
//!
//! Both loaders report here as they run — `renzora_plugin`'s C-ABI loader and
//! `renzora_native_plugin`'s directory loader — so the Settings UI can list
//! every installed plugin without re-deriving either loader's rules.
//!
//! That last part is the reason this exists rather than the settings panel
//! doing its own `read_dir`. "Is this a plugin?" is a real question with a
//! non-obvious answer on both sides: a C-ABI plugin is a library file that
//! exports one specific symbol and is not a proc-macro dylib, a native plugin is
//! a directory containing `src/lib.rs`, and both loaders skip entries for
//! reasons — wrong scope, statically linked already, ABI too old — that a
//! second implementation would silently disagree about. A panel that lists a
//! *different* set from the one the engine loaded is worse than no panel, and
//! the same mistake has been made here before with script extensions.
//!
//! # Enabling and disabling
//!
//! [`load_disabled_plugins`](crate::load_disabled_plugins) is the persisted
//! list, keyed by [`PluginEntry::id`], and both loaders consult it before
//! loading anything. Disabling **cannot** take effect until the next launch, and
//! that is structural rather than unfinished: a plugin adds systems, resources
//! and function pointers to the `App` while it is being assembled, and Bevy has
//! no way to withdraw those. Unmapping the image is worse still — a retired
//! system is still *in* the schedule, merely returning early.
//!
//! So the toggle records intent, and the loader acts on it at startup. The UI
//! says so plainly instead of pretending otherwise.

use bevy::prelude::*;

/// Which mechanism loaded (or would have loaded) a plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginKind {
    /// A `dylib` directory shipped as source and compiled against the SDK. Full
    /// `&mut World`; editor only.
    Native,
    /// A `cdylib` that links no Bevy and reaches the engine through a function
    /// table. Runs in a shipped game.
    Standalone,
}

impl PluginKind {
    pub fn label(self) -> &'static str {
        match self {
            PluginKind::Native => "Native",
            PluginKind::Standalone => "Standalone",
        }
    }
}

/// What happened to one plugin this launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginState {
    /// Installed into the `App`.
    Loaded,
    /// Turned off by the user; the loader did not open the file at all.
    Disabled,
    /// The loader declined it for a reason of its own — wrong scope for this
    /// process, already linked into the binary, an ABI it is too old for.
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
    /// The stable key: a native plugin's directory name, or a C-ABI plugin's
    /// library file stem with any `lib` prefix removed.
    ///
    /// This is what [`load_disabled_plugins`](crate::load_disabled_plugins)
    /// stores, so it has to be the same string on every platform — which is why
    /// the `lib` prefix is stripped rather than kept: the identical plugin is
    /// `grayscale.dll` on Windows and `libgrayscale.so` on Linux, and a
    /// preference file that survives moving between them should not disagree
    /// about which plugin was turned off.
    pub id: String,
    pub kind: PluginKind,
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
    pub fn record(&mut self, id: impl Into<String>, kind: PluginKind, state: PluginState) {
        let id = id.into();
        self.entries.retain(|e| !(e.id == id && e.kind == kind));
        self.entries.push(PluginEntry { id, kind, state });
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
/// A free function because both loaders run during `App` assembly, where the
/// resource may not exist yet and neither loader should have to care which of
/// them got there first.
pub fn record_plugin(
    world: &mut World,
    id: impl Into<String>,
    kind: PluginKind,
    state: PluginState,
) {
    world
        .get_resource_or_insert_with(PluginInventory::default)
        .record(id, kind, state);
}

/// The id a C-ABI plugin file is known by.
///
/// `lib` is stripped because a cdylib is `lib<crate>.so` on Unix and
/// `<crate>.dll` on Windows, and the persisted disable list must mean the same
/// thing on both.
pub fn plugin_id_from_path(path: &std::path::Path) -> String {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    stem.strip_prefix("lib").unwrap_or(&stem).to_string()
}

/// Where a plugin's store artwork lives: `<exe>/plugins/<id>/thumbnail.jpg`.
///
/// Here rather than in either panel because two of them need it — Settings →
/// Plugins and the exporter's plugin picker — and a thumbnail that showed up in
/// one place and not the other would look like a broken image rather than a
/// disagreement about the path.
///
/// Only a **native** plugin has somewhere to put one. A C-ABI plugin stages as a
/// loose library file with no directory beside it, so this resolves to a path
/// that does not exist and the caller draws its placeholder. That is the honest
/// outcome, not a gap to paper over: the file genuinely has nowhere to live yet.
///
/// `None` when the executable's own directory cannot be determined, which is the
/// same condition under which no plugins would have loaded either.
pub fn plugin_thumbnail_path(id: &str) -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    Some(dir.join("plugins").join(id).join("thumbnail.jpg"))
}
