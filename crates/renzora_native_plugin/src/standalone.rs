//! Building a **standalone** (C-ABI) plugin, where it is installed.
//!
//! The counterpart to [`renzora_plugin_build::Sdk`], and almost none of it. A
//! native plugin links the engine's own Bevy, so building one means driving
//! `rustc` by hand against ~444 MB of staged crate metadata pinned to one exact
//! toolchain. A standalone plugin links nothing: it is an ordinary cargo project
//! whose only dependency is `renzora_plugin`, so the build is `cargo build` and
//! the toolchain is whatever the user has.
//!
//! That inverts the rule the native side lives under. `cargo` must *never* be
//! pointed at a native plugin — outside the engine workspace it would resolve a
//! second Bevy from crates.io, with different `TypeId`s, producing a plugin that
//! compiles, loads and corrupts the World. Here cargo is exactly right, and
//! hand-driving `rustc` would be the mistake: these have real dependency graphs
//! (a proc macro, and whatever the author added), and resolving one is the job
//! cargo exists to do.
//!
//! ## What it compiles against
//!
//! `<install>/sdk/plugin-api/renzora_plugin`, staged as source. It lives inside
//! the SDK tree so the two travel together: the SDK ships as one `sdk.tar.zst`
//! that is unpacked by replacing `sdk/` wholesale, so anything staged beside it
//! rather than inside it would be deleted by the next update.
//!
//! It is not *part* of the SDK in any other sense — 1.1 MB of `.rs` against 3.1
//! GB of crate metadata, and needed in exactly the case the SDK is not.
//!
//! A plugin's manifest cannot name that path itself: what an author writes is
//! `path = "../../crates/renzora_plugin"`, which is where the crate sits in a
//! source checkout and nowhere at all in an install. So [`repoint_contract`]
//! fixes the dependency before building, which is more robust than any layout
//! that made the author's path happen to resolve — a plugin written outside this
//! repository can say anything, and this handles all of it.
//!
//! ## Why the stamp is not the SDK's
//!
//! A native plugin's artefact is bound to the images it was built against, so it
//! is rebuilt whenever the engine moves. A standalone one is bound to nothing: it
//! keeps loading into every later editor whose ABI MAJOR matches, which is the
//! entire promise of the C ABI. Rebuilding it on an engine update would do work
//! to produce a byte-identical file — and would demand a toolchain the user may
//! no longer have installed. So the stamp records the compiler, and only an
//! edit or a new rustc triggers a rebuild.

use std::path::Path;
use std::process::{Command, Stdio};

/// What a standalone artefact is bound to: the compiler that produced it.
///
/// Deliberately not the engine version. See the module docs.
pub fn stamp() -> String {
    Command::new(renzora_native_build::tool("rustc"))
        .arg("-V")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "rustc-unknown".to_string())
}

