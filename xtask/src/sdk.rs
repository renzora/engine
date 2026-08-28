//! `cargo renzora sdk` — stage the compile-time half of a plugin build.
//!
//! A marketplace plugin ships as Rust source and is compiled on the machine that
//! installs it, against the engine that is already there. For that to work the
//! user needs what `rustc` reads at compile time, which is emphatically *not*
//! what the editor needs at run time:
//!
//!   * `dist/<platform>/` holds the **code** — `bevy_dylib`, `renzora_dylib`,
//!     `std`. Loaded when the editor starts. Staged by [`crate::stage`].
//!   * `dist/sdk-<triple>/` holds the **blueprints** — `.rmeta` crate metadata,
//!     proc-macro dylibs, native import libraries. Read by `rustc`, never
//!     loaded. Staged here.
//!
//! The two cannot be merged. Linking a dylib discards the metadata of everything
//! it swallowed: `renzora_dylib.dll` carries `renzora_dylib`'s own metadata and
//! nothing else, so a compiler pointed at it alone cannot even resolve `Query`.
//! Measured the other way round too — a plugin that reads a 39 MB `bevy_ecs`
//! rlib links to 0.29 MB, because only the metadata half is consumed.
//!
//! # Why `.rmeta` and not `.rlib`
//!
//! That last measurement is the whole argument. An `.rlib` is an `ar` archive of
//! exactly two things: `lib.rmeta`, the crate's metadata, and the object code
//! `rustc` produced for it. A plugin build consumes only the first — it
//! typechecks against the metadata and takes the *code* from the three shared
//! images (`bevy_dylib`, `renzora_dylib`, `renzora_ember_dylib`), which is the
//! point of them existing. The object half is dead weight in this directory:
//! 3325 MB of staged rlib against 1492 MB of the metadata inside it.
//!
//! So [`metadata_only`] swaps each `.rlib` cargo names for the sibling `.rmeta`
//! cargo emitted for the same unit. Both filenames carry the same `-C metadata`
//! hash, so the pairing is exact by construction rather than by matching names —
//! this is not the directory scan the section below forbids.
//!
//! Verified rather than assumed: every native plugin in `plugins/` compiles and
//! links against a metadata-only SDK, and the resulting library is byte-identical
//! to the rlib-built one apart from the PE timestamp — measured at 16 differing
//! bytes out of 408 576 on a panel plugin.
//!
//! # Why dropping the object half is sound, and where it stops being sound
//!
//! `rustc` gives every crate in the link closure a linkage. Under
//! `-C prefer-dynamic`, a crate whose object code is **already inside a dylib
//! being linked** is included from that dylib and needs no archive; anything
//! else falls to static linkage, which demands an `.rlib` and says so —
//! `error[E0461]: crate 'X' required to be available in rlib format`. There is
//! no silent path: a crate that needed its objects and lost them is a hard
//! error naming the crate.
//!
//! Staged metadata is sound because all 950 of these crates are inside
//! `bevy_dylib`, `renzora_dylib` or `renzora_ember_dylib`. That is what those
//! images ARE.
//!
//! # The one crate that keeps its `.rlib`
//!
//! `bevy` — the facade a plugin is handed by `--extern` — is the exception, and
//! it is subtle enough to be worth the 42 KB.
//!
//! The facade is not inside `bevy_dylib`; that image holds `bevy_internal` and
//! below. It IS inside `renzora_dylib`, which was built against the facade rlib
//! and swallowed it. So a plugin that touches `renzora` links `renzora_dylib`
//! and resolves `bevy` through it — which is why a metadata-only `bevy` compiled
//! every real plugin here.
//!
//! But the dylib has to be *used*, not merely offered: an unused `--extern` is
//! not linked and contributes nothing. A plugin importing `bevy` and nothing
//! else therefore failed E0461 with a metadata-only facade, where an `.rlib`
//! builds it fine (both measured).
//!
//! Such a plugin cannot actually load — `renzora::plugin!` writes the ctor
//! symbol the loader looks for, and the macro alone is enough to pull the image
//! in (measured) — so this was a build error on the way to a library that would
//! have been skipped anyway. It is still a worse error than no error, and the
//! `.rlib` is 42 576 bytes against the `.rmeta`'s 41 784. Paying 792 bytes to
//! keep the rule "everything a plugin is *given* stays whole; everything it
//! reaches *through* those is metadata" is the better trade.
//!
//! # Why it is staged on every build
//!
//! An SDK only works against the engine it was cut from. Every metadata
//! filename carries a `-C metadata` hash of the build configuration, and
//! `renzora_dylib` changes whenever the contract crate does — so a stale SDK
//! beside a fresh editor produces plugins that fail to link, or worse, link
//! against a mismatched image. Staging it alongside the binaries is what makes
//! "stale" a state that cannot occur locally.
//!
//! That is affordable only because the files are **hardlinked**, not copied.
//! `target/` and `dist/` sit on one volume, so ~1050 links cost no disk and
//! effectively no time; a plain copy of 1.9 GB on every build would not be
//! tolerable. `link_or_copy` falls back to copying if the link fails, which is
//! what happens when someone puts `dist/` on another drive.
//!
//! A release does not ship this tree as-is. `scripts/package-release.sh`
//! compresses it to a single `sdk.tar.zst` (~444 MB) beside the executables and
//! deletes the extracted copy, so the download carries one file rather than
//! 1.9 GB of loose metadata. Bundling it rather than hosting a separate download
//! is deliberate: it removes a whole subsystem (a URL, progress, resume,
//! checksums, offline) and makes a version mismatch structurally impossible,
//! since the SDK in the folder is by construction the one that built the editor
//! next to it.
//!
//! `cargo renzora sdk` regenerates only this part when that is all you want.
//!
//! # Why the file list comes from cargo and never from a directory scan
//!
//! `target/dist/deps/` holds many `-C metadata` variants of the same crate — two
//! `ash`, two `windows`, six `hashbrown` — left by different feature
//! resolutions. Picking "the newest of each name" out of that directory produces
//! a set that looks complete, compresses, ships, and then **fails to compile**,
//! because the variants are from different generations and their metadata
//! disagrees. That is not hypothetical; it is what happened the first time this
//! was attempted here, and the same bug is recorded in the staging code of
//! another engine that took this approach.
//!
//! So the list is read from `cargo build --message-format=json`, whose
//! `compiler-artifact` lines name the exact files this build produced. That is
//! the only source that is right by construction.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use crate::{link_or_copy, Platform};

