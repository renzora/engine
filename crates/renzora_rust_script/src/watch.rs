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
//! captured closure may still hold pointers into it, and `renzora_plugin`'s
//! loader deadlocked in `FreeLibrary` and later crashed the runtime with an
//! access violation learning that lesson twice.
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
use std::path::PathBuf;
use std::time::SystemTime;

use bevy::prelude::*;
use bevy::tasks::{block_on, poll_once, AsyncComputeTaskPool, Task};
use renzora::core::console_log::{console_error, console_success};
use renzora::CurrentProject;

use crate::{load_library, sdk_root, LoadedScripts};

/// How often the scripts directory is stat'd, in seconds.
///
/// Polling rather than a filesystem watcher because the directory is small and
/// this avoids a second notify backend in the process — `renzora_plugin`'s
/// hot-reload already runs one, and two watchers on overlapping trees is a
/// source of double-fires nobody wants to debug.
const POLL_SECONDS: f32 = 0.5;

#[derive(Resource, Default)]
pub struct ScriptWatcher {
    /// Modification time last seen — or last *attempted*, on failure.
    seen: HashMap<String, SystemTime>,
    /// Builds in flight, keyed by file name, so one script recompiling does not
    /// stop another from being noticed.
    building: HashMap<String, Task<Result<PathBuf, String>>>,
    timer: f32,
}

impl ScriptWatcher {
    /// Record that `name` at `mtime` has already been dealt with.
    ///
    /// Called by the project-open build so the watcher does not immediately
    /// rebuild everything it just compiled. Without it the first poll after a
    /// project opens sees a directory full of files it has never heard of and
    /// starts a second rustc for every one.
    pub fn mark_seen(&mut self, name: String, mtime: SystemTime) {
        self.seen.insert(name, mtime);
    }
}

/// Notice changed or new `.rs` files and start building them.
pub fn watch(
    mut watcher: ResMut<ScriptWatcher>,
    project: Option<Res<CurrentProject>>,
    time: Res<Time>,
) {
    watcher.timer += time.delta_secs();
    if watcher.timer < POLL_SECONDS {
        return;
    }
    watcher.timer = 0.0;

    let Some(project) = project else { return };
    let scripts_dir = project.path.join("scripts");
    let Ok(entries) = std::fs::read_dir(&scripts_dir) else {
        return;
    };

    let Some(sdk_root) = sdk_root() else { return };
    let project_path = project.path.clone();

    for entry in entries.flatten() {
        let src = entry.path();
        if src.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let Some(name) = src.file_name().and_then(|n| n.to_str()).map(str::to_string) else {
            continue;
        };
        // Already building — let it finish rather than starting a second rustc
        // for the same file.
        if watcher.building.contains_key(&name) {
            continue;
        }
        let Ok(mtime) = entry.metadata().and_then(|m| m.modified()) else {
            continue;
        };
        if watcher.seen.get(&name) == Some(&mtime) {
            continue;
        }

        // Recorded BEFORE the build, so a script that fails to compile is not
        // retried until it is edited again.
        watcher.seen.insert(name.clone(), mtime);

        let sdk_root = sdk_root.clone();
        let project_path = project_path.clone();
        let src = src.clone();
        let task = AsyncComputeTaskPool::get().spawn(async move {
            // The SDK is re-read in the task rather than shared: it is a small
            // JSON file, and this keeps anything with a lifetime out of the
            // closure.
            let sdk = renzora_plugin_build::Sdk::load(sdk_root.join("sdk"))
                .map_err(|e| e.to_string())?;
            crate::build_to_path(&sdk, &project_path, &src)
        });
        watcher.building.insert(name, task);
    }
}

/// Load whatever finished building this frame.
///
/// Separate from [`watch`] because the load must happen on the main thread — it
/// mutates [`LoadedScripts`] — while the compile must not.
pub fn finish(world: &mut World) {
    let done: Vec<(String, Result<PathBuf, String>)> = {
        let Some(mut watcher) = world.get_resource_mut::<ScriptWatcher>() else {
            return;
        };
        // Polled exactly once each: `poll_once` takes the result, so a second
        // poll on a finished task would find nothing and the build would be
        // silently dropped.
        let mut done = Vec::new();
        for (name, task) in watcher.building.iter_mut() {
            if let Some(result) = block_on(poll_once(task)) {
                done.push((name.clone(), result));
            }
        }
        for (name, _) in &done {
            watcher.building.remove(name);
        }
        done
    };

    for (name, result) in done {
        match result.and_then(|lib_path| load_library(&lib_path)) {
            Ok((f, lib)) => {
                let mut loaded = world.resource_mut::<LoadedScripts>();
                loaded.insert(name.clone(), f, lib);
                info!("[rust-script] reloaded {name}");
                console_success("Script", format!("recompiled {name}"));
            }
            Err(e) => {
                error!("[rust-script] {name}: {e}");
                console_error("Script", format!("{name}\n{e}"));
            }
        }
    }
}
