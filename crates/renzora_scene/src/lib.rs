//! Renzora Scene — editor-side scene plugin that wires save/load to keybindings and splash state.
//!
//! The actual save/load and rehydration logic lives in `renzora_engine::scene_io`.

use bevy::prelude::*;

use renzora::core::{
    CurrentProject, EditorCamera, HideInHierarchy, MeshInstanceData, NewSceneRequested,
    OpenSceneRequested, OpenScenePathRequested, SaveAsSceneRequested, SaveSceneRequested,
    SceneCamera, SceneTabBuffers, TabSceneSnapshot, TabSwitchRequest, ToggleSettingsRequested,
};
use renzora_camera::OrbitCameraState;
use renzora_editor_framework::SplashState;
use renzora_engine::scene_io;
use renzora_ui::{DocTabKind, DocumentTabState};
use renzora_keybindings::{EditorAction, KeyBindings};
use renzora_shader::material::GraphMaterial;
use renzora_splash::{
    EditorLoadingOverlayActive, LoadingBytes, LoadingTaskHandle, LoadingTasks, TextureLoadProgress,
};
use renzora_viewport::model_flatten::ImportedRoot;

// Re-export so downstream code that was using `renzora_scene::{save_scene, load_scene, ...}` still works.
pub use scene_io::{load_current_scene, load_scene, save_current_scene, save_scene};

mod panel;

mod tab_asset_cache;
pub use tab_asset_cache::TabAssetCache;

mod thumbnail;
use thumbnail::PendingSceneThumbnail;

mod diagnostics;
mod conflict_prompt;
mod hot_reload;
mod missing_assets;
use diagnostics::SceneDiagnostics;

// Runtime warning capture now lives in the shared `renzora` dylib so the
// capture layer (installed by the runtime binary) and this panel (in the
// editor bundle) share one buffer across the dylib boundary. Re-exported
// here so existing `crate::runtime_warnings::*` paths keep resolving.
pub use renzora::runtime_warnings;
pub use renzora::runtime_warnings::runtime_warnings_layer;

// ============================================================================
// Tab Switch System
// ============================================================================

pub(crate) fn despawn_scene_entities(world: &mut World) -> Vec<Entity> {
    let mut to_despawn = Vec::new();
    {
        let mut query = world.query_filtered::<Entity, (
            With<Name>,
            Without<EditorCamera>,
            Without<HideInHierarchy>,
        )>();
        for entity in query.iter(world) {
            // Skip descendants of a `HideInHierarchy` root — that's the bevy_ui
            // editor chrome (the `ShellRoot` carries it) and other editor-internal
            // subtrees, which must survive scene clears.
            if !has_hidden_ancestor(world, entity) {
                to_despawn.push(entity);
            }
        }
    }
    for &entity in &to_despawn {
        if world.get_entity(entity).is_ok() {
            world.despawn(entity);
        }
    }
    // A wholesale scene teardown invalidates every entity id stored in the
    // Scene undo history (new/switch/load/simulate-restore all funnel through
    // here), so drop that stack — undo must not reach across a scene swap into
    // dangling entities. Asset-context stacks (material, other) are untouched;
    // the project-switch teardown clears those explicitly.
    if let Some(mut undo) = world.get_resource_mut::<renzora_undo::UndoStacks>() {
        undo.clear(&renzora_undo::UndoContext::Scene);
    }
    to_despawn
}

/// Whether any ancestor of `e` is marked [`HideInHierarchy`] (editor-internal —
/// chrome, gizmos, previews — that must not be despawned with the scene).
pub(crate) fn has_hidden_ancestor(world: &World, mut e: Entity) -> bool {
    while let Some(parent) = world.get::<ChildOf>(e).map(|c| c.parent()) {
        if world.get::<HideInHierarchy>(parent).is_some() {
            return true;
        }
        e = parent;
    }
    false
}

// ============================================================================
// Simulate-mode scene snapshot / restore
// ============================================================================

/// In-memory snapshot of the scene RON captured when Simulate mode starts, so
/// Stop can revert every mutation the simulation made. `None` until taken.
#[derive(Resource, Default)]
pub struct SimulateSnapshot(pub Option<String>);

/// Serialize the live scene into [`SimulateSnapshot`] before Simulate runs
/// (fired by `enter_simulate_mode`).
fn on_snapshot_scene_for_simulate(
    _trigger: On<renzora::core::SnapshotSceneForSimulate>,
    mut commands: Commands,
) {
    commands.queue(|world: &mut World| match scene_io::serialize_scene_to_string(world) {
        Ok(ron) => world.resource_mut::<SimulateSnapshot>().0 = Some(ron),
        Err(e) => renzora::core::console_log::console_error(
            "Simulate",
            format!("Failed to snapshot scene; Stop won't restore it: {e}"),
        ),
    });
}

/// Restore the scene from [`SimulateSnapshot`] when Simulate stops. Despawns the
/// live scene entities FIRST, then re-spawns the snapshot — the same
/// despawn→load sequence the tab switcher uses. Skipping the despawn is what
/// duplicated every entity in the hierarchy.
fn on_restore_simulate_snapshot(
    _trigger: On<renzora::core::RestoreSimulateSnapshot>,
    mut commands: Commands,
) {
    commands.queue(|world: &mut World| {
        let Some(ron) = world.resource_mut::<SimulateSnapshot>().0.take() else {
            return;
        };
        despawn_scene_entities(world);
        scene_io::load_scene_from_string(world, &ron);
    });
}

/// On re-entering the splash from the editor (File → Open Project),
/// tear down the previous project's scene state. Without this:
///   - Previous project's entities stay in the world.
///   - Their `Handle<Gltf>` still pin the previous project's assets via
///     `TabAssetCache`.
///   - When the new project's scene file rehydrates, the splash's
///     "Processing models" task counts the *combined* MeshInstanceData
///     set as the denominator — and the previous project's entities
///     never reach a "processed" state from the splash's POV (their
///     SceneRoot child was despawned by `flatten_pending_scenes` last
///     session), so the bar gets stuck at <100%.
///   - Tab buffers and asset cache hold paths from a project the user
///     just left.
///
/// Runs on `OnEnter(SplashState::Splash)` so it fires whether the user
/// is leaving the editor or coming straight from app start (in which
/// case it's a no-op since nothing's spawned yet).
fn teardown_for_project_switch(world: &mut World) {
    let count = despawn_scene_entities(world).len();
    // Switching projects closes every document, so wipe ALL undo stacks
    // (material/particle contexts too), not just Scene, which
    // `despawn_scene_entities` already cleared.
    if let Some(mut undo) = world.get_resource_mut::<renzora_undo::UndoStacks>() {
        undo.clear_all();
    }
    if let Some(mut buffers) = world.get_resource_mut::<SceneTabBuffers>() {
        buffers.buffers.clear();
    }
    if let Some(mut cache) = world.get_resource_mut::<TabAssetCache>() {
        // Drop every tab's entire handle set. Bevy's per-handle refcount
        // does the rest — no other tab is holding a strong handle to
        // the previous project's GLBs at this point, so they evict.
        let stats = tab_asset_cache::cache_stats(&cache);
        *cache = TabAssetCache::default();
        if stats.0 > 0 {
            debug!(
                "[scene] cleared TabAssetCache on splash entry ({} tabs, {} paths)",
                stats.0, stats.1
            );
        }
    }
    // Reset the editor-overlay tracker so a session left mid-flight (the
    // user opened a new project while a tab was decoding) doesn't carry
    // stale `active=true` + dangling task handles into the next editor
    // session.
    if let Some(mut progress) = world.get_resource_mut::<EditorLoadProgress>() {
        *progress = EditorLoadProgress::default();
    }
    if let Some(mut overlay_active) = world.get_resource_mut::<EditorLoadingOverlayActive>() {
        overlay_active.0 = false;
    }
    // Drop the previous project's scene thumbnails — its cached hits AND its
    // known-misses, which would otherwise short-circuit lookups for the new
    // project's identically-named scenes.
    if let Some(mut thumbs) =
        world.get_resource_mut::<renzora_editor_framework::SceneThumbnailRegistry>()
    {
        thumbs.reset();
    }
    if count > 0 {
        info!(
            "[scene] teardown for project switch: despawned {} entities",
            count
        );
    }
}

/// Record the scene tab `id` points at as `editor_last_scene` in
/// project.toml, so a project reload activates the scene the user was
/// actually looking at — not just the last one opened or saved. Writes only
/// when the value changed; no-op for pathless (unsaved) tabs.
fn remember_last_scene(world: &mut World, id: u64) {
    let rel = world
        .get_resource::<DocumentTabState>()
        .and_then(|ts| ts.tabs.iter().find(|t| t.id == id))
        .filter(|t| t.kind == DocTabKind::Scene)
        .and_then(|t| t.scene_path.clone());
    let Some(rel) = rel else {
        return;
    };
    if let Some(mut project) = world.get_resource_mut::<CurrentProject>() {
        if project.config.editor_last_scene.as_deref() != Some(rel.as_str()) {
            project.config.editor_last_scene = Some(rel);
            let _ = project.save_config();
        }
    }
}

/// The document-tab kind for a tab id, if the tab still exists.
fn tab_kind(world: &World, id: u64) -> Option<DocTabKind> {
    world
        .get_resource::<DocumentTabState>()
        .and_then(|ts| ts.tabs.iter().find(|t| t.id == id).map(|t| t.kind))
}

