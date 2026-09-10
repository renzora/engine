//! Renzora update sidecar.
//!
//! A running executable cannot replace itself, so the editor hands the last step
//! of an update to this separate process: wait for the editor to exit, swap the
//! installed engine for the staged one, put the old one back if anything goes
//! wrong, and relaunch.
//!
//! ```text
//! renzora-update --staged <path> --target <path> --pid <n> --relaunch <path>
//!                [--log <path>]
//! ```
//!
//! # What "the engine" is, per platform
//!
//! The previous updater replaced a single `.exe`. That stopped being what an
//! install *is*: the engine ships two executables (`renzora-editor` and
//! `renzora`), the C-ABI plugins under `plugins/`, and the OpenXR loader — and
//! on Linux it is one `.AppImage`, on macOS one `.app` bundle. So the swap has
//! two shapes, chosen from what `--staged` points at rather than from a platform
//! flag:
//!
//! * **`--staged` is a directory** — a Windows install folder or a macOS `.app`.
//!   The whole directory is replaced.
//! * **`--staged` is a file** — a Linux `.AppImage`. That one file is replaced.
//!
//! # Failure is the case that matters
//!
//! An update that fails halfway leaves someone with no engine and no process to
//! repair it, so the swap is always: move the current one aside (a *sibling*
//! rename, which is same-volume by construction and therefore atomic), install
//! the new one, and only then delete the backup. Any failure after the rename
//! restores the backup before reporting. The user's worst case is "the update
//! didn't happen", never "the engine is gone".
//!
//! # This binary must not depend on the directory it replaces
//!
//! It is launched from a **copy in a temp directory**, not from the install
//! folder, so it isn't holding a handle inside the tree it is deleting. Its
//! `.cargo/config.toml` also switches off the `prefer-dynamic` it would
//! otherwise inherit from the engine, which would make it import a
//! `std-<hash>.dll` living in that same doomed directory. Both halves are
//! required; either one alone still breaks.

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

/// How long to keep retrying the "move the current install aside" rename.
///
/// On Windows a directory rename fails while anything holds a handle inside it,
/// and the editor's own process exiting does not mean every handle is gone —
/// antivirus scanners and Explorer preview handlers routinely hold one for a
/// second or two afterwards. Failing immediately here would turn a routine race
/// into "update failed, try again".
const RENAME_RETRIES: u32 = 40;
const RENAME_RETRY_DELAY: Duration = Duration::from_millis(250);

struct Args {
    staged: PathBuf,
    target: PathBuf,
    pid: u32,
    relaunch: PathBuf,
    log: Option<PathBuf>,
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            report(None, &format!("Bad arguments: {e}"));
            std::process::exit(2);
        }
    };
    let log = args.log.clone();
    match perform_update(&args) {
        Ok(()) => {
            note(log.as_deref(), "update complete");
        }
        Err(e) => {
            report(
                log.as_deref(),
                &format!("Update failed: {e}\n\nYour existing Renzora install was restored."),
            );
            std::process::exit(1);
        }
    }
}

fn parse_args() -> Result<Args, String> {
    let mut staged = None;
    let mut target = None;
    let mut pid = None;
    let mut relaunch = None;
    let mut log = None;

    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < argv.len() {
        let key = argv[i].as_str();
        let val = argv.get(i + 1).ok_or_else(|| format!("{key} needs a value"))?;
        match key {
            "--staged" => staged = Some(PathBuf::from(val)),
            "--target" => target = Some(PathBuf::from(val)),
            "--relaunch" => relaunch = Some(PathBuf::from(val)),
            "--log" => log = Some(PathBuf::from(val)),
            "--pid" => pid = Some(val.parse::<u32>().map_err(|_| "--pid must be a number")?),
            other => return Err(format!("unknown argument {other}")),
        }
        i += 2;
    }

    Ok(Args {
        staged: staged.ok_or("missing --staged")?,
        target: target.ok_or("missing --target")?,
        pid: pid.ok_or("missing --pid")?,
        relaunch: relaunch.ok_or("missing --relaunch")?,
        log,
    })
}

