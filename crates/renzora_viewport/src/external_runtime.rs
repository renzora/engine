//! Play: the game runs as its own process, never inside the editor.
//!
//! Pressing Play saves the scene and starts this same executable again with
//! `--no-editor --project <dir>`, which boots it as the shipped game would.
//! The child handle lives in [`ExternalRuntime`]; [`poll_external_runtime`]
//! reaps it when the game window closes so the Play button flips back on its
//! own, and Stop kills it.
//!
//! **Why a process and not the viewport.** In-process play rendered the game
//! through the editor's own frame, so a game running at 10 FPS took the whole
//! editor down to 10 FPS with it, and a crash in the game was a crash in the
//! editor. As a separate process it has its own window, GPU device and frame
//! loop, and the editor stays fully usable while it runs. The two still share
//! one GPU, so a game that saturates it slows the editor's drawing, but no
//! longer its frame rate one-for-one.

use bevy::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Child;

/// How long after the spawn the game counts as still starting. The editor
/// cannot see from here when the child's window actually opens, and a cold
/// boot loads every plugin before it does, so this only covers the usual
/// delay before treating the game as up.
const PREPARE_GRACE_SECS: f32 = 2.0;

/// Which stage of the game process's life we're in.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePhase {
    /// No game running.
    #[default]
    Idle,
    /// Child spawned, its window probably not up yet.
    Preparing,
    /// The game is running.
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

    /// The running game's process id, which is how the editor finds the
    /// game's window to place in the Game panel.
    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }

    /// The game's window has been found, so it is up whatever the timer says.
    pub fn mark_running(&mut self) {
        if self.child.is_some() {
            self.phase = RuntimePhase::Running;
        }
    }

    /// Mark the runtime as just-spawned and start the grace timer. Called
    /// right after a successful spawn.
    pub fn begin_preparing(&mut self) {
        self.phase = RuntimePhase::Preparing;
        self.prepare_elapsed = 0.0;
    }
}

/// Locate the runtime binary to launch for external play.
///
/// **The editor and the game are one executable again**, which is what this
/// function has to answer to. It was written for the interval when they were
/// two: Bevy was statically linked, a loadable editor would have carried its own
/// copy and therefore its own `World` type, so the editor had to be
/// `renzora-editor[.exe]` beside a separate `renzora[.exe]`. In that world,
/// relaunching the current executable relaunched the *editor* — picking "Window"
/// opened a second editor instead of the game — so the self-relaunch fallback
/// was removed and this returned `None`.
///
/// `dynamic_linking` is back on and the editor is a loadable image again (see
/// `renzora_runtime::editor_image`). There is one `renzora[.exe]`, and whether
/// it is the editor is decided by `renzora_editor.dll` sitting beside it *and*
/// the absence of `--no-editor`. So the current executable is now the correct
/// answer, and returning `None` is what broke the Window play target: no binary
/// was found, and Play quietly fell back to in-viewport.
///
/// A separately packaged runtime still wins when there is one — a split
/// `editor/` + `runtime/` export has a real second binary and should use it.
///
/// 1. `<exe_dir>/runtime/renzora[.exe]` — a split package.
/// 2. `<exe_dir>/../runtime/renzora[.exe]` — the same, one level up.
/// 3. The retired `renzora-runtime[.exe]` in either place, for older trees.
/// 4. This executable, relaunched with `--no-editor`.
pub fn find_runtime_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;

    let suffix = if cfg!(target_os = "windows") { ".exe" } else { "" };
    let names = [format!("renzora{suffix}"), format!("renzora-runtime{suffix}")];

    for name in &names {
        let candidates = [
            Some(exe_dir.join("runtime").join(name)),
            exe_dir.parent().map(|d| d.join("runtime").join(name)),
            Some(exe_dir.join(name)),
        ];
        for candidate in candidates.into_iter().flatten() {
            // Skip the running binary here — it is the fallback below, taken
            // only once no separate runtime has been found.
            if candidate == exe || !candidate.exists() {
                continue;
            }
            return Some(candidate);
        }
    }
    // One binary: relaunch ourselves as a game. `--no-editor` is what makes that
    // a game rather than a second editor — `main` checks for it before it will
    // install the editor image.
    Some(exe)
}

