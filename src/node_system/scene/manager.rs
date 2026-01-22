use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use rfd::FileDialog;
use std::path::PathBuf;

use crate::core::{SceneNode, SceneTabId, SceneManagerState, SelectionState, HierarchyState, OrbitCameraState, SceneTab, TabCameraState};
use crate::node_system::registry::NodeRegistry;
use crate::project::CurrentProject;

use super::loader::{load_scene, SceneLoadResult};
use super::saver::save_scene;

/// Exclusive system that handles scene save/load requests
/// Must be exclusive because save_scene requires &mut World
pub fn handle_scene_requests(world: &mut World) {
    // Check for pending requests
    let (save_requested, save_as_requested, new_scene_requested, open_scene_requested, current_path, pending_tab_switch, pending_tab_close) = {
        let scene_state = world.resource::<SceneManagerState>();
        (
            scene_state.save_scene_requested,
            scene_state.save_scene_as_requested,
            scene_state.new_scene_requested,
            scene_state.open_scene_requested,
            scene_state.current_scene_path.clone(),
            scene_state.pending_tab_switch,
            scene_state.pending_tab_close,
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
}

fn do_save_scene(world: &mut World, path: &PathBuf) {
    // Get scene name from path
    let scene_name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string();

    // Use resource_scope to safely access registry while modifying world
    world.resource_scope(|world, registry: Mut<NodeRegistry>| {
        match save_scene(path, &scene_name, world, &registry) {
            Ok(()) => {
                info!("Scene saved to: {}", path.display());
            }
            Err(e) => {
                error!("Failed to save scene: {}", e);
            }
        }
    });
}

fn do_save_scene_as(world: &mut World) {
    // Get current project path for default location
    let default_path = world
        .get_resource::<CurrentProject>()
        .map(|p| p.resolve_path("scenes"))
        .unwrap_or_else(|| PathBuf::from("."));

    // Show file dialog
    let file = FileDialog::new()
        .add_filter("Scene", &["scene"])
        .set_directory(&default_path)
        .set_file_name("scene.scene")
        .save_file();

    if let Some(path) = file {
        // Update the current scene path
        {
            let mut scene_state = world.resource_mut::<SceneManagerState>();
            scene_state.current_scene_path = Some(path.clone());
        }

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

    // Show file dialog
    let file = FileDialog::new()
        .add_filter("Scene", &["scene"])
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
                name: scene_name,
                path: Some(path.clone()),
                ..Default::default()
            });

            // Switch to the new tab
            scene_state.active_scene_tab = new_idx;
            scene_state.current_scene_path = Some(path.clone());

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

        // Store load result to apply after resource_scope
        let mut load_result: Option<SceneLoadResult> = None;

        // Load the scene using exclusive world access
        world.resource_scope(|world, registry: Mut<NodeRegistry>| {
            world.resource_scope(|world, mut meshes: Mut<Assets<Mesh>>| {
                world.resource_scope(|world, mut materials: Mut<Assets<StandardMaterial>>| {
                    let mut command_queue = CommandQueue::default();
                    let mut commands = Commands::new(&mut command_queue, world);

                    match load_scene(&path, &mut commands, &mut meshes, &mut materials, &registry) {
                        Ok(result) => {
                            info!("Scene loaded from: {}", path.display());
                            load_result = Some(result);
                        }
                        Err(e) => {
                            error!("Failed to load scene: {}", e);
                        }
                    }

                    command_queue.apply(world);
                });
            });
        });

        // Apply camera state and expanded entities from the loaded scene
        if let Some(result) = load_result {
            {
                let mut orbit = world.resource_mut::<OrbitCameraState>();
                orbit.focus = bevy::prelude::Vec3::new(
                    result.editor_camera.orbit_focus[0],
                    result.editor_camera.orbit_focus[1],
                    result.editor_camera.orbit_focus[2],
                );
                orbit.distance = result.editor_camera.orbit_distance;
                orbit.yaw = result.editor_camera.orbit_yaw;
                orbit.pitch = result.editor_camera.orbit_pitch;
            }

            // Restore expanded entities in hierarchy
            {
                let mut hierarchy = world.resource_mut::<HierarchyState>();
                for entity in result.expanded_entities {
                    hierarchy.expanded_entities.insert(entity);
                }
            }
        }
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
