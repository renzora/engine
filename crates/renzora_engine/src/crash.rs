//! Crash reporting and error handling
//!
//! Provides a panic hook that captures crash information, saves it to disk,
//! and shows a native dialog. On next startup, the editor displays the
//! previous crash report in an egui window.

use std::backtrace::Backtrace;
use std::panic;
use std::sync::Mutex;

use bevy::prelude::*;

/// Global crash report storage
static CRASH_REPORT: Mutex<Option<CrashReport>> = Mutex::new(None);

/// Whether this process is an editor session. Set once by [`install_panic_hook`]
/// from `main` (which already accounts for `--no-editor` and server launches).
/// The panic hook and the crash-file helpers run outside the Bevy `World` (the
/// hook runs in the panic handler itself), so they can't read `EditorSession`
/// — they consult this process-global instead. Editor crashes overwrite
/// `~/.renzora/crashes/last_crash.txt` (picked up + shown next launch); game
/// crashes append `<exe_dir>/crash.log` beside the shipped binary.
static IS_EDITOR_PROCESS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn is_editor_process() -> bool {
    IS_EDITOR_PROCESS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Crash report information
#[derive(Clone, Debug)]
pub struct CrashReport {
    pub message: String,
    pub location: String,
    pub backtrace: String,
    pub timestamp: String,
}

impl CrashReport {
    /// Format the crash report for display/copying
    pub fn format(&self) -> String {
        format!(
            "=== CRASH REPORT ===\n\
             Timestamp: {}\n\n\
             Error: {}\n\
             Location: {}\n\n\
             === BACKTRACE ===\n\
             {}\n\
             === END CRASH REPORT ===",
            self.timestamp, self.message, self.location, self.backtrace
        )
    }
}

/// Scrub build-machine paths out of panic text before it's stored, shown, or
/// written to disk. Panic `Location`s are compiled into the binary with the
/// source paths of the machine that BUILT it — a shipped release would
/// otherwise show players absolute paths from the release machine, home
/// directory and username included (GH issue #67 reported a crash dialog
/// printing `C:\Users\<dev>\.cargo\registry\...\bevy_ecs-0.19.0\...`).
/// Canonical container builds already remap these at compile time
/// (`--remap-path-prefix` in `.cargo/config.toml`); this is the display-side
/// backstop for host-built binaries and any path the remap misses.
fn sanitize_paths(s: &str) -> String {
    let mut out = s.to_string();

    // `C:\Users\<name>\...` / `/home/<name>/...` / `/Users/<name>/...` → `~/...`
    // — drop the home prefix and the username segment. This runs FIRST so a
    // username containing spaces can't confuse the registry pass's
    // walk-back-to-whitespace below.
    for pat in ["C:\\Users\\", "C:/Users/", "c:\\users\\", "/home/", "/Users/"] {
        while let Some(hit) = out.find(pat) {
            let after_pat = hit + pat.len();
            match out[after_pat..].find(['\\', '/']) {
                Some(sep) => out.replace_range(hit..after_pat + sep, "~"),
                // Path ends inside the username (e.g. a bare `/home/name`).
                None => out.replace_range(hit.., "~"),
            }
        }
    }

    // `<cargo home>/registry/src/<index.crates.io-hash>/bevy_ecs-0.19.0/...`
    // → `<registry>/bevy_ecs-0.19.0/...`. Everything before the crate name is
    // the builder's machine, not information.
    for pat in ["registry\\src\\", "registry/src/"] {
        while let Some(hit) = out.find(pat) {
            // Walk back to the start of the path (best effort: to the last
            // whitespace/quote/paren).
            let start = out[..hit]
                .rfind([' ', '\t', '"', '\'', '('])
                .map(|i| i + 1)
                .unwrap_or(0);
            // Skip the registry index segment (`index.crates.io-<hash>`).
            let after_pat = hit + pat.len();
            let Some(sep) = out[after_pat..].find(['\\', '/']) else {
                break; // no crate segment follows; leave it rather than loop
            };
            out.replace_range(start..after_pat + sep + 1, "<registry>/");
        }
    }

    out
}

/// Install the custom panic hook. Call this before `app.run()`. `is_editor`
/// records whether this is an editor session so the hook can pick the right
/// crash-file location + dialog behaviour (it can't read the Bevy `World`).
pub fn install_panic_hook(is_editor: bool) {
    IS_EDITOR_PROCESS.store(is_editor, std::sync::atomic::Ordering::Relaxed);
    let default_hook = panic::take_hook();

    panic::set_hook(Box::new(move |panic_info| {
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };

        let location = if let Some(loc) = panic_info.location() {
            format!("{}:{}:{}", loc.file(), loc.line(), loc.column())
        } else {
            "Unknown location".to_string()
        };

        let backtrace = Backtrace::force_capture();
        let backtrace_str = format!("{backtrace}");

        let timestamp = chrono_lite_timestamp();

        let report = CrashReport {
            message: sanitize_paths(&message),
            location: sanitize_paths(&location),
            backtrace: sanitize_paths(&backtrace_str),
            timestamp,
        };

        if let Ok(mut guard) = CRASH_REPORT.lock() {
            *guard = Some(report.clone());
        }

        let _ = save_crash_report(&report);

        // Editor sessions get a native dialog; shipped games write crash.log
        // silently. `is_editor_process()` carries the decision out of `main`.
        #[cfg(all(
            feature = "crash_dialog",
            not(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))
        ))]
        {
            if is_editor_process() {
                show_crash_dialog(&report);
            }
        }

        default_hook(panic_info);
    }));
}

