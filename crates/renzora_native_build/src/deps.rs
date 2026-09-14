//! Third-party crates a native plugin declared, compiled by cargo.
//!
//! A native plugin is otherwise ONE crate compiled by a bare `rustc` against the
//! SDK — no dependency graph, which is what makes it build in about a second.
//! That leaves an obvious hole: a plugin could use nothing from crates.io at
//! all. This module closes it.
//!
//! # Why cargo can be trusted here, when it cannot be trusted with the plugin
//!
//! `cargo build` inside a native plugin directory is the one thing the plugin
//! docs forbid outright: `plugins/` is outside the workspace, so cargo resolves
//! a FRESH Bevy from crates.io, and the plugin that comes out has different
//! `TypeId`s from the engine. It builds, it loads, and it corrupts the World.
//!
//! The move here is to never let Bevy near cargo. A **separate manifest** is
//! synthesized carrying only the plugin's third-party dependencies —
//! `bevy`, `renzora` and every `renzora_*` are stripped out — and cargo builds
//! *that*. The plugin itself is still compiled by the same bare `rustc` as
//! before, still pointed at the SDK for Bevy and the contract crate; the rlibs
//! cargo produced are simply handed to it as extra `--extern`s.
//!
//! So the hazard is not avoided by discipline, it is unreachable: there is no
//! manifest anywhere in this path that mentions Bevy for cargo to resolve.
//!
//! # One toolchain, for both halves of the build
//!
//! The rlibs cargo produces here are read by the `rustc` that compiles the
//! plugin, and rustc refuses to read crate metadata written by a different
//! version of itself. So these two must be the SAME compiler, and "the same"
//! cannot be left to the environment to arrange.
//!
//! It will not arrange it. The plugin's own `rustc` is resolved by absolute path
//! through the SDK's pinned toolchain (see `renzora_plugin_build::toolchain`),
//! while cargo — spawned as a bare name — resolves through rustup's *default*,
//! which is whatever the machine happens to have set. A machine whose default is
//! `stable` and whose SDK pins 1.95.0 compiles the dependencies with one
//! compiler and reads them with another, and the build stops at
//! `error[E0514]: found crate 'sysinfo' compiled by an incompatible version of
//! rustc` — naming a crate the author did not choose the version of, and saying
//! nothing about the toolchain that actually differs.
//!
//! [`build`] therefore takes the SDK's rustc version and pins cargo to it, the
//! same way the plugin's own compiler is pinned. See [`Pinned`].
//!
//! # What a duplicate crate costs
//!
//! Nothing that matters. If a plugin asks for `serde` and the engine already
//! links its own, the plugin gets a second, privately linked copy. That is fine
//! for an ordinary library — the reason `renzora` and `renzora_ember` must be
//! shared is their process-global state (the translation table, the theme
//! palette), and a crate without such state has nothing to disagree about.
//!
//! And if such a type ever did try to cross into an engine API, the two copies
//! are different types to the compiler, so it fails at compile time with a type
//! mismatch. Loud, not silent — the opposite of the `TypeId` corruption above.
//!
//! # Opt-in by construction
//!
//! A plugin that declares nothing beyond `bevy`/`renzora` never reaches cargo at
//! all: [`build`] returns empty before running anything. That keeps the common
//! case exactly as fast and exactly as offline as it was — which matters,
//! because the SDK is otherwise self-contained and this is the one step that
//! needs a network.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::json;

/// The extra crates a plugin's own `rustc` invocation should be given.
#[derive(Debug, Default)]
pub struct Deps {
    /// `--extern <name>=<rlib>`, one per DIRECT dependency.
    ///
    /// Only the direct ones: those are the names the plugin can `use`.
    /// Everything transitive is found through [`Deps::search`] instead, the same
    /// way the SDK's own metadata files are.
    pub externs: Vec<(String, PathBuf)>,
    /// The `-L dependency=` path holding every rlib cargo produced.
    pub search: Option<PathBuf>,
}

/// Crates the SDK already provides, which are stripped from the synthesized
/// manifest because cargo would otherwise resolve its own copy.
///
/// Exactly the three a plugin is handed by `--extern`, plus the `renzora_*`
/// prefix so a future shared image is covered without editing this.
///
/// Deliberately NOT a `bevy_*` prefix. A plugin that writes `bevy_ecs = "0.19"`
/// has made a mistake — the SDK's Bevy is reached through the `bevy` facade —
/// and silently substituting the right thing would teach it to keep making that
/// mistake. It falls through to [`is_engine_crate`] and is refused by name.
fn is_sdk_crate(name: &str) -> bool {
    name == "bevy" || name == "renzora" || name.starts_with("renzora_")
}

