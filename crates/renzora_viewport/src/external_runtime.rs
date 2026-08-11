//! External-runtime play mode — spawns the exported `renzora-runtime`
//! binary as a child process pointed at the current project, instead of
//! doing the in-editor camera switch. Gives a "real exported game"
//! experience while the editor stays in editing mode.
//!
//! The child handle lives in [`ExternalRuntime`]; [`poll_external_runtime`]
//! reaps it when the runtime window closes so the play button flips back
//! to "Play" on its own. Pressing Play again while a child is alive sends
//! [`PlayModeState::request_stop`], which kills the child.

use bevy::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Child;

/// How long the "Preparing export runtime" overlay stays up after we spawn
/// the child, before flipping to the "runtime running / editor paused"
/// overlay. We can't observe when the child actually opens its OS window
/// from the parent process, so this grace period covers the typical
/// window-open delay so the user sees "preparing…" first.
const PREPARE_GRACE_SECS: f32 = 2.0;

/// Which stage of the external-runtime lifecycle we're in. Drives the
/// full-screen overlay that pauses the editor while the runtime owns the
/// screen.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePhase {
    /// No external runtime — editor behaves normally.
    #[default]
    Idle,
    /// Child spawned, window not up yet. Shows "Preparing export runtime".
    Preparing,
    /// Runtime window is up; editor is paused until the child exits.
    Running,
}

/// Tracks the running runtime child process, if any. Created at startup
/// and queried by the viewport header to decide whether the Play button
/// should render as Play or Stop.
#[derive(Resource, Default)]
pub struct ExternalRuntime {
    child: Option<Child>,
    phase: RuntimePhase,
    /// Seconds spent in [`RuntimePhase::Preparing`] so far.
    prepare_elapsed: f32,
}

impl ExternalRuntime {
    /// Whether a child runtime is currently running. Updated by
    /// [`poll_external_runtime`] each frame; reading it is cheap.
    pub fn is_alive(&self) -> bool {
        self.child.is_some()
    }

    /// Current lifecycle phase, used to drive the pause overlay.
    pub fn phase(&self) -> RuntimePhase {
        self.phase
    }

    /// Mark the runtime as just-spawned: show the "preparing" overlay and
    /// start the grace timer. Called right after a successful spawn.
    pub fn begin_preparing(&mut self) {
        self.phase = RuntimePhase::Preparing;
        self.prepare_elapsed = 0.0;
    }
}

/// Locate the runtime binary to launch for external play.
///
/// The runtime is **`renzora[.exe]`** and the editor is **`renzora-editor[.exe]`**
/// — two separate executables staged side by side. Once Bevy became statically
/// linked the editor stopped being a loadable bundle the host could decline, so
/// "runtime" is a different file rather than the same file told to behave.
///
/// That is why this looks for `renzora` and why there is **no fall back to the
/// current executable**. It used to end with `Some(exe)`, from when one binary
/// booted either way and `--no-editor` was enough to make it a game. With a
/// separate editor executable that fallback relaunches the *editor*: picking the
/// "Window" play target opened a second editor window instead of the game, which
/// is exactly what it was reported doing. Returning `None` instead makes the
/// caller log and fall back to in-viewport play, which at least plays the game.
///
/// It also looks for the retired `renzora-runtime[.exe]`, so an older staged
/// tree still works.
///
/// 1. `<exe_dir>/renzora[.exe]` — the normal staged layout.
/// 2. `<exe_dir>/runtime/renzora[.exe]` — a split `editor/` + `runtime/` package.
/// 3. `<exe_dir>/../runtime/renzora[.exe]` — the same, one level up.
pub fn find_runtime_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;

    let suffix = if cfg!(target_os = "windows") { ".exe" } else { "" };
    let names = [format!("renzora{suffix}"), format!("renzora-runtime{suffix}")];

    for name in &names {
        let candidates = [
            Some(exe_dir.join(name)),
            Some(exe_dir.join("runtime").join(name)),
            exe_dir.parent().map(|d| d.join("runtime").join(name)),
        ];
        for candidate in candidates.into_iter().flatten() {
            // Never hand back the binary we are already running. On a dev build
            // the editor can sit in the same directory under a name that matches,
            // and spawning ourselves is the bug this function had.
            if candidate == exe || !candidate.exists() {
                continue;
            }
            return Some(candidate);
        }
    }
    None
}

