//! Host side of the `renzora_plugin` C ABI.
//!
//! Counterpart to `renzora_plugin::sys`. Builds the function table, hands it to
//! a plugin's `renzora_plugin_init`, and turns whatever the plugin registers
//! into real Bevy components and systems.
//!
//! ## Why this is a separate path from `dynamic_plugin_loader`
//!
//! That crate loads `dylib` plugins which share `bevy_dylib` with the host and
//! get `&mut App` directly. It requires the plugin to have been compiled in the
//! same environment as the engine — same rustc, same Bevy feature set, same
//! `bevy_dylib-<hash>` — which is why third-party prebuilts are so painful.
//!
//! This path has no such requirement: the plugin links nothing, exports one
//! symbol, and receives every capability through a `#[repr(C)]` function table.
//! The two mechanisms coexist; in-tree engine plugins keep using the old one.
//!
//! ## The interesting part: dynamic systems that still run in parallel
//!
//! A plugin declares its query up front (`sys::QueryDesc`). We turn that into a
//! real Bevy query with `QueryParamBuilder`, so the resulting system carries
//! proper component access and the multi-threaded executor can schedule it
//! against everything else. The alternative — giving plugins open-ended `&mut
//! World` access — would force every plugin system to be exclusive and
//! serialise the whole schedule. See [`query`].
//!
//! ## Layout of this module
//!
//! - [`iface`] — the `Interface` table and the registration calls
//! - [`query`] — the query planner, the dispatcher, and cell marshalling
//! - [`commands`] — everything a running system can queue, poll or read
//! - [`assets`], [`render`] — what a plugin creates, held for its owning crate
//! - [`reload`] — hot-reload: layout checks, retirement, generation gating
//! - [`schema`] — what the host records about plugin types, and name lookup

/// The live-rebuild source watcher. Desktop-only: it drives `cargo` over plugin
/// source and watches the filesystem for saves, and a browser has neither. Its
/// `notify` dependency is target-scoped to match (see `Cargo.toml`), so on wasm
/// the module and the crate that backs it both disappear rather than shipping a
/// file watcher into a page.
#[cfg(not(target_arch = "wasm32"))]
pub mod dev;
pub mod input;
pub mod loader;

mod assets;
mod commands;
mod iface;
mod query;
mod reload;
mod render;
mod schema;

use bevy::prelude::*;

use crate::sys;

use iface::IFACE;
use reload::{GenGate, HostCtx};

// One flat public seam, unchanged by the split — every name here was a
// top-level item of this file before.
pub use assets::{CustomMaterialApplier, MaterialSlot, PluginAssets};
pub use commands::{
    discard_unhandled_service_calls, HostCommandSink, PluginHttpInbox, PluginHttpResponse,
    PluginServiceCalls, PluginServiceReplies, ServiceCall, ServiceReply,
};
pub use reload::{retire_slot, PluginGeneration};
pub use render::{
    BsnSpawner, PendingMaterial, PendingMaterials, PendingPostProcess, PendingPostProcesses,
    PendingRenderPass, PendingRenderPasses, PluginAudioBackend, PluginAudioBackendEntry,
    PluginNetBackend, PluginNetBackendEntry, PluginPanel, PluginPanels, PluginScriptBackend,
    PluginScriptBackends, RenderCallCtx,
};
pub use schema::{
    expose_component_data, expose_component_data_mut, HostDataComponents, PluginComponentInfo,
    PluginComponentSchemas, PluginComponents, PluginField, PluginResources,
};

/// The host's interface table.
///
/// A `'static` pointer, so a caller outside the system dispatch — a panel
/// action, say — can hand it to a plugin without owning one.
pub fn interface() -> *const sys::Interface {
    &IFACE
}

// ── Loading ──────────────────────────────────────────────────────────────────

/// Call a freshly-`dlopen`'d plugin's init function.
///
/// The library handle must outlive the process: every function pointer the
/// plugin registered points into it, so dropping it would leave the schedule
/// holding dangling entries. (Unloading safely needs a registration ledger and
/// a teardown pass — a separate piece of work.)
pub fn init_plugin(world: &mut World, init: sys::ExtensionInit) -> sys::InitResult {
    init_plugin_gen(world, init, PluginGeneration::default(), 0, usize::MAX)
}

/// Initialise a plugin as a numbered reload of a slot.
///
/// `counter`/`generation` are the slot's shared reload counter and the value it
/// holds for this load. Every system registered during this call captures them and
/// retires itself once the counter moves on — see `GenGate`.
pub fn init_plugin_gen(
    world: &mut World,
    init: sys::ExtensionInit,
    counter: PluginGeneration,
    generation: u32,
    slot: usize,
) -> sys::InitResult {
    // MUST be the `'static` table, not `interface()`. A plugin stores this
    // pointer so its render callbacks can reach the interface on later frames;
    // handing it a stack local leaves it dangling the moment this returns, and
    // the next `render_set_pipeline` reads a garbage function pointer. Systems
    // were unaffected because they get their interface from `SystemCall::iface`.
    let mut ctx = HostCtx {
        world,
        gate: GenGate {
            counter,
            at: generation,
        },
        slot,
        layout_conflict: false,
    };
    let result = unsafe { init(&IFACE, (&mut ctx as *mut HostCtx).cast()) };
    // A layout change is only discoverable once the plugin registers, i.e. part
    // way through init. Reporting failure here is what makes it a no-op: the
    // loader leaves the generation counter alone, so this build's systems are
    // permanently stale and the previous build carries on running.
    if ctx.layout_conflict {
        return sys::InitResult::Failed;
    }
    result
}
