use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use rfd::FileDialog;
use std::path::PathBuf;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

use crate::core::{AppState, MainCamera, SceneNode, SceneTabId, SceneManagerState, SelectionState, HierarchyState, OrbitCameraState, SceneTab, TabCameraState, DefaultCameraEntity, ViewportCamera};
use crate::component_system::{CameraNodeData, CameraRigData};
use crate::project::{CurrentProject, AppConfig, open_project};
use crate::{console_success, console_error, console_info};

use super::loader::load_scene_bevy;
use super::saver::save_scene_bevy;

/// Exclusive system that handles scene save/load requests
/// Must be exclusive because save_scene requires &mut World
pub fn handle_scene_requests(world: &mut World) {
    // Check for pending requests
    let (save_requested, save_as_requested, new_scene_requested, open_scene_requested, current_path, pending_tab_switch, pending_tab_close, new_project_requested, open_project_requested) = {
        let scene_state = world.resource::<SceneManagerState>();
        (
            scene_state.save_scene_requested,
            scene_state.save_scene_as_requested,
            scene_state.new_scene_requested,
            scene_state.open_scene_requested,
            scene_state.current_scene_path.clone(),
            scene_state.pending_tab_switch,
            scene_state.pending_tab_close,
            scene_state.new_project_requested,
            scene_state.open_project_requested,
        )
    };

    // Clear the request flags immediately
    {
        let mut scene_state = world.resource_mut::<SceneManagerState>();
        scene_state.save_scene_requested = false;
        scene_state.save_scene_as_requested = false;
        scene_state.new_scene_requested = false;
        scene_state.open_scene_requested = false;
        scene_state.pending_tab_switch = None;
        scene_state.pending_tab_close = None;
        scene_state.new_project_requested = false;
        scene_state.open_project_requested = false;
    }

    // Handle tab closing first (before switching)
    if let Some(tab_idx) = pending_tab_close {
        do_close_tab(world, tab_idx);
    }

    // Handle tab switching (before other operations)
    if let Some(new_tab_idx) = pending_tab_switch {
        do_tab_switch(world, new_tab_idx);
    }

    // Handle save requests
    if save_requested {
        if let Some(path) = current_path.clone() {
            do_save_scene(world, &path);
        } else {
            // No current path, do Save As instead
            do_save_scene_as(world);
        }
    }

    if save_as_requested {
        do_save_scene_as(world);
    }

    if new_scene_requested {
        do_new_scene(world);
    }

    if open_scene_requested {
        do_open_scene(world);
    }

    // Handle project-level requests
    if new_project_requested {
        do_new_project(world);
    }

    if open_project_requested {
        do_open_project(world);
    }
}

fn do_save_scene(world: &mut World, path: &PathBuf) {
    // Get scene name from path
    let scene_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string();

    // Use Bevy DynamicScene format with panic protection
    let save_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        save_scene_bevy(path, world)
    }));

    let save_success = match save_result {
        Ok(Ok(())) => {
            info!("Scene saved to: {}", path.display());
            console_success!("Scene", "Saved: {}", scene_name);
            true
        }
        Ok(Err(e)) => {
            error!("Failed to save scene: {}", e);
            console_error!("Scene", "Failed to save: {}", e);
            false
        }
        Err(panic_info) => {
            let panic_msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic during save".to_string()
            };
            error!("PANIC during scene save: {}", panic_msg);
            console_error!("Scene", "Save crashed: {}", panic_msg);
            false
        }
    };

    // Update tab name and state after successful save
    if save_success {
        let mut scene_state = world.resource_mut::<SceneManagerState>();
        let current_tab = scene_state.active_scene_tab;
        if let Some(tab) = scene_state.scene_tabs.get_mut(current_tab) {
            tab.name = scene_name;
            tab.path = Some(path.clone());
            tab.is_modified = false;
        }
        scene_state.current_scene_path = Some(path.clone());
        // Track this save so scene instances referencing this file can reload
        scene_state.recently_saved_scenes.push(path.clone());
    }
}

fn do_save_scene_as(world: &mut World) {
    // Get current project path for default location
    let default_path = world
        .get_resource::<CurrentProject>()
        .map(|p| p.resolve_path("scenes"))
        .unwrap_or_else(|| PathBuf::from("."));

    // Show file dialog for Bevy scene format (.ron)
    let file = FileDialog::new()
        .add_filter("Bevy Scene", &["ron"])
        .set_directory(&default_path)
        .set_file_name("scene.ron")
        .save_file();

    if let Some(path) = file {
        // do_save_scene will handle updating the tab name, path, and is_modified
        do_save_scene(world, &path);
    }
}