/// Stage everything a plugin compile needs into `<dist>/sdk/`.
///
/// `dist_root` is the staged platform directory — the same one holding the two
/// executables and the shared dylibs, so the SDK travels with the build it was
/// cut from.
/// Which build to cut the SDK from.
///
/// Defaults describe a plain `cargo renzora` on this machine. `docker/build-all.sh`
/// overrides them because the container builds every platform from one checkout:
/// the editor goes to `--target-dir target/editor`, and a cross build adds the
/// target triple to the path. Neither is guessable from here — and getting it
/// wrong silently stages the SDK of a DIFFERENT platform, which would compile a
/// plugin that cannot load.
#[derive(Default)]
pub struct From {
    /// `--target-dir`, relative to the repo. `None` = cargo's `target`.
    pub target_dir: Option<String>,
    /// `--target <triple>` for a cross build. `None` = build for this host.
    pub target: Option<String>,
}

impl From {
    /// The directory cargo puts this build's artifacts in.
    ///
    /// Mirrors cargo's own layout: `<target-dir>/[<triple>/]<profile>`. The SDK
    /// reads build-script output from here; the crate files themselves come from
    /// cargo's JSON as absolute paths and need no reconstruction.
    fn profile_dir(&self, repo: &Path) -> PathBuf {
        let mut p = repo.join(self.target_dir.as_deref().unwrap_or("target"));
        if let Some(triple) = &self.target {
            p = p.join(triple);
        }
        p.join(crate::profile())
    }
}

pub fn build(repo: &Path, plat: &Platform, dist_root: &Path) -> Result<PathBuf, ExitCode> {
    build_from(repo, plat, dist_root, &From::default())
}

/// Stage the SDK for a build that may not be this host's.
pub fn build_from(
    repo: &Path,
    plat: &Platform,
    dist_root: &Path,
    from: &From,
) -> Result<PathBuf, ExitCode> {
    // The TARGET triple, not the host's: it is what the manifest records and what
    // a plugin build passes to rustc. They coincide only for a native build.
    let triple = match &from.target {
        Some(t) => t.clone(),
        None => host_triple().map_err(|e| {
            eprintln!("[xtask] {e}");
            ExitCode::FAILURE
        })?,
    };

    // A second cargo invocation, and a deliberate one. The normal build's output
    // is what a developer reads; `--message-format=json` would replace the
    // `Compiling …` progress with machine output. On an already-built tree this
    // re-run resolves in a second or two and reprints the artifact list, which is
    // the only thing wanted here.
    let mut externs = Externs::default();
    let artifacts = artifact_files(repo, from, &mut externs).map_err(|e| {
        eprintln!("[xtask] sdk build failed: {e}");
        ExitCode::FAILURE
    })?;
    // Refuse to write an SDK that cannot compile anything. Both are recorded from
    // the build itself, so a miss means the workspace shape changed underneath
    // this code — worth stopping for, since the alternative is an SDK that ships
    // and then fails on someone else's machine.
    for (what, got) in [
        ("bevy", &externs.bevy),
        ("renzora", &externs.renzora),
        ("renzora_ember", &externs.ember),
    ] {
        if got.is_none() {
            eprintln!(
                "[xtask] sdk: no `--extern {what}` candidate in the build — is \
                 `dynamic_linking` enabled?"
            );
            return Err(ExitCode::FAILURE);
        }
    }

    let out = dist_root.join("sdk");
    let profile_dir = from.profile_dir(repo);
    stage(&profile_dir, plat, &out, &artifacts, &triple, &externs).map_err(|e| {
        eprintln!("[xtask] sdk staging failed: {e}");
        ExitCode::FAILURE
    })?;
    Ok(out)
}