/// Simple timestamp without external dependency
fn chrono_lite_timestamp() -> String {
    // `web_time`'s SystemTime on the web, `std`'s everywhere else. std's panics
    // on wasm ("time not implemented on this platform"), and panicking inside
    // the crash reporter is the worst possible place for it — the report that
    // explains the first failure would itself be the second one.
    #[cfg(target_arch = "wasm32")]
    use web_time::{SystemTime, UNIX_EPOCH};
    #[cfg(not(target_arch = "wasm32"))]
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let secs = duration.as_secs();

    let days = secs / 86400;
    let remaining = secs % 86400;
    let hours = remaining / 3600;
    let minutes = (remaining % 3600) / 60;
    let seconds = remaining % 60;

    let (year, month, day) = civil_from_days(days as i64);

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        year, month, day, hours, minutes, seconds
    )
}

/// Days-since-epoch → (year, month, day), Gregorian.
///
/// The previous version approximated with 365-day years and 30-day months,
/// which by 2026 put crash reports ~17 days in the future (14 leap days since
/// 1970 plus month-length drift) — bad for a file whose whole job is telling
/// you *when* something broke. This is Howard Hinnant's exact
/// `civil_from_days` algorithm, still dependency-free for the same reason the
/// caller is: the crash reporter must not pull in anything that can fail.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11], March-based
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Save the crash report to a file.
///
/// In the editor build the report is overwritten in `~/.renzora/crashes/last_crash.txt`
/// so the next editor launch can pick it up and show its dialog. In the runtime
/// build the report is appended to `<exe_dir>/crash.log` next to the shipped
/// binary, where players can find it without hunting through their home dir.
fn save_crash_report(report: &CrashReport) -> std::io::Result<()> {
    let crash_dir = get_crash_dir();
    std::fs::create_dir_all(&crash_dir)?;

    if is_editor_process() {
        // Editor: overwrite so the next editor launch shows just the latest.
        let crash_file = crash_dir.join("last_crash.txt");
        std::fs::write(&crash_file, report.format())?;
    } else {
        // Game: append beside the shipped binary where players can find it.
        use std::io::Write as _;
        let crash_file = crash_dir.join("crash.log");
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&crash_file)?;
        writeln!(f, "{}\n", report.format())?;
    }

    Ok(())
}

/// Get the crash report directory.
///
/// Editor builds keep history under the user's home dir; runtime builds drop
/// the file next to the executable so it ships with the game directory.
fn get_crash_dir() -> std::path::PathBuf {
    #[cfg(target_arch = "wasm32")]
    {
        std::path::PathBuf::from(".renzora/crashes")
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        if is_editor_process() {
            // Editor keeps history under the user's home dir.
            if let Some(home) = dirs::home_dir() {
                return home.join(".renzora").join("crashes");
            }
            return std::path::PathBuf::from(".renzora/crashes");
        }
        // Game drops the file next to the executable so it ships with the
        // game directory.
        if let Some(exe_dir) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        {
            return exe_dir;
        }
        std::path::PathBuf::from(".")
    }
}

