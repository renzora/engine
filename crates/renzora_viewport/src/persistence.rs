//! Persist viewport settings into `~/.renzora/settings.toml` under `[viewport]`.
//!
//! **These are per-user, not per-project.** They used to live in each
//! `project.toml` under `[editor].viewport`, which put camera sensitivity — how
//! fast the view turns under *your* hand, on *your* mouse — into a file the game
//! ships with, and made it something you had to set again in every project you
//! opened. It is a property of the person, like the UI scale beside it.
//!
//! That also empties the last editor-only table out of `project.toml`, so
//! `renzora_export` has nothing left to strip from a shipped build.
//!
//! See [`PersistedViewportSettings`] for what is saved and what is deliberately
//! left as session state.

use bevy::prelude::*;

use renzora::core::settings_file;
use renzora::core::viewport_types::{PersistedViewportSettings, ViewportSettings};
use renzora::core::CurrentProject;

/// The section name in `settings.toml`.
const SECTION: &str = "viewport";

/// Load the saved viewport settings once, at startup.
///
/// Startup rather than on project load, which is when they used to be applied:
/// they are no longer the project's, so there is nothing about opening one that
/// should change them.
pub fn apply_saved_settings(mut settings: ResMut<ViewportSettings>) {
    if let Some(saved) = settings_file::load_section::<PersistedViewportSettings>(SECTION) {
        saved.apply(&mut settings);
    }
}

/// Fold a project's old `[editor].viewport` table into the per-user section,
/// once, the first time that project is opened after the move.
///
/// Only when the user has no `[viewport]` section yet: the first project opened
/// donates its settings and every later one leaves them alone. Taking the last
/// project opened instead would mean your sensitivity silently changing every
/// time you switched projects, which is the behaviour being removed.
///
/// The table is left in the project's `project.toml`. Rewriting every project a
/// user opens, to delete two lines the loader now ignores, is a lot of writes to
/// files under version control for no gain — and `ProjectConfig` no longer
/// deserializes it, so it is inert either way.
pub fn migrate_project_prefs(
    project: Option<Res<CurrentProject>>,
    mut settings: ResMut<ViewportSettings>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let Some(project) = project else { return };
    *done = true;
    if settings_file::load_section::<PersistedViewportSettings>(SECTION).is_some() {
        return;
    }
    let Some(legacy) = legacy_viewport_prefs(&project.path) else {
        return;
    };
    legacy.apply(&mut settings);
    if let Err(e) = settings_file::save_section(SECTION, &legacy) {
        warn!("[viewport] could not migrate viewport settings: {e}");
    } else {
        info!(
            "[viewport] migrated viewport settings out of {}",
            project.path.join("project.toml").display()
        );
    }
}

/// Read `[editor].viewport` straight out of a project's `project.toml`.
///
/// Parsed from the raw file rather than from `ProjectConfig`, which no longer
/// has the field: this is the one place that still needs to see the old shape,
/// and giving the config struct a field back just to migrate it would keep the
/// editor table alive in the type that defines what a shipped project is.
fn legacy_viewport_prefs(root: &std::path::Path) -> Option<PersistedViewportSettings> {
    let text = std::fs::read_to_string(root.join("project.toml")).ok()?;
    let table = text.parse::<toml::Table>().ok()?;
    table
        .get("editor")?
        .get("viewport")?
        .clone()
        .try_into::<PersistedViewportSettings>()
        .ok()
}

/// Debounced save: when `ViewportSettings` changes, write the `[viewport]`
/// section back.
///
/// Debounced because a sensitivity slider mutates the resource every frame of a
/// drag, and each write is a read-modify-write of the settings file.
pub fn save_on_change(
    settings: Res<ViewportSettings>,
    time: Res<Time>,
    mut last_save: Local<f64>,
    mut pending: Local<bool>,
) {
    if settings.is_changed() {
        *pending = true;
    }
    if !*pending {
        return;
    }
    let now = time.elapsed_secs_f64();
    if *last_save != 0.0 && now - *last_save < 0.75 {
        return;
    }
    *last_save = now;
    *pending = false;

    let persisted = PersistedViewportSettings::from_settings(&settings);
    // Compare against what is on disk before writing: the resource is marked
    // changed by plenty that this snapshot does not carry, and a write per frame
    // of camera motion would be a file write per frame.
    if settings_file::load_section::<PersistedViewportSettings>(SECTION).as_ref() == Some(&persisted)
    {
        return;
    }
    if let Err(e) = settings_file::save_section(SECTION, &persisted) {
        warn!("[viewport] couldn't save viewport settings: {e}");
    }
}
