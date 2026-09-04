//! Build the repo's own native plugins the way a user's machine builds an
//! installed one.
//!
//! # Why these cannot go through `cargo build`
//!
//! `build_source_plugins` runs `cargo build` inside each `plugins/<name>/`, and
//! that is correct for a C-ABI plugin: it is a zero-dependency, own-workspace
//! project that links no Bevy, so cargo has nothing to get wrong.
//!
//! A native plugin links Bevy, and `plugins/` is deliberately outside the engine
//! workspace (see the root `Cargo.toml`). Cargo would therefore resolve it a
//! *fresh* Bevy from crates.io — a different compilation, different `TypeId`s —
//! and produce a plugin that builds cleanly, loads, and corrupts the World. The
//! only sound way to build one is against the artifacts the engine was actually
//! built from, which is exactly what the staged SDK holds.
//!
//! That constraint is a feature. A plugin author working from source builds
//! through the identical path a user does, rather than a dev-only shortcut that
//! proves nothing about what ships.
//!
//! # Which is which
//!
//! `crate-type`, because it already *is* the distinction rather than a
//! convention layered on top:
//!
//! ```toml
//! crate-type = ["cdylib"]   # C ABI     -> cargo, links no Bevy
//! crate-type = ["dylib"]    # Rust ABI  -> rustc against the SDK
//! ```
//!
//! # Where the flags come from
//!
//! `sdk/manifest.json`, which [`crate::sdk`] wrote a moment earlier. Both this
//! and `renzora_plugin_build` (which the editor uses) derive their command line
//! from that one file rather than each holding their own copy of the rules —
//! `--extern bevy` must be the facade crate, `--extern renzora` must be the
//! *dylib*, and a missing `-L native=` path fails at link with a message naming
//! a `.lib` file and nothing that explains why.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::sdk::{json_string, json_string_array};

/// Compile every native plugin under `plugins/` into the staged tree.
///
/// Returns false on the first failure, matching `build_source_plugins`.
/// Takes no `Platform`: the library extension and target triple come from the
/// SDK manifest, which is the authority on what the plugin is being built
/// *against* rather than what host this happens to be.
pub fn build_all(repo: &Path, dist_root: &Path) -> bool {
    let manifest_path = dist_root.join("sdk").join("manifest.json");
    let Ok(manifest) = std::fs::read_to_string(&manifest_path) else {
        // No SDK staged. Only reachable if the SDK step was skipped or failed,
        // which has already reported itself.
        return true;
    };
    // The whole file, newlines stripped: the JSON helpers scan one line, and the
    // manifest is pretty-printed for humans reading it in a release.
    let flat = manifest.replace('\n', " ");

    let Some(sdk) = Sdk::parse(&flat, dist_root) else {
        eprintln!("[xtask] sdk manifest at {} is unreadable", manifest_path.display());
        return false;
    };

    let root = repo.join("plugins");
    let Ok(entries) = std::fs::read_dir(&root) else {
        return true;
    };
    let mut dirs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    // Stable order so a build is reproducible and a failure is always the same
    // one first.
    dirs.sort();

    prune_orphans(&root, dist_root);

    for dir in dirs {
        if !is_native(&dir) {
            continue;
        }
        if !build_one(&dir, &sdk, dist_root) {
            return false;
        }
    }
    true
}

/// Delete staged native plugins whose source directory is gone.
///
/// Staging only ever *added*, and a plugin that was deleted or moved out of
/// `plugins/` therefore left its staged copy behind forever — source, manifest
/// and all. The editor cannot tell that copy from a real plugin: it has a
/// `src/lib.rs`, so `prebuild::needed()` says there is work to do, and the build
/// fails because the source it references no longer exists.
///
/// That was not a cosmetic leak. `main` runs the setup window whenever
/// `needed()` is true and restarts the process afterwards, so a staged orphan
/// that cannot build put the editor in an endless loop of setup windows. The
/// loader now remembers a failed build (see `renzora_native_plugin::layout`),
/// which stops the loop; this stops it being entered at all.
///
/// Only directories are considered, and only ones holding a `Cargo.toml` — a
/// staged plugin always has one, and refusing to recurse past that keeps this
/// from ever looking at an unrelated directory a user put in `dist/`.
///
/// **Marketplace installs are not orphans.** `dist/<platform>/plugins/` is
/// shared: xtask stages copies of the repo's own `plugins/*` there, and the
/// editor installs downloaded ones into the same directory, because that is
/// where the loader looks. A downloaded plugin has no source in the repo, so by
/// the rule above it looked exactly like an orphan and was deleted on the next
/// `cargo renzora` — the plugin you just installed, gone. They are told apart by
/// the `plugin.toml` sidecar the marketplace writes inside the directory it
/// installs, which is why that write is fatal there rather than best-effort.
fn prune_orphans(src_root: &Path, dist_root: &Path) {
    let staged_root = dist_root.join("plugins");
    let Ok(entries) = std::fs::read_dir(&staged_root) else {
        return;
    };
    for staged in entries.flatten().map(|e| e.path()) {
        if !staged.is_dir() || !staged.join("Cargo.toml").is_file() {
            continue;
        }
        // Installed from the marketplace, not staged from this repo. Not ours
        // to delete.
        if staged.join("plugin.toml").is_file() {
            continue;
        }
        let name = crate::file_name(&staged);
        if src_root.join(&name).is_dir() {
            continue;
        }
        match std::fs::remove_dir_all(&staged) {
            Ok(()) => println!("[xtask] removed staged plugin '{name}' (no longer in plugins/)"),
            Err(e) => eprintln!("[xtask] could not remove staged plugin '{name}': {e}"),
        }
    }
}

