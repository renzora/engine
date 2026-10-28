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

pub fn is_editor_process() -> bool {
    IS_EDITOR_PROCESS.load(std::sync::atomic::Ordering::Relaxed)
}

// ── Panics Bevy catches ──────────────────────────────────────────────────────
//
// Since Bevy 0.20 the schedule executor catches a panic in a system, command or
// observer and hands it to the world's `FallbackErrorHandler` as an error. The
// default handler panics again, so nothing changed by itself; the editor installs
// [`editor_error_handler`], which logs it to the Console and carries on, so one
// broken plugin or script system no longer takes the whole editor down.
//
// The panic hook runs BEFORE that catch and cannot see it coming, which is the
// same problem `renzora::core::recoverable` solves for the engine's own catches.
// Here the catcher is Bevy's, so the hook cannot be told in advance. Instead it
// defers: on a thread that runs systems it parks the report on the thread
// ([`DEFERRED`]) and in [`LAST_DEFERRED`], and shows nothing. The handler runs on
// that same thread straight after the catch and claims the report, which is what
// marks the panic as caught. A panic nobody catches unwinds out of `App::run`,
// where `main` calls [`report_uncaught`] to save and show the parked report.

/// Set when the editor's error handler is installed, so the hook only defers
/// when something will actually catch. The setup window's `App` and every shipped
/// game leave it unset and report panics as crashes, as before.
static ECS_CATCHES_PANICS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Tells a parked report apart from a later one, so a claim on one thread can
/// never clear a report another thread parked since.
static DEFERRED_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

thread_local! {
    /// This thread's parked report, waiting for the error handler to claim it.
    static DEFERRED: std::cell::RefCell<Option<(u64, CrashReport)>> =
        const { std::cell::RefCell::new(None) };
}

/// The newest report parked on any thread, for [`report_uncaught`].
static LAST_DEFERRED: Mutex<Option<(u64, CrashReport)>> = Mutex::new(None);

/// Locations of panics the handler has already claimed. A system that panics
/// every frame would otherwise capture a full backtrace every frame, which costs
/// milliseconds each and that no one will read.
static CLAIMED_LOCATIONS: Mutex<Option<std::collections::HashSet<String>>> = Mutex::new(None);

/// How many times each system, command or observer has failed, for the Console's
/// rate limit.
static FAILURES: Mutex<Option<std::collections::HashMap<String, u64>>> = Mutex::new(None);

/// Whether this thread runs systems, and so whether Bevy will catch a panic on it.
///
/// The main thread runs exclusive and non-`Send` systems and applies commands;
/// the compute pool runs the rest; pipelined rendering runs the render world on
/// its own named thread. IO, async-compute and plain `std` threads run no
/// systems, and a panic there is still a crash.
fn on_system_thread() -> bool {
    match std::thread::current().name() {
        Some("main") | Some("Render thread") => true,
        Some(name) => name.starts_with("Compute Task Pool"),
        None => false,
    }
}

