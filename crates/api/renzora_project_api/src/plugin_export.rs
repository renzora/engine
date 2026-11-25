//! What a plugin tells the exporter about itself.
//!
//! The exporter knows how to ship a plugin's library. It cannot know what else
//! a plugin needs in a game, or when a project needs the plugin at all, because
//! plugins come from the marketplace and the engine names none of them. So a
//! plugin says so itself:
//!
//! - **Script extensions.** A project with files of these extensions is scripted
//!   in this plugin's language: the export ticks the plugin and keeps the
//!   scripting layer. Without this, a project scripted only in a plugin's
//!   language exported a game with neither, and its scripts silently did
//!   nothing.
//! - **A stage function.** Run in the editor when an export starts, it lists the
//!   files the plugin needs in the game beside its library: a compiled build of
//!   the project's scripts, a runtime, data. They are written to the plugin's
//!   own folder in the game, `plugins/<id>/`, where the plugin finds them with
//!   `renzora::plugin_dir`. That folder exists in every desktop packaging mode,
//!   lean included: a plugin linked into a lean binary still gets it.
//!
//! ```ignore
//! renzora::register_plugin_export(renzora::PluginExport {
//!     plugin: "csharp".into(),
//!     script_extensions: vec!["cs".into()],
//!     stage: Some(stage_for_export),
//! });
//! ```
//!
//! The stage function runs in the editor, not in the export worker: the worker
//! may be the companion process, which loads no plugins. What it returns is
//! plain data (paths and bytes) and travels to the worker with the rest of the
//! job. A static registry, like the import formats, so a plugin and the export
//! dialog see one list through the shared `renzora_dylib` image.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// The export a stage function is asked about.
#[derive(Clone, Debug)]
pub struct PluginExportTarget<'a> {
    pub project_root: &'a Path,
    /// The platform's folder name, as the engine's own builds use it:
    /// `windows-x64`, `linux-x64`, `macos-arm64`, `web-wasm32`, ...
    pub platform: &'a str,
    /// Whether the plugin is compiled into a lean binary rather than shipped
    /// as a library. Its folder in the game exists either way.
    pub linked_in: bool,
    /// The plugin's [`PluginExportOption`]s as the user set them for this
    /// export, every one present (its default when the user left it alone).
    pub options: &'a HashMap<String, bool>,
}

impl PluginExportTarget<'_> {
    /// One of the plugin's options; `false` for an id it never declared.
    pub fn option(&self, id: &str) -> bool {
        self.options.get(id).copied().unwrap_or(false)
    }
}

/// A switch a plugin adds to the Export window, under its card in the Plugins
/// tab. Its value reaches the plugin's stage function, and is saved with the
/// export preset.
#[derive(Clone, Debug, Default)]
pub struct PluginExportOption {
    /// Stable, unique within the plugin: it is what presets save.
    pub id: String,
    pub label: String,
    /// One or two sentences under the label: what changes for the player.
    pub help: String,
    pub default: bool,
}

/// The key an option's value is kept under, across every plugin.
pub fn plugin_option_key(plugin: &str, option: &str) -> String {
    format!("{plugin}/{option}")
}

/// Where one shipped file's contents come from.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PluginFileSource {
    /// A file on this machine, copied.
    Copy(PathBuf),
    /// Contents made by the stage function, such as a manifest.
    Bytes(Vec<u8>),
}

/// One file a plugin ships with the game.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginFile {
    /// Relative to the plugin's place in the game, with `/` separators:
    /// `export/scripts/Game.dll` lands at `plugins/<id>/export/scripts/Game.dll`
    /// beside the game, or at [`plugin_archive_path`] in its archive.
    pub path: String,
    pub source: PluginFileSource,
    /// Packed into the game's archive instead of written beside the game, so a
    /// single-file export stays one file. Read back at run time with
    /// `game_archive_read(&plugin_archive_path(id, path))`.
    #[serde(default)]
    pub in_archive: bool,
}

