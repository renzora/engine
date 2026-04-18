//! Persist viewport header settings into `project.toml` under `[editor]`.
//!
//! These are editor-only fields. The runtime ignores them and export strips
//! them from shipped builds. See `PersistedViewportSettings` and
//! `EditorPrefs` in `renzora::core::viewport_types`.

use bevy::prelude::*;

use renzora::core::viewport_types::{EditorPrefs, PersistedViewportSettings, ViewportSettings};
use renzora::core::CurrentProject;

/// Applies `project.toml` editor prefs to `ViewportSettings` whenever a new
/// `CurrentProject` is inserted (project load / project switch).
pub fn apply_prefs_on_project_load(
    project: Option<Res<CurrentProject>>,
    mut settings: ResMut<ViewportSettings>,
    mut last_applied: Local<Option<std::path::PathBuf>>,
) {
    let Some(project) = project else { return };
    if last_applied.as_ref() == Some(&project.path) { return }
    if let Some(prefs) = &project.config.editor {
        prefs.viewport.apply(&mut settings);
    }
    *last_applied = Some(project.path.clone());
}

/// Debounced save: when `ViewportSettings` changes, mirror into
/// `CurrentProject.config.editor.viewport` and rewrite `project.toml`.
pub fn save_on_change(
    mut project: Option<ResMut<CurrentProject>>,
    settings: Res<ViewportSettings>,
    mut last_save: Local<f64>,
    time: Res<Time>,
) {
    if !settings.is_changed() { return }
    let Some(project) = project.as_mut() else { return };

    let now = time.elapsed_secs_f64();
    if *last_save != 0.0 && now - *last_save < 0.75 { return }

    let persisted = PersistedViewportSettings::from_settings(&settings);

    // Read-only compare first. Only if the persisted snapshot actually
    // differs do we call DerefMut (which would mark `CurrentProject` as
    // changed and cascade into `sync_project_asset_path` log spam).
    let needs_save = match &project.as_ref().config.editor {
        Some(prefs) => prefs.viewport != persisted,
        None => persisted != PersistedViewportSettings::default(),
    };
    if !needs_save { return }

    *last_save = now;

    let prefs = project.config.editor.get_or_insert_with(EditorPrefs::default);
    prefs.viewport = persisted;

    if let Err(e) = project.save_config() {
        warn!("[viewport] couldn't save project.toml: {e}");
    }
}
