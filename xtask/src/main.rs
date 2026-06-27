//! `cargo renzora` — native build + stage + run, the local mirror of Docker.
//!
//! Docker gives a contributor two things: a pinned toolchain (so everyone
//! compiles with the same rustc) and a build that arranges the output into a
//! runnable `dist/` layout. The toolchain pin is handled by `rust-toolchain.toml`
//! (rustup auto-selects 1.95.0). This binary handles the second half WITHOUT a
//! container, for the host platform only:
//!
//!   1. `cargo build --profile dist --workspace` — compile the binary, the
//!      editor bundle cdylib, the shared `bevy_dylib`/`renzora` dylibs, and every
//!      distribution plugin cdylib.
//!   2. Stage `dist/<platform>/` exactly like `docker/build-all.sh`'s
//!      `copy_shared_libs`: bevy_dylib + renzora + the editor bundle + Rust `std`
//!      beside the exe; every OTHER plugin cdylib into `plugins/`.
//!   3. (`run` only) launch the staged binary.
//!
//! Why a staging step at all: a bare `cargo run` leaves the plugin cdylibs flat
//! in `target/dist/` next to the exe, but the dynamic loader scans
//! `<exe-dir>/plugins/` — so those plugins compile but never load. Step 2 is the
//! one thing `cargo run` can't do, and the only reason `cargo renzora` exists.
//!
//! Cross-platform builds stay Docker-only (`renzora build` / `build-all.sh`):
//! this tool only ever produces artifacts for the machine it runs on.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// Host-platform naming. Filled at compile time from `cfg!` because xtask is
/// built for — and run on — the very platform it stages for.
struct Platform {
    /// `dist/<dir>` — matches `build-all.sh`'s platform directory names.
    dir: &'static str,
    /// Shared-library extension, no dot (`dll` / `so` / `dylib`).
    ext: &'static str,
    /// `lib` on Unix, empty on Windows — the Cargo dylib filename prefix.
    lib_prefix: &'static str,
    /// `.exe` on Windows, empty elsewhere.
    exe_suffix: &'static str,
}

fn platform() -> Platform {
    // Arch only distinguishes the dist dir name (x64 vs arm64); the binaries are
    // always native, so we never cross-target here.
    let arm = cfg!(target_arch = "aarch64");
    if cfg!(target_os = "windows") {
        Platform { dir: "windows-x64", ext: "dll", lib_prefix: "", exe_suffix: ".exe" }
    } else if cfg!(target_os = "macos") {
        Platform {
            dir: if arm { "macos-arm64" } else { "macos-x64" },
            ext: "dylib",
            lib_prefix: "lib",
            exe_suffix: "",
        }
    } else {
        Platform {
            dir: if arm { "linux-arm64" } else { "linux-x64" },
            ext: "so",
            lib_prefix: "lib",
            exe_suffix: "",
        }
    }
}

fn main() -> ExitCode {
    // Resolve the workspace root from this crate's manifest dir (`<root>/xtask`)
    // so `cargo renzora` works regardless of the caller's cwd.
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives at <repo>/xtask")
        .to_path_buf();

    let cmd = std::env::args().nth(1).unwrap_or_else(|| "run".to_string());
    let plat = platform();

    match cmd.as_str() {
        // Build + stage + launch — the default `cargo renzora`.
        "run" => {
            let out = match build_and_stage(&repo, &plat) {
                Ok(out) => out,
                Err(code) => return code,
            };
            launch(&repo, &out, &plat)
        }
        // Build + stage only — produce the dist/ folder, don't launch.
        "dist" => match build_and_stage(&repo, &plat) {
            Ok(out) => {
                println!("[xtask] staged {}", out.display());
                ExitCode::SUCCESS
            }
            Err(code) => code,
        },
        other => {
            eprintln!("[xtask] unknown command '{other}' (expected: run | dist)");
            ExitCode::from(2)
        }
    }
}

