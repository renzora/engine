//! Hot-reload machinery: what a re-init is allowed to change, what it must
//! take back, and how systems from a previous build retire themselves.
//!
//! Two invariants shape everything here. First, a `ComponentId`'s layout is
//! fixed permanently at registration, so a reload that moved a field would have
//! the plugin writing at new offsets into storage sized for the old struct —
//! hence [`verify_same_layout`], which refuses the whole reload rather than
//! corrupting live data. Second, Bevy cannot remove a system from a schedule,
//! so a retired system stays scheduled and returns immediately — see
//! [`GenGate`].

use bevy::ecs::component::ComponentId;
use bevy::prelude::*;

use crate::sys;

use super::assets::PluginAssets;
use super::render::{
    PluginAudioBackend, PluginNetBackend, PluginPanels, PluginScriptBackends, PendingMaterials,
    PendingPostProcesses, PendingRenderPasses,
};
use super::schema::{PluginComponentInfo, PluginComponentSchemas};

/// What the opaque `sys::Host` pointer actually points at.
///
/// Only valid for the duration of one `renzora_plugin_init` call — we hand the
/// plugin a pointer to a stack value and it may only call back while that frame
/// is live. A plugin that squirrels the pointer away and calls later is using
/// the API wrong; there is nothing we can do to stop it, exactly as with any C
/// API.
pub(crate) struct HostCtx<'w> {
    pub(crate) world: &'w mut World,
    /// Which reload of which plugin is registering. Handed to every system this
    /// init call creates, so a later reload can retire them.
    pub(crate) gate: GenGate,
    /// The plugin's slot index, stamped on everything it registers so
    /// [`retire_slot`] can take it back on the next reload.
    pub(crate) slot: usize,
    /// Set when a reload re-registers a component with a different memory layout
    /// than the live one. Fails the whole init — see `init_plugin_gen`.
    pub(crate) layout_conflict: bool,
}

/// Refuse the reload if `desc` is not byte-compatible with what is already
/// registered under this name; otherwise refresh the names the editor reads.
///
/// Bevy fixes a `ComponentId`'s layout permanently at registration, so a reload
/// that moved or added a field would have the plugin writing at new offsets into
/// storage sized for the old struct. Every live instance would be misread, and
/// nothing would say so.
///
/// Called from BOTH `register_component` and `register_resource`. That is the
/// whole reason it is a function: `register_resource` short-circuits on a known
/// name and never reaches `register_component`, so a guard living only in the
/// latter covered components and quietly missed every resource — which is the
/// worse case, since a resource's storage is a single allocation that a
/// grown struct writes straight off the end of.
///
/// Migrating instead — a second `ComponentId` plus a field-name remap, the way
/// `renzora_bsn::raw_registry` does it for scenes — is the real fix and is worth
/// doing. It is just much larger than making the hazard impossible.
///
/// # Safety
///
/// `desc.fields` must be valid for `desc.field_count` entries.
pub(crate) unsafe fn verify_same_layout(
    ctx: &mut HostCtx,
    existing: ComponentId,
    desc: &sys::ComponentDesc,
    name: &str,
) {
    let Some(stored) = component_info(ctx.world, existing) else {
        return;
    };
    match layout_change(&stored, desc) {
        Some(reason) => {
            let kind = if stored.is_resource { "resource" } else { "component" };
            error!(
                "plugin {kind} `{name}` changed layout on reload ({reason}) — refusing \
                 the reload, since what already holds it was allocated for the old \
                 layout. Restart to pick this up."
            );
            ctx.layout_conflict = true;
        }
        // Byte-compatible, so the data is still valid — but a field may have been
        // renamed or the display name changed, and the editor reads those.
        None => refresh_component_schema(ctx.world, existing, desc),
    }
}

/// The stored schema for a component id, if the host has one.
pub(crate) fn component_info(world: &World, id: ComponentId) -> Option<PluginComponentInfo> {
    world
        .get_resource::<PluginComponentSchemas>()?
        .0
        .iter()
        .find(|i| i.id == id)
        .cloned()
}

