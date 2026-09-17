//! Is the library from the last build still good, or does it have to be made
//! again?
//!
//! The loader used to answer "made again" every time. It wrote a stamp beside
//! the build and never read it back, so opening a project cost a full compile
//! (about ten seconds on the project this was built against) whether or not a
//! character had changed since the last launch. Always-fresh is the safe default
//! and it is the wrong one to *keep*: the whole point of opening an editor is
//! that it opens.
//!
//! Two questions have to agree before a build is skipped, and they fail in
//! different directions:
//!
//! 1. **Was it built against this engine?** The stamp is a content hash of the
//!    images a plugin binds to. A mismatch means the engine moved underneath it,
//!    and loading it anyway is the `TypeId` corruption the whole plugin design
//!    exists to prevent. This is the question that must never be got wrong.
//! 2. **Has the project changed since?** Source mtimes against the library's.
//!    Getting this wrong means a stale build, which is merely annoying: the user
//!    saves again, the watcher rebuilds, and the mistake costs one edit.
//!
//! So the first is exact and the second is a heuristic, which is the right way
//! round.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// What the last build left behind.
///
/// Two lines rather than JSON: the stamp, then the library's file name. The
/// filename has to be recorded because the build writes a *generation-suffixed*
/// name (`grove-1726449182734.dll`) so a reload never overwrites a mapped image,
/// which means the name cannot be derived, only remembered.
pub struct Built {
    pub stamp: String,
    pub library: PathBuf,
}

/// The stamp file's name, beside the staged crate.
const STAMP: &str = "stamp.txt";

/// Record a finished build so the next launch can skip it.
pub fn record(staged: &Path, stamp: &str, library: &Path) {
    let name = library.file_name().unwrap_or_default().to_string_lossy();
    let _ = std::fs::write(staged.join(STAMP), format!("{stamp}\n{name}\n"));
}

/// Delete every build but the one the stamp names.
///
/// Each build writes a generation-suffixed library (`grove-1726449182734.dll`)
/// because the previous one is mapped into this process and cannot be replaced.
/// That is the right thing to do and it litters: one library per save, plus the
/// `.pdb` and import library beside it, for as long as a session lasts. A day of
/// editing leaves a directory of dead builds and no way to tell which one is
/// live.
///
/// **Only safe before anything is loaded**, which is why this runs at startup
/// rather than after a rebuild. At that point no image in this directory is
/// mapped, so every file that is not the current build is free to remove. Called
/// after a reload it would be deleting the library the process is running from.
///
/// Best effort throughout. A file that will not delete is one that is somehow
/// still held, and the right response is to leave it and carry on rather than
/// refuse to open the project over housekeeping.
pub fn prune_old_builds(staged: &Path) {
    // Keep everything belonging to the recorded build: the library, its debug
    // symbols and its import library all share a stem.
    let keep = std::fs::read_to_string(staged.join(STAMP))
        .ok()
        .and_then(|text| text.lines().nth(1).map(|name| name.trim().to_string()))
        .and_then(|name| {
            Path::new(&name)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
        });

    let Ok(entries) = std::fs::read_dir(staged) else {
        return;
    };
    let mut removed = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        // Build output only. The staged crate root and its manifest live in this
        // directory too, and deleting those would cost a full restage.
        if !matches!(ext.as_str(), "dll" | "pdb" | "lib" | "exp" | "so" | "dylib") {
            continue;
        }
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        if keep.as_deref() == Some(stem.as_str()) {
            continue;
        }
        if std::fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    if removed > 0 {
        bevy::log::info!("[bevy-project] removed {removed} stale build file(s)");
    }
}

/// The library from the last build, if it can still be used.
///
/// `None` means build. Every reason to rebuild is a `None` here rather than an
/// error, because none of them is a problem: a missing stamp is a first run, a
/// moved engine is an upgrade, and a newer source file is the user doing their
/// job.
pub fn reusable(staged: &Path, project: &Path, want_stamp: &str) -> Option<Built> {
    let text = std::fs::read_to_string(staged.join(STAMP)).ok()?;
    let mut lines = text.lines();
    let stamp = lines.next()?.trim().to_string();
    if stamp != want_stamp {
        return None;
    }
    let library = staged.join(lines.next()?.trim());
    let built = std::fs::metadata(&library).ok()?.modified().ok()?;

    // The generated crate root is derived from the project's source, so it does
    // not need checking itself, but the manifest does, because a dependency
    // change is invisible in the `.rs` files.
    if newest_source(project)? > built {
        return None;
    }
    Some(Built { stamp, library })
}