fn perform_update(args: &Args) -> Result<(), String> {
    let log = args.log.as_deref();
    note(log, &format!("waiting for editor pid {}", args.pid));
    wait_for_process_exit(args.pid);

    if !args.staged.exists() {
        return Err(format!("staged update missing at {}", args.staged.display()));
    }

    // Move the current install aside. A SIBLING path, so the rename stays on one
    // volume and is therefore cheap and atomic — `~/.renzora/updates` is very
    // often a different drive from the install, and a cross-volume "rename" would
    // silently become a slow copy at exactly the wrong moment.
    // ── Do we own the place we are writing to? ──────────────────────────────
    // An editor dragged into `/Applications` by an administrator can usually be
    // replaced without asking for anything: the directory is group-writable by
    // `admin`, and most Macs have exactly one user, who is an admin. That is the
    // path taken here, silently, and it is the common one.
    //
    // A standard (non-admin) user is the exception. They needed an admin
    // password to install the editor and they need one to replace it, so the
    // swap is handed to `osascript`, which puts up the system authentication
    // dialog and runs it as root.
    //
    // Checked BEFORE touching anything rather than as a retry: the first step of
    // the swap is destructive (it renames the install aside), so discovering the
    // permission problem halfway would mean unwinding rather than never having
    // started. `access(2)` answers exactly the question — can *this* process
    // write here — where inspecting mode bits and group membership is a
    // reimplementation of the kernel's own check.
    #[cfg(target_os = "macos")]
    if !parent_is_writable(&args.target) {
        note(log, "install directory is not writable; asking for authorization");
        return elevated_swap(args, log);
    }

    let backup = backup_path(&args.target);
    let _ = remove_any(&backup);

    let had_previous = args.target.exists();
    if had_previous {
        note(log, &format!("moving {} aside", args.target.display()));
        rename_with_retry(&args.target, &backup)?;
    }

    note(log, &format!("installing into {}", args.target.display()));
    match install(&args.staged, &args.target) {
        Ok(()) => {}
        Err(e) => {
            // Put it back exactly as it was, then report. "The update didn't
            // happen" is a recoverable outcome; "the engine is gone" is not.
            let _ = remove_any(&args.target);
            if had_previous {
                let _ = fs::rename(&backup, &args.target);
            }
            return Err(e);
        }
    }

    let _ = remove_any(&backup);
    let _ = remove_any(&args.staged);

    note(log, &format!("relaunching {}", args.relaunch.display()));
    relaunch(&args.relaunch)
}

/// Can this process create and remove entries in `path`'s directory?
///
/// The swap replaces the bundle as a whole, so what matters is the *parent* —
/// `/Applications` — not the bundle's own mode.
#[cfg(target_os = "macos")]
fn parent_is_writable(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;

    let Some(parent) = path.parent() else { return false };
    let Ok(c) = std::ffi::CString::new(parent.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: `c` is a valid NUL-terminated string for the duration of the call.
    unsafe { libc::access(c.as_ptr(), libc::W_OK) == 0 }
}

/// The whole swap, as one shell script, run as root behind the system
/// authentication dialog.
///
/// One script rather than a series of elevated calls: each `do shell script …
/// with administrator privileges` is its own prompt, and asking somebody for
/// their password four times to install one update is how people learn to type
/// it without reading. It also keeps the swap atomic in the same way the
/// unprivileged path is — the rename happens inside the same root shell that
/// puts the new bundle in place.
///
/// # Ownership
///
/// `ditto` run as root produces a `root`-owned bundle. Left that way, the *next*
/// update would need a password too, even for an admin user who would otherwise
/// never see one — so the new bundle is chowned back to whoever owned the old
/// one. `$SUDO_USER` is not available here (this is not sudo), which is why the
/// uid and gid are read off the existing install before it moves.
#[cfg(target_os = "macos")]
fn elevated_swap(args: &Args, log: Option<&Path>) -> Result<(), String> {
    use std::os::unix::fs::MetadataExt;

    let backup = backup_path(&args.target);
    // Whoever owns the current install owns the new one. Falling back to the
    // real uid covers a first install, where there is nothing to read.
    let (uid, gid) = fs::metadata(&args.target)
        .map(|m| (m.uid(), m.gid()))
        // SAFETY: getuid/getgid cannot fail and take no arguments.
        .unwrap_or_else(|_| unsafe { (libc::getuid(), libc::getgid()) });

    let script = swap_script(&args.staged, &args.target, &backup, uid, gid);

    note(log, "requesting administrator authorization");
    let out = std::process::Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(format!(
            "do shell script {} with prompt \"Renzora Engine needs to update the app in your Applications folder.\" with administrator privileges",
            as_quote(&script)
        ))
        .output()
        .map_err(|e| format!("cannot run osascript: {e}"))?;

    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        // -128 is AppleScript's "user cancelled". Worth naming, because it is
        // not a failure of the update so much as an answer to it, and the
        // message a user sees should say what to do next.
        if err.contains("-128") || err.contains("User canceled") {
            return Err(
                "The update needs permission to replace the app in Applications. \
                 Nothing was changed — run the update again and authorize it, or \
                 drag the new version in yourself."
                    .to_string(),
            );
        }
        return Err(format!("Could not install the update: {}", err.trim()));
    }

    let _ = remove_any(&args.staged);
    note(log, "elevated swap complete");
    Ok(())
}

