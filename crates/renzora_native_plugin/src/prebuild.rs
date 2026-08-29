//! Getting the install ready **before** Bevy starts.
//!
//! A downloaded release arrives with the SDK still compressed and every native
//! plugin still source-only — `docker/build-all.sh` deliberately skips building
//! them, because the only sound way to build one is against the staged SDK, which
//! does not exist until it is unpacked. So the first launch after an install or
//! an update has real work to do before the editor can be the editor.
//!
//! # Why before `App::new()` and not during the splash
//!
//! [`NativePluginLoader`](crate::NativePluginLoader) loads plugins during `App`
//! assembly, which happens before the splash renders. Unpacking on the splash
//! would therefore arrive too late for the very thing that needed it: the loader
//! would already have found every plugin unbuilt, reported them, and moved on.
//! Doing the work first means that by the time the loader runs there is nothing
//! left to do, on that same launch.
//!
//! The cost is that there is no renderer yet, so progress is text. That is
//! acceptable here and nowhere else: `renzora-editor` keeps a console on every
//! platform (only the shipped runtime is `windows_subsystem = "windows"`), so
//! the output is actually seen.
//!
//! # Why it restarts afterwards
//!
//! Continuing in-process would work — nothing here leaves the process in a bad
//! state. Restarting is a deliberate simplification: the second launch takes the
//! ordinary path with an SDK present and every plugin built, which is the path
//! that gets exercised on every subsequent run. It means setup has exactly one
//! shape rather than being a special case threaded through boot.

use std::path::{Path, PathBuf};

use renzora_plugin_build::unpack::{self, SdkState};
use renzora_plugin_build::Sdk;

use crate::{exe_dir, layout, name_of, read_dir_sorted};

/// Where setup has got to, for a progress bar to draw.
///
/// Structured rather than preformatted text because the caller draws it: the
/// editor puts up a small window with a real bar, and a headless run turns the
/// same values into log lines. A `&str` callback could only ever serve the
/// second.
#[derive(Debug, Clone, PartialEq)]
pub enum Progress {
    /// Unpacking the SDK archive; `done`/`total` are COMPRESSED bytes, which is
    /// the only pair with a known total.
    Unpacking { done: u64, total: u64 },
    /// Compiling plugin `name`, number `index` of `total`.
    Building { name: String, index: usize, total: usize },
    /// One line the compiler wrote, as it wrote it.
    ///
    /// Carries no fraction — the bar stays where [`Building`](Self::Building)
    /// put it and only the caption changes. rustc says nothing until it
    /// finishes, so for a plugin with no third-party dependencies these arrive
    /// only when something is wrong; for one with dependencies they are cargo's
    /// `Compiling …` lines and cover most of the wait.
    Compiling { name: String, line: String },
    /// A step failed. Setup continues — this is a report, not a stop.
    Failed(String),
}

impl std::fmt::Display for Progress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Progress::Unpacking { done, total } => {
                let pct = done.saturating_mul(100) / (*total).max(1);
                write!(f, "Unpacking the Rust SDK… {pct}%")
            }
            Progress::Building { name, index, total } => {
                write!(f, "Building plugins… [{index}/{total}] {name}")
            }
            // One line, whatever the compiler produced. Trimmed because rustc
            // indents continuation lines heavily and a caption is one line wide.
            Progress::Compiling { name, line } => write!(f, "{name}: {}", line.trim()),
            Progress::Failed(e) => write!(f, "{e}"),
        }
    }
}

/// What [`run`] did, so the caller can decide whether to restart.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Prepared {
    /// The SDK was unpacked from its archive.
    pub unpacked_sdk: bool,
    /// How many plugins were compiled.
    pub built: usize,
}

impl Prepared {
    /// Whether anything happened at all. `false` on every launch after the first.
    pub fn did_work(&self) -> bool {
        self.unpacked_sdk || self.built > 0
    }
}

/// Is there any setup to do at all?
///
/// Called before anything is shown, so an ordinary launch never puts up a setup
/// window it would close a frame later. Cheap: a couple of directory stats, plus
/// a stamp read per plugin only when an SDK is actually present.
pub fn needed() -> bool {
    let Some(root) = exe_dir() else {
        return false;
    };
    if matches!(unpack::sdk_state(&root), SdkState::Packed { .. }) {
        return true;
    }
    let dir = root.join("plugins");
    let Ok(sdk) = Sdk::load(root.join("sdk")) else {
        return false;
    };
    let expected = sdk.stamp();
    let disabled = renzora::load_disabled_plugins();
    read_dir_sorted(&dir)
        .into_iter()
        .filter(|p| p.join("src").join("lib.rs").is_file())
        .filter(|p| !disabled.iter().any(|d| d == &name_of(p)))
        .any(|p| layout(&p, Some(&sdk), Some(&expected)).needs_build)
}