/// Directories that hold no source of the project's own.
///
/// The list lives in the contract crate because the **file watcher** needs the
/// same answer, and two lists would eventually disagree. This one existed first
/// and the watcher had none, which was not merely a slow walk but a loop: a
/// rebuild rewrites `.renzora/bevy/src/lib.rs`, the watcher reported a `.rs`
/// change inside the project, and that scheduled the rebuild that rewrote it
/// again.
pub use renzora::core::project_files::{is_build_output, BUILD_OUTPUT_DIRS as NON_SOURCE_DIRS};

/// The most recent modification time among the project's Rust and its manifest.
///
/// `None` when the project cannot be read at all, which is treated as "rebuild"
/// by the caller: the safe direction.
fn newest_source(project: &Path) -> Option<SystemTime> {
    fn walk(dir: &Path, newest: &mut Option<SystemTime>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            if path.is_dir() {
                if !NON_SOURCE_DIRS.contains(&name.as_str()) && !name.starts_with('.') {
                    walk(&path, newest);
                }
                continue;
            }
            let is_source = path.extension().and_then(|e| e.to_str()) == Some("rs")
                || name == "Cargo.toml";
            if !is_source {
                continue;
            }
            if let Ok(modified) = entry.metadata().and_then(|m| m.modified()) {
                if newest.is_none_or(|current| modified > current) {
                    *newest = Some(modified);
                }
            }
        }
    }

    let mut newest = None;
    walk(project, &mut newest);
    newest
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "renzora-bevy-cache-{name}-{}",
            SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::create_dir_all(dir.join(".renzora").join("bevy")).unwrap();
        std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"g\"\n").unwrap();
        std::fs::write(dir.join("src").join("main.rs"), "fn main() {}").unwrap();
        dir
    }

    /// Build the library *after* the sources, which is what a real build does.
    fn record_build(project: &Path, stamp: &str) -> PathBuf {
        let staged = project.join(".renzora").join("bevy");
        // mtime granularity on some filesystems is coarse enough that a file
        // written in the same instant compares equal, which would make the
        // "unchanged" case flaky. A real build takes seconds.
        std::thread::sleep(Duration::from_millis(20));
        let library = staged.join("g-1.dll");
        std::fs::write(&library, b"not really a dll").unwrap();
        record(&staged, stamp, &library);
        library
    }

    #[test]
    fn an_unchanged_project_reuses_its_library() {
        let dir = scratch("unchanged");
        let library = record_build(&dir, "abc+rustc-1.95.0");
        let staged = dir.join(".renzora").join("bevy");
        let reused = reusable(&staged, &dir, "abc+rustc-1.95.0").expect("reusable");
        assert_eq!(reused.library, library);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The question that must never be got wrong: a different engine means the
    /// library's `TypeId`s no longer match, and loading it is corruption.
    #[test]
    fn a_moved_engine_forces_a_rebuild() {
        let dir = scratch("stamp");
        record_build(&dir, "abc+rustc-1.95.0");
        let staged = dir.join(".renzora").join("bevy");
        assert!(reusable(&staged, &dir, "DIFFERENT+rustc-1.95.0").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_edited_source_file_forces_a_rebuild() {
        let dir = scratch("edited");
        record_build(&dir, "abc");
        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(dir.join("src").join("main.rs"), "fn main() { /* edited */ }").unwrap();
        let staged = dir.join(".renzora").join("bevy");
        assert!(reusable(&staged, &dir, "abc").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A dependency change is invisible in the `.rs` files, so the manifest has
    /// to count as source.
    #[test]
    fn an_edited_manifest_forces_a_rebuild() {
        let dir = scratch("manifest");
        record_build(&dir, "abc");
        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"g\"\n# changed\n").unwrap();
        let staged = dir.join(".renzora").join("bevy");
        assert!(reusable(&staged, &dir, "abc").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The build's own output must not count as a change, or the project would
    /// rebuild on every launch for ever, which is the bug this module fixes.
    #[test]
    fn the_builds_own_output_is_not_a_source_change() {
        let dir = scratch("selfref");
        record_build(&dir, "abc");
        std::thread::sleep(Duration::from_millis(20));
        // Exactly what a build leaves behind, written after the library.
        std::fs::create_dir_all(dir.join(".renzora").join("bevy").join("src")).unwrap();
        std::fs::write(
            dir.join(".renzora").join("bevy").join("src").join("lib.rs"),
            "// generated",
        )
        .unwrap();
        let staged = dir.join(".renzora").join("bevy");
        assert!(reusable(&staged, &dir, "abc").is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_first_run_has_nothing_to_reuse() {
        let dir = scratch("first");
        let staged = dir.join(".renzora").join("bevy");
        assert!(reusable(&staged, &dir, "abc").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The watcher's half of the rule, which is the half that was missing: the
    /// generated crate root is a `.rs` file inside the project, so without this
    /// every build scheduled the next one.
    #[test]
    fn the_generated_crate_root_is_not_a_source_change() {
        let root = Path::new("/project");
        assert!(is_build_output(
            root,
            &root.join(".renzora").join("bevy").join("src").join("lib.rs")
        ));
        assert!(is_build_output(root, &root.join("target").join("debug").join("build.rs")));
        assert!(is_build_output(root, &root.join(".git").join("COMMIT_EDITMSG")));
    }

    /// ...while the author's own source still counts, including a nested module
    /// and the manifest beside it.
    #[test]
    fn the_projects_own_source_is_a_source_change() {
        let root = Path::new("/project");
        assert!(!is_build_output(root, &root.join("src").join("main.rs")));
        assert!(!is_build_output(root, &root.join("src").join("game").join("player.rs")));
        assert!(!is_build_output(root, &root.join("Cargo.toml")));
    }

    /// A path outside the project answers "not source" rather than panicking on
    /// the failed `strip_prefix`. The watcher carries extra roots.
    #[test]
    fn a_path_outside_the_project_is_not_source() {
        assert!(is_build_output(Path::new("/project"), Path::new("/elsewhere/src/main.rs")));
    }

    /// Every dead build goes, the live one and its debug symbols stay, and the
    /// staged crate is not touched: deleting `src/lib.rs` or the manifest would
    /// cost a full restage for nothing.
    #[test]
    fn pruning_keeps_the_recorded_build_and_the_staged_crate() {
        let dir = scratch("prune");
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src").join("lib.rs"), "// generated").unwrap();
        std::fs::write(dir.join("Cargo.toml"), "[package]").unwrap();
        for name in ["game-1.dll", "game-1.pdb", "game-2.dll", "game-3.dll", "game-3.pdb"] {
            std::fs::write(dir.join(name), "x").unwrap();
        }
        record(&dir, "abc", &dir.join("game-3.dll"));

        prune_old_builds(&dir);

        assert!(dir.join("game-3.dll").exists(), "the live build must survive");
        assert!(dir.join("game-3.pdb").exists(), "its symbols share the stem");
        assert!(!dir.join("game-1.dll").exists());
        assert!(!dir.join("game-1.pdb").exists());
        assert!(!dir.join("game-2.dll").exists());
        assert!(dir.join("src").join("lib.rs").exists(), "the staged crate is not output");
        assert!(dir.join("Cargo.toml").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// No stamp is a first run, or a directory left by a build that never
    /// finished. Nothing is recorded as live, so every library in there is dead.
    #[test]
    fn pruning_without_a_stamp_removes_every_build() {
        let dir = scratch("prune_no_stamp");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("game-1.dll"), "x").unwrap();
        std::fs::write(dir.join("game-2.dll"), "x").unwrap();

        prune_old_builds(&dir);

        assert!(!dir.join("game-1.dll").exists());
        assert!(!dir.join("game-2.dll").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
