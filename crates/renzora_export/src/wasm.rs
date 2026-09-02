//! Turning a lean `renzora.wasm` into something a browser can open.
//!
//! `cargo build --target wasm32-unknown-unknown` produces a raw module with no
//! JS glue, and a browser cannot load one: every `#[wasm_bindgen]` export
//! (`set_rpak`, `start`) is reachable only through the bindings the
//! `wasm-bindgen` CLI generates from the module's custom sections. So the web
//! lane has a post-build chain the desktop ones do not — bindgen, then an
//! optional `wasm-opt -Oz`, then a host page — and this module is that chain.
//!
//! ## Why this duplicates `xtask/src/wasm.rs`
//!
//! It does, and the duplication is deliberate in the same way `xtask`'s own is:
//! `xtask` is a build tool outside the workspace with a hard zero-dependency
//! rule, and this runs inside the editor with the export modal's progress
//! channel and cancellation. Sharing would mean a crate that both a workspace
//! member and a non-member depend on, which is exactly the arrangement
//! `xtask/Cargo.toml` explains it cannot have.
//!
//! What is NOT duplicated is the part that would rot dangerously: the
//! wasm-bindgen version comes from the engine's `Cargo.lock` in both, so the CLI
//! always matches the crate the module was compiled against. That pin is the one
//! that fails in the browser rather than at build time, and neither copy can
//! drift from it.
//!
//! ## Everything optional is best-effort
//!
//! `wasm-opt` is a *size* pass. A missing binaryen makes the bundle several
//! times larger and changes nothing else, so it is a logged skip — the same
//! bargain `upx.rs` already strikes. `wasm-bindgen` is not optional: without it
//! there is no bundle at all, only a module nothing can call.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::toolchain::Toolchain;

/// The bundle's name, which fixes every filename in the output.
///
/// Deliberately the same name the prebuilt template uses, so a lean web export
/// and a template one produce byte-identical *layouts* — `renzora-runtime.js`,
/// `renzora-runtime_bg.wasm`, `index.html`, `game.rpak`. Anything that learned
/// the old layout (a hosting setup, a `<script>` tag someone wrote by hand)
/// keeps working when a project switches packaging mode.
pub const BUNDLE: &str = "renzora-runtime";

/// Everything `wasm-bindgen --target web` emitted that the page actually needs,
/// as `(path, name-inside-the-zip)` pairs.
///
/// A walk rather than the obvious two-name list, and the reason is `snippets/`.
/// Any crate using `#[wasm_bindgen(inline_js = ...)]` makes wasm-bindgen write
/// its JS into `snippets/<crate>-<hash>/inline0.js`, which the generated glue
/// then `import`s by relative path — `renzora_webfs` does exactly this, and it
/// is in the runtime's graph, not just the editor's. Listing `<bundle>.js` and
/// `<bundle>_bg.wasm` by hand would drop that directory and the bundle would
/// fail at load with a 404 on a file nobody had heard of.
///
/// `.d.ts` is the one thing skipped: TypeScript declarations are for a build
/// step a shipped game does not have.
pub fn bundle_files(out_dir: &Path) -> std::io::Result<Vec<(PathBuf, String)>> {
    let mut out = Vec::new();
    collect(out_dir, out_dir, &mut out)?;
    // Stable order so two exports of the same game produce the same zip.
    out.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(out)
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<(PathBuf, String)>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(root, &path, out)?;
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) == Some("ts") {
            continue;
        }
        // Zip entry names are always `/`-separated, whatever the host uses.
        let rel = path
            .strip_prefix(root)
            .map_err(std::io::Error::other)?
            .to_string_lossy()
            .replace('\\', "/");
        out.push((path, rel));
    }
    Ok(())
}

