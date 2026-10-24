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
//! # Dependencies that are built on Bevy
//!
//! Stripping Bevy from the manifest is not enough when a dependency is itself a
//! Bevy crate (avian, an ecosystem crate): cargo would resolve Bevy for it
//! regardless. With an SDK that carries `shared.tsv` (see [`Share`]), such a
//! graph is built anyway, under a `RUSTC_WRAPPER` (`shim.rs`) that takes every
//! crate the engine's shared images already contain from the SDK instead of
//! compiling it. The dependency then compiles against the engine's own Bevy and
//! `glam`, and its code reaches them through the shared images exactly as the
//! plugin's own code does. Only crates the table names are allowed through; a
//! Bevy crate at another version is still refused.
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

use crate::{json, shim};

/// The extra crates a plugin's own `rustc` invocation should be given.
#[derive(Debug, Default)]
pub struct Deps {
    /// `--extern <name>=<rlib>`, one per DIRECT dependency.
    ///
    /// Only the direct ones: those are the names the plugin can `use`.
    /// Everything transitive is found through [`Deps::search`] instead, the same
    /// way the SDK's own metadata files are.
    pub externs: Vec<(String, PathBuf)>,
    /// The `-L dependency=` paths holding everything cargo produced. Two when
    /// the build had an explicit `--target`: cargo then keeps what it built for
    /// the compiler (proc macros) apart from what it built for the plugin, and
    /// the plugin's compile reads both.
    pub search: Vec<PathBuf>,
}

/// What the SDK offers a dependency build beyond its `-L` paths: the crates its
/// shared images contain, and the lockfile they were resolved with.
pub struct Share<'a> {
    /// `shared.tsv`. See `shim.rs` for the format.
    pub table: &'a Path,
    /// The engine's `Cargo.lock`, so a dependency resolves the versions the
    /// shared images were built from rather than whatever is newest today. A
    /// newer patch release of a shared crate would not be in the table, and the
    /// dependency would compile its own copy of it.
    pub lockfile: Option<&'a Path>,
    /// The SDK's `-L dependency=` directories.
    pub search: &'a [PathBuf],
    /// The SDK's `-L native=` directories, for linking the wrapper itself.
    pub native: &'a [PathBuf],
    /// The target triple, passed to cargo as `--target`.
    pub triple: &'a str,
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
    build_with(plugin_dir, build_dir, toolchain, None)
}

/// [`build`], with the SDK's [`Share`] when it has one.
pub fn build_with(
    plugin_dir: &Path,
    build_dir: &Path,
    toolchain: &str,
    share: Option<&Share>,
) -> Result<Deps, String> {
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

    // An SDK without a table (one staged before it existed) shares nothing, and
    // the build is exactly what it was before: Bevy anywhere in the graph is
    // refused.
    let table = share
        .and_then(|s| {
            let root = s.table.parent()?;
            Some(shim::parse_table(&std::fs::read_to_string(s.table).ok()?, root))
        })
        .unwrap_or_default();
    let share = share.filter(|_| !table.is_empty());
    if let Some(lock) = share.and_then(|s| s.lockfile) {
        // Copied on every build, not only the first: `deps/` is deleted after a
        // successful one, and a lockfile left from a failed one may predate the
        // engine the SDK now describes.
        if lock.is_file() {
            std::fs::copy(lock, dir.join("Cargo.lock")).map_err(|e| e.to_string())?;
        }
    }

    reject_engine_crates(&dir, &pinned, &table, share.map(|s| s.triple))?;
    let wrap = match share {
        Some(s) => Some(Wrapper::prepare(&dir, &pinned, s)?),
        None => None,
    };
    let artifacts = compile(&dir, &pinned, share, wrap.as_ref())?;

    // Match cargo's artifacts back to the DIRECT dependency names. Cargo reports
    // a crate name, which is the dependency name with hyphens turned into
    // underscores — `wasm-bindgen` builds as `wasm_bindgen` — so both sides are
    // normalised before comparing.
    let direct: Vec<String> = wanted.iter().map(|(n, _)| n.replace('-', "_")).collect();
    let mut externs = Vec::new();
    for (name, lib) in artifacts {
        if !direct.contains(&name) {
            continue;
        }
        // A direct dependency the SDK supplied was never written; its recorded
        // copy is the one to hand the plugin.
        let lib = match (lib, wrap.as_ref()) {
            (Some(lib), _) => Some(lib),
            (None, Some(w)) => shim::read_record(&w.map, &shim::crate_key(&name)),
            (None, None) => None,
        };
        if let Some(lib) = lib {
            externs.push((name, lib));
        }
    }
    let target = dir.join("target");
    let search = match share {
        Some(s) => vec![target.join(s.triple).join("release").join("deps"), target.join("release").join("deps")],
        None => vec![target.join("release").join("deps")],
    };
    Ok(Deps { externs, search })
}