/// Crates that must not appear ANYWHERE in the resolved dependency graph.
///
/// Broader than [`is_sdk_crate`], and the difference is the whole point of the
/// guard. A second `bevy` is the obvious hazard, but the engine shares its types
/// through ~90 **subcrates** — `bevy_ecs`, `bevy_app`, `bevy_transform` — every
/// one of which the SDK stages. A dependency pulling `bevy_ecs` directly gives
/// the plugin a second `World` and `Entity`, which is the same corruption as a
/// second `bevy` and is not caught by looking for the facade alone.
///
/// The prefix match can in principle reject a third-party crate that merely
/// *names* itself `bevy_something` without depending on Bevy. That is a rare and
/// harmless false positive: such a crate is almost always a Bevy integration,
/// and if it genuinely is not, it would still be refused for its name rather
/// than for a real conflict — an error the author can read and act on, unlike
/// the silent `TypeId` mismatch the guard exists to prevent.
fn is_engine_crate(name: &str) -> bool {
    name == "bevy" || name.starts_with("bevy_") || name == "renzora" || name.starts_with("renzora_")
}

/// Whether this plugin's manifest names a dependency that is not the SDK's.
///
/// Cheap — it reads the manifest and parses nothing else — and it answers a
/// question worth asking before scheduling: a plugin with third-party
/// dependencies runs [`build`], which invokes **cargo** over a whole dependency
/// tree. That is a different order of cost from the single `rustc` every other
/// plugin needs, and the four that have any take long enough that one left to
/// the end of a parallel build reads as a hang.
pub fn has_third_party(plugin_dir: &Path) -> bool {
    std::fs::read_to_string(plugin_dir.join("Cargo.toml"))
        .ok()
        .and_then(|text| third_party_lines(&text).ok())
        .is_some_and(|deps| !deps.is_empty())
}

/// Compile `plugin_dir`'s third-party dependencies into `build_dir/deps`.
///
/// Returns empty — having run nothing — when the plugin declares none.
///
/// `toolchain` is the SDK manifest's `rustc` (e.g. `"1.95.0"`), and every cargo
/// this runs is pinned to it. It is not a hint: the rlibs produced here are read
/// by a `rustc` of exactly that version, and metadata from any other is refused.
/// See [`Pinned`] and the module docs.
pub fn build(plugin_dir: &Path, build_dir: &Path, toolchain: &str) -> Result<Deps, String> {
    let manifest = plugin_dir.join("Cargo.toml");
    let Ok(text) = std::fs::read_to_string(&manifest) else {
        return Ok(Deps::default());
    };
    let wanted = third_party_lines(&text)?;
    if wanted.is_empty() {
        return Ok(Deps::default());
    }

    // Resolved BEFORE the directory is touched, so a machine that cannot supply
    // the pinned compiler fails having written nothing — and says which
    // compiler, rather than leaving a half-built tree and an E0514 to come.
    let pinned = Pinned::resolve(toolchain)?;

    let dir = build_dir.join("deps");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    pinned.invalidate_stale_cache(&dir);
    write_manifest(&dir, &wanted)?;

    reject_engine_crates(&dir, &pinned)?;
    let artifacts = compile(&dir, &pinned)?;

    // Match cargo's artifacts back to the DIRECT dependency names. Cargo reports
    // a crate name, which is the dependency name with hyphens turned into
    // underscores — `wasm-bindgen` builds as `wasm_bindgen` — so both sides are
    // normalised before comparing.
    let direct: Vec<String> = wanted.iter().map(|(n, _)| n.replace('-', "_")).collect();
    let mut externs = Vec::new();
    for (name, rlib) in artifacts {
        if direct.contains(&name) {
            externs.push((name, rlib));
        }
    }
    Ok(Deps { externs, search: Some(dir.join("target").join("release").join("deps")) })
}

