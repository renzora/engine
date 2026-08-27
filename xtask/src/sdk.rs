//! `cargo renzora sdk` — stage the compile-time half of a plugin build.
//!
//! A marketplace plugin ships as Rust source and is compiled on the machine that
//! installs it, against the engine that is already there. For that to work the
//! user needs what `rustc` reads at compile time, which is emphatically *not*
//! what the editor needs at run time:
//!
//!   * `dist/<platform>/` holds the **code** — `bevy_dylib`, `renzora_dylib`,
//!     `std`. Loaded when the editor starts. Staged by [`crate::stage`].
//!   * `dist/sdk-<triple>/` holds the **blueprints** — `.rlib` metadata,
//!     proc-macro dylibs, native import libraries. Read by `rustc`, never
//!     loaded. Staged here.
//!
//! The two cannot be merged. Linking a dylib discards the metadata of everything
//! it swallowed: `renzora_dylib.dll` carries `renzora_dylib`'s own metadata and
//! nothing else, so a compiler pointed at it alone cannot even resolve `Query`.
//! Measured the other way round too — a plugin that reads a 39 MB `bevy_ecs`
//! rlib links to 0.29 MB, because only the metadata half is consumed.
//!
//! # Why it is staged on every build
//!
//! An SDK only works against the engine it was cut from. Every `.rlib` filename
//! carries a `-C metadata` hash of the build configuration, and `renzora_dylib`
//! changes whenever the contract crate does — so a stale SDK beside a fresh
//! editor produces plugins that fail to link, or worse, link against a
//! mismatched image. Staging it alongside the binaries is what makes "stale" a
//! state that cannot occur locally.
//!
//! That is affordable only because the files are **hardlinked**, not copied.
//! `target/` and `dist/` sit on one volume, so ~1050 links cost no disk and
//! effectively no time; a plain copy of 3.6 GB on every build would not be
//! tolerable. `link_or_copy` falls back to copying if the link fails, which is
//! what happens when someone puts `dist/` on another drive.
//!
//! It is still a separate **download**: `dist/<platform>/sdk/` is excluded from
//! the editor archive and published as its own asset, because most people never
//! write a plugin and should not pay ~555 MB compressed for the ability.
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

use crate::Platform;