/// The shell script the elevated swap runs, as a string.
///
/// Split out so it can be checked without a password prompt — a syntax error in
/// here would only ever surface on somebody else's machine, behind an
/// authentication dialog, after their editor had already quit.
///
/// Absolute paths for every command: this runs as root, and inheriting a `PATH`
/// from the user's environment is not something a root shell should do.
#[cfg(target_os = "macos")]
fn swap_script(staged: &Path, target: &Path, backup: &Path, uid: u32, gid: u32) -> String {
    let target = sh_quote(target);
    let staged = sh_quote(staged);
    let backup = sh_quote(backup);
    // `set -e` so a failed `mv` cannot be followed by a `ditto` into a path that
    // still holds the old install. The `||` block is the unwind: put the old
    // bundle back before giving up, so a failure leaves a working editor.
    format!(
        "set -e; \
         /bin/rm -rf {backup}; \
         if [ -e {target} ]; then /bin/mv {target} {backup}; fi; \
         /usr/bin/ditto {staged} {target} || {{ /bin/rm -rf {target}; \
         if [ -e {backup} ]; then /bin/mv {backup} {target}; fi; exit 1; }}; \
         /usr/sbin/chown -R {uid}:{gid} {target}; \
         /bin/rm -rf {backup}"
    )
}

/// Wrap a path for a POSIX shell: single-quote it, and close/escape/reopen for
/// any single quote inside.
///
/// The bundle is called `Renzora Engine.app`, so this is exercised on every
/// elevated update rather than being a rare edge.
#[cfg(target_os = "macos")]
fn sh_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', r"'\''"))
}

/// Wrap a string as an AppleScript literal.
///
/// The script goes through two parsers — AppleScript first, then the shell — so
/// it is quoted twice. Backslashes before quotes, and in that order, or an
/// escaped backslash would eat the escape of the quote after it.
#[cfg(target_os = "macos")]
fn as_quote(script: &str) -> String {
    format!("\"{}\"", script.replace('\\', r"\\").replace('"', "\\\""))
}

/// `<name>.renzora-backup` beside the target.
fn backup_path(target: &Path) -> PathBuf {
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "renzora".to_string());
    target.with_file_name(format!("{name}.renzora-backup"))
}

/// Install the staged engine at `target`.
///
/// A rename is tried first because it is instant when the staging directory
/// happens to be on the same volume; the recursive copy is the general case.
fn install(staged: &Path, target: &Path) -> Result<(), String> {
    if fs::rename(staged, target).is_ok() {
        return Ok(());
    }
    if staged.is_dir() {
        // macOS takes `ditto` rather than the recursive copy below, and the
        // difference is not cosmetic. Part of a code signature lives in extended
        // attributes, and `fs::copy` does not carry them — so a bundle that
        // reached this path (staging and install on different volumes, which is
        // the normal case for `~/.renzora/updates` against `/Applications`)
        // would arrive with a signature that no longer verifies. macOS then
        // refuses to launch it as "damaged", after the old copy has already been
        // deleted. `ditto` is the copy that preserves them.
        #[cfg(target_os = "macos")]
        return ditto(staged, target);
        #[cfg(not(target_os = "macos"))]
        copy_dir(staged, target)
    } else {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        fs::copy(staged, target).map_err(|e| format!("{}: {e}", target.display()))?;
        copy_exec_bit(staged, target);
        Ok(())
    }
}