fn do_new_scene(world: &mut World) {
    // Get current tab index
    let current_tab = {
        let scene_state = world.resource::<SceneManagerState>();
        scene_state.active_scene_tab
    };

    // Despawn only entities for the current tab
    clear_tab_entities(world, current_tab);

    // Clear current scene path and update tab
    {
        let mut scene_state = world.resource_mut::<SceneManagerState>();
        scene_state.current_scene_path = None;

        // Update the tab to show it's a new unsaved scene
        if let Some(tab) = scene_state.scene_tabs.get_mut(current_tab) {
            tab.name = "Untitled".to_string();
            tab.path = None;
        }
    }

    // Clear selection
    {
        let mut selection = world.resource_mut::<SelectionState>();
        selection.selected_entity = None;
    }

    info!("New scene created");
    console_info!("Scene", "New scene created");
}

/// Clear only entities belonging to a specific tab
fn clear_tab_entities(world: &mut World, tab_idx: usize) {
    let scene_entities: Vec<Entity> = world
        .query_filtered::<(Entity, &SceneTabId), With<SceneNode>>()
        .iter(world)
        .filter(|(_, tab_id)| tab_id.0 == tab_idx)
        .map(|(e, _)| e)
        .collect();

    for entity in scene_entities {
        world.despawn(entity);
    }
}

fn do_tab_switch(world: &mut World, new_tab_idx: usize) {
    // Get current tab index
    let current_tab_idx = {
        let scene_state = world.resource::<SceneManagerState>();
        scene_state.active_scene_tab
    };

    // IMPORTANT: First assign SceneTabId to any existing entities that don't have one
    // They belong to the current tab before we switch
    {
        let mut query = world.query_filtered::<Entity, (With<SceneNode>, Without<SceneTabId>)>();
        let entities_without_tab: Vec<Entity> = query.iter(world).collect();
        for entity in entities_without_tab {
            if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
                entity_mut.insert(SceneTabId(current_tab_idx));
            }
        }
    }

    // Save current camera state to the current tab
    let camera_state = {
        let orbit = world.resource::<OrbitCameraState>();
        TabCameraState {
            orbit_focus: orbit.focus,
            orbit_distance: orbit.distance,
            orbit_yaw: orbit.yaw,
            orbit_pitch: orbit.pitch,
            projection_mode: orbit.projection_mode,
        }
    };

    {
        let mut scene_state = world.resource_mut::<SceneManagerState>();
        if let Some(tab) = scene_state.scene_tabs.get_mut(current_tab_idx) {
            tab.camera_state = Some(camera_state);
        }
    }

    // Hide entities from current tab (all should have SceneTabId now)
    let mut query = world.query_filtered::<(Entity, &SceneTabId), With<SceneNode>>();
    let entities_to_hide: Vec<Entity> = query
        .iter(world)
        .filter(|(_, tab_id)| tab_id.0 == current_tab_idx)
        .map(|(e, _)| e)
        .collect();

    for entity in entities_to_hide {
        if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
            entity_mut.insert(Visibility::Hidden);
        }
    }

    // Show entities from new tab (only those with matching SceneTabId)
    let mut show_query = world.query_filtered::<(Entity, &SceneTabId), With<SceneNode>>();
    let entities_to_show: Vec<Entity> = show_query
        .iter(world)
        .filter(|(_, tab_id)| tab_id.0 == new_tab_idx)
        .map(|(e, _)| e)
        .collect();

    for entity in entities_to_show {
        if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
            entity_mut.insert(Visibility::Inherited);
        }
    }

    // Get new tab's camera state and path
    let (new_camera_state, new_path) = {
        let scene_state = world.resource::<SceneManagerState>();
        if let Some(tab) = scene_state.scene_tabs.get(new_tab_idx) {
            (tab.camera_state.clone(), tab.path.clone())
        } else {
            (None, None)
        }
    };

    // Apply camera state if available
    if let Some(cam) = new_camera_state {
        let mut orbit = world.resource_mut::<OrbitCameraState>();
        orbit.focus = cam.orbit_focus;
        orbit.distance = cam.orbit_distance;
        orbit.yaw = cam.orbit_yaw;
        orbit.pitch = cam.orbit_pitch;
    }

    // Update active tab and current scene path
    {
        let mut scene_state = world.resource_mut::<SceneManagerState>();
        scene_state.active_scene_tab = new_tab_idx;
        scene_state.current_scene_path = new_path;
        // Update the unified active document
        scene_state.active_document = Some(crate::core::TabKind::Scene(new_tab_idx));
    }

    // Clear selection
    {
        let mut selection = world.resource_mut::<SelectionState>();
        selection.selected_entity = None;
    }

    info!("Switched to tab {}", new_tab_idx);
}

