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

/// Tracks the running runtime child process, if any. Created at startup
/// and queried by the viewport header to decide whether the Play button
/// should render as Play or Stop.
#[derive(Resource, Default)]
pub struct ExternalRuntime {
    child: Option<Child>,
}

impl ExternalRuntime {
    /// Whether a child runtime is currently running. Updated by
    /// [`poll_external_runtime`] each frame; reading it is cheap.
    pub fn is_alive(&self) -> bool {
        self.child.is_some()
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
            runtime.child = None;
        }
        Ok(None) => {}
        Err(_) => {
            // try_wait failure is unrecoverable for this handle — drop it
            // so we don't keep retrying every frame.
            runtime.child = None;
        }
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