/// Copy a bundle preserving everything macOS keeps outside the file contents —
/// extended attributes, resource forks, ACLs.
///
/// `ditto` is in `/usr/bin` on every macOS install; it is part of the base
/// system, not the developer tools, so this does not require Xcode.
#[cfg(target_os = "macos")]
fn ditto(src: &Path, dst: &Path) -> Result<(), String> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let out = std::process::Command::new("/usr/bin/ditto")
        .arg(src)
        .arg(dst)
        .output()
        .map_err(|e| format!("cannot run ditto: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "ditto failed copying to {}: {}",
            dst.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

#[cfg_attr(target_os = "macos", allow(dead_code))]
fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    for entry in fs::read_dir(src).map_err(|e| format!("{}: {e}", src.display()))? {
        let entry = entry.map_err(|e| format!("{}: {e}", src.display()))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let ty = entry
            .file_type()
            .map_err(|e| format!("{}: {e}", from.display()))?;
        if ty.is_dir() {
            copy_dir(&from, &to)?;
        } else if ty.is_symlink() {
            copy_symlink(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|e| format!("{}: {e}", to.display()))?;
            copy_exec_bit(&from, &to);
        }
    }
    Ok(())
}

/// Recreate a symlink rather than following it.
///
/// macOS `.app` bundles are full of them (`Contents/Frameworks`, `Versions/A`),
/// and following them would both bloat the install and break the bundle's code
/// signature, which seals the layout as well as the bytes.
#[cfg(unix)]
fn copy_symlink(from: &Path, to: &Path) -> Result<(), String> {
    let dest = fs::read_link(from).map_err(|e| format!("{}: {e}", from.display()))?;
    let _ = fs::remove_file(to);
    std::os::unix::fs::symlink(&dest, to).map_err(|e| format!("{}: {e}", to.display()))
}

#[cfg(not(unix))]
fn copy_symlink(from: &Path, to: &Path) -> Result<(), String> {
    // Windows symlinks need a privilege the editor may not have, and nothing in
    // a Windows engine tree is one — copy the contents.
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| format!("{}: {e}", to.display()))
}

#[cfg(unix)]
fn copy_exec_bit(from: &Path, to: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = fs::metadata(from) {
        let mode = meta.permissions().mode();
        let _ = fs::set_permissions(to, fs::Permissions::from_mode(mode));
    }
}

#[cfg(not(unix))]
fn copy_exec_bit(_from: &Path, _to: &Path) {}

fn remove_any(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else if path.exists() {
        fs::remove_file(path)
    } else {
        Ok(())
    }
}

fn rename_with_retry(from: &Path, to: &Path) -> Result<(), String> {
    let mut last = String::new();
    for _ in 0..RENAME_RETRIES {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last = e.to_string();
                thread::sleep(RENAME_RETRY_DELAY);
            }
        }
    }
    Err(format!(
        "could not move {} aside after {}s ({last}) — something still has it open",
        from.display(),
        RENAME_RETRIES * RENAME_RETRY_DELAY.as_millis() as u32 / 1000
    ))
}

// ── Waiting for the editor ───────────────────────────────────────────────────

/// Block until the editor process is gone.
///
/// Deliberately infallible: if the process cannot be found, it has already
/// exited, which is the state we were waiting for. The only way this is wrong is
/// if the PID were reused within the handful of milliseconds between the editor
/// spawning us and calling `exit`, which no OS does.
#[cfg(windows)]
fn wait_for_process_exit(pid: u32) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, INFINITE, PROCESS_SYNCHRONIZE,
    };
    unsafe {
        let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if handle.is_null() {
            return;
        }
        WaitForSingleObject(handle, INFINITE);
        CloseHandle(handle);
    }
    // Handles outlive the process briefly; give the OS a moment to release the
    // file locks before trying to rename the directory out from under them.
    thread::sleep(Duration::from_millis(300));
}

/// `kill(pid, 0)` — the portable "does this process exist" probe.
///
/// The previous updater polled `/proc/<pid>`, which macOS does not have, so it
/// simply timed out after 30 seconds and swapped the tree while the editor was
/// very possibly still running.
#[cfg(unix)]
fn wait_for_process_exit(pid: u32) {
    // 5 minutes. A bound rather than `INFINITE` because unlike the Windows path
    // this is a poll, and a wrong PID would otherwise hang forever with no UI.
    for _ in 0..3000 {
        if unsafe { libc::kill(pid as libc::pid_t, 0) } != 0 {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    thread::sleep(Duration::from_millis(300));
}

// ── Relaunch ─────────────────────────────────────────────────────────────────

fn relaunch(path: &Path) -> Result<(), String> {
    // A macOS `.app` is a directory, not something to execute. `open` is what
    // launches a bundle with the right activation and working directory.
    if path.extension().and_then(|e| e.to_str()) == Some("app") {
        return Command::new("open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("failed to relaunch {}: {e}", path.display()));
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS: the editor must not die when this process exits.
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        Command::new(path)
            .creation_flags(DETACHED_PROCESS)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("failed to relaunch {}: {e}", path.display()))
    }
    #[cfg(not(windows))]
    {
        Command::new(path)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("failed to relaunch {}: {e}", path.display()))
    }
}

// ── Reporting ────────────────────────────────────────────────────────────────

fn note(log: Option<&Path>, message: &str) {
    if let Some(path) = log {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{message}");
        }
    }
}