fn build_and_stage(repo: &Path, plat: &Platform) -> Result<PathBuf, ExitCode> {
    if !build(repo) {
        eprintln!("[xtask] cargo build failed");
        return Err(ExitCode::FAILURE);
    }
    match stage(repo, plat) {
        Ok(out) => Ok(out),
        Err(e) => {
            eprintln!("[xtask] staging failed: {e}");
            Err(ExitCode::FAILURE)
        }
    }
}

/// Compile the workspace exactly as the container's editor lane does
/// (`build-all.sh`): the whole workspace on the `dist` profile, minus the
/// mobile crates (cdylib/staticlib targets that don't belong in a desktop
/// build) and minus this helper itself.
fn build(repo: &Path) -> bool {
    println!("[xtask] cargo build --profile dist --workspace …");
    Command::new(cargo())
        .current_dir(repo)
        .args([
            "build",
            "--profile",
            "dist",
            "--workspace",
            "--exclude",
            "renzora-android",
            "--exclude",
            "renzora-ios",
            "--exclude",
            "xtask",
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Port of `build-all.sh`'s `copy_shared_libs`: arrange `target/dist/` into a
/// clean, runnable `dist/<platform>/`.
fn stage(repo: &Path, plat: &Platform) -> std::io::Result<PathBuf> {
    let src = repo.join("target").join("dist");
    let deps = src.join("deps");
    let out = repo.join("dist").join(plat.dir);
    let plugins = out.join("plugins");
    std::fs::create_dir_all(&plugins)?;

    // Wipe prior artifacts so a removed plugin doesn't linger in dist/. Only the
    // exe + shared libs are swept; any other dist content (configs, assets a
    // packager dropped in) is left alone.
    clean_artifacts(&out, plat)?;
    clean_artifacts(&plugins, plat)?;

    // ── Host binary ──────────────────────────────────────────────────────────
    let bin_name = format!("renzora{}", plat.exe_suffix);
    let host_bin = out.join(&bin_name);
    copy(&src.join(&bin_name), &host_bin)?;
    #[cfg(unix)]
    make_executable(&host_bin)?;

    // ── bevy_dylib — the EXACT one the binary imports ────────────────────────
    // deps/ accumulates one `bevy_dylib-<hash>` per feature config across builds;
    // copying newest-by-mtime can pick a hash the binary does NOT link, giving
    // "bevy_dylib-<hash> not found" at runtime. So read the import name straight
    // out of the just-copied binary; fall back to newest only if unreadable.
    let bevy = bevy_dylib_import(&host_bin, plat)
        .and_then(|name| {
            let p = deps.join(&name);
            p.exists().then_some(p)
        })
        .or_else(|| newest_matching(&deps, &format!("{}bevy_dylib-", plat.lib_prefix), plat.ext));
    match bevy {
        Some(p) => copy(&p, &out.join(file_name(&p)))?,
        None => eprintln!("[xtask] WARN: no bevy_dylib found in {}", deps.display()),
    }

    // ── Rust std (prefer-dynamic links it as a shared lib) ───────────────────
    copy_rust_std(&out, plat)?;

    // ── SDK dylibs that ship beside the exe (host + every plugin link them) ──
    // `renzora` (folded contract + post-process) and `renzora_editor` (the
    // removable editor bundle). Each lives next to the exe, never in plugins/.
    for sdk in ["renzora", "renzora_editor"] {
        let name = format!("{}{}.{}", plat.lib_prefix, sdk, plat.ext);
        let p = src.join(&name);
        if p.exists() {
            copy(&p, &out.join(&name))?;
        }
    }

    // ── Distribution plugin cdylibs → plugins/ ───────────────────────────────
    let mut count = 0;
    for entry in std::fs::read_dir(&src)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let name = file_name(&path);
        if !name.ends_with(&format!(".{}", plat.ext)) || is_not_a_plugin(&name, plat) {
            continue;
        }
        copy(&path, &plugins.join(&name))?;
        count += 1;
    }

    // Native macOS dylibs record their absolute build path as the install name;
    // rewrite to @rpath so the relocated dist/ folder actually resolves at run.
    #[cfg(target_os = "macos")]
    fixup_macos(&out);

    let root = std::fs::read_dir(&out)?.flatten().filter(|e| e.path().is_file()).count();
    println!("[xtask] staged {} ({root} root files, {count} plugins)", out.display());
    Ok(out)
}

/// Files that look like plugin cdylibs but must NOT be swept into `plugins/`:
/// the shared SDK dylibs (shipped beside the exe), Rust internals, the editor
/// bundle (loaded from beside the exe), and a few crates that emit a cdylib but
/// carry no plugin (`plugin_bevy_hash`) — the loader would reject them, and a
/// stale one from the cargo cache would ship as dead weight. Mirrors the skip
/// list in `build-all.sh`.
fn is_not_a_plugin(name: &str, plat: &Platform) -> bool {
    let p = plat.lib_prefix;
    let e = plat.ext;
    let is = |stem: &str| name == format!("{p}{stem}.{e}");
    name.contains("bevy_dylib")
        || name.starts_with("std-")
        || name.starts_with("libstd-")
        || name.contains("renzora_macros")
        || is("renzora")
        || is("renzora_editor")
        || is("renzora_editor_bundle") // pre-rename name, in case it lingers in cache
        || is("renzora_postprocess") // now an rlib shim; a stale dylib has no add!
        || is("renzora_preview") // wasm helper cdylib, not an engine plugin
}

/// Scan the binary for its `(lib)?bevy_dylib-<hex>.<ext>` import string. Bevy's
/// dylib is imported by exact filename, so the name is present verbatim in the
/// binary's import table — a plain byte search finds it with no parsing.
fn bevy_dylib_import(bin: &Path, plat: &Platform) -> Option<String> {
    let data = std::fs::read(bin).ok()?;
    let needle = b"bevy_dylib-";
    let suffix = format!(".{}", plat.ext);
    let sb = suffix.as_bytes();
    let mut i = 0;
    while i + needle.len() < data.len() {
        if &data[i..i + needle.len()] == needle {
            // Include the "lib" prefix when present (Unix DT_NEEDED form).
            let start = if i >= 3 && &data[i - 3..i] == b"lib" { i - 3 } else { i };
            let mut j = i + needle.len();
            while j < data.len() && data[j].is_ascii_hexdigit() {
                j += 1;
            }
            // Require ≥1 hex digit and the matching extension immediately after.
            if j > i + needle.len() && j + sb.len() <= data.len() && &data[j..j + sb.len()] == sb {
                return Some(String::from_utf8_lossy(&data[start..j + sb.len()]).into_owned());
            }
        }
        i += 1;
    }
    None
}

/// Copy the dynamic Rust `std` shared lib (needed because `prefer-dynamic` links
/// std dynamically). It lives under `<sysroot>/lib/rustlib/<host-triple>/lib/`.
fn copy_rust_std(out: &Path, plat: &Platform) -> std::io::Result<()> {
    let (Some(sysroot), Some(triple)) = (sysroot(), host_triple()) else {
        eprintln!("[xtask] WARN: could not query rustc sysroot/triple; skipping std");
        return Ok(());
    };
    let dir = sysroot.join("lib").join("rustlib").join(triple).join("lib");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Ok(());
    };
    for entry in rd.flatten() {
        let name = file_name(&entry.path());
        let std_lib = (name.starts_with("std-") || name.starts_with("libstd-"))
            && name.ends_with(&format!(".{}", plat.ext));
        if std_lib {
            copy(&entry.path(), &out.join(&name))?;
        }
    }
    Ok(())
}

/// Launch the staged editor. cwd = repo root so the editor resolves project
/// assets the same way a plain `cargo run` does; plugins resolve via the loader's
/// `<exe-dir>/plugins/` scan, independent of cwd.
fn launch(repo: &Path, out: &Path, plat: &Platform) -> ExitCode {
    let bin = out.join(format!("renzora{}", plat.exe_suffix));
    println!("[xtask] launching {}", bin.display());
    let extra: Vec<String> = std::env::args().skip(2).collect();
    match Command::new(&bin).current_dir(repo).args(&extra).status() {
        Ok(s) => s.code().map(|c| ExitCode::from(c as u8)).unwrap_or(ExitCode::SUCCESS),
        Err(e) => {
            eprintln!("[xtask] failed to launch {}: {e}", bin.display());
            ExitCode::FAILURE
        }
    }
}

// ── small helpers ────────────────────────────────────────────────────────────

/// Honor cargo's chosen toolchain when xtask is itself invoked via cargo.
fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn copy(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::copy(src, dst).map(|_| ())
}

/// Remove only exe + shared-lib artifacts from a dir (keep everything else).
fn clean_artifacts(dir: &Path, plat: &Platform) -> std::io::Result<()> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in rd.flatten() {
        let name = file_name(&entry.path());
        if name.ends_with(&format!(".{}", plat.ext))
            || (!plat.exe_suffix.is_empty() && name.ends_with(plat.exe_suffix))
        {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    Ok(())
}

fn newest_matching(dir: &Path, prefix: &str, ext: &str) -> Option<PathBuf> {
    let suffix = format!(".{ext}");
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let n = file_name(p);
            n.starts_with(prefix) && n.ends_with(&suffix)
        })
        .max_by_key(|p| p.metadata().and_then(|m| m.modified()).ok())
}

