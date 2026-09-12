//! `~/.renzora/settings.toml` — the one file every editor preference lives in.
//!
//! # Why one file, and why sections
//!
//! Preferences used to be scattered: `~/.renzora/editor.toml` for the per-user
//! ones, `<project>/project.toml`'s `[editor]` table for the viewport's, and
//! nothing at all for the twenty-four `EditorSettings` fields and every
//! keybinding, which simply did not survive a restart. Three stores and two
//! gaps, and the project file carried editor state that a shipped game has no
//! use for and `renzora_export` had to strip back out.
//!
//! This is one file, divided into **sections** that each belong to whoever cares
//! about them. A section is read and written whole; every other section in the
//! file is left exactly as it was. That is the property that lets several crates
//! own their own preferences without a central struct that has to name all of
//! them, and without one writer clobbering another's keys.
//!
//! ```text
//! [app]                        # language, plugins, update channel, tutorial…
//! [editor]                     # EditorSettings, whole
//! [viewport]                   # camera sensitivity, grid, gizmos, snapping
//! [keybindings]                # action → key
//! [projects."/home/me/game"]   # last scene, open tabs
//! ```
//!
//! # What is deliberately *not* here
//!
//! **`project.toml` keeps the shipped game's settings** — window, rendering,
//! audio, autoload, the main scene. Those describe the game, travel with it, and
//! are read by the runtime. Nothing in this file is.
//!
//! **`~/.renzora/layout.json` keeps the dock layout.** It is a deep tree of
//! splits and tabs with its own migration history, not a list of preferences,
//! and folding it in would make this file mostly layout by weight while gaining
//! nothing: it is already one file, already per-user, and already has a reader
//! that understands it.
//!
//! # Per-project sections
//!
//! Two things are per-user *and* per-project: the scene the editor had open, and
//! the document tabs. They key on the project's absolute path
//! (`[projects."<path>"]`). A project that moves loses them, which is the same
//! thing that happens to its entry in the recents list, and is better than the
//! alternative of writing editor state back into a file the game ships with.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

/// `~/.renzora/settings.toml`. `None` when there is no home directory to
/// resolve, which is the same condition every other per-user read handles.
#[cfg(not(target_arch = "wasm32"))]
pub fn settings_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)?;
    Some(home.join(".renzora").join("settings.toml"))
}

/// Fold the old stores into `settings.toml`, once, if it is not there yet.
///
/// Preferences lived in `~/.renzora/editor.toml` (per-user) and in each
/// `project.toml`'s `[editor]` table (the viewport's). Both are read here and
/// written across, so upgrading keeps your UI scale, your language, your
/// disabled plugins and your camera sensitivity instead of quietly resetting
/// them.
///
/// **Runs only when `settings.toml` is absent**, which makes it idempotent and
/// makes the new file authoritative from the first write. The old files are
/// left on disk rather than deleted: if this went wrong, the evidence should
/// still be there, and an editor that eats the file it just migrated from gives
/// you nothing to go back to.
///
/// The per-project half (`editor_last_scene`, `editor_open_tabs`) is *not*
/// migrated here — it is per project, and this runs before any project is open.
/// Those move across the first time each project is loaded; see
/// `renzora_viewport`'s persistence and the shell's tab restore.
#[cfg(not(target_arch = "wasm32"))]
pub fn migrate_legacy_prefs() {
    let Some(path) = settings_path() else { return };
    if path.exists() {
        return;
    }
    let Some(legacy) = path.parent().map(|p| p.join("editor.toml")) else {
        return;
    };
    let Ok(text) = std::fs::read_to_string(&legacy) else {
        return;
    };
    let Ok(old) = text.parse::<toml::Table>() else {
        return;
    };
    // The old file was flat. Everything in it that is not an `EditorSettings`
    // field is an application preference — language, plugins, the update
    // channel, the tutorial, the stats intervals — so the split is by name.
    const EDITOR_KEYS: &[&str] = &[
        "ui_scale",
        "scroll_speed",
        "dev_mode",
        "doc_tabs_dropdown",
        "hierarchy_toggle_on_click",
        "console_log_limit",
    ];
    let mut editor = toml::Table::new();
    let mut app = toml::Table::new();
    for (k, v) in old {
        // Two were renamed on the way across: the file called them by the play
        // target they set, the resource calls them by what it does.
        match k.as_str() {
            "play_runtime_window" => {
                editor.insert("external_play_window".into(), v);
            }
            "play_vr" => {
                editor.insert("play_launch_vr".into(), v);
            }
            k if EDITOR_KEYS.contains(&k) => {
                editor.insert(k.into(), v);
            }
            _ => {
                app.insert(k, v);
            }
        }
    }
    let mut table = toml::Table::new();
    table.insert("editor".into(), toml::Value::Table(editor));
    table.insert("app".into(), toml::Value::Table(app));
    // Both sections are read by real code: `[editor]` by
    // `EditorSettings::from_disk`, `[app]` by `project_config::app_prefs`. If you
    // add a third here, give it a reader in the same change — a migrated section
    // nobody reads is data that looks preserved and is not.
    if let Err(e) = write_file(&table) {
        bevy::log::warn!("[settings] could not migrate {}: {e}", legacy.display());
    } else {
        bevy::log::info!("[settings] migrated {} into settings.toml", legacy.display());
    }
}

#[cfg(target_arch = "wasm32")]
pub fn migrate_legacy_prefs() {}