/// React to a document-tab switch. Only **scene** tabs own a 3D scene; asset
/// tabs (material / particle / script / shader) edit assets *on top
/// of* the current scene, so switching to one must NOT despawn the scene — that
/// would empty the viewport/hierarchy and (in the bevy_ui shell) despawn the
/// editor chrome, which the scene-clear query also catches. The live scene
/// belongs to the most-recently-active scene tab, tracked in `live_scene`.
fn handle_tab_switch(world: &mut World, mut live_scene: Local<Option<u64>>) {
    let Some(request) = world.remove_resource::<TabSwitchRequest>() else {
        return;
    };

    let old_id = request.old_tab_id;
    let new_id = request.new_tab_id;

    let old_is_scene = tab_kind(world, old_id) == Some(DocTabKind::Scene);
    let new_is_scene = tab_kind(world, new_id) == Some(DocTabKind::Scene);

    if live_scene.is_none() && old_is_scene {
        *live_scene = Some(old_id);
    }

    // Switching to an asset tab — keep the current scene loaded; nothing to swap.
    if !new_is_scene {
        if old_is_scene {
            *live_scene = Some(old_id);
        }
        return;
    }

    // Switching to a scene tab that's already live (e.g. coming back from a
    // material tab) — the scene was never despawned, so there's nothing to do.
    if *live_scene == Some(new_id) {
        remember_last_scene(world, new_id);
        return;
    }

    // A genuine scene→scene switch: snapshot the live scene, despawn it, and
    // restore the target scene's buffer. Save under the *live* scene tab, which
    // may differ from `old_id` if we're arriving from an asset tab.
    let save_id = live_scene.unwrap_or(old_id);

    // 1. Serialize current scene entities into buffer for the live scene tab
    let scene_ron = scene_io::serialize_scene_to_string(world).unwrap_or_else(|e| {
        warn!("Failed to serialize scene for tab {}: {}", save_id, e);
        "(entities: {}, resources: {})".to_string()
    });

    // 2. Save camera state
    let (focus, distance, yaw, pitch) =
        if let Some(orbit) = world.get_resource::<OrbitCameraState>() {
            (
                orbit.focus.to_array(),
                orbit.distance,
                orbit.yaw,
                orbit.pitch,
            )
        } else {
            let def = OrbitCameraState::default();
            (def.focus.to_array(), def.distance, def.yaw, def.pitch)
        };

    let snapshot = TabSceneSnapshot {
        scene_ron,
        camera_focus: focus,
        camera_distance: distance,
        camera_yaw: yaw,
        camera_pitch: pitch,
    };

    // Store snapshot
    if let Some(mut buffers) = world.get_resource_mut::<SceneTabBuffers>() {
        buffers.buffers.insert(save_id, snapshot);
    }

    // Pin every asset the leaving tab's entities currently reference so
    // that despawning them doesn't evict their textures / materials /
    // meshes. Without this the tab-switch-back lands on the deserialized
    // RON but its `MaterialResolver` / `asset_server.load` calls have to
    // re-decode textures from disk (and worse, anything outside the
    // resolver path renders blank). See `tab_asset_cache` module doc.
    tab_asset_cache::pin_live_tab_handles(world, save_id);

    // 3. Despawn all scene entities
    despawn_scene_entities(world);

    // 4. If target tab has a buffer, deserialize it + restore camera
    let target_snapshot = world
        .get_resource_mut::<SceneTabBuffers>()
        .and_then(|mut buffers| buffers.buffers.remove(&new_id));

    // Where the tab we're switching to lives on disk, if anywhere. Needed by
    // both branches below: the buffer branch to record what is now open, the
    // disk branch to load it.
    let target_path = world
        .get_resource::<DocumentTabState>()
        .and_then(|ts| ts.tabs.iter().find(|t| t.id == new_id))
        .and_then(|t| t.scene_path.clone())
        .and_then(|rel| {
            world
                .get_resource::<CurrentProject>()
                .map(|p| p.resolve_path(&rel))
        });

    if let Some(snap) = target_snapshot {
        scene_io::load_scene_from_string(world, &snap.scene_ron);
        // A buffer restore never goes through `load_scene`, which is the only
        // thing that records the open document in `SceneLoadState`. Left alone,
        // that record keeps naming whichever scene was loaded from disk last —
        // so after visiting a global scene's tab and switching away, everything
        // downstream still believes the global scene is the open one. The
        // autoload pass reads exactly that to avoid spawning a second copy of a
        // global scene you already have open, and so skipped it entirely.
        set_open_scene_path(world, target_path.as_deref());

        // Restore camera
        if let Some(mut orbit) = world.get_resource_mut::<OrbitCameraState>() {
            orbit.focus = Vec3::from_array(snap.camera_focus);
            orbit.distance = snap.camera_distance;
            orbit.yaw = snap.camera_yaw;
            orbit.pitch = snap.camera_pitch;
        }
    } else {
        // Reset the camera either way; a disk load below re-extracts it from
        // the scene's saved editor camera.
        if let Some(mut orbit) = world.get_resource_mut::<OrbitCameraState>() {
            let def = OrbitCameraState::default();
            orbit.focus = def.focus;
            orbit.distance = def.distance;
            orbit.yaw = def.yaw;
            orbit.pitch = def.pitch;
        }
        // No buffer but the tab has a path: a tab restored on project load
        // (`editor_open_tabs`) whose scene hasn't been visited yet — load it
        // from disk. A pathless tab is a genuinely new "+" tab: stays empty.
        if let Some(path) = target_path {
            scene_io::load_scene(world, &path);
            extract_orbit_from_scene_camera(world);
        } else {
            set_open_scene_path(world, None);
        }
    }

    *live_scene = Some(new_id);
    remember_last_scene(world, new_id);

    renzora::core::console_log::console_info(
        "Scene",
        format!("Switched scene tab {} -> {}", save_id, new_id),
    );
}

/// Point `SceneLoadState` at the document that is now open, for the paths that
/// swap scenes without going through [`scene_io::load_scene`].
///
/// `None` for a tab that has no file behind it yet — a fresh "+" tab is not a
/// scene anyone can name, and claiming it is the previous one is the same lie
/// this exists to stop telling.
fn set_open_scene_path(world: &mut World, path: Option<&std::path::Path>) {
    let Some(mut state) = world.get_resource_mut::<scene_io::SceneLoadState>() else {
        return;
    };
    match path {
        Some(path) => {
            state.current_path = Some(path.to_string_lossy().to_string());
            state.phase = scene_io::SceneLoadPhase::Ready;
            state.progress = 1.0;
        }
        None => {
            state.current_path = None;
            state.phase = scene_io::SceneLoadPhase::Idle;
            state.progress = 0.0;
        }
    }
}

// ============================================================================
// Orbit camera <-> scene component helpers
// ============================================================================

/// Stamp the current `OrbitCameraState` resource onto the `SceneCamera` entity
/// so it gets serialized into the scene RON.
fn stamp_orbit_on_scene_camera(world: &mut World) {
    let Some(orbit) = world.get_resource::<OrbitCameraState>().cloned() else {
        return;
    };
    let mut query = world.query_filtered::<Entity, With<SceneCamera>>();
    let entities: Vec<Entity> = query.iter(world).collect();
    for entity in entities {
        world.entity_mut(entity).insert(orbit.clone());
    }
}

/// Extract `OrbitCameraState` from the `SceneCamera` entity after loading,
/// apply it to the resource, and remove the component.
pub(crate) fn extract_orbit_from_scene_camera(world: &mut World) {
    let mut query = world.query_filtered::<(Entity, &OrbitCameraState), With<SceneCamera>>();
    let result: Option<(Entity, OrbitCameraState)> =
        query.iter(world).next().map(|(e, o)| (e, o.clone()));
    if let Some((entity, orbit)) = result {
        world.insert_resource(orbit);
        world.entity_mut(entity).remove::<OrbitCameraState>();
    }
}

// ============================================================================
// Keybinding-driven save
// ============================================================================

fn detect_file_keybindings(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
    input_focus: Option<Res<renzora::core::InputFocusState>>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
) {
    if play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode()) {
        return;
    }
    if keybindings.rebinding.is_some() {
        return;
    }
    // Don't trigger Ctrl+S / Ctrl+O / … while the user is typing in a UI field.
    if input_focus.is_some_and(|f| f.ui_wants_keyboard) {
        return;
    }

    if keybindings.just_pressed(EditorAction::SaveScene, &keyboard) {
        commands.insert_resource(SaveSceneRequested);
    }
    if keybindings.just_pressed(EditorAction::SaveSceneAs, &keyboard) {
        commands.insert_resource(SaveAsSceneRequested);
    }
    if keybindings.just_pressed(EditorAction::OpenScene, &keyboard) {
        commands.insert_resource(OpenSceneRequested);
    }
    if keybindings.just_pressed(EditorAction::NewScene, &keyboard) {
        commands.insert_resource(NewSceneRequested);
    }
    if keybindings.just_pressed(EditorAction::OpenSettings, &keyboard) {
        commands.insert_resource(ToggleSettingsRequested);
    }
}

fn save_scene_system(world: &mut World) {
    if world.remove_resource::<SaveSceneRequested>().is_none() {
        return;
    }

    // The scene is mirroring a running game right now; saving would write the
    // game's state into the level. Run once the game stops and the editor has
    // put its changes back. See `renzora::LivePlaySession`.
    if let Some(mut live) = world.get_resource_mut::<renzora::LivePlaySession>() {
        if !live.pending_save {
            live.pending_save = true;
            renzora::core::console_log::console_info(
                "Scene",
                "The game is running: the scene will save when you press Stop",
            );
        }
        return;
    }

    // Read the active tab's kind + path. CRITICAL: `DocumentTab.scene_path` is
    // populated for EVERY tab kind, so an asset tab (material /
    // particle / script / shader) carries *its own file's* path here. Writing the
    // world scene to that path overwrites the asset with scene RON — exactly how
    // `.material` files got clobbered when Ctrl+S was pressed with a material tab
    // focused. So only a scene tab maps Save → "this path"; an asset tab saves
    // the project's actual scene instead.
    let (active_is_scene, tab_scene_path) = world
        .get_resource::<renzora_ui::DocumentTabState>()
        .and_then(|tabs| tabs.tabs.get(tabs.active_tab))
        .map(|tab| (matches!(tab.kind, DocTabKind::Scene | DocTabKind::Other), tab.scene_path.clone()))
        .unwrap_or((true, None));

    // A scene tab with no path yet → Save As (handled before borrowing the
    // project, since inserting the request needs `&mut World`).
    if active_is_scene && tab_scene_path.is_none() {
        info!("Save: active scene tab has no path, redirecting to Save As");
        world.insert_resource(SaveAsSceneRequested);
        return;
    }

    let Some(project) = world.get_resource::<CurrentProject>() else {
        warn!("No project open — cannot save scene");
        return;
    };
    // Scene/Other tab → its own path; asset tab → the project's main scene
    // (NEVER the asset's file).
    let scene_rel: Option<String> = tab_scene_path.clone().filter(|_| active_is_scene);
    let save_path = match &scene_rel {
        Some(p) => project.resolve_path(p),
        None => project.main_scene_path(),
    };
    info!("Save: target scene path = {}", save_path.display());

    stamp_orbit_on_scene_camera(world);

    // Propagate interior edits of nested scene instances back to their
    // source .ron files. Only descendants are written — the instance
    // root's own Transform stays in the host and never leaks into source.
    scene_io::save_all_scene_instances(world, &save_path);

    if let Err(e) = scene_io::save_scene(world, &save_path) {
        error!("Failed to save scene: {}", e);
        return;
    }

    // Clear the active tab's modified flag only when it's the scene we just saved
    // (an asset tab's own editor owns its dirty state).
    if active_is_scene {
        if let Some(mut tabs) = world.get_resource_mut::<renzora_ui::DocumentTabState>() {
            let active = tabs.active_tab;
            if let Some(tab) = tabs.tabs.get_mut(active) {
                tab.is_modified = false;
            }
        }
    }

    // Remember this scene as the last-open so the editor reopens it next launch
    // (only ever a real scene path).
    if let Some(rel) = scene_rel {
        if let Some(mut project) = world.get_resource_mut::<CurrentProject>() {
            if project.config.editor_last_scene.as_deref() != Some(rel.as_str()) {
                project.config.editor_last_scene = Some(rel.clone());
                let _ = project.save_config();
            }
        }
    }

    // Snapshot the viewport for the browser tile. Queued rather than taken
    // here because this is an exclusive system and the readback needs the
    // render target — see `thumbnail::capture_scene_thumbnail`.
    world.insert_resource(PendingSceneThumbnail(save_path.clone()));

    renzora::core::console_log::console_success(
        "Scene",
        format!("Saved scene to {}", save_path.display()),
    );
}

// ============================================================================
// Save As
// ============================================================================

