//! One-way events the editor fires and other crates observe.
//!
//! Every one of these exists so a dependency does not: the editor asks for
//! physics to pause, for the scene to be snapshotted, for autoload scenes to
//! load, without linking the crate that does any of it. The observers live in
//! `renzora_physics` and `renzora_engine`; the events live here so both sides
//! resolve one `TypeId` across the dlopen boundary.

/// Sent by the editor to request pausing the physics simulation.
#[derive(bevy::prelude::Event)]
pub struct PausePhysics;

/// Sent by the editor to request unpausing the physics simulation.
#[derive(bevy::prelude::Event)]
pub struct UnpausePhysics;

/// Sent by the editor to request resetting all script runtime states.
#[derive(bevy::prelude::Event)]
pub struct ResetScriptStates;

/// Notification that scripts were hot-reloaded. The scripting crate triggers
/// this so the editor can show toast notifications without importing scripting.
#[derive(bevy::prelude::Event)]
pub struct ScriptsReloaded {
    pub names: Vec<String>,
}

/// Outcome of a mid-session plugin hot-load attempt — a `.dll`/`.so`/`.dylib`
/// dropped into the `plugins/` directory while the app is running.
///
/// The dynamic plugin loader builds the plugin into the live `World` via a
/// temporary `App` that borrows the running world, so any plugin that only
/// touches the **main** world (gameplay, components, resources, systems, UI)
/// activates on the next frame. A plugin that also targets the **render** world
/// (post-process effects, custom render-graph nodes) can't be wired into the
/// already-initialized renderer at runtime and needs a restart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotLoadOutcome {
    /// Built fully into the live world — active next frame.
    Loaded,
    /// Loaded as far as the main world allows, but the plugin also targets the
    /// render world, which can't be hot-wired. Restart to take full effect.
    NeedsReload,
    /// Not loaded (wrong scope for this host, incompatible ABI, or a plugin
    /// with the same name is already loaded — restart to replace it).
    Skipped,
    /// The plugin's `build` panicked or its entry symbol was missing.
    Failed,
}

/// Fired once per hot-load attempt by the dynamic plugin loader. Defined in the
/// shared `renzora` dylib so the binary-side loader that triggers it and the
/// editor-bundle observer that turns it into a toast resolve one `TypeId`
/// across the dlopen boundary (mirrors [`ScriptsReloaded`]). The runtime, which
/// has no toast UI, simply ignores it (the loader also logs every outcome).
#[derive(bevy::prelude::Event, Clone, Debug)]
pub struct HotPluginNotice {
    /// The plugin's file stem (e.g. `my_cool_effect`).
    pub id: String,
    /// What happened.
    pub outcome: HotLoadOutcome,
    /// A human-readable message suitable for a toast.
    pub message: String,
}

/// Sent by the editor to save the current scene before play mode.
#[derive(bevy::prelude::Event)]
pub struct SaveCurrentScene;

/// Snapshot the live scene into an in-memory buffer before entering Simulate
/// mode, so [`RestoreSimulateSnapshot`] can revert every mutation the simulation
/// makes (moved bodies, ragdoll pose, spawned/despawned entities) on Stop.
/// Observed by `renzora_engine` (which owns scene (de)serialization); the editor
/// only fires the event so the dependency direction stays one-way.
#[derive(bevy::prelude::Event)]
pub struct SnapshotSceneForSimulate;

/// Restore the scene captured by [`SnapshotSceneForSimulate`] when leaving
/// Simulate mode. A no-op if no snapshot was taken.
#[derive(bevy::prelude::Event)]
pub struct RestoreSimulateSnapshot;

/// Load the project's global (autoload) scenes so an editor play session sees
/// the same persistent HUD / music / networking content a shipped game does.
///
/// A game build loads these once at `Startup`; the editor has no such moment,
/// so Play fires this and Stop fires [`UnloadAutoloadScenes`]. Observed by
/// `renzora_engine` (which owns scene loading) — the editor only fires it, so
/// the dependency stays one-way, the same arrangement as
/// [`SnapshotSceneForSimulate`].
#[derive(bevy::prelude::Event)]
pub struct LoadAutoloadScenes;

/// Despawn everything [`LoadAutoloadScenes`] spawned, on Stop.
///
/// Teardown is by recorded entity id, not by "despawn all `Persistent`" — a
/// user can hand-tag `Persistent` from the inspector, and a blanket sweep would
/// delete authored scene content that merely shares the marker.
#[derive(bevy::prelude::Event)]
pub struct UnloadAutoloadScenes;

/// Fired by the editor immediately after a document tab is closed, with
/// the closed tab's id. Lets per-tab caches (asset handles, undo stacks,
/// etc.) drop their entries without coupling the editor to every
/// downstream consumer.
#[derive(bevy::prelude::Event, Debug, Clone, Copy)]
pub struct TabClosed {
    pub tab_id: u64,
}