/// The compiled `shim.rs` and the directory it records substitutions in.
struct Wrapper {
    exe: PathBuf,
    map: PathBuf,
}

impl Wrapper {
    /// Compile the wrapper into `dir/shim/`, with the pinned compiler.
    ///
    /// From source on the machine that runs it, like everything else here, which
    /// keeps the SDK free of executables and this crate free of dependencies.
    /// It is one file of `std` and takes about a second.
    fn prepare(dir: &Path, pinned: &Pinned, share: &Share) -> Result<Self, String> {
        let shim_dir = dir.join("shim");
        let map = dir.join("shim-map");
        // A record from an earlier build names a unit hash cargo may reuse for a
        // different SDK; starting empty means every record was written by this one.
        let _ = std::fs::remove_dir_all(&map);
        std::fs::create_dir_all(&shim_dir).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&map).map_err(|e| e.to_string())?;
        std::fs::write(shim_dir.join("shim.rs"), include_str!("shim.rs")).map_err(|e| e.to_string())?;
        std::fs::write(
            shim_dir.join("main.rs"),
            "#[path = \"shim.rs\"]\nmod shim;\n\nfn main() {\n    shim::run()\n}\n",
        )
        .map_err(|e| e.to_string())?;
        let exe = shim_dir.join(if cfg!(windows) { "renzora_rustc_shim.exe" } else { "renzora_rustc_shim" });