fn save_as_scene_system(world: &mut World) {
    if world.remove_resource::<SaveAsSceneRequested>().is_none() {
        return;
    }

    let Some(project) = world.get_resource::<CurrentProject>() else {
        warn!("No project open — cannot Save As");
        return;
    };
    let scenes_dir = project.resolve_path("scenes");

    #[cfg(not(target_arch = "wasm32"))]
    {
        let file = renzora_editor_framework::file_dialog::editor_file_dialog(world)
            .set_title("Save Scene As")
            .set_directory(&scenes_dir)
            .add_filter("Scene File", &["bsn"])
            .set_file_name("new_scene.bsn")
            .save_file();

        let Some(file_path) = file else { return };

        // Save the scene to the chosen path
        stamp_orbit_on_scene_camera(world);
        if let Err(e) = scene_io::save_scene(world, &file_path) {
            error!("Failed to save scene: {}", e);
            return;
        }

        // Remember the new file as the scene being edited, as a plain save
        // does. This used to set `main_scene` instead: saving a copy under a
        // new name made that copy the scene the game boots into, with nothing
        // on screen to say so, and left the remembered last scene on the old
        // file. The boot scene changes only through "Set as Boot Scene".
        let relative = {
            let mut project = world.resource_mut::<CurrentProject>();
            let rel = project.make_relative(&file_path);
            if let Some(ref r) = rel {
                project.config.editor_last_scene = Some(r.clone());
                if let Err(e) = project.save_config() {
                    warn!("Failed to save project.toml: {}", e);
                }
            }
            rel
        };

        // Update active tab
        if let Some(mut tabs) = world.get_resource_mut::<renzora_ui::DocumentTabState>() {
            let active = tabs.active_tab;
            if let Some(tab) = tabs.tabs.get_mut(active) {
                tab.is_modified = false;
                if let Some(ref rel) = relative {
                    tab.scene_path = Some(rel.clone());
                }
                if let Some(name) = file_path.file_stem() {
                    tab.name = name.to_string_lossy().to_string();
                }
            }
        }

        // Same viewport snapshot the plain Save takes — a Save As is the first
        // time this path exists, so without it a brand-new scene would have no
        // tile until the next Ctrl+S.
        world.insert_resource(PendingSceneThumbnail(file_path.clone()));

        renzora::core::console_log::console_success(
            "Scene",
            format!("Saved scene as {}", file_path.display()),
        );
    }
}

// ============================================================================
// New Scene
// ============================================================================

fn new_scene_system(world: &mut World) {
    if world.remove_resource::<NewSceneRequested>().is_none() {
        return;
    }

    // "New Scene" opens a fresh document in its own tab rather than wiping the
    // scene on the current tab — the work already open stays open and remains
    // switchable, matching every other document-based editor. This mirrors the
    // document tab bar's "+" button: add an empty scene tab, then fire a
    // `TabSwitchRequest` so `handle_tab_switch` caches the leaving scene into
    // its buffer, despawns it, and lands on the new tab. The new tab is
    // pathless and buffer-less, so `handle_tab_switch` restores it as a fresh
    // empty scene — exactly what "New Scene" should show.
    let Some(mut tabs) = world.get_resource_mut::<renzora_ui::DocumentTabState>() else {
        return;
    };
    let idx = tabs.add_tab("Untitled Scene".into(), None);
    let switch = tabs.activate_tab(idx);
    if let Some((old_id, new_id)) = switch {
        world.insert_resource(TabSwitchRequest {
            old_tab_id: old_id,
            new_tab_id: new_id,
        });
    }

    renzora::core::console_log::console_info("Scene", "New scene created in a new tab");
}

// ============================================================================
// Open Scene
// ============================================================================

fn open_scene_system(world: &mut World) {
    if world.remove_resource::<OpenSceneRequested>().is_none() {
        return;
    }

    let Some(project) = world.get_resource::<CurrentProject>() else {
        warn!("No project open — cannot Open Scene");
        return;
    };
    let scenes_dir = project.resolve_path("scenes");

    #[cfg(not(target_arch = "wasm32"))]
    {
        let file = renzora_editor_framework::file_dialog::editor_file_dialog(world)
            .set_title("Open Scene")
            .set_directory(&scenes_dir)
            .add_filter("Scene File", &["bsn"])
            .pick_file();

        let Some(file_path) = file else { return };

        // Same path as the asset browser's "Open Scene": buffer the current
        // tab's scene, then open (or re-activate) the picked scene in its own
        // document tab and record it as `editor_last_scene`. The old in-place
        // behaviour silently discarded the active tab's scene and rewrote
        // `main_scene` — the *game's* boot scene — as a side effect of an
        // editor navigation.
        panel::open_scene(world, &file_path);

        renzora::core::console_log::console_success(
            "Scene",
            format!("Opened scene {}", file_path.display()),
        );
    }
}

/// Open a specific scene file (requested by path, e.g. the asset browser's
/// double-click / "Open Scene" context item) in its own document tab. Defers to
/// [`panel::open_scene`], which saves the current tab's buffer, creates a tab,
/// and loads the file from disk — so the new tab shows the scene's contents
/// rather than the empty world a bare `open_asset_tab` would leave behind.
fn open_scene_path_system(world: &mut World) {
    let Some(req) = world.remove_resource::<OpenScenePathRequested>() else {
        return;
    };
    // A project-relative path (`scenes/level.bsn`, the form every other path in
    // a project takes) used to be read against the editor's working directory,
    // which is wherever the executable happened to be started from. It failed
    // to load, and the tab it opened kept the bad path.
    let path = match world.get_resource::<CurrentProject>() {
        Some(project) if req.0.is_relative() => project.path.join(&req.0),
        _ => req.0,
    };
    panel::open_scene(world, &path);
}

// ============================================================================
// Load on entering editor
// ============================================================================

/// Tracks the splash-loading tasks that hold the loading screen up while the
/// scene's assets are fetched and instantiated. Two phases get their own
/// task so the user sees the work as the bar fills:
///
/// 1. **Loading assets** — Gltf bytes read from disk, textures decoded,
///    StandardMaterial assets created. Bound to `PendingMeshInstanceRehydrate`
///    presence: while a model still carries that marker, its Gltf asset
///    hasn't finished loading.
/// 2. **Processing models** — Bevy's `SceneSpawner` runs `write_to_world` for
///    each scene, instantiating the GLB hierarchy under the `SceneRoot`
///    child. Bound to whether the `SceneRoot` has children of its own.
///
/// (Material compilation runs lazily in the editor — drag-dropped models
/// kick the production pipeline; load-path models render via the Gltf
/// loader's `StandardMaterial`s. There's nothing to gate the splash on.)
#[derive(Resource, Default)]
struct SceneLoadProgress {
    loading_task: Option<LoadingTaskHandle>,
    processing_task: Option<LoadingTaskHandle>,
    /// How many `MeshInstanceData` entities the scene file rehydrated.
    /// Denominator for both tasks.
    total_instances: u32,
    /// Per-task progress counters (incremental — `LoadingTasks::advance` is
    /// additive, so we track the previous value for delta computation).
    last_loaded: u32,
    last_processed: u32,
    /// On-disk byte size of each distinct GLB the scene references (`model_path`
    /// → file size), gathered once when loading starts. Drives the real byte
    /// loader; empty when the scene has no external models.
    gltf_sizes: std::collections::HashMap<String, u64>,
}

/// Loads the scene file the moment we transition into `SplashState::Loading`,
/// then registers a `LoadingTasks` task whose progress reflects how many
/// `MeshInstanceData` entities still need their GLB scene fetched and
/// instantiated. The loading screen stays up until every pending GLB has
/// landed (plus the standard `min_frames_remaining` grace period), so the
/// editor only opens onto a fully-populated entity tree.
/// Real texture-decode progress. Textures are stripped from the GLB into external
/// `.rmip` files and loaded as separate images by the material resolver *after*
/// the scene spawns, so they're the genuine remaining work. Count how many of the
/// spawned models' material textures have actually finished, into
/// `TextureLoadProgress` — the loading screen shows it and waits on it.
#[allow(clippy::too_many_arguments)]
fn tick_texture_progress(
    state: Res<State<SplashState>>,
    asset_server: Res<AssetServer>,
    std_mats: Res<Assets<StandardMaterial>>,
    graph_mats: Option<Res<Assets<GraphMaterial>>>,
    std_q: Query<&MeshMaterial3d<StandardMaterial>, (Without<EditorCamera>, Without<HideInHierarchy>)>,
    graph_q: Query<&MeshMaterial3d<GraphMaterial>, (Without<EditorCamera>, Without<HideInHierarchy>)>,
    progress: Option<ResMut<TextureLoadProgress>>,
    images: Res<Assets<Image>>,
) {
    if !matches!(state.get(), SplashState::Loading) {
        return;
    }
    let Some(mut progress) = progress else { return };
    let count = count_scene_textures(&asset_server, &images, &std_mats, graph_mats.as_deref(), &std_q, &graph_q);
    progress.loaded = count.loaded;
    progress.total = count.total;
}

/// The scene's material textures, by how far each has got.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
struct TextureCount {
    loaded: u32,
    failed: u32,
    total: u32,
}

impl TextureCount {
    /// Every texture has either arrived or failed: nothing is left to wait on.
    fn settled(self) -> bool {
        self.loaded + self.failed >= self.total
    }
}

/// Count the distinct textures the spawned models' materials use.
fn count_scene_textures(
    asset_server: &AssetServer,
    image_assets: &Assets<Image>,
    std_mats: &Assets<StandardMaterial>,
    graph_mats: Option<&Assets<GraphMaterial>>,
    std_q: &Query<&MeshMaterial3d<StandardMaterial>, (Without<EditorCamera>, Without<HideInHierarchy>)>,
    graph_q: &Query<&MeshMaterial3d<GraphMaterial>, (Without<EditorCamera>, Without<HideInHierarchy>)>,
) -> TextureCount {
    let mut images: Vec<Handle<Image>> = Vec::new();
    for m in std_q {
        if let Some(mat) = std_mats.get(&m.0) {
            tab_asset_cache::collect_standard_material_images(mat, &mut images);
        }
    }
    if let Some(graph_mats) = graph_mats {
        for m in graph_q {
            if let Some(mat) = graph_mats.get(&m.0) {
                tab_asset_cache::collect_standard_material_images(&mat.base, &mut images);
                for slot in mat.extension.d2_slots().into_iter().flatten() {
                    images.push(slot.clone());
                }
            }
        }
    }

    let mut seen: std::collections::HashSet<bevy::asset::AssetId<Image>> = std::collections::HashSet::new();
    let mut count = TextureCount::default();
    for h in &images {
        if seen.insert(h.id()) {
            count.total += 1;
            match asset_server.get_load_state(h.id()) {
                Some(bevy::asset::LoadState::Loaded) => count.loaded += 1,
                Some(bevy::asset::LoadState::Failed(_)) => count.failed += 1,
                // Made at runtime rather than read from a file, so the asset
                // server has no load state for it: present means ready. Counted
                // as pending, one such image kept a load waiting forever.
                None if image_assets.get(h.id()).is_some() => count.loaded += 1,
                _ => {}
            }
        }
    }
    count
}

/// Give up on a load that has not settled after this long, and say what it
/// was still waiting on: a texture that never arrives must not leave the
/// timeline running for the rest of the session.
const LOAD_GIVE_UP: std::time::Duration = std::time::Duration::from_secs(120);

