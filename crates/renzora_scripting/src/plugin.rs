#![allow(unused_variables, unused_assignments, dead_code)]

use bevy::prelude::*;
use std::path::PathBuf;

use crate::command::CharacterCommandQueue;
use crate::plugin_bridge::PluginHttpBridge;
use crate::component::ScriptComponent;
use crate::engine::ScriptEngine;
use crate::input::{update_script_input, ScriptInput};
use crate::resources::update_script_timers;
use crate::resources::ScriptTimers;
use crate::systems::execution::{
    ScriptCommandQueue, ScriptEnvironmentCommands, ScriptLogBuffer, ScriptReflectionQueue,
};

/// Events emitted when scripts are hot-reloaded.
#[derive(Resource, Default)]
pub struct ScriptReloadEvents {
    pub reloaded: Vec<String>,
}

/// System sets for ordering scripting systems
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ScriptingSet {
    /// Pre-script systems (input, timers)
    PreScript,
    /// Script execution
    ScriptExecution,
    /// Post-script command processing
    CommandProcessing,
    /// Debug draw
    DebugDraw,
    /// Cleanup
    Cleanup,
}

/// Scripting plugin — registers backends, input collection, script execution,
/// and command processing systems.
pub struct ScriptingPlugin {
    /// Path to the scripts folder
    pub scripts_folder: Option<PathBuf>,
}

impl ScriptingPlugin {
    pub fn new() -> Self {
        Self {
            scripts_folder: None,
        }
    }

    pub fn with_scripts_folder(mut self, path: impl Into<PathBuf>) -> Self {
        self.scripts_folder = Some(path.into());
        self
    }
}

impl Default for ScriptingPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ScriptingPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] ScriptingPlugin");
        // The engine starts with no backends at all. A language arrives as a
        // plugin — `adopt_plugin_backends` below picks up whatever registered —
        // so a build with no language plugin present simply runs no scripts,
        // rather than carrying an interpreter it may never use.
        let mut engine = ScriptEngine::new();

        if let Some(ref folder) = self.scripts_folder {
            engine.set_scripts_folder(folder.clone());
        }

        app.insert_resource(engine)
            .init_resource::<ScriptInput>()
            .init_resource::<ScriptTimers>()
            .init_resource::<ScriptCommandQueue>()
            .init_resource::<renzora::ScriptDrawBuffer>()
            .init_resource::<renzora::ScriptDrawSurfaces>()
            .init_resource::<CharacterCommandQueue>()
            .init_resource::<renzora::TransformWriteQueue>()
            .init_resource::<ScriptLogBuffer>()
            .init_resource::<ScriptEnvironmentCommands>()
            .init_resource::<ScriptReflectionQueue>()
            .init_resource::<ScriptReloadEvents>()
            .init_resource::<ScriptsActive>()
            .init_resource::<crate::extension::ScriptExtensions>()
            .init_resource::<crate::get_handler::AssetProgressBridge>()
            .init_resource::<crate::get_handler::SceneLoadBridge>()
            .init_resource::<renzora::ScriptSceneInbox>()
            .init_resource::<renzora::GameEventQueue>()
            .init_resource::<crate::perf::ScriptPerfStats>()
            .init_resource::<crate::http::HttpInbox>()
            // The C-ABI surface: standalone plugins issuing HTTP requests.
            .add_plugins(PluginHttpBridge)
            .register_type::<ScriptComponent>()
            // Configure system set ordering
            .configure_sets(
                Update,
                (
                    ScriptingSet::PreScript,
                    ScriptingSet::ScriptExecution,
                    ScriptingSet::CommandProcessing,
                    ScriptingSet::DebugDraw,
                    ScriptingSet::Cleanup,
                )
                    .chain(),
            )
            // Pre-script systems (always run — input collection is cheap)
            .add_systems(
                Update,
                (
                    update_scripts_active,
                    update_script_input,
                    update_script_timers,
                )
                    .in_set(ScriptingSet::PreScript),
            )
            // Script execution — only when scripts should run
            .add_systems(
                Update,
                crate::systems::run_scripts
                    .in_set(ScriptingSet::ScriptExecution)
                    .run_if(scripts_should_run),
            )
            // Command processing — only when scripts should run
            .add_systems(
                Update,
                crate::systems::apply_script_commands
                    .in_set(ScriptingSet::CommandProcessing)
                    .run_if(scripts_should_run),
            )
            // Reflection-based component writes — exclusive system, runs after commands
            .add_systems(
                Update,
                crate::systems::apply_reflection_sets
                    .after(ScriptingSet::CommandProcessing)
                    .run_if(scripts_should_run),
            )
            // Adopt any language a plugin registered. Runs in PreScript so a
            // backend is live before the first hook of the same frame.
            .add_systems(
                Update,
                crate::plugin_backend::adopt_plugin_backends.in_set(ScriptingSet::PreScript),
            )
            // Sync scripts folder from CurrentProject
            .add_systems(Update, sync_scripts_folder.in_set(ScriptingSet::PreScript))
            // Hot-reload: check for modified script files
            .add_systems(
                Update,
                check_script_hot_reload.in_set(ScriptingSet::PreScript),
            );

        // Listen for editor reset-script-states event (Observer pattern)
        app.add_observer(handle_reset_script_states);

        // Tell backends when a scripted entity goes away. A backend keeps a VM
        // per (entity, script) so a script's globals are per-entity state, and
        // without this that map grows for the life of the process — which is
        // what happened before, since the in-tree interpreter's `evict_entity`
        // existed and nothing ever called it.
        app.add_systems(Update, evict_despawned_scripts.in_set(ScriptingSet::Cleanup));

        // Bridge blueprint lifecycle/cursor ScriptActions (the interpreter only
        // emits ScriptActions; despawn + cursor lock would otherwise be no-ops).
        app.add_observer(handle_blueprint_lifecycle_actions);
    }
}

