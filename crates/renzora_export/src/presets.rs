//! Named export configurations, and where they are kept.
//!
//! The export modal used to show every platform in one flat list and hold the
//! settings for whichever was selected in memory — so the settings were gone at
//! the next launch, and there was no way to keep two shipping configurations
//! side by side. A preset is that configuration given a name: platform,
//! packaging, window, plugins, capabilities, the lot.
//!
//! ## Where they live, and why that is a compromise
//!
//! `~/.renzora/export_presets.toml`, beside `editor.toml` and
//! `renderer.toml` — machine-local, like the rest of that directory.
//!
//! Presets are not really machine-local, though: an output path and a stripped
//! feature set describe the *game*, not the monitor it was configured on. So
//! this file is keyed by project directory rather than being one global list,
//! which keeps two projects from overwriting each other's presets. What it
//! cannot do is travel — a teammate cloning the repository starts with none, and
//! moving a project directory orphans its entry. If sharing them ever matters,
//! the fix is to move the file into the project; nothing here depends on its
//! location beyond [`presets_path`].

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use bevy::prelude::warn;
use renzora::core::WindowMode;
use serde::{Deserialize, Serialize};

use crate::overlay::{ExportOverlayState, PackagingMode, PluginLinkMode};
use crate::templates::Platform;

/// One named export configuration.
///
/// Every field here is a *setting*. The overlay's transient state — the build
/// log, the running task, the scanned plugin list, which sections are folded —
/// is deliberately absent: a preset describes what to build, not what a
/// particular build is doing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportPreset {
    pub name: String,
    pub platform: Platform,
    pub packaging_mode: PackagingMode,

    #[serde(default)]
    pub window_mode: WindowMode,
    #[serde(default = "default_width")]
    pub window_width: u32,
    #[serde(default = "default_height")]
    pub window_height: u32,
    #[serde(default)]
    pub console_logging: bool,
    #[serde(default = "default_compression")]
    pub compression_level: i32,
    #[serde(default)]
    pub upx_compress: bool,
    #[serde(default)]
    pub icon_path: Option<String>,
    #[serde(default)]
    pub include_server: bool,
    /// Ship the plugin SDK so the game can be modded. Defaults to ON, including
    /// for a preset written before this existed — a preset saved when there was
    /// no choice was saved under the behaviour that shipped it.
    #[serde(default = "default_true")]
    pub enable_modding: bool,
    #[serde(default)]
    pub binary_name: String,
    #[serde(default)]
    pub output_dir: String,

    #[serde(default)]
    pub mesh_simplify: bool,
    #[serde(default = "default_simplify_ratio")]
    pub mesh_simplify_ratio: f32,
    #[serde(default)]
    pub mesh_quantize: bool,
    #[serde(default)]
    pub mesh_generate_lods: bool,
    #[serde(default = "default_lod_levels")]
    pub mesh_lod_levels: u32,

    #[serde(default)]
    pub selected_plugins: HashSet<String>,
    #[serde(default)]
    pub plugin_link_mode: PluginLinkMode,
    /// `#[serde(default)]`, so a preset written before bundling existed reads
    /// back as "no bundle" rather than failing to parse.
    #[serde(default)]
    pub bundle_app: bool,
    /// Engine capability toggles (id → on). Only the ids that differ from the
    /// default would be enough, but the full map is written so a preset keeps
    /// meaning the same thing when a later engine adds a capability that
    /// defaults on.
    #[serde(default)]
    pub capabilities: HashMap<String, bool>,
}

fn default_width() -> u32 {
    1280
}
fn default_height() -> u32 {
    720
}
fn default_compression() -> i32 {
    3
}
fn default_simplify_ratio() -> f32 {
    0.5
}
fn default_lod_levels() -> u32 {
    3
}
fn default_true() -> bool {
    true
}