fn sysroot() -> Option<PathBuf> {
    let out = Command::new("rustc").arg("--print").arg("sysroot").output().ok()?;
    out.status.success().then(|| PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()))
}

/// The host target triple, read from `rustc -vV`'s `host:` line — used to locate
/// the std shared lib under the sysroot.
fn host_triple() -> Option<String> {
    let out = Command::new("rustc").arg("-vV").output().ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("host: "))
        .map(|s| s.trim().to_string())
}

#[cfg(unix)]
fn make_executable(p: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(p)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(p, perms)
}

/// Rewrite native-build absolute install names to `@rpath/<name>` so the staged
/// `dist/` resolves at runtime: the exe carries an `@loader_path` rpath (from
/// `.cargo/config.toml`), and plugins get `@loader_path/..` so their deps
/// resolve in the exe dir one level up. Best-effort — warns if Xcode's
/// `install_name_tool`/`codesign` aren't present.
#[cfg(target_os = "macos")]
fn fixup_macos(out: &Path) {
    let mut files: Vec<PathBuf> = Vec::new();
    let push_dylibs = |dir: &Path, files: &mut Vec<PathBuf>| {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if file_name(&p).ends_with(".dylib") {
                    files.push(p);
                }
            }
        }
    };
    files.push(out.join("renzora"));
    push_dylibs(out, &mut files);
    let plugins = out.join("plugins");
    push_dylibs(&plugins, &mut files);

    for f in &files {
        if !f.exists() {
            continue;
        }
        let name = file_name(f);
        if name.ends_with(".dylib") {
            let _ = Command::new("install_name_tool")
                .args(["-id", &format!("@rpath/{name}")])
                .arg(f)
                .status();
        }
        // Rewrite any dependency recorded as an absolute build path under target/.
        if let Ok(o) = Command::new("otool").arg("-L").arg(f).output() {
            for line in String::from_utf8_lossy(&o.stdout).lines().skip(1) {
                let dep = line.trim().split_whitespace().next().unwrap_or("");
                if dep.contains("/target/") && dep.starts_with('/') {
                    let base = dep.rsplit('/').next().unwrap_or(dep);
                    let _ = Command::new("install_name_tool")
                        .args(["-change", dep, &format!("@rpath/{base}")])
                        .arg(f)
                        .status();
                }
            }
        }
        if f.starts_with(&plugins) {
            let _ = Command::new("install_name_tool")
                .args(["-add_rpath", "@loader_path/.."])
                .arg(f)
                .status();
        }
        // install_name_tool invalidates the ad-hoc signature; arm64 macOS refuses
        // invalid signatures, so re-sign each touched file ad-hoc.
        let _ = Command::new("codesign").args(["-s", "-", "-f"]).arg(f).status();
    }
}