/// A `plugins/<name>/` holding a `dylib` crate.
///
/// Checks for the quoted `"dylib"` rather than the bare word: `"cdylib"` also
/// ends in `dylib`, and matching loosely would send every C-ABI plugin down this
/// path.
pub fn is_native(dir: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(dir.join("Cargo.toml")) else {
        return false;
    };
    text.lines()
        .filter(|l| l.trim_start().starts_with("crate-type"))
        .any(|l| l.contains("\"dylib\""))
}

/// The parts of `sdk/manifest.json` a plugin build needs.
struct Sdk {
    lib_ext: String,
    triple: String,
    /// `stamp` recorded beside a built plugin, so staleness is decided by the
    /// same rule here and in the editor: the artifact the plugin is bound to,
    /// plus the compiler that produced it.
    stamp: String,
    extern_bevy: PathBuf,
    extern_renzora: PathBuf,
    /// The shared ember image, absent only for an SDK staged before panels were
    /// reachable from a plugin.
    extern_ember: Option<PathBuf>,
    dependency: Vec<PathBuf>,
    native: Vec<PathBuf>,
}

impl Sdk {
    fn parse(flat: &str, dist_root: &Path) -> Option<Self> {
        let root = dist_root.join("sdk");
        let rustc = json_string(flat, "rustc")?;
        let bevy = json_string(flat, "bevy")?;
        let renzora = json_string(flat, "renzora")?;
        // Must agree with `renzora_plugin_build::Sdk::stamp` exactly — the
        // editor rebuilds anything whose stamp does not match what IT computes,
        // so a build staged here under a different rule would be rebuilt on
        // first launch. `build_id` is the content hash of the linked images;
        // the filename form is the pre-`build_id` fallback.
        let stamp = match json_string(flat, "build_id") {
            Some(id) if !id.is_empty() => format!("{id}+rustc-{rustc}"),
            _ => {
                let bevy_name = Path::new(&bevy).file_name()?.to_str()?.to_string();
                format!("{bevy_name}+rustc-{rustc}")
            }
        };
        Some(Sdk {
            lib_ext: json_string(flat, "lib_ext")?,
            triple: json_string(flat, "triple")?,
            stamp,
            extern_bevy: root.join(&bevy),
            extern_renzora: root.join(&renzora),
            extern_ember: json_string(flat, "renzora_ember")
                .filter(|s| !s.is_empty())
                .map(|s| root.join(s)),
            dependency: json_string_array(flat, "dependency")
                .iter()
                .map(|d| root.join(d))
                .collect(),
            native: json_string_array(flat, "native").iter().map(|d| root.join(d)).collect(),
        })
    }
}

