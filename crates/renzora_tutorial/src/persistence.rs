//! First-run tracking, persisted per-project in `project.toml` under
//! `[editor].tutorial_completed` (the editor-only `EditorPrefs` bag — runtime
//! ignores it, export strips it).

use renzora::core::viewport_types::EditorPrefs;
use renzora::core::CurrentProject;

/// Has the tutorial NOT yet been completed/skipped for this project? `true` the
/// first time the editor opens a project, which is when we auto-launch.
pub fn is_first_run(project: &CurrentProject) -> bool {
    !project
        .config
        .editor
        .as_ref()
        .map(|e| e.tutorial_completed)
        .unwrap_or(false)
}

/// Record that the tutorial is finished (completed or skipped) and write it to
/// disk so it never auto-launches for this project again. Updates the live
/// resource too so a second trigger in the same session sees the new value.
pub fn mark_completed(project: &mut CurrentProject) {
    project
        .config
        .editor
        .get_or_insert_with(EditorPrefs::default)
        .tutorial_completed = true;
    if let Err(e) = project.save_config() {
        bevy::log::warn!("[tutorial] failed to persist completion to project.toml: {e}");
    }
}
