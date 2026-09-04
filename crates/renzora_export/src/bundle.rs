//! Wrap a finished export into the shape its platform expects a *game* to have:
//! an `.AppDir`/`.AppImage` on Linux, a `.app` on macOS.
//!
//! # Why this is not `xtask`'s bundler
//!
//! `xtask/src/bundle.rs` does the same job for the engine's own `dist/`, and it
//! is gated `#[cfg(target_os = ...)]` — correct there, because it wraps the tree
//! it just built for the machine it is running on.
//!
//! An export is the opposite situation: the platform being wrapped is the one
//! the user picked, which is usually not this one. Both formats are directory
//! layouts plus text files — an `AppRun` script, a `.desktop` entry, an
//! `Info.plist` — so producing either from either host needs no platform tooling
//! and no `#[cfg]`. What the host cannot do is *sign* a `.app` or run
//! `appimagetool`, and both of those are optional: the unsigned bundle and the
//! bare `AppDir` are already usable.
//!
//! # What it is for
//!
//! A lean export is one self-contained binary, which sounds like it needs no
//! wrapper — and on Linux that is nearly true, since an `AppImage` of a static
//! binary buys little beyond the icon and the double-click. On macOS it is not
//! true at all: a bare executable opens a terminal window, has no icon, cannot
//! carry an `Info.plist`, and Gatekeeper treats it as a downloaded command-line
//! tool. A game ships as a `.app` or it does not ship.
//!
//! It composes with every packaging mode rather than being a fourth one. A
//! copy-based export benefits more, not less: its binary, its shared libraries
//! and its `plugins/` folder all have to travel together, and a bundle is
//! exactly the thing that keeps them together.

use std::path::{Path, PathBuf};

use crate::Platform;

/// Everything the wrapper needs to name and fill a bundle.
pub struct BundleSpec<'a> {
    /// Directory the export has already written: binary, libraries, `plugins/`,
    /// `.rpak`. Its contents are MOVED into the bundle.
    pub output_dir: &'a Path,
    /// The game's executable file name, as written by the export.
    pub binary_name: &'a str,
    /// Shown to the player — the bundle's own name, window title and menu entry.
    pub app_name: &'a str,
    /// Reverse-DNS bundle id for macOS. `org.renzora.<slug>` when the project
    /// has not set one.
    pub identifier: &'a str,
    /// PNG icon, if the project has one. A `.desktop` entry and an `Info.plist`
    /// both reference an icon; neither requires that it exist.
    pub icon_png: Option<&'a Path>,
}

/// Wrap `output_dir` for `platform`. Returns what it produced.
///
/// `Ok(None)` for a platform with no bundle format — Windows ships a folder, and
/// the web ships a directory a server points at. Not an error: the caller offers
/// the toggle wherever it is harmless and this decides whether it means anything.
pub fn wrap(
    platform: Platform,
    spec: &BundleSpec,
    progress: &mut dyn FnMut(String),
) -> Result<Option<PathBuf>, String> {
    match platform {
        Platform::LinuxX64 | Platform::LinuxArm64 => linux(spec, progress).map(Some),
        Platform::MacOSX64 | Platform::MacOSArm64 => macos(spec).map(Some),
        _ => Ok(None),
    }
}

/// Is there a bundle format for this platform at all?
///
/// Read by the dialog, so the toggle is absent rather than present-and-inert on
/// a platform it cannot affect.
pub fn supported(platform: Platform) -> bool {
    matches!(
        platform,
        Platform::LinuxX64 | Platform::LinuxArm64 | Platform::MacOSX64 | Platform::MacOSArm64
    )
}