fn do_close_tab(world: &mut World, tab_idx: usize) {
    // Get current state
    let (num_tabs, active_tab) = {
        let scene_state = world.resource::<SceneManagerState>();
        (scene_state.scene_tabs.len(), scene_state.active_scene_tab)
    };

    // Don't close if it's the last tab
    if num_tabs <= 1 {
        return;
    }

    // First, despawn all entities belonging to this tab
    {
        let mut query = world.query_filtered::<(Entity, &SceneTabId), With<SceneNode>>();
        let entities_to_despawn: Vec<Entity> = query
            .iter(world)
            .filter(|(_, scene_tab_id)| scene_tab_id.0 == tab_idx)
            .map(|(e, _)| e)
            .collect();

        for entity in entities_to_despawn {
            world.despawn(entity);
        }
    }

    // Update SceneTabId for entities in higher-indexed tabs (decrement their tab ID)
    {
        let mut query = world.query_filtered::<(Entity, &SceneTabId), With<SceneNode>>();
        let entities_to_update: Vec<(Entity, usize)> = query
            .iter(world)
            .filter(|(_, scene_tab_id)| scene_tab_id.0 > tab_idx)
            .map(|(e, tab_id)| (e, tab_id.0))
            .collect();

        for (entity, old_tab_id) in entities_to_update {
            if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
                entity_mut.insert(SceneTabId(old_tab_id - 1));
            }
        }
    }

    // Remove the tab and update active_scene_tab
    {
        let mut scene_state = world.resource_mut::<SceneManagerState>();
        scene_state.scene_tabs.remove(tab_idx);

        // Adjust active tab index
        if active_tab == tab_idx {
            // Was on the closed tab, switch to adjacent
            scene_state.active_scene_tab = if tab_idx > 0 { tab_idx - 1 } else { 0 };
        } else if active_tab > tab_idx {
            // Active tab was after closed tab, decrement index
            scene_state.active_scene_tab = active_tab - 1;
        }
        // else: active tab was before closed tab, no change needed

        // Update current_scene_path to match the new active tab
        let new_active = scene_state.active_scene_tab;
        scene_state.current_scene_path = scene_state
            .scene_tabs
            .get(new_active)
            .and_then(|t| t.path.clone());

        // Update the unified active document
        scene_state.active_document = Some(crate::core::TabKind::Scene(new_active));
    }

    // Show entities from the new active tab
    let new_active_tab = {
        let scene_state = world.resource::<SceneManagerState>();
        scene_state.active_scene_tab
    };

    {
        let mut query = world.query_filtered::<(Entity, &SceneTabId), With<SceneNode>>();
        let entities_to_show: Vec<Entity> = query
            .iter(world)
            .filter(|(_, scene_tab_id)| scene_tab_id.0 == new_active_tab)
            .map(|(e, _)| e)
            .collect();

        for entity in entities_to_show {
            if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
                entity_mut.insert(Visibility::Inherited);
            }
        }
    }

    info!("Closed tab {}", tab_idx);
}