/// Is a Rust toolchain available at all?
///
/// Checked before a build is attempted so "you need Rust installed" can be said
/// once, plainly, instead of arriving as a failed build per plugin.
pub fn have_toolchain() -> bool {
    Command::new(renzora_native_build::tool("cargo"))
        .arg("-V")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Compile the plugin rooted at `dir` and put its library at `out`.
///
/// Reports each line the compiler writes, for a caller with somewhere to show
/// progress — a cold build pulls the derive crate's `syn`/`quote` and is the one
/// case where this takes more than a second or two.
///
/// On success returns the stamp to record beside the artefact.
pub fn compile(
    dir: &Path,
    out: &Path,
    on_line: &mut dyn FnMut(&str),
) -> Result<String, String> {
    // Before anything is spawned: cargo cannot even parse a manifest whose path
    // dependency does not exist, so this is the difference between building and
    // a bare "failed to read …/Cargo.toml".
    repoint_contract(dir)?;

    // The author's own profile, not `--release`. 59 of the plugins shipped with
    // the engine are `#![no_std]`, which cannot unwind on stable and therefore
    // sets `panic = "abort"` under `[profile.dist]`. Built under plain release
    // every one of them fails to link, with nothing in the error naming a
    // profile as the reason.
    let profile = if declares_dist_profile(dir) { "dist" } else { "release" };

    // One target directory for every standalone plugin, named explicitly rather
    // than arranged by a `.cargo/config.toml` sitting above them.
    //
    // Sharing it is worth real time: each of these compiles `renzora_plugin` and
    // the derive crate's `syn`/`quote` behind it, which is seconds per plugin
    // paid once for the set instead of once each. But a config file buys that by
    // redirecting EVERY cargo run underneath it, including the one that builds a
    // *native* plugin's third-party dependencies — and that one reports its own
    // output path to rustc, so the redirect surfaces as `error[E0460]` naming a
    // crate nobody mentioned. Explicit here, so it applies to exactly the builds
    // it was meant for.
    let target_dir = target_dir(dir);
    let mut child = Command::new(renzora_native_build::tool("cargo"))
        .current_dir(dir)
        .args(["build", "--profile", profile])
        .arg("--target-dir")
        .arg(&target_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run cargo: {e}"))?;

    // stderr, because that is where cargo's diagnostics and progress go.
    if let Some(err) = child.stderr.take() {
        use std::io::BufRead;
        for line in std::io::BufReader::new(err).lines().map_while(Result::ok) {
            if !line.trim().is_empty() {
                on_line(&line);
            }
        }
    }
    let status = child.wait().map_err(|e| format!("cargo: {e}"))?;
    if !status.success() {
        return Err("failed to compile".to_string());
    }

    stage(dir, &target_dir, profile, out)?;
    Ok(stamp())
}

/// Copy what cargo produced to where the loader looks.
///
/// The rename matters. Cargo emits a cdylib as `lib<crate>.so` on Unix and
/// `<crate>.dll` on Windows, so an artefact kept under cargo's name gives one
/// plugin two identities depending on the platform — while the disable list, the
/// reload queue, the update check and the marketplace sidecar all key on the
/// directory name. `build/<crate>.<ext>` is the one layout, and it is the same
/// one a native plugin's `rustc` invocation writes.
fn stage(dir: &Path, target_dir: &Path, profile: &str, out: &Path) -> Result<(), String> {
    let ext = std::env::consts::DLL_EXTENSION;
    // Cargo names the artefact after the PACKAGE; the loader identifies a plugin
    // by its DIRECTORY. Normally those agree, and the one case where they cannot
    // is the one the marketplace exists to create: two sellers ship a crate
    // called `vignette`, so the second is installed as `vignette_2` to keep the
    // first from being overwritten, while its manifest still says `vignette`.
    // Reading the directory name here would look for a file cargo never wrote.
    //
    // (The native side has no equivalent problem — its `rustc` invocation passes
    // `--crate-name <dir>`, so the artefact is named after the directory by
    // construction.)
    let package = package_name(dir).unwrap_or_else(|| crate::name_of(dir)).replace('-', "_");
    let file = format!("{}{package}.{ext}", std::env::consts::DLL_PREFIX);

    // Exactly one place to look, because [`compile`] told cargo where to build.
    // Searching a list of plausible directories is how a stale artefact from an
    // earlier layout gets staged in place of the one just built.
    let built = target_dir.join(profile).join(&file);
    if !built.is_file() {
        return Err(format!(
            "built, but produced no {file} in {} — is it a cdylib, and does the package \
             name match the folder?",
            target_dir.join(profile).display()
        ));
    }

    renzora_native_build::stage_atomically(&built, out)
        .map_err(|e| format!("staging {}: {e}", out.display()))?;
    Ok(())
}

/// The `[package] name` a manifest declares.
///
/// Line-based, like the rest of the manifest reading on this path: these crates
/// carry no TOML parser, and the one field wanted is written one way by every
/// manifest cargo will accept. Stops at the next table header so a `name` under
/// `[dependencies.foo]` or `[[bin]]` cannot be mistaken for the package's.
fn package_name(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("Cargo.toml")).ok()?;
    let mut in_package = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_package = t == "[package]";
            continue;
        }
        if !in_package || !t.starts_with("name") {
            continue;
        }
        if let Some((_, rest)) = t.split_once('=') {
            return Some(rest.trim().trim_matches('"').to_string());
        }
    }
    None
}