/// Tell the user something went wrong.
///
/// This process has no window and the editor it would have reported through is
/// gone, so on Windows the only thing that reaches a user is a message box. The
/// log is always written regardless, because that is what a bug report can
/// actually include.
fn report(log: Option<&Path>, message: &str) {
    note(log, message);

    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::iter::once;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

        let wide = |s: &str| -> Vec<u16> { OsStr::new(s).encode_wide().chain(once(0)).collect() };
        let title = wide("Renzora Update");
        let body = wide(message);
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                body.as_ptr(),
                title.as_ptr(),
                MB_ICONERROR | MB_OK,
            );
        }
    }
    #[cfg(not(windows))]
    {
        eprintln!("{message}");
    }
}

#[cfg(all(test, target_os = "macos"))]
mod macos_tests {
    use super::*;

    /// The bundle is called `Renzora Engine.app`, so an unquoted path splits
    /// into two arguments and the swap moves something that does not exist.
    #[test]
    fn shell_quoting_survives_spaces() {
        let q = sh_quote(Path::new("/Applications/Renzora Engine.app"));
        assert_eq!(q, "'/Applications/Renzora Engine.app'");
    }

    /// A single quote cannot be escaped inside single quotes, so it has to close
    /// them, escape itself, and reopen.
    #[test]
    fn shell_quoting_survives_a_quote() {
        let q = sh_quote(Path::new("/Users/o'brien/App.app"));
        assert_eq!(q, r"'/Users/o'\''brien/App.app'");
    }

    /// Backslash before quote, in that order — reversed, the escape added for a
    /// quote would itself be escaped and the string would end early.
    #[test]
    fn applescript_quoting_escapes_both() {
        assert_eq!(as_quote(r#"a"b"#), r#""a\"b""#);
        assert_eq!(as_quote(r"a\b"), r#""a\\b""#);
        assert_eq!(as_quote(r#"a\"b"#), r#""a\\\"b""#);
    }

    /// The generated script must be valid shell. A syntax error here would
    /// only appear on a user's machine, behind a password dialog, after their
    /// editor had quit — the worst possible place to find one.
    #[test]
    fn the_swap_script_is_valid_shell() {
        let script = swap_script(
            Path::new("/Users/a b/.renzora/updates/r1/staged/Renzora Engine.app"),
            Path::new("/Applications/Renzora Engine.app"),
            Path::new("/Applications/Renzora Engine.app.renzora-backup"),
            501,
            20,
        );
        let out = std::process::Command::new("bash")
            .arg("-n")
            .arg("-c")
            .arg(&script)
            .output()
            .expect("run bash -n");
        assert!(
            out.status.success(),
            "generated script is not valid shell:\n{script}\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// It must also survive AppleScript's parser on the way to that shell.
    #[test]
    fn the_swap_script_survives_applescript() {
        let script = swap_script(
            Path::new("/tmp/staged/Renzora Engine.app"),
            Path::new("/Applications/Renzora Engine.app"),
            Path::new("/Applications/Renzora Engine.app.renzora-backup"),
            501,
            20,
        );
        // Compile the AppleScript without running it: `-e` plus a syntax check
        // via `osadecompile` is awkward, so instead wrap it in a false branch —
        // AppleScript still parses the whole thing, but nothing executes.
        let wrapped = format!("if false then\ndo shell script {}\nend if", as_quote(&script));
        let out = std::process::Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg(&wrapped)
            .output()
            .expect("run osascript");
        assert!(
            out.status.success(),
            "AppleScript rejected the command: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// The real thing: build the same double-quoted command the elevated swap
    /// builds, and run it through `osascript` — without `with administrator
    /// privileges`, so it needs no password — to prove both parsers accept it
    /// and the path arrives in one piece.
    #[test]
    fn a_spacey_path_survives_both_parsers() {
        let path = Path::new("/tmp/Renzora Engine.app");
        let script = format!("echo {}", sh_quote(path));
        let out = std::process::Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg(format!("do shell script {}", as_quote(&script)))
            .output()
            .expect("run osascript");
        assert!(
            out.status.success(),
            "osascript rejected the command: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            "/tmp/Renzora Engine.app"
        );
    }
}