        let rustc = pinned.rustc.clone().unwrap_or_else(|| crate::tool("rustc"));
        let mut cmd = Command::new(rustc);
        crate::hide_console(&mut cmd);
        cmd.args(["--edition", "2021", "-O", "-A", "warnings", "--crate-type", "bin"])
            .args(["--crate-name", "renzora_rustc_shim"])
            .arg(shim_dir.join("main.rs"))
            .arg("-o")
            .arg(&exe);
        // The same link setup as a plugin on this target (see `rustc::args`), so
        // the wrapper needs nothing the SDK does not already carry. Without the
        // SDK's import libraries the toolchain's default linker is left to find
        // its own.
        if share.triple.contains("windows-msvc") && !share.native.is_empty() {
            cmd.args(["-C", "linker=rust-lld", "-C", "target-feature=+crt-static"]);
        }
        for n in share.native {
            cmd.arg("-L").arg(format!("native={}", n.display()));
        }
        let out = cmd.stderr(Stdio::piped()).output().map_err(|e| format!("could not run rustc: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "compiling the dependency build's compiler wrapper failed ({})\n{}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(Self { exe, map })
    }
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
///
/// # Why this asks what is installed first
///
/// `rustup which --toolchain <v>` **installs `<v>` if it is missing.** Naming a
/// toolchain on a rustup command line is a request for it, so a lookup meant to
/// answer "is this here?" instead downloads several hundred megabytes and
/// answers "it is now".
///
/// That is not theoretical and it is not cheap. CI passed `1.0.0` — chosen for a
/// test on the belief that no such toolchain could exist — and rustup fetched
/// Rust 1.0.0, whose cargo predates `cargo metadata` by a dozen releases, so the
/// build failed with `No such subcommand` and exit 127 instead of the "install
/// Rust 1.95.0" this is supposed to say.
///
/// It also took a decision that is not this function's to take. Installing a
/// toolchain is a ~400 MB download that `renzora_plugin_build::toolchain`
/// deliberately puts behind a prompt saying so (see `Toolchain::needs`). A
/// dependency build that quietly installs one behind the user's back defeats
/// that, on a path that is not even the one that asks.
///
/// `rustup toolchain list` reads a directory and touches no network, so the
/// question is asked of what is already on the machine. A version that is
/// genuinely missing then falls through to the caller's error — which names it
/// and says how to install it, which is the outcome that was wanted.
fn rustup_which(version: &str, bin: &str) -> Option<PathBuf> {
    if !toolchain_installed(version) {
        return None;
    }
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

/// Whether rustup already has `version`, without going near the network.
///
/// `rustup toolchain list` prints one toolchain per line as `<name>-<triple>`,
/// sometimes followed by a marker: `1.95.0-aarch64-apple-darwin (active)`. A
/// pinned version matches the `<name>` half.
///
/// The `-` in the prefix test is load-bearing: without it `1.9` would match
/// `1.95.0-…` and the wrong toolchain would be reported as present.
fn toolchain_installed(version: &str) -> bool {
    let Ok(out) = crate::hide_console(&mut Command::new(crate::tool("rustup")))
        .args(["toolchain", "list"])
        .output()
    else {
        return false;
    };
    if !out.status.success() {
        return false;
    }
    lists_toolchain(&String::from_utf8_lossy(&out.stdout), version)
}

/// Whether `listing` — the output of `rustup toolchain list` — contains
/// `version`.
///
/// Split from [`toolchain_installed`] so the match can be tested without rustup,
/// because getting it wrong is not a visible failure: a version reported present
/// when it is not resolves to some *other* toolchain's cargo, which is precisely
/// the silent compiler mismatch this module exists to stop.
fn lists_toolchain(listing: &str, version: &str) -> bool {
    listing.lines().any(|line| {
        let name = line.split_whitespace().next().unwrap_or_default();
        // The `-` is load-bearing. A bare `starts_with` would let `1.9` match
        // `1.95.0-aarch64-apple-darwin`, and the build would then be pinned to a
        // compiler the SDK was not staged with — reported as a success.
        name == version || name.strip_prefix(version).is_some_and(|rest| rest.starts_with('-'))
    })
}

#[cfg(test)]
mod toolchain_list_tests {
    use super::lists_toolchain;

    /// Real `rustup toolchain list` output, markers and all.
    const LISTING: &str = "\
stable-aarch64-apple-darwin (default)
1.93.0-aarch64-apple-darwin
1.95.0-aarch64-apple-darwin (active)
";

    #[test]
    fn an_installed_version_is_found_with_or_without_a_marker() {
        assert!(lists_toolchain(LISTING, "1.95.0"), "the active one");
        assert!(lists_toolchain(LISTING, "1.93.0"), "one with no marker");
        assert!(lists_toolchain(LISTING, "stable"), "a channel name");
    }

    /// The failure that would not announce itself: a prefix of a version that IS
    /// installed must not count as installed.
    #[test]
    fn a_prefix_of_an_installed_version_is_not_a_match() {
        for near in ["1.9", "1.", "1", "1.95", "sta"] {
            assert!(
                !lists_toolchain(LISTING, near),
                "`{near}` must not match a longer version"
            );
        }
    }

    #[test]
    fn a_version_that_is_not_there_is_not_found() {
        for absent in ["1.94.0", "9999.0.0", "1.0.0", "nightly"] {
            assert!(!lists_toolchain(LISTING, absent), "`{absent}` is not installed");
        }
    }

    /// No rustup, or a rustup that printed nothing.
    #[test]
    fn an_empty_listing_has_nothing() {
        assert!(!lists_toolchain("", "1.95.0"));
    }
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
///
/// With a shared table the question becomes the real one: would this be a
/// SECOND copy of a crate the engine has? A Bevy crate the SDK shares at the
/// same version is the engine's own copy (the wrapper substitutes it). One the
/// engine does not have at all (`bevy_heavy`, an ecosystem crate) is compiled
/// against the engine's Bevy and is nobody's duplicate, and neither is one that
/// only runs inside the compiler (`bevy_macro_utils`). What is refused is a
/// crate the engine shares at a different version: a plugin asking for Bevy
/// 0.19 under a 0.20 engine. `renzora_*` stays strict, because those come
/// through the SDK's `--extern`s and nowhere else.
fn reject_engine_crates(
    dir: &Path,
    pinned: &Pinned,
    table: &[shim::SharedCrate],
    triple: Option<&str>,
) -> Result<(), String> {
    let mut cmd = pinned.command();
    cmd.current_dir(dir).args(["metadata", "--format-version", "1", "--quiet"]);
    // Only this platform's graph. Unfiltered, the resolve includes every
    // target's dependencies (`bevy_android` on Windows), which the build never
    // compiles and which would be judged as if it did.
    if let Some(triple) = triple {
        cmd.args(["--filter-platform", triple]);
    }
    let out = cmd.stderr(Stdio::piped()).output().map_err(|e| format!("could not run cargo: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "`cargo metadata` failed ({})\n{}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut found: Vec<String> = Vec::new();
    for (name, version) in json::all_strings(&text, "id").iter().filter_map(|id| package_of(id)) {
        if !is_engine_crate(&name) {
            continue;
        }
        if table.iter().any(|c| c.package == name && c.version == version) {
            continue;
        }
        let shared = table.iter().find(|c| c.package == name).map(|c| c.version.as_str());
        let contract = name == "renzora" || name.starts_with("renzora_");
        if !table.is_empty() && shared.is_none() && !contract {
            continue;
        }
        let entry = match shared {
            Some(engine) => format!("{name} {version} (the engine shares {engine})"),
            None => format!("{name} {version}"),
        };
        if !found.contains(&entry) {
            found.push(entry);
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
             A dependency built on Bevy is fine when it asks for the versions the \
             engine itself was built with, because those are taken from the SDK. \
             Fix it by picking a version of the dependency made for this engine's \
             Bevy, by dropping it, or by reaching the same types through `bevy::` \
             and `renzora::`.",
            found.join(", ")
        ));
    }
    Ok(())
}

/// `(name, version)` from a cargo package id.
///
/// Cargo writes two forms. The current one is a URL with a fragment:
/// `registry+https://…/crates.io-index#glam@0.33.12`, or `#0.1.0` alone when the
/// name is the URL's last path segment (a path or git package in a directory of
/// its own name). The older one is `glam 0.33.12 (registry+https://…)`.
pub fn package_of(id: &str) -> Option<(String, String)> {
    if let Some((url, fragment)) = id.split_once('#') {
        if let Some((name, version)) = fragment.split_once('@') {
            return Some((name.to_string(), version.to_string()));
        }
        let path = url.split('?').next().unwrap_or(url).trim_end_matches('/');
        let name = path.rsplit('/').next()?.trim_end_matches(".git");
        return Some((name.to_string(), fragment.to_string()));
    }
    let mut parts = id.split_whitespace();
    let name = parts.next()?;
    let version = parts.next()?;
    parts.next()?.starts_with('(').then(|| (name.to_string(), version.to_string()))
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
///
/// With a [`Share`], cargo runs under the wrapper and with an explicit
/// `--target`. The flag is what lets the wrapper substitute safely: cargo then
/// builds the units it needs for the compiler itself (proc macros, build
/// scripts, and their dependencies) without it, and those have to be real
/// compiled code, never the SDK's metadata.
fn compile(
    dir: &Path,
    pinned: &Pinned,
    share: Option<&Share>,
    wrap: Option<&Wrapper>,
) -> Result<Vec<(String, Option<PathBuf>)>, String> {
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
    let mut cmd = pinned.command();
    cmd.current_dir(dir)
        .args(["build", "--release", "--message-format=json-render-diagnostics"])
        .arg("--target-dir")
        .arg(dir.join("target"));
    if let (Some(share), Some(wrap)) = (share, wrap) {
        cmd.args(["--target", share.triple])
            // Cargo sets these for every unit it compiles. Inherited from
            // whatever started the editor (`cargo renzora` sets them), they would
            // also reach the wrapper for cargo's own compiler queries, and name
            // a package that is not being built.
            .env_remove("CARGO_PKG_NAME")
            .env_remove("CARGO_PKG_VERSION")
            .env("RUSTC_WRAPPER", &wrap.exe)
            .env(shim::ENV_TABLE, share.table)
            .env(shim::ENV_MAP, &wrap.map)
            .env(
                shim::ENV_SEARCH,
                std::env::join_paths(share.search).map_err(|e| format!("SDK path: {e}"))?,
            );
    }
    let out = cmd.stderr(Stdio::piped()).output().map_err(|e| format!("could not run cargo: {e}"))?;
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
        let files = json::string_array(line, "filenames");
        if let Some(rlib) = files.iter().find(|f| f.ends_with(".rlib")) {
            found.push((name, Some(PathBuf::from(rlib))));
        } else if files.is_empty() {
            // A unit the wrapper took from the SDK: cargo lists only the files
            // that exist, and it wrote none.
            found.push((name, None));
        }
    }
    Ok(found)
}
