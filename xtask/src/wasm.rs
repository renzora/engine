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

/// `cargo renzora wasm [--no-opt]`.
///
/// `--no-opt` skips the `wasm-opt -Oz` pass. That pass is the slowest step in
/// the whole web build by a wide margin — minutes on a 150–200 MB module, longer
/// than everything else combined once the cargo cache is warm — and it is purely
/// a size optimisation. When the loop is "change something, reload the tab" the
/// bundle is being served off localhost, so ten times larger costs nothing and
/// the wait is the only thing you feel.
///
/// Skipping it also means binaryen is never fetched, which is the difference
/// between this command working offline and not.
pub fn build_and_stage(repo: &Path, args: &[String]) -> ExitCode {
    let skip_opt = args.iter().any(|a| a == "--no-opt" || a == "--skip-opt");
    if let Some(bad) = args
        .iter()
        .find(|a| !matches!(a.as_str(), "--no-opt" | "--skip-opt"))
    {
        // Fail rather than ignore: a mistyped `--no-optimize` that silently ran
        // the slow pass anyway would be maddening to diagnose.
        eprintln!("[xtask] unknown argument '{bad}' (expected: --no-opt)");
        return ExitCode::from(2);
    }

    if !target_installed() {
        eprintln!(
            "[xtask] the wasm32-unknown-unknown target is not installed\n\
             [xtask]   rustup target add wasm32-unknown-unknown"
        );
        return ExitCode::from(1);
    }
    // Checked up front rather than after two long builds: without it there is
    // no bundle at the end, only a raw .wasm no browser can load.
    if !ensure_wasm_bindgen(repo) {
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
        if skip_opt {
            println!("[xtask] --no-opt: skipping wasm-opt for {bundle}");
        } else {
            optimize(repo, &out, bundle);
        }
        if let Err(e) = write_shell(&out, bundle) {
            eprintln!("[xtask] could not write {bundle}.html: {e}");
            return ExitCode::from(1);
        }
    }

    println!("[xtask] staged {}", out.display());
    if !skip_opt {
        println!("[xtask] (iterating? `cargo renzora wasm --no-opt` skips the slow wasm-opt pass)");
    }
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
        // `renzora_editor_app` declares exactly one bin, but name it anyway:
        // explicit target selection is what keeps this honest if the package
        // ever grows a second one.
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
fn optimize(repo: &Path, out: &Path, bundle: &str) {
    let Some(exe) = ensure_wasm_opt(repo) else {
        println!("[xtask] {bundle} left unoptimized (hundreds of MB)");
        return;
    };
    let module = out.join(format!("{bundle}_bg.wasm"));
    println!("[xtask] wasm-opt -Oz {bundle} (slow)");
    let ok = Command::new(&exe)
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
  /* The canvas is sized by Bevy (`fit_canvas_to_parent`), from THIS element.
     Hence a fixed-size parent and no width/height on the canvas itself: sizing
     the canvas in CSS instead fights Bevy for control of the surface, and a
     parent whose size depends on its children feeds back into a canvas that
     grows on every resize. */
  #stage {{ position: fixed; inset: 0; }}
  canvas {{ display: block; outline: none; }}
  /* The boot overlay sits ABOVE the canvas and fades out on the first rendered
     frame, so the module's download and Bevy's own startup read as one wait
     instead of a blank page followed by a flash of unpainted canvas. */
  #boot {{ position: fixed; inset: 0; z-index: 10; display: grid;
          place-items: center; gap: 18px; align-content: center;
          background: #14151a; color: #e6e8eb;
          font: 400 14px/1.5 system-ui, -apple-system, sans-serif;
          transition: opacity .35s ease; }}
  #boot.done {{ opacity: 0; pointer-events: none; }}
  #boot h1 {{ margin: 0; font-size: 15px; font-weight: 500; letter-spacing: .3px; }}
  #bar {{ width: 260px; height: 3px; border-radius: 3px; background: #23262d;
         overflow: hidden; }}
  #bar > i {{ display: block; height: 100%; width: 0; border-radius: 3px;
             background: #5b8cff; transition: width .2s ease; }}
  #pct {{ font-variant-numeric: tabular-nums; color: #8a8f98; font-size: 12px; }}
</style>
</head>
<body>
<div id="boot">
  <h1>{title}</h1>
  <div id="bar"><i></i></div>
  <div id="pct">starting…</div>