/// Frames the texture count must hold still before it is believed. Materials
/// are resolved after their meshes spawn, so for a few frames a model can
/// report no textures at all, which would read as "done".
const TEXTURE_SETTLE_FRAMES: u32 = 30;

/// Frames with no new stage, once the required ones are in, before the load is
/// called finished. Gives the stages other crates record (virtual geometry,
/// environment lighting) the chance to land in the same timeline.
const LOAD_QUIET_FRAMES: u32 = 60;

#[derive(Default)]
struct TimelineTracker {
    generation: u32,
    last_count: TextureCount,
    stable_frames: u32,
    marks_seen: usize,
    quiet_frames: u32,
    last_resolved: usize,
    resolved_stable_frames: u32,
    pipelines_idle_frames: u32,
    peak_pipelines: usize,
}

/// Frames the resolved-material count must hold still, every mesh resolved,
/// before the materials are called done: meshes are handed their materials
/// over many frames while a large model's hierarchy fills in.
const MATERIALS_SETTLE_FRAMES: u32 = 30;

/// Frames with no render pipeline queued or compiling before the shaders are
/// called done. Short, because a compile that is still coming shows up as
/// queued within a frame of the material that needs it.
const PIPELINES_IDLE_FRAMES: u32 = 10;

/// How many render pipelines are queued or compiling, counted in the render
/// world and read in the main one. A mesh draws nothing until its material's
/// pipelines exist, and every graph material has its own shader, so on a
/// scene with many materials this is the stretch where models sit there black
/// after the loading screen has gone.
#[derive(Resource, Clone, Default)]
struct PendingPipelines(std::sync::Arc<std::sync::atomic::AtomicUsize>);

/// Report what meshes and images hold to [`renzora::load_timeline::AssetMemory`],
/// twice a second.
///
/// A mesh or image Bevy only needs on the GPU has its CPU copy dropped after
/// upload; the rest keep both. Those CPU copies are what the editor itself
/// holds, so they are counted separately from the ones held only on the GPU,
/// and the images' GPU size is estimated from their format beside them.
fn measure_asset_memory(
    meshes: Res<Assets<Mesh>>,
    images: Res<Assets<Image>>,
    asset_server: Res<AssetServer>,
    font_atlases: Option<Res<bevy::text::FontAtlasSet>>,
    memory: Option<ResMut<renzora::load_timeline::AssetMemory>>,
    mut frames: Local<u32>,
) {
    use renzora::load_timeline::AssetMemoryRow;
    *frames = frames.wrapping_add(1);
    let Some(mut memory) = memory.filter(|_| *frames % 30 == 1) else {
        return;
    };

    // Text keeps every glyph it has drawn in atlases, one set per font, size
    // and smoothing, each with a CPU copy so glyphs can be added later. They
    // are images made at runtime like any other, so without naming them they
    // read as a mystery among the render targets.
    let atlas_ids: std::collections::HashSet<AssetId<Image>> = font_atlases
        .as_deref()
        .map(|set| set.values().flatten().map(|atlas| atlas.texture.id()).collect())
        .unwrap_or_default();
    let atlas_keys = font_atlases.as_deref().map_or(0, |set| set.len());

    let (mut cpu_meshes, mut cpu_mesh_bytes, mut gpu_only_meshes) = (0, 0u64, 0);
    for (_, mesh) in meshes.iter() {
        match mesh_cpu_bytes(mesh) {
            Some(bytes) => {
                cpu_meshes += 1;
                cpu_mesh_bytes += bytes;
            }
            None => gpu_only_meshes += 1,
        }
    }
    memory.set(AssetMemoryRow {
        kind: "meshes (CPU copies)",
        count: cpu_meshes,
        bytes: cpu_mesh_bytes,
        in_process: true,
        note: format!("{gpu_only_meshes} more held only on the GPU"),
    });

    let (mut cpu_images, mut cpu_image_bytes, mut gpu_only_images, mut gpu_bytes) = (0, 0u64, 0, 0u64);
    // Every CPU copy with its size, so the report can name the largest: a
    // total alone does not say whether it is the project's textures or the
    // editor's own environment maps and thumbnails.
    let mut largest: Vec<(u64, AssetId<Image>)> = Vec::new();
    for (id, image) in images.iter() {
        match &image.data {
            Some(data) => {
                cpu_images += 1;
                cpu_image_bytes += data.capacity() as u64;
                largest.push((data.capacity() as u64, id));
            }
            None => gpu_only_images += 1,
        }
        gpu_bytes += image_gpu_bytes(image);
    }
    largest.sort_unstable_by_key(|(bytes, _)| std::cmp::Reverse(*bytes));
    let from_files: u64 = largest
        .iter()
        .filter(|(_, id)| asset_server.get_path(*id).is_some())
        .map(|(bytes, _)| bytes)
        .sum();
    let atlas_bytes: u64 = largest
        .iter()
        .filter(|(_, id)| atlas_ids.contains(id))
        .map(|(bytes, _)| bytes)
        .sum();
    // Images made at runtime have no path, so they are described by their
    // label, size and format, and alike ones are counted together: a hundred
    // small thumbnails of one size say more as one line than as a hundred
    // entries that each fall below the cut.
    let mut groups: std::collections::HashMap<String, (usize, u64)> = std::collections::HashMap::new();
    for (bytes, id) in &largest {
        let font_atlas = atlas_ids.contains(id);
        let what = match asset_server.get_path(*id) {
            Some(path) => path.to_string(),
            None if font_atlas => images.get(*id).map_or_else(
                || "font atlas".to_string(),
                |image| format!("font atlas, {}x{}", image.width(), image.height()),
            ),
            None => images.get(*id).map_or_else(
                || "made at runtime".to_string(),
                |image| {
                    let descriptor = &image.texture_descriptor;
                    let size = descriptor.size;
                    let label = descriptor.label.map_or(String::new(), |label| format!(" \"{label}\""));
                    // A render target with a CPU copy is a buffer of zeros
                    // nothing reads, so it is worth saying which ones are.
                    let target = if descriptor
                        .usage
                        .contains(bevy::render::render_resource::TextureUsages::RENDER_ATTACHMENT)
                    {
                        " render target"
                    } else {
                        ""
                    };
                    format!(
                        "made at runtime{label}{target}, {}x{}x{} {:?}",
                        size.width, size.height, size.depth_or_array_layers, descriptor.format
                    )
                },
            ),
        };
        let group = groups.entry(what).or_default();
        group.0 += 1;
        group.1 += bytes;
    }
    let mut groups: Vec<(String, (usize, u64))> = groups.into_iter().collect();
    groups.sort_unstable_by_key(|(_, (_, bytes))| std::cmp::Reverse(*bytes));
    let named: Vec<String> = groups
        .iter()
        .take(10)
        .map(|(what, (count, bytes))| format!("{} {what} x{count}", megabytes(*bytes)))
        .collect();
    memory.set(AssetMemoryRow {
        kind: "images (CPU copies)",
        count: cpu_images,
        bytes: cpu_image_bytes,
        in_process: true,
        note: format!(
            "{gpu_only_images} more held only on the GPU; {} from files, {} in {} font atlases for {atlas_keys} \
             font sizes, the rest made at runtime; largest: {}",
            megabytes(from_files),
            megabytes(atlas_bytes),
            atlas_ids.len(),
            named.join(", ")
        ),
    });
    memory.set(AssetMemoryRow {
        kind: "images (on the GPU)",
        count: cpu_images + gpu_only_images,
        bytes: gpu_bytes,
        in_process: false,
        note: "from each image's size, format and mip count".to_string(),
    });
}

/// Report what the GPU allocator holds to the memory breakdown, every two
/// seconds: what is allocated, what is reserved in memory blocks behind it,
/// and the labels holding the most.
///
/// This is the one figure the editor's own rows cannot estimate. Buffers,
/// render targets, acceleration structures and upload staging all come from
/// these blocks, and on Windows the blocks in system memory count toward the
/// process's committed memory, which is where most of a project load's
/// growth was showing up with nothing in the asset rows to explain it. Not
/// every backend can report; then the row says so.
fn measure_gpu_allocations(
    device: Option<Res<bevy::render::renderer::RenderDevice>>,
    memory: Option<ResMut<renzora::load_timeline::AssetMemory>>,
    mut frames: Local<u32>,
) {
    use renzora::load_timeline::AssetMemoryRow;
    *frames = frames.wrapping_add(1);
    let (Some(device), Some(mut memory)) = (device, memory.filter(|_| *frames % 120 == 2)) else {
        return;
    };
    let Some(report) = device.wgpu_device().generate_allocator_report() else {
        memory.set(AssetMemoryRow {
            kind: "GPU allocations",
            count: 0,
            bytes: 0,
            in_process: false,
            note: "this graphics backend gives no allocation report".to_string(),
        });
        return;
    };
    let mut by_label: std::collections::HashMap<&str, (usize, u64)> = std::collections::HashMap::new();
    for allocation in &report.allocations {
        let label = if allocation.name.is_empty() { "(no label)" } else { allocation.name.as_str() };
        let entry = by_label.entry(label).or_default();
        entry.0 += 1;
        entry.1 += allocation.size;
    }
    let mut labels: Vec<(&str, (usize, u64))> = by_label.into_iter().collect();
    labels.sort_unstable_by_key(|(_, (_, bytes))| std::cmp::Reverse(*bytes));
    let largest: Vec<String> = labels
        .iter()
        .take(12)
        .map(|(label, (count, bytes))| format!("{} {label} x{count}", megabytes(*bytes)))
        .collect();
    memory.set(AssetMemoryRow {
        kind: "GPU allocations",
        count: report.allocations.len(),
        bytes: report.total_allocated_bytes,
        in_process: false,
        note: format!(
            "{} reserved in {} blocks; largest: {}",
            megabytes(report.total_reserved_bytes),
            report.blocks.len(),
            largest.join(", ")
        ),
    });
}

/// Give the load timeline a way to read the GPU allocator's totals, once the
/// render device exists, so every stage records them beside the process's
/// figures. Without it a stage that commits gigabytes cannot be told apart
/// into what the GPU took and what the editor's own heap did.
fn install_gpu_probe(
    device: Option<Res<bevy::render::renderer::RenderDevice>>,
    timeline: Option<ResMut<renzora::load_timeline::LoadTimeline>>,
) {
    let (Some(device), Some(mut timeline)) = (device, timeline) else { return };
    if timeline.has_gpu_probe() {
        return;
    }
    let device = device.wgpu_device().clone();
    timeline.set_gpu_probe(Box::new(move || {
        let report = device.generate_allocator_report()?;
        Some(renzora::load_timeline::GpuMemory {
            allocated: report.total_allocated_bytes,
            reserved: report.total_reserved_bytes,
        })
    }));
}

/// The bytes a mesh's vertex attributes and indices hold on the CPU, or `None`
/// for a mesh whose data has been handed to the render world.
fn mesh_cpu_bytes(mesh: &Mesh) -> Option<u64> {
    let attributes: u64 = mesh
        .try_attributes()
        .ok()?
        .map(|(_, values)| values.get_bytes().len() as u64)
        .sum();
    let indices = match mesh.try_indices_option() {
        Ok(Some(bevy::mesh::Indices::U16(v))) => (v.len() * 2) as u64,
        Ok(Some(bevy::mesh::Indices::U32(v))) => (v.len() * 4) as u64,
        _ => 0,
    };
    Some(attributes + indices)
}

