//! Compile a marketplace plugin from Rust source, against the SDK staged beside
//! the editor.
//!
//! A plugin ships as source and is built on the machine that installs it. That
//! is what lets a plugin keep working when the engine changes: the stamp stops
//! matching, the plugin rebuilds, and nothing had to be republished. The cost is
//! that this file has to get the compiler invocation exactly right, because most
//! of the ways to get it wrong do not produce an error.
//!
//! # Why `rustc` directly and not `cargo`
//!
//! A plugin is ONE crate whose only dependencies are already compiled and
//! staged. There is no dependency graph to resolve, so cargo would add a
//! lockfile, a target directory, and its own opinion about `-C metadata` — the
//! last of which is the entire reason another engine taking this approach needs
//! a `RUSTC_WRAPPER` to force the extern edges back into line. Naming the
//! artifacts directly avoids all of it, and takes about a second.
//!
//! # The three ways this goes wrong quietly
//!
//! 1. **`--extern renzora` pointed at `librenzora.rlib`.** Compiles, links,
//!    loads. The plugin then holds a *private* copy of the contract crate's
//!    process-global state: `t()` returns raw keys, its warnings and log lines
//!    land in buffers nobody drains, asset reads find no loader. Measured, the
//!    difference is a 5.45 MB plugin against a 0.29 MB one. It must be the
//!    shared `renzora_dylib`, aliased to the name `renzora`.
//!
//! 2. **`--extern bevy` pointed at the dylib.** The mirror image, and it fails
//!    loudly rather than quietly — `bevy_dylib` re-exports `bevy_internal`, so
//!    `bevy::prelude` does not resolve. `bevy` must be the facade *rlib*, which
//!    declares `extern crate bevy_dylib` itself — and the one crate the SDK
//!    stages whole rather than as metadata, for reasons `xtask::sdk` explains.
//!    The two are asymmetric because only one of them has a facade.
//!
//! 3. **A missing `-L native=` path.** Produces `LNK1181: cannot open input
//!    file 'windows.0.52.0.lib'`, which names a file but nothing that says where
//!    it should have come from. Those directories reach a normal cargo build
//!    through build-script output that this invocation never sees, so the SDK
//!    stages copies and the manifest lists them.
//!
//! All three are settled by [`Sdk::load`] reading the manifest rather than by
//! anything here reconstructing them.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

pub mod toolchain;
pub mod unpack;
pub use toolchain::Toolchain;
pub use unpack::SdkState;

use renzora_native_build as native_build;
/// Re-exported so a caller holding an [`Sdk`] can reach the dependency machinery
/// without also depending on the core crate. The logic lives there because
/// `xtask` builds the repo's own plugins the same way and must not drift.
pub use renzora_native_build::deps;
/// Re-exported for the same reason as [`deps`]: everything that looks for an
/// SDK has to agree on where the install root is, and inside a Linux AppImage
/// that is NOT the executable's parent.
pub use renzora_native_build::install;

/// What the SDK's `manifest.json` records, written by `cargo renzora sdk`.
#[derive(Debug, Deserialize)]
pub struct Manifest {
    /// Target triple the artifacts were built for.
    pub triple: String,
    /// The exact rustc that produced them, e.g. `1.95.0`.
    pub rustc: String,
    /// Shared-library extension, no dot.
    pub lib_ext: String,
    /// Content hash of the images a plugin binds to — see [`Sdk::stamp`].
    ///
    /// Optional so an SDK cut before this field existed still loads; that one
    /// falls back to the old filename-based stamp, which is weaker but not
    /// wrong for the case it can see.
    #[serde(default)]
    pub build_id: Option<String>,
    /// `--extern <name>=<sdk-relative path>` for the crates a plugin may use.
    pub r#extern: Externs,
    pub link_search: LinkSearch,
}