</div>
<div id="stage"><canvas id="bevy"></canvas></div>
<script type="module">
  import init from './{bundle}.js';

  const boot = document.getElementById('boot');
  const fill = document.querySelector('#bar > i');
  const pct  = document.getElementById('pct');
  const say  = (t) => {{ pct.textContent = t; }};

  // Hide the overlay when Bevy has actually PAINTED, not when `init()` returns.
  // winit's wasm event loop escapes `main` by throwing, so the init promise
  // settles well before the first frame — removing the overlay there uncovers
  // an unpainted canvas. Bevy sets the canvas backing size on startup, so watch
  // for that and give it one more frame to draw into.
  const reveal = () => {{
    if (boot.classList.contains('done')) return;
    boot.classList.add('done');
    setTimeout(() => boot.remove(), 400);
  }};
  const canvas = document.getElementById('bevy');
  new MutationObserver((_m, obs) => {{
    if (canvas.width > 0 && canvas.height > 0) {{
      obs.disconnect();
      requestAnimationFrame(() => requestAnimationFrame(reveal));
    }}
  }}).observe(canvas, {{ attributes: true, attributeFilter: ['width', 'height'] }});

  // WebGPU only: the wasm build enables bevy's `webgpu` feature and has no
  // WebGL path to fall back to. Say so plainly rather than failing deep in
  // adapter selection.
  if (!navigator.gpu) {{
    say('This build needs WebGPU — try Chrome or Edge 113+.');
  }} else {{
    // Stream the module ourselves so the wait has a progress bar. This build is
    // hundreds of MB unoptimised and tens optimised; a bare "Loading…" for that
    // long is indistinguishable from a hang. Falls back to plain `init()` if the
    // server sends no content-length (or anything else goes wrong) — progress is
    // a nicety and must never be the reason the editor fails to start.
    (async () => {{
      try {{
        const res = await fetch('./{bundle}_bg.wasm');
        if (!res.ok) throw new Error(res.status + ' ' + res.statusText);
        const total = +res.headers.get('content-length') || 0;
        if (!total || !res.body) {{
          say('loading…');
          await init();
          return;
        }}
        const chunks = [];
        let got = 0;
        const reader = res.body.getReader();
        for (;;) {{
          const {{ done, value }} = await reader.read();
          if (done) break;
          chunks.push(value);
          got += value.length;
          const f = got / total;
          fill.style.width = (f * 100).toFixed(1) + '%';
          say((got / 1048576).toFixed(0) + ' / ' + (total / 1048576).toFixed(0) + ' MB');
        }}
        fill.style.width = '100%';
        say('compiling…');
        const bytes = new Uint8Array(got);
        let at = 0;
        for (const c of chunks) {{ bytes.set(c, at); at += c.length; }}
        await init({{ module_or_path: bytes }});
      }} catch (e) {{
        boot.classList.remove('done');
        say('Failed to start: ' + e);
        console.error(e);
      }}
    }})();
  }}
</script>
</body>
</html>
"#
    );
    std::fs::write(out.join(format!("{bundle}.html")), html)
}

