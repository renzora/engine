//! Wrap a staged `dist/<platform>/` into the shape that ships.
//!
//! On Windows a staged tree already *is* the shipping layout — a flat folder
//! with the exe at the top. Linux and macOS are not: they ship an AppImage and
//! a `.app`, and several things downstream look for the binary at the path
//! inside those bundles rather than at the top of the platform directory:
//!
//! ```text
//! Platform::LinuxX64 => bundle_inner(&pdir, ".AppDir", &["renzora"]),
//! Platform::MacOSX64 => bundle_inner(&pdir, ".app", &["Contents", "MacOS", "renzora"]),
//! ```
//!
//! — `TemplateManager::scan` in `renzora_export`, which is how the editor finds
//! a locally built export template, and `package-release.sh`, which walks the
//! same three layouts. A flat Linux or macOS tree is invisible to both.
//!
//! This used to live only in `docker/build-all.sh`, which was fine while every
//! published desktop build came out of a container. It doesn't any more: an
//! editor carries a plugin SDK, an SDK cannot be cross-built (its proc macros
//! belong to whatever ran the compiler), so the published editors are built
//! natively by this xtask instead — and they arrived flat.
//!
//! **Opt-in, via `cargo renzora dist --bundle`.** Wrapping on every build would
//! make local iteration pay for `mksquashfs` and would move the binary out from
//! under `cargo renzora`'s launch step. CI asks for it; a contributor doesn't.
//!
//! # What stays outside the bundle, and why it is a Linux rule only
//!
//! On Linux `sdk/` (or `sdk.tar.zst`) and `resources/` stay OUTSIDE the
//! AppImage. `renzora_native_build::install::root()` resolves the install
//! directory from `$APPIMAGE` when it is set, which points at the `.AppImage`
//! *file*, so the editor looks beside the bundle rather than within it — and it
//! has to, because an AppImage is a read-only squashfs that a 1.9 GB tree could
//! not be unpacked into even if it fitted.
//!
//! **macOS is the opposite case and used to be handled as if it were the same
//! one.** There is no `$APPIMAGE` equivalent, so `root()` is just
//! `current_exe().parent()` — `Renzora Engine.app/Contents/MacOS/`. Anything
//! left at the top of the platform directory is somewhere the editor never
//! looks. That shipped: `sdk.tar.zst` sat beside the `.app`, `sdk_state()`
//! returned `Absent`, and the first-launch setup that unpacks the SDK simply
//! never ran, so Rust scripts and native plugins could not be built at all.
//! `resources/icon.png` was invisible for the same reason.
//!
//! So on macOS `resources/` is moved in here, and `sdk.tar.zst` is placed in
//! `Contents/MacOS/` by whoever packs it (the `Pack the plugin SDK` step in
//! `.github/workflows/build-engine.yml`) — it cannot happen here, because at
//! wrap time the SDK is still the extracted tree that packing consumes, and
//! sealing a bundle with 1.9 GB of crate metadata inside it is both slow and
//! pointless.
//!
//! The archive is only ever READ from there. `renzora_native_build::install`
//! unpacks it to `~/Library/Application Support/renzora/sdk` and never deletes
//! it, because writing 1.9 GB into `Contents/` and removing a sealed resource
//! both invalidate the bundle's signature — which is the one thing a `.app` is
//! supposed to be able to promise. That is what lets this step seal the bundle
//! and expect the seal to survive being used.
//!
//! Not a total guarantee, and the remaining gap is worth knowing about: a
//! marketplace install, or a bundled plugin whose stamp has gone stale, still
//! writes into `Contents/MacOS/plugins/`. A shipped install whose stamps match
//! does not, so an ordinary launch now leaves the bundle untouched — where the
//! SDK unpack used to make that impossible for every user on every first run.

use std::path::Path;

use crate::Platform;