fn do_open_scene(world: &mut World) {
    // Get current project path for default location
    let default_path = world
        .get_resource::<CurrentProject>()
        .map(|p| p.resolve_path("scenes"))
        .unwrap_or_else(|| PathBuf::from("."));

    // Show file dialog for Bevy scene format
    let file = FileDialog::new()
        .add_filter("Bevy Scene", &["ron"])
        .set_directory(&default_path)
        .pick_file();

    if let Some(path) = file {
        // Get scene name from path
        let scene_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string();

        // IMPORTANT: First assign SceneTabId to any existing entities that don't have one
        // This must happen BEFORE we change active_scene_tab, so old entities get the old tab ID
        let current_tab_idx = {
            let scene_state = world.resource::<SceneManagerState>();
            scene_state.active_scene_tab
        };

        // Assign SceneTabId to entities without one (they belong to the current tab)
        {
            let mut query = world.query_filtered::<Entity, (With<SceneNode>, Without<SceneTabId>)>();
            let entities_without_tab: Vec<Entity> = query.iter(world).collect();
            for entity in entities_without_tab {
                if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
                    entity_mut.insert(SceneTabId(current_tab_idx));
                }
            }
        }

        // Save current tab's camera state before switching
        let camera_state = {
            let orbit = world.resource::<OrbitCameraState>();
            TabCameraState {
                orbit_focus: orbit.focus,
                orbit_distance: orbit.distance,
                orbit_yaw: orbit.yaw,
                orbit_pitch: orbit.pitch,
                projection_mode: orbit.projection_mode,
            }
        };

        // Create a new tab for this scene and switch to it
        let new_tab_idx = {
            let mut scene_state = world.resource_mut::<SceneManagerState>();

            if let Some(tab) = scene_state.scene_tabs.get_mut(current_tab_idx) {
                tab.camera_state = Some(camera_state);
            }

            let new_idx = scene_state.scene_tabs.len();

            // Create and add new tab
            scene_state.scene_tabs.push(SceneTab {
                name: scene_name.clone(),
                path: Some(path.clone()),
                ..Default::default()
            });

            // Add to unified tab order
            scene_state.tab_order.push(crate::core::TabKind::Scene(new_idx));

            // Switch to the new tab
            scene_state.active_scene_tab = new_idx;
            scene_state.current_scene_path = Some(path.clone());
            // Update the unified active document
            scene_state.active_document = Some(crate::core::TabKind::Scene(new_idx));

            new_idx
        };

        // Hide all scene entities from other tabs (all should have SceneTabId now)
        {
            let mut query = world.query_filtered::<(Entity, &SceneTabId), With<SceneNode>>();
            let entities_to_hide: Vec<Entity> = query
                .iter(world)
                .filter(|(_, tab_id)| tab_id.0 != new_tab_idx)
                .map(|(e, _)| e)
                .collect();

            for entity in entities_to_hide {
                if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
                    entity_mut.insert(Visibility::Hidden);
                }
            }
        }

        // Use Bevy DynamicScene format (async loading)
        world.resource_scope(|world, asset_server: Mut<AssetServer>| {
            let mut command_queue = CommandQueue::default();
            let mut commands = Commands::new(&mut command_queue, world);

            let _result = load_scene_bevy(&mut commands, &asset_server, &path, new_tab_idx);

            command_queue.apply(world);
        });

        // Editor metadata (camera state, expanded entities) is embedded in the scene
        // and will be applied automatically by on_bevy_scene_ready when the scene loads

        info!("Loading scene: {}", path.display());
        console_success!("Scene", "Loading: {}", scene_name);
    }
}

/// System to automatically assign SceneTabId to new scene entities
pub fn assign_scene_tab_ids(
    mut commands: Commands,
    scene_state: Res<SceneManagerState>,
    new_entities: Query<Entity, (With<SceneNode>, Without<SceneTabId>)>,
) {
    let current_tab = scene_state.active_scene_tab;
    for entity in new_entities.iter() {
        commands.entity(entity).insert(SceneTabId(current_tab));
    }
}

/// System to handle Ctrl+S keyboard shortcut for saving
pub fn handle_save_shortcut(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut scene_state: ResMut<SceneManagerState>,
) {
    let ctrl_pressed = keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight);
    let shift_pressed = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

    if ctrl_pressed && keyboard.just_pressed(KeyCode::KeyS) {
        if shift_pressed {
            // Ctrl+Shift+S = Save As
            scene_state.save_scene_as_requested = true;
        } else {
            // Ctrl+S = Save
            scene_state.save_scene_requested = true;
        }
    }

    // Ctrl+N = New Scene
    if ctrl_pressed && keyboard.just_pressed(KeyCode::KeyN) {
        scene_state.new_scene_requested = true;
    }

    // Ctrl+O = Open Scene
    if ctrl_pressed && keyboard.just_pressed(KeyCode::KeyO) {
        scene_state.open_scene_requested = true;
    }
}

/// System to handle making a camera the default game camera
pub fn handle_make_default_camera(
    mut hierarchy: ResMut<HierarchyState>,
    mut default_camera: ResMut<DefaultCameraEntity>,
    mut cameras: Query<(Entity, &mut CameraNodeData)>,
    mut camera_rigs: Query<(Entity, &mut CameraRigData), Without<CameraNodeData>>,
) {
    if let Some(target_entity) = hierarchy.pending_make_default_camera.take() {
        // Clear is_default_camera on all cameras and rigs
        for (_, mut cam_data) in cameras.iter_mut() {
            cam_data.is_default_camera = false;
        }
        for (_, mut rig_data) in camera_rigs.iter_mut() {
            rig_data.is_default_camera = false;
        }

        // Set is_default_camera on the target (could be camera or rig)
        if let Ok((_, mut cam_data)) = cameras.get_mut(target_entity) {
            cam_data.is_default_camera = true;
            default_camera.entity = Some(target_entity);
            info!("Set camera {:?} as default game camera", target_entity);
            console_success!("Camera", "Set as default game camera");
        } else if let Ok((_, mut rig_data)) = camera_rigs.get_mut(target_entity) {
            rig_data.is_default_camera = true;
            default_camera.entity = Some(target_entity);
            info!("Set camera rig {:?} as default game camera", target_entity);
            console_success!("Camera Rig", "Set as default game camera");
        }
    }

    // Check if any camera or rig is set as default
    let has_default = cameras.iter().any(|(_, data)| data.is_default_camera)
        || camera_rigs.iter().any(|(_, data)| data.is_default_camera);

    // Auto-assign first camera/rig as default if no default exists
    if !has_default {
        // Prefer regular cameras first
        if let Some((entity, mut cam_data)) = cameras.iter_mut().next() {
            cam_data.is_default_camera = true;
            default_camera.entity = Some(entity);
            info!("Auto-assigned camera {:?} as default game camera", entity);
        } else if let Some((entity, mut rig_data)) = camera_rigs.iter_mut().next() {
            rig_data.is_default_camera = true;
            default_camera.entity = Some(entity);
            info!("Auto-assigned camera rig {:?} as default game camera", entity);
        }
    } else {
        // Update the resource to match the actual default
        for (entity, data) in cameras.iter() {
            if data.is_default_camera {
                default_camera.entity = Some(entity);
                return;
            }
        }
        for (entity, data) in camera_rigs.iter() {
            if data.is_default_camera {
                default_camera.entity = Some(entity);
                return;
            }
        }
    }
}

