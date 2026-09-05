//! Downloading an update, staging it, and handing the swap to the sidecar.
//!
//! The editor cannot replace itself while it is running, so the last step is
//! done by `renzora-update`, a tiny separate binary that ships beside the editor
//! (`tools/updater`). This module gets everything ready for it and then gets out
//! of the way.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::check::ReleaseEntry;

const USER_AGENT: &str = "renzora-editor";
/// An engine zip is 75–150 MB; the default request timeout is sized for API
/// calls, not for that.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60 * 30);

/// What an install of the engine physically *is* on this platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallKind {
    /// A directory of files — a Windows install folder, or a macOS `.app`
    /// bundle (which is a directory too, just one Finder renders specially).
    Directory,
    /// A single file — a Linux `.AppImage`.
    File,
}

/// Where this engine is installed and how to put a new one there.
#[derive(Debug, Clone)]
pub struct InstallLayout {
    pub kind: InstallKind,
    /// The thing the sidecar replaces.
    pub target: PathBuf,
    /// What it launches afterwards.
    pub relaunch: PathBuf,
    /// True when the editor is running out of a source checkout's `dist/`.
    ///
    /// Updating in place there would overwrite build output with a release —
    /// recoverable with a rebuild, but never what anyone meant. The UI offers a
    /// check but no install when this is set.
    pub is_source_checkout: bool,
}

impl InstallLayout {
    /// The same layout aimed somewhere else — what the dialog's install-path
    /// field produces.
    ///
    /// Everything derived from the path has to be re-derived, not carried over:
    /// which binary to relaunch depends on whether the new target is a bundle,
    /// a file or a plain directory, and `is_source_checkout` is a property of
    /// *that* path — pointing the install at `C:\Program Files\Renzora` must not
    /// keep warning about overwriting build output just because the editor you
    /// clicked in happened to be running from a checkout (and the reverse
    /// matters far more).
    pub fn retargeted(&self, target: PathBuf) -> Self {
        let is_bundle = target.extension().and_then(|e| e.to_str()) == Some("app");
        // A single-file install (Linux AppImage) and a macOS bundle both relaunch
        // the thing that was replaced; a plain directory relaunches the editor
        // inside it, exactly as `detect_layout` does.
        let relaunch = if self.kind == InstallKind::File || is_bundle {
            target.clone()
        } else {
            target.join(engine_exe())
        };
        Self {
            kind: self.kind.clone(),
            is_source_checkout: is_source_checkout(&target),
            target,
            relaunch,
        }
    }
}

/// Work out what to replace, from where the running editor actually lives.
pub fn detect_layout() -> Result<InstallLayout, String> {
    let exe = std::env::current_exe().map_err(|e| format!("Cannot locate this executable: {e}"))?;
    let exe_dir = exe
        .parent()
        .ok_or("This executable has no parent directory")?
        .to_path_buf();

    // A Linux AppImage mounts itself read-only under /tmp, so `current_exe()`
    // points into the mount, not the install. The runtime exports $APPIMAGE with
    // the real path — the only way to find the file the user actually has.
    #[cfg(target_os = "linux")]
    if let Some(appimage) = std::env::var_os("APPIMAGE").map(PathBuf::from) {
        if appimage.is_file() {
            return Ok(InstallLayout {
                kind: InstallKind::File,
                target: appimage.clone(),
                relaunch: appimage,
                is_source_checkout: false,
            });
        }
    }

    // macOS: the unit of install is the bundle, not the executable buried three
    // levels inside it. Replacing only `Contents/MacOS/renzora-editor` would
    // leave the old Info.plist, resources and — fatally — the old code
    // signature, which seals the bundle's layout as well as its bytes.
    #[cfg(target_os = "macos")]
    {
        let mut dir: Option<&Path> = Some(exe_dir.as_path());
        while let Some(d) = dir {
            if d.extension().and_then(|e| e.to_str()) == Some("app") {
                return Ok(InstallLayout {
                    kind: InstallKind::Directory,
                    target: d.to_path_buf(),
                    relaunch: d.to_path_buf(),
                    is_source_checkout: is_source_checkout(d),
                });
            }
            dir = d.parent();
        }
    }

    // The one binary — see `engine_exe`. Pointing this at `renzora-editor` meant
    // that even an install that *did* succeed relaunched into a file that has
    // not shipped since the editor became a loadable image.
    let relaunch = exe_dir.join(engine_exe());
    Ok(InstallLayout {
        is_source_checkout: is_source_checkout(&exe_dir),
        kind: InstallKind::Directory,
        target: exe_dir,
        relaunch,
    })
}