/// Stage everything a plugin compile needs into `<dist>/sdk/`.
///
/// `dist_root` is the staged platform directory — the same one holding the two
/// executables and the shared dylibs, so the SDK travels with the build it was
/// cut from.
pub fn build(repo: &Path, plat: &Platform, dist_root: &Path) -> Result<PathBuf, ExitCode> {
    let triple = host_triple().map_err(|e| {
        eprintln!("[xtask] {e}");
        ExitCode::FAILURE
    })?;

    // A second cargo invocation, and a deliberate one. The normal build's output
    // is what a developer reads; `--message-format=json` would replace the
    // `Compiling …` progress with machine output. On an already-built tree this
    // re-run resolves in a second or two and reprints the artifact list, which is
    // the only thing wanted here.
    let mut externs = Externs::default();
    let artifacts = artifact_files(repo, &mut externs).map_err(|e| {
        eprintln!("[xtask] sdk build failed: {e}");
        ExitCode::FAILURE
    })?;
    // Refuse to write an SDK that cannot compile anything. Both are recorded from
    // the build itself, so a miss means the workspace shape changed underneath
    // this code — worth stopping for, since the alternative is an SDK that ships
    // and then fails on someone else's machine.
    for (what, got) in [("bevy", &externs.bevy), ("renzora", &externs.renzora)] {
        if got.is_none() {
            eprintln!(
                "[xtask] sdk: no `--extern {what}` candidate in the build — is \
                 `dynamic_linking` enabled?"
            );
            return Err(ExitCode::FAILURE);
        }
    }

    let out = dist_root.join("sdk");
    stage(repo, plat, &out, &artifacts, &triple, &externs).map_err(|e| {
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
}

fn artifact_files(repo: &Path, externs: &mut Externs) -> Result<Vec<(PathBuf, bool)>, String> {
    let out = Command::new("cargo")
        .current_dir(repo)
        .args([
            "build",
            "--profile",
            "dist",
            "--workspace",
            "--bin",
            "renzora",
            "--bin",
            "renzora-editor",
            "--message-format=json-render-diagnostics",
        ])
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
        for f in json_string_array(line, "filenames") {
            // `.rlib` is the metadata a plugin typechecks against; `.dll`/`.so`
            // is either a proc-macro rustc *runs*, or a dylib it links; `.lib`
            // is the MSVC import library for the latter (Unix links the
            // `.so`/`.dylib` directly and has no analogue).
            if f.ends_with(".rlib") || f.ends_with(".lib") || is_shared_lib(&f) {
                files.insert((PathBuf::from(f), proc_macro));
            }
        }
    }
    Ok(files.into_iter().collect())
}

fn is_shared_lib(f: &str) -> bool {
    f.ends_with(".dll") || f.ends_with(".so") || f.ends_with(".dylib")
}

/// Hardlink `src` into `dst`, copying only if that fails.
///
/// This is what makes staging 3.6 GB on every build a non-event: `target/` and
/// `dist/` are normally the same volume, so a link is instant and consumes no
/// additional disk. Deleting `target/` later does not break the SDK — the data
/// survives while any link to it remains.
///
/// The fallback matters for the cases where linking cannot work: `dist/` on a
/// different drive, or a filesystem without hardlinks. Those pay the copy, which
/// is what the code did before and is merely slow rather than wrong.
fn link_or_copy(src: &Path, dst: &Path) -> std::io::Result<()> {
    // A leftover from a previous stage would make `hard_link` fail with
    // AlreadyExists and silently fall through to a copy, so clear it first.
    let _ = std::fs::remove_file(dst);
    match std::fs::hard_link(src, dst) {
        Ok(()) => Ok(()),
        Err(_) => crate::copy(src, dst),
    }
}

/// Note which artifacts a plugin build should point `--extern` at.
///
/// The two are NOT symmetric, and getting either wrong fails in a way that does
/// not name the cause:
///
/// * **`bevy` → the facade `.rlib`.** `bevy` is an ordinary rlib that itself
///   declares `extern crate bevy_dylib` under `dynamic_linking`, so pointing at
///   it is what routes the code to the shared image. Pointing `--extern bevy` at
///   `bevy_dylib` directly does not work at all: that crate re-exports
///   `bevy_internal`, so `bevy::prelude` does not resolve.
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
fn capture_extern(line: &str, out: &mut Externs) {
    let Some(name) = json_string(line, "name") else {
        return;
    };
    let files = json_string_array(line, "filenames");
    let basename = |ext: &str| {
        files
            .iter()
            .filter(|f| f.ends_with(ext))
            .filter_map(|f| Path::new(f).file_name()?.to_str().map(str::to_string))
            .next()
    };
    match name.as_str() {
        "bevy" if json_string_array(line, "features").iter().any(|f| f == "dynamic_linking") => {
            out.bevy = basename(".rlib");
        }
        "renzora_dylib" => {
            out.renzora = files
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
    repo: &Path,
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

    let (mut n_rlib, mut n_proc, mut n_imp) = (0usize, 0usize, 0usize);
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
        } else if name.ends_with(".rlib") {
            (&deps, &mut n_rlib)
        } else {
            (&deps, &mut n_imp)
        };
        if src.exists() {
            link_or_copy(src, &dst_dir.join(name))?;
            *counter += 1;
        }
    }

    let mut native_dirs = stage_native(repo, &native)?;
    native_dirs.extend(stage_msvc(&native, triple)?);

    let manifest = manifest_json(plat, triple, &native_dirs, externs, n_rlib, n_proc, n_imp);
    std::fs::write(out.join("manifest.json"), manifest)?;

    println!(
        "[xtask] sdk: {n_rlib} rlib, {n_proc} proc-macro, {n_imp} import lib, \
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
fn stage_native(repo: &Path, native: &Path) -> std::io::Result<Vec<String>> {
    let build = repo.join("target").join("dist").join("build");
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
    for (i, src) in sources.iter().enumerate() {
        let src = Path::new(src);
        if !src.is_dir() {
            continue;
        }
        let name = format!("{i:02}");
        let dst = native.join(&name);
        std::fs::create_dir_all(&dst)?;
        for e in std::fs::read_dir(src)?.flatten() {
            let p = e.path();
            // One level only. These directories hold loose `.lib`/`.a` files;
            // any subdirectory is a build script's own scratch space.
            if p.is_file() {
                link_or_copy(&p, &dst.join(e.file_name()))?;
            }
        }
        staged.push(name);
    }
    Ok(staged)
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
        // Not fatal: a build here still links via rustc's own auto-detection.
        // Only the *shipped* SDK needs these, and that is built where they are.
        eprintln!(
            "[xtask] sdk: no MSVC import libraries found — plugins will build on \
             this machine but not on one without Visual Studio"
        );
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

/// Directories that may hold the MSVC + Windows SDK import libraries.
///
/// Two shapes. In the cross-compile container they are xwin's splat at fixed
/// paths; on a Windows host they are under the Visual Studio and Windows Kits
/// installs, whose version directories are globbed rather than pinned so a
/// toolchain update does not silently stop finding them.
fn msvc_lib_dirs() -> Vec<PathBuf> {
    let xwin = [
        "/xwin/crt/lib/x86_64",
        "/xwin/sdk/lib/um/x86_64",
        "/xwin/sdk/lib/ucrt/x86_64",
    ];
    let from_xwin: Vec<PathBuf> = xwin.iter().map(PathBuf::from).filter(|p| p.is_dir()).collect();
    if !from_xwin.is_empty() {
        return from_xwin;
    }

    let mut out = Vec::new();

    // Windows Kits: <Lib>/<version>/{um,ucrt}/x64 — this is where `kernel32`,
    // `user32` and `ucrt` live.
    for ver in newest_first("C:/Program Files (x86)/Windows Kits/10/Lib") {
        out.extend([ver.join("um/x64"), ver.join("ucrt/x64")]);
    }

    // Visual Studio: <root>/<year>/<edition>/VC/Tools/MSVC/<version>/lib/x64 —
    // `msvcrt` and `vcruntime` live only here, and the EDITION level is easy to
    // miss (2022/Community, not just 2022). Omitting it finds 11 of the 13 libs
    // and fails on exactly the two that are not part of the Windows SDK.
    for year in newest_first("C:/Program Files/Microsoft Visual Studio") {
        for edition in newest_first(year.to_string_lossy().as_ref()) {
            for ver in newest_first(edition.join("VC/Tools/MSVC").to_string_lossy().as_ref()) {
                out.push(ver.join("lib/x64"));
            }
        }
    }

    out.retain(|p| p.is_dir());
    out
}

/// Subdirectories of `root`, newest name first.
///
/// Versions sort lexically well enough here (`10.0.26100.0`, `14.44.35207`), and
/// newest-first means a machine with several installs uses the one Visual Studio
/// itself would pick.
fn newest_first(root: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    dirs.reverse();
    dirs
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
fn manifest_json(
    plat: &Platform,
    triple: &str,
    native_dirs: &[String],
    externs: &Externs,
    n_rlib: usize,
    n_proc: usize,
    n_imp: usize,
) -> String {
    let natives = native_dirs
        .iter()
        .map(|d| format!("\"native/{d}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let bevy = externs.bevy.as_deref().unwrap_or_default();
    let renzora = externs.renzora.as_deref().unwrap_or_default();
    let rustc = rustc_version().unwrap_or_default();
    format!(
        "{{\n  \"triple\": \"{triple}\",\n  \"rustc\": \"{rustc}\",\n  \"lib_ext\": \"{}\",\n  \
         \"extern\": {{\n    \"bevy\": \"deps/{bevy}\",\n    \"renzora\": \"deps/{renzora}\"\n  }},\n  \
         \"link_search\": {{\n    \"dependency\": [\"deps\", \"host-deps\"],\n    \
         \"native\": [{natives}]\n  }},\n  \
         \"counts\": {{ \"rlib\": {n_rlib}, \"proc_macro\": {n_proc}, \"import_lib\": {n_imp} }}\n}}\n",
        plat.ext
    )
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
