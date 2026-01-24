//! Plugin Core System
//!
//! This module provides the infrastructure for loading and managing editor plugins.
//! Plugins are loaded from the project's plugins/ directory when a project is opened.

pub mod abi;
pub mod api;
pub mod dependency;
pub mod host;
pub mod registry;
pub mod traits;

pub use abi::*;
pub use api::{
    ContextMenuLocation, EditorApi, EditorApiImpl, InspectorDefinition, MenuItem, MenuLocation,
    PanelDefinition, PanelLocation, PendingOperation, StatusBarAlign, StatusBarItem, ToolbarItem,
    TabLocation, PluginTab,
};
pub use host::PluginHost;
pub use registry::PluginRegistry;
pub use traits::*;

use bevy::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;

use abi::{EntityIdExt, PluginTransformExt};
use crate::commands::CommandHistory;
use crate::core::{AppState, EditorEntity, SelectionState};
use crate::project::CurrentProject;

/// Plugin that manages the plugin host lifecycle
pub struct PluginCorePlugin;

impl Plugin for PluginCorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PluginHost>()
            .init_resource::<PluginRegistry>()
            .add_systems(
                Update,
                (
                    check_project_plugins,
                    sync_bevy_to_plugins,
                    update_plugins,
                    apply_plugin_operations,
                    dispatch_selection_events,
                )
                    .chain()
                    .run_if(in_state(AppState::Editor)),
            )
            // Exclusive system for direct World access
            .add_systems(
                Update,
                (
                    update_plugins_with_world,
                    process_plugin_undo_redo,
                )
                    .chain()
                    .run_if(in_state(AppState::Editor)),
            );
    }
}

/// Check if project changed and load/unload plugins accordingly
fn check_project_plugins(
    mut plugin_host: ResMut<PluginHost>,
    current_project: Option<Res<CurrentProject>>,
    mut last_project_path: Local<Option<PathBuf>>,
) {
    let current_path = current_project.as_ref().map(|p| p.path.clone());

    // Check if project changed
    if *last_project_path != current_path {
        // Unload existing plugins if any
        if plugin_host.plugin_count() > 0 {
            info!("Project changed, unloading plugins...");
            plugin_host.unload_all_plugins();
        }

        // Load plugins from new project if there is one
        if let Some(ref project_path) = current_path {
            let plugins_dir = project_path.join("plugins");
            plugin_host.set_plugin_dir(plugins_dir);

            if let Err(e) = plugin_host.discover_and_load_plugins() {
                error!("Failed to load project plugins: {}", e);
            }
        }

        *last_project_path = current_path;
    }

    // Check for hot reload (file changes in plugin directory)
    plugin_host.check_for_changes();
}

/// Exclusive system that gives plugins direct World access
fn update_plugins_with_world(world: &mut World) {
    // Temporarily take the PluginHost out of the world to avoid borrow issues
    let mut plugin_host = world.remove_resource::<PluginHost>().expect("PluginHost resource missing");

    // Call on_world_update for all plugins
    plugin_host.update_with_world(world);

    // Put the PluginHost back
    world.insert_resource(plugin_host);
}

/// Sync Bevy state to the plugin API before plugin update
fn sync_bevy_to_plugins(
    mut plugin_host: ResMut<PluginHost>,
    selection: Res<SelectionState>,
    command_history: Res<CommandHistory>,
    entities: Query<(Entity, &EditorEntity, &Transform, Option<&ChildOf>)>,
    children_query: Query<&Children>,
) {
    // Build state snapshots
    let selected = selection.selected_entity.map(EntityId::from_bevy);

    let mut transforms = HashMap::new();
    let mut names = HashMap::new();
    let mut visibility = HashMap::new();
    let mut parents = HashMap::new();
    let mut children_map: HashMap<EntityId, Vec<EntityId>> = HashMap::new();

    for (entity, editor_entity, transform, child_of) in entities.iter() {
        let id = EntityId::from_bevy(entity);
        transforms.insert(id, PluginTransform::from_bevy(*transform));
        names.insert(id, editor_entity.name.clone());
        visibility.insert(id, editor_entity.visible);

        // Get parent
        let parent_id = child_of.map(|c| EntityId::from_bevy(c.0));
        parents.insert(id, parent_id);

        // Get children
        if let Ok(children) = children_query.get(entity) {
            let child_ids: Vec<EntityId> = children.iter()
                .map(|c| EntityId::from_bevy(c))
                .collect();
            children_map.insert(id, child_ids);
        } else {
            children_map.insert(id, Vec::new());
        }
    }

    // Sync to plugin API
    let api = plugin_host.api_mut();
    api.sync_from_bevy(selected, transforms, names, visibility, parents, children_map);

    // Sync undo/redo state
    api.can_undo = command_history.can_undo();
    api.can_redo = command_history.can_redo();
}

