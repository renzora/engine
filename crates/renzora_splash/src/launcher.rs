//! Editor-subprocess launcher — runs inside the splash process.
//!
//! When the user picks a project, `SplashState::Loading` transitions and this
//! plugin:
//!
//! 1. Pre-scans the plugin directories (next to the exe + the project root) so
//!    we know the real total plugin count before the subprocess starts.
//! 2. Spawns the editor binary as a child process with `--project <path>`,
//!    capturing its stdout/stderr via OS pipes.
//! 3. Reads both pipes on background threads, updating [`LoadProgress`] with
//!    real-time data parsed from log lines:
//!    - `[dynamic-plugin] Loading 'X'` / `Registered 'X'` → plugins phase
//!    - `[progress] thumbnails N/M <name>` → thumbnails phase
//! 4. Re-emits every line on the splash's own stdout/stderr so the terminal
//!    still sees the full log.
//! 5. Watches for [`EDITOR_READY_SENTINEL`] — when seen, the splash exits.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::window::{MonitorSelection, PrimaryWindow, WindowPosition};

use crate::project::CurrentProject;
use crate::{LoadingTaskHandle, LoadingTasks, SplashState, LOADING_WINDOW_SIZE};

pub const EDITOR_READY_SENTINEL: &str = "<<<RENZORA_EDITOR_READY>>>";

#[cfg(target_os = "windows")]
const DLL_EXT: &str = "dll";
#[cfg(target_os = "linux")]
const DLL_EXT: &str = "so";
#[cfg(target_os = "macos")]
const DLL_EXT: &str = "dylib";

/// Whole-program phase labels. The launcher progresses through these in order
/// as it parses messages from the editor subprocess.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LoadPhase {
    #[default]
    Starting,
    Plugins,
    Thumbnails,
    Finalizing,
}

impl LoadPhase {
    pub fn label(self) -> &'static str {
        match self {
            LoadPhase::Starting => "Starting editor",
            LoadPhase::Plugins => "Loading plugins",
            LoadPhase::Thumbnails => "Generating material thumbnails",
            LoadPhase::Finalizing => "Finalizing",
        }
    }
}

#[derive(Clone, Default)]
pub struct LoadProgressSnapshot {
    pub phase: LoadPhase,
    pub current: String,
    pub done: u32,
    pub total: u32,
}

impl LoadProgressSnapshot {
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.done as f32 / self.total as f32).clamp(0.0, 1.0)
        }
    }
}

#[derive(Resource, Clone, Default)]
pub struct LoadProgress {
    inner: Arc<Mutex<LoadProgressSnapshot>>,
}

impl LoadProgress {
    pub fn snapshot(&self) -> LoadProgressSnapshot {
        self.inner.lock().unwrap().clone()
    }
    fn set_phase_with_total(&self, phase: LoadPhase, total: u32) {
        let mut g = self.inner.lock().unwrap();
        if g.phase != phase {
            g.phase = phase;
            g.done = 0;
            g.current = String::new();
        }
        g.total = total;
    }
    fn set_phase(&self, phase: LoadPhase) {
        let mut g = self.inner.lock().unwrap();
        if g.phase != phase {
            g.phase = phase;
            g.done = 0;
            g.current = String::new();
        }
    }
    fn set_current(&self, name: &str) {
        self.inner.lock().unwrap().current = name.to_string();
    }
    fn plugin_registered(&self, name: &str) {
        let mut g = self.inner.lock().unwrap();
        g.phase = LoadPhase::Plugins;
        g.done = g.done.saturating_add(1).min(g.total.max(g.done + 1));
        g.current = name.to_string();
    }
    fn set_thumbnail_progress(&self, done: u32, total: u32, name: &str) {
        let mut g = self.inner.lock().unwrap();
        g.phase = LoadPhase::Thumbnails;
        g.done = done;
        g.total = total;
        if !name.is_empty() {
            g.current = name.to_string();
        }
    }
    fn set_finalizing(&self) {
        let mut g = self.inner.lock().unwrap();
        g.phase = LoadPhase::Finalizing;
        g.done = g.total;
        g.current = String::new();
    }
}

#[derive(Resource)]
struct SubprocessState {
    child: Arc<Mutex<Option<Child>>>,
    sentinel_seen: Arc<Mutex<bool>>,
    task_handle: LoadingTaskHandle,
}

pub struct SplashLauncherPlugin;

impl Plugin for SplashLauncherPlugin {
    fn build(&self, app: &mut App) {
        info!("[splash] SplashLauncherPlugin");
        app.init_resource::<LoadProgress>()
            .add_systems(
                OnEnter(SplashState::Loading),
                (shrink_splash_window, spawn_editor_subprocess).chain(),
            )
            .add_systems(
                Update,
                watch_for_ready.run_if(in_state(SplashState::Loading)),
            );
    }
}

/// Shrink the splash's own window to the compact loader size and center it.
/// Only runs in the splash process (this plugin is not added in the editor).
fn shrink_splash_window(mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    for mut window in windows.iter_mut() {
        window
            .resolution
            .set(LOADING_WINDOW_SIZE.0, LOADING_WINDOW_SIZE.1);
        window.resizable = false;
        window.position = WindowPosition::Centered(MonitorSelection::Primary);
    }
}