/// The icon to put in the bundle: the author's, the editor's default, or a
/// generated square.
///
/// Three steps because each covers a case the next cannot. An author who picked
/// an icon gets theirs. A project that picked none gets the default staged
/// beside the editor — which is the case a DOWNLOADED editor is in, since it has
/// no repository to read one from. And if even that is missing, a generated
/// square, because the alternative is failing a build that took minutes over
/// artwork: `appimagetool` refuses an AppDir whose `.desktop` names an icon that
/// is not there.
///
/// Returned as PNG bytes rather than a path, since two of the three sources are
/// not files on disk in any predictable place.
fn icon_bytes(spec: &BundleSpec) -> Result<Vec<u8>, String> {
    if let Some(picked) = spec.icon_png.filter(|p| p.is_file()) {
        // Re-encoded rather than copied, for the reason `icon.rs` gives: the
        // picker accepts any raster format the editor can read, and the file it
        // hands over may well not be a PNG despite where it is going.
        if let Ok(bytes) = crate::icon::load_square(picked).and_then(|b| crate::icon::to_png(&b)) {
            return Ok(bytes);
        }
    }
    if let Some(default) =
        crate::build::editor_dir().map(|d| d.join("resources").join("icon.png"))
    {
        if default.is_file() {
            if let Ok(bytes) = std::fs::read(&default) {
                return Ok(bytes);
            }
        }
    }
    crate::icon::to_png(&crate::icon::placeholder())
}

// ── Linux ────────────────────────────────────────────────────────────────────

/// `AppRun` is what an AppImage executes. It has to resolve the payload relative
/// to itself rather than to the working directory, because a user launches an
/// AppImage from wherever they happen to be — and the runtime looks for its
/// `plugins/` folder beside the executable.
const APPRUN: &str = "#!/bin/sh\n\
    HERE=\"$(dirname \"$(readlink -f \"$0\")\")\"\n\
    export LD_LIBRARY_PATH=\"$HERE:$HERE/plugins:${LD_LIBRARY_PATH:-}\"\n\
    exec \"$HERE/{BIN}\" \"$@\"\n";

fn linux(spec: &BundleSpec, progress: &mut dyn FnMut(String)) -> Result<PathBuf, String> {
    let appdir = build_appdir(spec)?;
    squash(spec, &appdir, progress)
}

/// Assemble the `AppDir` — everything an AppImage contains, before it is one.
///
/// Split from [`squash`] so the layout can be tested without a network: the
/// squashing step provisions `appimagetool`, and a unit test that reaches
/// GitHub is a unit test that fails on a train.
fn build_appdir(spec: &BundleSpec) -> Result<PathBuf, String> {
    let appdir = spec.output_dir.join(format!("{}.AppDir", spec.app_name));
    if appdir.exists() {
        std::fs::remove_dir_all(&appdir).map_err(|e| format!("clear {}: {e}", appdir.display()))?;
    }
    std::fs::create_dir_all(&appdir).map_err(|e| format!("create {}: {e}", appdir.display()))?;

    move_payload(spec.output_dir, &appdir, &appdir)?;

    let apprun = appdir.join("AppRun");
    write(&apprun, APPRUN.replace("{BIN}", spec.binary_name))?;
    make_executable(&apprun)?;

    let slug = slug(spec.app_name);
    write(
        &appdir.join(format!("{slug}.desktop")),
        format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name={name}\n\
             Exec={bin}\n\
             Icon={slug}\n\
             Categories=Game;\n\
             Terminal=false\n",
            name = spec.app_name,
            bin = spec.binary_name,
        ),
    )?;

    // `.DirIcon` as well as the named icon: appimagetool reads the former,
    // desktop environments read the latter.
    //
    // Written unconditionally, because for an AppImage the icon is not
    // decoration. The `.desktop` entry above names one, and appimagetool refuses
    // an AppDir where that file is missing — so a project that never set an icon
    // could not be packaged at all, failing at the last step of a build that
    // took minutes.
    let png = icon_bytes(spec)?;
    write(&appdir.join(format!("{slug}.png")), &png)?;
    write(&appdir.join(".DirIcon"), &png)?;

    Ok(appdir)
}