/// Update all plugins (called every frame)
fn update_plugins(mut plugin_host: ResMut<PluginHost>, time: Res<Time>) {
    plugin_host.update(time.delta_secs());
}

/// Apply pending operations from plugins to Bevy world
fn apply_plugin_operations(
    mut plugin_host: ResMut<PluginHost>,
    mut selection: ResMut<SelectionState>,
    mut transforms: Query<&mut Transform>,
    mut editor_entities: Query<&mut EditorEntity>,
    mut commands: Commands,
) {
    let operations = plugin_host.api_mut().take_pending_operations();

    for op in operations {
        match op {
            PendingOperation::SetSelectedEntity(entity_id) => {
                selection.selected_entity = entity_id.and_then(|id| id.to_bevy());
            }
            PendingOperation::SetTransform { entity, transform } => {
                if let Some(bevy_entity) = entity.to_bevy() {
                    if let Ok(mut t) = transforms.get_mut(bevy_entity) {
                        *t = transform.to_bevy();
                    }
                }
            }
            PendingOperation::SetEntityName { entity, name } => {
                if let Some(bevy_entity) = entity.to_bevy() {
                    if let Ok(mut editor_entity) = editor_entities.get_mut(bevy_entity) {
                        editor_entity.name = name;
                    }
                }
            }
            PendingOperation::SetEntityVisible { entity, visible } => {
                if let Some(bevy_entity) = entity.to_bevy() {
                    if let Ok(mut editor_entity) = editor_entities.get_mut(bevy_entity) {
                        editor_entity.visible = visible;
                    }
                }
            }
            PendingOperation::SpawnEntity(def) => {
                let transform: Transform = def.transform.to_bevy();
                let mut entity_commands = commands.spawn((
                    transform,
                    EditorEntity {
                        name: def.name.clone(),
                        visible: true,
                        locked: false,
                    },
                    crate::core::SceneNode,
                ));

                if let Some(parent_id) = def.parent {
                    if let Some(parent_entity) = parent_id.to_bevy() {
                        entity_commands.insert(ChildOf(parent_entity));
                    }
                }

                info!("Plugin spawned entity: {}", def.name);
            }
            PendingOperation::DespawnEntity(entity_id) => {
                if let Some(bevy_entity) = entity_id.to_bevy() {
                    commands.entity(bevy_entity).despawn();
                    info!("Plugin despawned entity: {:?}", entity_id);
                }
            }
            PendingOperation::ReparentEntity { entity, new_parent } => {
                if let Some(bevy_entity) = entity.to_bevy() {
                    if let Some(parent_entity) = new_parent.and_then(|id| id.to_bevy()) {
                        // Reparent to new parent
                        commands.entity(bevy_entity).insert(ChildOf(parent_entity));
                        info!("Plugin reparented entity to {:?}", new_parent);
                    } else {
                        // Remove parent (make root)
                        commands.entity(bevy_entity).remove::<ChildOf>();
                        info!("Plugin moved entity to root");
                    }
                }
            }
            PendingOperation::LoadAsset(path) => {
                info!("Plugin requested asset load: {}", path);
                // TODO: Connect to asset server
            }
        }
    }
}

/// Track selection changes and dispatch events to plugins
fn dispatch_selection_events(
    mut plugin_host: ResMut<PluginHost>,
    selection: Res<SelectionState>,
    mut last_selection: Local<Option<Entity>>,
) {
    let current = selection.selected_entity;

    if current != *last_selection {
        // Selection changed - dispatch events
        if let Some(old) = *last_selection {
            plugin_host.queue_event(EditorEvent::EntityDeselected(EntityId::from_bevy(old)));
        }
        if let Some(new) = current {
            plugin_host.queue_event(EditorEvent::EntitySelected(EntityId::from_bevy(new)));
        }
        *last_selection = current;
    }
}

/// Exclusive system to process undo/redo operations from plugins
fn process_plugin_undo_redo(world: &mut World) {
    // Check for pending undo/redo requests from plugins
    let (do_undo, do_redo) = {
        let mut plugin_host = world.resource_mut::<PluginHost>();
        let api = plugin_host.api_mut();
        let undo = api.pending_undo;
        let redo = api.pending_redo;
        // Clear the flags
        api.pending_undo = false;
        api.pending_redo = false;
        (undo, redo)
    };

    // Process undo request
    if do_undo {
        if crate::commands::undo(world) {
            info!("Plugin triggered undo");
        }
    }

    // Process redo request
    if do_redo {
        if crate::commands::redo(world) {
            info!("Plugin triggered redo");
        }
    }
}