/// Run the build and collect the `.rlib` / proc-macro / import-library paths it
/// reports.
///
/// `--message-format=json-render-diagnostics` rather than plain `json`: the
/// artifact lines still arrive on stdout, but warnings and errors keep their
/// normal human rendering on stderr instead of arriving as JSON nobody reads.
/// The two `--extern` targets a plugin build must be given, as `deps/`-relative
/// filenames. See [`capture_extern`] for why each is the file it is.
#[derive(Default)]
pub struct Externs {
    bevy: Option<String>,
    renzora: Option<String>,
    ember: Option<String>,
}

fn artifact_files(
    repo: &Path,
    from: &From,
    externs: &mut Externs,
) -> Result<Vec<(PathBuf, bool)>, String> {
    // Must be the profile the engine was actually built with, or this resolves a
    // different set of artifacts and the SDK describes a build nobody has.
    let profile = crate::profile();
    let mut cmd = Command::new("cargo");
    cmd.current_dir(repo).args([
        "build",
        "--profile",
        &profile,
        "--workspace",
        // BYTE-FOR-BYTE the engine build's selection — same exclusions as
        // `crate::build` and `docker/build-all.sh`, and deliberately no `--bin`
        // filter.
        //
        // This step exists to re-READ a finished build, and any difference in
        // package selection stops it doing that. Under resolver v2 features are
        // unified over the SELECTED packages, so narrowing to
        // `--bin renzora --bin renzora-editor` resolves a different feature set,
        // re-fingerprints the graph, and cargo rebuilds it — observed pulling
        // `wasm-bindgen`, `js-sys` and `web-sys` into a linux-x64 lane that had
        // no business with them.
        //
        // Restricting the targets looked like it would save work. It cost a
        // second compilation of the entire workspace.
        "--exclude",
        "renzora-android",
        "--exclude",
        "renzora-ios",
        "--message-format=json-render-diagnostics",
    ]);
    // Must match the build being staged EXACTLY. A different `--target-dir` or a
    // missing `--target` does not fail — it resolves a different set of
    // artifacts, and the SDK then describes a build nobody has.
    if let Some(dir) = &from.target_dir {
        cmd.args(["--target-dir", dir]);
    }
    if let Some(target) = &from.target {
        cmd.args(["--target", target]);
    }
    let out = cmd
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| format!("spawn cargo: {e}"))?;
    if !out.status.success() {
        return Err(format!("cargo exited {}", out.status));
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let mut files = BTreeSet::new();
    for line in text.lines() {
        if !line.contains("\"reason\":\"compiler-artifact\"") {
            continue;
        }
        capture_extern(line, externs);
        // Classify by cargo's own `target.kind`, NEVER by file extension. A
        // proc-macro and a linkable dylib are both a `.dll` here, and they must
        // land in different directories — but a linkable dylib also ships a
        // `.dll.lib` import library that rustc expects to find BESIDE it.
        // Splitting that pair by extension produces a link that silently omits
        // `bevy_dylib` and then fails on undefined generic symbols
        // (`RawVec::grow_one`, `FixedBitSet::drop`) that name nothing a reader
        // could connect back to a staging bug.
        let proc_macro = json_string_array(line, "kind").iter().any(|k| k == "proc-macro");
        let extern_target = is_bevy_extern(line);
        for f in json_string_array(line, "filenames") {
            // `.rlib` is the metadata a plugin typechecks against — staged as
            // the `.rmeta` half of it, see [`metadata_only`]; `.dll`/`.so` is
            // either a proc-macro rustc *runs*, or a dylib it links; `.lib` is
            // the MSVC import library for the latter (Unix links the
            // `.so`/`.dylib` directly and has no analogue).
            if f.ends_with(".rlib") {
                let p = Path::new(&f);
                files.insert((if extern_target { p.to_path_buf() } else { metadata_only(p) }, proc_macro));
            } else if f.ends_with(".lib") || is_shared_lib(&f) {
                files.insert((PathBuf::from(f), proc_macro));
            }
        }
    }
    Ok(files.into_iter().collect())
}

fn is_shared_lib(f: &str) -> bool {
    f.ends_with(".dll") || f.ends_with(".so") || f.ends_with(".dylib")
}