/// Stage one plugin's source into `dist/` and compile it there.
///
/// The build happens in the STAGED copy, not in `plugins/<name>/`, so the result
/// is byte-for-byte the layout a user's install has — source, `build/` and a
/// stamp in one directory the loader can rebuild from later. Building in the
/// repo and copying the artefact would produce a `dist/` that works but cannot
/// heal itself.
fn build_one(src_dir: &Path, sdk: &Sdk, dist_root: &Path) -> bool {
    let name = crate::file_name(src_dir);
    let out_dir = dist_root.join("plugins").join(&name);
    let build = out_dir.join("build");
    let lib = build.join(format!("{}.{}", name.replace('-', "_"), sdk.lib_ext));
    let stamp_file = build.join("stamp.txt");

    if let Err(e) = mirror_source(src_dir, &out_dir) {
        eprintln!("[xtask] {name}: staging source failed: {e}");
        return false;
    }
    if let Err(e) = std::fs::create_dir_all(&build) {
        eprintln!("[xtask] {name}: {e}");
        return false;
    }

    // Two reasons to rebuild, and both are needed. The stamp catches "the engine
    // moved" — the case a user hits. Source mtime catches "the author just
    // edited it", which the stamp cannot see because the SDK did not change.
    let stamp_ok = std::fs::read_to_string(&stamp_file).is_ok_and(|s| s == sdk.stamp);
    if stamp_ok && lib.is_file() && !newer_than(src_dir, &lib) {
        return true;
    }

    // The command line comes from `renzora_native_build`, which the editor also
    // uses — that is the whole reason the crate exists. This used to be a second
    // copy of the same flags, and a change to either drifted from the other
    // without anything looking wrong on its own.
    let crate_name = name.replace('-', "_");
    let src = out_dir.join("src").join("lib.rs");
    let target = renzora_native_build::Target {
        triple: &sdk.triple,
        crate_name: &crate_name,
        extern_bevy: &sdk.extern_bevy,
        extern_renzora: &sdk.extern_renzora,
        extern_ember: sdk.extern_ember.as_deref(),
        dependency: &sdk.dependency,
        native: &sdk.native,
        plugin_dir: &out_dir,
        build_dir: &build,
        src: &src,
        out: &lib,
    };
    // Assembling the arguments can itself run a cargo build for the plugin's
    // third-party dependencies, so it happens before the line announcing rustc.
    let args = match renzora_native_build::rustc::args(&target) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("[xtask] {name}: {e}");
            return false;
        }
    };

    println!("[xtask] rustc --crate-type dylib ({name})");
    let mut cmd = Command::new("rustc");
    for (key, value) in renzora_native_build::rustc::env_vars(&target) {
        cmd.env(key, value);
    }
    cmd.args(&args);
    let ok = cmd.status().map(|s| s.success()).unwrap_or(false);
    if !ok {
        eprintln!("[xtask] {name}: failed to compile");
        return false;
    }
    // Same prune the editor does after its own rustc, so a staged plugin and an
    // installed one hold the same files. The staged tree is meant to be
    // byte-for-byte the layout a user's install has, and shipping the PDB here
    // but not there would make this path stop proving that.
    renzora_native_build::rustc::prune_byproducts(&lib);
    // And the same install-name rewrite, for the same reason: `fixup_macos`
    // sweeps the staged tree one directory deep, so it never reaches a native
    // plugin at `plugins/<name>/build/`, and a plugin an editor compiles later
    // is not in that tree at all when staging runs.
    renzora_native_build::rustc::fixup_install_names(&lib);
    if let Err(e) = std::fs::write(&stamp_file, &sdk.stamp) {
        eprintln!("[xtask] {name}: writing stamp: {e}");
        return false;
    }
    true
}

/// Copy `Cargo.toml`, `src/` and `thumbnail.jpg` into the staged plugin
/// directory.
///
/// `build/` is left alone — it holds the previous artefact and stamp, which is
/// what makes the "nothing changed" path skip the compile.
///
/// The thumbnail is staged because it is read at *runtime*, not build time:
/// Settings → Plugins and the exporter's plugin picker both load
/// `<exe>/plugins/<id>/thumbnail.jpg`. Leave it out and the artwork shows in a
/// source checkout — where the repo copy happens to be where the panel looks —
/// and silently turns into a placeholder in every real install. Compare
/// `include_bytes!` assets, which must go UNDER `src/` for the opposite reason:
/// those are read by rustc, which only ever sees the staged tree.
fn mirror_source(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst.join("src"))?;
    let manifest = src.join("Cargo.toml");
    if manifest.is_file() {
        copy_if_changed(&manifest, &dst.join("Cargo.toml"))?;
    }
    let thumb = src.join("thumbnail.jpg");
    if thumb.is_file() {
        copy_if_changed(&thumb, &dst.join("thumbnail.jpg"))?;
    }
    copy_tree(&src.join("src"), &dst.join("src"))
}

/// Copy a source tree, recursively.
///
/// Recursive because a plugin big enough to be worth shipping has submodules,
/// and the one-level version failed in a way nobody would connect to staging: a
/// `src/ui/mod.rs` the author can see in their editor is simply absent from the
/// staged copy, and rustc reports `file not found for module` against a path
/// that plainly exists.
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)?.flatten() {
        let p = e.path();
        let dst = to.join(e.file_name());
        if p.is_dir() {
            copy_tree(&p, &dst)?;
        } else if p.is_file() {
            copy_if_changed(&p, &dst)?;
        }
    }
    Ok(())
}