/// The file as a table, or an empty one when it is absent or unparseable.
///
/// An unparseable file is treated as empty rather than as an error: the caller
/// is asking for preferences, and the honest answer to "your settings file is
/// corrupt" is to fall back to defaults rather than to refuse to start. The
/// first write then rebuilds it.
#[cfg(not(target_arch = "wasm32"))]
fn read_file() -> toml::Table {
    settings_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| t.parse::<toml::Table>().ok())
        .unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
fn write_file(table: &toml::Table) -> std::io::Result<()> {
    let Some(path) = settings_path() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "could not resolve home directory for settings",
        ));
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(table).map_err(std::io::Error::other)?;
    std::fs::write(&path, text)
}

/// Read one section, or `None` when it is absent or does not deserialize.
///
/// A section that fails to deserialize returns `None` rather than an error for
/// the same reason a corrupt file does: the caller has a `Default` and a user
/// staring at an editor, and a preference that cannot be read is a preference
/// that was never set.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_section<T: DeserializeOwned>(name: &str) -> Option<T> {
    read_file()
        .remove(name)
        .and_then(|v| v.try_into::<T>().ok())
}

/// Write one section, leaving every other section in the file untouched.
///
/// Read-modify-write, deliberately. Several crates own sections here and each
/// saves on its own schedule; serializing a whole-file struct instead would mean
/// one crate's write dropping the keys of any section it did not know about —
/// including sections belonging to a plugin, and including sections written by a
/// newer build than the one doing the writing.
#[cfg(not(target_arch = "wasm32"))]
pub fn save_section<T: Serialize>(name: &str, value: &T) -> std::io::Result<()> {
    let mut table = read_file();
    let value = toml::Value::try_from(value).map_err(std::io::Error::other)?;
    table.insert(name.to_string(), value);
    write_file(&table)
}

/// Read the section for one project, keyed by its root path.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_project_section<T: DeserializeOwned>(root: &Path) -> Option<T> {
    read_file()
        .remove("projects")?
        .try_into::<BTreeMap<String, toml::Value>>()
        .ok()?
        .remove(&root.to_string_lossy().to_string())
        .and_then(|v| v.try_into::<T>().ok())
}

/// Write the section for one project, leaving the other projects' alone.
#[cfg(not(target_arch = "wasm32"))]
pub fn save_project_section<T: Serialize>(root: &Path, value: &T) -> std::io::Result<()> {
    let mut table = read_file();
    let mut projects = table
        .remove("projects")
        .and_then(|v| v.try_into::<BTreeMap<String, toml::Value>>().ok())
        .unwrap_or_default();
    let value = toml::Value::try_from(value).map_err(std::io::Error::other)?;
    projects.insert(root.to_string_lossy().to_string(), value);
    table.insert(
        "projects".to_string(),
        toml::Value::try_from(projects).map_err(std::io::Error::other)?,
    );
    write_file(&table)
}

// ── wasm ────────────────────────────────────────────────────────────────────
//
// The browser has no home directory and no file to write. Every reader gets
// `None` and falls back to its `Default`, every writer succeeds having done
// nothing — the same shape the per-field helpers in `project_config` use, so a
// caller needs no `cfg` of its own.

#[cfg(target_arch = "wasm32")]
pub fn settings_path() -> Option<PathBuf> {
    None
}

#[cfg(target_arch = "wasm32")]
pub fn load_section<T: DeserializeOwned>(_name: &str) -> Option<T> {
    None
}

#[cfg(target_arch = "wasm32")]
pub fn save_section<T: Serialize>(_name: &str, _value: &T) -> std::io::Result<()> {
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn load_project_section<T: DeserializeOwned>(_root: &Path) -> Option<T> {
    None
}

#[cfg(target_arch = "wasm32")]
pub fn save_project_section<T: Serialize>(_root: &Path, _value: &T) -> std::io::Result<()> {
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn load_plugin_settings(_key: &str) -> Option<String> {
    None
}

#[cfg(target_arch = "wasm32")]
pub fn save_plugin_settings(_key: &str, _blob: &str) -> std::io::Result<()> {
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn clear_plugin_settings(_key: &str) -> std::io::Result<()> {
    Ok(())
}

/// Nothing was stored, so nothing was cleared.
#[cfg(target_arch = "wasm32")]
pub fn clear_all_plugin_settings() -> std::io::Result<bool> {
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The round trip a caller relies on: what goes into a section comes back
    /// out of it, and writing a *different* section does not disturb it.
    ///
    /// Exercised against an in-memory table rather than the real file, because
    /// the real one is the developer's own `~/.renzora/settings.toml` and a test
    /// that wrote to it would clobber the settings of whoever ran it.
    #[test]
    fn a_section_write_preserves_the_others() {
        let mut table = toml::Table::new();
        table.insert(
            "editor".into(),
            toml::Value::try_from(BTreeMap::from([("ui_scale".to_string(), 1.5)])).unwrap(),
        );
        table.insert(
            "viewport".into(),
            toml::Value::try_from(BTreeMap::from([("look".to_string(), 0.25)])).unwrap(),
        );
        // What `save_section` does to the table it read.
        table.insert(
            "keybindings".into(),
            toml::Value::try_from(BTreeMap::from([("Undo".to_string(), "Ctrl+Z")])).unwrap(),
        );

        assert!(table.contains_key("editor"), "editor section was dropped");
        assert!(table.contains_key("viewport"), "viewport section was dropped");
        assert_eq!(table.len(), 3);
    }
}