/// Is `start` inside an engine source checkout?
///
/// Same signature `renzora_export::build::find_engine_source` looks for — a
/// workspace root is a `Cargo.toml` beside a `crates/` directory and a
/// `src/main.rs`, all three so a sub-crate's manifest can't be mistaken for it.
/// Duplicated rather than depended on: this crate has no other reason to link
/// the exporter, and it is three lines.
fn is_source_checkout(start: &Path) -> bool {
    let mut dir = Some(start);
    while let Some(d) = dir {
        if d.join("Cargo.toml").is_file()
            && d.join("crates").is_dir()
            && d.join("src").join("main.rs").is_file()
        {
            return true;
        }
        dir = d.parent();
    }
    false
}

/// Root of the per-user staging area for a given release.
pub fn staging_root(tag: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)?;
    // Sanitised: a tag reaches this from a network response, and it becomes a
    // path component.
    let safe: String = tag
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '.' { c } else { '_' })
        .collect();
    Some(home.join(".renzora").join("updates").join(safe))
}

/// Live state of a download running on a worker thread.
pub struct DownloadHandle {
    pub downloaded: Arc<AtomicU64>,
    pub total: u64,
    pub done: Arc<AtomicBool>,
    /// `Ok(staged source path)` once finished, or the failure.
    pub outcome: Arc<Mutex<Option<Result<PathBuf, String>>>>,
}

/// Download the engine zip for one release, verify it, and extract it.
///
/// Takes the chosen [`ReleaseEntry`] rather than "the newest", because the
/// dialog lets you pick a version — including going back to an older one.
///
/// Returns immediately; poll [`DownloadHandle`].
pub fn spawn_download(entry: &ReleaseEntry, layout: &InstallLayout) -> Result<DownloadHandle, String> {
    let url = entry
        .download_url
        .clone()
        .ok_or("That release has no build for your platform.")?;
    let expected_sha = entry.sha256.clone();
    let root = staging_root(&entry.tag)
        .ok_or("No home directory to stage the update in (neither HOME nor USERPROFILE is set).")?;
    let kind = layout.kind.clone();

    let handle = DownloadHandle {
        downloaded: Arc::new(AtomicU64::new(0)),
        total: entry.size,
        done: Arc::new(AtomicBool::new(false)),
        outcome: Arc::new(Mutex::new(None)),
    };

    let downloaded = handle.downloaded.clone();
    let done = handle.done.clone();
    let outcome = handle.outcome.clone();

    std::thread::spawn(move || {
        let res = download_and_stage(&url, &root, expected_sha.as_deref(), kind, &downloaded);
        if let Ok(mut slot) = outcome.lock() {
            *slot = Some(res);
        }
        done.store(true, Ordering::SeqCst);
    });

    Ok(handle)
}