/// Spawn the runtime pointed at `project_path`. Returns the child handle
/// on success. The runtime accepts `--project <path>` and treats either a
/// directory (looks for `project.toml` inside) or the `.toml` itself as
/// valid input — see `renzora_engine::parse_project_arg`.
///
/// `--no-editor` is always passed: it's what makes the self-relaunch fallback
/// boot as a game, and a harmless no-op for a dedicated runtime binary (which
/// has no editor bundle to suppress in the first place). `vr` adds `--vr`, the
/// OpenXR boot flag — the "VR Headset" play target.
///
/// Its output is piped into the editor console (category `Runtime`) rather than
/// inherited. On Windows the runtime is a GUI-subsystem binary launched from
/// another GUI process, so it has no console attached and everything it logs —
/// every `[audio]`, `[autoload]` and `[plugin]` line, every panic — went
/// nowhere. Debugging a game that behaves differently outside the editor meant
/// running it by hand from a terminal to find out why.
pub fn spawn_runtime(binary: &Path, project_path: &Path, vr: bool) -> std::io::Result<Child> {
    use std::process::{Command, Stdio};
    let mut command = Command::new(binary);
    command.arg("--no-editor").arg("--project").arg(project_path);
    if vr {
        command.arg("--vr");
    }
    // Piped, and therefore *must* be drained: a pipe nobody reads fills up and
    // then blocks the child on its next log line. The reader threads below are
    // what makes this safe, not just useful.
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command.spawn()?;
    if let Some(stdout) = child.stdout.take() {
        forward_to_console(stdout);
    }
    if let Some(stderr) = child.stderr.take() {
        forward_to_console(stderr);
    }
    Ok(child)
}

/// Read `stream` line by line on its own thread, pushing each line into the
/// editor console. Detached: the read ends when the child closes the pipe, so
/// the thread retires itself with the process it was reading.
fn forward_to_console(stream: impl std::io::Read + Send + 'static) {
    use renzora::core::console_log::console_log;
    use std::io::{BufRead, BufReader};

    std::thread::spawn(move || {
        // Lossy rather than strict UTF-8: a runtime that dies mid-line, or logs
        // a path the OS handed it in some other encoding, must not silence the
        // rest of the stream.
        for line in BufReader::new(stream).split(b'\n') {
            let Ok(bytes) = line else { break };
            let line = String::from_utf8_lossy(&bytes);
            let line = strip_ansi(line.trim_end_matches('\r'));
            if line.trim().is_empty() {
                continue;
            }
            let (level, message) = classify_runtime_line(&line);
            console_log(level, "Runtime", message);
        }
    });
}

/// Drop SGR colour escapes. `tracing_subscriber` colours its output whether or
/// not the sink is a terminal, and a pipe is never one, so without this every
/// forwarded line arrives wrapped in `\x1b[…m` noise.
fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        // `ESC [ … <final byte in @..~>`. Anything else after ESC is a short
        // escape we simply drop along with its one following character.
        if chars.next() != Some('[') {
            continue;
        }
        for c in chars.by_ref() {
            if ('@'..='~').contains(&c) {
                break;
            }
        }
    }
    out
}

/// Split a forwarded log line into a console level and the text to show.
///
/// The runtime's lines look like `<timestamp>  INFO  <target>: <message>`. Both
/// the timestamp and the level are dropped: the console renders its own level
/// colour and the timestamp is noise next to the editor's own lines.
fn classify_runtime_line(line: &str) -> (renzora::core::console_log::LogLevel, String) {
    use renzora::core::console_log::LogLevel;

    let mut rest = line.trim_start();
    // An RFC3339 timestamp is the first token and always ends in `Z`.
    if let Some((first, tail)) = rest.split_once(char::is_whitespace) {
        if first.ends_with('Z') && first.contains(':') {
            rest = tail.trim_start();
        }
    }

    for (token, level) in [
        ("ERROR", LogLevel::Error),
        ("WARN", LogLevel::Warning),
        ("INFO", LogLevel::Info),
        ("DEBUG", LogLevel::Info),
        ("TRACE", LogLevel::Info),
    ] {
        if let Some(tail) = rest.strip_prefix(token) {
            if tail.starts_with(char::is_whitespace) {
                return (level, tail.trim_start().to_string());
            }
        }
    }

    // No level token: wgpu/naga chatter, or a panic. A panic is the one thing
    // here nobody can afford to have filed as routine information.
    let level = if line.contains("panicked at") || line.starts_with("thread '") {
        LogLevel::Error
    } else {
        LogLevel::Info
    };
    (level, rest.to_string())
}