/// The shared build directory for standalone plugins: `<plugins>/target`.
///
/// Beside the plugins rather than inside each one, so the API crate and the
/// derive macro's dependencies compile once for the whole set. Falls back to a
/// per-plugin directory for a plugin that somehow has no parent.
fn target_dir(dir: &Path) -> std::path::PathBuf {
    dir.parent().map(|p| p.join("target")).unwrap_or_else(|| dir.join("target"))
}

/// Does the manifest declare `[profile.dist]`?
fn declares_dist_profile(dir: &Path) -> bool {
    std::fs::read_to_string(dir.join("Cargo.toml"))
        .map(|t| t.lines().any(|l| l.trim_start().starts_with("[profile.dist]")))
        .unwrap_or(false)
}

/// Where the plugin API is staged, relative to a plugin directory.
///
/// `<install>/plugins/<name>/` → `<install>/sdk/plugin-api/renzora_plugin`.
/// Relative rather than absolute so a manifest written by one install still
/// resolves after the folder is moved or copied to another machine — which
/// matters because this string is written INTO the plugin's manifest, and that
/// manifest is what gets published if the plugin is later uploaded.
const API_FROM_PLUGIN: &str = "../../sdk/plugin-api/renzora_plugin";

/// Point a plugin's `renzora_plugin` dependency at the staged plugin API.
///
/// Only when it does not already resolve, and only by writing a file whose
/// content actually changed — both matter. A plugin being developed inside an
/// engine checkout has a path that works and must be left exactly alone; and an
/// unconditional write would move the manifest's mtime, which is one of the two
/// things [`crate::layout`] uses to decide a plugin is stale. Rewriting on every
/// build would therefore make every plugin rebuild on every launch, forever.
pub fn repoint_contract(dir: &Path) -> Result<(), String> {
    let manifest = dir.join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|e| format!("{}: {e}", manifest.display()))?;

    // Already resolvable — an in-checkout plugin, or one repointed earlier.
    if let Some(path) = declared_path(&text) {
        if dir.join(&path).join("Cargo.toml").is_file() {
            return Ok(());
        }
    }

    let Some(fixed) = repoint(&text, API_FROM_PLUGIN) else {
        return Err(
            "manifest declares no `renzora_plugin` dependency — not a standalone plugin".into(),
        );
    };
    if fixed == text {
        return Ok(());
    }
    std::fs::write(&manifest, fixed).map_err(|e| format!("{}: {e}", manifest.display()))
}

/// The `path` a manifest's `renzora_plugin` dependency declares, if any.
fn declared_path(manifest: &str) -> Option<String> {
    let line = dep_line(manifest)?;
    let rest = line.split_once("path")?.1.split_once('"')?.1;
    Some(rest.split('"').next()?.to_string())
}

/// The `renzora_plugin = …` line, ignoring one inside a comment.
fn dep_line(manifest: &str) -> Option<&str> {
    manifest.lines().find(|l| {
        let t = l.trim_start();
        !t.starts_with('#') && t.starts_with("renzora_plugin") && t.contains('=')
    })
}

