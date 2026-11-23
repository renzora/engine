//! GPU-pass → driving-component registry.
//!
//! The editor's "GPU Pass Breakdown" reads per-pass GPU timings from Bevy's
//! `RenderDiagnosticsPlugin`. Those are render-graph *node names* with no link
//! back to the ECS — so on their own they tell you a pass cost 2ms, not *what*
//! made it run. This registry supplies that link: each entry says "passes whose
//! name starts with `prefix` are driven by entities carrying component `C`", and
//! the breakdown counts the live entities of that component at display time.
//!
//! Crucially this is **not hardcoded** to the engine's own passes. Any plugin —
//! first- or third-party — that adds its own GPU render pass registers the
//! association via [`AppEditorExt::register_gpu_pass_source`], so its pass shows
//! up attributed in the breakdown without the debugger knowing about it. The
//! engine registers its built-ins (environment maps, shadow-casting lights, …)
//! the exact same way.

use bevy::ecs::component::ComponentId;
use bevy::prelude::*;
use std::borrow::Cow;

/// One attribution rule: render passes whose name starts with `prefix` are
/// driven by entities carrying the component identified by `component`.
#[derive(Clone)]
pub struct GpuPassSource {
    /// A pass matches this rule if its render-graph name starts with this.
    pub prefix: Cow<'static, str>,
    /// The component whose live entity count is shown as the pass's driver.
    pub component: ComponentId,
    /// Singular noun for one driver, e.g. "environment map" / "directional light".
    pub noun: Cow<'static, str>,
}

/// Registry of [`GpuPassSource`] rules, consumed by the editor's GPU Pass
/// Breakdown. Populated via [`AppEditorExt::register_gpu_pass_source`].
#[derive(Resource, Default)]
pub struct GpuPassSourceRegistry {
    pub entries: Vec<GpuPassSource>,
}

impl GpuPassSourceRegistry {
    pub fn register(&mut self, entry: GpuPassSource) {
        self.entries.push(entry);
    }

    /// The first registered rule whose `prefix` matches `pass`, if any. Register
    /// more specific prefixes before broader ones if they could overlap.
    pub fn lookup(&self, pass: &str) -> Option<&GpuPassSource> {
        self.entries
            .iter()
            .find(|e| pass.starts_with(e.prefix.as_ref()))
    }
}