/// An image's size on the GPU: every mip level of every layer, in its format's
/// blocks.
fn image_gpu_bytes(image: &Image) -> u64 {
    let descriptor = &image.texture_descriptor;
    let format = descriptor.format;
    let (block_w, block_h) = format.block_dimensions();
    let block_bytes = format.block_copy_size(None).unwrap_or(4) as u64;
    let layers = descriptor.size.depth_or_array_layers.max(1) as u64;
    (0..descriptor.mip_level_count)
        .map(|mip| {
            let w = (descriptor.size.width >> mip).max(1);
            let h = (descriptor.size.height >> mip).max(1);
            w.div_ceil(block_w) as u64 * h.div_ceil(block_h) as u64 * block_bytes * layers
        })
        .sum()
}

/// Record when Bevy itself finishes with a model's assets: the model, the scene
/// inside it that gets instantiated, and the point where every texture it
/// names has loaded too. These say where the time between "models loaded" and
/// "model instantiated by Bevy" goes, which the editor's own stages cannot see.
fn track_model_asset_events(
    timeline: Option<ResMut<renzora::load_timeline::LoadTimeline>>,
    asset_server: Res<AssetServer>,
    mut models: MessageReader<AssetEvent<bevy::gltf::Gltf>>,
    mut scenes: MessageReader<AssetEvent<bevy::world_serialization::WorldAsset>>,
) {
    let Some(mut timeline) = timeline.filter(|t| t.is_running()) else {
        models.clear();
        scenes.clear();
        return;
    };
    let path = |id: bevy::asset::UntypedAssetId| {
        asset_server.get_path(id).map(|p| p.to_string()).unwrap_or_default()
    };
    for event in models.read() {
        match event {
            AssetEvent::Added { id } => timeline.mark("model asset added", path((*id).into())),
            AssetEvent::LoadedWithDependencies { id } => {
                timeline.mark("model loaded with dependencies", path((*id).into()))
            }
            _ => {}
        }
    }
    for event in scenes.read() {
        if let AssetEvent::Added { id } = event {
            timeline.mark("model scene asset added", path((*id).into()));
        }
    }
}

fn count_pending_pipelines(
    cache: Res<bevy::render::render_resource::PipelineCache>,
    pending: Res<PendingPipelines>,
) {
    use bevy::render::render_resource::CachedPipelineState;
    let count = cache
        .pipelines()
        .filter(|p| matches!(p.state, CachedPipelineState::Queued | CachedPipelineState::Creating(_)))
        .count();
    pending.0.store(count, std::sync::atomic::Ordering::Relaxed);
}

/// Follow a project load past the splash and into the editor, where textures
/// keep arriving after the loading screen has gone, and close the timeline
/// once everything has settled. See [`renzora::load_timeline`].
#[allow(clippy::too_many_arguments)]
fn track_load_timeline(
    timeline: Option<ResMut<renzora::load_timeline::LoadTimeline>>,
    state: Res<State<SplashState>>,
    time: Res<Time<Real>>,
    asset_server: Res<AssetServer>,
    std_mats: Res<Assets<StandardMaterial>>,
    graph_mats: Option<Res<Assets<GraphMaterial>>>,
    std_q: Query<&MeshMaterial3d<StandardMaterial>, (Without<EditorCamera>, Without<HideInHierarchy>)>,
    graph_q: Query<&MeshMaterial3d<GraphMaterial>, (Without<EditorCamera>, Without<HideInHierarchy>)>,
    viewport_cameras: Query<(&Camera, Has<bevy::camera::RenderTarget>), With<renzora::core::ViewportCamera>>,
    images: Res<Assets<Image>>,
    unresolved: Query<(), (With<renzora::core::MaterialRef>, Without<renzora::core::MaterialResolved>)>,
    resolved: Query<(), With<renzora::core::MaterialResolved>>,
    pending_pipelines: Option<Res<PendingPipelines>>,
    asset_memory: Option<Res<renzora::load_timeline::AssetMemory>>,
    mut tracker: Local<TimelineTracker>,
) {
    let Some(mut timeline) = timeline else { return };
    if !timeline.is_running() {
        return;
    }
    if tracker.generation != timeline.generation() {
        *tracker = TimelineTracker { generation: timeline.generation(), ..default() };
    }
    timeline.note_frame(time.delta());

    // Until this, the viewport has nothing to draw with and shows black.
    if viewport_cameras.iter().any(|(camera, has_target)| camera.is_active && has_target) {
        timeline.mark("viewport camera ready", "");
    }
    if matches!(state.get(), SplashState::Editor) {
        timeline.mark("editor opened", "");
    }

    if timeline.has("models spawned") && !timeline.has("textures loaded") {
        let count = count_scene_textures(&asset_server, &images, &std_mats, graph_mats.as_deref(), &std_q, &graph_q);
        if count == tracker.last_count {
            tracker.stable_frames += 1;
        } else {
            tracker.last_count = count;
            tracker.stable_frames = 0;
        }
        if count.settled() && tracker.stable_frames >= TEXTURE_SETTLE_FRAMES {
            let detail = if count.failed > 0 {
                format!("{} textures, {} failed", count.total, count.failed)
            } else {
                format!("{} textures", count.total)
            };
            timeline.mark("textures loaded", detail);
        }
    }

    if timeline.has("models spawned") && !timeline.has("materials resolved") {
        let count = resolved.iter().count();
        if count == tracker.last_resolved {
            tracker.resolved_stable_frames += 1;
        } else {
            tracker.last_resolved = count;
            tracker.resolved_stable_frames = 0;
        }
        if unresolved.is_empty() && tracker.resolved_stable_frames >= MATERIALS_SETTLE_FRAMES {
            timeline.mark("materials resolved", format!("{count} mesh(es)"));
        }
    }

    let pipelines = pending_pipelines
        .as_ref()
        .map_or(0, |p| p.0.load(std::sync::atomic::Ordering::Relaxed));
    tracker.peak_pipelines = tracker.peak_pipelines.max(pipelines);
    if timeline.has("materials resolved") && !timeline.has("shaders compiled") {
        tracker.pipelines_idle_frames = if pipelines == 0 { tracker.pipelines_idle_frames + 1 } else { 0 };
        if tracker.pipelines_idle_frames >= PIPELINES_IDLE_FRAMES {
            timeline.mark(
                "shaders compiled",
                format!("up to {} pipeline(s) compiling at once", tracker.peak_pipelines),
            );
        }
    }

    let required = ["models spawned", "editor opened", "textures loaded", "materials resolved", "shaders compiled"];
    if required.iter().all(|stage| timeline.has(stage)) {
        if timeline.marks().len() == tracker.marks_seen {
            tracker.quiet_frames += 1;
        } else {
            tracker.marks_seen = timeline.marks().len();
            tracker.quiet_frames = 0;
        }
        if tracker.quiet_frames >= LOAD_QUIET_FRAMES {
            if let Some(memory) = &asset_memory {
                timeline.set_appendix(memory.report());
            }
            timeline.finish();
        }
    } else if timeline.running_for().is_some_and(|t| t >= LOAD_GIVE_UP) {
        let mut missing: Vec<String> = required
            .iter()
            .filter(|stage| !timeline.has(stage))
            .map(|stage| stage.to_string())
            .collect();
        if let Some(text) = missing.iter_mut().find(|s| *s == "textures loaded") {
            let c = tracker.last_count;
            *text = format!("textures ({} of {} loaded, {} failed)", c.loaded, c.total, c.failed);
        }
        if let Some(text) = missing.iter_mut().find(|s| *s == "materials resolved") {
            *text = format!("materials ({} mesh(es) unresolved)", unresolved.iter().count());
        }
        if let Some(text) = missing.iter_mut().find(|s| *s == "shaders compiled") {
            *text = format!("shaders ({pipelines} pipeline(s) still compiling)");
        }
        if let Some(memory) = &asset_memory {
            timeline.set_appendix(memory.report());
        }
        timeline.give_up(missing.join(", "));
    }
}

/// Forget the remembered last scene and any remembered tab whose file is no
/// longer on disk, returning whether anything was dropped.
///
/// Both are written while the editor runs and read back on the next open, and
/// a file deleted or moved outside the editor in between used to survive the
/// trip. A missing last scene loaded nothing but stayed the save target, so the
/// project's main scene could come up in a tab named after the dead file, and
/// every Ctrl+S and autosave from then on went to that dead path instead of the
/// main scene. A missing tab came back as an empty scene with nothing to say
/// why. Dropping them falls back to the main scene, which is what opening a
/// project with nothing remembered already does.
fn forget_missing_files(project: &mut CurrentProject) -> bool {
    let root = project.path.clone();
    let gone = |rel: &str| !root.join(rel).exists();
    let mut changed = false;
    if project.config.editor_last_scene.as_deref().is_some_and(|s| !s.is_empty() && gone(s)) {
        project.config.editor_last_scene = None;
        changed = true;
    }
    let before = project.config.editor_open_tabs.len();
    project.config.editor_open_tabs.retain(|tab| !gone(&tab.path));
    changed |= project.config.editor_open_tabs.len() != before;
    changed
}