/// Bridge blueprint-emitted `ScriptAction`s that have no other handler:
/// `despawn_self`, `despawn` (target by name, empty = self), and
/// `lock_cursor`/`unlock_cursor`. Text scripts reach these via `ScriptCommand`;
/// the blueprint interpreter can only emit `ScriptAction`s, so it routes here.
fn handle_blueprint_lifecycle_actions(
    trigger: On<renzora::ScriptAction>,
    mut commands: Commands,
    names: Query<(Entity, &Name)>,
    mut cursor: Query<&mut bevy::window::CursorOptions>,
) {
    let action = trigger.event();
    match action.name.as_str() {
        "despawn_self" => {
            if let Ok(mut e) = commands.get_entity(action.entity) {
                e.despawn();
            }
        }
        "despawn" => {
            let target = match &action.target_entity {
                Some(n) => names.iter().find(|(_, nm)| nm.as_str() == n).map(|(e, _)| e),
                None => Some(action.entity),
            };
            if let Some(t) = target {
                if let Ok(mut e) = commands.get_entity(t) {
                    e.despawn();
                }
            }
        }
        "lock_cursor" => {
            if let Ok(mut c) = cursor.single_mut() {
                c.grab_mode = bevy::window::CursorGrabMode::Locked;
                c.visible = false;
            }
        }
        "unlock_cursor" => {
            if let Ok(mut c) = cursor.single_mut() {
                c.grab_mode = bevy::window::CursorGrabMode::None;
                c.visible = true;
            }
        }
        _ => {}
    }
}

/// Reset all script runtime states when the editor sends `ResetScriptStates`.
fn handle_reset_script_states(
    _trigger: On<renzora::ResetScriptStates>,
    mut query: Query<&mut ScriptComponent>,
) {
    for mut sc in &mut query {
        for entry in sc.scripts.iter_mut() {
            entry.runtime_state.initialized = false;
            entry.runtime_state.has_error = false;
        }
    }
}

/// Whether scripts should execute this frame — computed once, read by many.
///
/// Four systems gate on this answer (three here, plus `renzora_rust_script`'s
/// `dispatch`), and Bevy evaluates a run condition per system rather than
/// sharing one result. Computing it inside each condition therefore ran the
/// underlying scan four times a frame to reach the same conclusion, so it lives
/// in a resource that [`update_scripts_active`] fills once in
/// `ScriptingSet::PreScript` instead.
#[derive(Resource, Default)]
pub struct ScriptsActive(pub bool);