/// A cargo pinned to one Rust toolchain, and the `rustc` it must drive.
///
/// # Why pinning cargo is not enough on its own
///
/// Two separate things decide which compiler builds these rlibs, and setting
/// either alone leaves a hole.
///
/// `RUSTUP_TOOLCHAIN` picks the toolchain a rustup **shim** resolves to. It
/// outranks a directory override and a `rust-toolchain.toml`, which matters
/// because plugins live in a directory the user owns and may have put one of
/// those above — measured: with `channel = "1.93.0"` in a parent directory,
/// `RUSTUP_TOOLCHAIN=1.95.0 rustc -vV` still reports 1.95.0. But it is only read
/// by a shim, and [`crate::tool`] deliberately prefers `$CARGO` — which cargo
/// sets, to an absolute non-shim path, for everything it spawns. An editor
/// launched by `cargo renzora` therefore hands this a cargo that ignores the
/// variable entirely.
///
/// `RUSTC` decides which compiler *that* cargo invokes, whichever cargo it is.
/// Measured: a 1.94.0 cargo with `RUSTC` pointing at 1.95.0 produces an rlib
/// stamped `rustc 1.95.0`. It closes the hole the variable above leaves, and it
/// also displaces an inherited `$RUSTC` pointing at the toolchain that built the
/// editor rather than the one the SDK was staged with.
///
/// Both are set, from one resolved answer, so the two mechanisms cannot disagree
/// with each other.
struct Pinned {
    /// The cargo to spawn — absolute when rustup could name it.
    cargo: PathBuf,
    /// The version it is pinned to, e.g. `1.95.0`.
    version: String,
    /// The compiler cargo must use. `None` only when rustup could not name one
    /// and the compiler already on `PATH` was verified to be the right version.
    rustc: Option<PathBuf>,
}

impl Pinned {
    /// Find cargo for `version`, or say why the machine cannot supply it.
    ///
    /// rustup first, because it is the only thing that can be *asked* for a
    /// specific version — the same order, and for the same reason, as
    /// `renzora_plugin_build::toolchain::resolve` uses for the plugin's own
    /// compiler.
    ///
    /// The fallback is a machine with no rustup at all, where Rust was installed
    /// some other way. There is exactly one compiler there and nothing can
    /// redirect it, so the only useful question is whether it happens to be the
    /// right one — asked here, and refused loudly if not, rather than left for
    /// rustc to discover as an E0514 against a crate the author never picked.
    fn resolve(version: &str) -> Result<Self, String> {
        if let Some(cargo) = rustup_which(version, "cargo") {
            return Ok(Self {
                cargo,
                version: version.to_string(),
                rustc: rustup_which(version, "rustc"),
            });
        }
        let found = path_rustc_release();
        if found.as_deref() == Some(version) {
            let cargo = crate::tool("cargo");
            return Ok(Self { cargo, version: version.to_string(), rustc: None });
        }
        Err(format!(
            "this plugin's dependencies must be compiled by Rust {version} — the \
             version the SDK was built with — but the compiler here is {}.\n\
             Install it with `rustup toolchain install {version}`.\n\
             (Building them with any other version produces crate metadata this \
             SDK's rustc refuses to read, which surfaces as `error[E0514]` \
             against one of the dependencies.)",
            found.as_deref().unwrap_or("not something this could identify"),
        ))
    }

    /// A `Command` for the pinned cargo, with the environment already applied.
    fn command(&self) -> Command {
        let mut cmd = Command::new(&self.cargo);
        crate::hide_console(&mut cmd);
        cmd.env("RUSTUP_TOOLCHAIN", &self.version);
        match &self.rustc {
            Some(rustc) => {
                cmd.env("RUSTC", rustc);
            }
            // Nothing to point at, so an inherited one must not be left to take
            // effect: it would name the toolchain that built whatever spawned
            // the editor, which is the mismatch this type exists to prevent.
            None => {
                cmd.env_remove("RUSTC");
            }
        }
        cmd
    }

    /// Drop a `target/` left behind by a different toolchain.
    ///
    /// A successful build deletes `deps/` wholesale (see the note in
    /// `renzora_plugin_build`), so this only ever sees what a **failed** one
    /// left — and a failure is precisely when the tree is most likely to have
    /// been built by the wrong compiler. Without this, fixing the toolchain and
    /// building again reuses those rlibs and fails identically, which reads as
    /// the fix not having worked.
    ///
    /// Keyed on a stamp of our own rather than on cargo's `.rustc_info.json`:
    /// that file is cargo's private format, and cargo's own invalidation did not
    /// catch this case to begin with.
    fn invalidate_stale_cache(&self, dir: &Path) {
        let stamp = dir.join(".toolchain");
        if std::fs::read_to_string(&stamp).is_ok_and(|s| s.trim() == self.version) {
            return;
        }
        let _ = std::fs::remove_dir_all(dir.join("target"));
        let _ = std::fs::write(&stamp, format!("{}\n", self.version));
    }
}

