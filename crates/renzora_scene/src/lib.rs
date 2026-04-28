//! Renzora Scene — editor-side scene plugin that wires save/load to keybindings and splash state.
//!
//! The actual save/load and rehydration logic lives in `renzora_engine::scene_io`.

use bevy::prelude::*;

use renzora::core::{CurrentProject, MeshInstanceData, SaveSceneRequested, SaveAsSceneRequested, NewSceneRequested, OpenSceneRequested, ToggleSettingsRequested, HideInHierarchy, EditorCamera, SceneCamera, TabSwitchRequest, TabSceneSnapshot, SceneTabBuffers};
use renzora_camera::OrbitCameraState;
use renzora_keybindings::{EditorAction, KeyBindings};
use renzora_engine::scene_io;
use renzora_editor::SplashState;
use renzora_splash::{LoadingTaskHandle, LoadingTasks};

// Re-export so downstream code that was using `renzora_scene::{save_scene, load_scene, ...}` still works.
pub use scene_io::{save_scene, load_scene, save_current_scene, load_current_scene};

mod panel;
pub use panel::ScenesPanel;

mod tab_asset_cache;
pub use tab_asset_cache::TabAssetCache;

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
            to_despawn.push(entity);
        }
    }
    for &entity in &to_despawn {
        if world.get_entity(entity).is_ok() {
            world.despawn(entity);
        }
    }
    to_despawn
}

fn handle_tab_switch(world: &mut World) {
    let Some(request) = world.remove_resource::<TabSwitchRequest>() else {
        return;
    };

    let old_id = request.old_tab_id;
    let new_id = request.new_tab_id;

    // 1. Serialize current scene entities into buffer for old tab
    let scene_ron = scene_io::serialize_scene_to_string(world)
        .unwrap_or_else(|e| {
            warn!("Failed to serialize scene for tab {}: {}", old_id, e);
            "(entities: {}, resources: {})".to_string()
        });

    // 2. Save camera state
    let (focus, distance, yaw, pitch) = if let Some(orbit) = world.get_resource::<OrbitCameraState>() {
        (orbit.focus.to_array(), orbit.distance, orbit.yaw, orbit.pitch)
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
        buffers.buffers.insert(old_id, snapshot);
    }

    // 3. Despawn all scene entities
    despawn_scene_entities(world);

    // 4. If target tab has a buffer, deserialize it + restore camera
    let target_snapshot = world
        .get_resource_mut::<SceneTabBuffers>()
        .and_then(|mut buffers| buffers.buffers.remove(&new_id));

    if let Some(snap) = target_snapshot {
        scene_io::load_scene_from_string(world, &snap.scene_ron);

        // Restore camera
        if let Some(mut orbit) = world.get_resource_mut::<OrbitCameraState>() {
            orbit.focus = Vec3::from_array(snap.camera_focus);
            orbit.distance = snap.camera_distance;
            orbit.yaw = snap.camera_yaw;
            orbit.pitch = snap.camera_pitch;
        }
    } else {
        // New empty tab — reset camera to default
        if let Some(mut orbit) = world.get_resource_mut::<OrbitCameraState>() {
            let def = OrbitCameraState::default();
            orbit.focus = def.focus;
            orbit.distance = def.distance;
            orbit.yaw = def.yaw;
            orbit.pitch = def.pitch;
        }
    }

    renzora::core::console_log::console_info(
        "Scene",
        format!("Switched from tab {} to tab {}", old_id, new_id),
    );
}

// ============================================================================
// Orbit camera <-> scene component helpers
// ============================================================================