/// Recompute [`ScriptsActive`] for this frame.
///
/// In the editor: true when `PlayModeState` says scripts are running, or when at
/// least one script is being *previewed* (the inspector's per-script play
/// button) — in which case `run_scripts` runs only the previewing scripts. In a
/// standalone runtime there is no `PlayModeState`, so always.
///
/// The preview scan is the only branch that touches the query, and it is
/// deliberately last: play mode running is answered from the resource alone, so
/// the common in-play case never iterates anything. When play is stopped the
/// scan visits one entity per *scripted* entity — `ScriptComponent` is absent
/// until a script is actually attached (the inspector's always-visible Scripts
/// drawer is UI over an absence; see `renzora_inspector::scripts`), so this
/// stays proportional to the scripts in the scene rather than to its size.
///
/// `pub` because `ScriptingPlugin` is behind the runtime's strippable
/// `scripting` feature while `RustScriptPlugin` is added unconditionally, so a
/// lean export that ships no Lua still has to fill this resource for `.rs`
/// scripts to be gated at all. That plugin re-adds this exact system when it
/// finds itself running without a scripting host — sharing the function rather
/// than copying the rule, which is how the two paths used to drift.
pub fn update_scripts_active(
    mut active: ResMut<ScriptsActive>,
    play_mode: Option<Res<renzora::PlayModeState>>,
    scripts: Query<&ScriptComponent>,
) {
    let running = match play_mode {
        Some(pm) if pm.is_scripts_running() => true,
        Some(_) => scripts
            .iter()
            .any(|sc| sc.scripts.iter().any(|e| e.enabled && e.preview)),
        None => true, // standalone runtime — always run
    };
    // Write only on a real transition: `ResMut` sets the change tick on every
    // `deref_mut`, and this runs every frame.
    if active.0 != running {
        active.0 = running;
    }
}

/// Run condition: scripts should execute this frame. See [`ScriptsActive`].
pub fn scripts_should_run(active: Res<ScriptsActive>) -> bool {
    active.0
}

/// Check all active scripts for file changes and reload if modified.
fn check_script_hot_reload(
    engine: Res<ScriptEngine>,
    mut scripts: Query<&mut ScriptComponent>,
    mut reload_events: ResMut<ScriptReloadEvents>,
    mut commands: Commands,
    mut timer: Local<f32>,
    time: Res<Time>,
) {
    // Only check every 0.5 seconds to avoid hammering the filesystem
    *timer += time.delta_secs();
    if *timer < 0.5 {
        return;
    }
    *timer = 0.0;

    reload_events.reloaded.clear();

    for mut sc in scripts.iter_mut() {
        // Read through `Mut`'s immutable `Deref` first and bail before ever
        // touching `DerefMut`. Nothing is stale the overwhelming majority of the
        // time, and `deref_mut` sets the component's change tick whether or not
        // the write that follows changes anything — so the obvious
        // `for entry in sc.scripts.iter_mut()` marked *every* `ScriptComponent`
        // in the scene as `Changed` twice a second, forever.
        //
        // That is not a small waste. `renzora_hierarchy`'s `AssetBadgeChanges`
        // watches `Changed<ScriptComponent>` to know when a script badge needs
        // redrawing, so the storm set `HierarchyDirty` on a 0.5 s cadence and
        // forced `update_hierarchy_cache` to run `build_entity_tree` — a
        // full-world scan on an exclusive system — at 2 Hz forever, in a scene
        // where nothing had changed. The cost is O(entities in the world), not
        // O(scripts), so it hurt most exactly where it was least deserved.
        //
        // `needs_reload` takes `&self` and stays true until `reload` runs, so
        // re-asking below for the rare stale entry is free of side effects.
        let any_stale = sc.scripts.iter().any(|entry| {
            entry.enabled
                && entry
                    .script_path
                    .as_ref()
                    .is_some_and(|path| engine.needs_reload(path))
        });
        if !any_stale {
            continue;
        }

        for entry in sc.scripts.iter_mut() {
            let Some(ref path) = entry.script_path else {
                continue;
            };
            if !entry.enabled {
                continue;
            }

            if engine.needs_reload(path) {
                let display_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                match engine.reload(path) {
                    Ok(_) => {
                        entry.runtime_state.initialized = false;
                        entry.runtime_state.has_error = false;
                        reload_events.reloaded.push(display_name);
                        info!("[Scripting] Hot-reloaded: {}", path.display());
                    }
                    Err(e) => {
                        warn!(
                            "[Scripting] Hot-reload failed for {}: {}",
                            path.display(),
                            e
                        );
                    }
                }
            }
        }
    }

    // Notify the editor (or any observer) about reloaded scripts
    if !reload_events.reloaded.is_empty() {
        let names = reload_events.reloaded.clone();
        commands.queue(move |world: &mut World| {
            world.trigger(renzora::ScriptsReloaded { names });
        });
    }
}