/// Detach the running child, if any, and kill it. Returns whether a child
/// was killed (so callers can log meaningfully).
pub fn kill_runtime(runtime: &mut ExternalRuntime) -> bool {
    runtime.phase = RuntimePhase::Idle;
    runtime.prepare_elapsed = 0.0;
    let Some(mut child) = runtime.child.take() else {
        return false;
    };
    // Best-effort kill — if the child has already exited we don't care.
    let _ = child.kill();
    let _ = child.wait();
    true
}

/// Replace the tracked child with a new one. Any previously tracked child
/// is killed first so we never leak runtime processes.
pub fn replace_child(runtime: &mut ExternalRuntime, child: Child) {
    let _ = kill_runtime(runtime);
    runtime.child = Some(child);
}

/// Reap the child if it exited on its own (user closed the runtime
/// window, runtime panicked, etc.) so [`ExternalRuntime::is_alive`] flips
/// back to false without anyone having to press Stop.
pub fn poll_external_runtime(mut runtime: ResMut<ExternalRuntime>) {
    let Some(child) = runtime.child.as_mut() else {
        return;
    };
    match child.try_wait() {
        Ok(Some(status)) => {
            // How it ended, not just that it did: a runtime that dies on its
            // own is indistinguishable from one the user closed, and the two
            // want very different reactions.
            match status.code() {
                Some(0) | None => {
                    renzora::core::console_log::console_info("PlayMode", "Runtime window closed")
                }
                Some(code) => renzora::core::console_log::console_error(
                    "PlayMode",
                    format!("Runtime exited with code {code}"),
                ),
            }
            // Runtime window closed (or it crashed) — drop the handle and
            // lift the pause overlay so the editor is usable again.
            runtime.child = None;
            runtime.phase = RuntimePhase::Idle;
            runtime.prepare_elapsed = 0.0;
        }
        Ok(None) => {}
        Err(_) => {
            // try_wait failure is unrecoverable for this handle — drop it
            // so we don't keep retrying every frame.
            runtime.child = None;
            runtime.phase = RuntimePhase::Idle;
            runtime.prepare_elapsed = 0.0;
        }
    }
}

/// Tick the "preparing" grace timer and flip to [`RuntimePhase::Running`]
/// once it elapses, so the overlay transitions from "Preparing export
/// runtime" to the "editor paused" message after the window has had time to
/// appear.
pub fn advance_runtime_phase(time: Res<Time>, mut runtime: ResMut<ExternalRuntime>) {
    if runtime.phase != RuntimePhase::Preparing {
        return;
    }
    // The child can die during the grace window (e.g. instant crash); poll
    // will have reset us to Idle in that case, so only advance if still alive.
    if !runtime.is_alive() {
        runtime.phase = RuntimePhase::Idle;
        return;
    }
    runtime.prepare_elapsed += time.delta_secs();
    if runtime.prepare_elapsed >= PREPARE_GRACE_SECS {
        runtime.phase = RuntimePhase::Running;
    }
}

/// Reap any running child when the editor decides to exit, then leave the
/// process immediately. Without the reap the runtime would be orphaned: on
/// Windows a child isn't tied to its parent's lifetime by default, and on
/// Linux/macOS the same is true without an explicit job/process group.
///
/// Reads `AppExit` events rather than firing on `Drop` because by the
/// time the `App` is being torn down, ECS resources are already gone.
///
/// The `std::process::exit` is deliberate: letting the editor unwind
/// normally tears down the whole World on the main thread — FreeLibrary of
/// the editor bundle + plugin dlls, wgpu device destruction, worker-thread
/// cleanup — which stalls for tens of seconds ("Not Responding" on Windows,
/// "didn't close properly" on macOS). None of that teardown does anything
/// the OS doesn't already do at process exit, and nothing in the engine
/// saves state from a `Drop` impl (saves happen on user action; the one
/// AppExit consumer is this system). Runs in `Last`, after every system in
/// the final frame. Set `RENZORA_FULL_TEARDOWN=1` to get the old unwinding
/// exit back when debugging teardown itself.
pub fn kill_on_app_exit(
    mut exits: MessageReader<bevy::app::AppExit>,
    mut runtime: ResMut<ExternalRuntime>,
) {
    let Some(exit) = exits.read().last().cloned() else {
        return;
    };
    kill_runtime(&mut runtime);
    if std::env::var_os("RENZORA_FULL_TEARDOWN").is_some() {
        return;
    }
    let code = match exit {
        bevy::app::AppExit::Success => 0,
        bevy::app::AppExit::Error(n) => i32::from(n.get()),
    };
    info!("[exit] fast exit (code {code})");
    std::process::exit(code);
}