/// Turn a finished `AppDir` into a single `.AppImage`.
fn squash(
    spec: &BundleSpec,
    appdir: &Path,
    progress: &mut dyn FnMut(String),
) -> Result<PathBuf, String> {
    // An `AppDir` is already runnable — `./AppRun` starts the game —
    // so failing here still leaves something that works, and the single file is
    // the only thing lost. But "still works" is not what was asked for: a user
    // who ticked *AppImage* wants an AppImage, and a directory named `.AppDir`
    // is not one.
    let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x86_64" };
    let image = spec.output_dir.join(format!("{}-{arch}.AppImage", spec.app_name));
    let tool = match appimagetool(arch, progress) {
        Ok(t) => t,
        Err(e) => {
            progress(format!("Could not obtain appimagetool ({e}); keeping the AppDir"));
            return Ok(appdir.to_path_buf());
        }
    };

    let status = std::process::Command::new(&tool)
        .env("ARCH", arch)
        // appimagetool is itself an AppImage, and mounting one needs FUSE —
        // absent in a container, on a minimal desktop, and increasingly by
        // default. This makes it unpack itself to a temp dir and run from there,
        // which works everywhere and costs a second.
        .env("APPIMAGE_EXTRACT_AND_RUN", "1")
        .arg(appdir)
        .arg(&image)
        .output();
    match status {
        Ok(o) if o.status.success() => {
            // The AppDir was scaffolding. Leaving it beside the image doubles
            // what the user uploads and leaves two things that look like the
            // deliverable.
            let _ = std::fs::remove_dir_all(appdir);
            Ok(image)
        }
        Ok(o) => {
            progress(format!(
                "appimagetool failed ({}); keeping the AppDir: {}",
                o.status,
                String::from_utf8_lossy(&o.stderr).trim()
            ));
            Ok(appdir.to_path_buf())
        }
        Err(e) => {
            progress(format!("Could not run appimagetool ({e}); keeping the AppDir"));
            Ok(appdir.to_path_buf())
        }
    }
}

/// Find `appimagetool`, fetching it if this machine has none.
///
/// Provisioned rather than required, the same way the lean build provisions a
/// Rust toolchain: an export that stops to tell someone to install a packaging
/// tool has failed at the one job it was asked to do. The order goes from most
/// deliberate to least — an explicit override, then whatever the user installed,
/// then our own cached copy, then the network.
///
/// Cached in `~/.renzora/tools/` so it is fetched once per machine rather than
/// once per export — see [`crate::templates::user_tools_dir`] for why that is
/// not the install directory.
fn appimagetool(arch: &str, progress: &mut dyn FnMut(String)) -> Result<PathBuf, String> {
    if let Some(explicit) = std::env::var_os("RENZORA_APPIMAGETOOL") {
        let path = PathBuf::from(explicit);
        if path.is_file() {
            return Ok(path);
        }
        return Err("RENZORA_APPIMAGETOOL is set but is not a file".into());
    }
    if std::process::Command::new("appimagetool")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
    {
        return Ok(PathBuf::from("appimagetool"));
    }

    let cached = crate::templates::user_tools_dir()
        .ok_or("cannot locate a home directory to cache in")?
        .join(format!("appimagetool-{arch}.AppImage"));
    if cached.is_file() {
        return Ok(cached);
    }

    let url = format!(
        "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-{arch}.AppImage"
    );
    progress("Fetching appimagetool (once per machine)...".to_string());
    let response = renzora_net::Request::get(&url)
        .header("User-Agent", "renzora-export")
        .send()
        .map_err(|e| format!("{e}"))?;
    if !response.is_ok() {
        return Err(format!("HTTP {}", response.status));
    }
    if let Some(parent) = cached.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    // Written to a sibling and renamed, so an interrupted download cannot leave
    // a half-file that every later export then tries to execute.
    let tmp = cached.with_extension("part");
    std::fs::write(&tmp, &response.body).map_err(|e| format!("{}: {e}", tmp.display()))?;
    make_executable(&tmp)?;
    std::fs::rename(&tmp, &cached).map_err(|e| format!("{}: {e}", cached.display()))?;
    Ok(cached)
}

// ── macOS ────────────────────────────────────────────────────────────────────

fn macos(spec: &BundleSpec) -> Result<PathBuf, String> {
    let app = spec.output_dir.join(format!("{}.app", spec.app_name));
    let contents = app.join("Contents");
    let macos_dir = contents.join("MacOS");
    let resources = contents.join("Resources");
    if app.exists() {
        std::fs::remove_dir_all(&app).map_err(|e| format!("clear {}: {e}", app.display()))?;
    }
    std::fs::create_dir_all(&macos_dir).map_err(|e| format!("create {}: {e}", macos_dir.display()))?;
    std::fs::create_dir_all(&resources)
        .map_err(|e| format!("create {}: {e}", resources.display()))?;

    move_payload(spec.output_dir, &macos_dir, &app)?;

    // The `Info.plist` names an icon whether or not one exists, and a `.app`
    // with a missing `CFBundleIconFile` shows the generic application icon —
    // which is indistinguishable from a broken bundle. The placeholder is
    // 256×256, one of the sizes `icns` accepts.
    if let Some(bytes) = icon_bytes(spec).ok().as_deref().and_then(icns) {
        let _ = std::fs::write(resources.join("icon.icns"), bytes);
    }

    write(&contents.join("Info.plist"), plist(spec))?;
    Ok(app)
}

