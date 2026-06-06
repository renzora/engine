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

/// Locate the runtime binary that ships next to this editor build.
///
/// In a packaged build the editor lives at `dist/{platform}/editor/<exe>`
/// and the runtime sibling is at `dist/{platform}/runtime/renzora-runtime[.exe]`.
/// `cargo run` from the workspace produces no such sibling, so this
/// returns `None` and the caller falls back to in-editor play mode.
pub fn find_runtime_binary() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let editor_dir = exe.parent()?;
    let dist_dir = editor_dir.parent()?;
    let runtime_dir = dist_dir.join("runtime");

    let bin_name = if cfg!(target_os = "windows") {
        "renzora-runtime.exe"
    } else {
        "renzora-runtime"
    };

    let candidate = runtime_dir.join(bin_name);
    candidate.exists().then_some(candidate)
}

/// Spawn the runtime pointed at `project_path`. Returns the child handle
/// on success. The runtime accepts `--project <path>` and treats either a
/// directory (looks for `project.toml` inside) or the `.toml` itself as
/// valid input — see `renzora_engine::parse_project_arg`.
pub fn spawn_runtime(binary: &Path, project_path: &Path) -> std::io::Result<Child> {
    use std::process::Command;
    Command::new(binary)
        .arg("--project")
        .arg(project_path)
        .spawn()
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
        Ok(Some(_status)) => {
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

/// Reap any running child when the editor decides to exit. Without this
/// the runtime would be orphaned: on Windows a child isn't tied to its
/// parent's lifetime by default, and on Linux/macOS the same is true
/// without an explicit job/process group.
///
/// Reads `AppExit` events rather than firing on `Drop` because by the
/// time the `App` is being torn down, ECS resources are already gone.
pub fn kill_on_app_exit(
    mut exits: MessageReader<bevy::app::AppExit>,
    mut runtime: ResMut<ExternalRuntime>,
) {
    if exits.read().next().is_some() {
        kill_runtime(&mut runtime);
    }
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