/// System to snap a camera entity's transform to the current editor viewport camera position
pub fn handle_snap_camera_to_viewport(
    mut hierarchy: ResMut<HierarchyState>,
    orbit_camera: Res<OrbitCameraState>,
    mut transforms: Query<&mut Transform>,
) {
    if let Some(target_entity) = hierarchy.pending_snap_to_viewport.take() {
        let viewport_transform = orbit_camera.calculate_transform();
        if let Ok(mut transform) = transforms.get_mut(target_entity) {
            *transform = viewport_transform;
            info!("Snapped camera {:?} to viewport position", target_entity);
            console_success!("Camera", "Snapped to viewport position");
        }
    }
}

/// Check if any scene tab has unsaved changes
fn has_unsaved_changes(world: &World) -> bool {
    let scene_state = world.resource::<SceneManagerState>();
    scene_state.scene_tabs.iter().any(|tab| tab.is_modified)
}

/// Prompt the user to save unsaved changes before a project switch.
/// Returns true if the operation should proceed, false if cancelled.
fn prompt_save_before_project_switch(world: &mut World) -> bool {
    if !has_unsaved_changes(world) {
        return true;
    }

    let result = rfd::MessageDialog::new()
        .set_title("Unsaved Changes")
        .set_description("You have unsaved changes. Would you like to save before continuing?")
        .set_buttons(rfd::MessageButtons::YesNoCancel)
        .show();

    match result {
        rfd::MessageDialogResult::Yes => {
            // Save all modified tabs
            let tabs_to_save: Vec<(usize, Option<PathBuf>)> = {
                let scene_state = world.resource::<SceneManagerState>();
                scene_state.scene_tabs.iter().enumerate()
                    .filter(|(_, tab)| tab.is_modified)
                    .map(|(i, tab)| (i, tab.path.clone()))
                    .collect()
            };

            for (_idx, path) in tabs_to_save {
                if let Some(path) = path {
                    do_save_scene(world, &path);
                } else {
                    // No path - need Save As
                    do_save_scene_as(world);
                }
            }
            true
        }
        rfd::MessageDialogResult::No => {
            // Discard changes, proceed
            true
        }
        rfd::MessageDialogResult::Cancel | _ => {
            // User cancelled
            false
        }
    }
}

/// Clean up all editor entities to prepare for project switch
fn cleanup_editor_for_project_switch(world: &mut World) {
    // Despawn all scene entities
    let scene_entities: Vec<Entity> = world
        .query_filtered::<Entity, With<SceneNode>>()
        .iter(world)
        .collect();

    for entity in scene_entities {
        world.despawn(entity);
    }

    // Despawn editor cameras (MainCamera, ViewportCamera)
    let camera_entities: Vec<Entity> = world
        .query_filtered::<Entity, Or<(With<MainCamera>, With<ViewportCamera>)>>()
        .iter(world)
        .collect();

    for entity in camera_entities {
        world.despawn(entity);
    }

    // Reset SceneManagerState to defaults
    let default_state = SceneManagerState::default();
    *world.resource_mut::<SceneManagerState>() = default_state;

    // Clear selection
    world.resource_mut::<SelectionState>().selected_entity = None;
}

/// Handle "New Project" - save prompt, cleanup, return to splash
fn do_new_project(world: &mut World) {
    if !prompt_save_before_project_switch(world) {
        return;
    }

    cleanup_editor_for_project_switch(world);

    // Remove CurrentProject resource
    world.remove_resource::<CurrentProject>();

    // Transition to splash screen
    world.resource_mut::<NextState<AppState>>().set(AppState::Splash);

    info!("Returning to splash screen");
    console_info!("Project", "Closed project");
}