fn load_scene_on_enter_loading(world: &mut World) {
    info!("[loading] entered Loading state, kicking off scene load");
    let project_name = world
        .get_resource::<CurrentProject>()
        .map(|p| p.config.name.clone())
        .unwrap_or_else(|| "project".to_string());
    world
        .get_resource_or_insert_with(renzora::load_timeline::LoadTimeline::default)
        .begin(project_name);

    // Ensure the asset reader knows the project path before loading the scene.
    if let Some(project) = world.get_resource::<CurrentProject>() {
        let path = project.path.clone();
        if let Some(asset_path) = world.get_resource::<renzora_engine::ProjectAssetPath>() {
            info!("[scene] Syncing project asset path: {}", path.display());
            asset_path.set(path);
        }
    }

    if let Some(mut project) = world.get_resource_mut::<CurrentProject>() {
        if forget_missing_files(&mut project) {
            if let Err(e) = project.save_config() {
                warn!("[scene] could not save project.toml after dropping missing files: {e}");
            }
        }
    }

    // Editor: prefer the last scene the user had open; fall back to the
    // project's boot scene if there's no saved last scene. Runtime builds
    // use `main_scene` via `scene_io::load_current_scene` — this branch is
    // editor-only.
    if let Some(project) = world.get_resource::<CurrentProject>() {
        let relative = project
            .config
            .editor_last_scene
            .as_ref()
            .filter(|s| !s.is_empty())
            .cloned()
            .unwrap_or_else(|| project.config.main_scene.clone());
        let path = project.resolve_path(&relative);
        scene_io::load_scene(world, &path);
    }
    extract_orbit_from_scene_camera(world);

    // Update first tab to reflect the loaded scene
    let scene_info = world.get_resource::<CurrentProject>().map(|p| {
        let rel = p
            .config
            .editor_last_scene
            .as_ref()
            .filter(|s| !s.is_empty())
            .cloned()
            .unwrap_or_else(|| p.config.main_scene.clone());
        let name = std::path::Path::new(&rel)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled Scene".to_string());
        (rel, name)
    });

    // Rebuild the document tab set from the project's persisted tab list
    // (`editor_open_tabs`), discarding whatever the resource held before —
    // the freshly seeded "Untitled Scene" tab on first open, or a previous
    // project's tabs when reopening from inside the editor. The loaded
    // scene's tab is guaranteed to exist and becomes the active tab; other
    // scene tabs stay unloaded until clicked (see `handle_tab_switch`).
    if let Some((scene_path, scene_name)) = scene_info {
        let open_tabs = world
            .get_resource::<CurrentProject>()
            .map(|p| p.config.editor_open_tabs.clone())
            .unwrap_or_default();
        if let Some(mut tabs) = world.get_resource_mut::<DocumentTabState>() {
            tabs.tabs.clear();
            for entry in &open_tabs {
                let kind = DocTabKind::from_persist_name(&entry.kind);
                // Hand-edited configs may hold duplicates; the tab bar can't.
                if tabs.find_by_path(&entry.path, kind).is_some() {
                    continue;
                }
                let name = std::path::Path::new(&entry.path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| entry.path.clone());
                tabs.add_tab_of_kind(name, Some(entry.path.clone()), kind);
            }
            let active = tabs
                .find_by_path(&scene_path, DocTabKind::Scene)
                .unwrap_or_else(|| tabs.add_tab(scene_name, Some(scene_path)));
            tabs.active_tab = active;
            tabs.touch_scene_mru(active);
        }
    }

    // Count the model instances we need to resolve. Used as the denominator
    // for the progress bar.
    let instance_count = {
        let mut q = world.query_filtered::<Entity, With<MeshInstanceData>>();
        q.iter(world).count() as u32
    };

    // Gather the real on-disk size of every distinct GLB the scene references,
    // for a genuine byte loader. The asset reader resolves `model_path` under the
    // project root, so we stat `project_path/{model_path}`.
    let project_root = world.get_resource::<CurrentProject>().map(|p| p.path.clone());
    let mut gltf_sizes: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    {
        let mut q = world.query::<&MeshInstanceData>();
        for data in q.iter(world) {
            if let Some(path) = data.model_path.as_deref() {
                if !gltf_sizes.contains_key(path) {
                    let bytes = project_root
                        .as_ref()
                        .map(|r| r.join(path))
                        .and_then(|f| std::fs::metadata(f).ok())
                        .map(|m| m.len())
                        .unwrap_or(0);
                    gltf_sizes.insert(path.to_string(), bytes);
                }
            }
        }
    }
    let total_bytes: u64 = gltf_sizes.values().sum();
    if let Some(mut lb) = world.get_resource_mut::<LoadingBytes>() {
        lb.loaded = 0;
        lb.total = total_bytes;
    }
    if let Some(mut timeline) = world.get_resource_mut::<renzora::load_timeline::LoadTimeline>() {
        timeline.mark(
            "scene parsed",
            format!(
                "{instance_count} model instance(s), {} model file(s), {}",
                gltf_sizes.len(),
                megabytes(total_bytes)
            ),
        );
    }

    // Two tasks so the user sees each phase as the bar fills:
    //   1. Asset I/O   — Gltf bytes from disk, texture decode.
    //   2. Scene spawn — Bevy's SceneSpawner instantiating the GLB.
    let n = instance_count.max(1);
    let (loading_task, processing_task) = world
        .get_resource_mut::<LoadingTasks>()
        .map(|mut tasks| {
            (
                Some(tasks.register("Loading assets", n)),
                Some(tasks.register("Processing models", n)),
            )
        })
        .unwrap_or((None, None));
    world.insert_resource(SceneLoadProgress {
        loading_task,
        processing_task,
        total_instances: instance_count,
        last_loaded: 0,
        last_processed: 0,
        gltf_sizes,
    });

    // Ask the hierarchy to auto-select its top entity once the cache is
    // populated. The flag is consumed by `auto_select_first_hierarchy_entity`
    // in `renzora_hierarchy` — we can't do it here because entities have
    // only just been queued for spawn and the hierarchy tree won't be
    // built until the next frame.
    if let Some(mut flag) =
        world.get_resource_mut::<renzora_editor_framework::AutoSelectFirstHierarchyEntity>()
    {
        flag.0 = true;
    }
}

/// Runs every frame in `SplashState::Loading`. Reports how many
/// `MeshInstanceData` entities are *fully* spawned — meaning their GLB
/// asset has loaded, `finish_mesh_instance_rehydrate` has spawned a
/// `SceneRoot` child, *and* Bevy's `SceneSpawner` has populated the GLB
/// hierarchy under that `SceneRoot`. The last condition is the one that
/// matters for visibility — `PendingMeshInstanceRehydrate` getting removed
/// only signals "asset arrived"; mesh entities don't appear under the
/// SceneRoot for another frame or two while Bevy actually instantiates.
#[allow(clippy::too_many_arguments)]
fn tick_scene_load_progress(
    progress: Option<ResMut<SceneLoadProgress>>,
    mut tasks: ResMut<LoadingTasks>,
    instances: Query<(
        &MeshInstanceData,
        Option<&Children>,
        Has<scene_io::MeshInstanceLoadFailed>,
    )>,
    pending: Query<&MeshInstanceData, With<scene_io::PendingMeshInstanceRehydrate>>,
    loaded_q: Query<&MeshInstanceData, Without<scene_io::PendingMeshInstanceRehydrate>>,
    loading_bytes: Option<ResMut<LoadingBytes>>,
    children_q: Query<&Children>,
    scene_roots: Query<Entity, With<bevy::world_serialization::WorldAssetRoot>>,
    timeline: Option<ResMut<renzora::load_timeline::LoadTimeline>>,
) {
    let Some(mut progress) = progress else { return };
    let mut timeline = timeline;
    let total_bytes: u64 = progress.gltf_sizes.values().sum();
    let mut mark = |stage: &str, detail: String| {
        if let Some(timeline) = timeline.as_mut() {
            timeline.mark(stage, detail);
        }
    };

    // Real byte progress: sum the on-disk size of every distinct GLB whose
    // instances are no longer pending (their asset finished loading).
    if let Some(mut lb) = loading_bytes {
        use std::collections::HashSet;
        let mut done: HashSet<&str> = HashSet::new();
        for data in loaded_q.iter() {
            if let Some(p) = data.model_path.as_deref() {
                done.insert(p);
            }
        }
        lb.loaded = done.iter().filter_map(|p| progress.gltf_sizes.get(*p)).sum();
        lb.total = progress.gltf_sizes.values().sum();
    }

    // Empty-scene case: complete every task immediately so the grace
    // timer is the only thing holding the loading screen up.
    if progress.total_instances == 0 {
        for task in [progress.loading_task, progress.processing_task]
            .into_iter()
            .flatten()
        {
            tasks.complete(task);
        }
        mark("models loaded", "no models".to_string());
        mark("models spawned", "no models".to_string());
        return;
    }

    // ── Phase 1: asset I/O ─────────────────────────────────────────────
    //
    // An instance is "loaded" iff:
    //  * its `model_path` is `None` (nothing to load), or
    //  * it no longer carries `PendingMeshInstanceRehydrate` — meaning the
    //    Gltf asset finished loading and `finish_mesh_instance_rehydrate`
    //    advanced it.
    let total = progress.total_instances;
    let still_pending = pending.iter().filter(|d| d.model_path.is_some()).count() as u32;
    let loaded = total.saturating_sub(still_pending);

    let mut current_loading_name: Option<String> = None;
    if loaded < total {
        // Find the first not-yet-loaded model's name to surface as detail.
        if let Some(p) = pending.iter().find_map(|d| d.model_path.as_ref()) {
            current_loading_name = Some(short_name(p));
        }
    }

    if let Some(task) = progress.loading_task {
        if loaded > progress.last_loaded {
            tasks.advance(task, loaded - progress.last_loaded);
            progress.last_loaded = loaded;
        }
        match &current_loading_name {
            Some(n) => tasks.set_detail(task, n.clone()),
            None => {
                if loaded >= total {
                    tasks.complete(task);
                }
            }
        }
    }
    if loaded >= total {
        mark("models loaded", format!("{total} model instance(s), {}", megabytes(total_bytes)));
    }

    // ── Phase 2: scene spawn ───────────────────────────────────────────
    //
    // An instance is "processed" iff:
    //  * its `model_path` is `None` (no GLB, trivially processed), or
    //  * its parent has a `SceneRoot` child whose own `Children` is
    //    non-empty — proof Bevy's `SceneSpawner` has actually run
    //    `write_to_world`, not just queued the asset.
    let mut next_processing_name: Option<String> = None;
    let processed: u32 = instances
        .iter()
        .filter(|(data, kids, load_failed)| {
            // No model to load, or the load failed (missing file) — either way
            // there's nothing left to wait on, so count it as processed.
            if data.model_path.is_none() || *load_failed {
                return true;
            }
            let done = kids
                .map(|c| {
                    c.iter().any(|child| {
                        scene_roots.contains(child)
                            && children_q
                                .get(child)
                                .map(|gc| !gc.is_empty())
                                .unwrap_or(false)
                    })
                })
                .unwrap_or(false);
            if !done && next_processing_name.is_none() {
                if let Some(p) = data.model_path.as_ref() {
                    next_processing_name = Some(short_name(p));
                }
            }
            done
        })
        .count() as u32;

    if let Some(task) = progress.processing_task {
        if processed > progress.last_processed {
            tasks.advance(task, processed - progress.last_processed);
            progress.last_processed = processed;
        }
        match next_processing_name {
            Some(n) => tasks.set_detail(task, n),
            None => {
                if processed >= total {
                    tasks.complete(task);
                }
            }
        }
    }
    if processed >= total {
        mark("models spawned", format!("{total} model instance(s)"));
    }
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}

// ============================================================================
// In-editor loading overlay
// ============================================================================
//
// When a tab switch or scene-open queues fresh `MeshInstanceData` entities
// that aren't yet warm, we want the modal overlay (rendered by
// `renzora_splash::editor_loading_overlay_ui_system`) to surface progress
// the same way the splash bar does on initial project load. The trigger
// has to handle a quirk: even *warm* tab switches go through
// rehydrate_mesh_instances → finish_mesh_instance_rehydrate → SceneSpawner
// for ~3 frames before the SceneRoot is populated. Showing the overlay
// for those 3 frames would just be a flicker. So we only fire the overlay
// after pending work has persisted for `OVERLAY_DELAY_FRAMES` — that
// hysteresis hides warm switches and lets cold ones (real I/O) surface
// the bar.

#[derive(Resource, Default)]
struct EditorLoadProgress {
    /// Frames we've continuously seen pending work without dismissing.
    /// Used as both a debounce (don't flash for warm switches) and the
    /// trigger threshold for activating the overlay.
    pending_frames: u32,
    /// Frames we've continuously seen *no* pending work. Used as a
    /// hold-down so the overlay doesn't flicker if a single frame's
    /// query happens to be momentarily empty between phases.
    idle_frames: u32,
    /// Whether tasks have been registered (and the overlay is showing).
    /// Distinct from `EditorLoadingOverlayActive` so we can tear down
    /// task state and overlay flag together.
    active: bool,
    loading_task: Option<LoadingTaskHandle>,
    processing_task: Option<LoadingTaskHandle>,
    /// Number of `MeshInstanceData` entities the overlay was kicked off
    /// with. Used as the per-task denominator for the duration of this
    /// overlay session — additional entities arriving mid-session don't
    /// rebase the bar's total (would look like progress going backwards).
    total_instances: u32,
    last_loaded: u32,
    last_processed: u32,
}

