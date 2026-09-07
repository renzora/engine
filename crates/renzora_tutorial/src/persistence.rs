//! First-run + per-chapter tracking, persisted **per-user** in
//! `~/.renzora/editor.toml` alongside the other machine-local editor prefs.
//!
//! This used to live per-project in `project.toml`, and that was wrong: the
//! tutorial teaches the *editor*, not the project, so every new project
//! re-launched the onboarding overlay at a user who had already sat through it.
//! Keyed per-user, it auto-launches exactly once — on the first editor run after
//! installing the engine.
//!
//! Two separate facts are recorded, because they answer different questions:
//! `tutorial_completed` gates the **auto-launch** (set once the user has engaged
//! with the tutorial at all, including by skipping it), while
//! `tutorial_chapters` is the list of chapter ids actually *finished*, which the
//! picker ticks off and uses to unlock the next chapter.
//!
//! Projects written before the move still carry the old `[editor]` keys, so
//! [`is_first_run`] folds them into the per-user file the first time it sees
//! them — an existing user who already finished the tutorial doesn't get it
//! thrown at them again just because the answer moved house.

use renzora::core::project_config;
use renzora::core::CurrentProject;

/// Has the tutorial NOT yet been engaged with by this user? `true` only until
/// the first editor session that shows it, which is when we auto-launch.
///
/// `project` is consulted solely to migrate the pre-move per-project answer; it
/// no longer decides anything on its own.
pub fn is_first_run(project: &CurrentProject) -> bool {
    migrate_from_project(project);
    !project_config::load_tutorial_completed()
}

/// Fold a legacy per-project answer into the per-user file. Runs at most once
/// per project that still has the old keys — after the first call the per-user
/// file says "completed", so the `load_tutorial_completed` guard short-circuits.
fn migrate_from_project(project: &CurrentProject) {
    if project_config::load_tutorial_completed() {
        return;
    }
    let Some((completed, chapters)) = legacy_tutorial_prefs(&project.path) else {
        return;
    };
    if !completed {
        return;
    }
    for id in &chapters {
        mark_chapter_done(id);
    }
    mark_completed();
}

/// Read the old `[editor].tutorial_*` keys straight out of a project's
/// `project.toml`.
///
/// Parsed from the raw file because `ProjectConfig` no longer has an `editor`
/// table — that was the last editor-only thing in a file a game ships with, and
/// it went when the viewport settings did. Keeping a field on the config struct
/// purely so this migration could read it would have kept the table alive in the
/// type that defines what a project *is*.
#[cfg(not(target_arch = "wasm32"))]
fn legacy_tutorial_prefs(root: &std::path::Path) -> Option<(bool, Vec<String>)> {
    let text = std::fs::read_to_string(root.join("project.toml")).ok()?;
    let table = text.parse::<toml::Table>().ok()?;
    let editor = table.get("editor")?;
    let completed = editor
        .get("tutorial_completed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let chapters = editor
        .get("tutorial_chapters")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    Some((completed, chapters))
}

#[cfg(target_arch = "wasm32")]
fn legacy_tutorial_prefs(_root: &std::path::Path) -> Option<(bool, Vec<String>)> {
    None
}

/// Record that the tutorial has been engaged with (a chapter finished, or the
/// whole thing skipped) so it never auto-launches for this user again.
pub fn mark_completed() {
    #[cfg(not(target_arch = "wasm32"))]
    if let Err(e) = project_config::save_tutorial_completed(true) {
        bevy::log::warn!("[tutorial] failed to persist progress to editor.toml: {e}");
    }
}

/// Record that `chapter_id` was finished. Idempotent — finishing a chapter twice
/// doesn't duplicate the entry.
pub fn mark_chapter_done(chapter_id: &str) {
    let mut chapters = project_config::load_tutorial_chapters();
    if chapters.iter().any(|c| c == chapter_id) {
        return;
    }
    chapters.push(chapter_id.to_string());
    #[cfg(not(target_arch = "wasm32"))]
    if let Err(e) = project_config::save_tutorial_chapters(&chapters) {
        bevy::log::warn!("[tutorial] failed to persist progress to editor.toml: {e}");
    }
}

/// Which chapters this user has finished, in `CHAPTERS` order — one flag per
/// chapter, which is the shape the picker's tick/unlock logic wants. Reads the
/// prefs file once for the whole set rather than once per chapter.
pub fn chapters_done<'a>(ids: impl Iterator<Item = &'a str>) -> Vec<bool> {
    let done = project_config::load_tutorial_chapters();
    ids.map(|id| done.iter().any(|c| c == id)).collect()
}
