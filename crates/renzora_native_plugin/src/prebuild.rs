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
use std::sync::atomic::{AtomicUsize, Ordering};

use renzora_plugin_build::unpack::{self, SdkState};
use renzora_plugin_build::Sdk;

use crate::{exe_dir, is_native_source, layout, name_of};

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
    ///
    /// `index` is the order the build STARTED in, not how many are finished.
    /// Several arrive at once, because several plugins compile at a time — so
    /// this drives the caption and not the bar. [`Built`](Self::Built) is what
    /// measures progress.
    Building { name: String, index: usize, total: usize },
    /// Plugin `name` finished, successfully or not; `done` of `total` are now
    /// complete.
    ///
    /// Separate from [`Building`](Self::Building) because with a parallel build
    /// the two are genuinely different numbers: eight plugins start within
    /// milliseconds of each other and finish four seconds later. A bar driven by
    /// starts jumps to 8/52 and then sits still, which is the one thing a
    /// progress bar must not do; driven by completions it climbs once per plugin.
    Built { name: String, done: usize, total: usize },
    /// One line the compiler wrote, as it wrote it.
    ///
    /// Carries no fraction — the bar stays where [`Building`](Self::Building)
    /// put it and only the caption changes. rustc says nothing until it
    /// finishes, so for a plugin with no third-party dependencies these arrive
    /// only when something is wrong; for one with dependencies they are cargo's
    /// `Compiling …` lines and cover most of the wait.
    ///
    /// Carries `index`/`total` as well as the name, because this REPLACES the
    /// caption `Building` put up. Without them the counter vanishes the moment
    /// the compiler says anything, which reads as the progress having been lost.
    Compiling { name: String, index: usize, total: usize, line: String },
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
            Progress::Built { name, done, total } => {
                write!(f, "Built {name} ({done}/{total})")
            }
            // One line, whatever the compiler produced. Trimmed because rustc
            // indents continuation lines heavily and a caption is one line wide.
            // Keeps the counter so the caption never goes backwards.
            Progress::Compiling { name, index, total, line } => {
                write!(f, "[{index}/{total}] {name}: {}", line.trim())
            }
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

/// A missing toolchain that stopped work, and whether the editor can fix it.
///
/// Two states rather than one, because the remedies are not comparable. Asking
/// rustup to add a toolchain is a bounded, reversible thing the user has already
/// consented to by installing rustup. Putting Rust on a machine that has none is
/// a decision about their machine, and an editor should not make it by pressing
/// its own button.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolchainGap {
    /// rustup is here; the pinned toolchain is not. One command, ~400 MB.
    Installable { version: String },
    /// No Rust at all, and the fix is rustup's installer.
    RustupMissing,
}

/// What, if anything, has to be installed before plugins can be built.
///
/// Answered after a run rather than before it, so it describes what actually
/// stopped rather than what might.
pub fn toolchain_gap() -> Option<ToolchainGap> {
    let root = exe_dir()?;
    match Sdk::load(crate::sdk_dir(&root)).ok()?.toolchain() {
        renzora_plugin_build::Toolchain::ToolchainMissing { version } => {
            Some(ToolchainGap::Installable { version })
        }
        // No rustup, or a `rustc` on `PATH` that is the wrong version. Either
        // way the fix is the same: install rustup, which can then be asked for
        // the pinned compiler.
        renzora_plugin_build::Toolchain::RustupMissing { .. } => Some(ToolchainGap::RustupMissing),
        renzora_plugin_build::Toolchain::Ready(_) => None,
    }
}