/// Frames of continuous pending work before the overlay activates. ~166ms
/// at 60fps — below human perception but enough to absorb warm tab
/// switches that complete in ≤10 frames.
const OVERLAY_DELAY_FRAMES: u32 = 10;
/// Frames of no pending work before we tear down an active overlay. Holds
/// the overlay one extra beat so a single empty-query frame between
/// phases doesn't dismiss it prematurely.
const OVERLAY_HOLD_DOWN_FRAMES: u32 = 6;

/// Tick the in-editor loading overlay. Mirrors `tick_scene_load_progress`
/// for the splash but auto-detects work instead of being kicked off by an
/// `OnEnter` system, and uses hysteresis to skip the overlay for warm
/// switches.
fn tick_editor_load_progress(
    mut progress: ResMut<EditorLoadProgress>,
    mut tasks: ResMut<LoadingTasks>,
    mut overlay_active: ResMut<EditorLoadingOverlayActive>,
    // `Without<ImportedRoot>` is the load-vs-done discriminator. Drop
    // paths attach `ImportedRoot` immediately (so they never enter the
    // count); the load path attaches it via the SceneInstanceReady
    // observer once Bevy has fully populated the GLB hierarchy. After
    // that, `flatten_pending_scenes` may despawn the SceneRoot child
    // (gltf wrappers get collapsed into the parent), which would make
    // a "has SceneRoot child with non-empty children" check
    // misclassify finished entities as still-unspawned forever. Filtering
    // on `ImportedRoot` is both simpler and survives flatten.
    instances: Query<
        (
            &MeshInstanceData,
            Option<&Children>,
            Has<scene_io::MeshInstanceLoadFailed>,
        ),
        Without<ImportedRoot>,
    >,
    pending: Query<&MeshInstanceData, With<scene_io::PendingMeshInstanceRehydrate>>,
    children_q: Query<&Children>,
    scene_roots: Query<Entity, With<bevy::world_serialization::WorldAssetRoot>>,
) {
    // Two sub-counts feed the two-phase bar:
    //   * `pending_assets` — entities still carrying
    //     `PendingMeshInstanceRehydrate` (Gltf hasn't loaded yet).
    //   * `unspawned` — entities whose model_path is set but whose
    //     SceneRoot child either doesn't exist yet or has no Children.
    // `pending_assets` is a subset of `unspawned` (an entity pending the
    // Gltf has no SceneRoot child either), so the total-outstanding
    // work is just `unspawned`.
    let pending_assets = pending.iter().filter(|d| d.model_path.is_some()).count() as u32;
    let unspawned: u32 = instances
        .iter()
        .filter(|(data, kids, load_failed)| {
            // Nothing to load, or the load failed (missing file) — not
            // outstanding work, so don't count it as unspawned.
            if data.model_path.is_none() || *load_failed {
                return false;
            }
            !instance_populated(*kids, &scene_roots, &children_q)
        })
        .count() as u32;
    let total_outstanding = unspawned;
    let any_pending = total_outstanding > 0;

    // Hysteresis state machine: count up while pending, count down once
    // pending drops to zero. The overlay only activates after a sustained
    // pending streak; only deactivates after a sustained idle streak.
    if any_pending {
        progress.pending_frames = progress.pending_frames.saturating_add(1);
        progress.idle_frames = 0;
    } else {
        progress.idle_frames = progress.idle_frames.saturating_add(1);
        if !progress.active {
            // Reset the build-up counter so a future warm-switch-then-
            // cold-switch sequence still gets fresh hysteresis.
            progress.pending_frames = 0;
        }
    }

    // ── Activation: enough sustained work to justify a modal ──────────
    if !progress.active && progress.pending_frames >= OVERLAY_DELAY_FRAMES && any_pending {
        // `total_outstanding` is the denominator for both phases — set
        // *now* so a slow trickle of new entities mid-session doesn't
        // make the bar appear to move backwards.
        let n = total_outstanding.max(1);
        progress.total_instances = n;
        progress.last_loaded = 0;
        progress.last_processed = 0;
        progress.loading_task = Some(tasks.register("Loading assets", n));
        progress.processing_task = Some(tasks.register("Processing models", n));
        progress.active = true;
        overlay_active.0 = true;
    }

    // ── While active: update the bar ──────────────────────────────────
    if progress.active {
        let total = progress.total_instances;
        // Loaded = total minus those still pending the GLB asset.
        let loaded = total.saturating_sub(pending_assets);
        // Processed = total minus those whose SceneRoot is unpopulated.
        let processed = total.saturating_sub(unspawned);

        let mut current_loading_name: Option<String> = None;
        if let Some(p) = pending.iter().find_map(|d| d.model_path.as_ref()) {
            current_loading_name = Some(short_name(p));
        }

        if let Some(task) = progress.loading_task {
            if loaded > progress.last_loaded {
                tasks.advance(task, loaded - progress.last_loaded);
                progress.last_loaded = loaded;
            }
            match &current_loading_name {
                Some(n) => tasks.set_detail(task, n.clone()),
                None => {
                    if loaded >= total {
                        tasks.complete(task);
                    }
                }
            }
        }

        let mut next_processing_name: Option<String> = None;
        for (data, kids, load_failed) in instances.iter() {
            if data.model_path.is_none() || load_failed {
                continue;
            }
            if !instance_populated(kids, &scene_roots, &children_q) {
                if let Some(p) = data.model_path.as_ref() {
                    next_processing_name = Some(short_name(p));
                    break;
                }
            }
        }

        if let Some(task) = progress.processing_task {
            if processed > progress.last_processed {
                tasks.advance(task, processed - progress.last_processed);
                progress.last_processed = processed;
            }
            match next_processing_name {
                Some(n) => tasks.set_detail(task, n),
                None => {
                    if processed >= total {
                        tasks.complete(task);
                    }
                }
            }
        }

        // ── Deactivation: enough sustained idleness to dismiss ────────
        if !any_pending && progress.idle_frames >= OVERLAY_HOLD_DOWN_FRAMES {
            progress.active = false;
            progress.loading_task = None;
            progress.processing_task = None;
            progress.total_instances = 0;
            progress.last_loaded = 0;
            progress.last_processed = 0;
            progress.pending_frames = 0;
            progress.idle_frames = 0;
            overlay_active.0 = false;
            // Drain completed tasks so the next overlay starts with a
            // clean register set. Each session registers fresh tasks.
            tasks.clear_completed();
        }
    }
}

/// Whether a model instance's file has been spawned into it: a world-asset
/// root child that has filled in, or anything else under it, which is the
/// spawned scene after the flatten pass has dissolved the wrapper into the
/// instance.
///
/// The second half is what makes this the check rather than a hint. The
/// tracker otherwise leans on `ImportedRoot`, put on by the spawn-ready
/// observer, and an instance that ended up flattened without it (seen after
/// undoing the delete of a model) has no world-asset root left to find either,
/// so it counted as unspawned forever and held the loading overlay at its last
/// percentage with the model plainly in the scene.
fn instance_populated(
    kids: Option<&Children>,
    scene_roots: &Query<Entity, With<bevy::world_serialization::WorldAssetRoot>>,
    children_q: &Query<&Children>,
) -> bool {
    kids.is_some_and(|c| {
        c.iter().any(|child| {
            !scene_roots.contains(child) || children_q.get(child).is_ok_and(|gc| !gc.is_empty())
        })
    })
}

/// Strip the directory off an asset-relative path, leaving just the
/// file name (or stem) for display in the loading screen.
fn short_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| path.to_string())
}

#[cfg(test)]
mod missing_file_tests {
    use super::*;
    use renzora::core::EditorOpenTab;