/// How long winit waits between forced wakeups while the editor is paused.
/// Each wakeup runs one update — enough to repaint the (static) pause
/// overlay and let [`poll_external_runtime`] notice the runtime window
/// closing — but slow enough that the editor stops continuously rendering
/// and hands the GPU to the running game.
const PAUSED_WAKE_INTERVAL_MS: u64 = 250;

/// Stashes the editor's normal [`WinitSettings`] while it's paused so we can
/// restore the exact cadence it had before the runtime took over.
#[derive(Resource, Default)]
pub struct PausedRenderState {
    saved: Option<bevy::winit::WinitSettings>,
}

/// Throttle the editor's update/render loop while the external runtime is
/// active, and restore it when the runtime window closes.
///
/// The throttle engages the moment Play is pressed (during `Preparing`, not
/// just `Running`) so the editor stops rendering immediately rather than
/// ramping down. While throttled, winit only wakes every
/// [`PAUSED_WAKE_INTERVAL_MS`]; together with the deactivated editor cameras
/// and the static overlay, the editor sits on a frozen dark screen instead
/// of doing per-frame rendering until the runtime exits.
pub fn apply_runtime_pause_render(
    runtime: Res<ExternalRuntime>,
    mut winit: ResMut<bevy::winit::WinitSettings>,
    mut state: ResMut<PausedRenderState>,
) {
    use bevy::winit::UpdateMode;
    use std::time::Duration;

    let paused = runtime.phase != RuntimePhase::Idle;
    match (paused, state.saved.is_some()) {
        // Entering the paused state — stash the live settings, then drop both
        // focused and unfocused cadence to the slow wakeup interval.
        (true, false) => {
            state.saved = Some(winit.clone());
            let low =
                UpdateMode::reactive_low_power(Duration::from_millis(PAUSED_WAKE_INTERVAL_MS));
            winit.focused_mode = low;
            winit.unfocused_mode = low;
        }
        // Leaving the paused state — restore the editor's normal cadence.
        (false, true) => {
            if let Some(prev) = state.saved.take() {
                *winit = prev;
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora::core::console_log::LogLevel;

    #[test]
    fn colour_escapes_are_stripped() {
        let line = "\u{1b}[2m2026-08-10T13:24:25Z\u{1b}[0m \u{1b}[32m INFO\u{1b}[0m ready";
        assert_eq!(strip_ansi(line), "2026-08-10T13:24:25Z  INFO ready");
    }

    #[test]
    fn the_timestamp_and_level_come_off_the_front() {
        let (level, message) =
            classify_runtime_line("2026-08-10T13:24:25.159700Z  WARN renzora_audio: no ears");
        assert_eq!(level, LogLevel::Warning);
        assert_eq!(message, "renzora_audio: no ears");
    }

    /// A line the runtime's own logger didn't write — wgpu chatter, a panic —
    /// still has to arrive, and a panic must not arrive as routine info.
    #[test]
    fn an_unprefixed_line_survives_and_a_panic_is_an_error() {
        let (level, message) = classify_runtime_line("wgpu: picked adapter");
        assert_eq!(level, LogLevel::Info);
        assert_eq!(message, "wgpu: picked adapter");

        let (level, _) = classify_runtime_line("thread 'main' panicked at src/main.rs:12:5:");
        assert_eq!(level, LogLevel::Error);
    }

    /// `INFORMATIONAL` is not the `INFO` token, and a word boundary is the only
    /// thing that tells them apart.
    #[test]
    fn a_level_token_needs_a_word_boundary() {
        let (_, message) = classify_runtime_line("INFOrmational text");
        assert_eq!(message, "INFOrmational text");
    }
}
