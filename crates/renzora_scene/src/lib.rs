//! Renzora Scene — editor-side scene plugin that wires save/load to keybindings and splash state.
//!
//! The actual save/load and rehydration logic lives in `renzora_engine::scene_io`.

use bevy::prelude::*;

use renzora::core::{CurrentProject, SaveSceneRequested, SaveAsSceneRequested, NewSceneRequested, OpenSceneRequested, ToggleSettingsRequested, HideInHierarchy, EditorCamera, SceneCamera, TabSwitchRequest, TabSceneSnapshot, SceneTabBuffers};
use renzora_camera::OrbitCameraState;
use renzora_keybindings::{EditorAction, KeyBindings};
use renzora_engine::scene_io;
use renzora_editor::SplashState;

// Re-export so downstream code that was using `renzora_scene::{save_scene, load_scene, ...}` still works.
pub use scene_io::{save_scene, load_scene, save_current_scene, load_current_scene};

mod panel;
pub use panel::ScenesPanel;

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

fn load_scene_on_enter(world: &mut World) {
    info!("load_scene_on_enter triggered");

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

    // Ask the hierarchy to auto-select its top entity once the cache is
    // populated. The flag is consumed by `auto_select_first_hierarchy_entity`
    // in `renzora_hierarchy` — we can't do it here because entities have
    // only just been queued for spawn and the hierarchy tree won't be
    // built until the next frame.
    if let Some(mut flag) = world.get_resource_mut::<renzora_editor::AutoSelectFirstHierarchyEntity>() {
        flag.0 = true;
    }
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
            .add_systems(OnEnter(SplashState::Editor), load_scene_on_enter)
            .add_systems(
                Update,
                (
                    scene_io::rehydrate_meshes,
                    scene_io::rehydrate_cameras,
                    scene_io::rehydrate_suns,
                    scene_io::rehydrate_visibility,
                    scene_io::rehydrate_mesh_instances,
                    scene_io::finish_mesh_instance_rehydrate,
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