/// Run `wasm-bindgen` over the built module, writing the glue + `_bg.wasm` into
/// `out_dir`.
pub fn bindgen(
    toolchain: &Toolchain,
    engine_src: &Path,
    module: &Path,
    out_dir: &Path,
    progress: &mut dyn FnMut(String),
) -> Result<(), String> {
    let mut exe = ensure_bindgen(toolchain, engine_src, progress)?;
    progress("Generating JavaScript bindings…".into());
    let status = exe
        .args([
            "--out-dir".as_ref(),
            out_dir.as_os_str(),
            "--out-name".as_ref(),
            BUNDLE.as_ref(),
            "--target".as_ref(),
            "web".as_ref(),
            module.as_os_str(),
        ])
        .status()
        .map_err(|e| format!("Failed to run wasm-bindgen: {e}"))?;
    if !status.success() {
        return Err("wasm-bindgen failed to generate the web bindings".into());
    }
    Ok(())
}

/// `wasm-opt -Oz` over the emitted module, fetching binaryen if it isn't around.
///
/// Never fails the export. The feature flags mirror `xtask`'s list exactly —
/// they are not optional extras but the wasm proposals rustc's output already
/// uses, and omitting one makes `wasm-opt` reject a module it should accept.
pub fn optimize(engine_src: &Path, out_dir: &Path, progress: &mut dyn FnMut(String)) {
    let Some(exe) = ensure_wasm_opt(engine_src, progress) else {
        progress(
            "Shipping the bundle unoptimized — it will be several times larger, but \
             it works. Put `wasm-opt` on PATH and export again to shrink it."
                .into(),
        );
        return;
    };
    let module = out_dir.join(format!("{BUNDLE}_bg.wasm"));
    let before = std::fs::metadata(&module).map(|m| m.len()).unwrap_or(0);
    progress("Optimizing the wasm module (wasm-opt -Oz, slow)…".into());
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
        progress("wasm-opt failed — shipping the bundle unoptimized".into());
        return;
    }
    let after = std::fs::metadata(&module).map(|m| m.len()).unwrap_or(0);
    if before > 0 && after > 0 {
        progress(format!(
            "wasm-opt: {:.1} MB → {:.1} MB",
            before as f64 / 1_048_576.0,
            after as f64 / 1_048_576.0,
        ));
    }
}

