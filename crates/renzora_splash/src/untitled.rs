//! The scratch project the editor opens when you have not chosen one.
//!
//! The splash is an overlay over a live editor, and dismissing it has to leave
//! something behind you can actually work in: drag a mesh into the viewport,
//! open the material editor, import a texture. All of that writes somewhere, and
//! around a hundred systems read [`CurrentProject`] with no branch for its
//! absence. Rather than give every one of them an empty state, the editor always
//! has a project, and this is the one it has when the user has not picked any.
//!
//! It is an ordinary project in an ordinary folder, created by the same
//! [`create_project`](crate::project::create_project) that the New Project
//! button calls. That is the whole trick: nothing downstream can tell the
//! difference, so nothing downstream needs to. The only thing that knows is
//! [`UntitledProject`], a marker resource read by the title bar, the recents
//! list, and File > Create Project.
//!
//! # Why it is kept between launches
//!
//! Blender's `untitled.blend` is discarded on quit, and losing an afternoon of
//! messing about to a reflex Alt+F4 is a bad trade for a folder that costs
//! nothing to keep. So the scratch project persists: quitting without creating
//! a project leaves the work exactly where it was, and the next launch opens
//! straight back into it.
//!
//! The cost is that "Untitled" accumulates whatever you last did in it, rather
//! than being clean each time. That is the right default for not losing work,
//! and **File > Create Project** is how you take a session somewhere permanent
//! and leave the scratch folder behind.

use std::path::PathBuf;

use renzora::{CurrentProject, UntitledProject};

/// `~/.renzora/untitled`, or `None` when there is no home directory to resolve.
///
/// Beside `settings.toml` rather than in the OS cache directory, and not under
/// the engine install: a cache is something the system may delete, and the
/// install is something an update replaces. This holds unsaved work, so it goes
/// where the user's other engine state already lives.
#[cfg(not(target_arch = "wasm32"))]
pub fn scratch_root() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)?;
    Some(home.join(".renzora").join("untitled"))
}

/// The name shown wherever a project name is shown, while untitled.
///
/// Not run through `lang::t` at the point it is written into `project.toml`: it
/// is stored on disk, and a name that changed with the editor's language would
/// mean the folder read differently on the next launch in another language.
/// The *display* of it is localized at the point of display.
pub const UNTITLED_NAME: &str = "Untitled";

/// Open the scratch project, creating it the first time.
///
/// `Err` carries something worth showing a user: it means neither the existing
/// scratch project could be read nor a new one written, which is a disk problem
/// rather than a state the editor should paper over.
#[cfg(not(target_arch = "wasm32"))]
pub fn open_or_create() -> Result<CurrentProject, String> {
    let root = scratch_root().ok_or_else(|| {
        "no home directory, so there is nowhere to keep unsaved work".to_string()
    })?;

    // An existing scratch project is opened rather than recreated, or every
    // launch would wipe the work this folder exists to preserve.
    let manifest = root.join("project.toml");
    if manifest.is_file() {
        return crate::project::open_project(&manifest).map_err(|e| {
            format!("{} could not be opened: {e}", root.display())
        });
    }

    crate::project::create_project(&root, UNTITLED_NAME)
        .map_err(|e| format!("{} could not be created: {e}", root.display()))
}

/// Is this project the scratch one?
///
/// A path comparison rather than a flag read off the project, so it answers
/// correctly for a project opened by any route: `--project ~/.renzora/untitled`
/// is still the scratch project, and should not present itself as a real one
/// just because it arrived through a different door.
#[cfg(not(target_arch = "wasm32"))]
pub fn is_scratch(project: &CurrentProject) -> bool {
    scratch_root().is_some_and(|root| {
        // Canonicalized so a symlinked or differently-cased home does not read
        // as a different project. Falls back to the plain paths when either side
        // does not exist yet, which is the case on the very first launch.
        match (root.canonicalize(), project.path.canonicalize()) {
            (Ok(a), Ok(b)) => a == b,
            _ => root == project.path,
        }
    })
}