/// Wrap `out` in place. A no-op on Windows, where the staged tree already ships.
pub(crate) fn wrap(repo: &Path, out: &Path, plat: &Platform) -> std::io::Result<()> {
    let _ = (repo, out, plat);
    #[cfg(target_os = "linux")]
    return linux::wrap(repo, out);
    #[cfg(target_os = "macos")]
    return macos::wrap(repo, out);
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        println!("[xtask] --bundle: nothing to do on this platform (a flat tree is what ships)");
        Ok(())
    }
}

/// `mv` for the files this module moves: same filesystem every time, so a
/// rename is enough, but fall back to copy+remove rather than fail if the
/// staging directory ever straddles a mount.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    std::fs::copy(from, to)?;
    std::fs::remove_file(from)
}

/// Move every file directly under `dir` whose name ends in `suffix` into `into`.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn move_matching(dir: &Path, suffix: &str, into: &Path) -> std::io::Result<()> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = crate::file_name(&path);
        if name.ends_with(suffix) {
            move_file(&path, &into.join(&name))?;
        }
    }
    Ok(())
}

/// Move the binaries the bundle carries. `renzora` is required — it is the
/// runtime, and the export template downstream reads *it* rather than the
/// editor. The other two are optional: a runtime-only tree is a valid thing to
/// wrap, which is what keeps the container's template lanes working.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn move_binaries(out: &Path, into: &Path) -> std::io::Result<()> {
    move_file(&out.join("renzora"), &into.join("renzora"))?;
    for name in ["renzora-editor", "renzora-update"] {
        let src = out.join(name);
        if src.is_file() {
            move_file(&src, &into.join(name))?;
        }
    }
    Ok(())
}

/// Move a whole directory into the bundle. A no-op when it was never staged —
/// `resources/` only exists when the repository had an `icon.png` to stage.
#[cfg(target_os = "macos")]
fn move_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    if !from.is_dir() {
        return Ok(());
    }
    let _ = std::fs::remove_dir_all(to);
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    // Staging and bundle are the same filesystem in every path that reaches
    // here, so the rename is what runs. Copy is the fallback for a `dist/` that
    // straddles a mount, matching `move_file`.
    copy_tree(from, to)?;
    std::fs::remove_dir_all(from)
}

/// Recursive copy, for `move_dir`'s cross-filesystem fallback.
#[cfg(target_os = "macos")]
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

