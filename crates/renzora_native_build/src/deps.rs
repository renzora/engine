//! Third-party crates a native plugin declared, compiled by cargo.
//!
//! A native plugin is otherwise ONE crate compiled by a bare `rustc` against the
//! SDK — no dependency graph, which is what makes it build in about a second.
//! That leaves an obvious hole: a plugin cannot use anything from crates.io,
//! while a C-ABI plugin (built by cargo, linking no Bevy) can use whatever it
//! likes. This module closes it.
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

/// Compile `plugin_dir`'s third-party dependencies into `build_dir/deps`.
///
/// Returns empty — having run nothing — when the plugin declares none.
pub fn build(plugin_dir: &Path, build_dir: &Path) -> Result<Deps, String> {
    let manifest = plugin_dir.join("Cargo.toml");
    let Ok(text) = std::fs::read_to_string(&manifest) else {
        return Ok(Deps::default());
    };
    let wanted = third_party_lines(&text)?;
    if wanted.is_empty() {
        return Ok(Deps::default());
    }

    let dir = build_dir.join("deps");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    write_manifest(&dir, &wanted)?;

    reject_engine_crates(&dir)?;
    let artifacts = compile(&dir)?;

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
fn reject_engine_crates(dir: &Path) -> Result<(), String> {
    let out = crate::hide_console(&mut Command::new("cargo"))
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
             Fix it by dropping that dependency, reaching the same types through \
             `bevy::` (which the SDK provides), or writing a C-ABI plugin — those \
             share no types with the engine and may depend on anything.",
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
fn compile(dir: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    // `stderr` is PIPED, not inherited: an inherited stderr is what forces a
    // console to exist for the child, which is the window `hide_console` is
    // suppressing. Cargo's progress lines go into the pipe and are dropped; its
    // diagnostics still reach the caller through the non-zero exit below.
    let out = crate::hide_console(&mut Command::new("cargo"))
        .current_dir(dir)
        .args(["build", "--release", "--message-format=json-render-diagnostics"])
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