fn download_and_stage(
    url: &str,
    root: &Path,
    expected_sha: Option<&str>,
    kind: InstallKind,
    downloaded: &Arc<AtomicU64>,
) -> Result<PathBuf, String> {
    // Start from nothing: a previous half-finished attempt at the same tag would
    // otherwise be extracted over and silently mixed with the new one.
    let _ = fs::remove_dir_all(root);
    fs::create_dir_all(root).map_err(|e| format!("Cannot create {}: {e}", root.display()))?;

    let zip_path = root.join("engine.zip");
    let mut file =
        fs::File::create(&zip_path).map_err(|e| format!("Cannot write {}: {e}", zip_path.display()))?;

    // Streamed rather than buffered whole: an engine zip is well over 100 MB,
    // and this is also the only way to report progress — the transport reports
    // no length of its own, so the size from the release manifest is the total.
    let mut stream = renzora_net::Request::get(url)
        .header("User-Agent", USER_AGENT)
        .timeout(DOWNLOAD_TIMEOUT)
        .send_stream()
        .map_err(|e| format!("Download failed: {e}"))?;

    let mut hasher = Sha256::new();
    let mut got: u64 = 0;
    for chunk in &mut stream {
        file.write_all(&chunk.data)
            .map_err(|e| format!("Cannot write update file: {e}"))?;
        hasher.update(&chunk.data);
        got += chunk.data.len() as u64;
        downloaded.store(got, Ordering::SeqCst);
    }
    // A stream that fails halfway delivers what it got and then stops, which is
    // indistinguishable from success until asked — so ask.
    if let Some(e) = stream.error() {
        return Err(format!("Download failed: {e}"));
    }
    file.flush().map_err(|e| format!("Cannot finish writing: {e}"))?;
    drop(file);

    let actual = hex(&hasher.finalize());
    if let Some(expected) = expected_sha {
        if actual != expected {
            let _ = fs::remove_dir_all(root);
            return Err(format!(
                "Checksum mismatch — expected {expected}, got {actual}. Nothing was installed."
            ));
        }
    }

    let staged = root.join("staged");
    let reader = fs::File::open(&zip_path).map_err(|e| format!("Cannot reopen download: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|e| format!("Downloaded file is not a zip: {e}"))?;
    archive
        .extract(&staged)
        .map_err(|e| format!("Could not extract the update: {e}"))?;
    let _ = fs::remove_file(&zip_path);
    ensure_appimage_executable(&staged);

    resolve_staged_source(&staged, kind)
}

/// Find, inside the extracted tree, the thing that corresponds to what we are
/// replacing.
///
/// The engine zip's shape follows the platform: Windows is a flat folder, macOS
/// is a single `.app`, Linux is a single `.AppImage`. Matching that up here —
/// rather than in the sidecar — keeps the sidecar a dumb, reliable file mover.
fn resolve_staged_source(staged: &Path, kind: InstallKind) -> Result<PathBuf, String> {
    let entries: Vec<PathBuf> = fs::read_dir(staged)
        .map_err(|e| format!("Cannot read the extracted update: {e}"))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();

    let ends_with = |p: &Path, ext: &str| p.extension().and_then(|e| e.to_str()) == Some(ext);

    match kind {
        InstallKind::File => entries
            .iter()
            .find(|p| ends_with(p, "AppImage"))
            .cloned()
            .ok_or_else(|| {
                "This release has no .AppImage for your platform, so an in-place update isn't \
                 possible. Download it from the release page instead."
                    .to_string()
            }),
        InstallKind::Directory => {
            // macOS: the bundle inside.
            if let Some(app) = entries.iter().find(|p| ends_with(p, "app") && p.is_dir()) {
                return Ok(app.clone());
            }
            // Windows: the flat tree itself. Some zip tools produce a single
            // wrapping folder, so unwrap one level if that is what we got.
            for root in std::iter::once(staged.to_path_buf())
                .chain(entries.iter().filter(|p| p.is_dir()).cloned())
            {
                if has_engine_binary(&root) {
                    // The binary alone is the *runtime*; the editor image beside
                    // it is what makes the tree an engine. Both ship as assets on
                    // the same release, so this is a distinction a user can
                    // actually land on.
                    if !editor_image(&root) {
                        return Err(format!(
                            "That download is the game runtime, not the engine — it has \
                             {} but no editor image beside it. Use the <platform>.zip \
                             asset rather than renzora-runtime-<platform>.zip.",
                            engine_exe()
                        ));
                    }
                    return Ok(root);
                }
            }
            // Linux's engine asset is a single `.AppImage`, and the install may
            // still be a directory — a folder the user picked, or a checkout's
            // `dist/`. Install the extracted tree *as* that directory: the
            // sidecar swaps a directory for a directory perfectly well
            // (`install_new` renames, or recurses), so the target ends up
            // holding the AppImage.
            //
            // This used to be refused, on the reasoning that you cannot replace
            // a folder with a file. True, and beside the point — nobody asked to
            // replace the folder with a file, they asked for the new engine to
            // be in that folder. Refusing also made a *fresh empty directory*
            // an error, which is the most ordinary install target there is.
            if entries.iter().any(|p| ends_with(p, "AppImage")) {
                return Ok(staged.to_path_buf());
            }
            Err(format!(
                "The downloaded update doesn't contain {}.",
                engine_exe()
            ))
        }
    }
}

/// Name of the engine executable in a release tree.
///
/// **`renzora`, not `renzora-editor`.** There is one binary: it runs as the
/// editor when `renzora_editor.<dll|so|dylib>` sits beside it and as the shipped
/// game when it does not (CLAUDE.md §5). `renzora-editor` was a second
/// executable that existed only while Bevy was statically linked, and it has not
/// shipped since. Looking for it here is what made every flat-tree update fail
/// with "doesn't contain an editor binary" — the download was fine; the filename
/// was stale.
fn engine_exe() -> &'static str {
    if cfg!(windows) {
        "renzora.exe"
    } else {
        "renzora"
    }
}

/// The loadable editor image, whose presence is what makes a release tree the
/// *engine* rather than the game runtime.
fn editor_image(dir: &Path) -> bool {
    [
        "renzora_editor.dll",
        "renzora_editor.so",
        "librenzora_editor.so",
        "renzora_editor.dylib",
        "librenzora_editor.dylib",
    ]
    .iter()
    .any(|n| dir.join(n).is_file())
}

fn has_engine_binary(dir: &Path) -> bool {
    dir.join(engine_exe()).is_file()
}

/// The `.AppImage` directly inside `dir`, if that is what the payload is.
fn appimage_in(dir: &Path) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().and_then(|e| e.to_str()) == Some("AppImage"))
}

