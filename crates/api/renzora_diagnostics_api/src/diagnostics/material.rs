//! Per-material compile and resolve timing.
//!
//! Populated by `resolve_material_refs` as it walks each
//! `MaterialRef`-bearing entity. Records cache hit / compile counts
//! and the duration of each compile so the debugger's Material
//! Resolver panel can show "what's slow, what's failing, what's
//! getting hit hot".

use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use bevy::prelude::*;

/// Cap on how many recent compile failures to remember. 20 is enough
/// to scroll the panel without bounded growth across a long session.
pub const MAX_RECENT_FAILURES: usize = 20;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum MaterialKind {
    #[default]
    Unknown,
    /// Trivial-fast-path `.material` → plain `StandardMaterial`.
    Standard,
    /// Procedural `.material` → `GraphMaterial` (`ExtendedMaterial`).
    Graph,
    /// `.shader` / `.wgsl` → `CodeShaderMaterial`.
    Code,
}

impl MaterialKind {
    pub fn label(self) -> &'static str {
        match self {
            MaterialKind::Unknown => "?",
            MaterialKind::Standard => "std",
            MaterialKind::Graph => "graph",
            MaterialKind::Code => "code",
        }
    }
}

/// One row of the per-material perf table.
#[derive(Clone, Debug, Default)]
pub struct MaterialPerf {
    pub kind: MaterialKind,
    /// How many entities re-used the cached handle (no recompile).
    pub cache_hits: u64,
    /// How many full compiles ran for this path (cold load + every
    /// `MaterialCache::invalidate` follow-up).
    pub compile_count: u64,
    /// How many compiles returned `None` (parse failure, missing
    /// dependency, etc.).
    pub fail_count: u64,
    /// Most recent successful compile's wall-clock duration. The
    /// headline number for "is this material slow to load".
    pub last_compile: Duration,
    /// All-time worst compile duration for this path.
    pub max_compile: Duration,
    /// Most recent error message, kept until the next successful
    /// compile clears it.
    pub last_error: Option<String>,
}

#[derive(Resource, Default)]
pub struct MaterialPerfStats {
    pub per_path: HashMap<String, MaterialPerf>,
    pub total_cache_hits: u64,
    pub total_compiles: u64,
    pub total_failures: u64,
    pub total_compile_time: Duration,
    /// Newest first.
    pub recent_failures: VecDeque<(String, String)>,
}

impl MaterialPerfStats {
    pub fn record_cache_hit(&mut self, path: &str, kind: MaterialKind) {
        let entry = self.per_path.entry(path.to_string()).or_default();
        if entry.kind == MaterialKind::Unknown {
            entry.kind = kind;
        }
        entry.cache_hits += 1;
        self.total_cache_hits += 1;
    }

    pub fn record_compile_success(&mut self, path: &str, kind: MaterialKind, dur: Duration) {
        let entry = self.per_path.entry(path.to_string()).or_default();
        entry.kind = kind;
        entry.compile_count += 1;
        entry.last_compile = dur;
        if dur > entry.max_compile {
            entry.max_compile = dur;
        }
        entry.last_error = None;
        self.total_compiles += 1;
        self.total_compile_time += dur;
    }

    pub fn record_compile_failure(&mut self, path: &str, dur: Duration, error: String) {
        let entry = self.per_path.entry(path.to_string()).or_default();
        entry.fail_count += 1;
        entry.last_compile = dur;
        entry.last_error = Some(error.clone());
        self.total_failures += 1;
        self.recent_failures.push_front((path.to_string(), error));
        while self.recent_failures.len() > MAX_RECENT_FAILURES {
            self.recent_failures.pop_back();
        }
    }

    /// `(path, perf)` rows sorted by `last_compile` descending. The
    /// debugger panel uses this for its main table.
    pub fn snapshot(&self) -> Vec<(String, MaterialPerf)> {
        let mut out: Vec<(String, MaterialPerf)> =
            self.per_path.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        out.sort_by_key(|b| std::cmp::Reverse(b.1.last_compile));
        out
    }
}
