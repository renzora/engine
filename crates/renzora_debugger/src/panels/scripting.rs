//! Scripting diagnostics state.
//!
//! Surfaces per-script execution timing collected by `renzora_scripting`'s
//! `ScriptPerfStats` resource. Each entry is one script (identified by
//! its filesystem path) showing last/avg/max on_update durations, call
//! count, and the most recent error if any.
//!
//! Rendered by the native (ember) Scripting panel in [`crate::native`].

use std::path::PathBuf;

use renzora_scripting::perf::{ScriptPerf, ScriptPerfTotals};

/// Per-frame snapshot the panel renders from. Updated by
/// `update_scripting_diag_state` in the debugger plugin.
#[derive(bevy::prelude::Resource, Default, Clone)]
pub struct ScriptingDiagState {
    pub entities_with_script: usize,
    pub total_script_attachments: usize,
    pub backend_count: usize,
    pub scripts_folder: Option<String>,
    pub totals: ScriptPerfTotals,
    /// `(path, perf)` sorted by last on_update descending.
    pub per_script: Vec<(PathBuf, ScriptPerf)>,
    pub current_frame: u64,
}