/// Where a plugin's archived file lives in the game's archive. Under
/// `.renzora/`, which the export never packs from the project, so no project
/// file can collide with it.
pub fn plugin_archive_path(plugin: &str, rel: &str) -> String {
    format!(".renzora/plugins/{plugin}/{}", rel.trim_start_matches('/'))
}

/// What a stage function hands the exporter.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PluginExportFiles {
    pub files: Vec<PluginFile>,
    /// Lines for the export log: what was shipped, what the player will need.
    pub notes: Vec<String>,
}

impl PluginExportFiles {
    /// Ship `source` at `path`, beside the game.
    pub fn copy(&mut self, path: impl Into<String>, source: impl Into<PathBuf>) -> &mut Self {
        self.files.push(PluginFile {
            path: path.into(),
            source: PluginFileSource::Copy(source.into()),
            in_archive: false,
        });
        self
    }

    /// Ship `bytes` at `path`, beside the game.
    pub fn bytes(&mut self, path: impl Into<String>, bytes: impl Into<Vec<u8>>) -> &mut Self {
        self.files.push(PluginFile {
            path: path.into(),
            source: PluginFileSource::Bytes(bytes.into()),
            in_archive: false,
        });
        self
    }

    /// Pack `source` into the game's archive at `path`.
    pub fn archive_copy(&mut self, path: impl Into<String>, source: impl Into<PathBuf>) -> &mut Self {
        self.files.push(PluginFile {
            path: path.into(),
            source: PluginFileSource::Copy(source.into()),
            in_archive: true,
        });
        self
    }

    /// Pack `bytes` into the game's archive at `path`.
    pub fn archive_bytes(&mut self, path: impl Into<String>, bytes: impl Into<Vec<u8>>) -> &mut Self {
        self.files.push(PluginFile {
            path: path.into(),
            source: PluginFileSource::Bytes(bytes.into()),
            in_archive: true,
        });
        self
    }

    /// Ship every file under `dir` at `path/<its path under dir>`.
    pub fn copy_dir(&mut self, path: &str, dir: &Path) -> std::io::Result<&mut Self> {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
            for entry in std::fs::read_dir(dir)? {
                let p = entry?.path();
                if p.is_dir() {
                    walk(&p, out)?;
                } else {
                    out.push(p);
                }
            }
            Ok(())
        }
        let mut found = Vec::new();
        walk(dir, &mut found)?;
        found.sort();
        let prefix = path.trim_end_matches('/');
        for f in found {
            let rel = f.strip_prefix(dir).unwrap_or(&f).to_string_lossy().replace('\\', "/");
            let at = if prefix.is_empty() { rel } else { format!("{prefix}/{rel}") };
            self.copy(at, f);
        }
        Ok(self)
    }
}

/// Lists the files a plugin ships, or says why the export cannot go ahead
/// (a script that does not compile, say). Runs on the editor's main thread
/// when the export starts, so it should reuse work already done rather than
/// start long builds.
pub type PluginStageFn = fn(&PluginExportTarget) -> Result<PluginExportFiles, String>;

/// One plugin's declaration. `..Default::default()` keeps a declaration
/// building when a later engine adds a field.
#[derive(Clone, Debug, Default)]
pub struct PluginExport {
    /// The plugin's id: the name of its folder under `plugins/`.
    pub plugin: String,
    /// Script file extensions it runs, lowercase, without the dot.
    pub script_extensions: Vec<String>,
    pub stage: Option<PluginStageFn>,
    /// Switches shown in the Export window, passed to `stage`.
    pub options: Vec<PluginExportOption>,
}

impl PluginExport {
    /// This plugin's option values out of the export's saved ones (keyed by
    /// [`plugin_option_key`]), with each default filled in.
    pub fn option_values(&self, saved: &HashMap<String, bool>) -> HashMap<String, bool> {
        self.options
            .iter()
            .map(|o| {
                let v = saved.get(&plugin_option_key(&self.plugin, &o.id)).copied().unwrap_or(o.default);
                (o.id.clone(), v)
            })
            .collect()
    }
}

static EXPORTS: RwLock<Vec<PluginExport>> = RwLock::new(Vec::new());