/// Make sure a `wasm-bindgen` CLI of the RIGHT version is on PATH, installing
/// it if not.
///
/// The version is not hardcoded here — it is read out of the engine's
/// `Cargo.lock`, because the constraint is not "some wasm-bindgen" but "the
/// exact version of the `wasm-bindgen` *crate* the module was compiled
/// against". The CLI and the crate emit and expect the same ABI for the glue,
/// and a mismatch fails at load in the browser with a message that points at
/// neither: `wasm-bindgen` on the CLI side just refuses, or worse, produces JS
/// whose imports the module does not have.
///
/// Deriving it from the lockfile means a `cargo update` that moves wasm-bindgen
/// silently fixes this tool too, instead of leaving a stale pin to discover the
/// hard way. (`docker/wasm/Dockerfile` still pins its copy by hand — that one
/// is a container layer and cannot read the lockfile at image-build time.)
///
/// Installing rather than instructing is the point of putting this in xtask:
/// `cargo renzora wasm` should work on a fresh checkout the way every other
/// `cargo renzora` command does, without a README step.
fn ensure_wasm_bindgen(repo: &Path) -> bool {
    let Some(want) = locked_wasm_bindgen_version(repo) else {
        eprintln!(
            "[xtask] could not find the wasm-bindgen version in Cargo.lock\n\
             [xtask] (is the lockfile present? the web build needs it to pin the CLI)"
        );
        return false;
    };

    if installed_bindgen_version().as_deref() == Some(want.as_str()) {
        return true;
    }

    match installed_bindgen_version() {
        Some(have) => println!(
            "[xtask] wasm-bindgen {have} is installed but the lockfile wants {want} — reinstalling"
        ),
        None => println!("[xtask] wasm-bindgen not found — installing {want}"),
    }
    println!("[xtask] cargo install wasm-bindgen-cli --version {want} --locked (a few minutes, once)");

    let ok = Command::new(cargo())
        .args(["install", "wasm-bindgen-cli", "--version", &want, "--locked"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        eprintln!(
            "[xtask] installing wasm-bindgen-cli failed\n\
             [xtask]   cargo install wasm-bindgen-cli --version {want} --locked"
        );
        return false;
    }

    // Verify rather than trust: `cargo install` can succeed into a directory
    // that is not on PATH, which would otherwise surface later as a confusing
    // "not found" after an apparently successful install.
    match installed_bindgen_version() {
        Some(have) if have == want => true,
        Some(have) => {
            eprintln!("[xtask] installed {want} but PATH still resolves wasm-bindgen {have}");
            false
        }
        None => {
            eprintln!(
                "[xtask] wasm-bindgen still not on PATH after install \
                 — is ~/.cargo/bin on your PATH?"
            );
            false
        }
    }
}

/// Binaryen release to fetch when `wasm-opt` isn't already around.
///
/// Pinned so a local `-Oz` and CI's produce the same bundle. CI gets its copy
/// from the wasm image's apt `binaryen` (see docker/wasm/Dockerfile), which is
/// whatever Debian ships — the two can drift, and that shows up as a size
/// difference, never a correctness one.
const BINARYEN_VERSION: &str = "121";

/// Find `wasm-opt`, downloading binaryen if it isn't installed.
///
/// Unlike wasm-bindgen this is NOT installable through cargo: the `wasm-opt`
/// crate on crates.io is bindings only and has no `[[bin]]`, so
/// `cargo install wasm-opt` fails with "there is nothing to install". Binaryen
/// publishes prebuilt archives per platform instead, so that is what this
/// fetches — with `curl` and `tar`, both of which ship with Windows 10+ as well
/// as every Unix, keeping xtask's zero-dependency rule intact.
///
/// Everything here is best-effort and returns `None` on any failure: `-Oz` is a
/// size pass, not a correctness one, and a local iteration loop that cannot
/// reach GitHub should still get a working (fat) bundle rather than an error.
fn ensure_wasm_opt(repo: &Path) -> Option<PathBuf> {
    if let Some(found) = which("wasm-opt") {
        return Some(found);
    }
    // Cached under target/ so it is disposable and never committed.
    let dir = repo.join("target").join("tools");
    let exe_name = if cfg!(windows) { "wasm-opt.exe" } else { "wasm-opt" };
    let cached = dir
        .join(format!("binaryen-version_{BINARYEN_VERSION}"))
        .join("bin")
        .join(exe_name);
    if cached.is_file() {
        return Some(cached);
    }

    let Some(slug) = binaryen_platform_slug() else {
        println!("[xtask] no binaryen build published for this platform — skipping wasm-opt");
        return None;
    };
    let archive_name = format!("binaryen-version_{BINARYEN_VERSION}-{slug}.tar.gz");
    let url = format!(
        "https://github.com/WebAssembly/binaryen/releases/download/\
         version_{BINARYEN_VERSION}/{archive_name}"
    );

    if std::fs::create_dir_all(&dir).is_err() {
        return None;
    }
    let archive = dir.join(&archive_name);

    println!("[xtask] wasm-opt not found — fetching binaryen {BINARYEN_VERSION} (once)");
    println!("[xtask]   {url}");
    let fetched = Command::new("curl")
        // -f so an HTML 404 page is an error rather than a "successful"
        // download of something tar will then choke on.
        .args(["-fsSL", "-o"])
        .arg(&archive)
        .arg(&url)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !fetched {
        println!("[xtask] could not download binaryen — continuing without wasm-opt");
        let _ = std::fs::remove_file(&archive);
        return None;
    }

    let extracted = Command::new("tar")
        .arg("-xzf")
        .arg(&archive)
        .arg("-C")
        .arg(&dir)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    let _ = std::fs::remove_file(&archive);
    if !extracted {
        println!("[xtask] could not extract binaryen — continuing without wasm-opt");
        return None;
    }

    if cached.is_file() {
        Some(cached)
    } else {
        println!(
            "[xtask] binaryen extracted but {} is missing — continuing without wasm-opt",
            cached.display()
        );
        None
    }
}

/// Binaryen's release-asset platform suffix, or `None` where it publishes none.
fn binaryen_platform_slug() -> Option<&'static str> {
    let arm = cfg!(target_arch = "aarch64");
    if cfg!(target_os = "windows") {
        // No arm64 Windows asset is published; that host falls back to the
        // x86_64 build under emulation, which works for a build-time tool.
        Some("x86_64-windows")
    } else if cfg!(target_os = "macos") {
        Some(if arm { "arm64-macos" } else { "x86_64-macos" })
    } else if cfg!(target_os = "linux") {
        Some(if arm { "aarch64-linux" } else { "x86_64-linux" })
    } else {
        None
    }
}

/// The `wasm-bindgen` crate version the engine links, from `Cargo.lock`.
///
/// Scanned line-wise rather than parsed: xtask has zero dependencies by design
/// (see its Cargo.toml — it must stay instant to compile, since every
/// `cargo renzora` builds it), so there is no TOML parser here. The lockfile's
/// `[[package]]` blocks put `version` immediately after `name`, which makes the
/// scan reliable enough for one well-known key.
fn locked_wasm_bindgen_version(repo: &Path) -> Option<String> {
    let lock = std::fs::read_to_string(repo.join("Cargo.lock")).ok()?;
    let mut lines = lock.lines();
    while let Some(line) = lines.next() {
        if line.trim() != r#"name = "wasm-bindgen""# {
            continue;
        }
        let version = lines.next()?.trim();
        let rest = version.strip_prefix("version = \"")?;
        return Some(rest.strip_suffix('"')?.to_string());
    }
    None
}

/// `wasm-bindgen --version` → `"0.2.108"`, or `None` if it is not runnable.
fn installed_bindgen_version() -> Option<String> {
    let out = Command::new("wasm-bindgen").arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    // Prints `wasm-bindgen 0.2.108`.
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .nth(1)
        .map(|v| v.to_string())
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
