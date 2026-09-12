//! Recompiling a script when its source changes, without blocking the editor.
//!
//! A Rust script costs about a second to build. Doing that on the main thread
//! would freeze the editor on every save — brief, but exactly at the moment the
//! author is watching for a result, which is the worst time to stutter. So the
//! compile runs on a task pool and only the load happens on the main thread,
//! where it is a `dlopen` and a pointer swap.
//!
//! # Why the old library is never unloaded
//!
//! A reload maps a NEW image and repoints [`LoadedScripts`]; the old one stays
//! mapped for the life of the process. It has to: a schedule, a `Local`, or a
//! captured closure may still hold pointers into it, and the plugin loader
//! deadlocked in `FreeLibrary` and later crashed the runtime with an access
//! violation learning that lesson twice.
//!
//! So an afternoon of saves leaks a few hundred KB each — a script is ~200 KB —
//! and a restart reclaims all of it. That is the price of editing native code in
//! a running process, and it is cheap next to not being able to edit at all.
//!
//! # Why a failed build is not retried
//!
//! The recorded modification time is updated when a build is *started*, not when
//! it succeeds. A script that fails to compile therefore stays quiet until it is
//! edited again, instead of rebuilding and re-reporting the same error every
//! poll — which is what turns a compile error into a scrolling wall.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use bevy::prelude::*;
use bevy::tasks::{block_on, poll_once, AsyncComputeTaskPool, Task};
use renzora::content_problems::{ContentProblem, ContentProblems, ProblemSeverity};
use renzora::core::console_log::{console_error, console_success};
use renzora::core::project_files::ProjectFileChanged;
use renzora::CurrentProject;

use crate::{load_library, sdk_dir, LoadedScripts};

#[derive(Resource, Default)]
pub struct ScriptWatcher {
    /// Modification time last seen — or last *attempted*, on failure.
    ///
    /// Keyed by full path rather than file name. The name is what
    /// [`LoadedScripts`] uses, because a script is attached by path and
    /// dispatched by that path's file name, but two folders may hold a
    /// `player.rs` and the watcher has to track their mtimes apart or an edit
    /// to one would look like an edit to both.
    seen: HashMap<PathBuf, SystemTime>,
    /// Builds in flight, so one script recompiling does not stop another from
    /// being noticed.
    building: HashMap<PathBuf, Task<Result<PathBuf, String>>>,
    /// The one-off project walk that runs when a project opens.
    ///
    /// Only then. This used to run on a 0.5s timer, inline on the main thread,
    /// and it walks the WHOLE project (see `collect_project_scripts` for why it
    /// must be the whole thing), stats every entry, and reads every `.rs` file
    /// to the end looking for the `script!` macro. On a project with a few
    /// thousand files that measured **110 ms**, twice a second, on the thread
    /// that draws frames: about seven frames of budget in one frame, which
    /// reads as a regular stutter and takes the average frame rate down with
    /// it. An empty project walks instantly, which is why it looked like an
    /// engine regression rather than a project-size one.
    ///
    /// Steady state is now `ProjectFileChanged`, so the walk exists for one
    /// reason: to discover scripts that were already on disk when the project
    /// opened, which no file event will ever mention.
    scan: Option<Task<Vec<(PathBuf, SystemTime)>>>,
    /// The project root the opening walk has already been run for, so it runs
    /// once per project and not once per `CurrentProject` write.
    scanned_root: Option<PathBuf>,
}

impl ScriptWatcher {
    /// Record that the script at `path`, modified at `mtime`, has already been
    /// dealt with.
    ///
    /// Called by the project-open build so the watcher does not immediately
    /// rebuild everything it just compiled. Without it the first poll after a
    /// project opens sees a tree full of files it has never heard of and starts
    /// a second rustc for every one.
    pub fn mark_seen(&mut self, path: PathBuf, mtime: SystemTime) {
        self.seen.insert(path, mtime);
    }

    /// Record that `root` has already been walked, so [`scan_on_project_open`]
    /// does not walk it again.
    ///
    /// Called by the project-open build, which does that walk itself. Without
    /// it the two raced: the opening scan runs in `Update` and fires as soon as
    /// `CurrentProject` exists, which is during `Loading`, while the build runs
    /// on `OnEnter(SplashState::Editor)` and is therefore later. The scan
    /// finished first, found a `seen` map the build had not filled yet, and
    /// started a second rustc for every script in the project.
    ///
    /// That showed up as `loaded spinner.rs` and `reloaded spinner.rs` eight
    /// milliseconds apart at every launch: two concurrent compiles of the same
    /// file, and a leaked image for the one that lost.
    pub fn mark_scanned(&mut self, root: PathBuf) {
        self.scanned_root = Some(root);
    }
}