/// Install [`editor_error_handler`] on `world`.
fn install_editor_error_handler(world: &mut World) {
    world.insert_resource(bevy::ecs::error::FallbackErrorHandler(editor_error_handler));
    ECS_CATCHES_PANICS.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// The editor's `FallbackErrorHandler`: a failure becomes a Console error, and
/// the editor keeps running.
///
/// Anything below panic severity goes to Bevy's own `match_severity`, which
/// already only logs it. Panic severity covers two cases: a real panic (the hook
/// parked a report for it on this thread) and an error a system returned without
/// choosing a severity, which Bevy also treats as a panic. Both are logged.
///
/// Rate-limited per name: the first failure is logged in full, then the count at
/// 10, 100, 1000 and so on, so a system failing every frame says so without
/// burying the Console.
fn editor_error_handler(err: bevy::ecs::error::BevyError, ctx: bevy::ecs::error::ErrorContext) {
    if !matches!(err.severity(), bevy::ecs::error::Severity::Panic) {
        bevy::ecs::error::match_severity(err, ctx);
        return;
    }

    let claimed = DEFERRED.with(|d| d.borrow_mut().take());
    if let Some((id, report)) = &claimed {
        if let Ok(mut last) = LAST_DEFERRED.lock() {
            if last.as_ref().is_some_and(|(last_id, _)| last_id == id) {
                *last = None;
            }
        }
        if let Ok(mut seen) = CLAIMED_LOCATIONS.lock() {
            seen.get_or_insert_with(Default::default).insert(report.location.clone());
        }
    }

    let name = ctx.name().to_string();
    let count = FAILURES
        .lock()
        .map(|mut f| {
            let n = f.get_or_insert_with(Default::default).entry(name.clone()).or_insert(0);
            *n += 1;
            *n
        })
        .unwrap_or(1);

    if count == 1 {
        match &claimed {
            Some((_, report)) => error!(
                "{} `{}` panicked: {} (at {}). The editor caught it and kept running; \
                 whatever it does is skipped while it keeps failing.",
                ctx.kind(),
                name,
                report.message,
                report.location
            ),
            None => error!("{} `{}` failed: {}", ctx.kind(), name, err),
        }
    } else if is_power_of_ten(count) {
        warn!("{} `{}` has now failed {} times", ctx.kind(), name, count);
    }
}

fn is_power_of_ten(mut n: u64) -> bool {
    while n >= 10 && n.is_multiple_of(10) {
        n /= 10;
    }
    n == 1
}

/// Save and show a parked report whose panic nothing caught.
///
/// `main` calls this when `App::run` unwinds, which is where a panic outside any
/// system ends up. Without it, deferring would turn every such crash into a
/// silent exit with no `last_crash.txt`, the one file that tells a crash from a
/// hang afterwards.
pub fn report_uncaught() {
    let Some((_, report)) = LAST_DEFERRED.lock().ok().and_then(|mut l| l.take()) else {
        return;
    };
    if let Ok(mut guard) = CRASH_REPORT.lock() {
        *guard = Some(report.clone());
    }
    let _ = save_crash_report(&report);
    eprintln!("{}", report.format());
    #[cfg(all(
        feature = "crash_dialog",
        not(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))
    ))]
    if is_editor_process() {
        show_crash_dialog(&report);
    }
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
        // A panic the engine is about to catch (a script, a plugin constructor,
        // a backend link) is not a crash. Reporting it as one put the modal crash
        // dialog over an editor that was still running, and did so every frame
        // for a script that panics every frame. Only the location is kept, for
        // the catcher's own Console line; the backtrace is skipped too, since
        // capturing one per frame is what a looping panic would otherwise cost.
        if renzora::core::recoverable::panic_will_be_caught() {
            if let Some(loc) = panic_info.location() {
                renzora::core::recoverable::record_location(sanitize_paths(&format!(
                    "{}:{}:{}",
                    loc.file(),
                    loc.line(),
                    loc.column()
                )));
            }
            return;
        }

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

        let location = sanitize_paths(&location);
        let defer = ECS_CATCHES_PANICS.load(std::sync::atomic::Ordering::Relaxed)
            && on_system_thread();

        // A deferred panic at a location the handler has already claimed is a
        // system failing again; its backtrace was captured the first time.
        let repeat = defer
            && CLAIMED_LOCATIONS
                .lock()
                .ok()
                .is_some_and(|s| s.as_ref().is_some_and(|s| s.contains(&location)));
        let backtrace_str = if repeat {
            String::new()
        } else {
            format!("{}", Backtrace::force_capture())
        };

        let timestamp = chrono_lite_timestamp();

        let report = CrashReport {
            message: sanitize_paths(&message),
            location,
            backtrace: sanitize_paths(&backtrace_str),
            timestamp,
        };

        // Bevy is about to catch this and hand it to the editor's error
        // handler, which claims the report. See "Panics Bevy catches" above.
        if defer {
            let id = DEFERRED_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            DEFERRED.with(|d| *d.borrow_mut() = Some((id, report.clone())));
            if let Ok(mut last) = LAST_DEFERRED.lock() {
                *last = Some((id, report));
            }
            return;
        }

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