/// Spawn the runtime pointed at `project_path`. Returns the child handle
/// on success. The runtime accepts `--project <path>` and treats either a
/// directory (looks for `project.toml` inside) or the `.toml` itself as
/// valid input — see `renzora_engine::parse_project_arg`.
///
/// `--no-editor` is always passed: it's what makes the self-relaunch fallback
/// boot as a game, and a harmless no-op for a dedicated runtime binary (which
/// has no editor bundle to suppress in the first place). `embed_parent` is
/// the editor's window handle when the game should go in the Game panel.
///
/// Its output is piped into the editor console (category `Runtime`) rather than
/// inherited. On Windows the runtime is a GUI-subsystem binary launched from
/// another GUI process, so it has no console attached and everything it logs —
/// every `[audio]`, `[autoload]` and `[plugin]` line, every panic — went
/// nowhere. Debugging a game that behaves differently outside the editor meant
/// running it by hand from a terminal to find out why.
pub fn spawn_runtime(
    binary: &Path,
    project_path: &Path,
    embed_parent: Option<isize>,
    live_link: Option<&renzora::LiveLinkEndpoint>,
    profiling_key: Option<&str>,
) -> std::io::Result<Child> {
    use std::process::{Command, Stdio};
    let mut command = Command::new(binary);
    command.arg("--no-editor").arg("--project").arg(project_path);
    // The editor window the game makes its own window a child of, so it can
    // sit in the Game panel; see `renzora_runtime::embed`.
    if let Some(parent) = embed_parent {
        command.arg("--embed-parent").arg(parent.to_string());
    }
    // Where to connect so edits made in the editor reach this game. See
    // `renzora_live_link`.
    if let Some(link) = live_link {
        command
            .arg("--live-link")
            .arg(link.port.to_string())
            .arg("--live-link-token")
            .arg(&link.token);
    }
    // The key the companion pairs with, so it can profile this game. On this
    // machine only: no `--profiling-lan`.
    if let Some(key) = profiling_key {
        command.arg("--profiling-key").arg(key);
    }
    // Piped, and therefore *must* be drained: a pipe nobody reads fills up and
    // then blocks the child on its next log line. The reader threads below are
    // what makes this safe, not just useful.
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command.spawn()?;
    // Ended with the editor however the editor ends: a game left behind holds
    // the executable open and fails the next build's staging step.
    renzora::child_process::tie_to_editor(&child, "the game");
    if let Some(stdout) = child.stdout.take() {
        renzora::child_process::forward_to_console(stdout, "Runtime");
    }
    if let Some(stderr) = child.stderr.take() {
        renzora::child_process::forward_to_console(stderr, "Runtime");
    }
    Ok(child)
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
            // Game window closed (or it crashed): drop the handle so the
            // Play button reads Play again.
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
/// once it elapses.
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
/// The immediate exit is deliberate: letting the editor unwind normally tears
/// down the whole World on the main thread — FreeLibrary of the editor bundle +
/// plugin dlls, wgpu device destruction, worker-thread cleanup — which stalls
/// for tens of seconds ("Not Responding" on Windows, "didn't close properly" on
/// macOS). None of that teardown does anything the OS doesn't already do at
/// process exit, and nothing in the engine saves state from a `Drop` impl (saves
/// happen on user action; the one AppExit consumer is this system). Runs in
/// `Last`, after every system in the final frame.
///
/// It leaves through [`renzora::exit_now`] rather than `std::process::exit`,
/// which was still slow for a reason this system could not see: `exit` runs
/// libc's atexit chain and every shared object's destructors first, 7.2 s of
/// them with a project open from an AppImage build. See that function.
/// Set `RENZORA_FULL_TEARDOWN=1` to get the old unwinding exit back when
/// debugging teardown itself.
///
/// Native only, and not merely because `exit_now` is: on the web there is no
/// process teardown to skip and no external runtime to kill. A tab's lifetime
/// is the tab's, and `AppExit` just stops the loop.
#[cfg(not(target_arch = "wasm32"))]
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
    renzora::exit_now(code);
}