/// Is this artifact line the `bevy` facade a plugin is handed by `--extern`?
///
/// The one crate that keeps its full `.rlib`, and the one place the rule in
/// [`metadata_only`] does not hold — see "The one crate that keeps its `.rlib`"
/// in the module docs. Selected on the `dynamic_linking` feature for the same
/// reason [`capture_extern`] does: two `bevy` units exist and only one is the
/// right one.
fn is_bevy_extern(line: &str) -> bool {
    json_string(line, "name").as_deref() == Some("bevy")
        && json_string_array(line, "features").iter().any(|f| f == "dynamic_linking")
}

/// The metadata half of an `.rlib`: the sibling `.rmeta` cargo emitted for the
/// same compilation unit.
///
/// Falls back to the `.rlib` itself when there is none, which is what happens
/// for a unit cargo did not pipeline. Staging the archive there costs disk and
/// nothing else, so the fallback is silent — the point is that no unit can go
/// missing because its metadata was not written separately.
///
/// This is safe in a way a directory scan is not (see the module docs). The
/// `.rmeta` is not *searched for* by crate name among the many `-C metadata`
/// variants in `deps/`; it is derived from the exact path cargo just reported,
/// keeping the same hash, so it can only be the metadata of that one unit.
fn metadata_only(rlib: &Path) -> PathBuf {
    let rmeta = rlib.with_extension("rmeta");
    if rmeta.is_file() { rmeta } else { rlib.to_path_buf() }
}

/// Note which artifacts a plugin build should point `--extern` at.
///
/// The two are NOT symmetric, and getting either wrong fails in a way that does
/// not name the cause:
///
/// * **`bevy` → the facade crate's metadata.** `bevy` is an ordinary rlib that
///   itself declares `extern crate bevy_dylib` under `dynamic_linking`, so
///   pointing at it is what routes the code to the shared image. Pointing
///   `--extern bevy` at `bevy_dylib` directly does not work at all: that crate
///   re-exports `bevy_internal`, so `bevy::prelude` does not resolve.
///
///   Its `.rmeta` rather than its `.rlib`, like every other staged crate — the
///   facade contributes no code of its own to link, only the declaration that
///   sends `rustc` to `bevy_dylib` (module docs).
///
///   Two `bevy` units exist in this workspace, built for different feature
///   resolutions, and only one carries `dynamic_linking`. The other compiles a
///   plugin fine and then fails to link. Selecting on the feature is exact;
///   selecting on filename order is a coin toss.
///
/// * **`renzora` → the `renzora_dylib` shared library, aliased.** The contract
///   crate has no facade to declare the `extern crate` for it, so the alias is
///   the only thing that routes its code to the shared image. Pointing at
///   `librenzora.rlib` instead compiles AND links, and silently gives the plugin
///   a private copy of the contract crate's process-global state — a dead
///   translation table, warnings and logs going to buffers nobody drains.
///
/// * **`renzora_ember` → the `renzora_ember_dylib` shared library, aliased.**
///   The same shape as `renzora`, one layer up, and it fails the same way:
///   `librenzora_ember.rlib` compiles and links, and hands the plugin a private
///   theme palette, stylesheet and toolbar list. Its panel then paints in
///   ember's default colours no matter what theme the user picked. Measured, a
///   one-panel plugin is 32.2 MB the wrong way and 0.09 MB the right way.
fn capture_extern(line: &str, out: &mut Externs) {
    let Some(name) = json_string(line, "name") else {
        return;
    };
    let files = json_string_array(line, "filenames");
    match name.as_str() {
        "bevy" if is_bevy_extern(line) => {
            // The `.rlib`, not the `.rmeta`: this is the one crate `is_bevy_extern`
            // keeps whole, so it is also the one whose staged name is still the
            // archive's. The two must agree or the manifest points at a file the
            // SDK does not contain.
            out.bevy = files
                .iter()
                .find(|f| f.ends_with(".rlib"))
                .and_then(|f| Path::new(f).file_name()?.to_str().map(str::to_string));
        }
        "renzora_dylib" => {
            out.renzora = files
                .iter()
                .filter(|f| is_shared_lib(f))
                .filter_map(|f| Path::new(f).file_name()?.to_str().map(str::to_string))
                .next();
        }
        "renzora_ember_dylib" => {
            out.ember = files
                .iter()
                .filter(|f| is_shared_lib(f))
                .filter_map(|f| Path::new(f).file_name()?.to_str().map(str::to_string))
                .next();
        }
        _ => {}
    }
}