/// Tracks whether we've already synced the scripts folder for the current project.
#[derive(Resource, Default)]
struct ScriptsFolderSynced(Option<PathBuf>);

/// System that sets the scripts folder on the engine when a project is loaded.
fn sync_scripts_folder(
    project: Option<Res<renzora::CurrentProject>>,
    mut engine: ResMut<ScriptEngine>,
    mut synced: Local<Option<PathBuf>>,
) {
    let current_path = project.as_ref().map(|p| p.path.clone());
    if *synced == current_path {
        return; // already synced
    }
    *synced = current_path.clone();
    if let Some(path) = current_path {
        info!("[Scripting] Scripts folder set to: {:?}", path);
        engine.set_scripts_folder(path);
    }
}

// No entity gets a `ScriptComponent` automatically.
//
// There used to be an `Insert, Name` observer here that gave one to every named
// entity that wasn't a `bevy_ui` node, so that naming a thing in the editor was
// enough to attach a script to it. That convenience cost more than it was worth:
// each insert is a deferred *archetype move* (the entity's whole component set
// is copied to a new table), it doubled the archetype count for the populations
// it touched, and it left the executor's `&ScriptComponent` query walking
// hundreds of entities that had no scripts on them at all. The observer had
// already been narrowed once, to exclude editor chrome — ~955 empty components
// on an empty scene — which is the shape of the problem.
//
// The component is now only ever inserted deliberately, and every path that
// needs one creates it on demand: the inspector's Scripts entry (`add_fn` in
// `renzora_scripting_editor`), dropping a script or blueprint file onto an
// entity (`renzora_hierarchy::native::asset_drop`), creating a script from the
// hierarchy's New Asset menu, saving a blueprint graph
// (`renzora_blueprint_editor::graph_panel`), and authored game UI, which needs
// one for `<input bind="Entity.var">` to resolve against
// (`renzora_ember::game_ui`).

/// Drop backend state for entities whose `ScriptComponent` has gone.
///
/// The `still_scripted` check is load-bearing, not defensive. `run_scripts`
/// **takes** the `ScriptComponent` off the entity for the duration of the run
/// and re-inserts it at the end, so it can use `&mut World` freely — and a
/// `take` registers as a removal. Without the check this fires for every
/// scripted entity every frame and evicts the VM that is still in use, which
/// resets any state a script keeps in a global: an accumulator stops
/// accumulating, and the entity looks frozen while still responding to
/// inspector edits.
///
/// `run_scripts` is exclusive, so its re-insert has already landed by the time
/// this runs in `Cleanup` — an entity that still has the component is one the
/// executor merely borrowed.
fn evict_despawned_scripts(
    mut removed: RemovedComponents<ScriptComponent>,
    still_scripted: Query<(), With<ScriptComponent>>,
    engine: Res<ScriptEngine>,
) {
    for entity in removed.read() {
        if still_scripted.get(entity).is_err() {
            engine.evict_entity(entity.to_bits());
        }
    }
}