/// Copy `src` over `dst` only when the contents differ.
///
/// `fs::copy` does not preserve modification time, and this runs *before* the
/// staleness check — so an unconditional copy re-stamped every staged source
/// file with the current time on every `cargo renzora`, leaving the source
/// newer than the library beside it. That looked harmless here (the check
/// compares the *repo* source against the built library, so xtask correctly
/// skipped the compile) and then cost a second per plugin somewhere else
/// entirely: the editor's loader runs the same mtime test against the STAGED
/// directory, found every plugin stale, and rebuilt all of them at startup —
/// after every single build, forever.
///
/// Comparing bytes rather than mtimes because the files are a few KB and the
/// question being asked is genuinely "did this change", not "was it touched".
fn copy_if_changed(src: &Path, dst: &Path) -> std::io::Result<()> {
    if let (Ok(a), Ok(b)) = (std::fs::read(src), std::fs::read(dst)) {
        if a == b {
            return Ok(());
        }
    }
    crate::copy(src, dst)
}

/// Whether anything under `dir/src` is newer than `target`.
///
/// Recursive, to match [`copy_tree`]: a change in `src/ui/panel.rs` has to
/// trigger a rebuild, and the one-level version silently did not — the plugin
/// staged with the new source and kept the old library.
fn newer_than(dir: &Path, target: &Path) -> bool {
    let Ok(built) = std::fs::metadata(target).and_then(|m| m.modified()) else {
        return true;
    };
    fn any_newer(dir: &Path, built: std::time::SystemTime) -> bool {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return false;
        };
        entries.flatten().any(|e| {
            let p = e.path();
            if p.is_dir() {
                return any_newer(&p, built);
            }
            e.metadata()
                .and_then(|m| m.modified())
                .map(|t| t > built)
                .unwrap_or(true)
        })
    }
    any_newer(&dir.join("src"), built)
}

#[cfg(test)]
mod prune_tests {
    use super::*;

    /// A staged plugin: a directory with a `Cargo.toml`, as xtask leaves behind.
    fn staged(dist: &Path, name: &str) -> PathBuf {
        let dir = dist.join("plugins").join(name);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        std::fs::write(dir.join("src").join("lib.rs"), "").unwrap();
        dir
    }

    /// The same, plus the sidecar the marketplace writes to claim ownership.
    fn installed(dist: &Path, name: &str) -> PathBuf {
        let dir = staged(dist, name);
        std::fs::write(dir.join("plugin.toml"), "name = \"x\"\n").unwrap();
        dir
    }

    fn temp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("renzora-prune-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn removes_a_staged_copy_whose_source_is_gone() {
        let root = temp("orphan");
        let src = root.join("plugins");
        std::fs::create_dir_all(&src).unwrap();
        let dist = root.join("dist");
        let orphan = staged(&dist, "was_deleted");

        prune_orphans(&src, &dist);

        assert!(!orphan.exists(), "an orphaned staged copy should be pruned");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn keeps_a_staged_copy_that_still_has_source() {
        let root = temp("kept");
        let src = root.join("plugins");
        std::fs::create_dir_all(src.join("still_here")).unwrap();
        let dist = root.join("dist");
        let live = staged(&dist, "still_here");

        prune_orphans(&src, &dist);

        assert!(live.exists(), "a plugin still in plugins/ must survive");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The regression this guard exists for: a plugin installed from the
    /// marketplace has no source in the repo, so the orphan rule matched it and
    /// `cargo renzora` deleted what had just been installed.
    #[test]
    fn keeps_a_marketplace_install_with_no_source_in_the_repo() {
        let root = temp("marketplace");
        let src = root.join("plugins");
        std::fs::create_dir_all(&src).unwrap();
        let dist = root.join("dist");
        let downloaded = installed(&dist, "renzora_lumen");

        prune_orphans(&src, &dist);

        assert!(
            downloaded.exists(),
            "a marketplace install must survive staging — it is not an orphan"
        );
        assert!(downloaded.join("src").join("lib.rs").is_file(), "its source must survive too");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Ownership is what decides it, not whether source happens to exist.
    #[test]
    fn the_sidecar_protects_even_when_a_same_named_source_exists() {
        let root = temp("both");
        let src = root.join("plugins");
        std::fs::create_dir_all(src.join("renzora_lumen")).unwrap();
        let dist = root.join("dist");
        let downloaded = installed(&dist, "renzora_lumen");

        prune_orphans(&src, &dist);

        assert!(downloaded.exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn ignores_directories_that_are_not_plugins() {
        let root = temp("unrelated");
        let src = root.join("plugins");
        std::fs::create_dir_all(&src).unwrap();
        let dist = root.join("dist");
        let other = dist.join("plugins").join("some_user_folder");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("notes.txt"), "hello").unwrap();

        prune_orphans(&src, &dist);

        assert!(other.exists(), "a directory with no Cargo.toml is not ours to touch");
        let _ = std::fs::remove_dir_all(&root);
    }
}