/// Pull a `"field": "value"` string out of JSON text.
///
/// Whitespace after the colon is optional because both shapes occur here:
/// cargo's `--message-format=json` is dense, while the manifest this crate
/// writes is pretty-printed for anyone reading it in a release.
pub(crate) fn json_string(text: &str, field: &str) -> Option<String> {
    let rest = after_key(text, field)?;
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// The text just past `"field":` and any following whitespace.
fn after_key<'a>(text: &'a str, field: &str) -> Option<&'a str> {
    let key = format!("\"{field}\":");
    let start = text.find(&key)? + key.len();
    Some(text[start..].trim_start())
}

/// Pull a `"field":["a","b"]` array of strings out of one JSON line.
///
/// Hand-rolled because xtask carries no dependencies on purpose. It scans quotes
/// and escapes rather than splitting on commas: these values are absolute paths,
/// Windows ones arrive with every separator escaped as `\\`, and a naive split
/// would also break on any path containing a comma.
pub(crate) fn json_string_array(text: &str, field: &str) -> Vec<String> {
    let Some(rest) = after_key(text, field).and_then(|r| r.strip_prefix('[')) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_str = false;
    let mut escaped = false;
    for c in rest.chars() {
        if !in_str {
            match c {
                '"' => in_str = true,
                ']' => break,
                _ => {}
            }
        } else if escaped {
            cur.push(c);
            escaped = false;
        } else {
            match c {
                '\\' => escaped = true,
                '"' => {
                    in_str = false;
                    out.push(std::mem::take(&mut cur));
                }
                _ => cur.push(c),
            }
        }
    }
    out
}

/// Copy the collected artifacts, plus the native import-library directories, and
/// write the manifest that ties them to the engine build they belong to.
fn stage(
    profile_dir: &Path,
    plat: &Platform,
    out: &Path,
    artifacts: &[(PathBuf, bool)],
    triple: &str,
    externs: &Externs,
) -> std::io::Result<()> {
    let deps = out.join("deps");
    let host_deps = out.join("host-deps");
    let native = out.join("native");
    for d in [&deps, &host_deps, &native] {
        // Wipe first: a crate removed from the workspace must not linger here as
        // a stale metadata file that a later plugin build could still resolve.
        let _ = std::fs::remove_dir_all(d);
        std::fs::create_dir_all(d)?;
    }

    let (mut n_meta, mut n_proc, mut n_imp) = (0usize, 0usize, 0usize);
    for (src, proc_macro) in artifacts {
        let Some(name) = src.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        // Proc-macro crates are HOST artifacts — the compiler loads and executes
        // them, so when cross-compiling they are built for a different target
        // than everything else. Their own directory is what lets the plugin
        // build pass two distinct `-L dependency` paths.
        //
        // Everything else goes to `deps/` together, which keeps each linkable
        // dylib next to its import library.
        let (dst_dir, counter) = if *proc_macro {
            (&host_deps, &mut n_proc)
        } else if name.ends_with(".rmeta") || name.ends_with(".rlib") {
            (&deps, &mut n_meta)
        } else {
            (&deps, &mut n_imp)
        };
        if src.exists() {
            link_or_copy(src, &dst_dir.join(name))?;
            *counter += 1;
        }
    }

    let mut native_dirs = stage_native(profile_dir, &native)?;
    native_dirs.extend(stage_msvc(&native, triple)?);

    let build_id = build_id(out, externs);
    let counts = Counts { metadata: n_meta, proc_macro: n_proc, import_lib: n_imp };
    let manifest = manifest_json(plat, triple, &native_dirs, externs, &build_id, &counts);
    std::fs::write(out.join("manifest.json"), manifest)?;

    println!(
        "[xtask] sdk: {n_meta} metadata, {n_proc} proc-macro, {n_imp} import lib, \
         {} native dir(s)",
        native_dirs.len()
    );
    Ok(())
}