/// Rewrite the dependency to carry `path`, preserving everything else it says.
///
/// Three forms reach this, and the features on them are not decoration:
/// `features = ["libm"]` with `default-features = false` is what makes a
/// `no_std` plugin compile at all, so a rewrite that dropped them would turn a
/// working plugin into a wall of missing-`sqrt` errors.
fn repoint(manifest: &str, path: &str) -> Option<String> {
    let target = dep_line(manifest)?.to_string();
    let (before, _) = target.split_once('=')?;
    let value = target.split_once('=')?.1.trim();

    let fixed = if let Some((head, tail)) = value.split_once("path") {
        // `{ path = "…", … }` — substitute the value between the next quotes.
        let (_, after_open) = tail.split_once('"')?;
        let (_, rest) = after_open.split_once('"')?;
        format!("{before}={head}path = \"{path}\"{rest}")
    } else if let Some(inner) = value.strip_prefix('{') {
        // `{ version = "1", features = [..] }` — add a path to the table.
        format!("{before}= {{ path = \"{path}\",{}", inner)
    } else {
        // `renzora_plugin = "1"` — a bare version.
        format!("{before}= {{ path = \"{path}\" }}")
    };
    Some(manifest.replace(&target, &fixed))
}

#[cfg(test)]
mod tests {
    use super::{declared_path, repoint};

    const API: &str = "../../sdk/plugin-api/renzora_plugin";

    /// The form every plugin in this repository uses. The features are the part
    /// worth pinning: `default-features = false` + `features = ["libm"]` is what
    /// makes a `no_std` plugin compile at all, so a rewrite that dropped them
    /// would turn a working plugin into a wall of missing-`sqrt` errors.
    #[test]
    fn a_checkout_path_is_repointed_and_features_survive() {
        let out = repoint(
            "[dependencies]\nrenzora_plugin = { path = \"../../crates/renzora_plugin\", \
             default-features = false, features = [\"libm\"] }\n",
            API,
        )
        .unwrap();
        assert!(out.contains(&format!("path = \"{API}\"")), "{out}");
        assert!(out.contains("default-features = false"), "{out}");
        assert!(out.contains("features = [\"libm\"]"), "{out}");
        assert_eq!(declared_path(&out).as_deref(), Some(API));
    }

    /// What a plugin authored outside this repository says once the crate is
    /// published. It has no path at all, so one is added rather than substituted.
    #[test]
    fn a_bare_version_gains_a_path() {
        let out = repoint("[dependencies]\nrenzora_plugin = \"1\"\n", API).unwrap();
        assert_eq!(declared_path(&out).as_deref(), Some(API));

        let table = repoint(
            "[dependencies]\nrenzora_plugin = { version = \"1\", features = [\"anim\"] }\n",
            API,
        )
        .unwrap();
        assert_eq!(declared_path(&table).as_deref(), Some(API));
        assert!(table.contains("features = [\"anim\"]"), "{table}");
    }

    /// Repointing twice must produce the same bytes. `repoint_contract` only
    /// writes when the content changed, and that check is what stops every
    /// launch from moving the manifest's mtime — which `layout` reads as "the
    /// author edited it" and answers with a rebuild of every plugin, forever.
    #[test]
    fn repointing_is_idempotent() {
        let once = repoint(
            "[dependencies]\nrenzora_plugin = { path = \"../../crates/renzora_plugin\" }\n",
            API,
        )
        .unwrap();
        assert_eq!(repoint(&once, API).unwrap(), once);
    }

    /// A commented-out dependency is not a dependency.
    #[test]
    fn a_manifest_without_the_dependency_is_refused() {
        assert!(repoint("[dependencies]\n# renzora_plugin = \"1\"\nlibm = \"0.2\"\n", API)
            .is_none());
    }

    /// The marketplace collision: a second seller's `vignette` is installed as
    /// `vignette_2` so the first is not overwritten, but cargo still names the
    /// library after the package. Staging by directory name looks for a file
    /// that was never written.
    #[test]
    fn the_artefact_is_found_by_package_name_not_folder_name() {
        let dir = std::env::temp_dir().join(format!("renzora_pkg_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"vignette\"\nversion = \"0.1.0\"\n\n             [dependencies]\nnoise = { version = \"1\", package = \"other\" }\n",
        )
        .unwrap();
        assert_eq!(super::package_name(&dir).as_deref(), Some("vignette"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