/// The marker to insert alongside a `CurrentProject`, if it is the scratch one.
#[cfg(not(target_arch = "wasm32"))]
pub fn marker_for(project: &CurrentProject) -> Option<UntitledProject> {
    is_scratch(project).then_some(UntitledProject)
}

/// Copy a project folder to `dest`, skipping what should not follow it.
///
/// Build output and caches are rebuilt from source and can be gigabytes, so
/// copying them would turn "create a project" into a long operation that
/// produces a worse result: `.renzora` in particular records a build keyed to a
/// path that is about to change.
///
/// Returns the number of files copied, which is what the Console line reports:
/// "created with 41 files" is the difference between a project that carried the
/// work over and one that quietly did not.
#[cfg(not(target_arch = "wasm32"))]
pub fn copy_project(src: &std::path::Path, dest: &std::path::Path) -> std::io::Result<u32> {
    /// Directories never worth copying into a new project.
    ///
    /// `.cache` holds generated thumbnails, `.renzora` a build stamped against
    /// the old path, `target` a cargo tree, `.git` a history that belongs to the
    /// scratch folder rather than to the project being made from it.
    const SKIP: &[&str] = &[".cache", ".renzora", "target", ".git", "node_modules"];

    let mut copied = 0;
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        if SKIP.iter().any(|skip| std::ffi::OsStr::new(skip) == name) {
            continue;
        }
        let from = entry.path();
        let to = dest.join(&name);
        if entry.file_type()?.is_dir() {
            copied += copy_project(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
            copied += 1;
        }
    }
    Ok(copied)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// The scratch root has to sit under the same per-user directory as the
    /// rest of the engine's state. A test rather than a comment because the
    /// path is assembled from environment variables, and the failure mode of
    /// getting it wrong is unsaved work written somewhere nobody looks.
    #[test]
    fn the_scratch_root_is_under_the_engine_state_directory() {
        let Some(root) = scratch_root() else {
            // No home on this machine, which is the documented `None` case.
            return;
        };
        assert!(
            root.ends_with(std::path::Path::new(".renzora").join("untitled")),
            "{} is not ~/.renzora/untitled",
            root.display()
        );
    }

    /// The whole presentation layer hangs off this answer, so a project that is
    /// merely *near* the scratch folder must not be mistaken for it.
    #[test]
    fn a_project_beside_the_scratch_folder_is_not_the_scratch_project() {
        let Some(root) = scratch_root() else { return };
        let neighbour = CurrentProject {
            path: root.with_file_name("untitled-backup"),
            config: Default::default(),
        };
        assert!(!is_scratch(&neighbour));
        assert!(marker_for(&neighbour).is_none());
    }

    /// The work has to arrive in the new project, and the build output has to
    /// not. Both halves matter: the first is the whole feature, and the second
    /// is the difference between a copy that takes a moment and one that takes
    /// minutes and lands a stale build in a folder with a new path.
    #[test]
    fn creating_a_project_carries_the_work_and_leaves_the_build_output() {
        let base = std::env::temp_dir().join(format!(
            "renzora-copy-project-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let src = base.join("scratch");
        let dest = base.join("made");
        std::fs::create_dir_all(src.join("scenes")).expect("scenes");
        std::fs::create_dir_all(src.join(".renzora").join("bevy")).expect("build dir");
        std::fs::create_dir_all(src.join("target")).expect("target");
        std::fs::write(src.join("project.toml"), "name = \"Untitled\"").expect("manifest");
        std::fs::write(src.join("scenes").join("main.bsn"), "// scene").expect("scene");
        std::fs::write(src.join(".renzora").join("bevy").join("lib.rs"), "generated")
            .expect("generated");
        std::fs::write(src.join("target").join("big.rlib"), "artefact").expect("artefact");

        let copied = copy_project(&src, &dest).expect("copy");

        assert_eq!(copied, 2, "the manifest and the scene, and nothing else");
        assert!(dest.join("project.toml").is_file());
        assert!(dest.join("scenes").join("main.bsn").is_file());
        assert!(!dest.join(".renzora").exists(), "a build keyed to the old path");
        assert!(!dest.join("target").exists(), "a cargo tree");

        let _ = std::fs::remove_dir_all(&base);
    }
}
