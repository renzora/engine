//! First-run + per-chapter tracking, persisted per-project in `project.toml`
//! under `[editor]` (the editor-only `EditorPrefs` bag — runtime ignores it,
//! export strips it).
//!
//! Two separate facts are recorded, because they answer different questions:
//! `tutorial_completed` gates the **auto-launch** (set once the user has engaged
//! with the tutorial at all, including by skipping it — nobody wants it
//! re-appearing every time they open the project), while `tutorial_chapters` is
//! the list of chapter ids actually *finished*, which the picker ticks off.

use renzora::core::viewport_types::EditorPrefs;
use renzora::core::CurrentProject;

/// Has the tutorial NOT yet been engaged with for this project? `true` the first
/// time the editor opens a project, which is when we auto-launch.
pub fn is_first_run(project: &CurrentProject) -> bool {
    !project
        .config
        .editor
        .as_ref()
        .map(|e| e.tutorial_completed)
        .unwrap_or(false)
}

/// Record that the tutorial has been engaged with (a chapter finished, or the
/// whole thing skipped) so it never auto-launches for this project again.
/// Updates the live resource too, so a second trigger in the same session sees
/// the new value.
pub fn mark_completed(project: &mut CurrentProject) {
    project
        .config
        .editor
        .get_or_insert_with(EditorPrefs::default)
        .tutorial_completed = true;
    save(project);
}

/// Record that `chapter_id` was finished. Idempotent — finishing a chapter twice
/// doesn't duplicate the entry.
pub fn mark_chapter_done(project: &mut CurrentProject, chapter_id: &str) {
    let prefs = project.config.editor.get_or_insert_with(EditorPrefs::default);
    if prefs.tutorial_chapters.iter().any(|c| c == chapter_id) {
        return;
    }
    prefs.tutorial_chapters.push(chapter_id.to_string());
    save(project);
}

/// Which chapters this project has finished.
pub fn is_chapter_done(project: &CurrentProject, chapter_id: &str) -> bool {
    project
        .config
        .editor
        .as_ref()
        .is_some_and(|e| e.tutorial_chapters.iter().any(|c| c == chapter_id))
}

fn save(project: &mut CurrentProject) {
    if let Err(e) = project.save_config() {
        bevy::log::warn!("[tutorial] failed to persist progress to project.toml: {e}");
    }
}