/// Stamp the current `OrbitCameraState` resource onto the `SceneCamera` entity
/// so it gets serialized into the scene RON.
fn stamp_orbit_on_scene_camera(world: &mut World) {
    let Some(orbit) = world.get_resource::<OrbitCameraState>().map(|o| o.clone()) else {
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
    let result: Option<(Entity, OrbitCameraState)> = query
        .iter(world)
        .next()
        .map(|(e, o)| (e, o.clone()));
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
    play_mode: Option<Res<renzora::core::PlayModeState>>,
) {
    if play_mode.as_ref().map_or(false, |pm| pm.is_in_play_mode()) { return; }
    if keybindings.rebinding.is_some() { return; }

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

    // Get the active tab's scene_path
    let tab_scene_path = world
        .get_resource::<renzora_ui::DocumentTabState>()
        .and_then(|tabs| {
            tabs.tabs.get(tabs.active_tab)
                .and_then(|tab| tab.scene_path.clone())
        });

    let Some(tab_scene_path) = tab_scene_path else {
        // No path yet — redirect to Save As
        info!("Save: active tab has no scene_path, redirecting to Save As");
        world.insert_resource(SaveAsSceneRequested);
        return;
    };

    let Some(project) = world.get_resource::<CurrentProject>() else {
        warn!("No project open — cannot save scene");
        return;
    };
    let save_path = project.resolve_path(&tab_scene_path);
    info!("Save: active tab scene_path={:?}, resolved={}", tab_scene_path, save_path.display());

    stamp_orbit_on_scene_camera(world);

    // Propagate interior edits of nested scene instances back to their
    // source .ron files. Only descendants are written — the instance
    // root's own Transform stays in the host and never leaks into source.
    scene_io::save_all_scene_instances(world, &save_path);

    if let Err(e) = scene_io::save_scene(world, &save_path) {
        error!("Failed to save scene: {}", e);
        return;
    }

    // Clear modified flag
    if let Some(mut tabs) = world.get_resource_mut::<renzora_ui::DocumentTabState>() {
        let active = tabs.active_tab;
        if let Some(tab) = tabs.tabs.get_mut(active) {
            tab.is_modified = false;
        }
    }

    // Remember this scene as the last-open so the editor reopens it next launch.
    if let Some(mut project) = world.get_resource_mut::<CurrentProject>() {
        if project.config.editor_last_scene.as_deref() != Some(tab_scene_path.as_str()) {
            project.config.editor_last_scene = Some(tab_scene_path.clone());
            let _ = project.save_config();
        }
    }

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
        let file = rfd::FileDialog::new()
            .set_title("Save Scene As")
            .set_directory(&scenes_dir)
            .add_filter("Scene File", &["ron"])
            .set_file_name("new_scene.ron")
            .save_file();

        let Some(file_path) = file else { return };

        // Save the scene to the chosen path
        stamp_orbit_on_scene_camera(world);
        if let Err(e) = scene_io::save_scene(world, &file_path) {
            error!("Failed to save scene: {}", e);
            return;
        }

        // Update main_scene to point to the new file
        let relative = {
            let mut project = world.resource_mut::<CurrentProject>();
            let rel = project.make_relative(&file_path);
            if let Some(ref r) = rel {
                project.config.main_scene = r.clone();
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

    // Despawn all scene entities (keep editor infrastructure)
    despawn_scene_entities(world);

    // Update active tab
    if let Some(mut tabs) = world.get_resource_mut::<renzora_ui::DocumentTabState>() {
        let active = tabs.active_tab;
        if let Some(tab) = tabs.tabs.get_mut(active) {
            tab.name = "Untitled Scene".to_string();
            tab.scene_path = None;
            tab.is_modified = false;
        }
    }

    // Reset camera
    if let Some(mut orbit) = world.get_resource_mut::<OrbitCameraState>() {
        let def = OrbitCameraState::default();
        orbit.focus = def.focus;
        orbit.distance = def.distance;
        orbit.yaw = def.yaw;
        orbit.pitch = def.pitch;
    }

    renzora::core::console_log::console_info("Scene", "New scene created (cleared all entities)");
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
        let file = rfd::FileDialog::new()
            .set_title("Open Scene")
            .set_directory(&scenes_dir)
            .add_filter("Scene File", &["ron"])
            .pick_file();

        let Some(file_path) = file else { return };

        // Despawn current scene entities
        despawn_scene_entities(world);

        // Load the new scene
        scene_io::load_scene(world, &file_path);
        extract_orbit_from_scene_camera(world);

        // Update main_scene to point to the opened file
        let relative = {
            let mut project = world.resource_mut::<CurrentProject>();
            let rel = project.make_relative(&file_path);
            if let Some(ref r) = rel {
                project.config.main_scene = r.clone();
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

        renzora::core::console_log::console_success(
            "Scene",
            format!("Opened scene {}", file_path.display()),
        );
    }
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
}

/// Loads the scene file the moment we transition into `SplashState::Loading`,
/// then registers a `LoadingTasks` task whose progress reflects how many
/// `MeshInstanceData` entities still need their GLB scene fetched and
/// instantiated. The loading screen stays up until every pending GLB has
/// landed (plus the standard `min_frames_remaining` grace period), so the
/// editor only opens onto a fully-populated entity tree.
fn load_scene_on_enter_loading(world: &mut World) {
    info!("[loading] entered Loading state, kicking off scene load");

    // Ensure the asset reader knows the project path before loading the scene.
    if let Some(project) = world.get_resource::<CurrentProject>() {
        let path = project.path.clone();
        if let Some(asset_path) = world.get_resource::<renzora_engine::ProjectAssetPath>() {
            info!("[scene] Syncing project asset path: {}", path.display());
            asset_path.set(path);
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

    if let Some((scene_path, scene_name)) = scene_info {
        if let Some(mut tabs) = world.get_resource_mut::<renzora_ui::DocumentTabState>() {
            if let Some(tab) = tabs.tabs.get_mut(0) {
                tab.name = scene_name;
                tab.scene_path = Some(scene_path);
            }
        }
    }

    // Count the model instances we need to resolve. Used as the denominator
    // for the progress bar.
    let instance_count = {
        let mut q = world.query_filtered::<Entity, With<MeshInstanceData>>();
        q.iter(world).count() as u32
    };

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
    });

    // Ask the hierarchy to auto-select its top entity once the cache is
    // populated. The flag is consumed by `auto_select_first_hierarchy_entity`
    // in `renzora_hierarchy` — we can't do it here because entities have
    // only just been queued for spawn and the hierarchy tree won't be
    // built until the next frame.
    if let Some(mut flag) = world.get_resource_mut::<renzora_editor::AutoSelectFirstHierarchyEntity>() {
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
fn tick_scene_load_progress(
    progress: Option<ResMut<SceneLoadProgress>>,
    mut tasks: ResMut<LoadingTasks>,
    instances: Query<(&MeshInstanceData, Option<&Children>)>,
    pending: Query<&MeshInstanceData, With<scene_io::PendingMeshInstanceRehydrate>>,
    children_q: Query<&Children>,
    scene_roots: Query<Entity, With<SceneRoot>>,
) {
    let Some(mut progress) = progress else { return };

    // Empty-scene case: complete every task immediately so the grace
    // timer is the only thing holding the loading screen up.
    if progress.total_instances == 0 {
        for task in [progress.loading_task, progress.processing_task]
            .into_iter()
            .flatten()
        {
            tasks.complete(task);
        }
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
    let still_pending = pending
        .iter()
        .filter(|d| d.model_path.is_some())
        .count() as u32;
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
        .filter(|(data, kids)| {
            if data.model_path.is_none() {
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

// ============================================================================
// Plugin
// ============================================================================

#[derive(Default)]
pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ScenePlugin");
        use renzora_editor::{AppEditorExt, ComponentIconEntry};
        app.register_panel(panel::ScenesPanel::default());

        // Hierarchy icon for nested-scene instance roots (distinguishes them
        // from plain folder-like grouping entities).
        app.register_component_icon(ComponentIconEntry {
            type_id: std::any::TypeId::of::<renzora::SceneInstance>(),
            name: "Scene Instance",
            icon: egui_phosphor::regular::FILM_STRIP,
            color: [170, 200, 255],
            priority: 75,
            dynamic_icon_fn: None,
        });
        app.init_resource::<SceneTabBuffers>()
            .init_resource::<SceneLoadProgress>()
            .init_resource::<TabAssetCache>()
            // When the user closes a tab, drop its strong handles so
            // the assets it pinned can evict (assuming no other tab
            // still references them).
            .add_observer(tab_asset_cache::evict_closed_tab)
            // Scene load shifts to `OnEnter(Loading)` — the user's scene
            // file is parsed and entities rehydrated *behind* the loading
            // screen. The screen stays up until every GLB asset has been
            // fetched and instantiated, so the editor only opens onto a
            // fully-populated, race-free entity tree.
            .add_systems(OnEnter(SplashState::Loading), load_scene_on_enter_loading)
            // Rehydrate systems run during Loading too. They drive GLB
            // resolution + spawn while the loading screen ticks.
            .add_systems(
                Update,
                (
                    scene_io::rehydrate_meshes,
                    scene_io::rehydrate_cameras,
                    scene_io::rehydrate_suns,
                    scene_io::rehydrate_lights,
                    scene_io::rehydrate_visibility,
                    scene_io::rehydrate_mesh_instances,
                    scene_io::finish_mesh_instance_rehydrate,
                    tab_asset_cache::cache_added_mesh_instances,
                    tick_scene_load_progress,
                )
                    .run_if(in_state(SplashState::Loading)),
            )
            // Editor-state systems unchanged: rehydrate stays available
            // (drop creates `MeshInstanceData` post-load, so the rehydrate
            // hooks still need to fire), plus the file-action handlers.
            .add_systems(
                Update,
                (
                    scene_io::rehydrate_meshes,
                    scene_io::rehydrate_cameras,
                    scene_io::rehydrate_suns,
                    scene_io::rehydrate_lights,
                    scene_io::rehydrate_visibility,
                    scene_io::rehydrate_mesh_instances,
                    scene_io::finish_mesh_instance_rehydrate,
                    tab_asset_cache::cache_added_mesh_instances,
                    detect_file_keybindings,
                    save_scene_system,
                    save_as_scene_system,
                    new_scene_system,
                    open_scene_system,
                    handle_tab_switch,
                )
                    .run_if(in_state(SplashState::Editor)),
            );
    }
}