/// Copy every directory a build script announced via `cargo:rustc-link-search`.
///
/// These hold the native import libraries the linker needs — `windows.0.52.0.lib`
/// and friends — and **a plugin build fails without them**, with
/// `LNK1181: cannot open input file`, which names the library but nothing that
/// explains where it was supposed to come from.
///
/// They have to be copied rather than referenced, because most of them point
/// into `target/dist/build/<pkg>/out`, a directory that exists only on the
/// machine that built the engine. Each is flattened to a numbered subdirectory
/// and recorded in the manifest, so the plugin build can regenerate the
/// `-L native=` flags from paths that exist on the machine doing the compiling.
fn stage_native(profile_dir: &Path, native: &Path) -> std::io::Result<Vec<String>> {
    let build = profile_dir.join("build");
    let mut sources = BTreeSet::new();
    if let Ok(entries) = std::fs::read_dir(&build) {
        for e in entries.flatten() {
            let output = e.path().join("output");
            let Ok(text) = std::fs::read_to_string(&output) else {
                continue;
            };
            for line in text.lines() {
                // Both spellings are live: `cargo:` is the classic form and
                // `cargo::` the newer namespaced one, and a workspace this size
                // has build scripts of both vintages.
                let Some(rest) = line
                    .strip_prefix("cargo::rustc-link-search=")
                    .or_else(|| line.strip_prefix("cargo:rustc-link-search="))
                else {
                    continue;
                };
                // The `KIND=PATH` form is optional. Only `native` (and the bare
                // default, which means native) name import-library directories.
                let path = match rest.split_once('=') {
                    Some(("native", p)) => p,
                    Some((_, _)) => continue,
                    None => rest,
                };
                sources.insert(path.to_string());
            }
        }
    }

    let mut staged = Vec::new();
    for src in sources.iter() {
        let src = Path::new(src);
        if !src.is_dir() {
            continue;
        }
        let name = format!("{:02}", staged.len());
        let dst = native.join(&name);
        std::fs::create_dir_all(&dst)?;
        let mut kept = 0usize;
        for e in std::fs::read_dir(src)?.flatten() {
            let p = e.path();
            // One level only, and only what a linker can consume. Any
            // subdirectory is a build script's own scratch space — and so is
            // most of what sits BESIDE the library it produced.
            if p.is_file() && is_link_input(&p) {
                link_or_copy(&p, &dst.join(e.file_name()))?;
                kept += 1;
            }
        }
        // A source directory that contributed no link input is not a `-L native=`
        // path, it is a build script's temp dir that happened to be announced.
        // Dropping it here keeps it out of the manifest too, so the plugin build
        // does not carry a search path with nothing to find in it.
        if kept == 0 {
            let _ = std::fs::remove_dir(&dst);
            continue;
        }
        staged.push(name);
    }
    Ok(staged)
}

/// Can the linker consume this file, or is it a build script's leftovers?
///
/// These directories are `target/dist/build/<pkg>/out` — a build script's whole
/// working directory, not a curated lib folder. Alongside the `.lib` it produced
/// sit the object files it compiled to get there, plus `flag_check.c` /
/// `flag_check.obj` probes and stray `.rs`. Measured here: 55 MB of real link
/// input against 9 MB of scratch, 175 `.o` files among it.
///
/// # Why an extension allowlist is safe, having checked
///
/// The worry is a build script that hands the linker an object file directly
/// rather than an archive. That reaches a build through
/// `cargo:rustc-link-arg` — and **this workspace emits none** (grepped across
/// every `target/dist/build/*/output`; the count is zero). Every directive here
/// is a `rustc-link-lib=static=<name>` or `dylib=<name>`, each of which resolves
/// to `<name>.lib` on Windows or `lib<name>.a` on Unix.
///
/// It could not reach a *plugin* link in any case: `rustc-link-arg` applies to
/// the package's own targets, never to a downstream dependent, and a plugin
/// build is a fresh `rustc` that gets only the `-L native=` paths from the
/// manifest. The names it asks for arrive through crate metadata, and they are
/// always library names.
///
/// Shared libraries are kept as well as static ones. Windows resolves a dylib
/// dependency through its `.lib` import library so only that matters here, but
/// a Linux or macOS SDK can legitimately have a `.so`/`.dylib` sitting in one of
/// these directories, and dropping it would break that platform only — the worst
/// kind of bug to introduce from a Windows machine.
fn is_link_input(p: &Path) -> bool {
    let Some(ext) = p.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(ext, "lib" | "a" | "so" | "dylib" | "tbd")
}

/// The MSVC libraries a plugin link names but Rust does not supply.
///
/// `kernel32`, `user32`, `msvcrt` and friends come from Visual Studio and the
/// Windows SDK. On a developer's machine rustc finds them by auto-detecting the
/// Visual Studio install — which is why every plugin built here has linked
/// without them being staged, and why that proves nothing about a machine
/// without Visual Studio, where the link fails with
/// `LNK1181: cannot open input file 'kernel32.lib'`.
///
/// Asking a user to install Build Tools to try a plugin is not reasonable, and
/// it is not necessary: Microsoft permits redistributing these, which is what
/// `xwin` exists for and what `docker/windows/Dockerfile` already relies on to
/// cross-compile Windows binaries from Linux with no Visual Studio present. The
/// same three `-L native=` paths work locally.
///
/// This is a curated list rather than the whole `um/x64` directory, which is
/// hundreds of megabytes against these thirteen at ~10 MB. Curation is a little
/// brittle — a plugin using an API nobody here has used yet needs another entry
/// — but the failure is loud and names the missing file, so extending it is a
/// one-line change with an obvious trigger.
const MSVC_LIBS: &[&str] = &[
    "kernel32", "user32", "shell32", "gdi32", "advapi32", "opengl32", "ntdll", "userenv",
    "ws2_32", "dbghelp", "msvcrt", "vcruntime", "ucrt",
];