/// Why `desc` is not byte-compatible with the live registration, or `None` if it
/// is.
///
/// Compares what actually decides whether existing bytes are still readable:
/// total size, and each field's offset and kind. Field *names* are deliberately
/// not part of this — a rename leaves every byte where it was, so it is a schema
/// refresh rather than a layout change.
///
/// # Safety
///
/// `desc.fields` must be valid for `desc.field_count` entries.
unsafe fn layout_change(
    stored: &PluginComponentInfo,
    desc: &sys::ComponentDesc,
) -> Option<String> {
    if stored.size != desc.size {
        return Some(format!("size {} → {}", stored.size, desc.size));
    }
    let fields = if desc.fields.is_null() {
        &[][..]
    } else {
        std::slice::from_raw_parts(desc.fields, desc.field_count)
    };
    if stored.fields.len() != fields.len() {
        return Some(format!(
            "{} field(s) → {}",
            stored.fields.len(),
            fields.len()
        ));
    }
    for (old, new) in stored.fields.iter().zip(fields) {
        if old.offset != new.offset {
            return Some(format!(
                "field `{}` moved from offset {} to {}",
                old.name, old.offset, new.offset
            ));
        }
        if old.kind != new.kind {
            return Some(format!(
                "field `{}` changed from {} to {}",
                old.name,
                old.kind.name(),
                new.kind.name()
            ));
        }
    }
    None
}

/// Update the names and default of a component whose layout did not change.
///
/// # Safety
///
/// `desc.fields` must be valid for `desc.field_count` entries, and
/// `desc.default_init` (if set) must write `desc.size` bytes.
unsafe fn refresh_component_schema(
    world: &mut World,
    id: ComponentId,
    desc: &sys::ComponentDesc,
) {
    let names: Vec<String> = if desc.fields.is_null() {
        Vec::new()
    } else {
        std::slice::from_raw_parts(desc.fields, desc.field_count)
            .iter()
            .map(|f| f.name.as_str().to_string())
            .collect()
    };
    let display = desc.display_name.as_str().to_string();
    let Some(mut schemas) = world.get_resource_mut::<PluginComponentSchemas>() else {
        return;
    };
    let Some(info) = schemas.0.iter_mut().find(|i| i.id == id) else {
        return;
    };
    for (field, name) in info.fields.iter_mut().zip(names) {
        field.name = name;
    }
    if !display.is_empty() {
        info.display_name = display;
    }
}

/// Drop everything slot `slot` registered, except the things a reload must keep.
///
/// **Kept:** components and resources. Their `ComponentId`s are name-keyed and
/// reload-stable by design, and the data lives in the host's ECS — which is the
/// whole reason hot-reload is tractable here. Retiring them would delete the state
/// the reload exists to preserve.
///
/// **Taken back:** panels, render passes and post-process effects, all of which the
/// new build re-registers. Without this a reload would duplicate them.
///
/// **Not here:** systems. Bevy cannot remove one from a schedule, so they retire
/// themselves by generation instead — see [`GenGate`].
pub fn retire_slot(world: &mut World, slot: usize) {
    if let Some(mut panels) = world.get_resource_mut::<PluginPanels>() {
        panels.0.retain(|p| p.owner != slot);
    }
    if let Some(mut passes) = world.get_resource_mut::<PendingRenderPasses>() {
        passes.0.retain(|p| p.owner != slot);
    }
    if let Some(mut effects) = world.get_resource_mut::<PendingPostProcesses>() {
        effects.0.retain(|e| e.owner != slot);
    }
    if let Some(mut mats) = world.get_resource_mut::<PendingMaterials>() {
        mats.0.retain(|m| m.owner != slot);
    }
    // A retired backend's `entry` points into a library about to be unmapped.
    // Leaving it registered would turn the next `on_update` into a call through
    // a dangling function pointer, so this one is not merely tidy.
    if let Some(mut backends) = world.get_resource_mut::<PluginScriptBackends>() {
        backends.0.retain(|b| b.owner != slot);
    }
    // Same hazard, and worse consequences: an audio backend's entry is called
    // from a frame loop that has no idea the library went away, and `state`
    // points into the unmapped image too. Clearing it takes the game silent
    // until a backend registers again, which is the correct outcome — the
    // alternative is a call through a dangling pointer on the next frame.
    if let Some(mut audio) = world.get_resource_mut::<PluginAudioBackend>() {
        if audio.0.as_ref().is_some_and(|b| b.owner == slot) {
            audio.0 = None;
        }
    }
    // And the network backend, for the same reason. This one has a second
    // hazard the other two do not: threads inside the plugin are mid-transfer
    // and will try to hand back events. `renzora_net` sees the backend vanish
    // and fails every request still waiting on it, which is what stops a
    // background thread blocking forever on an answer that can no longer come.
    if let Some(mut net) = world.get_resource_mut::<PluginNetBackend>() {
        if net.0.as_ref().is_some_and(|b| b.owner == slot) {
            net.0 = None;
        }
    }

    // GPU assets are the one thing that leaks visibly if this is skipped: a
    // reloaded plugin creates a fresh mesh and material every cycle, and
    // `sys.rs` notes the VRAM growth that follows. Dropping the strong handle is
    // enough — `Assets<T>` frees the underlying resource once nothing holds it.
    let assets = world
        .get_resource_mut::<PluginAssets>()
        .map(|mut a| {
            let meshes = std::mem::take(&mut a.meshes);
            let materials = std::mem::take(&mut a.materials);
            (meshes, materials)
        })
        .unwrap_or_default();
    let (meshes, materials) = assets;
    let mut kept_meshes = Vec::new();
    for (owner, handle) in meshes {
        if owner == slot {
            drop(handle);
        } else {
            kept_meshes.push((owner, handle));
        }
    }
    let mut kept_materials = Vec::new();
    for (owner, handle) in materials {
        if owner == slot {
            drop(handle);
        } else {
            kept_materials.push((owner, handle));
        }
    }
    if let Some(mut a) = world.get_resource_mut::<PluginAssets>() {
        a.meshes = kept_meshes;
        a.materials = kept_materials;
    }
}

