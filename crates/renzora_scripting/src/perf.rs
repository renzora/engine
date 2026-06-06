//! Per-script execution timing.
//!
//! Populated by the `run_scripts` system as it calls each lifecycle
//! hook (`on_ready`, `on_update`, `on_rpc`, `on_ui`). Each call's
//! duration is recorded against the script path, with rolling averages
//! so the debugger panel can show "which scripts are expensive this
//! frame" without dragging in a profiler.
//!
//! Cost: each hook gains a `Instant::now()` before / `elapsed()` after,
//! plus a HashMap insert. Sub-microsecond compared to script execution,
//! so always-on is fine.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::prelude::*;

/// How many recent on_update durations to keep per script for the
/// rolling average. 60 frames ≈ 1s at 60Hz — long enough that a
/// one-frame spike doesn't drown out the typical cost, short enough
/// to follow real perf changes.
pub const ROLLING_WINDOW: usize = 60;

/// Recorded performance for one script, identified by its resolved
/// filesystem path.
#[derive(Default, Clone, Debug)]
pub struct ScriptPerf {
    /// Most recent `on_update` duration. The headline number — what
    /// the debugger panel shows as "current cost".
    pub last_on_update: Duration,
    /// Rolling-window average of `on_update`. Smooths over single-frame
    /// noise (GC pauses, JIT-style hot paths) so trends are readable.
    pub avg_on_update: Duration,
    /// All-time worst `on_update`. Spikes that have happened — useful
    /// for catching infrequent slow paths.
    pub max_on_update: Duration,
    /// Total number of `on_update` calls observed so far. Combined
    /// with `avg_on_update` this approximates total CPU time spent in
    /// the script.
    pub on_update_calls: u64,

    /// Most recent `on_ready`. Fires once per script-load; useful for
    /// spotting heavy initialization code.
    pub last_on_ready: Duration,
    /// Most recent `on_rpc` (network RPC delivery to the script). Zero
    /// for scripts that don't handle RPCs.
    pub last_on_rpc: Duration,
    /// Most recent `on_ui` (markup-UI callback). Zero for scripts
    /// that don't bind to bevy_hui widgets.
    pub last_on_ui: Duration,

    /// Number of hook calls that returned an error this session.
    /// Bumps every `on_*` failure so the panel can highlight scripts
    /// that are quietly throwing.
    pub error_count: u64,
    /// The most recent error message; cleared on the next successful
    /// call so transient errors fade out.
    pub last_error: Option<String>,

    /// Rolling buffer of recent on_update durations for the average.
    /// `VecDeque` would be nicer but keeping it a Vec keeps the struct
    /// Clone-cheap (no allocator dance).
    pub recent_on_update: Vec<Duration>,

    /// Frame the script was last touched. Lets the panel grey out
    /// "stale" entries for scripts whose owning entity despawned or
    /// got moved to a tab the user isn't viewing.
    pub last_seen_frame: u64,
}

impl ScriptPerf {
    pub fn record_on_update(&mut self, dur: Duration, frame: u64) {
        self.last_on_update = dur;
        self.on_update_calls += 1;
        if dur > self.max_on_update {
            self.max_on_update = dur;
        }
        self.recent_on_update.push(dur);
        if self.recent_on_update.len() > ROLLING_WINDOW {
            self.recent_on_update.remove(0);
        }
        let total: Duration = self.recent_on_update.iter().sum();
        self.avg_on_update = total / self.recent_on_update.len().max(1) as u32;
        self.last_seen_frame = frame;
    }
}

/// Shared store of per-script perf. Lives as a `Resource` so the
/// debugger panel can read it from `&World`. `run_scripts` mutates it
/// each frame around the hook-call sites.
#[derive(Resource, Default)]
pub struct ScriptPerfStats {
    pub per_script: HashMap<PathBuf, ScriptPerf>,
    pub frame: u64,
}

impl ScriptPerfStats {
    /// Snapshot for the debugger panel — sorted by `last_on_update`
    /// descending so the most expensive scripts surface first.
    pub fn snapshot(&self) -> Vec<(PathBuf, ScriptPerf)> {
        let mut out: Vec<(PathBuf, ScriptPerf)> = self
            .per_script
            .iter()
            .map(|(p, s)| (p.clone(), s.clone()))
            .collect();
        out.sort_by_key(|x| std::cmp::Reverse(x.1.last_on_update));
        out
    }

    /// Aggregate stats across all scripts for the panel header.
    pub fn totals(&self) -> ScriptPerfTotals {
        let mut t = ScriptPerfTotals::default();
        for s in self.per_script.values() {
            t.total_last_update += s.last_on_update;
            t.total_avg_update += s.avg_on_update;
            t.total_calls += s.on_update_calls;
            t.total_errors += s.error_count;
            if s.error_count > 0 {
                t.scripts_with_errors += 1;
            }
        }
        t.script_count = self.per_script.len();
        t
    }

    pub(crate) fn record_on_update(
        &mut self,
        path: &Path,
        dur: Duration,
        result: Result<(), &str>,
    ) {
        let frame = self.frame;
        let entry = self.per_script.entry(path.to_path_buf()).or_default();
        entry.record_on_update(dur, frame);
        match result {
            Ok(()) => entry.last_error = None,
            Err(msg) => {
                entry.error_count += 1;
                entry.last_error = Some(msg.to_string());
            }
        }
    }

    pub(crate) fn record_on_ready(
        &mut self,
        path: &Path,
        dur: Duration,
        result: Result<(), &str>,
    ) {
        let frame = self.frame;
        let entry = self.per_script.entry(path.to_path_buf()).or_default();
        entry.last_on_ready = dur;
        entry.last_seen_frame = frame;
        if let Err(msg) = result {
            entry.error_count += 1;
            entry.last_error = Some(msg.to_string());
        }
    }

    pub(crate) fn record_on_rpc(&mut self, path: &Path, dur: Duration) {
        let frame = self.frame;
        let entry = self.per_script.entry(path.to_path_buf()).or_default();
        entry.last_on_rpc = dur;
        entry.last_seen_frame = frame;
    }

    pub(crate) fn record_on_ui(&mut self, path: &Path, dur: Duration) {
        let frame = self.frame;
        let entry = self.per_script.entry(path.to_path_buf()).or_default();
        entry.last_on_ui = dur;
        entry.last_seen_frame = frame;
    }

    pub(crate) fn tick_frame(&mut self) {
        self.frame = self.frame.wrapping_add(1);
    }
}

#[derive(Default, Clone, Copy)]
pub struct ScriptPerfTotals {
    pub script_count: usize,
    pub total_last_update: Duration,
    pub total_avg_update: Duration,
    pub total_calls: u64,
    pub total_errors: u64,
    pub scripts_with_errors: usize,
}
