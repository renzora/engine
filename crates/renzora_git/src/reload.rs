//! Keeping the editor's open scene honest when git rewrites it on disk.
//!
//! This is the part of a git panel that has nothing to do with git, and the part
//! that does real damage if it is missing.
//!
//! Checking out a branch, pulling, merging, reverting, resetting, discarding a
//! file — every one of those can replace `scenes/level.bsn` on disk while the
//! editor is holding the *previous* version of that scene as live entities. Git
//! reports success, the panel updates, and nothing on screen changes. The scene
//! the user is looking at is now a version that exists nowhere else, and the next
//! `Ctrl+S` writes it back over the file git just checked out. The user asked for
//! the old scene, got it, and silently destroyed it by saving.
//!
//! So after any operation that can touch the working tree, the open scene is
//! re-read from disk if and only if the file actually changed.
//!
//! # Why a fingerprint and not the changed-path list
//!
//! Git can be asked what changed, but not uniformly: a `pull` moves HEAD and has a
//! before/after pair to diff, while `git restore` on one file moves nothing and
//! has no such pair. Each operation would need its own way of answering, and each
//! is a chance to answer wrongly in the direction that skips a reload.
//!
//! The file on disk is the thing that actually matters, so it is what gets
//! measured: hash the open scene before the operation and again after, and reload
//! on any difference. It is one small file read either side of an operation that
//! already spawned a process, and it cannot be fooled by an operation whose
//! changed-path list this crate didn't think to ask for.
//!
//! Length is compared alongside the hash so an edit that collides in 64 bits still
//! has to also preserve the exact byte length.

use std::path::{Path, PathBuf};

use bevy::prelude::*;

/// Identity of a file's contents at a moment in time.
///
/// `None` in a [`SceneWatch`] means "no such file", which is a state worth
/// distinguishing rather than folding into "unchanged": checking out a branch that
/// predates the scene deletes it, and checking one out that adds it creates it.
/// Both are changes the editor has to follow.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Fingerprint {
    len: u64,
    hash: u64,
}

/// Hash a file's contents, or `None` if it cannot be read (missing, or locked).
pub fn fingerprint(path: &Path) -> Option<Fingerprint> {
    use std::hash::{Hash, Hasher};
    let bytes = std::fs::read(path).ok()?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    Some(Fingerprint {
        len: bytes.len() as u64,
        hash: h.finish(),
    })
}

/// The open scene as it was just before a git operation started.
#[derive(Clone, Debug)]
pub struct SceneWatch {
    pub path: PathBuf,
    pub before: Option<Fingerprint>,
}

/// Absolute path of the scene the editor currently has open, if any.
///
/// `SceneLoadState::current_path` is whatever string the loader was handed, which
/// is usually absolute but is relative on the paths that set it from a project
/// config value — so it is resolved against the project root either way.
pub fn open_scene_path(world: &World) -> Option<PathBuf> {
    let current = world
        .get_resource::<renzora_engine::scene_io::SceneLoadState>()?
        .current_path
        .clone()?;
    if current.is_empty() {
        return None;
    }
    let path = PathBuf::from(&current);
    if path.is_absolute() {
        return Some(path);
    }
    let project = world.get_resource::<renzora::core::CurrentProject>()?;
    Some(project.path.join(path))
}

/// Fingerprint the open scene, to be compared after the operation.
///
/// Returns `None` when there is nothing to watch — no scene open, or the editor is
/// in play mode (see [`reconcile`]).
pub fn watch_open_scene(world: &World) -> Option<SceneWatch> {
    if in_play_mode(world) {
        return None;
    }
    let path = open_scene_path(world)?;
    let before = fingerprint(&path);
    Some(SceneWatch { path, before })
}

/// Play mode runs the game's own copy of the world; reloading the scene under it
/// would reset the running game as a side effect of a git operation, which is
/// never what the user meant by "pull". The staleness is also not dangerous here,
/// because stopping play restores the editor's own scene state anyway.
fn in_play_mode(world: &World) -> bool {
    world
        .get_resource::<renzora::core::PlayModeState>()
        .is_some_and(|p| p.is_in_play_mode())
}