/// A plugin slot's reload counter, shared between the slot and every system the
/// plugin registered.
///
/// One `Arc` per slot rather than a `World` lookup because a dispatcher checks it
/// on every run: reading an atomic it already owns costs nothing, whereas a
/// resource lookup would mean declaring access the system does not otherwise need
/// and would serialise plugin systems against each other.
pub type PluginGeneration = std::sync::Arc<std::sync::atomic::AtomicU32>;

/// Lets a system tell whether the plugin that registered it has since reloaded.
///
/// Bevy cannot remove a system from a schedule, so a reloaded plugin's old
/// systems stay in it forever. Rather than restructure every registration to live
/// in a swappable sub-schedule — which would force the runner to be exclusive and
/// stop plugin systems parallelising with engine systems in *every* build,
/// reloading or not — a retired system stays scheduled and returns immediately.
///
/// The cost is that a long dev session accumulates no-op systems, each still
/// paying its param fetch. That is a dev-only cost, cleared by a restart, and a
/// shipped game never reloads so it never has one.
#[derive(Clone)]
pub(crate) struct GenGate {
    pub(crate) counter: PluginGeneration,
    /// The counter's value when the capturing system registered.
    pub(crate) at: u32,
}

impl GenGate {
    /// Live only while this system's generation IS the slot's current one.
    ///
    /// The counter is bumped only after init succeeds, so:
    ///
    /// - **Reload succeeded** — counter moves to N. The previous build's systems
    ///   (N-1) go stale, the new build's (N) are live.
    /// - **Reload failed** — counter stays at N-1. The previous build's systems are
    ///   still live and the new build's, which registered at N before the failure
    ///   was known, are stale. That is what keeps a bad reload from running two
    ///   builds at once.
    ///
    /// This was `at < counter` — "stale once the counter moves PAST you" — on the
    /// reasoning that a system registered during init must not be stale before the
    /// bump. It cannot run during init: the whole reload happens inside one
    /// exclusive system, so no frame elapses between registration and the bump. The
    /// asymmetry solved nothing and broke the failure case, leaving a refused
    /// build's systems live alongside the previous build's — two sets of systems,
    /// one of them reading a struct whose layout the host had just rejected.
    pub(crate) fn stale(&self) -> bool {
        self.at != self.counter.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// Run a host interface body, converting a panic into a caller-visible failure.
///
/// Every function in [`crate::sys::Interface`] is `extern "C"`, so a panic
/// inside one cannot unwind and aborts the process instead — the editor dies
/// because a plugin asked for something in a state we did not anticipate. This
/// is the host-side counterpart to the guard the ergonomic layer puts around
/// plugin systems; the boundary is dangerous in both directions and it took a
/// test-suite abort to notice we had only armed one side.
pub(crate) fn guard_host<R>(what: &str, fallback: R, body: impl FnOnce() -> R) -> R {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)) {
        Ok(v) => v,
        Err(_) => {
            error!("[plugin] host call `{what}` panicked — returning a failure to the plugin");
            fallback
        }
    }
}
