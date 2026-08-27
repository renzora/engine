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
//! `--extern bevy` must be the facade *rlib*, `--extern renzora` must be the
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
    dependency: Vec<PathBuf>,
    native: Vec<PathBuf>,
}

impl Sdk {
    fn parse(flat: &str, dist_root: &Path) -> Option<Self> {
        let root = dist_root.join("sdk");
        let rustc = json_string(flat, "rustc")?;
        let bevy = json_string(flat, "bevy")?;
        let renzora = json_string(flat, "renzora")?;
        let bevy_name = Path::new(&bevy).file_name()?.to_str()?.to_string();
        Some(Sdk {
            lib_ext: json_string(flat, "lib_ext")?,
            triple: json_string(flat, "triple")?,
            stamp: format!("{bevy_name}+rustc-{rustc}"),
            extern_bevy: root.join(&bevy),
            extern_renzora: root.join(&renzora),
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

    println!("[xtask] rustc --crate-type dylib ({name})");
    let mut cmd = Command::new("rustc");
    cmd
        // Bevy's derives resolve their own crate paths through `BevyManifest`,
        // which reads `$CARGO_MANIFEST_DIR/Cargo.toml`. Nothing runs cargo here,
        // so without these a `#[derive(Component)]` or `bsn!` fails as a bare
        // "proc macro panicked" naming the macro rather than the missing var.
        .env("CARGO_MANIFEST_DIR", &out_dir)
        .env("CARGO_PKG_NAME", name.replace('-', "_"))
        .args(["--edition", "2021", "--crate-type", "dylib"])
        // Without this, rustc names the crate after the FILE — every plugin
        // becomes `lib`, and every log line it emits is tagged `INFO lib:`,
        // indistinguishable from every other plugin's.
        .args(["--crate-name", &name.replace('-', "_")])
        .args(["-C", "prefer-dynamic"])
        // A bare `rustc` defaults to `opt-level=0`, so without this every plugin
        // built here runs unoptimised. 2 matches `[profile.dist]` — the same
        // level the engine it calls into was built at — and measured 224 KB ->
        // 109 KB on a small script with no change in build time.
        .args(["-C", "opt-level=2"]);

    // `.cargo/config.toml` pins this for cargo builds; a bare rustc would fall
    // back to MSVC `link.exe`, which fails on the exported-symbol count.
    if sdk.triple.contains("windows-msvc") {
        cmd.args(["-C", "linker=rust-lld"]);
    }

    cmd.arg("--extern").arg(format!("bevy={}", sdk.extern_bevy.display()));
    cmd.arg("--extern").arg(format!("renzora={}", sdk.extern_renzora.display()));
    for d in &sdk.dependency {
        cmd.arg("-L").arg(format!("dependency={}", d.display()));
    }
    for n in &sdk.native {
        cmd.arg("-L").arg(format!("native={}", n.display()));
    }
    cmd.arg("-o").arg(&lib).arg(out_dir.join("src").join("lib.rs"));

    let ok = cmd.status().map(|s| s.success()).unwrap_or(false);
    if !ok {
        eprintln!("[xtask] {name}: failed to compile");
        return false;
    }
    if let Err(e) = std::fs::write(&stamp_file, &sdk.stamp) {
        eprintln!("[xtask] {name}: writing stamp: {e}");
        return false;
    }
    true
}

/// Copy `Cargo.toml` and `src/` into the staged plugin directory.
///
/// `build/` is left alone — it holds the previous artefact and stamp, which is
/// what makes the "nothing changed" path skip the compile.
fn mirror_source(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst.join("src"))?;
    let manifest = src.join("Cargo.toml");
    if manifest.is_file() {
        crate::copy(&manifest, &dst.join("Cargo.toml"))?;
    }
    let from = src.join("src");
    // One level. A plugin with submodules is a fair thing to want and this is
    // where to grow support for it; today `lib.rs` is the whole contract.
    for e in std::fs::read_dir(&from)?.flatten() {
        let p = e.path();
        if p.is_file() {
            crate::copy(&p, &dst.join("src").join(e.file_name()))?;
        }
    }
    Ok(())
}

/// Whether anything under `dir/src` is newer than `target`.
fn newer_than(dir: &Path, target: &Path) -> bool {
    let Ok(built) = std::fs::metadata(target).and_then(|m| m.modified()) else {
        return true;
    };
    let Ok(entries) = std::fs::read_dir(dir.join("src")) else {
        return false;
    };
    entries.flatten().any(|e| {
        e.metadata()
            .and_then(|m| m.modified())
            .map(|t| t > built)
            .unwrap_or(true)
    })
}