    fn project(name: &str) -> CurrentProject {
        let root = std::env::temp_dir().join(format!("renzora_missing_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("scenes")).unwrap();
        std::fs::write(root.join("scenes/main.bsn"), "").unwrap();
        CurrentProject { path: root, config: Default::default() }
    }

    fn tab(path: &str) -> EditorOpenTab {
        EditorOpenTab { path: path.into(), kind: "scene".into() }
    }

    /// The bug: a deleted test scene stayed the remembered scene, so the main
    /// scene opened under its name and every save went to the dead path.
    #[test]
    fn a_deleted_last_scene_and_its_tab_are_forgotten() {
        let mut p = project("deleted");
        p.config.editor_last_scene = Some("scenes/gone.bsn".into());
        p.config.editor_open_tabs = vec![tab("scenes/main.bsn"), tab("scenes/gone.bsn")];

        assert!(forget_missing_files(&mut p));
        let _ = std::fs::remove_dir_all(&p.path);

        assert_eq!(p.config.editor_last_scene, None);
        assert_eq!(p.config.editor_open_tabs, [tab("scenes/main.bsn")]);
    }

    #[test]
    fn files_that_exist_are_kept() {
        let mut p = project("kept");
        p.config.editor_last_scene = Some("scenes/main.bsn".into());
        p.config.editor_open_tabs = vec![tab("scenes/main.bsn")];

        assert!(!forget_missing_files(&mut p), "nothing to drop means no rewrite");
        let _ = std::fs::remove_dir_all(&p.path);

        assert_eq!(p.config.editor_last_scene.as_deref(), Some("scenes/main.bsn"));
        assert_eq!(p.config.editor_open_tabs.len(), 1);
    }
}

#[cfg(test)]
mod short_name_tests {
    use super::short_name;

    #[test]
    fn the_directory_is_stripped_for_display() {
        assert_eq!(short_name("scenes/levels/forest.scene"), "forest.scene");
        assert_eq!(short_name("forest.scene"), "forest.scene");
    }

    /// Asset paths are `/`-separated, but this also receives OS paths from the
    /// open-file dialog — which on Windows are backslash-separated.
    ///
    /// Windows-only on purpose. `Path::file_name` is platform-aware: `\` is a
    /// separator on Windows and an ordinary filename character everywhere else,
    /// so on Linux this same call correctly returns the whole string. Asserting
    /// it unconditionally is what made this test fail in CI against code that
    /// was right — the backslash case cannot arise on a platform that does not
    /// produce backslash paths.
    #[cfg(windows)]
    #[test]
    fn windows_separators_are_handled_on_windows() {
        assert_eq!(short_name(r"C:\projects\game\scenes\forest.scene"), "forest.scene");
    }

    /// The cross-platform half: a `/`-separated asset path is what the loading
    /// screen actually sees, and that works everywhere.
    #[test]
    fn asset_paths_are_stripped_on_every_platform() {
        assert_eq!(short_name("scenes/levels/forest.scene"), "forest.scene");
        assert_eq!(short_name("a/b/c/d/e.glb"), "e.glb");
    }

    /// A path with no file name still has to render *something* in the loading
    /// screen — a blank line reads as a hung load.
    #[test]
    fn a_path_with_no_file_name_falls_back_to_the_whole_path() {
        assert_eq!(short_name(""), "");
        assert_eq!(short_name("scenes/.."), "scenes/..");
    }

    #[test]
    fn a_trailing_separator_still_finds_the_last_segment() {
        assert_eq!(short_name("scenes/levels/"), "levels");
    }
}

/// Observer: surface "scene loaded but skipped some component types" as
/// a warning toast. Fires whenever the lossy scene deserializer in
/// `renzora_engine::scene_io` had to drop one or more entries to make
/// the scene parseable — typically because the scene was authored in
/// the editor but is being loaded in a runtime build that doesn't
/// register editor-only types.
fn toast_skipped_scene_types(
    trigger: On<scene_io::SceneLoadedWithSkippedTypes>,
    mut toasts: ResMut<renzora_ui::Toasts>,
) {
    let event = trigger.event();
    if event.skipped.is_empty() {
        return;
    }
    let scene_label = std::path::Path::new(&event.path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("scene");
    let message = match event.skipped.len() {
        1 => format!(
            "{}: skipped unregistered type `{}`",
            scene_label, event.skipped[0]
        ),
        n => format!(
            "{}: skipped {} unregistered types (`{}`, …)",
            scene_label, n, event.skipped[0]
        ),
    };
    toasts.warning(message);
}

// ============================================================================
// Plugin
// ============================================================================

/// Inspector section for [`renzora::SceneInstance`] roots: shows the source
/// scene (read-only — reference edits go through drag-drop / context menu, not
/// free text) and the world-streaming controls. Streaming radii only matter
/// when `Streamed` is on, but they stay visible either way — hiding fields on
/// a toggle makes the section jump around while scrubbing.
fn scene_instance_entry() -> renzora_editor_framework::InspectorEntry {
    use renzora_editor_framework::{FieldDef, FieldType, FieldValue, InspectorEntry};
    InspectorEntry {
        type_id: "scene_instance",
        display_name: "Scene Instance",
        icon: "film-slate",
        category: "object",
        has_fn: |world, entity| world.get::<renzora::SceneInstance>(entity).is_some(),
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Source",
                field_type: FieldType::ReadOnly,
                get_fn: |w, e| {
                    w.get::<renzora::SceneInstance>(e)
                        .map(|i| FieldValue::ReadOnly(i.source.clone()))
                },
                set_fn: |_, _, _| {},
            },
            FieldDef {
                name: "Streamed",
                field_type: FieldType::Bool,
                get_fn: |w, e| {
                    w.get::<renzora::SceneInstance>(e)
                        .map(|i| FieldValue::Bool(i.streamed))
                },
                set_fn: |w, e, v| {
                    if let FieldValue::Bool(b) = v {
                        if let Some(mut i) = w.get_mut::<renzora::SceneInstance>(e) {
                            if i.streamed != b {
                                i.streamed = b;
                            }
                        }
                    }
                },
            },
            FieldDef {
                name: "Load Radius",
                field_type: FieldType::Float { speed: 1.0, min: 1.0, max: 10000.0 },
                get_fn: |w, e| {
                    w.get::<renzora::SceneInstance>(e)
                        .map(|i| FieldValue::Float(i.load_radius))
                },
                set_fn: |w, e, v| {
                    if let FieldValue::Float(r) = v {
                        if let Some(mut i) = w.get_mut::<renzora::SceneInstance>(e) {
                            if i.load_radius != r {
                                i.load_radius = r;
                            }
                        }
                    }
                },
            },
            FieldDef {
                name: "Unload Radius",
                field_type: FieldType::Float { speed: 1.0, min: 1.0, max: 10000.0 },
                get_fn: |w, e| {
                    w.get::<renzora::SceneInstance>(e)
                        .map(|i| FieldValue::Float(i.unload_radius))
                },
                set_fn: |w, e, v| {
                    if let FieldValue::Float(r) = v {
                        if let Some(mut i) = w.get_mut::<renzora::SceneInstance>(e) {
                            if i.unload_radius != r {
                                i.unload_radius = r;
                            }
                        }
                    }
                },
            },
        ],
    }
}

#[derive(Default)]
pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ScenePlugin");
        use renzora_editor_framework::{AppEditorExt, ComponentIconEntry};

        // Hierarchy icon for nested-scene instance roots (distinguishes them
        // from plain folder-like grouping entities).
        app.register_component_icon(ComponentIconEntry {
            type_id: std::any::TypeId::of::<renzora::SceneInstance>(),
            name: "Scene Instance",
            icon: "film-slate",
            color: [170, 200, 255],
            priority: 75,
            dynamic_icon_fn: None,
        });
        app.register_inspector(scene_instance_entry());
        let pending_pipelines = PendingPipelines::default();
        if let Some(render_app) = app.get_sub_app_mut(bevy::render::RenderApp) {
            render_app
                .insert_resource(pending_pipelines.clone())
                .add_systems(bevy::render::Render, count_pending_pipelines);
        }
        app.init_resource::<SceneTabBuffers>()
            .insert_resource(pending_pipelines)
            .init_resource::<renzora::load_timeline::LoadTimeline>()
            .init_resource::<renzora::load_timeline::AssetMemory>()
            .add_systems(
                Update,
                (
                    track_load_timeline,
                    track_model_asset_events,
                    measure_asset_memory,
                    measure_gpu_allocations,
                    install_gpu_probe,
                ),
            )
            .init_resource::<SceneLoadProgress>()
            .init_resource::<EditorLoadProgress>()
            .init_resource::<TabAssetCache>()
            .init_resource::<tab_asset_cache::SceneDiagSnapshot>()
            // Refresh the Scene Diagnostics probe's input snapshot. Exclusive
            // system (uses a dozen queries internally via SystemState), cached
            // in a resource so the probe's report is a cheap read.
            //
            // It does several full-scene passes + clones every StandardMaterial
            // handle, so its cost scales with the scene (O(entities+materials)).
            // That's fine for a debug readout but ruinous on a 100k+ entity
            // scene if it runs every frame regardless of whether anyone looks,
            // so gate it on a companion actually watching the probe, and
            // throttle to 4 Hz while one is. Unwatched it costs nothing; the
            // snapshot it feeds is read by nothing else (see `diagnostics`).
            .add_systems(
                Update,
                tab_asset_cache::update_scene_diag_snapshot
                    .run_if(in_state(SplashState::Editor))
                    .run_if(renzora::probes::probe_watched(diagnostics::PROBE_ID))
                    .run_if(bevy::time::common_conditions::on_timer(
                        std::time::Duration::from_millis(250),
                    )),
            )
            // Reload the open scene when its file changes outside the editor.
            // Chained because the collector hands the exclusive system its work
            // through a resource, and an intervening frame would mean a save
            // landing a frame later than it needs to.
            .init_resource::<hot_reload::PendingSceneReloads>()
            .init_resource::<hot_reload::SceneConflicts>()
            .add_systems(
                Update,
                (
                    hot_reload::collect_scene_changes,
                    hot_reload::apply_scene_reloads,
                    // After the reload, so a conflict raised this frame gets its
                    // prompt on the same frame rather than the next.
                    conflict_prompt::conflict_prompt_buttons,
                    conflict_prompt::spawn_conflict_prompt,
                )
                    .chain()
                    .run_if(in_state(SplashState::Editor)),
            )
            // Warn when the scene points at a file that has just been deleted.
            // Not chained with the reload above: it reads the same messages but
            // acts on the removals that one deliberately ignores.
            .add_systems(
                Update,
                missing_assets::report_missing_assets.run_if(in_state(SplashState::Editor)),
            )
            .add_plugins(SceneDiagnostics)
            // When the user closes a tab, drop its strong handles so
            // the assets it pinned can evict (assuming no other tab
            // still references them).
            .add_observer(tab_asset_cache::evict_closed_tab)
            .add_observer(toast_skipped_scene_types)
            // Simulate-mode scene snapshot/restore. Lives here because this is the
            // crate with both `scene_io` and `despawn_scene_entities` — restoring
            // MUST despawn the live scene before re-spawning the snapshot, exactly
            // as the tab switcher does, or every entity gets duplicated.
            .init_resource::<SimulateSnapshot>()
            .add_observer(on_snapshot_scene_for_simulate)
            .add_observer(on_restore_simulate_snapshot)
            // Re-entering Splash means the user is opening a different
            // project (or returning to the picker). Despawn the previous
            // project's scene entities and clear per-tab caches so the
            // new Loading session starts on a clean world.
            .add_systems(OnEnter(SplashState::Splash), teardown_for_project_switch)
            // Scene load shifts to `OnEnter(Loading)` — the user's scene
            // file is parsed and entities rehydrated *behind* the loading
            // screen. The screen stays up until every GLB asset has been
            // fetched and instantiated, so the editor only opens onto a
            // fully-populated, race-free entity tree.
            .add_systems(OnEnter(SplashState::Loading), load_scene_on_enter_loading)
            // Splash → Editor transition: the splash's Loading + Processing
            // tasks have completed and (with the min-frames-remaining grace)
            // are still sitting in `LoadingTasks` with completed=total. If
            // we left them, the editor overlay's first session would
            // aggregate them into its bar and start partway full. Drop
            // them so each editor overlay session starts at 0%.
            .add_systems(
                OnEnter(SplashState::Editor),
                |mut tasks: ResMut<LoadingTasks>| {
                    tasks.clear();
                },
            )
            // Rehydrate systems run during Loading too. They drive GLB
            // resolution + spawn while the loading screen ticks.
            .add_systems(
                Update,
                (
                    // Chained: persisted mesh edits must replace the pristine
                    // primitive mesh in the same frame it rehydrates.
                    (scene_io::rehydrate_meshes, scene_io::apply_edited_meshes).chain(),
                    scene_io::rehydrate_cameras,
                    scene_io::rehydrate_suns,
                    scene_io::rehydrate_lights,
                    scene_io::rehydrate_visibility,
                    scene_io::rehydrate_mesh_instances,
                    scene_io::finish_mesh_instance_rehydrate,
                    tab_asset_cache::cache_added_mesh_instances,
                    tick_scene_load_progress,
                    tick_texture_progress,
                )
                    .run_if(in_state(SplashState::Loading)),
            )
            // Editor-state systems unchanged: rehydrate stays available
            // (drop creates `MeshInstanceData` post-load, so the rehydrate
            // hooks still need to fire), plus the file-action handlers.
            .add_systems(
                Update,
                (
                    (scene_io::rehydrate_meshes, scene_io::apply_edited_meshes).chain(),
                    scene_io::rehydrate_cameras,
                    scene_io::rehydrate_suns,
                    scene_io::rehydrate_lights,
                    scene_io::rehydrate_visibility,
                    scene_io::rehydrate_mesh_instances,
                    scene_io::finish_mesh_instance_rehydrate,
                    tab_asset_cache::cache_added_mesh_instances,
                    tick_editor_load_progress,
                    detect_file_keybindings,
                    save_scene_system,
                    save_as_scene_system,
                    thumbnail::capture_scene_thumbnail,
                    new_scene_system,
                    open_scene_system,
                    open_scene_path_system,
                    handle_tab_switch,
                )
                    .run_if(in_state(SplashState::Editor)),
            );
    }
}

renzora::add!(ScenePlugin, Editor);