fn spawn_editor_subprocess(
    project: Option<Res<CurrentProject>>,
    progress: Res<LoadProgress>,
    mut tasks: ResMut<LoadingTasks>,
    mut commands: Commands,
) {
    let Some(project) = project else {
        error!("[launcher] Loading entered with no CurrentProject — cannot spawn editor");
        return;
    };
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            error!("[launcher] Cannot get current exe path: {}", e);
            return;
        }
    };

    // Pre-count plugins so the progress bar has a real denominator from frame 1.
    let mut plugin_total: u32 = 0;
    if let Some(exe_dir) = exe.parent() {
        plugin_total += count_dlls(&exe_dir.join("plugins"), false);
    }
    plugin_total += count_dlls(&project.path, true);

    progress.set_phase_with_total(LoadPhase::Plugins, plugin_total);
    info!("[launcher] Pre-scanned {} dynamic plugins", plugin_total);

    let mut child = match Command::new(&exe)
        .arg("--project")
        .arg(&project.path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            error!("[launcher] Failed to spawn editor subprocess: {}", e);
            return;
        }
    };

    info!("[launcher] Spawned editor subprocess (pid {})", child.id());

    let sentinel_seen = Arc::new(Mutex::new(false));

    if let Some(stdout) = child.stdout.take() {
        spawn_reader(stdout, progress.clone(), sentinel_seen.clone(), false);
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_reader(stderr, progress.clone(), sentinel_seen.clone(), true);
    }

    let task_handle = tasks.register("Editor startup", 1);

    commands.insert_resource(SubprocessState {
        child: Arc::new(Mutex::new(Some(child))),
        sentinel_seen,
        task_handle,
    });
}

fn count_dlls(dir: &Path, recursive: bool) -> u32 {
    if !dir.exists() {
        return 0;
    }
    let mut count: u32 = 0;
    let mut stack: Vec<PathBuf> = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if recursive {
                    stack.push(path);
                }
                continue;
            }
            if path.extension().and_then(|s| s.to_str()) != Some(DLL_EXT) {
                continue;
            }
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if stem.starts_with("bevy_dylib") || stem.starts_with("std-") {
                continue;
            }
            count += 1;
        }
    }
    count
}

fn spawn_reader<R>(
    source: R,
    progress: LoadProgress,
    sentinel: Arc<Mutex<bool>>,
    is_stderr: bool,
) where
    R: std::io::Read + Send + 'static,
{
    std::thread::spawn(move || {
        let reader = BufReader::new(source);
        for line in reader.lines().flatten() {
            if line.contains(EDITOR_READY_SENTINEL) {
                *sentinel.lock().unwrap() = true;
                continue;
            }
            parse_progress_line(&line, &progress);

            // Re-emit unfiltered so the terminal still sees the full log.
            if is_stderr {
                eprintln!("{}", line);
            } else {
                println!("{}", line);
            }
        }
    });
}

fn parse_progress_line(line: &str, progress: &LoadProgress) {
    // `[dynamic-plugin] Loading 'NAME' (KIND)` — announces start of a plugin load.
    if let Some(rest) = line.split_once("[dynamic-plugin] Loading '") {
        if let Some(name) = rest.1.split('\'').next() {
            progress.set_phase(LoadPhase::Plugins);
            progress.set_current(name);
        }
        return;
    }
    // `[dynamic-plugin] Registered 'NAME'` — plugin finished loading.
    if let Some(rest) = line.split_once("[dynamic-plugin] Registered '") {
        if let Some(name) = rest.1.split('\'').next() {
            progress.plugin_registered(name);
        }
        return;
    }
    // `[progress] thumbnails DONE/TOTAL NAME` — editor-emitted thumbnail progress.
    if let Some(rest) = line.split_once("[progress] thumbnails ") {
        let mut parts = rest.1.splitn(2, ' ');
        let frac = parts.next().unwrap_or("");
        let name = parts.next().unwrap_or("");
        if let Some((d, t)) = frac.split_once('/') {
            let done: u32 = d.parse().unwrap_or(0);
            let total: u32 = t.parse().unwrap_or(0);
            progress.set_thumbnail_progress(done, total, name);
            if total > 0 && done >= total {
                progress.set_finalizing();
            }
        }
    }
}

fn watch_for_ready(
    state: Option<Res<SubprocessState>>,
    mut tasks: ResMut<LoadingTasks>,
) {
    let Some(state) = state else { return };

    if *state.sentinel_seen.lock().unwrap() {
        tasks.complete(state.task_handle);
        info!("[launcher] editor subprocess signaled ready — exiting splash");
        std::process::exit(0);
    }

    let child_arc = state.child.clone();
    let mut guard = match child_arc.lock() {
        Ok(g) => g,
        Err(e) => {
            error!("[launcher] Subprocess mutex poisoned: {}", e);
            return;
        }
    };
    if let Some(child) = guard.as_mut() {
        match child.try_wait() {
            Ok(Some(status)) => {
                error!(
                    "[launcher] Editor subprocess exited before ready: {}",
                    status
                );
                std::process::exit(status.code().unwrap_or(1));
            }
            Ok(None) => {}
            Err(e) => error!("[launcher] Failed to poll subprocess status: {}", e),
        }
    }
}