/// Declare what a plugin needs from the exporter. A second declaration for
/// the same plugin replaces the first, so a plugin rebuilt and reloaded does
/// not list itself twice.
pub fn register_plugin_export(export: PluginExport) {
    if let Ok(mut all) = EXPORTS.write() {
        all.retain(|e| e.plugin != export.plugin);
        all.push(export);
    }
}

/// Every declaration, in registration order.
pub fn plugin_exports() -> Vec<PluginExport> {
    EXPORTS.read().map(|e| e.clone()).unwrap_or_default()
}

/// Every script extension any plugin runs.
pub fn plugin_script_extensions() -> Vec<String> {
    let mut out: Vec<String> = plugin_exports().into_iter().flat_map(|e| e.script_extensions).collect();
    out.sort();
    out.dedup();
    out
}

/// The plugin folder a relative path may be written under, or `None` for one
/// that would land outside it (`..`, an absolute path, a drive prefix). The
/// exporter writes nothing it cannot place inside the plugin's own folder.
pub fn plugin_file_path(plugin_dir: &Path, rel: &str) -> Option<PathBuf> {
    let rel = rel.replace('\\', "/");
    if rel.is_empty() || rel.starts_with('/') {
        return None;
    }
    let mut out = plugin_dir.to_path_buf();
    for part in rel.split('/') {
        match part {
            "" | "." => continue,
            ".." => return None,
            p if p.contains(':') => return None,
            p => out.push(p),
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stage(_: &PluginExportTarget) -> Result<PluginExportFiles, String> {
        Ok(PluginExportFiles::default())
    }

    #[test]
    fn a_plugin_declares_once_and_its_extensions_are_listed() {
        register_plugin_export(PluginExport {
            plugin: "test_lang".into(),
            script_extensions: vec!["tstl".into()],
            ..Default::default()
        });
        register_plugin_export(PluginExport {
            plugin: "test_lang".into(),
            script_extensions: vec!["tstl".into(), "tstl2".into()],
            stage: Some(stage),
            ..Default::default()
        });
        let mine: Vec<_> = plugin_exports().into_iter().filter(|e| e.plugin == "test_lang").collect();
        assert_eq!(mine.len(), 1);
        assert!(mine[0].stage.is_some());
        let exts = plugin_script_extensions();
        assert!(exts.contains(&"tstl".to_string()) && exts.contains(&"tstl2".to_string()));
    }

    #[test]
    fn an_option_is_its_default_until_saved() {
        let export = PluginExport {
            plugin: "p".into(),
            options: vec![
                PluginExportOption { id: "bundle".into(), default: true, ..Default::default() },
                PluginExportOption { id: "trim".into(), default: false, ..Default::default() },
            ],
            ..Default::default()
        };
        let mut saved = HashMap::new();
        let values = export.option_values(&saved);
        assert_eq!(values.get("bundle"), Some(&true));
        assert_eq!(values.get("trim"), Some(&false));
        saved.insert(plugin_option_key("p", "bundle"), false);
        // Another plugin's option of the same name is not this one's.
        saved.insert(plugin_option_key("q", "trim"), true);
        let values = export.option_values(&saved);
        assert_eq!(values.get("bundle"), Some(&false));
        assert_eq!(values.get("trim"), Some(&false));
    }

    #[test]
    fn a_file_cannot_leave_the_plugin_folder() {
        let dir = Path::new("game/plugins/x");
        assert_eq!(plugin_file_path(dir, "export/a.dll"), Some(dir.join("export").join("a.dll")));
        assert_eq!(plugin_file_path(dir, "a\\b.txt"), Some(dir.join("a").join("b.txt")));
        assert!(plugin_file_path(dir, "../evil.dll").is_none());
        assert!(plugin_file_path(dir, "export/../../evil.dll").is_none());
        assert!(plugin_file_path(dir, "/abs").is_none());
        assert!(plugin_file_path(dir, "C:/abs").is_none());
        assert!(plugin_file_path(dir, "").is_none());
    }
}