/// Handle "Open Project" - save prompt, file dialog, validate, switch
fn do_open_project(world: &mut World) {
    if !prompt_save_before_project_switch(world) {
        return;
    }

    // Show file dialog for project.toml
    let file = FileDialog::new()
        .set_title("Open Project")
        .add_filter("Project File", &["toml"])
        .pick_file();

    let Some(file) = file else {
        return;
    };

    // Validate the project
    let project = match open_project(&file) {
        Ok(project) => project,
        Err(e) => {
            console_error!("Project", "Failed to open project: {}", e);
            rfd::MessageDialog::new()
                .set_title("Invalid Project")
                .set_description(&format!("Failed to open project: {}", e))
                .set_buttons(rfd::MessageButtons::Ok)
                .show();
            return;
        }
    };

    // Add to recent projects
    if let Some(mut app_config) = world.get_resource_mut::<AppConfig>() {
        app_config.add_recent_project(project.path.clone());
        let _ = app_config.save();
    }

    cleanup_editor_for_project_switch(world);

    // Insert new project
    world.insert_resource(project);

    // Transition: go to Splash briefly then back to Editor so OnEnter(Editor) re-runs
    world.resource_mut::<NextState<AppState>>().set(AppState::Splash);
    // We set a flag so splash knows to immediately transition to editor
    world.insert_resource(PendingProjectReopen);

    info!("Opening new project");
    console_info!("Project", "Opening project...");
}

/// Marker resource indicating that the splash screen should immediately transition to Editor
/// because a project was opened via the File menu (not from the splash screen UI)
#[derive(Resource)]
pub struct PendingProjectReopen;

/// System that handles the "Export Project..." request from the File menu.
/// Applies export dialog settings, saves them to project.toml, then copies
/// the executable, project files, and icon to a user-chosen folder.
pub fn handle_export_request(
    mut scene_state: ResMut<SceneManagerState>,
    mut current_project: Option<ResMut<CurrentProject>>,
) {
    if !scene_state.export_project_requested {
        return;
    }
    scene_state.export_project_requested = false;

    let Some(ref mut project) = current_project else {
        console_error!("Export", "No project is open");
        return;
    };

    // Apply dialog settings to project config
    let dialog = &scene_state.export_dialog;
    project.config.window.fullscreen = dialog.fullscreen;
    project.config.window.width = dialog.width;
    project.config.window.height = dialog.height;
    project.config.window.resizable = dialog.resizable;
    project.config.icon = if dialog.icon_path.is_empty() {
        None
    } else {
        Some(dialog.icon_path.clone())
    };

    // Save updated project.toml
    let config_path = project.path.join("project.toml");
    match toml::to_string_pretty(&project.config) {
        Ok(content) => {
            if let Err(e) = std::fs::write(&config_path, &content) {
                console_error!("Export", "Failed to save project.toml: {}", e);
            }
        }
        Err(e) => {
            console_error!("Export", "Failed to serialize config: {}", e);
        }
    }

    let binary_name = if dialog.binary_name.is_empty() {
        project.config.name.clone()
    } else {
        dialog.binary_name.clone()
    };
    let icon_path = dialog.icon_path.clone();

    // Pick destination folder
    let Some(export_dir) = FileDialog::new()
        .set_title("Export Project")
        .pick_folder()
    else {
        // Reset dialog state since we're cancelling
        scene_state.export_dialog = Default::default();
        return;
    };

    match export_project(project, &export_dir, &binary_name, &icon_path) {
        Ok(()) => {
            console_success!("Export", "Project exported to {}", export_dir.display());
            info!("Project exported to {}", export_dir.display());
        }
        Err(e) => {
            console_error!("Export", "Failed to export: {}", e);
            error!("Failed to export project: {}", e);
        }
    }

    // Reset dialog state
    scene_state.export_dialog = Default::default();
}

/// Copy the executable and project files to the export directory.
fn export_project(
    project: &CurrentProject,
    export_dir: &std::path::Path,
    binary_name: &str,
    icon_path: &str,
) -> Result<(), String> {
    // Copy the running executable with the chosen binary name
    let exe_path = std::env::current_exe().map_err(|e| format!("Cannot locate executable: {}", e))?;
    let dest_exe = export_dir.join(format!("{}.exe", binary_name));
    std::fs::copy(&exe_path, &dest_exe).map_err(|e| format!("Failed to copy executable: {}", e))?;

    // Embed icon into the exported binary if set
    if !icon_path.is_empty() {
        let icon_src = project.path.join(icon_path);
        if icon_src.exists() {
            if let Err(e) = embed_icon_in_exe(&dest_exe, &icon_src) {
                warn!("Failed to embed icon in executable: {}", e);
            }
        }
    }

    // Copy project.toml
    let src_toml = project.path.join("project.toml");
    if src_toml.exists() {
        std::fs::copy(&src_toml, export_dir.join("project.toml"))
            .map_err(|e| format!("Failed to copy project.toml: {}", e))?;
    }

    // Copy project directories
    for dir_name in &["assets", "scenes", "scripts", "blueprints"] {
        let src = project.path.join(dir_name);
        if src.exists() {
            copy_dir_recursive(&src, &export_dir.join(dir_name))?;
        }
    }

    Ok(())
}