/// Record that the GPU device was lost, as a crash report the next launch
/// shows. A device loss is not a panic, so nothing else writes one, and the
/// relaunch that follows would otherwise look like a crash with no cause.
/// `recovery` names where the open scene was saved, when it was.
pub fn record_device_lost(description: &str, recovery: Option<&std::path::Path>) {
    let saved = match recovery {
        Some(path) => format!(
            "\n\nThe open scene was saved to {}. Your scene file itself was not touched; \
             open the recovery copy to get back what was unsaved.",
            path.display()
        ),
        None => String::new(),
    };
    let report = CrashReport {
        message: sanitize_paths(&format!(
            "The GPU stopped responding (device lost), so the renderer could not continue and \
             the editor restarted.\n\nReason: {description}{saved}"
        )),
        location: "GPU device lost".into(),
        backtrace: String::new(),
        timestamp: chrono_lite_timestamp(),
    };
    if let Err(e) = save_crash_report(&report) {
        error!("[crash] could not save the device-lost report: {e}");
    }
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
    // Delete the crash file after reading
    let _ = std::fs::remove_file(&crash_file);
    parse_report(&content)
}

/// A report as [`CrashReport::format`] wrote it, read back.
fn parse_report(content: &str) -> Option<CrashReport> {
    let mut message = String::new();
    let mut location = String::new();
    let mut backtrace = String::new();
    let mut timestamp = String::new();
    let mut in_backtrace = false;
    // A message runs from `Error:` to `Location:`. Reading only the `Error:`
    // line kept the first line of a multi-line message and dropped the rest:
    // a device-lost report lost its reason and where the scene was saved.
    let mut in_message = false;

    for line in content.lines() {
        if let Some(ts) = line.strip_prefix("Timestamp: ") {
            timestamp = ts.to_string();
        } else if let Some(err) = line.strip_prefix("Error: ") {
            message = err.to_string();
            in_message = true;
        } else if let Some(loc) = line.strip_prefix("Location: ") {
            location = loc.to_string();
            in_message = false;
        } else if in_message {
            message.push('\n');
            message.push_str(line);
        } else if line == "=== BACKTRACE ===" {
            in_backtrace = true;
        } else if line == "=== END CRASH REPORT ===" {
            in_backtrace = false;
        } else if in_backtrace {
            backtrace.push_str(line);
            backtrace.push('\n');
        }
    }

    // The blank line `format` puts between the message and `Location:`.
    let message = message.trim_end().to_string();
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
        if is_editor_process() {
            install_editor_error_handler(app.world_mut());
        }
    }

    /// The render world is a world of its own with its own handler, and the
    /// render sub-app only exists once `RenderPlugin` has built, which is why
    /// this is in `finish` rather than `build`.
    fn finish(&self, app: &mut App) {
        if !is_editor_process() {
            return;
        }
        if let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) {
            install_editor_error_handler(render_app.world_mut());
        }
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
    use super::{civil_from_days, is_power_of_ten, parse_report, sanitize_paths, CrashReport};

    /// A report with a message over several lines reads back whole: a
    /// device-lost report keeps its reason and where the scene was saved.
    #[test]
    fn a_multi_line_message_reads_back_whole() {
        let report = CrashReport {
            message: "The GPU stopped responding.\n\nReason: timeout\n\nSaved to ~/x.bsn.".into(),
            location: "GPU device lost".into(),
            backtrace: "frame 0\nframe 1\n".into(),
            timestamp: "2026-10-28 00:35:25 UTC".into(),
        };
        let read = parse_report(&report.format()).expect("the report parses");
        assert_eq!(read.message, report.message);
        assert_eq!(read.location, report.location);
        assert_eq!(read.timestamp, report.timestamp);
        assert_eq!(read.backtrace.trim_end(), report.backtrace.trim_end());
    }

    /// The whole point: under the editor's handler a panicking system is
    /// logged and the schedule, and the next frame, still run.
    #[test]
    fn a_panicking_system_does_not_stop_the_schedule() {
        use bevy::prelude::*;

        #[derive(Resource, Default)]
        struct Ran(u32);

        let mut world = World::new();
        world.init_resource::<Ran>();
        super::install_editor_error_handler(&mut world);
        let mut schedule = Schedule::default();
        schedule.add_systems((
            || -> () { panic!("broken plugin system") },
            |mut ran: ResMut<Ran>| ran.0 += 1,
        ));
        for _ in 0..3 {
            schedule.run(&mut world);
        }
        assert_eq!(world.resource::<Ran>().0, 3);
    }

    #[test]
    fn repeat_failures_are_reported_at_powers_of_ten() {
        let reported: Vec<u64> = (1..=2000).filter(|n| is_power_of_ten(*n)).collect();
        assert_eq!(reported, vec![1, 10, 100, 1000]);
    }

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