impl ExportPreset {
    /// A new preset for `platform`, with everything else left at its default.
    pub fn new(name: impl Into<String>, platform: Platform) -> Self {
        Self {
            name: name.into(),
            platform,
            packaging_mode: PackagingMode::SeparateFiles,
            window_mode: WindowMode::default(),
            window_width: default_width(),
            window_height: default_height(),
            console_logging: false,
            compression_level: default_compression(),
            upx_compress: false,
            icon_path: None,
            include_server: false,
            enable_modding: true,
            binary_name: String::new(),
            output_dir: String::new(),
            mesh_simplify: false,
            mesh_simplify_ratio: default_simplify_ratio(),
            mesh_quantize: false,
            mesh_generate_lods: false,
            mesh_lod_levels: default_lod_levels(),
            selected_plugins: HashSet::new(),
            bundle_app: false,
            plugin_link_mode: PluginLinkMode::default(),
            capabilities: HashMap::new(),
        }
    }

    /// Snapshot the overlay's current settings under `name`.
    pub fn capture(name: impl Into<String>, state: &ExportOverlayState) -> Self {
        Self {
            name: name.into(),
            platform: state.platform,
            packaging_mode: state.packaging_mode,
            window_mode: state.window_mode,
            window_width: state.window_width,
            window_height: state.window_height,
            console_logging: state.console_logging,
            compression_level: state.compression_level,
            upx_compress: state.upx_compress,
            icon_path: state.icon_path.clone(),
            include_server: state.include_server,
            enable_modding: state.enable_modding,
            binary_name: state.binary_name.clone(),
            output_dir: state.output_dir.clone(),
            mesh_simplify: state.mesh_simplify,
            mesh_simplify_ratio: state.mesh_simplify_ratio,
            mesh_quantize: state.mesh_quantize,
            mesh_generate_lods: state.mesh_generate_lods,
            mesh_lod_levels: state.mesh_lod_levels,
            selected_plugins: state.selected_plugins.clone(),
            bundle_app: state.bundle_app,
            plugin_link_mode: state.plugin_link_mode,
            capabilities: state.capabilities.clone(),
        }
    }

    /// Push this preset's settings into the overlay.
    ///
    /// Leaves the transient half alone — the build log, the active task, the
    /// scanned plugin list. `plugins_scanned_for` is cleared when the platform
    /// changes so the next frame rescans: the available plugins are per-platform
    /// (a Linux template brings `.so` plugins, and offering the editor's `.dll`s
    /// while exporting for Linux would list libraries the game cannot load).
    pub fn apply(&self, state: &mut ExportOverlayState) {
        self.apply_inner(state, true);
    }

    /// Apply without claiming the user chose it.
    ///
    /// The modal auto-selects the first preset when it loads a project's list,
    /// which is a starting point rather than a decision — so it must not pin the
    /// feature toggles and stop the project scan from answering. The difference
    /// is load-bearing: a preset's `capabilities` map is captured automatically
    /// by [`ExportOverlayState::sync_active_preset`] on every close and every
    /// export, so it holds the last state rather than an intention, and letting
    /// it win meant every project that had ever been exported was pinned to
    /// whatever that map happened to say.
    pub fn apply_as_default(&self, state: &mut ExportOverlayState) {
        self.apply_inner(state, false);
    }

    fn apply_inner(&self, state: &mut ExportOverlayState, pin: bool) {
        if state.platform != self.platform {
            state.plugins_scanned = false;
            state.plugins_scanned_for = None;
        }
        state.platform = self.platform;
        state.packaging_mode = self.packaging_mode;
        state.window_mode = self.window_mode;
        state.window_width = self.window_width;
        state.window_height = self.window_height;
        state.console_logging = self.console_logging;
        state.compression_level = self.compression_level;
        state.upx_compress = self.upx_compress;
        state.icon_path = self.icon_path.clone();
        state.include_server = self.include_server;
        state.enable_modding = self.enable_modding;
        state.binary_name = self.binary_name.clone();
        state.output_dir = self.output_dir.clone();
        state.mesh_simplify = self.mesh_simplify;
        state.mesh_simplify_ratio = self.mesh_simplify_ratio;
        state.mesh_quantize = self.mesh_quantize;
        state.mesh_generate_lods = self.mesh_generate_lods;
        state.mesh_lod_levels = self.mesh_lod_levels;
        state.selected_plugins = self.selected_plugins.clone();
        state.bundle_app = self.bundle_app;
        state.plugin_link_mode = self.plugin_link_mode;
        // Merge rather than replace: a capability added by a newer engine is
        // absent from an older preset, and dropping it would silently strip a
        // feature the user never chose to turn off.
        for (id, on) in &self.capabilities {
            state.capabilities.insert(id.clone(), *on);
        }
        // A preset the user deliberately switched to is their answer to the
        // feature question, so the platform re-scan above must not re-derive it
        // from the project a frame later. See `apply_as_default` for the case
        // where it is not an answer at all.
        state.choices_pinned = pin;
    }
}

