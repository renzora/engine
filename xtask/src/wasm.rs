//! `cargo renzora wasm` — build and stage the two web bundles locally.
//!
//! The native mirror of `docker/build-all.sh`'s `build_wasm` lane, in the same
//! way [`crate::stage`] mirrors its `copy_shared_libs`: same output layout
//! (`dist/web-wasm32/`), same bundle names, same HTML shell. The container
//! remains the path for a release artefact; this exists so the web build can be
//! iterated on — and actually *opened in a browser* — without a 60-minute CI
//! round-trip for every change.
//!
//! **Keep this and `build_wasm` in step.** They are deliberately duplicated
//! rather than shared (one is bash in a pinned image, the other is Rust on the
//! host), so a change to the bundle layout or the shell has to be made twice.
//! That is the existing bargain in this file's neighbour, not a new one.
//!
//! Unlike every other `cargo renzora` command this one cross-compiles: the host
//! is not wasm. That is fine here because nothing links a host artefact — the
//! whole point of the target is that it runs in a browser.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use crate::cargo;

/// The two bundles, each `(cargo args, built .wasm stem, bundle name)`.
///
/// Separate target dirs, deliberately: `renzora_app --features wasm` resolves
/// `renzora` WITHOUT its `editor` feature and `renzora_editor_app` resolves it
/// WITH. Sharing one dir makes each build evict the ~86 shared packages the
/// other just compiled — the same trap CLAUDE.md §2 warns about for `-p` builds.
const BUNDLES: &[(&str, &str, &str)] = &[
    ("renzora_app", "renzora", "renzora-runtime"),
    ("renzora_editor_app", "renzora-editor", "renzora-editor"),
];

pub fn build_and_stage(repo: &Path) -> ExitCode {
    if !target_installed() {
        eprintln!(
            "[xtask] the wasm32-unknown-unknown target is not installed\n\
             [xtask]   rustup target add wasm32-unknown-unknown"
        );
        return ExitCode::from(1);
    }
    // Checked up front rather than after two long builds: without it there is
    // no bundle at the end, only a raw .wasm no browser can load.
    if which("wasm-bindgen").is_none() {
        eprintln!(
            "[xtask] wasm-bindgen not found on PATH\n\
             [xtask]   cargo install wasm-bindgen-cli --version 0.2.108 --locked\n\
             [xtask] (0.2.108 matches docker/wasm/Dockerfile — a mismatched CLI \
             emits glue the module rejects at load)"
        );
        return ExitCode::from(1);
    }

    let out = repo.join("dist").join("web-wasm32");
    if let Err(e) = std::fs::create_dir_all(&out) {
        eprintln!("[xtask] could not create {}: {e}", out.display());
        return ExitCode::from(1);
    }

    for (package, stem, bundle) in BUNDLES {
        let target_dir = format!("target/wasm{}", if *package == "renzora_app" { "" } else { "-editor" });
        if !build_one(repo, package, &target_dir) {
            eprintln!("[xtask] cargo build failed for {package}");
            return ExitCode::from(1);
        }
        let built = repo
            .join(&target_dir)
            .join("wasm32-unknown-unknown/dist")
            .join(format!("{stem}.wasm"));
        if !built.exists() {
            eprintln!("[xtask] expected {} — not produced", built.display());
            return ExitCode::from(1);
        }
        if !bindgen(repo, &built, &out, bundle) {
            return ExitCode::from(1);
        }
        optimize(&out, bundle);
        if let Err(e) = write_shell(&out, bundle) {
            eprintln!("[xtask] could not write {bundle}.html: {e}");
            return ExitCode::from(1);
        }
    }

    println!("[xtask] staged {}", out.display());
    println!(
        "[xtask] serve it over HTTP — a module script will not load from file://\n\
         [xtask]   python -m http.server -d {}",
        out.display()
    );
    ExitCode::SUCCESS
}

