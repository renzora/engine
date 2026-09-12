//! Asking which version of a scene wins when both the file and the editor moved.
//!
//! Raised by [`hot_reload`](super::hot_reload) under
//! [`ExternalSceneEdits::Prompt`], which is the shipped default. Until it is
//! answered **neither side is touched**: the file keeps what was written to it
//! and the world keeps what the user did, which is the only state that cannot
//! lose anything.
//!
//! # Why this is asked rather than decided
//!
//! Both edits are deliberate. Someone meant to save that file and someone meant
//! to move that entity, so "which was intentional" does not break the tie. What
//! does is that the version on disk survives being ignored and the version in
//! memory does not — but that argues for a safe *default*, not for never asking.
//! A project whose scenes are generated elsewhere wants the opposite, and says
//! so through the setting.
//!
//! Dismissing is deliberately not destructive in either direction: keeping the
//! editor's version leaves the file untouched, so saving still overwrites it and
//! reopening the scene still takes it.

use bevy::prelude::*;
use renzora::lang::t as tr;
use renzora_ember::font::EmberFonts;

use crate::hot_reload::{PendingSceneReloads, SceneConflicts};

/// The overlay, carrying the path it is asking about.
///
/// The path rather than an index: conflicts resolve out of order (a second file
/// can change while the first prompt is open), and an index into a `Vec` that
/// another system is draining is the classic way to answer the wrong question.
#[derive(Component)]
pub(crate) struct ConflictPromptRoot(std::path::PathBuf);

#[derive(Component)]
pub(crate) struct ConflictKeepMine;

#[derive(Component)]
pub(crate) struct ConflictTakeDisk;

/// Raise a prompt for the first unanswered conflict.
///
/// One at a time. Two overlays stacked on each other would be unreadable, and
/// the second conflict is still queued when the first is answered.
pub(crate) fn spawn_conflict_prompt(
    conflicts: Res<SceneConflicts>,
    open: Query<&ConflictPromptRoot>,
    fonts: Option<Res<EmberFonts>>,
    project: Option<Res<renzora::core::CurrentProject>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    if !open.is_empty() {
        return;
    }
    let Some(path) = conflicts.pending.first().cloned() else {
        return;
    };

    let shown = project
        .and_then(|p| p.make_relative(&path))
        .unwrap_or_else(|| path.to_string_lossy().to_string());

    // "Reload from Disk" last, so it takes the accent: Escape, a backdrop click
    // and the title bar's × all keep the editor's version, which means the safe
    // answer is already reachable without aiming and the destructive one should
    // have to be clicked. See `confirm_dialog`.
    let (root, buttons) = renzora_ember::widgets::confirm_dialog(
        &mut commands,
        &fonts,
        &tr("scene.conflict.title"),
        format!(
            "{shown} was changed outside the editor, and you have unsaved changes \
             to it here.\n\nReloading discards your unsaved changes. Keeping yours \
             leaves the file alone."
        ),
        460.0,
        196.0,
        &[&tr("scene.conflict.keep"), &tr("scene.conflict.reload")],
    );

    if let [keep, take] = buttons[..] {
        commands.entity(keep).insert(ConflictKeepMine);
        commands.entity(take).insert(ConflictTakeDisk);
    }
    commands.entity(root).insert(ConflictPromptRoot(path));
}

/// Drive the buttons.
///
/// Escape, a backdrop click and the title bar's × are handled by ember's
/// generic `overlay_dismiss`, which despawns the root without touching
/// `SceneConflicts` — so a dismissed prompt comes back. That is deliberate: an
/// unanswered conflict is still a conflict, and silently forgetting it would
/// leave the editor showing one scene while the file held another with nothing
/// on screen to say so.
pub(crate) fn conflict_prompt_buttons(
    keep: Query<&Interaction, (Changed<Interaction>, With<ConflictKeepMine>)>,
    take: Query<&Interaction, (Changed<Interaction>, With<ConflictTakeDisk>)>,
    roots: Query<(Entity, &ConflictPromptRoot)>,
    mut conflicts: ResMut<SceneConflicts>,
    mut reloads: ResMut<PendingSceneReloads>,
    mut commands: Commands,
) {
    let keep = keep.iter().any(|i| *i == Interaction::Pressed);
    let take = take.iter().any(|i| *i == Interaction::Pressed);
    if !keep && !take {
        return;
    }
    for (entity, root) in roots.iter() {
        if take {
            conflicts.resolve_with_disk(&root.0, &mut reloads);
        } else {
            conflicts.dismiss(&root.0);
        }
        commands.entity(entity).try_despawn();
    }
}
