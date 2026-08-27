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
//!    declares `extern crate bevy_dylib` itself. The two are asymmetric because
//!    only one of them has a facade.
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

/// What the SDK's `manifest.json` records, written by `cargo renzora sdk`.
#[derive(Debug, Deserialize)]
pub struct Manifest {
    /// Target triple the artifacts were built for.
    pub triple: String,
    /// The exact rustc that produced them, e.g. `1.95.0`.
    pub rustc: String,
    /// Shared-library extension, no dot.
    pub lib_ext: String,
    /// `--extern <name>=<sdk-relative path>` for the two crates a plugin may use.
    pub r#extern: Externs,
    pub link_search: LinkSearch,
}

#[derive(Debug, Deserialize)]
pub struct Externs {
    pub bevy: String,
    pub renzora: String,
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
    /// No SDK staged. The caller should offer to download it.
    Missing(PathBuf),
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
    Io(std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Missing(p) => write!(f, "no plugin SDK at {}", p.display()),
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
    /// user has already downloaded ~555 MB and pressed Install.
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
    /// rebuild. It is the `bevy_dylib` artifact filename plus the rustc version,
    /// because both are already exactly the things that must not drift — cargo
    /// hashes the whole build configuration into that filename, so nothing here
    /// has to decide what "compatible" means or keep a hash function in step.
    pub fn stamp(&self) -> String {
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

        let mut cmd = Command::new(rustc);
        // Bevy's derives resolve their own paths through `BevyManifest`, which
        // reads `$CARGO_MANIFEST_DIR/Cargo.toml` to decide whether to emit
        // `bevy::…` or `bevy_ecs::…`. Running rustc directly means no cargo set
        // this, and the failure is a bare `error: proc macro panicked` naming
        // the macro rather than the missing variable — reached by anything using
        // `#[derive(Component)]` or `bsn!`, which is most plugins.
        let name = crate_name(dir);
        cmd.env("CARGO_MANIFEST_DIR", &manifest)
            .env("CARGO_PKG_NAME", &name)
            // Without this rustc names the crate after the FILE, so every plugin
            // is called `lib` and every log line it emits is tagged `INFO lib:`,
            // indistinguishable from every other plugin's.
            .arg("--crate-name")
            .arg(&name);
        cmd.arg("--edition")
            .arg("2021")
            .arg("--crate-type")
            .arg("dylib")
            // The plugin must IMPORT Bevy and the contract crate, not embed
            // them. Without this it links its own copies and stops sharing the
            // `World` the whole design exists to share.
            .arg("-C")
            .arg("prefer-dynamic")
            // A bare `rustc` defaults to `opt-level=0`. Nothing was setting this,
            // so every plugin and script built here ran UNOPTIMISED — which for a
            // script called once per frame per entity is the expensive half. The
            // size is only the visible one.
            //
            // 2 rather than 3: measured on a small script, 224 KB -> 109 KB with
            // no change in build time, where 3 gained nothing (110 KB) and `s`/`z`
            // were worse (122 KB). It also matches the engine's own
            // `[profile.dist]`, so a plugin is built the way the code it calls
            // into was. `debuginfo` and `strip` are left alone: measured as no
            // change, because rustc outside cargo already emits neither.
            .arg("-C")
            .arg("opt-level=2");

        // rust-lld, matching `.cargo/config.toml`. That file configures *cargo*,
        // so a bare rustc silently falls back to MSVC `link.exe`, which fails
        // this link on the exported-symbol count.
        if self.manifest.triple.contains("windows-msvc") {
            cmd.arg("-C").arg("linker=rust-lld");
        }

        for (name, rel) in [
            ("bevy", &self.manifest.r#extern.bevy),
            ("renzora", &self.manifest.r#extern.renzora),
        ] {
            cmd.arg("--extern")
                .arg(format!("{name}={}", self.root.join(rel).display()));
        }
        for d in &self.manifest.link_search.dependency {
            cmd.arg("-L")
                .arg(format!("dependency={}", self.root.join(d).display()));
        }
        for n in &self.manifest.link_search.native {
            cmd.arg("-L")
                .arg(format!("native={}", self.root.join(n).display()));
        }
        cmd.arg("-o").arg(out).arg(&src);

        let output = cmd.output().map_err(|e| Error::NoRustc(e.to_string()))?;
        if !output.status.success() {
            return Err(Error::Compile(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ));
        }
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
    let out = Command::new("rustc")
        .arg("-vV")
        .output()
        .map_err(|e| e.to_string())?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("release: "))
        .map(|s| s.trim().to_string())
        .ok_or_else(|| "no `release:` line in `rustc -vV`".to_string())
}