/// Move `out/plugins/*<suffix>` into `into/plugins/`, removing the now-empty
/// source directory so the wrapped tree has one plugins dir, not two.
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn move_plugins(out: &Path, into: &Path, suffix: &str) -> std::io::Result<()> {
    let src = out.join("plugins");
    if !src.is_dir() {
        return Ok(());
    }
    let dst = into.join("plugins");
    std::fs::create_dir_all(&dst)?;
    move_matching(&src, suffix, &dst)?;
    // Only if empty — a leftover file means something unexpected is in there
    // and silently deleting it would be worse than leaving the directory.
    let _ = std::fs::remove_dir(&src);
    Ok(())
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;

    const APPRUN: &str = r#"#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
export LD_LIBRARY_PATH="$HERE:$HERE/plugins:${LD_LIBRARY_PATH:-}"
# This AppImage IS the editor, so launch the editor binary. `renzora` beside it
# is the runtime the editor spawns for Play — falling back to it keeps an
# editor-less (runtime-only) build launchable rather than silently dead.
if [ -x "$HERE/renzora-editor" ]; then
    exec "$HERE/renzora-editor" "$@"
fi
exec "$HERE/renzora" "$@"
"#;

    const DESKTOP: &str = "[Desktop Entry]\n\
        Type=Application\n\
        Name=Renzora Engine\n\
        Exec=renzora-editor\n\
        Icon=renzora-engine\n\
        Categories=Development;Graphics;\n\
        Terminal=false\n";

    pub(super) fn wrap(repo: &Path, out: &Path) -> std::io::Result<()> {
        if !out.join("renzora").is_file() {
            println!("[xtask] --bundle: no renzora binary in {} — nothing to wrap", out.display());
            return Ok(());
        }

        let appdir = out.join("Renzora Engine.AppDir");
        if appdir.exists() {
            std::fs::remove_dir_all(&appdir)?;
        }
        std::fs::create_dir_all(appdir.join("plugins"))?;

        super::move_binaries(out, &appdir)?;
        super::move_matching(out, ".so", &appdir)?;
        super::move_plugins(out, &appdir, ".so")?;

        let apprun = appdir.join("AppRun");
        std::fs::write(&apprun, APPRUN)?;
        crate::make_executable(&apprun)?;
        std::fs::write(appdir.join("renzora-engine.desktop"), DESKTOP)?;

        // `.DirIcon` as well as the named icon: appimagetool wants the former,
        // desktop environments read the latter.
        let icon = repo.join("icon.png");
        if icon.is_file() {
            std::fs::copy(&icon, appdir.join("renzora-engine.png"))?;
            std::fs::copy(&icon, appdir.join(".DirIcon"))?;
        }

        // appimagetool is optional on purpose. The AppDir alone already
        // satisfies everything that reads the tree — `TemplateManager::scan`
        // looks for the directory, not the `.AppImage` — so a runner without
        // the tool still produces a usable build rather than failing the lane.
        let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x86_64" };
        let image = out.join(format!("Renzora Engine-{arch}.AppImage"));
        let status = std::process::Command::new("appimagetool")
            .env("ARCH", arch)
            .arg(&appdir)
            .arg(&image)
            .status();
        match status {
            Ok(s) if s.success() => println!("[xtask] built {}", image.display()),
            Ok(_) => println!("[xtask] WARN: appimagetool failed; AppDir left at {}", appdir.display()),
            Err(_) => {
                println!("[xtask] appimagetool not found; AppDir left at {}", appdir.display())
            }
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    pub(super) fn wrap(repo: &Path, out: &Path) -> std::io::Result<()> {
        if !out.join("renzora").is_file() {
            println!("[xtask] --bundle: no renzora binary in {} — nothing to wrap", out.display());
            return Ok(());
        }

        let app = out.join("Renzora Engine.app");
        let macos_dir = app.join("Contents").join("MacOS");
        let res_dir = app.join("Contents").join("Resources");
        if app.exists() {
            std::fs::remove_dir_all(&app)?;
        }
        std::fs::create_dir_all(macos_dir.join("plugins"))?;
        std::fs::create_dir_all(&res_dir)?;

        super::move_binaries(out, &macos_dir)?;
        super::move_matching(out, ".dylib", &macos_dir)?;
        super::move_plugins(out, &macos_dir, ".dylib")?;
        // `resources/icon.png` is read as `current_exe().parent()/resources/`
        // by the splash, the top menu and the exporter's fallback, so on macOS
        // it belongs beside the executable inside the bundle. Left at the top
        // of the platform directory it is simply never found, and the editor
        // draws no brand mark. Before `seal()`, so the ad-hoc signature covers
        // it.
        super::move_dir(&out.join("resources"), &macos_dir.join("resources"))?;

        let icon = repo.join("icon.png");
        if icon.is_file() {
            match icns(&std::fs::read(&icon)?) {
                Some(bytes) => std::fs::write(res_dir.join("renzora.icns"), bytes)?,
                None => println!("[xtask] WARN: unsupported icon size; .app ships without an icon"),
            }
        }

        // CFBundleExecutable names the EDITOR — double-clicking the bundle must
        // open the editor, not the game runtime that ships alongside it for
        // Play. Falls back to `renzora` so a runtime-only tree still yields a
        // launchable bundle.
        let main_bin =
            if macos_dir.join("renzora-editor").is_file() { "renzora-editor" } else { "renzora" };
        std::fs::write(app.join("Contents").join("Info.plist"), plist(main_bin))?;
        seal(&app);
        println!("[xtask] built {}", app.display());
        Ok(())
    }

    /// Ad-hoc sign the assembled bundle.
    ///
    /// `fixup_macos` already signed each file, but it did so while they were
    /// still loose, and that is not the same signature. Once a Mach-O sits at
    /// `Contents/MacOS/<CFBundleExecutable>` macOS evaluates it under *bundle*
    /// rules, which want a sealed `Contents/_CodeSignature/CodeResources`
    /// covering the rest of the tree. Without one the signature is not missing,
    /// it is **invalid** — arm64 SIGKILLs the process at `exec` and Gatekeeper
    /// reports a downloaded copy as *"Renzora Engine is damaged and can't be
    /// opened"*, which is how this shipped from 2026-08-29 until it was found.
    ///
    /// So this has to run last: the seal covers `Info.plist`, so signing before
    /// that file is written produces a signature that fails the moment it is.
    ///
    /// This is the `rcodesign sign "$APP"` the container's `wrap_macos_app` did.
    /// The step was simply lost when the macOS editors moved off osxcross onto
    /// native runners and the wrap was ported here.
    ///
    /// Best-effort, and it stays that way deliberately: `codesign` is Xcode's,
    /// so a machine with only the Rust toolchain has none, and refusing to
    /// produce a `.app` at all would be worse than producing one the person who
    /// asked for it can still run locally. CI is the case that must not ship
    /// unsigned, and it checks the result rather than trusting this.
    ///
    /// Ad-hoc gets a launchable bundle, not a silent one: unnotarized, the
    /// first launch of a downloaded copy still needs Open Anyway. Only a
    /// Developer ID identity plus notarization removes that, and neither exists
    /// in this pipeline yet.
    fn seal(app: &Path) {
        let ok = std::process::Command::new("codesign")
            .args(["--force", "--deep", "--sign", "-"])
            .arg(app)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            println!(
                "[xtask] WARN: codesign failed; macOS will refuse to launch {}",
                app.display()
            );
        }
    }

    /// Build a single-image `.icns` from a PNG.
    ///
    /// An icns is a header plus typed chunks, and a chunk may be a PNG verbatim,
    /// so a one-size icon needs no Apple tooling and no `python3` — which is
    /// what `docker/build-all.sh` shelled out to. The chunk type is keyed off
    /// the PNG's pixel width, read from the IHDR at bytes 16..20.
    fn icns(png: &[u8]) -> Option<Vec<u8>> {
        let width = u32::from_be_bytes(png.get(16..20)?.try_into().ok()?);
        let typ: &[u8; 4] = match width {
            16 => b"icp4",
            32 => b"icp5",
            64 => b"icp6",
            128 => b"ic07",
            256 => b"ic08",
            512 => b"ic09",
            1024 => b"ic10",
            _ => return None,
        };
        let mut chunk = Vec::with_capacity(png.len() + 8);
        chunk.extend_from_slice(typ);
        chunk.extend_from_slice(&((png.len() + 8) as u32).to_be_bytes());
        chunk.extend_from_slice(png);

        let mut out = Vec::with_capacity(chunk.len() + 8);
        out.extend_from_slice(b"icns");
        out.extend_from_slice(&((chunk.len() + 8) as u32).to_be_bytes());
        out.extend_from_slice(&chunk);
        Some(out)
    }

    fn plist(main_bin: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>            <string>Renzora Engine</string>
    <key>CFBundleDisplayName</key>     <string>Renzora Engine</string>
    <key>CFBundleIdentifier</key>      <string>org.renzora.engine</string>
    <key>CFBundleExecutable</key>      <string>{main_bin}</string>
    <key>CFBundleIconFile</key>        <string>renzora</string>
    <key>CFBundlePackageType</key>     <string>APPL</string>
    <key>CFBundleVersion</key>         <string>0.2.0</string>
    <key>CFBundleShortVersionString</key> <string>0.2.0</string>
    <key>LSMinimumSystemVersion</key>  <string>11.0</string>
    <key>NSHighResolutionCapable</key> <true/>
</dict>
</plist>
"#
        )
    }
}