/// The host page for an exported game, used by BOTH web packaging modes.
///
/// ## The canvas is not optional
///
/// `renzora_runtime` configures the window with `canvas: Some("#bevy")`, and
/// bevy_winit 0.19 does not fall back when that selector matches nothing — it
/// **panics**, which on wasm reaches the page as a bare
/// `RuntimeError: unreachable` with the real message only in the console:
///
/// ```text
/// panicked at bevy_winit/src/winit_windows.rs:294:21:
/// Cannot find element: #bevy.
/// ```
///
/// So `<canvas id="bevy">` has to be in the markup before `start()`. It sits
/// inside a fixed-inset `#stage` and carries **no width/height of its own**,
/// because the window also sets `fit_canvas_to_parent: true`: Bevy sizes the
/// canvas from its parent, and CSS sizing the canvas instead fights it for the
/// surface — a parent sized by its child then feeds back and the canvas grows on
/// every resize. This is the same structure `xtask`'s dev shell uses, for the
/// same reasons.
///
/// ## The rpak goes in before `start`
///
/// `renzora_engine::vfs` reads the injected bytes out of a `OnceLock` while the
/// app is being built, so a rpak that arrives after `start()` is never seen and
/// the game boots against an empty filesystem.
///
/// ## Why the loader streams the module itself
///
/// An exported game is tens of MB optimised and over a hundred without
/// `wasm-opt`. A motionless "Loading…" for that long is indistinguishable from a
/// hang, so the module is fetched with a progress bar and handed to `init`.
/// Every part of that is best-effort: no `content-length`, no `body`, or any
/// other surprise falls back to a plain `init()`, because progress is a nicety
/// and must never be why a game fails to start.
///
/// The prebuilt-template path writes the same page — the two modes differ only
/// in where the module came from, and a page that drifted between them would be
/// a bug reproducible on one packaging mode and not the other.
pub fn index_html(title: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
  html, body {{ margin: 0; height: 100%; background: #050410; overflow: hidden; }}
  /* Fixed-size parent; Bevy sizes the canvas from it via `fit_canvas_to_parent`. */
  #stage {{ position: fixed; inset: 0; }}
  canvas {{ display: block; outline: none; }}
  /* Above the canvas, faded out on the first painted frame, so the download and
     Bevy's own startup read as one wait rather than a blank page followed by a
     flash of unpainted canvas. */
  #boot {{ position: fixed; inset: 0; z-index: 10; display: grid;
          place-items: center; gap: 18px; align-content: center;
          background: #050410; color: #e6e8eb;
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
  import init, {{ set_rpak, start }} from './{bundle}.js';

  const boot = document.getElementById('boot');
  const fill = document.querySelector('#bar > i');
  const pct  = document.getElementById('pct');
  const say  = (t) => {{ pct.textContent = t; }};

  // Reveal when Bevy has actually PAINTED, not when `start()` returns: winit's
  // wasm event loop escapes by throwing, so control comes back well before the
  // first frame and removing the overlay there uncovers an unpainted canvas.
  // Bevy sets the canvas backing size on startup, so watch for that.
  const canvas = document.getElementById('bevy');
  const reveal = () => {{
    if (boot.classList.contains('done')) return;
    boot.classList.add('done');
    setTimeout(() => boot.remove(), 400);
  }};
  new MutationObserver((_m, obs) => {{
    if (canvas.width > 0 && canvas.height > 0) {{
      obs.disconnect();
      requestAnimationFrame(() => requestAnimationFrame(reveal));
    }}
  }}).observe(canvas, {{ attributes: true, attributeFilter: ['width', 'height'] }});

  // WebGPU only: this build enables bevy's `webgpu` feature and has no WebGL
  // path to fall back to. Say so plainly rather than failing deep in adapter
  // selection, where the message means nothing to a player.
  if (!navigator.gpu) {{
    say('This game needs WebGPU — try Chrome or Edge 113+.');
  }} else {{
    (async () => {{
      try {{
        const rpakResp = await fetch('./game.rpak');
        if (!rpakResp.ok) throw new Error('game.rpak: ' + rpakResp.status + ' ' + rpakResp.statusText);
        const rpakBytes = new Uint8Array(await rpakResp.arrayBuffer());

        const res = await fetch('./{bundle}_bg.wasm');
        if (!res.ok) throw new Error(res.status + ' ' + res.statusText);
        const total = +res.headers.get('content-length') || 0;
        if (!total || !res.body) {{
          say('loading…');
          await init();
        }} else {{
          const chunks = [];
          let got = 0;
          const reader = res.body.getReader();
          for (;;) {{
            const {{ done, value }} = await reader.read();
            if (done) break;
            chunks.push(value);
            got += value.length;
            fill.style.width = ((got / total) * 100).toFixed(1) + '%';
            say((got / 1048576).toFixed(0) + ' / ' + (total / 1048576).toFixed(0) + ' MB');
          }}
          fill.style.width = '100%';
          say('compiling…');
          const bytes = new Uint8Array(got);
          let at = 0;
          for (const c of chunks) {{ bytes.set(c, at); at += c.length; }}
          await init({{ module_or_path: bytes }});
        }}

        // Before `start`, always — see this function's doc.
        set_rpak(rpakBytes);
        say('starting…');
        start();
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
"#,
        title = title,
        bundle = BUNDLE,
    )
}

/// Resolve a `wasm-bindgen` CLI whose version matches the `wasm-bindgen` crate
/// in the engine's `Cargo.lock`, installing it if need be.
///
/// The version is read from the lockfile rather than pinned here because the
/// constraint is not "some wasm-bindgen" but "the exact version of the crate
/// this module was compiled against". The CLI and the crate agree on the shape
/// of the generated glue, and a mismatch is not caught at build time — it
/// surfaces in the browser as imports the module does not have, or as a flat
/// refusal from the CLI, neither of which points at the version.
fn ensure_bindgen(
    toolchain: &Toolchain,
    engine_src: &Path,
    progress: &mut dyn FnMut(String),
) -> Result<Command, String> {
    let want = locked_bindgen_version(engine_src).ok_or(
        "Could not read the wasm-bindgen version from the engine's Cargo.lock — \
         the web build needs it to pin the matching CLI.",
    )?;

    if let Some(have) = installed_bindgen_version(toolchain) {
        if have == want {
            return toolchain
                .tool_command("wasm-bindgen")
                .ok_or_else(|| "wasm-bindgen vanished between probe and use".to_string());
        }
        progress(format!(
            "wasm-bindgen {have} is installed but this engine needs {want} — reinstalling"
        ));
    } else {
        progress(format!("Installing wasm-bindgen {want} (one-time, a few minutes)…"));
    }

    let status = toolchain
        .cargo_command()
        .args(["install", "wasm-bindgen-cli", "--version", &want, "--locked"])
        .status()
        .map_err(|e| format!("Failed to run cargo install: {e}"))?;
    if !status.success() {
        return Err(format!(
            "Installing wasm-bindgen-cli {want} failed. Install it by hand and export again:\n  \
             cargo install wasm-bindgen-cli --version {want} --locked"
        ));
    }

    // Verify rather than trust: `cargo install` can succeed into a bin dir that
    // is not on `PATH`, which would otherwise surface as a confusing "not found"
    // straight after an apparently successful install.
    match installed_bindgen_version(toolchain) {
        Some(have) if have == want => toolchain
            .tool_command("wasm-bindgen")
            .ok_or_else(|| "wasm-bindgen vanished between probe and use".to_string()),
        Some(have) => Err(format!(
            "Installed wasm-bindgen {want}, but {have} is what resolves — a second copy \
             earlier on PATH is shadowing it."
        )),
        None => {
            let hint = toolchain
                .install_bin_dir()
                .map(|d| format!(" (expected it in {})", d.display()))
                .unwrap_or_default();
            Err(format!(
                "wasm-bindgen is still not runnable after installing it{hint} — \
                 is cargo's bin directory on PATH?"
            ))
        }
    }
}

/// The `wasm-bindgen` version the engine's lockfile pins.
fn locked_bindgen_version(engine_src: &Path) -> Option<String> {
    let lock = std::fs::read_to_string(engine_src.join("Cargo.lock")).ok()?;
    let mut lines = lock.lines();
    while let Some(line) = lines.next() {
        if line.trim() != r#"name = "wasm-bindgen""# {
            continue;
        }
        let version = lines.next()?.trim();
        return Some(version.strip_prefix("version = \"")?.strip_suffix('"')?.to_string());
    }
    None
}

/// `wasm-bindgen --version` → `"0.2.108"`, or `None` when it is not runnable.
fn installed_bindgen_version(toolchain: &Toolchain) -> Option<String> {
    let mut cmd = toolchain.tool_command("wasm-bindgen")?;
    let out = cmd.arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    // Prints `wasm-bindgen 0.2.108`.
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .nth(1)
        .map(str::to_string)
}

/// Binaryen release to fetch when `wasm-opt` isn't already around.
///
/// Pinned so a local `-Oz` and CI's produce the same bundle, and kept in step
/// with `xtask`'s copy — the two share the cache directory, so a bump here that
/// xtask does not follow simply means each fetches its own.
const BINARYEN_VERSION: &str = "121";

/// The `wasm-opt` this export will use, downloading binaryen if there is none.
///
/// # Why the editor is allowed to fetch this
///
/// It was not, originally, on the reasoning that a network fetch mid-export is a
/// surprise. That does not survive contact with what this lane already does: a
/// lean web export **bootstraps rustup** when there is no Rust, `rustup target
/// add`s wasm32, and `cargo install`s a version-matched `wasm-bindgen` — three
/// downloads, in this same flow, for the same reason. Binaryen is the fourth and
/// the smallest, and the export dialog already promises the others are
/// "installed automatically if missing".
///
/// The difference that matters is not whether it downloads but what a failure
/// costs. `-Oz` is a **size** pass: every failure here returns `None` and the
/// export ships a fat but perfectly working bundle. That is why nothing in this
/// function is an error — no binaryen for the platform, no network, a bad
/// archive, all of them are a logged skip.
///
/// `curl` and `tar` rather than a Rust HTTP client, mirroring `xtask`: both ship
/// with Windows 10+ and every Unix, and binaryen publishes plain release
/// tarballs.
fn ensure_wasm_opt(engine_src: &Path, progress: &mut dyn FnMut(String)) -> Option<PathBuf> {
    let exe_name = if cfg!(windows) { "wasm-opt.exe" } else { "wasm-opt" };

    // 1. Already on PATH — a developer's own binaryen wins over anything we fetch.
    if let Some(paths) = std::env::var_os("PATH") {
        if let Some(found) = std::env::split_paths(&paths)
            .map(|d| d.join(exe_name))
            .find(|p| p.is_file())
        {
            return Some(found);
        }
    }

    // 2. A cached copy — ours, or one `cargo renzora wasm` fetched earlier. Any
    //    version optimizes correctly, so take whatever is sitting there rather
    //    than insisting on the pinned one and re-downloading beside it.
    let tools = engine_src.join("target").join("tools");
    if let Ok(entries) = std::fs::read_dir(&tools) {
        if let Some(found) = entries
            .filter_map(Result::ok)
            .map(|e| e.path().join("bin").join(exe_name))
            .find(|p| p.is_file())
        {
            return Some(found);
        }
    }

    // 3. Fetch it, once, into the cache xtask also uses.
    let slug = binaryen_platform_slug()?;
    let archive_name = format!("binaryen-version_{BINARYEN_VERSION}-{slug}.tar.gz");
    let url = format!(
        "https://github.com/WebAssembly/binaryen/releases/download/\
         version_{BINARYEN_VERSION}/{archive_name}"
    );
    std::fs::create_dir_all(&tools).ok()?;
    let archive = tools.join(&archive_name);

    progress(format!(
        "Fetching binaryen {BINARYEN_VERSION} for wasm-opt (one-time, ~10 MB)…"
    ));
    // `-f` so a 404's HTML body is an error rather than a "successful" download
    // of something tar then chokes on.
    let fetched = Command::new("curl")
        .args(["-fsSL", "-o"])
        .arg(&archive)
        .arg(&url)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !fetched {
        let _ = std::fs::remove_file(&archive);
        progress("Could not download binaryen — continuing without wasm-opt".into());
        return None;
    }

    let extracted = Command::new("tar")
        .arg("-xzf")
        .arg(&archive)
        .arg("-C")
        .arg(&tools)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    let _ = std::fs::remove_file(&archive);
    if !extracted {
        progress("Could not extract binaryen — continuing without wasm-opt".into());
        return None;
    }

    let cached = tools
        .join(format!("binaryen-version_{BINARYEN_VERSION}"))
        .join("bin")
        .join(exe_name);
    if cached.is_file() {
        Some(cached)
    } else {
        progress(format!(
            "binaryen unpacked but {} is missing — continuing without wasm-opt",
            cached.display()
        ));
        None
    }
}

/// Binaryen's release-asset platform suffix, or `None` where it publishes none.
fn binaryen_platform_slug() -> Option<&'static str> {
    let arm = cfg!(target_arch = "aarch64");
    if cfg!(target_os = "windows") {
        // No arm64 Windows asset is published; that host runs the x86_64 build
        // under emulation, which is fine for a build-time tool.
        Some("x86_64-windows")
    } else if cfg!(target_os = "macos") {
        Some(if arm { "arm64-macos" } else { "x86_64-macos" })
    } else if cfg!(target_os = "linux") {
        Some(if arm { "aarch64-linux" } else { "x86_64-linux" })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_pinned_bindgen_version() {
        let dir = std::env::temp_dir().join("renzora-wasm-lockfile-test");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("Cargo.lock"),
            "[[package]]\nname = \"something\"\nversion = \"1.0.0\"\n\n\
             [[package]]\nname = \"wasm-bindgen\"\nversion = \"0.2.108\"\n",
        )
        .unwrap();
        assert_eq!(locked_bindgen_version(&dir).as_deref(), Some("0.2.108"));
    }

    #[test]
    fn no_lockfile_is_not_a_panic() {
        assert_eq!(
            locked_bindgen_version(Path::new("/definitely/not/a/checkout")),
            None
        );
    }

    /// The `snippets/` case, which a hand-written two-name list silently drops —
    /// and which is not hypothetical: `renzora_webfs` uses `inline_js` and is in
    /// the runtime's wasm graph.
    #[test]
    fn the_bundle_walk_keeps_snippets_and_drops_declarations() {
        let dir = std::env::temp_dir().join("renzora-wasm-bundle-walk-test");
        let _ = std::fs::remove_dir_all(&dir);
        let snippets = dir.join("snippets").join("renzora_webfs-abc123");
        std::fs::create_dir_all(&snippets).unwrap();
        std::fs::write(dir.join(format!("{BUNDLE}.js")), "glue").unwrap();
        std::fs::write(dir.join(format!("{BUNDLE}_bg.wasm")), "module").unwrap();
        std::fs::write(dir.join(format!("{BUNDLE}.d.ts")), "types").unwrap();
        std::fs::write(snippets.join("inline0.js"), "js").unwrap();

        let names: Vec<String> = bundle_files(&dir).unwrap().into_iter().map(|(_, n)| n).collect();

        assert!(names.contains(&format!("{BUNDLE}.js")), "{names:?}");
        assert!(names.contains(&format!("{BUNDLE}_bg.wasm")), "{names:?}");
        assert!(
            names.contains(&"snippets/renzora_webfs-abc123/inline0.js".to_string()),
            "the glue imports this by relative path: {names:?}"
        );
        assert!(
            !names.iter().any(|n| n.ends_with(".d.ts")),
            "declarations are for a build step a game does not have: {names:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_page_loads_the_rpak_before_starting() {
        let html = index_html("My Game");
        let rpak = html.find("set_rpak(rpakBytes)").expect("injects the rpak");
        let start = html.find("start();").expect("starts the app");
        assert!(rpak < start, "the rpak must be injected before start()");
        assert!(html.contains("./renzora-runtime.js"));
    }

    /// The page MUST carry `<canvas id="bevy">`. `renzora_runtime` sets
    /// `canvas: Some("#bevy")` and bevy_winit panics rather than falling back
    /// when the selector matches nothing — which reaches the page as a bare
    /// `RuntimeError: unreachable`, with `Cannot find element: #bevy` visible
    /// only in the console. A page without it is a game that cannot start.
    #[test]
    fn the_page_provides_the_canvas_bevy_looks_for() {
        let html = index_html("My Game");
        assert!(
            html.contains(r#"<canvas id="bevy">"#),
            "bevy_winit resolves `#bevy` and panics if it is absent:\n{html}"
        );
        // Sized by its parent, never by CSS on the canvas itself — the window
        // also sets `fit_canvas_to_parent`, and the two fight over the surface.
        assert!(html.contains("#stage"), "the canvas needs a fixed-size parent");
        assert!(
            !html.contains("canvas.width = window.innerWidth"),
            "hand-sizing the canvas fights `fit_canvas_to_parent`"
        );
    }
}
