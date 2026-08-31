//! What a built plugin is allowed to leave behind.
//!
//! `-o foo.dll` writes more than `foo.dll`: on Windows MSVC the linker adds an
//! import library and, when symbols are on, a PDB that is routinely larger than
//! the plugin. None of it can be loaded — the host opens a plugin by symbol at
//! runtime and nothing ever *links against* one — so it is build residue that
//! happened to land in the install directory.
//!
//! The risk in deleting by pattern is deleting the wrong thing, and the two ways
//! that happens are worth pinning: taking the loadable image itself, or reaching
//! a sibling that belongs to something else (`stamp.txt` is what the loader
//! compares to decide whether a rebuild is due — losing it means every launch
//! recompiles). Both are asserted below.
//!
//! No `tempfile`: this crate is documented as having no dependencies of its own,
//! and xtask depends on it precisely so that stays true.
//!
//! Deletion only. Rewriting the library itself to drop its `.rustc` metadata
//! section was tried — it is 41% of the file — and the result does not load;
//! see the note on `prune_byproducts` for why static checks all pass on an image
//! Windows then refuses.

use std::path::{Path, PathBuf};

use renzora_native_build::rustc::prune_byproducts;

/// A scratch directory unique to one test, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        // Nanoseconds as well as the tag: cargo runs integration tests in
        // parallel threads, and two sharing a directory would delete each
        // other's fixtures and fail intermittently.
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("renzora-prune-{tag}-{stamp}"));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        Self(dir)
    }

    fn touch(&self, name: &str) -> PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, b"x").expect("write fixture");
        p
    }

    fn has(&self, name: &str) -> bool {
        self.0.join(name).exists()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn removes_windows_import_library_and_pdb() {
    let s = Scratch::new("win");
    let dll = s.touch("vignette.dll");
    s.touch("vignette.dll.lib");
    s.touch("vignette.pdb");

    prune_byproducts(&dll);

    assert!(s.has("vignette.dll"), "the loadable image must survive");
    assert!(!s.has("vignette.dll.lib"), "import library is not loadable");
    assert!(!s.has("vignette.pdb"), "debug symbols are not loadable");
}

#[test]
fn keeps_the_stamp() {
    // The loader compares `stamp.txt` against the current engine to decide
    // whether a plugin is stale. Deleting it would not break anything visibly —
    // it would just silently recompile every plugin on every launch.
    let s = Scratch::new("stamp");
    let dll = s.touch("vignette.dll");
    s.touch("stamp.txt");

    prune_byproducts(&dll);

    assert!(s.has("stamp.txt"), "stamp must survive the prune");
    assert!(s.has("vignette.dll"));
}

#[test]
fn leaves_other_plugins_alone() {
    // Every candidate name is derived from the artifact being pruned, so a
    // sibling plugin sharing the directory keeps all of its files — including
    // the by-products that are its own to clean up.
    let s = Scratch::new("siblings");
    let dll = s.touch("vignette.dll");
    s.touch("orrery.dll");
    s.touch("orrery.pdb");

    prune_byproducts(&dll);

    assert!(s.has("orrery.dll"));
    assert!(s.has("orrery.pdb"), "another plugin's by-products are not ours");
}

#[test]
fn unix_shared_object_has_nothing_to_remove() {
    // Linux emits the `.so` and nothing else, so the prune is a no-op there
    // rather than an error. Guards against a future rule that strips by
    // extension and takes the library itself on a platform with no suffix pair.
    let s = Scratch::new("unix");
    let so = s.touch("libvignette.so");

    prune_byproducts(&so);

    assert!(s.has("libvignette.so"));
}

#[test]
fn tolerates_a_missing_directory() {
    // Best-effort by contract: a build that succeeded must not be reported as
    // failed because cleanup could not run.
    prune_byproducts(Path::new("does/not/exist/plugin.dll"));
}