/// Ask rustup for the absolute path to one toolchain's `cargo` or `rustc`.
///
/// `None` when rustup is absent or does not have that toolchain — the caller
/// treats both the same way, so they are not distinguished.
fn rustup_which(version: &str, bin: &str) -> Option<PathBuf> {
    let out = crate::hide_console(&mut Command::new(crate::tool("rustup")))
        .args(["which", "--toolchain", version, bin])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let path = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim().to_string());
    path.is_file().then_some(path)
}

/// The release string of whatever `rustc` resolves to here, e.g. `1.95.0`.
fn path_rustc_release() -> Option<String> {
    let out = crate::hide_console(&mut Command::new(crate::tool("rustc")))
        .arg("-vV")
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("release: "))
        .map(|s| s.trim().to_string())
}

/// The `[dependencies]` entries that are NOT the engine's, verbatim.
///
/// Line-based rather than parsed, because this crate has no TOML parser and
/// carries no dependencies at all. Every native plugin manifest writes one
/// dependency per line, which makes copying the line the exact operation wanted:
/// whatever the author put on the right-hand side (a version, a feature list, a
/// git ref) carries over untouched, and this code never has to understand it.
///
/// The one shape that would break it is the `[dependencies.foo]` sub-table, so
/// that is refused explicitly rather than silently ignored — a dependency
/// dropped without a word would surface as a confusing "cannot find crate" much
/// later, pointing at the plugin's source instead of at its manifest.
fn third_party_lines(text: &str) -> Result<Vec<(String, String)>, String> {
    if let Some(line) = text.lines().find(|l| l.trim_start().starts_with("[dependencies.")) {
        return Err(format!(
            "`{}` — a native plugin's dependencies must be written one per line \
             (`foo = {{ version = \"1\" }}`), not as a sub-table",
            line.trim()
        ));
    }
    let mut out = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            // `[dependencies]` only. `[dev-dependencies]` and `[build-dependencies]`
            // are deliberately not honoured: nothing here runs tests or build
            // scripts for the plugin, so accepting them would download and
            // compile crates that could never be used.
            inside = t == "[dependencies]";
            continue;
        }
        if !inside || t.is_empty() || t.starts_with('#') {
            continue;
        }
        let Some((name, _)) = t.split_once('=') else {
            continue;
        };
        let name = name.trim().trim_matches('"').to_string();
        if name.is_empty() || is_sdk_crate(&name) {
            continue;
        }
        out.push((name, t.to_string()));
    }
    Ok(out)
}

/// Write the synthesized deps-only crate.
///
/// `[workspace]` makes it a workspace ROOT. Without it cargo walks upwards
/// looking for one and can adopt a directory it has no business being part of —
/// and this lives under a staged `plugins/<name>/build/`, which is exactly the
/// kind of place an unrelated manifest might sit above.
fn write_manifest(dir: &Path, deps: &[(String, String)]) -> Result<(), String> {
    let body: String = deps.iter().map(|(_, line)| format!("{line}\n")).collect();
    std::fs::write(
        dir.join("Cargo.toml"),
        format!(
            "# GENERATED — the plugin's third-party dependencies, and nothing else.\n\
             # `bevy`/`renzora*` are stripped on purpose: see `deps.rs`. Editing this\n\
             # file does nothing; it is rewritten from the plugin's Cargo.toml.\n\
             [workspace]\n\n\
             [package]\n\
             name = \"plugin_deps\"\n\
             version = \"0.1.0\"\n\
             edition = \"2021\"\n\n\
             [lib]\n\
             path = \"lib.rs\"\n\
             crate-type = [\"rlib\"]\n\n\
             [dependencies]\n{body}"
        ),
    )
    .map_err(|e| e.to_string())?;
    // A crate has to have a root module. Nothing ever calls into it — its only
    // job is to give cargo a reason to build the dependency graph.
    std::fs::write(dir.join("lib.rs"), "// Anchor for the dependency graph.\n")
        .map_err(|e| e.to_string())
}