/// Start a full walk when the project ROOT changes, and only then.
///
/// Steady-state change detection is [`watch`], driven by the project watcher.
/// A file event can only tell you about a file that changed while you were
/// listening, though, so something still has to find the scripts that were
/// already sitting on disk when the project was opened. That is this, once per
/// project.
///
/// Gated on the root rather than on `CurrentProject::is_changed()`, which is a
/// different and much more frequent question: that resource is also written when
/// a scene is renamed, when the main scene changes, and on every edit in the
/// project settings tab. Keying the walk on change detection would have run it
/// on all of those, which is the 110ms stall this system exists to stop doing.
pub fn scan_on_project_open(
    mut watcher: ResMut<ScriptWatcher>,
    project: Option<Res<CurrentProject>>,
) {
    let Some(project) = project else { return };
    if watcher.scanned_root.as_deref() == Some(project.path.as_path()) || watcher.scan.is_some() {
        return;
    }
    watcher.scanned_root = Some(project.path.clone());
    // The whole project, through the same walk the project-open build and the
    // exporter use. Watching `scripts/` alone was a third reader of "which files
    // are scripts" that disagreed with the other two: a script kept beside the
    // scene that uses it compiled at startup, shipped in an export, and then
    // silently stopped rebuilding on save (the edit appeared to do nothing, with
    // the previous build still loaded and nothing logged). So the breadth is
    // deliberate, and the cost of it is paid off the main thread.
    let root = project.path.clone();
    watcher.scan = Some(AsyncComputeTaskPool::get().spawn(async move {
        // Stat in the same task: a `metadata` call per script is small next to
        // the walk, but it is the same kind of blocking work and there is no
        // reason to hand it back to the main thread.
        crate::collect_project_scripts(&root)
            .into_iter()
            .filter_map(|src| {
                let mtime = std::fs::metadata(&src).and_then(|m| m.modified()).ok()?;
                Some((src, mtime))
            })
            .collect()
    }));
}

/// Build scripts that changed on disk, and land the opening walk when it
/// finishes.
///
/// The steady-state path reads [`ProjectFileChanged`] and touches only the files
/// it names. Before this it polled every 0.5s and re-walked the entire project
/// to find out whether anything had happened, which on an asset-heavy project
/// cost 110ms of a frame twice a second to answer "no" almost every time. A save
/// is now noticed as soon as the watcher debounce elapses rather than up to half
/// a second later, and a quiet project costs nothing at all.
pub fn watch(
    mut watcher: ResMut<ScriptWatcher>,
    project: Option<Res<CurrentProject>>,
    mut changes: MessageReader<ProjectFileChanged>,
) {
    let Some(project) = project else {
        // Drained rather than left: without a project there is nothing to build,
        // and letting them pile up would mean a burst of stale work the moment
        // one opens.
        changes.clear();
        return;
    };

    // Anything the opening walk turned up, plus anything that changed since.
    // `poll_once` takes the result, so the task is finished with either way.
    let mut sources: Vec<(PathBuf, SystemTime)> = watcher
        .scan
        .as_mut()
        .and_then(|task| block_on(poll_once(task)))
        .inspect(|_| watcher.scan = None)
        .unwrap_or_default();

    for change in changes.read() {
        if !change.has_extension("rs") {
            continue;
        }
        if !change.is_live() {
            // Gone. Forget its modification time so that restoring the file, or
            // a `git checkout` putting it back, rebuilds it rather than matching
            // a stale entry and doing nothing. The loaded image stays mapped:
            // unloading one has deadlocked and later crashed this process, and
            // a script whose source is deleted mid-session should keep running
            // rather than take the editor with it.
            watcher.seen.remove(&change.path);
            continue;
        }
        // Cheap here, ruinous in a walk: this reads the file to check for the
        // `script!` macro, which is exactly what made the full-project scan
        // expensive. One file that just changed is a different proposition.
        if !crate::declares_script(&change.path) {
            continue;
        }
        let Ok(mtime) = std::fs::metadata(&change.path).and_then(|m| m.modified()) else {
            continue;
        };
        sources.push((change.path.clone(), mtime));
    }

    if sources.is_empty() {
        return;
    }

    let Some(sdk_dir) = sdk_dir() else { return };
    let project_path = project.path.clone();

    for (src, mtime) in sources {
        // Already building: let it finish rather than starting a second rustc
        // for the same file.
        if watcher.building.contains_key(&src) {
            continue;
        }
        if watcher.seen.get(&src) == Some(&mtime) {
            continue;
        }

        // Recorded BEFORE the build, so a script that fails to compile is not
        // retried until it is edited again.
        watcher.seen.insert(src.clone(), mtime);

        let sdk_dir = sdk_dir.clone();
        let project_path = project_path.clone();
        let build_src = src.clone();
        let task = AsyncComputeTaskPool::get().spawn(async move {
            // The SDK is re-read in the task rather than shared: it is a small
            // JSON file, and this keeps anything with a lifetime out of the
            // closure.
            let sdk = renzora_plugin_build::Sdk::load(sdk_dir)
                .map_err(|e| e.to_string())?;
            crate::build_to_path(&sdk, &project_path, &build_src)
        });
        watcher.building.insert(src, task);
    }
}

