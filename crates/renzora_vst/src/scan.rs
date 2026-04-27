//! Background scan of standard CLAP plugin paths.
//!
//! The scan is intentionally cheap: it walks each known root, collects every
//! file/bundle whose name ends with `.clap`, and sends the results back to
//! the main thread via a one-shot channel. Once `clack-host` is enabled we
//! will load each candidate inside a `catch_unwind` to read its plugin
//! factory; today we only stat the file.

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;
use std::sync::Mutex;
use std::thread;

use bevy::prelude::*;

use crate::registry::{PluginDescriptor, PluginRegistry};

/// The result of a single scan run, ready to be applied to the registry.
pub struct ScanResult {
    pub plugins: Vec<PluginDescriptor>,
    pub roots: Vec<PathBuf>,
}

/// Resource owning the receiver half of the scan worker channel. Created on
/// startup; a fresh `(sender, receiver)` is made on every scan kick-off.
#[derive(Resource)]
pub struct ScanChannel {
    pub rx: Mutex<Option<Receiver<ScanResult>>>,
}

impl Default for ScanChannel {
    fn default() -> Self {
        Self { rx: Mutex::new(None) }
    }
}

/// Standard CLAP plugin search paths per platform. Mirrors the convention
/// used by `clack-finder`; reproduced here so we don't pull a git-only dep
/// just for the path list.
pub fn standard_clap_paths() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if cfg!(windows) {
        if let Some(p) = std::env::var_os("COMMONPROGRAMFILES") {
            roots.push(PathBuf::from(p).join("CLAP"));
        }
        if let Some(p) = std::env::var_os("LOCALAPPDATA") {
            roots.push(PathBuf::from(p).join("Programs").join("Common").join("CLAP"));
        }
    } else if cfg!(target_os = "macos") {
        if let Some(home) = home_dir() {
            roots.push(home.join("Library/Audio/Plug-Ins/CLAP"));
        }
        roots.push(PathBuf::from("/Library/Audio/Plug-Ins/CLAP"));
    } else {
        // Linux + other unix
        if let Some(home) = home_dir() {
            roots.push(home.join(".clap"));
        }
        roots.push(PathBuf::from("/usr/lib/clap"));
        roots.push(PathBuf::from("/usr/local/lib/clap"));
    }
    roots
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

/// Walk the standard plugin paths in a worker thread and send a
/// [`ScanResult`] back when done.
pub fn start_initial_scan(mut commands: Commands, registry: ResMut<PluginRegistry>) {
    let (tx, rx) = std::sync::mpsc::channel::<ScanResult>();
    commands.insert_resource(ScanChannel { rx: Mutex::new(Some(rx)) });

    let flag = registry.scan_in_progress.clone();
    flag.store(true, Ordering::Relaxed);
    let roots = standard_clap_paths();

    thread::Builder::new()
        .name("clap-scan".into())
        .spawn(move || {
            let mut found = Vec::new();
            for root in &roots {
                walk(root, &mut found);
            }
            let plugins: Vec<PluginDescriptor> = found
                .into_iter()
                .map(PluginDescriptor::from_path)
                .collect();
            let _ = tx.send(ScanResult {
                plugins,
                roots,
            });
            flag.store(false, Ordering::Relaxed);
        })
        .ok();
}

/// Drain any completed scan results into the registry.
pub fn poll_scan_results(
    channel: Option<Res<ScanChannel>>,
    mut registry: ResMut<PluginRegistry>,
) {
    let Some(channel) = channel else { return };
    let Ok(mut slot) = channel.rx.lock() else { return };
    let Some(rx) = slot.as_ref() else { return };

    match rx.try_recv() {
        Ok(result) => {
            info!(
                "[plugins] Scan complete: {} plugin(s) across {} root(s)",
                result.plugins.len(),
                result.roots.len(),
            );
            registry.set_all(result.plugins, result.roots);
            // One-shot: drop the receiver so we don't keep polling.
            *slot = None;
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {}
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            // Worker exited without sending — empty result.
            *slot = None;
        }
    }
}

/// Recursively walk a directory looking for `.clap` files / bundles.
/// On macOS a `.clap` is a directory bundle; on Windows / Linux it's a file.
/// We treat both uniformly: anything whose basename ends with `.clap`.
fn walk(root: &std::path::Path, out: &mut Vec<PathBuf>) {
    let Ok(read) = std::fs::read_dir(root) else { return };
    for entry in read.flatten() {
        let path = entry.path();
        let is_clap = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("clap"))
            .unwrap_or(false);
        if is_clap {
            out.push(path);
            continue;
        }
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            // Bound depth: stop recursing once we're past plausible vendor
            // subfolders. CLAP plugins typically live one or two levels
            // deep at most.
            walk(&path, out);
        }
    }
}