/// Copy [`MSVC_LIBS`] into the SDK, from wherever this machine keeps them.
///
/// Returns the manifest-relative directory, or nothing when the target does not
/// need them — Linux and macOS have a system linker and libc already, so this is
/// Windows-only.
fn stage_msvc(native: &Path, triple: &str) -> std::io::Result<Option<String>> {
    if !triple.contains("windows-msvc") {
        return Ok(None);
    }
    let sources = msvc_lib_dirs();
    if sources.is_empty() {
        // The normal case on a developer's machine, and not a problem there:
        // rustc auto-detects Visual Studio, which is how the engine itself was
        // just built. Only an SDK that will be SHIPPED needs these carried with
        // it, and that one is cut where xwin is. Silent rather than a warning,
        // because a warning on every local build teaches people to ignore it.
        return Ok(None);
    }

    let dst = native.join("msvc");
    std::fs::create_dir_all(&dst)?;
    let mut found = 0usize;
    for lib in MSVC_LIBS {
        let name = format!("{lib}.lib");
        if let Some(src) = sources.iter().map(|d| d.join(&name)).find(|p| p.is_file()) {
            link_or_copy(&src, &dst.join(&name))?;
            found += 1;
        }
    }
    if found < MSVC_LIBS.len() {
        eprintln!(
            "[xtask] sdk: {found}/{} MSVC import libraries found",
            MSVC_LIBS.len()
        );
    }
    Ok(Some("msvc".to_string()))
}

/// Directories holding the MSVC + Windows SDK import libraries, or none.
///
/// Two sources, and deliberately no third.
///
/// **xwin's splat**, at the fixed paths `docker/windows/Dockerfile` already
/// passes to the engine build. This is the one that matters: the SDK a user
/// downloads is cut by CI, and CI's Windows lane is that container.
///
/// **`RENZORA_MSVC_LIBS`**, a `;`-separated list, for a lane that has the
/// libraries somewhere else — a native Windows runner, or a developer producing
/// a shippable SDK by hand.
///
/// # Why no Visual Studio discovery
///
/// An earlier version globbed `C:/Program Files/Microsoft Visual Studio/...` and
/// `C:/Program Files (x86)/Windows Kits/...`. That was wrong twice over. It
/// hardcodes a drive letter, so a machine with Visual Studio on D: silently
/// stages nothing; and it hardcodes `Program Files`, ignoring redirection. Doing
/// it *properly* means `vswhere.exe` for Visual Studio and a registry read for
/// the Windows Kits root — real work, in a helper that is meant to stay
/// dependency-free.
///
/// It also buys nothing. A developer's machine already has these libraries where
/// rustc auto-detects them, which is why plugins have always linked here without
/// staging any. Only the *shipped* SDK needs them baked in, and that is built
/// where xwin is. So finding nothing is normal on a dev host, not a failure.
fn msvc_lib_dirs() -> Vec<PathBuf> {
    if let Ok(list) = std::env::var("RENZORA_MSVC_LIBS") {
        let dirs: Vec<PathBuf> = list
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
            .collect();
        if !dirs.is_empty() {
            return dirs;
        }
    }
    ["/xwin/crt/lib/x86_64", "/xwin/sdk/lib/um/x86_64", "/xwin/sdk/lib/ucrt/x86_64"]
        .iter()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .collect()
}

/// The manifest a plugin build reads to reconstruct the `rustc` command line.
///
/// Hand-written rather than serialised, because xtask carries no dependencies on
/// purpose (see its `Cargo.toml`) and this is the only JSON it ever emits.
///
/// Both `extern` entries are `deps/`-relative paths INTO THE SDK, including the
/// shared `renzora_dylib` — even though an identical copy sits beside the editor
/// executable. On Windows the linker needs the `.dll.lib` import library beside
/// the `.dll`, and only the SDK copy has that pairing; pointing at the staged
/// runtime copy instead gets as far as `undefined symbol: renzora::lang::t`.
/// How many of each kind of artifact was staged.
///
/// Bundled rather than passed as three `usize`s so they cannot be swapped at the
/// call site, where nothing would catch it — the manifest would simply report a
/// wrong count that no reader could tell was wrong.
struct Counts {
    /// Crate metadata files — `.rmeta`, or an `.rlib` for a unit that had none.
    metadata: usize,
    proc_macro: usize,
    import_lib: usize,
}