fn build_one(repo: &Path, package: &str, target_dir: &str) -> bool {
    let mut args = vec![
        "build".into(),
        "--profile".into(),
        "dist".into(),
        "-p".into(),
        package.to_string(),
    ];
    if package == "renzora_app" {
        // The runtime's web configuration: no default features (which would drag
        // in `dynamic_linking` and `xr`, neither of which exists on wasm).
        args.push("--no-default-features".into());
        args.push("--features".into());
        args.push("wasm".into());
    } else {
        // The editor package also carries a `renzora` bin target; name the one
        // we want so cargo doesn't build both.
        args.push("--bin".into());
        args.push("renzora-editor".into());
    }
    args.push("--target".into());
    args.push("wasm32-unknown-unknown".into());
    args.push("--target-dir".into());
    args.push(target_dir.into());

    println!("[xtask] cargo {}", args.join(" "));
    Command::new(cargo())
        .current_dir(repo)
        .args(&args)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn bindgen(repo: &Path, built: &Path, out: &Path, bundle: &str) -> bool {
    println!("[xtask] wasm-bindgen {bundle}");
    let ok = Command::new("wasm-bindgen")
        .current_dir(repo)
        .args([
            "--out-dir".as_ref(),
            out.as_os_str(),
            "--out-name".as_ref(),
            bundle.as_ref(),
            "--target".as_ref(),
            "web".as_ref(),
            built.as_os_str(),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        eprintln!("[xtask] wasm-bindgen failed for {bundle}");
    }
    ok
}

/// `wasm-opt -Oz`, when binaryen is around.
///
/// Optional on purpose: it is a size pass, not a correctness one, and it is the
/// single slowest step in the web build (minutes, on a 150–200 MB module). A
/// local iteration loop wants the bundle now and does not care that it is ten
/// times larger; CI runs the same pass and does care.
fn optimize(out: &Path, bundle: &str) {
    if which("wasm-opt").is_none() {
        println!("[xtask] wasm-opt not found — {bundle} left unoptimized (hundreds of MB)");
        return;
    }
    let module = out.join(format!("{bundle}_bg.wasm"));
    println!("[xtask] wasm-opt -Oz {bundle} (slow)");
    let ok = Command::new("wasm-opt")
        .args([
            "-Oz",
            "--enable-bulk-memory",
            "--enable-sign-ext",
            "--enable-nontrapping-float-to-int",
            "--enable-mutable-globals",
            "--enable-reference-types",
            "--enable-multivalue",
        ])
        .arg(&module)
        .arg("-o")
        .arg(&module)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        eprintln!("[xtask] WARN: wasm-opt failed for {bundle} — bundle left unoptimized");
    }
}

/// The host page. wasm-bindgen emits glue and a module but nothing to load them
/// from; without this the staged directory has no entry point.
fn write_shell(out: &Path, bundle: &str) -> std::io::Result<()> {
    let title = if bundle == "renzora-editor" { "Renzora Editor" } else { "Renzora" };
    // `id="bevy"` is what bevy_winit looks for on wasm. Without a canvas it
    // creates its own, which the stylesheet below never reaches — the symptom
    // is a viewport that ignores the window size.
    let html = format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
  html, body {{ margin: 0; height: 100%; background: #14151a; overflow: hidden; }}
  canvas {{ display: block; width: 100vw; height: 100vh; outline: none; }}
  #boot {{ position: fixed; inset: 0; display: grid; place-items: center;
          font: 14px system-ui, sans-serif; color: #8a8f98; }}
</style>
</head>
<body>
<div id="boot">Loading {title}…</div>
<canvas id="bevy"></canvas>
<script type="module">
  import init from './{bundle}.js';
  // WebGPU only: the wasm build enables bevy's `webgpu` feature and has no
  // WebGL path to fall back to. Say so plainly rather than failing deep in
  // adapter selection.
  if (!navigator.gpu) {{
    document.getElementById('boot').textContent =
      'This build needs WebGPU. Try Chrome or Edge 113+.';
  }} else {{
    init()
      .then(() => document.getElementById('boot').remove())
      .catch(e => {{
        document.getElementById('boot').textContent = 'Failed to start: ' + e;
        console.error(e);
      }});
  }}
</script>
</body>
</html>
"#
    );
    std::fs::write(out.join(format!("{bundle}.html")), html)
}

fn target_installed() -> bool {
    Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("wasm32-unknown-unknown"))
        // No rustup (a distro-packaged toolchain, say) — assume it is there and
        // let cargo produce the real error rather than blocking on a guess.
        .unwrap_or(true)
}

/// `which`, without the dependency. Only used for optional tooling, so a probe
/// that cannot run is treated as "absent" rather than fatal.
fn which(exe: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT".into())
            .split(';')
            .map(|e| e.to_lowercase())
            .collect()
    } else {
        vec![String::new()]
    };
    for dir in std::env::split_paths(&path) {
        for ext in &exts {
            let candidate = dir.join(format!("{exe}{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}
