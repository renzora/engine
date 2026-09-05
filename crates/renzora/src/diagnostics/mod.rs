//! Editor diagnostics: what the debug panels read.
//!
//! These types live in the contract crate for the same reason `AudioLink` does.
//! The panels that draw them are moving out of the workspace and into a native
//! plugin, and a native plugin links `bevy`, `renzora` and `renzora_ember` and
//! nothing else (`Externs` in `renzora_plugin_build`). A panel reading
//! `renzora_shader::material::perf::MaterialPerfStats` could never be a plugin,
//! however small the type was.
//!
//! So the *vocabulary* is here and the *collection* stays where the work
//! happens: `renzora_shader` still times material compiles and
//! `renzora_scripting` still times script hooks, they just write into a
//! resource both sides can name. Each re-exports its old path, so nothing that
//! referenced these types had to change.
//!
//! # Counts, not machinery
//!
//! The material and scripting panels read `MaterialCache` and `ScriptEngine`
//! today, and neither of those could move: one owns the compiled-material cache
//! and the other owns an interpreter. Reading them showed the panels only ever
//! wanted *counts* off both, so what crosses into this crate is
//! [`MaterialCacheCounts`] and [`ScriptInventory`], published by their owners.
//! That is a much smaller contract than the machinery, and it keeps the cache
//! and the engine free to change shape without touching a plugin.
//!
//! # Cost
//!
//! Behind the `diagnostics` feature, which `renzora_shader` and
//! `renzora_scripting` both enable. That is deliberately status-quo-preserving:
//! these types were already compiled into any build carrying either of those
//! crates, and a lean export that strips both still carries none of this.

use bevy::prelude::*;

/// Per-material compile and resolve timing. Populated by `renzora_shader`.
pub mod material;
/// Per-script execution timing. Populated by `renzora_scripting`.
pub mod script;

/// How many materials of each kind the resolver is holding.
///
/// Published by `renzora_shader` rather than read off `MaterialCache` directly,
/// because the cache is the resolver's own working state and has no business
/// being part of a plugin's ABI. The panel only ever showed these four numbers.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MaterialCacheCounts {
    pub standard: usize,
    pub graph: usize,
    pub code: usize,
    /// Entries in the master-material metadata table.
    pub master_meta: usize,
}

impl MaterialCacheCounts {
    pub fn total(&self) -> usize {
        self.standard + self.graph + self.code
    }
}

/// What the scripting layer is currently carrying.
///
/// Published by `renzora_scripting`, for the same reason as
/// [`MaterialCacheCounts`]: the panel wants a handful of numbers and a path,
/// not the `ScriptEngine`.
#[derive(Resource, Default, Clone, Debug, PartialEq, Eq)]
pub struct ScriptInventory {
    /// Entities carrying at least one script.
    pub entities_with_script: usize,
    /// Total script attachments across those entities. Higher than
    /// `entities_with_script` whenever anything carries more than one.
    pub total_attachments: usize,
    /// Registered language backends.
    pub backend_count: usize,
    /// The project's scripts folder, if a project is open.
    pub scripts_folder: Option<String>,
}