/// Check if there's a crash report from a previous session
pub fn check_previous_crash() -> Option<CrashReport> {
    let crash_file = get_crash_dir().join("last_crash.txt");

    if !crash_file.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&crash_file).ok()?;

    let mut message = String::new();
    let mut location = String::new();
    let mut backtrace = String::new();
    let mut timestamp = String::new();
    let mut in_backtrace = false;

    for line in content.lines() {
        if let Some(ts) = line.strip_prefix("Timestamp: ") {
            timestamp = ts.to_string();
        } else if let Some(err) = line.strip_prefix("Error: ") {
            message = err.to_string();
        } else if let Some(loc) = line.strip_prefix("Location: ") {
            location = loc.to_string();
        } else if line == "=== BACKTRACE ===" {
            in_backtrace = true;
        } else if line == "=== END CRASH REPORT ===" {
            in_backtrace = false;
        } else if in_backtrace {
            backtrace.push_str(line);
            backtrace.push('\n');
        }
    }

    // Delete the crash file after reading
    let _ = std::fs::remove_file(&crash_file);

    if message.is_empty() {
        return None;
    }

    Some(CrashReport {
        message,
        location,
        backtrace,
        timestamp,
    })
}

/// Show a native crash dialog using rfd, with option to copy to clipboard.
/// Called only for editor sessions (gated by `is_editor_process()` at the call
/// site); shipped games write `crash.log` silently instead. Compiled out
/// entirely without `crash_dialog`, which is what keeps `rfd` and `arboard` out
/// of a lean export.
#[cfg(all(
    feature = "crash_dialog",
    not(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))
))]
fn show_crash_dialog(report: &CrashReport) {
    let short_message = format!(
        "The application has crashed.\n\n\
         Error: {}\n\
         Location: {}\n\n\
         A crash report has been saved to:\n\
         {}\n\n\
         Would you like to copy the full crash report to clipboard?",
        report.message,
        report.location,
        get_crash_dir().join("last_crash.txt").display()
    );

    let result = rfd::MessageDialog::new()
        .set_title("Renzora Engine - Crash Report")
        .set_description(&short_message)
        .set_level(rfd::MessageLevel::Error)
        .set_buttons(rfd::MessageButtons::YesNo)
        .show();

    if result == rfd::MessageDialogResult::Yes {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(report.format());
        }
    }
}

// =============================================================================
// Crash-report state + plugin (runtime). The native (ember) overlay that shows
// a previous-session crash while in the editor lives in `renzora_engine_editor`
// (it needs `renzora_ember`); it reads `CrashReportWindowState` from here.
// =============================================================================

/// Previous-session crash surfaced to the UI. Set by `check_for_previous_crash_system`
/// (runtime, here); read + cleared by the editor crash overlay.
#[derive(Resource, Default)]
pub struct CrashReportWindowState {
    pub show_window: bool,
    pub report: Option<CrashReport>,
}

/// Installs crash reporting: inits `CrashReportWindowState` and the startup
/// check for a previous crash. The editor's `EngineEditorPlugin` adds the
/// overlay that renders it.
pub struct CrashReportPlugin;

impl Plugin for CrashReportPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] CrashReportPlugin");
        app.init_resource::<CrashReportWindowState>()
            .add_systems(Startup, check_for_previous_crash_system);
    }
}

/// System to check for previous crash on startup
fn check_for_previous_crash_system(mut state: ResMut<CrashReportWindowState>) {
    if let Some(report) = check_previous_crash() {
        warn!("Previous session crashed: {}", report.message);
        state.report = Some(report);
        state.show_window = true;
    }
}

#[cfg(test)]
mod tests {
    use super::{civil_from_days, sanitize_paths};

    #[test]
    fn civil_dates_are_exact() {
        // Day numbers cross-checked against Python's datetime. The leap-year
        // cases are the ones the old 365/30 approximation got wrong.
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(10956), (1999, 12, 31));
        assert_eq!(civil_from_days(11016), (2000, 2, 29)); // 400-year leap rule
        assert_eq!(civil_from_days(19782), (2024, 2, 29));
        assert_eq!(civil_from_days(20695), (2026, 8, 30));
        assert_eq!(civil_from_days(20818), (2026, 12, 31));
    }

    #[test]
    fn scrubs_home_dirs() {
        assert_eq!(
            sanitize_paths("at /home/dev/projects/game/src/main.rs:1:1"),
            "at ~/projects/game/src/main.rs:1:1"
        );
        assert_eq!(
            sanitize_paths(r"at C:\Users\Some Name\game\src\main.rs:1:1"),
            r"at ~\game\src\main.rs:1:1"
        );
    }
}