#[derive(Debug, Deserialize)]
pub struct Externs {
    pub bevy: String,
    pub renzora: String,
    /// The UI framework, as the shared `renzora_ember_dylib` image.
    ///
    /// Optional for the same reason as [`Manifest::build_id`]: an SDK staged
    /// before panels were reachable from a plugin has no entry, and a plugin
    /// that does not draw UI still builds against it.
    #[serde(default)]
    pub renzora_ember: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LinkSearch {
    /// Passed as `-L dependency=` — crate metadata and linkable dylibs.
    pub dependency: Vec<String>,
    /// Passed as `-L native=` — native import libraries.
    pub native: Vec<String>,
}

/// A staged SDK, ready to compile against.
pub struct Sdk {
    root: PathBuf,
    manifest: Manifest,
}

#[derive(Debug)]
pub enum Error {
    /// No SDK at all — neither an extracted tree nor the shipped archive.
    ///
    /// A source checkout that has not run `cargo renzora`, or a build that
    /// shipped without one. Nothing the engine can do about it by itself.
    Missing(PathBuf),
    /// The SDK shipped with this build but has not been unpacked yet.
    ///
    /// Distinct from [`Error::Missing`] because it is the opposite situation and
    /// the opposite remedy: everything needed is already on disk, one
    /// decompression away. Reporting it as "missing" — which is what happened
    /// before this variant existed — sends someone looking for a download that
    /// does not exist, while `sdk.tar.zst` sits beside the executable they just
    /// launched.
    ///
    /// Carries the archive and its size so a caller can say how much work
    /// unpacking is, and act on it without going looking for the file again.
    Packed { archive: PathBuf, bytes: u64 },
    /// The SDK is present but unreadable or malformed.
    Manifest(String),
    /// The pinned compiler is not available.
    ///
    /// Separate from [`Error::Compile`] on purpose: it is recoverable by
    /// installing a toolchain, and the caller can say so precisely instead of
    /// relaying `E0514` from a build that never had a chance. `state` carries
    /// which kind of recovery — see [`Toolchain::needs`].
    Toolchain {
        needs: String,
        found: String,
        state: Toolchain,
    },
    /// rustc could not be run at all.
    NoRustc(String),
    /// rustc ran and rejected the plugin. Carries its stderr verbatim: it is
    /// written for the plugin author, and rewriting it would only lose detail.
    Compile(String),
    /// The plugin's third-party dependencies could not be resolved or built.
    ///
    /// Separate from [`Error::Compile`] because the plugin's own source is fine
    /// and the remedy is in its `Cargo.toml` — including the case this is most
    /// worth naming, a dependency that drags Bevy back in. See [`crate::deps`].
    Deps(String),
    Io(std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Missing(p) => write!(f, "no plugin SDK at {}", p.display()),
            Error::Packed { bytes, .. } => write!(
                f,
                "the plugin SDK has not been unpacked yet ({} MB compressed). It \
                 ships with the engine and is needed to compile Rust scripts and \
                 native plugins",
                bytes / 1_048_576
            ),
            Error::Manifest(e) => write!(f, "unreadable SDK manifest: {e}"),
            // Prefer the state's own wording, which is written for a dialog and
            // says what will be downloaded and where. The `needs`/`found` form
            // is the fallback for logs, where the versions matter more than the
            // remedy.
            Error::Toolchain { needs, found, state } => match state.needs() {
                Some(msg) => write!(f, "{msg}"),
                None => write!(
                    f,
                    "this SDK was built with Rust {needs}, but `rustc` here is {found} — \
                     install it with `rustup toolchain install {needs}`"
                ),
            },
            Error::NoRustc(e) => write!(f, "could not run rustc: {e}"),
            Error::Compile(e) => write!(f, "{e}"),
            Error::Deps(e) => write!(f, "{e}"),
            Error::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl Sdk {
    /// Load the SDK staged at `root` (the `sdk/` directory beside the editor).
    pub fn load(root: impl Into<PathBuf>) -> Result<Self, Error> {
        let root = root.into();
        let path = root.join("manifest.json");
        if !path.is_file() {
            // Before reporting "missing", look for the archive a release ships.
            // `root` is `<exe dir>/sdk`, so the archive is its sibling. The two
            // states need different words: one is a build without an SDK, the
            // other is an SDK one decompression away, and calling the second
            // "missing" sent people looking for a download that does not exist.
            let exe_dir = root.parent().unwrap_or(&root);
            if let unpack::SdkState::Packed { archive, bytes } = unpack::sdk_state(exe_dir) {
                return Err(Error::Packed { archive, bytes });
            }
            return Err(Error::Missing(root));
        }
        let text = std::fs::read_to_string(&path)?;
        let manifest: Manifest =
            serde_json::from_str(&text).map_err(|e| Error::Manifest(e.to_string()))?;
        Ok(Sdk { root, manifest })
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Locate the compiler this SDK was built with.
    ///
    /// The result is a *state*, not a yes/no, because the three answers need
    /// different things from the caller: nothing, an install through an existing
    /// rustup, or the user's agreement to put Rust on the machine at all. See
    /// [`Toolchain`].
    pub fn toolchain(&self) -> Toolchain {
        toolchain::resolve(&self.manifest.rustc)
    }

    /// Whether the pinned compiler is present.
    ///
    /// Worth checking before compiling rather than after: a mismatch is refused
    /// at the metadata layer with `error[E0514]: found crate 'bevy' compiled by
    /// an incompatible version of rustc`, which is accurate but arrives once the
    /// user has already downloaded ~444 MB and pressed Install.
    pub fn check_toolchain(&self) -> Result<(), Error> {
        match self.toolchain() {
            Toolchain::Ready(_) => Ok(()),
            other => Err(Error::Toolchain {
                needs: self.manifest.rustc.clone(),
                found: rustc_release().unwrap_or_else(|_| "none".into()),
                state: other,
            }),
        }
    }

    /// A token identifying what a plugin built against this SDK is bound to.
    ///
    /// Stored beside a built plugin and compared on load; a mismatch means
    /// rebuild.
    ///
    /// It is the manifest's `build_id` — a content hash of the images a plugin
    /// links, computed once when the SDK was staged. It has to be content-based:
    /// the previous stamp was the `bevy` rlib filename plus the rustc version,
    /// and cargo derives that filename from the build *configuration*, never
    /// from source. Editing `crates/renzora` therefore left every filename
    /// identical, so no installed plugin rebuilt and each kept loading against a
    /// contract crate whose layouts had moved — silently, because Rust mangles
    /// symbols from a crate's stable id rather than its contents, so the imports
    /// still resolved.
    ///
    /// The old form is the fallback for an SDK staged before `build_id` existed.
    /// It catches a rustc or Bevy change and misses a contract-crate one, which
    /// is exactly the gap this replaced — but a stamp that only sometimes
    /// notices still beats refusing to load anything.
    pub fn stamp(&self) -> String {
        if let Some(id) = &self.manifest.build_id {
            return format!("{id}+rustc-{}", self.manifest.rustc);
        }
        let bevy = Path::new(&self.manifest.r#extern.bevy)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?");
        format!("{}+rustc-{}", bevy, self.manifest.rustc)
    }

    /// Compile the plugin rooted at `dir` (holding `src/lib.rs`) into `out`.
    ///
    /// `out`'s parent must exist. On success the library is written and the
    /// stamp is returned for the caller to record beside it.
    pub fn compile(&self, dir: &Path, out: &Path) -> Result<String, Error> {
        self.compile_with(dir, out, &mut |_| {})
    }

    /// [`compile`](Self::compile), reporting each line the compiler writes as it
    /// writes it.
    ///
    /// For a caller with somewhere to show progress — the first-run window,
    /// which would otherwise hold a motionless bar for the length of the build.
    /// The lines are rustc's diagnostics and, for a plugin with third-party
    /// dependencies, cargo's `Compiling …` output.
    pub fn compile_with(
        &self,
        dir: &Path,
        out: &Path,
        on_line: &mut dyn FnMut(&str),
    ) -> Result<String, Error> {
        // Resolved to an absolute path, not spawned as bare `rustc`. A clean
        // machine has none; a developer's machine may have several, and the
        // wrong one gets further before failing than no compiler at all.
        let state = self.toolchain();
        let Some(rustc) = state.rustc().cloned() else {
            return Err(Error::Toolchain {
                needs: self.manifest.rustc.clone(),
                found: rustc_release().unwrap_or_else(|_| "none".into()),
                state,
            });
        };
        let src = dir.join("src").join("lib.rs");
        let manifest = ensure_cargo_manifest(dir)?;

        // The command line itself lives in `renzora_native_build`, because
        // `xtask` has to produce a byte-identical one when it builds the repo's
        // own plugins. Everything below is just resolving this SDK's manifest
        // into the absolute paths that shared code takes.
        let name = crate_name(dir);
        let bevy = self.root.join(&self.manifest.r#extern.bevy);
        let renzora = self.root.join(&self.manifest.r#extern.renzora);
        let ember = self.manifest.r#extern.renzora_ember.as_ref().map(|e| self.root.join(e));
        let dependency: Vec<PathBuf> =
            self.manifest.link_search.dependency.iter().map(|d| self.root.join(d)).collect();
        let native: Vec<PathBuf> =
            self.manifest.link_search.native.iter().map(|n| self.root.join(n)).collect();

        let target = native_build::Target {
            triple: &self.manifest.triple,
            crate_name: &name,
            extern_bevy: &bevy,
            extern_renzora: &renzora,
            extern_ember: ember.as_deref(),
            dependency: &dependency,
            native: &native,
            plugin_dir: &manifest,
            // The plugin's own `build/` directory, which is where the
            // third-party dependency crate is synthesized.
            build_dir: out.parent().unwrap_or(dir),
            src: &src,
            out,
        };
        let args = native_build::rustc::args(&target).map_err(Error::Deps)?;

        let mut cmd = Command::new(&rustc);
        // No console window for the compiler — its output is streamed to the
        // caller below and shown in the setup window instead.
        native_build::hide_console(&mut cmd);
        for (key, value) in native_build::rustc::env_vars(&target) {
            cmd.env(key, value);
        }
        cmd.args(&args);

        // Streamed rather than collected with `output()`, so a caller can show
        // what the compiler is saying while it says it. The first-run window
        // otherwise sits on a motionless bar for the whole build with nothing to
        // read — and a plugin that is going to fail says so on this stream.
        //
        // stderr only: rustc's diagnostics and cargo's `Compiling …` progress
        // both go there, and its stdout carries nothing a person wants.
        cmd.stderr(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::null());
        let mut child = cmd.spawn().map_err(|e| Error::NoRustc(e.to_string()))?;

        let mut collected = String::new();
        if let Some(stderr) = child.stderr.take() {
            use std::io::BufRead;
            for line in std::io::BufReader::new(stderr).lines().map_while(Result::ok) {
                on_line(&line);
                // Kept as well as reported, because the error a failure returns
                // is the whole diagnostic — a caller showing one line at a time
                // is not a substitute for the full text in the Console.
                collected.push_str(&line);
                collected.push('\n');
            }
        }
        let status = child.wait().map_err(|e| Error::NoRustc(e.to_string()))?;
        if !status.success() {
            return Err(Error::Compile(collected));
        }
        // Drop the import library and debug symbols the linker left beside the
        // plugin. Nothing loads them, and on Windows they outweigh the plugin.
        // After success only: a failed build's leftovers are worth reading.
        native_build::rustc::prune_byproducts(out);
        // Before the stamp is recorded, so a plugin is never marked built until
        // it points at the images the host actually has mapped.
        native_build::rustc::fixup_install_names(out);
        Ok(self.stamp())
    }
}

/// A crate name for the plugin, from its directory.
///
/// Hyphens become underscores because that is what cargo would have done, and
/// what `#[unsafe(no_mangle)]`-free code paths expect of a crate identifier.
fn crate_name(dir: &Path) -> String {
    dir.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("plugin")
        .replace('-', "_")
}

/// Make sure `dir` has a `Cargo.toml`, and return `dir`.
///
/// Not because anything runs cargo — nothing does — but because Bevy's proc
/// macros read the manifest at `$CARGO_MANIFEST_DIR` to work out how to name
/// Bevy's own crates in the code they emit. Without a manifest they panic with
/// `Cargo manifest does not exist at path …`, which points at the macro rather
/// than at the missing file.
///
/// An author's own manifest is left alone: a plugin developed as a normal cargo
/// project locally should keep working the way its author set it up. One is
/// generated only when there is none, listing `bevy` so `BevyManifest` resolves
/// to the facade rather than the subcrates.
fn ensure_cargo_manifest(dir: &Path) -> Result<PathBuf, Error> {
    let path = dir.join("Cargo.toml");
    if !path.is_file() {
        let name = crate_name(dir);
        std::fs::write(
            &path,
            format!(
                "# Generated so Bevy's proc macros can resolve their own crate paths.\n\
                 # Nothing runs cargo here; the plugin is compiled by rustc directly\n\
                 # against the SDK. Replace this with your own manifest to develop the\n\
                 # plugin as an ordinary cargo project — it will not be overwritten.\n\
                 [package]\n\
                 name = \"{name}\"\n\
                 version = \"0.1.0\"\n\
                 edition = \"2021\"\n\
                 \n\
                 [lib]\n\
                 crate-type = [\"dylib\"]\n\
                 \n\
                 [dependencies]\n\
                 bevy = \"0.19\"\n\
                 renzora = \"*\"\n"
            ),
        )?;
    }
    Ok(dir.to_path_buf())
}

/// The local rustc's release string, e.g. `1.95.0`.
fn rustc_release() -> Result<String, String> {
    let out = native_build::hide_console(&mut Command::new("rustc"))
        .arg("-vV")
        .output()
        .map_err(|e| e.to_string())?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("release: "))
        .map(|s| s.trim().to_string())
        .ok_or_else(|| "no `release:` line in `rustc -vV`".to_string())
}