/// What to launch once the swap is done.
///
/// Cannot be settled in [`detect_layout`], which runs before anything is known
/// about the payload: a directory install normally relaunches the engine binary
/// inside it, but a directory that just received a Linux AppImage has to
/// relaunch *that* — there is no `renzora` in it to run. Deriving it from the
/// staged source is what makes "install the AppImage into a folder" actually
/// come back up afterwards instead of failing at the last step.
fn relaunch_after(staged_source: &Path, layout: &InstallLayout) -> PathBuf {
    // A single-file install, and a macOS bundle, both relaunch the thing that
    // was replaced.
    if layout.kind == InstallKind::File
        || layout.target.extension().and_then(|e| e.to_str()) == Some("app")
    {
        return layout.target.clone();
    }
    match staged_source.is_dir().then(|| appimage_in(staged_source)).flatten() {
        Some(img) => match img.file_name() {
            Some(name) => layout.target.join(name),
            None => layout.relaunch.clone(),
        },
        None => layout.relaunch.clone(),
    }
}

/// Make sure an extracted `.AppImage` is executable.
///
/// The zip records mode 0755 and `package-release.sh` restores it before
/// archiving for exactly this reason, but an archive that lost the bit anywhere
/// along the way would install fine and then fail to launch, which is a much
/// worse failure than a loud one here. Cheap insurance.
#[cfg(unix)]
fn ensure_appimage_executable(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Some(img) = appimage_in(dir) {
        if let Ok(meta) = fs::metadata(&img) {
            let mode = meta.permissions().mode();
            if mode & 0o111 == 0 {
                let _ = fs::set_permissions(&img, fs::Permissions::from_mode(mode | 0o755));
            }
        }
    }
}

#[cfg(not(unix))]
fn ensure_appimage_executable(_dir: &Path) {}

