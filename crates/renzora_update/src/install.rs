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

    let relaunch = exe_dir.join(if cfg!(windows) {
        "renzora-editor.exe"
    } else {
        "renzora-editor"
    });
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
            // Windows / extracted Linux: the flat tree itself. Some zip tools
            // produce a single wrapping folder, so unwrap one level if that is
            // what we got.
            if has_editor_binary(staged) {
                return Ok(staged.to_path_buf());
            }
            if let Some(only) = entries.iter().find(|p| p.is_dir()) {
                if has_editor_binary(only) {
                    return Ok(only.clone());
                }
            }
            Err("The downloaded update doesn't contain an editor binary.".to_string())
        }
    }
}

fn has_editor_binary(dir: &Path) -> bool {
    dir.join("renzora-editor").is_file() || dir.join("renzora-editor.exe").is_file()
}

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
        .arg(&layout.relaunch)
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

    #[test]
    fn a_flat_windows_tree_is_its_own_source() {
        let dir = temp_dir("flat");
        fs::write(dir.join("renzora-editor.exe"), b"x").unwrap();
        fs::write(dir.join("renzora.exe"), b"x").unwrap();
        assert_eq!(
            resolve_staged_source(&dir, InstallKind::Directory).unwrap(),
            dir
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_single_wrapping_folder_is_unwrapped() {
        let dir = temp_dir("wrapped");
        let inner = dir.join("renzora");
        fs::create_dir_all(&inner).unwrap();
        fs::write(inner.join("renzora-editor"), b"x").unwrap();
        assert_eq!(
            resolve_staged_source(&dir, InstallKind::Directory).unwrap(),
            inner
        );
        fs::remove_dir_all(&dir).unwrap();
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

    #[test]
    fn hex_is_lowercase_and_padded() {
        assert_eq!(hex(&[0x00, 0x0f, 0xff]), "000fff");
    }
}