/// Build a single-image `.icns` from a PNG.
///
/// An icns is a header plus typed chunks, and a chunk may be a PNG verbatim, so
/// a one-size icon needs no Apple tooling — which is what makes this work from a
/// Linux host. The chunk type is keyed off the PNG's pixel width, read from the
/// IHDR at bytes 16..20; an unusual size yields `None` and the bundle ships
/// without an icon rather than with a corrupt one.
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

fn plist(spec: &BundleSpec) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>               <string>{name}</string>
    <key>CFBundleDisplayName</key>        <string>{name}</string>
    <key>CFBundleIdentifier</key>         <string>{id}</string>
    <key>CFBundleExecutable</key>         <string>{bin}</string>
    <key>CFBundleIconFile</key>           <string>icon</string>
    <key>CFBundlePackageType</key>        <string>APPL</string>
    <key>CFBundleVersion</key>            <string>1.0</string>
    <key>CFBundleShortVersionString</key> <string>1.0</string>
    <key>LSMinimumSystemVersion</key>     <string>11.0</string>
    <key>NSHighResolutionCapable</key>    <true/>
    <key>LSApplicationCategoryType</key>  <string>public.app-category.games</string>
</dict>
</plist>
"#,
        name = spec.app_name,
        id = spec.identifier,
        bin = spec.binary_name,
    )
}

// ── Shared ───────────────────────────────────────────────────────────────────