/// Embed an ICO or PNG icon into a Windows PE executable using the UpdateResource API.
#[cfg(windows)]
fn embed_icon_in_exe(exe_path: &std::path::Path, icon_path: &std::path::Path) -> Result<(), String> {
    use std::io::{Cursor, Read};

    let icon_data = std::fs::read(icon_path)
        .map_err(|e| format!("Failed to read icon file: {}", e))?;

    // If it's a PNG, convert to ICO format first
    let ico_data = if icon_path.extension().and_then(|e| e.to_str()) == Some("png") {
        png_to_ico(&icon_data)?
    } else {
        icon_data
    };

    // Parse ICO header
    let mut cursor = Cursor::new(&ico_data);
    let mut header = [0u8; 6];
    std::io::Read::read_exact(&mut cursor, &mut header)
        .map_err(|e| format!("Invalid ICO header: {}", e))?;

    let image_count = u16::from_le_bytes([header[4], header[5]]) as usize;
    if image_count == 0 {
        return Err("ICO file contains no images".into());
    }

    // Parse directory entries (16 bytes each)
    let mut entries = Vec::with_capacity(image_count);
    for _ in 0..image_count {
        let mut entry = [0u8; 16];
        std::io::Read::read_exact(&mut cursor, &mut entry)
            .map_err(|e| format!("Invalid ICO entry: {}", e))?;
        entries.push(entry);
    }

    // Open the exe for resource updates
    let exe_wide: Vec<u16> = exe_path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let handle = unsafe {
        windows_sys::Win32::System::LibraryLoader::BeginUpdateResourceW(
            exe_wide.as_ptr(),
            0, // don't delete existing resources
        )
    };
    if handle.is_null() {
        return Err("BeginUpdateResource failed".into());
    }

    let rt_icon = 3u16;        // RT_ICON
    let rt_group_icon = 14u16;  // RT_GROUP_ICON

    // Write each icon image as an RT_ICON resource
    for (i, entry) in entries.iter().enumerate() {
        let data_size = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]) as usize;
        let data_offset = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]) as usize;

        if data_offset + data_size > ico_data.len() {
            unsafe { windows_sys::Win32::System::LibraryLoader::EndUpdateResourceW(handle, 1); }
            return Err("ICO data offset out of bounds".into());
        }

        let image_data = &ico_data[data_offset..data_offset + data_size];
        let resource_id = (i + 1) as u16;

        let ok = unsafe {
            windows_sys::Win32::System::LibraryLoader::UpdateResourceW(
                handle,
                rt_icon as *const u16,
                resource_id as *const u16,
                0x0409, // LANG_ENGLISH
                image_data.as_ptr() as *const _,
                image_data.len() as u32,
            )
        };
        if ok == 0 {
            unsafe { windows_sys::Win32::System::LibraryLoader::EndUpdateResourceW(handle, 1); }
            return Err(format!("UpdateResource failed for icon {}", i));
        }
    }

    // Build RT_GROUP_ICON data: GRPICONDIR header + GRPICONDIRENTRY per image
    // GRPICONDIR: reserved(2) + type(2) + count(2) = 6 bytes
    // GRPICONDIRENTRY: width(1) + height(1) + colors(1) + reserved(1) + planes(2) + bpp(2) + size(4) + id(2) = 14 bytes
    let grp_size = 6 + image_count * 14;
    let mut grp_data = vec![0u8; grp_size];
    // Header
    grp_data[0..2].copy_from_slice(&0u16.to_le_bytes()); // reserved
    grp_data[2..4].copy_from_slice(&1u16.to_le_bytes()); // type = icon
    grp_data[4..6].copy_from_slice(&(image_count as u16).to_le_bytes());

    for (i, entry) in entries.iter().enumerate() {
        let offset = 6 + i * 14;
        // Copy first 12 bytes from ICO entry (width, height, colors, reserved, planes, bpp, size)
        grp_data[offset..offset + 12].copy_from_slice(&entry[0..12]);
        // Replace the file offset (4 bytes) with the resource ID (2 bytes)
        let resource_id = (i + 1) as u16;
        grp_data[offset + 12..offset + 14].copy_from_slice(&resource_id.to_le_bytes());
    }

    let ok = unsafe {
        windows_sys::Win32::System::LibraryLoader::UpdateResourceW(
            handle,
            rt_group_icon as *const u16,
            1 as *const u16, // group ID 1 (main icon)
            0x0409,
            grp_data.as_ptr() as *const _,
            grp_data.len() as u32,
        )
    };
    if ok == 0 {
        unsafe { windows_sys::Win32::System::LibraryLoader::EndUpdateResourceW(handle, 1); }
        return Err("UpdateResource failed for group icon".into());
    }

    // Commit
    let ok = unsafe {
        windows_sys::Win32::System::LibraryLoader::EndUpdateResourceW(handle, 0)
    };
    if ok == 0 {
        return Err("EndUpdateResource failed".into());
    }

    Ok(())
}

