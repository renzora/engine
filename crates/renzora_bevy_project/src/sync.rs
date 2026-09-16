//! Rebuild a Bevy project when its source changes, without blocking the editor.
//!
//! The loop this closes is the one that made people ask for the feature: edit in
//! your IDE, save, and find out. Before this, "find out" meant quitting the
//! editor and relaunching it.
//!
//! ```text
//! save src/level.rs
//!   │
//!   ├─ rebuild on a task pool          the editor keeps running at 60 fps
//!   ├─ errors land in Console+Problems while you are still looking at the code
//!   └─ on success: "Restart to load"   one click, ~3s, back where you were
//! ```
//!
//! # Why a restart and not a swap
//!
//! A Rust script can be hot-swapped because it is *one function pointer* the
//! engine holds: rebuild, `dlopen`, repoint, done (see
//! `renzora_rust_script::watch`). A plugin is not that. `add_plugins` scatters
//! the project's systems through Bevy's schedules as function pointers, and Bevy
//! has no API to take them back out, so a second `add_plugins` would leave the
//! old systems running beside the new ones, with no way to tell which is which.
//!
//! So the rebuild happens live and the *load* waits for a restart. That splits
//! the wait in the right place: the compile is the slow half (about 7 seconds on
//! the project this was built against) and it happens while you keep working;
//! the restart is about 3 seconds and happens when you ask.
//!
//! # Why the error half matters more than the restart half
//!
//! Most saves do not produce a build you want to look at. They produce a
//! compile error. Reporting it the moment it happens, in the editor you already
//! have open, is most of the value here, and it costs nothing, because the
//! build had to run anyway to find out.
//!
//! # Why a failed build is not retried
//!
//! The same rule as the script watcher, for the same reason: a build is recorded
//! as attempted when it *starts*. A project that does not compile stays quiet
//! until it is edited again, rather than rebuilding and re-reporting the same
//! error on every poll and turning one mistake into a scrolling wall.

use std::path::PathBuf;
use std::time::SystemTime;

use bevy::prelude::*;
use bevy::tasks::{block_on, poll_once, AsyncComputeTaskPool, Task};
use renzora::content_problems::{ContentProblem, ContentProblems, ProblemSeverity};
use renzora::core::bevy_project;
use renzora::core::console_log::{console_error, console_info, console_success};
use renzora::core::project_files::ProjectFileChanged;
use renzora::CurrentProject;

/// What the background rebuild is doing, and what came of it.
///
/// A resource rather than log lines alone because "your code is newer than what
/// is running" is a *state* the editor should be able to show, not an event that
/// scrolls away thirty seconds after it mattered.
#[derive(Resource, Default)]
pub struct ProjectSync {
    /// The most recent source change we have not yet built.
    pending: Option<SystemTime>,
    /// The running build, if any.
    building: Option<Task<Result<Vec<String>, String>>>,
    /// A build finished and the code on disk is newer than the code loaded in
    /// this process. Cleared only by a restart, because only a restart can load
    /// it.
    pub restart_wanted: bool,
    /// Why the last build failed, for a panel that wants to show it.
    pub last_error: Option<String>,
}

impl ProjectSync {
    /// Is a rebuild running right now?
    pub fn is_building(&self) -> bool {
        self.building.is_some()
    }
}