/// Add the pinned toolchain through the rustup that is already installed.
///
/// Re-exported so a caller does not have to reach past this module into the SDK
/// crate for the one action the setup window offers.
pub fn install_toolchain(version: &str) -> Result<(), String> {
    renzora_plugin_build::toolchain::install_toolchain(version)
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
    let disabled = renzora::load_disabled_plugins();
    let sdk = Sdk::load(crate::sdk_dir(&root)).ok();
    let native_stamp = sdk.as_ref().map(|s| s.stamp());
    for p in crate::plugin_entries(&root) {
        if disabled.iter().any(|d| d == &name_of(&p)) {
            continue;
        }
        if is_native_source(&p) {
            let Some(sdk) = sdk.as_ref() else { continue };
            if layout(&p, Some(sdk), native_stamp.as_deref()).needs_build {
                return true;
            }
        }
    }
    false
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
            Ok(tree) => {
                // Delete the archive once the tree is in place. Keeping it would
                // hold ~444 MB forever for no benefit: an update replaces the
                // whole install directory, so it is never the source of a repair
                // — a re-extract would come from the new download, not this file.
                //
                // Safe only because it happens AFTER `extract` returned Ok, and
                // `extract` renames the finished tree into place atomically. A
                // failure leaves the archive untouched and retryable.
                //
                // ── Except inside a macOS bundle, where it must NOT be deleted
                // The archive is a sealed resource of a signed `.app` there, and
                // removing it invalidates the signature the same way unpacking
                // into `Contents/` used to. `extract` wrote the tree out to
                // Application Support precisely so the bundle stays untouched,
                // and deleting the archive here would give back everything that
                // bought. It also stops being dead weight: the tree now lives
                // outside the install, so a user who clears Application Support
                // has nothing left to rebuild from except this file.
                //
                // The test is where the tree landed, not the platform — if it is
                // not under `root`, `root` is not ours to write to.
                if tree.starts_with(&root) {
                    if let Err(e) = std::fs::remove_file(&archive) {
                        report(Progress::Failed(format!(
                            "could not remove {}: {e}",
                            archive.display()
                        )));
                    }
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
/// Where `plugin` should actually be compiled.
///
/// Itself, unless it sits somewhere that must not be written to — in which case
/// its source is copied under `writable` and that copy is returned.
///
/// Only the source is copied. Any `build/` in the original is the artefact this
/// call exists to replace, and carrying it over would make the copy look
/// already-built to the very check that decided it was stale.
fn mirror_for_build(plugin: &Path, writable: &Path) -> Result<PathBuf, String> {
    // Already in the writable root — the common case, and every case off macOS.
    if plugin.parent() == Some(writable) {
        return Ok(plugin.to_path_buf());
    }
    let dest = writable.join(name_of(plugin));
    // A previous mirror of an older version would otherwise be merged with the
    // new source, leaving files from both.
    let _ = std::fs::remove_dir_all(&dest);
    copy_source(plugin, &dest).map_err(|e| format!("could not stage for build: {e}"))?;
    Ok(dest)
}

/// Recursive copy of a plugin's source, skipping `build/`.
fn copy_source(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == "build" {
            continue;
        }
        let src = entry.path();
        let dst = to.join(&name);
        if entry.file_type()?.is_dir() {
            copy_source(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

fn build_stale(root: &Path, report: &mut impl FnMut(Progress)) -> usize {
    // Without an SDK there is nothing to compile against, so every plugin is
    // left alone and the setup window says so rather than failing one build at
    // a time.
    let sdk = Sdk::load(crate::sdk_dir(root)).ok();
    let native_stamp = sdk.as_ref().map(|s| s.stamp());
    // The same list the loader will walk, minus the plugins the user switched
    // off — compiling one of those would be work for something that will not run.
    let disabled = renzora::load_disabled_plugins();
    let mut pending: Vec<PathBuf> = Vec::new();
    for p in crate::plugin_entries(root) {
        if disabled.iter().any(|d| d == &name_of(&p)) {
            continue;
        }
        if is_native_source(&p)
            && sdk.is_some()
            && layout(&p, sdk.as_ref(), native_stamp.as_deref()).needs_build
        {
            pending.push(p);
        }
    }
    if pending.is_empty() {
        return 0;
    }

    // Longest first. A plugin with third-party dependencies runs cargo over a
    // whole dependency tree — `text3d` pulls `lyon_tessellation`, `system_monitor`
    // pulls `sysinfo` and `nvml-wrapper` — which takes a minute or so against the
    // ~4 s a single `rustc` needs for everything else.
    //
    // `plugin_entries` returns them alphabetically, which put `system_monitor`
    // and `text3d` near the end: the pool drained around the two slowest and the
    // whole build appeared to hang on its last plugin with 31 cores idle. Nothing
    // was wrong, it was just the worst possible order. Starting the expensive
    // ones first overlaps them with the cheap majority instead.
    //
    // `sort_by_key` is stable, so the alphabetical order survives within each
    // group and the build stays reproducible.
    pending.sort_by_key(|p| !renzora_native_build::deps::has_third_party(p));

    let total = pending.len();
    let built = AtomicUsize::new(0);
    let writable = renzora_plugin_build::install::plugins_write_dir(root);

    // One `rustc` per plugin, several at a time. The builds are independent —
    // separate crates, each writing only into its own `build/` — so the only
    // thing serialising them was the loop.
    //
    // Measured on a 32-core machine at ~4.6 s per plugin: 16 plugins took 70 s
    // sequentially, 20 s at 8-way and 19 s at 16-way. The curve flattens early
    // because `rustc` already threads its own codegen units, so past a handful
    // of processes they are competing for the same cores. The cap is therefore
    // about memory rather than CPU: each `rustc` loads the SDK's 921 metadata
    // files, and fifty of those at once would be a lot of resident set on a
    // machine that has 52 plugins to build.
    let jobs = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .clamp(1, MAX_BUILD_JOBS)
        .min(total);

    // Workers send progress; the caller's `report` stays on this thread, because
    // it is an `FnMut` the caller owns and draws a window with.
    let (tx, rx) = std::sync::mpsc::channel::<Progress>();
    let next = AtomicUsize::new(0);
    // Finished, successfully or not — what the bar measures. `built` counts only
    // the successes, which is what the caller is told at the end.
    let done = AtomicUsize::new(0);

    std::thread::scope(|scope| {
        for _ in 0..jobs {
            let tx = tx.clone();
            let (next, built, pending, writable, sdk, native_stamp) =
                (&next, &built, &pending, &writable, &sdk, &native_stamp);
            let done = &done;
            scope.spawn(move || loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= pending.len() {
                    break;
                }
                let plugin = &pending[i];
                let name = name_of(plugin);
                build_one(plugin, i, total, writable, sdk, native_stamp, built, &tx);
                // Sent whatever the outcome: the bar measures work finished, and
                // a plugin that failed to compile is as finished as one that did
                // not. Reporting only successes would leave the bar short of the
                // end with nothing left to run.
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                let _ = tx.send(Progress::Built { name, done: n, total });
            });
        }
        // The workers hold the only remaining senders, so the drain below ends
        // when the last one finishes. Without this the loop never returns.
        drop(tx);
        for progress in rx {
            report(progress);
        }
    });

    built.load(Ordering::Relaxed)
}

/// The most `rustc` processes to run at once. See the note in [`build_stale`].
const MAX_BUILD_JOBS: usize = 8;

/// Compile one pending plugin, reporting through `tx`.
///
/// Split out of [`build_stale`] only so the worker body is not three levels of
/// closure deep; it is the same sequence the loop used to run inline.
#[allow(clippy::too_many_arguments)]
fn build_one(
    plugin: &Path,
    i: usize,
    total: usize,
    writable: &Path,
    sdk: &Option<Sdk>,
    native_stamp: &Option<String>,
    built: &AtomicUsize,
    tx: &std::sync::mpsc::Sender<Progress>,
) {
    let report = |p: Progress| {
        // The receiver outlives every worker (it is drained inside the same
        // scope), so this only fails if the caller has already gone away.
        let _ = tx.send(p);
    };
    let name = name_of(plugin);
    report(Progress::Building { name: name.clone(), index: i + 1, total });
    {

        // A plugin that shipped inside a macOS `.app` cannot be built where it
        // sits: `build/` would land in `Contents/MacOS/plugins/<name>/` and
        // invalidate the bundle's signature. The write SUCCEEDS — a signed
        // bundle is not read-only — so nothing would report it, and the damage
        // would only surface the next time Gatekeeper assessed the app.
        //
        // So the source is copied to the writable root and built there. On the
        // next scan that copy shadows the bundled original (`plugin_entries`
        // puts the writable root first), which is the same precedence a
        // marketplace install gets, and everything downstream still sees one
        // self-contained plugin directory.
        //
        // Normally dead code on a shipped install: plugins ship prebuilt with a
        // stamp matching the SDK beside them, so nothing goes stale until the
        // engine updates — and an update replaces the whole bundle, stamps
        // included. This is the safety net for when that reasoning is wrong.
        let plugin = &match mirror_for_build(plugin, writable) {
            Ok(p) => p,
            Err(e) => {
                report(Progress::Failed(format!("{name}: {e}")));
                return;
            }
        };
        let expected = native_stamp.clone().unwrap_or_default();
        let l = layout(plugin, sdk.as_ref(), native_stamp.as_deref());
        if let Err(e) = std::fs::create_dir_all(plugin.join("build")) {
            report(Progress::Failed(format!("{name}: {e}")));
            return;
        }
        let mut on_line = |line: &str| {
            // Blank lines are separators in rustc/cargo output, not status. Left
            // in, they render as a bare "name:" with nothing after it, which
            // looks like the build stalled on an unnamed step.
            if line.trim().is_empty() {
                return;
            }
            report(Progress::Compiling {
                name: name.clone(),
                index: i + 1,
                total,
                line: line.to_string(),
            });
        };
        let outcome = sdk
            .as_ref()
            .expect("a plugin is only pushed as pending when the SDK loaded")
            .compile_with(plugin, &l.lib_path, &mut on_line)
            .map_err(|e| e.to_string());
        match outcome {
            Ok(stamp) => match std::fs::write(&l.stamp_path, &stamp) {
                Ok(()) => {
                    built.fetch_add(1, Ordering::Relaxed);
                    // Built: forget any earlier failure, so a later one is not
                    // mistaken for it.
                    let _ = std::fs::remove_file(&l.fail_path);
                }
                Err(e) => report(Progress::Failed(format!("{name}: writing stamp: {e}"))),
            },
            // Reported and skipped. The loader will say it again, in the place a
            // user is looking, once there is a Problems panel to say it in.
            //
            // The failure is also RECORDED, and that part is load-bearing: this
            // pass is what `needed()` asks about, and `main` restarts the process
            // after running it. Retrying a plugin that cannot compile therefore
            // does not merely waste a compile — it asks the same question after
            // the restart, gets the same answer, and reopens this window forever.
            // See `layout` in the crate root.
            Err(e) => {
                let _ = std::fs::write(&l.fail_path, &expected);
                report(Progress::Failed(format!("{name} failed to build: {e}")));
            }
        }
    }
}

/// Relaunch this executable with the same arguments and exit.
///
/// Kept as a name here — `main`'s boot sequence reads better for it — but the
/// implementation moved to the contract crate, because installing a plugin from
/// the marketplace needs the same restart for the same reason and should not
/// have to reach into the prebuild module to get it.
///
/// Desktop-only, following `renzora::restart_process`, which is: a page has no
/// process to relaunch. Both callers already gate the block they use it from,
/// and on the web [`needed`] answers false anyway — there is no SDK beside a
/// bundle and nothing a browser could compile if there were.
#[cfg(not(target_arch = "wasm32"))]
pub fn restart() -> ! {
    renzora::restart_process()
}

#[cfg(test)]
mod mirror_tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    /// A plugin already in the writable root builds where it is — no copy, no
    /// second directory, and the same path back.
    #[test]
    fn a_writable_plugin_is_left_alone() {
        let root = tmp("renzora_mirror_noop");
        let writable = root.join("plugins");
        let plugin = writable.join("clouds");
        std::fs::create_dir_all(plugin.join("src")).unwrap();
        assert_eq!(mirror_for_build(&plugin, &writable).unwrap(), plugin);
    }

    /// A bundled plugin is copied out, so the build never writes into the
    /// directory it shipped in.
    #[test]
    fn a_bundled_plugin_is_mirrored_out() {
        let root = tmp("renzora_mirror_out");
        let bundled = root.join("Contents/MacOS/plugins/clouds");
        std::fs::create_dir_all(bundled.join("src")).unwrap();
        std::fs::write(bundled.join("Cargo.toml"), "[package]").unwrap();
        std::fs::write(bundled.join("src/lib.rs"), "// source").unwrap();
        // The stale artefact this rebuild exists to replace.
        std::fs::create_dir_all(bundled.join("build")).unwrap();
        std::fs::write(bundled.join("build/stamp.txt"), "old").unwrap();

        let writable = root.join("data/plugins");
        let out = mirror_for_build(&bundled, &writable).unwrap();

        assert_eq!(out, writable.join("clouds"));
        assert!(out.join("Cargo.toml").is_file(), "manifest copied");
        assert!(out.join("src/lib.rs").is_file(), "sources copied recursively");
        assert!(
            !out.join("build").exists(),
            "the stale build must NOT come along — it would look already-built"
        );
        // The original is untouched, which is the whole point.
        assert!(bundled.join("build/stamp.txt").is_file());
    }

    /// Re-mirroring replaces rather than merges, or files from an older version
    /// would survive alongside the new ones.
    #[test]
    fn re_mirroring_replaces_the_previous_copy() {
        let root = tmp("renzora_mirror_replace");
        let bundled = root.join("Contents/MacOS/plugins/clouds");
        std::fs::create_dir_all(&bundled).unwrap();
        std::fs::write(bundled.join("new.rs"), "new").unwrap();

        let writable = root.join("data/plugins");
        std::fs::create_dir_all(writable.join("clouds")).unwrap();
        std::fs::write(writable.join("clouds/stale.rs"), "old").unwrap();

        let out = mirror_for_build(&bundled, &writable).unwrap();
        assert!(out.join("new.rs").is_file());
        assert!(!out.join("stale.rs").exists(), "leftovers from the old copy must go");
    }
}