/// Convert a PNG image to ICO format (single-image ICO).
#[cfg(windows)]
fn png_to_ico(png_data: &[u8]) -> Result<Vec<u8>, String> {
    use image::ImageReader;
    use std::io::Cursor;

    let reader = ImageReader::new(Cursor::new(png_data))
        .with_guessed_format()
        .map_err(|e| format!("Failed to read PNG: {}", e))?;
    let img = reader.decode()
        .map_err(|e| format!("Failed to decode PNG: {}", e))?;
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());

    // Re-encode as PNG for the ICO container (modern ICO supports embedded PNG)
    let mut png_buf = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(Cursor::new(&mut png_buf));
    image::ImageEncoder::write_image(
        encoder,
        rgba.as_raw(),
        w,
        h,
        image::ExtendedColorType::Rgba8,
    ).map_err(|e| format!("Failed to encode PNG for ICO: {}", e))?;

    // Build ICO: header(6) + entry(16) + png data
    let mut ico = Vec::with_capacity(6 + 16 + png_buf.len());
    // ICONDIR header
    ico.extend_from_slice(&0u16.to_le_bytes()); // reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // type = icon
    ico.extend_from_slice(&1u16.to_le_bytes()); // count = 1
    // ICONDIRENTRY
    ico.push(if w >= 256 { 0 } else { w as u8 }); // width (0 = 256)
    ico.push(if h >= 256 { 0 } else { h as u8 }); // height
    ico.push(0); // color count
    ico.push(0); // reserved
    ico.extend_from_slice(&1u16.to_le_bytes()); // planes
    ico.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
    ico.extend_from_slice(&(png_buf.len() as u32).to_le_bytes()); // size
    ico.extend_from_slice(&22u32.to_le_bytes()); // offset (6 + 16 = 22)
    // Image data
    ico.extend_from_slice(&png_buf);

    Ok(ico)
}

#[cfg(not(windows))]
fn embed_icon_in_exe(_exe_path: &std::path::Path, _icon_path: &std::path::Path) -> Result<(), String> {
    Ok(()) // No-op on non-Windows
}

/// Recursively copy a directory and all its contents.
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("Failed to create {}: {}", dst.display(), e))?;

    let entries = std::fs::read_dir(src)
        .map_err(|e| format!("Failed to read {}: {}", src.display(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Directory entry error: {}", e))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            // Skip editor-only and version control directories
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str == ".editor" || name_str == ".git" || name_str == ".svn" {
                continue;
            }
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)
                .map_err(|e| format!("Failed to copy {}: {}", src_path.display(), e))?;
        }
    }

    Ok(())
}

/// System to automatically save the scene when it's modified.
/// Saves periodically (based on auto_save_interval) if the scene has a path and is modified.
pub fn auto_save_scene(
    time: Res<Time>,
    mut scene_state: ResMut<SceneManagerState>,
) {
    if !scene_state.auto_save_enabled {
        return;
    }

    // Update timer
    scene_state.auto_save_timer += time.delta_secs();

    // Check if it's time to auto-save
    if scene_state.auto_save_timer >= scene_state.auto_save_interval {
        scene_state.auto_save_timer = 0.0;

        // Check if current scene is modified and has a path
        let (is_modified, has_path, scene_name) = if let Some(tab) = scene_state.active_tab() {
            (tab.is_modified, tab.path.is_some(), tab.name.clone())
        } else {
            (false, false, "None".to_string())
        };

        info!("Auto-save check: scene={}, is_modified={}, has_path={}", scene_name, is_modified, has_path);

        if is_modified && has_path {
            // Request a save
            scene_state.save_scene_requested = true;
            info!("Auto-saving scene...");
            console_info!("Scene", "Auto-saving...");
        } else if !has_path && is_modified {
            // Scene is modified but not yet saved - inform user
            info!("Auto-save skipped: scene has unsaved changes but no file path (save manually first)");
        }
    }
}
