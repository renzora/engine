//! The `rustc` command line that compiles a native plugin.
//!
//! One definition, used by both callers. `renzora_plugin_build` runs it for a
//! plugin a user installed into a downloaded editor; `xtask` runs it for the
//! repo's own plugins during staging. They must agree exactly — a flag present
//! in one and not the other produces a plugin that builds for a developer and
//! not for a user, or the reverse, and neither side would be obviously wrong to
//! read.
//!
//! # What this returns, and what it deliberately does not
//!
//! Arguments only. The two callers differ in ways that have nothing to do with
//! the command line: the editor resolves an absolute `rustc` through a pinned
//! rustup toolchain, while xtask uses whatever `rustc` is on `PATH`. Handing
//! back a `Vec<String>` lets each build its own `Command` and keeps the shared
//! part to the thing that actually has to match.
//!
//! Both callers must still set [`ENV_VARS`] — see its note, because the failure
//! when they do not is a bare "proc macro panicked".

use std::path::{Path, PathBuf};

use crate::deps;

/// Everything the command line depends on, resolved to absolute paths.
///
/// The two callers reach these from different places (a serde-parsed manifest in
/// one, a hand-parsed one in the other), so this takes the resolved values
/// rather than a manifest type neither of them would naturally hold.
pub struct Target<'a> {
    /// e.g. `x86_64-pc-windows-msvc`. Only the linker choice reads it.
    pub triple: &'a str,
    /// The plugin's crate name, hyphens already turned into underscores.
    pub crate_name: &'a str,
    /// `--extern bevy=` — the facade **rlib**, which declares
    /// `extern crate bevy_dylib` and routes Bevy's code to the shared image.
    pub extern_bevy: &'a Path,
    /// `--extern renzora=` — the `renzora_dylib` shared library, aliased.
    pub extern_renzora: &'a Path,
    /// `--extern renzora_ember=` — the shared UI image. Absent only for an SDK
    /// staged before panels were reachable from a plugin.
    pub extern_ember: Option<&'a Path>,
    /// `-L dependency=` — crate metadata and linkable dylibs.
    pub dependency: &'a [PathBuf],
    /// `-L native=` — native import libraries from build scripts.
    pub native: &'a [PathBuf],
    /// The plugin's source directory, holding `Cargo.toml` and `src/`.
    pub plugin_dir: &'a Path,
    /// Where the plugin's own artefacts go; the third-party dependency build
    /// gets a `deps/` subdirectory of it.
    pub build_dir: &'a Path,
    /// The crate root, normally `<plugin_dir>/src/lib.rs`.
    pub src: &'a Path,
    /// The library to write.
    pub out: &'a Path,
}

/// Environment both callers must set before running the returned arguments.
///
/// Bevy's derives resolve their own crate paths through `BevyManifest`, which
/// reads `$CARGO_MANIFEST_DIR/Cargo.toml` to decide whether to emit `bevy::…` or
/// `bevy_ecs::…`. Running `rustc` directly means no cargo set it, and the
/// failure is a bare `error: proc macro panicked` naming the macro rather than
/// the missing variable — reached by anything using `#[derive(Component)]` or
/// `bsn!`, which is most plugins.
///
/// Returns `(CARGO_MANIFEST_DIR, CARGO_PKG_NAME)`.
pub fn env_vars(t: &Target) -> [(&'static str, String); 2] {
    [
        ("CARGO_MANIFEST_DIR", t.plugin_dir.display().to_string()),
        ("CARGO_PKG_NAME", t.crate_name.to_string()),
    ]
}

/// Assemble the full argument list.
///
/// Compiles the plugin's third-party dependencies as a side effect when it has
/// any (see [`crate::deps`]); returns their error unchanged when it cannot.
pub fn args(t: &Target) -> Result<Vec<String>, String> {
    let mut a: Vec<String> = Vec::new();
    // A macro rather than a closure: a closure capturing `a` holds the mutable
    // borrow for the whole function, which blocks the `a.push(format!(…))` calls
    // that have to be interleaved with these.
    macro_rules! push {
        ($($s:expr),+ $(,)?) => { $( a.push($s.to_string()); )+ };
    }

    // Without this rustc names the crate after the FILE, so every plugin is
    // called `lib` and every log line it emits is tagged `INFO lib:`,
    // indistinguishable from every other plugin's.
    push!("--crate-name", t.crate_name);
    push!("--edition", "2021");
    push!("--crate-type", "dylib");
    // The plugin must IMPORT Bevy and the contract crate, not embed them.
    // Without this it links its own copies and stops sharing the `World` the
    // whole design exists to share.
    push!("-C", "prefer-dynamic");
    // A bare `rustc` defaults to `opt-level=0`, so without this every plugin and
    // script built here runs UNOPTIMISED — which for a script called once per
    // frame per entity is the expensive half; the size is only the visible one.
    //
    // 2 rather than 3: measured on a small script, 224 KB -> 109 KB with no
    // change in build time, where 3 gained nothing (110 KB) and `s`/`z` were
    // worse (122 KB). It also matches the engine's own `[profile.dist]`.
    push!("-C", "opt-level=2");

    // rust-lld, matching `.cargo/config.toml`. That file configures *cargo*, so
    // a bare rustc silently falls back to MSVC `link.exe`, which fails this link
    // on the exported-symbol count.
    if t.triple.contains("windows-msvc") {
        push!("-C", "linker=rust-lld");
    }

    push!("--extern", format!("bevy={}", t.extern_bevy.display()));
    push!("--extern", format!("renzora={}", t.extern_renzora.display()));
    // Passed unconditionally when the SDK has it — an unused `--extern` costs a
    // plugin nothing (it is not even linked), and requiring plugins to opt in
    // would mean an author's first panel fails to resolve `use renzora_ember::…`
    // with no hint that a switch exists.
    if let Some(ember) = t.extern_ember {
        push!("--extern", format!("renzora_ember={}", ember.display()));
    }

    // Third-party crates the plugin declared, compiled by cargo from a manifest
    // that mentions no Bevy. Empty — and cargo never runs — unless the plugin
    // asked for something.
    let extra = deps::build(t.plugin_dir, t.build_dir)?;
    for (name, rlib) in &extra.externs {
        push!("--extern", format!("{name}={}", rlib.display()));
    }
    if let Some(search) = &extra.search {
        push!("-L", format!("dependency={}", search.display()));
    }

    for d in t.dependency {
        push!("-L", format!("dependency={}", d.display()));
    }
    for n in t.native {
        push!("-L", format!("native={}", n.display()));
    }
    push!("-o", t.out.display(), t.src.display());
    Ok(a)
}