fn manifest_json(
    plat: &Platform,
    triple: &str,
    native_dirs: &[String],
    externs: &Externs,
    build_id: &str,
    counts: &Counts,
) -> String {
    let natives = native_dirs
        .iter()
        .map(|d| format!("\"native/{d}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let bevy = externs.bevy.as_deref().unwrap_or_default();
    let renzora = externs.renzora.as_deref().unwrap_or_default();
    let ember = externs.ember.as_deref().unwrap_or_default();
    let rustc = rustc_version().unwrap_or_default();
    format!(
        "{{\n  \"triple\": \"{triple}\",\n  \"rustc\": \"{rustc}\",\n  \"lib_ext\": \"{}\",\n  \
         \"build_id\": \"{build_id}\",\n  \
         \"extern\": {{\n    \"bevy\": \"deps/{bevy}\",\n    \"renzora\": \"deps/{renzora}\",\n    \
         \"renzora_ember\": \"deps/{ember}\"\n  }},\n  \
         \"link_search\": {{\n    \"dependency\": [\"deps\", \"host-deps\"],\n    \
         \"native\": [{natives}]\n  }},\n  \
         \"counts\": {{ \"metadata\": {}, \"proc_macro\": {}, \"import_lib\": {} }}\n}}\n",
        plat.ext, counts.metadata, counts.proc_macro, counts.import_lib
    )
}

/// A token identifying the exact engine build a plugin compiled here is bound to.
///
/// Recorded beside a built plugin and compared on load; a mismatch means rebuild.
///
/// # Why this is a content hash and not a set of filenames
///
/// The obvious stamp — and the first one used here — was the `bevy` rlib
/// filename plus the rustc version, on the reasoning that cargo hashes the whole
/// build configuration into that name so nothing here has to decide what
/// "compatible" means. That is true of the *configuration* and false of the
/// thing that actually matters.
///
/// Cargo's `-C metadata` hash is computed from the package id, features, profile
/// and target — **not** from source content. So editing `crates/renzora`, which
/// is where boundary-crossing types are required to live and therefore the crate
/// most likely to move under a plugin, leaves every filename in this SDK exactly
/// as it was. The stamp matched, no plugin rebuilt, and each one kept loading
/// against a `renzora_dylib.dll` whose struct layouts had changed underneath it.
/// Nothing catches that later: Rust symbol mangling encodes the crate's stable
/// id, not its contents, so the imports still resolve and the plugin reads the
/// new image through the old layout.
///
/// Hashing the images the plugin actually binds to has none of that gap. It
/// costs about 30 ms once per SDK stage, and the editor never pays it — it reads
/// the result out of the manifest.
///
/// The dependency filenames go in too, cheaply: a dep whose metadata hash moved
/// (a version bump, a feature change) is a real incompatibility that the three
/// extern artifacts might not reflect.
fn build_id(out: &Path, externs: &Externs) -> String {
    // FNV-1a, 64-bit. Hand-rolled because xtask carries no dependencies on
    // purpose, and nothing here needs collision resistance against an adversary
    // — only against two different engine builds accidentally agreeing.
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(PRIME);
        }
    };

    // Every staged metadata file, by name. Sorted so the hash does not depend on
    // directory iteration order, which varies between machines.
    for dir in ["deps", "host-deps"] {
        let mut names: Vec<String> = std::fs::read_dir(out.join(dir))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .collect();
        names.sort();
        for n in names {
            eat(n.as_bytes());
        }
    }

    // The three images a plugin links, by content. This is the half that sees a
    // source edit.
    for name in [&externs.bevy, &externs.renzora, &externs.ember]
        .into_iter()
        .flatten()
    {
        if let Ok(bytes) = std::fs::read(out.join("deps").join(name)) {
            eat(&bytes);
        }
    }
    format!("{h:016x}")
}

/// The target triple this toolchain builds for, e.g. `x86_64-pc-windows-msvc`.
///
/// Read from `rustc -vV` rather than assembled from `cfg!`, so it always matches
/// the string cargo used for the artifacts being staged.
fn host_triple() -> Result<String, String> {
    rustc_vv("host: ")
}

/// The exact rustc that produced these artifacts, e.g. `1.95.0`.
///
/// Recorded because it is a **hard** compatibility boundary, not a hint. Rust's
/// `.rlib` metadata format is versioned and rustc refuses to read another
/// version's — building a plugin with 1.93.0 against a 1.95.0 SDK stops at
/// `error[E0514]: found crate 'bevy' compiled by an incompatible version of
/// rustc`, before a line of the plugin compiles.
///
/// E0514 is a clear message, but it arrives after someone has downloaded the SDK
/// and clicked Install. Comparing this string to the local `rustc -vV` first
/// turns it into "this needs Rust 1.95.0, you have 1.93.0" at the moment it can
/// still be acted on — and `rustup run <version> rustc …` makes acting on it a
/// single flag rather than a toolchain the user has to manage.
fn rustc_version() -> Result<String, String> {
    rustc_vv("release: ")
}

fn rustc_vv(field: &str) -> Result<String, String> {
    let out = Command::new("rustc")
        .arg("-vV")
        .output()
        .map_err(|e| format!("spawn rustc: {e}"))?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix(field))
        .map(|s| s.trim().to_string())
        .ok_or_else(|| format!("no `{}` line in `rustc -vV`", field.trim_end()))
}