/// The whole file: project directory → that project's presets.
#[derive(Debug, Default, Serialize, Deserialize)]
struct PresetFile {
    #[serde(default, flatten)]
    projects: HashMap<String, ProjectPresets>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ProjectPresets {
    #[serde(default)]
    presets: Vec<ExportPreset>,
}

/// `~/.renzora/export_presets.toml`. `None` when there is no home directory to
/// resolve, which is the same condition under which `editor.toml` gives up.
pub fn presets_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)?;
    Some(home.join(".renzora").join("export_presets.toml"))
}

/// The key a project is stored under.
///
/// Lossy-to-string and slash-normalised so the same directory produces the same
/// key whichever way the path arrived — a Windows project reached once as
/// `C:\game` and once as `C:/game` should not end up with two sets of presets.
fn project_key(project_root: &Path) -> String {
    project_root.to_string_lossy().replace('\\', "/")
}

/// Load the presets for one project. An unreadable or malformed file yields an
/// empty list rather than an error: presets are a convenience, and refusing to
/// open the export modal because a hand-edited TOML has a typo would be worse
/// than starting from none.
pub fn load(project_root: &Path) -> Vec<ExportPreset> {
    let Some(path) = presets_path() else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let file: PresetFile = match toml::from_str(&text) {
        Ok(f) => f,
        Err(e) => {
            warn!("export presets at {} could not be read: {e}", path.display());
            return Vec::new();
        }
    };
    file.projects.get(&project_key(project_root)).map(|p| p.presets.clone()).unwrap_or_default()
}

/// Replace this project's presets, leaving every other project's alone.
pub fn save(project_root: &Path, presets: &[ExportPreset]) -> std::io::Result<()> {
    let Some(path) = presets_path() else {
        return Err(std::io::Error::other("no home directory to write export presets to"));
    };

    // Read-modify-write: the file holds every project, so serialising only this
    // one would delete the others.
    let mut file: PresetFile = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| toml::from_str(&t).ok())
        .unwrap_or_default();

    let key = project_key(project_root);
    if presets.is_empty() {
        file.projects.remove(&key);
    } else {
        file.projects.insert(key, ProjectPresets { presets: presets.to_vec() });
    }

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = toml::to_string_pretty(&file).map_err(std::io::Error::other)?;
    std::fs::write(&path, text)
}


/// A name not already taken, as `base`, `base 2`, `base 3`, … — used by both
/// "add" and "duplicate", so neither can produce two presets the UI cannot tell
/// apart.
pub fn unique_name(base: &str, existing: &[ExportPreset]) -> String {
    if !existing.iter().any(|p| p.name == base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|candidate| !existing.iter().any(|p| p.name == *candidate))
        .unwrap_or_else(|| base.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only a preset the user deliberately switched to may pin the feature
    /// toggles; the one the modal auto-selects on open may not.
    ///
    /// This is the whole reason a 2D project kept opening with the entire 3D
    /// pipeline ticked after detection shipped: the project had a preset, the
    /// modal selected it because it was first, that pinned the toggles, and the
    /// scan never got to answer. The map it pinned to had been captured
    /// automatically by `sync_active_preset`, so it was not a choice anyone made.
    #[test]
    fn only_a_deliberate_preset_switch_pins_the_feature_toggles() {
        let mut preset = ExportPreset::new("Windows (x64)", Platform::WindowsX64);
        preset.capabilities.insert("render_3d".into(), true);

        let mut state = ExportOverlayState::default();
        preset.apply_as_default(&mut state);
        assert!(
            !state.choices_pinned,
            "auto-selecting the first preset must leave the scan free to answer"
        );
        // It still applies — the pin is about authority, not about whether the
        // settings land.
        assert_eq!(state.capabilities.get("render_3d"), Some(&true));

        let mut state = ExportOverlayState::default();
        preset.apply(&mut state);
        assert!(state.choices_pinned, "a deliberate switch is an answer");
    }
}