/// Note that the project's Rust changed.
///
/// Only `.rs` and `Cargo.toml`: everything else under the project is an asset,
/// and the asset pipeline already reloads those without a compiler. Rebuilding
/// the crate because a `.png` was saved would turn every texture tweak into a
/// seven-second build.
pub fn note_source_changes(
    mut changes: MessageReader<ProjectFileChanged>,
    project: Option<Res<CurrentProject>>,
    mut sync: ResMut<ProjectSync>,
) {
    let Some(project) = project else { return };
    if !project.config.kind.is_code_first() {
        // Drained anyway: an unread reader accumulates, and a project that is
        // not code-first still generates plenty of file events.
        changes.clear();
        return;
    }
    for change in changes.read() {
        // Project files only. The watcher also carries directories registered
        // through `ExtraWatchRoots` (the engine's own `languages/` folder is
        // one), and those are nobody's crate.
        if change.root != project.path {
            continue;
        }
        let path = &change.path;
        // The build's own output is not a source change, and treating it as one
        // is a loop rather than a wasted compile: a rebuild rewrites
        // `.renzora/bevy/src/lib.rs`, which is a `.rs` file inside the project,
        // which schedules the rebuild that rewrites it again, every 0.4s, each
        // one leaving another generation-suffixed library on disk. `target` is
        // the same story with cargo's generated sources.
        if crate::cache::is_build_output(&project.path, path) {
            continue;
        }
        let is_rust = path.extension().and_then(|e| e.to_str()) == Some("rs");
        let is_manifest = path.file_name().and_then(|n| n.to_str()) == Some("Cargo.toml");
        if is_rust || is_manifest {
            sync.pending = Some(SystemTime::now());
        }
    }
}

/// Start a rebuild for a change that has settled.
///
/// Debounced, because a save is not one event. An editor that writes to a
/// temporary file and renames it produces several, a formatter on save produces
/// more, and "save all" across six files produces six. Building on the first one
/// means building five more times against source that is still moving.
pub fn start_rebuild(
    project: Option<Res<CurrentProject>>,
    mut sync: ResMut<ProjectSync>,
    time: Res<Time<Real>>,
    mut settled: Local<f32>,
) {
    /// How long the source has to stay still. Long enough to absorb a save-all,
    /// short enough that it feels like a consequence of pressing Ctrl+S.
    const DEBOUNCE: f32 = 0.4;

    let Some(project) = project else { return };
    if !project.config.kind.is_code_first() || sync.is_building() || sync.pending.is_none() {
        *settled = 0.0;
        return;
    }
    *settled += time.delta_secs();
    if *settled < DEBOUNCE {
        return;
    }
    *settled = 0.0;
    sync.pending = None;

    let root = project.path.clone();
    console_info("Bevy", "rebuilding the project");
    sync.building = Some(AsyncComputeTaskPool::get().spawn(async move { rebuild(root) }));
}

/// The build itself, off the main thread.
///
/// Nothing here touches the `World`, which is what makes it safe to run on a
/// task pool while the editor keeps drawing.
fn rebuild(root: PathBuf) -> Result<Vec<String>, String> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = root;
        Err("a Bevy project cannot be compiled in a browser".to_string())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let krate = bevy_project::inspect(&root)
            .ok_or_else(|| format!("{} is no longer a readable Bevy crate", root.display()))?;
        let staged = crate::entry::stage(&krate, &root).map_err(|e| e.to_string())?;
        let install_root = renzora_plugin_build::install::root()
            .ok_or_else(|| "could not find the editor's own directory".to_string())?;
        let sdk = renzora_plugin_build::Sdk::load(renzora_plugin_build::install::sdk_dir(
            &install_root,
        ))
        .map_err(|e| e.to_string())?;

        // A fresh filename per build, for the reason the loader documents: the
        // library from the previous build is mapped into this process and cannot
        // be replaced on Windows.
        let generation = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let out = staged.dir.join(format!(
            "{}-{generation}.{}",
            krate.crate_name(),
            sdk.manifest().lib_ext
        ));

        let mut notes = staged.notes;
        crate::compile(&sdk, &krate, &root, &staged.dir, &out, &mut notes, &mut |_| {})?;
        Ok(notes)
    }
}