/// Unpack the SDK if it is still an archive, then build any plugin that needs it.
///
/// Returns what it did. Errors are reported and swallowed rather than returned:
/// a plugin that will not compile must not stop the editor from starting, and the
/// loader will report it again in its own words once the `App` exists.
pub fn run(report: &mut impl FnMut(Progress)) -> Prepared {
    let mut done = Prepared::default();
    let Some(root) = exe_dir() else {
        return done;
    };

    if let SdkState::Packed { archive, bytes } = unpack::sdk_state(&root) {
        report(Progress::Unpacking { done: 0, total: bytes });
        let result = unpack::extract(&archive, &root, |read| {
            report(Progress::Unpacking { done: read, total: bytes })
        });
        match result {
            Ok(_) => {
                // Delete the archive once the tree is in place. Keeping it would
                // hold ~444 MB forever for no benefit: an update replaces the
                // whole install directory, so it is never the source of a repair
                // — a re-extract would come from the new download, not this file.
                //
                // Safe only because it happens AFTER `extract` returned Ok, and
                // `extract` renames the finished tree into place atomically. A
                // failure leaves the archive untouched and retryable.
                if let Err(e) = std::fs::remove_file(&archive) {
                    report(Progress::Failed(format!(
                        "could not remove {}: {e}",
                        archive.display()
                    )));
                }
                done.unpacked_sdk = true;
            }
            // Not fatal. Everything already built still runs; what is lost is the
            // ability to build more, which the editor reports in context later.
            Err(e) => report(Progress::Failed(format!("SDK could not be unpacked: {e}"))),
        }
    }

    done.built = build_stale(&root, report);
    done
}

/// Compile every plugin whose artefact is missing or stale.
///
/// Deliberately does NOT load anything: loading is the `App`'s job, and doing it
/// here would map images into a process that is about to be replaced.
fn build_stale(root: &Path, report: &mut impl FnMut(Progress)) -> usize {
    let dir = root.join("plugins");
    if !dir.is_dir() {
        return 0;
    }
    let Ok(sdk) = Sdk::load(root.join("sdk")) else {
        // No SDK: nothing can be built, and saying so here would duplicate the
        // loader's message with less context.
        return 0;
    };
    let expected = sdk.stamp();

    // The same list the loader will walk, minus the plugins the user switched
    // off — compiling one of those would be work for something that will not run.
    let disabled = renzora::load_disabled_plugins();
    let pending: Vec<PathBuf> = read_dir_sorted(&dir)
        .into_iter()
        .filter(|p| p.join("src").join("lib.rs").is_file())
        .filter(|p| !disabled.iter().any(|d| d == &name_of(p)))
        .filter(|p| layout(p, Some(&sdk), Some(&expected)).needs_build)
        .collect();
    if pending.is_empty() {
        return 0;
    }

    let total = pending.len();
    let mut built = 0;
    for (i, plugin) in pending.iter().enumerate() {
        let name = name_of(plugin);
        report(Progress::Building { name: name.clone(), index: i + 1, total });
        let l = layout(plugin, Some(&sdk), Some(&expected));
        if let Err(e) = std::fs::create_dir_all(plugin.join("build")) {
            report(Progress::Failed(format!("{name}: {e}")));
            continue;
        }
        match sdk.compile_with(plugin, &l.lib_path, &mut |line| {
            report(Progress::Compiling { name: name.clone(), line: line.to_string() });
        }) {
            Ok(stamp) => match std::fs::write(&l.stamp_path, &stamp) {
                Ok(()) => built += 1,
                Err(e) => report(Progress::Failed(format!("{name}: writing stamp: {e}"))),
            },
            // Reported and skipped. The loader will say it again, in the place a
            // user is looking, once there is a Problems panel to say it in.
            Err(e) => report(Progress::Failed(format!("{name} failed to build: {e}"))),
        }
    }
    built
}

/// Relaunch this executable with the same arguments and exit.
///
/// Spawned detached rather than waited on: this process is finished, and holding
/// it open would leave a redundant parent in the task list for the whole session.
pub fn restart() -> ! {
    let exe = std::env::current_exe();
    if let Ok(exe) = exe {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let _ = std::process::Command::new(exe).args(args).spawn();
    }
    std::process::exit(0)
}