/// Start the sidecar and quit so it can do the swap.
///
/// Never returns on success — the process exits.
pub fn launch_sidecar(staged_source: &Path, layout: &InstallLayout) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("Cannot locate this executable: {e}"))?;
    let exe_dir = exe.parent().ok_or("This executable has no parent directory")?;
    let name = if cfg!(windows) {
        "renzora-update.exe"
    } else {
        "renzora-update"
    };

    let installed = exe_dir.join(name);
    if !installed.is_file() {
        return Err(format!(
            "The update helper ({name}) isn't next to the editor, so the update can't be \
             installed automatically. Download the new version from the release page."
        ));
    }

    // Run it from a COPY in the temp directory. Launched from the install folder
    // it would be holding an open handle inside the very directory it is about
    // to rename away — which Windows refuses outright, and which on any platform
    // means the updater deletes the file it is executing.
    let sidecar = std::env::temp_dir().join(format!("{}-{}", std::process::id(), name));
    fs::copy(&installed, &sidecar)
        .map_err(|e| format!("Cannot stage the update helper: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&sidecar, fs::Permissions::from_mode(0o755));
    }

    let log = staging_root("logs")
        .map(|p| p.join("last-update.log"))
        .unwrap_or_else(|| std::env::temp_dir().join("renzora-last-update.log"));

    let mut cmd = std::process::Command::new(&sidecar);
    cmd.arg("--staged")
        .arg(staged_source)
        .arg("--target")
        .arg(&layout.target)
        .arg("--relaunch")
        // Derived from what is actually being installed, not from the layout
        // alone — see `relaunch_after`.
        .arg(relaunch_after(staged_source, layout))
        .arg("--pid")
        .arg(std::process::id().to_string())
        .arg("--log")
        .arg(&log);

    cmd.spawn()
        .map_err(|e| format!("Could not start the update helper: {e}"))?;

    // The sidecar is waiting on this PID, so exiting IS the handoff. `exit`
    // rather than `AppExit` for the same reason the editor's own quit path uses
    // it: a full Bevy teardown can hang on a plugin's `FreeLibrary`, and an
    // editor that never dies is an update that never happens.
    std::process::exit(0);
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "renzora_update_{}_{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn staging_root_sanitises_the_tag() {
        // The tag comes from a network response and becomes a path component.
        let root = staging_root("../../etc/evil").unwrap();
        let last = root.file_name().unwrap().to_string_lossy().to_string();
        assert!(!last.contains('/') && !last.contains('\\'));
        assert_eq!(last, ".._.._etc_evil");
        // A real tag survives intact.
        let ok = staging_root("r1-alpha7-nightly-16aug26").unwrap();
        assert!(ok.ends_with("r1-alpha7-nightly-16aug26"));
    }

    /// Lay out a release tree the way the published `windows-x64.zip` actually
    /// is: the one binary plus the loadable editor image.
    ///
    /// These tests used to write `renzora-editor.exe`, which is why the stale
    /// filename in `resolve_staged_source` survived the editor becoming a
    /// loadable image — the suite asserted the bug rather than the release.
    fn write_engine_tree(dir: &Path) {
        fs::write(dir.join(engine_exe()), b"x").unwrap();
        fs::write(dir.join("renzora_editor.dll"), b"x").unwrap();
    }

    #[test]
    fn a_flat_windows_tree_is_its_own_source() {
        let dir = temp_dir("flat");
        write_engine_tree(&dir);
        assert_eq!(
            resolve_staged_source(&dir, InstallKind::Directory).unwrap(),
            dir
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_single_wrapping_folder_is_unwrapped() {
        let dir = temp_dir("wrapped");
        let inner = dir.join("renzora-engine");
        fs::create_dir_all(&inner).unwrap();
        write_engine_tree(&inner);
        assert_eq!(
            resolve_staged_source(&dir, InstallKind::Directory).unwrap(),
            inner
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The runtime asset (`renzora-runtime-<platform>.zip`) ships the binary with
    /// no editor image. Installing it would silently downgrade the editor to a
    /// game, so it is refused by name rather than accepted.
    #[test]
    fn a_runtime_only_tree_is_refused_by_name() {
        let dir = temp_dir("runtime-only");
        fs::write(dir.join(engine_exe()), b"x").unwrap();
        let err = resolve_staged_source(&dir, InstallKind::Directory).unwrap_err();
        assert!(err.contains("game runtime"), "{err}");
        fs::remove_dir_all(&dir).unwrap();
    }

    /// The reported bug: a Linux engine asset is a single `.AppImage`, and the
    /// install target is a directory — a folder the user picked, or a checkout's
    /// `dist/`. That installs: the extracted tree becomes the target directory,
    /// which then holds the AppImage.
    ///
    /// This was refused at first, which also made a fresh empty directory an
    /// error — the most ordinary install target there is.
    #[test]
    fn an_appimage_payload_installs_into_a_directory() {
        let dir = temp_dir("appimage-into-dir");
        fs::write(dir.join("Renzora Engine-x86_64.AppImage"), b"x").unwrap();
        assert_eq!(
            resolve_staged_source(&dir, InstallKind::Directory).unwrap(),
            dir
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    /// ...and the relaunch follows the payload, not the layout. A directory
    /// install normally relaunches the engine binary inside it; one that just
    /// received an AppImage has no such binary and must relaunch the image.
    #[test]
    fn relaunch_follows_an_appimage_into_a_directory() {
        let staged = temp_dir("relaunch-appimage");
        fs::write(staged.join("Renzora Engine-x86_64.AppImage"), b"x").unwrap();
        let target = std::env::temp_dir().join("renzora_update_target");
        let layout = InstallLayout {
            kind: InstallKind::Directory,
            target: target.clone(),
            relaunch: target.join(engine_exe()),
            is_source_checkout: false,
        };
        assert_eq!(
            relaunch_after(&staged, &layout),
            target.join("Renzora Engine-x86_64.AppImage")
        );

        // A flat engine tree keeps the layout's own answer.
        let flat = temp_dir("relaunch-flat");
        write_engine_tree(&flat);
        assert_eq!(relaunch_after(&flat, &layout), target.join(engine_exe()));

        fs::remove_dir_all(&staged).unwrap();
        fs::remove_dir_all(&flat).unwrap();
    }

    #[test]
    fn a_macos_bundle_is_found_inside_the_tree() {
        let dir = temp_dir("bundle");
        let app = dir.join("Renzora Engine.app");
        fs::create_dir_all(app.join("Contents").join("MacOS")).unwrap();
        assert_eq!(
            resolve_staged_source(&dir, InstallKind::Directory).unwrap(),
            app
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_appimage_install_wants_the_appimage() {
        let dir = temp_dir("appimage");
        let img = dir.join("Renzora Engine-x86_64.AppImage");
        fs::write(&img, b"x").unwrap();
        assert_eq!(resolve_staged_source(&dir, InstallKind::File).unwrap(), img);
        // ...and says so plainly when the release has none, rather than
        // installing something shaped wrong.
        let empty = temp_dir("appimage_empty");
        assert!(resolve_staged_source(&empty, InstallKind::File).is_err());
        fs::remove_dir_all(&dir).unwrap();
        fs::remove_dir_all(&empty).unwrap();
    }

    #[test]
    fn a_tree_with_no_editor_is_rejected() {
        let dir = temp_dir("noeditor");
        fs::write(dir.join("readme.txt"), b"x").unwrap();
        assert!(resolve_staged_source(&dir, InstallKind::Directory).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn source_checkout_detection_needs_all_three_markers() {
        let dir = temp_dir("checkout");
        assert!(!is_source_checkout(&dir));
        fs::write(dir.join("Cargo.toml"), b"").unwrap();
        assert!(!is_source_checkout(&dir));
        fs::create_dir_all(dir.join("crates")).unwrap();
        assert!(!is_source_checkout(&dir));
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("src").join("main.rs"), b"").unwrap();
        assert!(is_source_checkout(&dir));
        // ...and it looks upward, which is the case that matters: the editor
        // runs from `<checkout>/dist/<platform>/`.
        let nested = dir.join("dist").join("windows-x64");
        fs::create_dir_all(&nested).unwrap();
        assert!(is_source_checkout(&nested));
        fs::remove_dir_all(&dir).unwrap();
    }

    /// Retargeting has to re-derive, not carry over: the whole point of moving
    /// the install off a checkout is that the overwrite confirmation goes away.
    #[test]
    fn retargeting_re_derives_relaunch_and_checkout() {
        let dir = temp_dir("retarget");
        fs::write(dir.join("Cargo.toml"), b"").unwrap();
        fs::create_dir_all(dir.join("crates")).unwrap();
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("src").join("main.rs"), b"").unwrap();
        // Deliberately a sibling of the checkout, not a child: a child would
        // still walk up into it and the test would prove nothing.
        let elsewhere = std::env::temp_dir().join(format!(
            "renzora_update_retarget_elsewhere_{}",
            std::process::id()
        ));

        let checkout = InstallLayout {
            kind: InstallKind::Directory,
            target: dir.clone(),
            relaunch: dir.join(engine_exe()),
            is_source_checkout: true,
        };
        assert!(checkout.retargeted(dir.clone()).is_source_checkout);
        let moved = checkout.retargeted(elsewhere.clone());
        assert!(!moved.is_source_checkout);
        assert_eq!(moved.relaunch, elsewhere.join(engine_exe()));

        // A single-file install relaunches the file it just replaced, not a
        // binary "inside" it.
        let appimage = InstallLayout {
            kind: InstallKind::File,
            target: dir.join("Renzora.AppImage"),
            relaunch: dir.join("Renzora.AppImage"),
            is_source_checkout: false,
        };
        let to = elsewhere.join("Renzora.AppImage");
        assert_eq!(appimage.retargeted(to.clone()).relaunch, to);

        // ...and so does a macOS bundle, which is a directory that is still one
        // unit of install.
        let bundle = InstallLayout {
            kind: InstallKind::Directory,
            target: dir.join("Renzora.app"),
            relaunch: dir.join("Renzora.app"),
            is_source_checkout: false,
        };
        let to = elsewhere.join("Renzora.app");
        assert_eq!(bundle.retargeted(to.clone()).relaunch, to);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn hex_is_lowercase_and_padded() {
        assert_eq!(hex(&[0x00, 0x0f, 0xff]), "000fff");
    }
}