/// What [`reconcile`] decided to do, so the caller can say so.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Reconciled {
    /// The file is byte-identical; the editor's scene is still correct.
    Unchanged,
    /// Re-read from disk.
    Reloaded(PathBuf),
    /// The scene file is gone (checked out a revision that predates it). It is
    /// deliberately *not* unloaded: throwing away the open scene because a branch
    /// switch removed its file would destroy work the user can still save
    /// somewhere else, and a warning lets them decide.
    Vanished(PathBuf),
}

/// Re-read the open scene if git changed it underneath the editor.
///
/// Called after the operation completes, with the [`SceneWatch`] taken before it
/// started.
pub fn reconcile(world: &mut World, watch: &SceneWatch) -> Reconciled {
    // Entering play mode during the operation: leave the running game alone.
    if in_play_mode(world) {
        return Reconciled::Unchanged;
    }
    // The user switched scene tabs while the operation ran, so the fingerprint
    // belongs to a scene that is no longer the live one. Reloading it now would
    // pull the *other* scene into the world.
    if open_scene_path(world).as_deref() != Some(watch.path.as_path()) {
        return Reconciled::Unchanged;
    }

    let after = fingerprint(&watch.path);
    if after == watch.before {
        return Reconciled::Unchanged;
    }
    if after.is_none() {
        return Reconciled::Vanished(watch.path.clone());
    }

    // The buffer the document-tab machinery keeps for this tab is deliberately
    // left alone. It is only read when switching *to* a tab, and switching *away*
    // re-serialises the live world first — so once the world holds the reloaded
    // scene, the buffer is correct without being touched. Clearing it would be the
    // riskier move: a tab with no buffer restores as an empty scene.
    renzora_engine::scene_io::load_scene(world, &watch.path);
    Reconciled::Reloaded(watch.path.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "renzora-git-reload-{}-{name}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("scene.bsn")
    }

    #[test]
    fn a_missing_file_has_no_fingerprint() {
        assert_eq!(fingerprint(Path::new("does/not/exist.bsn")), None);
    }

    #[test]
    fn identical_contents_fingerprint_identically() {
        let a = tmp("same-a");
        let b = tmp("same-b");
        std::fs::write(&a, b"(entities: {})").unwrap();
        std::fs::write(&b, b"(entities: {})").unwrap();
        assert_eq!(fingerprint(&a), fingerprint(&b));
    }

    /// The whole mechanism rests on this: a checkout that rewrites the scene has
    /// to produce a different fingerprint, or the reload never happens.
    #[test]
    fn changed_contents_fingerprint_differently() {
        let p = tmp("changed");
        std::fs::write(&p, b"before").unwrap();
        let before = fingerprint(&p);
        std::fs::write(&p, b"after!").unwrap();
        let after = fingerprint(&p);
        assert_ne!(before, after);
        // Same length, different bytes — so length alone would have missed it.
        assert_eq!(before.unwrap().len, after.unwrap().len);
    }

    /// A file being created or deleted are both changes, and `None != Some`
    /// is what makes them detectable without a separate existence check.
    #[test]
    fn appearing_and_vanishing_both_read_as_changes() {
        let p = tmp("vanish");
        std::fs::write(&p, b"here").unwrap();
        let present = fingerprint(&p);
        std::fs::remove_file(&p).unwrap();
        let absent = fingerprint(&p);
        assert!(present.is_some());
        assert_eq!(absent, None);
        assert_ne!(present, absent);
    }

    /// An empty file is a real state (a truncated scene), distinct from a missing
    /// one — the first is `Reloaded`, the second `Vanished`.
    #[test]
    fn an_empty_file_is_not_the_same_as_a_missing_one() {
        let p = tmp("empty");
        std::fs::write(&p, b"").unwrap();
        assert!(fingerprint(&p).is_some());
        assert_ne!(fingerprint(&p), None);
    }
}