/// Move everything the export wrote into the bundle's executable directory.
///
/// Moves rather than copies: the export directory is the deliverable, and
/// leaving a second loose copy of a 100 MB binary beside the bundle doubles the
/// size of what the user then has to upload, while looking like the bundle
/// failed to take it.
///
/// `skip` is the bundle root, so the walk cannot descend into what it is
/// building — the `.app` and the `.AppDir` live inside the directory being
/// drained.
fn move_payload(from: &Path, into: &Path, skip: &Path) -> Result<(), String> {
    let entries =
        std::fs::read_dir(from).map_err(|e| format!("read {}: {e}", from.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path == skip {
            continue;
        }
        let dest = into.join(entry.file_name());
        // `rename` first: same filesystem by construction, so this is instant
        // for a binary that may be hundreds of megabytes. The copy is for the
        // case it is not — a bind mount, or an output directory on another
        // volume.
        if std::fs::rename(&path, &dest).is_ok() {
            continue;
        }
        if path.is_dir() {
            copy_tree(&path, &dest)?;
            let _ = std::fs::remove_dir_all(&path);
        } else {
            std::fs::copy(&path, &dest)
                .map_err(|e| format!("copy {} → {}: {e}", path.display(), dest.display()))?;
            let _ = std::fs::remove_file(&path);
        }
    }
    Ok(())
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| format!("create {}: {e}", to.display()))?;
    let entries = std::fs::read_dir(from).map_err(|e| format!("read {}: {e}", from.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let dest = to.join(entry.file_name());
        if path.is_dir() {
            copy_tree(&path, &dest)?;
        } else {
            std::fs::copy(&path, &dest)
                .map_err(|e| format!("copy {} → {}: {e}", path.display(), dest.display()))?;
        }
    }
    Ok(())
}

fn write(path: &Path, contents: impl AsRef<[u8]>) -> Result<(), String> {
    std::fs::write(path, contents).map_err(|e| format!("write {}: {e}", path.display()))
}

/// Mark a file executable where that is a thing. `AppRun` is a shell script and
/// an AppImage that cannot execute it is not an AppImage.
fn make_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("chmod {}: {e}", path.display()))?;
    }
    // On Windows the bit does not exist, and the AppDir is being produced for
    // another machine to run — where the archive's own mode bits carry it.
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// A filename-safe, lowercase form of the app name, for the `.desktop` entry and
/// the icon beside it. Those two must agree — a `.desktop` naming an icon that
/// is not there shows a generic placeholder.
pub(crate) fn slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut last_dash = true;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "game".to_string()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("renzora_bundle_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("plugins")).unwrap();
        std::fs::write(d.join("mygame"), b"ELF").unwrap();
        std::fs::write(d.join("mygame.rpak"), b"pak").unwrap();
        std::fs::write(d.join("plugins").join("rain.so"), b"so").unwrap();
        d
    }

    fn spec<'a>(dir: &'a Path) -> BundleSpec<'a> {
        BundleSpec {
            output_dir: dir,
            binary_name: "mygame",
            app_name: "My Game",
            identifier: "org.renzora.my-game",
            icon_png: None,
        }
    }

    /// Everything the export wrote has to end up inside the bundle — a `.rpak`
    /// or a `plugins/` folder left outside it is a game that starts and then
    /// cannot find its content.
    #[test]
    fn the_payload_moves_into_the_app_bundle() {
        let d = spec_dir("macos");
        let app = wrap(Platform::MacOSX64, &spec(&d), &mut |_| {}).unwrap().unwrap();

        let macos = app.join("Contents").join("MacOS");
        assert!(macos.join("mygame").is_file());
        assert!(macos.join("mygame.rpak").is_file());
        assert!(macos.join("plugins").join("rain.so").is_file());
        assert!(app.join("Contents").join("Info.plist").is_file());
        // Moved, not copied: a second 100 MB binary beside the bundle doubles
        // what the user uploads and looks like the bundle failed to take it.
        assert!(!d.join("mygame").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_appdir_is_runnable() {
        let d = spec_dir("linux");
        // `build_appdir` rather than `wrap`: the squashing step provisions
        // appimagetool over the network, and what is being asserted here is the
        // layout, which is complete before that runs.
        let appdir = build_appdir(&spec(&d)).unwrap();
        assert!(appdir.join("AppRun").is_file());
        assert!(appdir.join("mygame").is_file());
        assert!(appdir.join("plugins").join("rain.so").is_file());
        assert!(appdir.join("my-game.desktop").is_file());
        let apprun = std::fs::read_to_string(appdir.join("AppRun")).unwrap();
        assert!(apprun.contains("exec \"$HERE/mygame\""), "{apprun}");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Windows ships a folder and the web ships a directory a server points at.
    /// Neither is an error — the toggle simply means nothing there.
    #[test]
    fn a_platform_with_no_bundle_format_is_not_an_error() {
        let d = spec_dir("windows");
        assert!(wrap(Platform::WindowsX64, &spec(&d), &mut |_| {}).unwrap().is_none());
        assert!(!supported(Platform::WindowsX64));
        assert!(supported(Platform::MacOSArm64));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// An AppImage cannot be built without an icon — `appimagetool` refuses an
    /// AppDir whose `.desktop` names one that is not there, which turned a
    /// project with no artwork into a build that failed at the last step after
    /// several minutes of compiling.
    #[test]
    fn an_appdir_always_carries_an_icon() {
        let d = spec_dir("noicon");
        let appdir = build_appdir(&spec(&d)).unwrap();
        let named = appdir.join("my-game.png");
        assert!(named.is_file(), "the .desktop names my-game; the file must exist");
        assert!(appdir.join(".DirIcon").is_file(), "appimagetool reads .DirIcon");

        // A real PNG, not a placeholder file — and the name in the `.desktop`
        // has to match, or the icon silently does not appear.
        let bytes = std::fs::read(&named).unwrap();
        assert_eq!(&bytes[1..4], b"PNG", "not a PNG");
        let desktop = std::fs::read_to_string(appdir.join("my-game.desktop")).unwrap();
        assert!(desktop.contains("Icon=my-game"), "{desktop}");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// The placeholder is 256×256 because `icns` only accepts a fixed set of
    /// sizes, and a `.app` whose icon silently failed to encode shows the
    /// generic application icon — which looks like a broken bundle.
    #[test]
    fn the_placeholder_is_an_icns_size() {
        let png = crate::icon::to_png(&crate::icon::placeholder()).unwrap();
        let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
        assert_eq!(width, 256);
        assert!(icns(&png).is_some(), "256 must be an accepted icns size");
    }

    #[test]
    fn the_slug_survives_a_hostile_name() {
        assert_eq!(slug("My Game"), "my-game");
        assert_eq!(slug("  Ünïcode!!  "), "n-code");
        assert_eq!(slug("!!!"), "game");
    }
}