/// Collect a finished rebuild and say what happened.
pub fn finish_rebuild(
    mut sync: ResMut<ProjectSync>,
    mut problems: Option<ResMut<ContentProblems>>,
    project: Option<Res<CurrentProject>>,
) {
    // `poll_once` takes the result, so the task is finished with either way.
    let Some(result) = sync
        .building
        .as_mut()
        .and_then(|task| block_on(poll_once(task)))
    else {
        return;
    };
    sync.building = None;

    let label = project
        .as_ref()
        .map(|p| p.config.name.clone())
        .unwrap_or_else(|| "the project".to_string());

    match result {
        Ok(notes) => {
            sync.last_error = None;
            sync.restart_wanted = true;
            for note in notes {
                info!("[bevy-project] {note}");
            }
            if let Some(problems) = problems.as_mut() {
                problems.clear_path(&problem_path(project.as_deref()));
            }
            console_success(
                "Bevy",
                format!("{label} rebuilt; restart the editor to load it"),
            );
        }
        Err(report) => {
            // Both, and neither is redundant: the Console is where the author is
            // already looking, and Problems is where an error survives the next
            // thirty lines of log.
            console_error("Bevy", report.clone());
            error!("[bevy-project] {report}");
            if let Some(problems) = problems.as_mut() {
                problems.set(
                    problem_path(project.as_deref()),
                    vec![ContentProblem {
                        severity: ProblemSeverity::Error,
                        message: report.clone(),
                        line: None,
                        node_id: None,
                    }],
                );
            }
            sync.last_error = Some(report);
        }
    }
}

/// The `ContentProblems` key a build failure is filed under.
///
/// The project's crate root, so the Problems panel points at a file the author
/// recognises and a rebuild replaces its own previous entry instead of stacking
/// one per save. `set` overwrites the whole list for a path, which is exactly
/// the behaviour wanted: the newest build's errors are the only true ones.
fn problem_path(project: Option<&CurrentProject>) -> String {
    project
        .and_then(|p| bevy_project::inspect(&p.path))
        .map(|krate| krate.root.to_string_lossy().to_string())
        .unwrap_or_else(|| "Cargo.toml".to_string())
}

/// Say in the status bar what the background build is doing.
///
/// The one place a *state* belongs. A Console line saying "rebuilt" is gone by
/// the time you have finished reading the code you just saved; "Rebuilt, press
/// F5 to load" has to still be there when you look up.
///
/// Uses the same override the autosave countdown uses, so the two cannot both
/// claim the label: whichever wrote last wins, and neither leaves it stuck
/// (`None` restores "Ready").
pub fn show_sync_status(
    sync: Res<ProjectSync>,
    project: Option<Res<CurrentProject>>,
    status: Option<ResMut<renzora::core::shell::ShellReadyStatus>>,
    mut owned: Local<bool>,
) {
    let Some(mut status) = status else { return };
    let code_first = project.is_some_and(|p| p.config.kind.is_code_first());
    if !code_first {
        return;
    }

    let (label, color) = if sync.is_building() {
        (Some("Rebuilding project…".to_string()), Some(AMBER))
    } else if sync.last_error.is_some() {
        (Some("Project failed to build".to_string()), Some(RED))
    } else if sync.restart_wanted {
        (
            Some(format!("Rebuilt, {RESTART_HINT} to load")),
            Some(GREEN),
        )
    } else {
        (None, None)
    };

    // Only write when this system has something to say, or when it is clearing
    // its own message. Writing `None` unconditionally would stamp on the
    // autosave countdown every frame.
    if label.is_some() {
        *owned = true;
        status.label = label;
        status.color = color;
    } else if *owned {
        *owned = false;
        status.label = None;
        status.color = None;
    }
}

const AMBER: [u8; 3] = [228, 132, 52];
const RED: [u8; 3] = [220, 80, 80];
const GREEN: [u8; 3] = [48, 196, 140];

/// What the status line tells the user to press.
const RESTART_HINT: &str = "File ▸ Import Bevy Project";

/// Restart into the open project once its rebuild is in.
///
/// Public so a button or a menu row can call it. The path comes from
/// `CurrentProject` rather than being remembered, because the restart argument
/// and the open project must be the same thing or the successor opens something
/// else entirely.
#[cfg(not(target_arch = "wasm32"))]
pub fn restart_into_current(world: &mut World) {
    let Some(path) = world.get_resource::<CurrentProject>().map(|p| p.path.clone()) else {
        return;
    };
    bevy_project::restart_into(&path);
}