/// Refuse a dependency graph that contains the engine's own crates.
///
/// The whole design rests on Bevy never being resolved by cargo, and a *direct*
/// `bevy` entry is already stripped. This catches the indirect case: a plugin
/// depending on some crate that itself depends on Bevy would pull a second Bevy
/// compilation in through the back door, and produce precisely the plugin that
/// loads and corrupts the World.
///
/// `cargo metadata` resolves the graph without compiling any of it — measured at
/// ~12 s against a Bevy-pulling manifest — so the refusal costs seconds rather
/// than the half-hour a Bevy build would have taken before failing.
fn reject_engine_crates(dir: &Path, pinned: &Pinned) -> Result<(), String> {
    let out = pinned
        .command()
        .current_dir(dir)
        .args(["metadata", "--format-version", "1", "--quiet"])
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("could not run cargo: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "`cargo metadata` failed ({})\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut found: Vec<String> = Vec::new();
    for name in json::all_strings(&text, "name") {
        if is_engine_crate(&name) && !found.contains(&name) {
            found.push(name);
        }
    }
    if !found.is_empty() {
        found.sort();
        return Err(format!(
            "the plugin's dependencies pull in the engine's own crates ({}).\n\
             A native plugin already links Bevy and the contract crate from the \
             SDK as shared images. A second compilation of them would have \
             different `TypeId`s, so the plugin would build, load, and then read \
             the engine's `World` through the wrong layouts.\n\
             Fix it by dropping that dependency, or by reaching the same types \
             through `bevy::` and `renzora::`, which the SDK provides.",
            found.join(", ")
        ));
    }
    Ok(())
}

/// Build the deps crate and return `(crate name, rlib)` for everything produced.
///
/// `--release` so a plugin's dependencies are optimised like the plugin itself
/// (`-C opt-level=2`) and like the engine they run inside; a debug dependency
/// under an optimised caller is a performance cliff nobody would think to look
/// for.
///
/// The file list comes from cargo's own artifact messages and NEVER from reading
/// the directory — the same rule the SDK staging follows, and for the same
/// reason: `target/release/deps/` accumulates several `-C metadata` variants of
/// one crate, and picking by name produces a set that looks right and then fails
/// to compile.
fn compile(dir: &Path, pinned: &Pinned) -> Result<Vec<(String, PathBuf)>, String> {
    // `stderr` is PIPED, not inherited: an inherited stderr is what forces a
    // console to exist for the child, which is the window `hide_console` is
    // suppressing. Cargo's progress lines go into the pipe and are dropped; its
    // diagnostics still reach the caller through the non-zero exit below.
    // `--target-dir` is passed EXPLICITLY, and it is not a preference. The caller
    // reports `<dir>/target/release/deps` as the `-L dependency=` search path,
    // and without this that is an assumption about cargo's default rather than a
    // fact: cargo discovers config by walking up from the working directory, so
    // any `.cargo/config.toml` above the plugin — anywhere, including one a user
    // put beside their own plugins — can set `build.target-dir` and silently
    // redirect the output.
    //
    // The failure that reaches the author is not "wrong directory". It is
    // `error[E0460]: found possibly newer version of crate X`, because the search
    // path then points at whatever a PREVIOUS run left behind while the fresh
    // rlibs sit somewhere else, and rustc resolves one dependency from each. The
    // message names a crate the plugin never mentioned and says nothing about a
    // target directory.
    let out = pinned
        .command()
        .current_dir(dir)
        .args(["build", "--release", "--message-format=json-render-diagnostics"])
        .arg("--target-dir")
        .arg(dir.join("target"))
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("could not run cargo: {e}"))?;
    if !out.status.success() {
        // Carry cargo's own diagnostics: stderr is piped now (see above), so
        // this is the only place they can reach the author.
        return Err(format!(
            "building the plugin's dependencies failed ({})\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut found = Vec::new();
    for line in text.lines() {
        if !line.contains("\"reason\":\"compiler-artifact\"") {
            continue;
        }
        // The first `"name"` in a compiler-artifact line is the TARGET's, which
        // is the crate name — what `--extern` has to be keyed by, and what
        // differs from the package name whenever a package renames its lib.
        let Some(name) = json::string(line, "name") else {
            continue;
        };
        if let Some(rlib) = json::string_array(line, "filenames").into_iter().find(|f| f.ends_with(".rlib")) {
            found.push((name, PathBuf::from(rlib)));
        }
    }
    Ok(found)
}