/// Load whatever finished building this frame.
///
/// Separate from [`watch`] because the load must happen on the main thread — it
/// mutates [`LoadedScripts`] — while the compile must not.
pub fn finish(world: &mut World) {
    let done: Vec<(PathBuf, Result<PathBuf, String>)> = {
        let Some(mut watcher) = world.get_resource_mut::<ScriptWatcher>() else {
            return;
        };
        // Polled exactly once each: `poll_once` takes the result, so a second
        // poll on a finished task would find nothing and the build would be
        // silently dropped.
        let mut done = Vec::new();
        for (src, task) in watcher.building.iter_mut() {
            if let Some(result) = block_on(poll_once(task)) {
                done.push((src.clone(), result));
            }
        }
        for (src, _) in &done {
            watcher.building.remove(src);
        }
        done
    };

    for (src, result) in done {
        // Back to the file name for the registry: the dispatcher looks a script
        // up by the file name of the path it was attached with, so that is the
        // key an image has to land under however it was found.
        let name = src
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();
        match result.and_then(|lib_path| load_library(&lib_path)) {
            Ok((f, hook, lib)) => {
                let mut loaded = world.resource_mut::<LoadedScripts>();
                loaded.insert(name.clone(), f, hook, lib);
                info!("[rust-script] reloaded {name}");
                console_success("Script", format!("recompiled {name}"));
                report_problem(world, &src, None);
            }
            Err(e) => {
                error!("[rust-script] {name}: {e}");
                console_error("Script", format!("{name}\n{e}"));
                report_problem(world, &src, Some(&e));
            }
        }
    }
}

/// Put a build result on the file, where the Problems panel will find it.
///
/// Logging it was not enough, and the gap was not cosmetic. A failed build went
/// to the Console, which scrolls, and to `tracing`, which is off-screen — while
/// the Problems panel, the one place anybody looks to ask whether the project
/// is healthy, went on reporting the file as clean. A script that would not
/// compile was then indistinguishable from one that had: the editor kept
/// running the previous image, every save appeared to do nothing, and there was
/// no surface anywhere saying why. A material that fails to compile has said so
/// here since it was written; a script should too.
///
/// `None` clears, so a build that succeeds takes the last failure down with it.
fn report_problem(world: &mut World, src: &Path, error: Option<&str>) {
    let Some(root) = world
        .get_resource::<CurrentProject>()
        .map(|project| project.path.clone())
    else {
        return;
    };
    // Project-relative and forward-slashed, which is the key every other
    // producer uses; an absolute path would open a second row for the same file.
    let path = src
        .strip_prefix(&root)
        .unwrap_or(src)
        .to_string_lossy()
        .replace('\\', "/");

    let problems = match error {
        None => Vec::new(),
        Some(message) => vec![ContentProblem {
            severity: ProblemSeverity::Error,
            line: rustc_line(message, src),
            message: message.to_string(),
            node_id: None,
        }],
    };
    if let Some(mut store) = world.get_resource_mut::<ContentProblems>() {
        // By severity, not wholesale: the panel may already be carrying
        // warnings for this file from somewhere else, and a build result has no
        // business dropping them.
        store.set_severity(path, ProblemSeverity::Error, problems);
    }
}

/// The line rustc blamed, from the first `--> <this file>:LINE:COL` it printed.
///
/// Restricted to the script itself on purpose. rustc happily points into the
/// SDK or into a macro expansion in the same message, and sending the panel to
/// a file the author cannot open is worse than sending it nowhere.
fn rustc_line(output: &str, src: &Path) -> Option<usize> {
    let name = src.to_string_lossy();
    output.lines().find_map(|line| {
        line.trim_start()
            .strip_prefix("--> ")?
            .strip_prefix(name.as_ref())?
            .strip_prefix(':')?
            .split(':')
            .next()?
            .parse()
            .ok()
    })
}
