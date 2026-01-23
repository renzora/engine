//! Plugin Core System
//!
//! This module provides the infrastructure for loading and managing editor plugins.
//! Plugins are loaded from DLLs at runtime and can extend the editor with new
//! functionality like custom panels, menu items, gizmos, etc.

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
};
pub use host::PluginHost;
pub use registry::PluginRegistry;
pub use traits::*;

use bevy::prelude::*;
use std::collections::HashMap;

use abi::{EntityIdExt, PluginTransformExt};
use crate::core::{AppState, EditorEntity, SelectionState};

/// Plugin that manages the plugin host lifecycle
pub struct PluginCorePlugin;

impl Plugin for PluginCorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PluginHost>()
            .init_resource::<PluginRegistry>()
            .add_systems(Startup, initialize_plugin_host)
            .add_systems(
                Update,
                (
                    sync_bevy_to_plugins,
                    update_plugins,
                    apply_plugin_operations,
                    dispatch_selection_events,
                )
                    .chain()
                    .run_if(in_state(AppState::Editor)),
            );
    }
}

fn initialize_plugin_host(mut plugin_host: ResMut<PluginHost>) {
    if let Err(e) = plugin_host.discover_and_load_plugins() {
        error!("Failed to load plugins: {}", e);
    }
}

/// Sync Bevy state to the plugin API before plugin update
fn sync_bevy_to_plugins(
    mut plugin_host: ResMut<PluginHost>,
    selection: Res<SelectionState>,
    entities: Query<(Entity, &EditorEntity, &Transform)>,
) {
    // Build state snapshots
    let selected = selection.selected_entity.map(EntityId::from_bevy);

    let mut transforms = HashMap::new();
    let mut names = HashMap::new();

    for (entity, editor_entity, transform) in entities.iter() {
        let id = EntityId::from_bevy(entity);
        transforms.insert(id, PluginTransform::from_bevy(*transform));
        names.insert(id, editor_entity.name.clone());
    }

    // Sync to plugin API
    plugin_host.api_mut().sync_from_bevy(selected, transforms, names);
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
            PendingOperation::SpawnEntity(def) => {
                let transform: Transform = def.transform.to_bevy();
                let mut entity_commands = commands.spawn((
                    transform,
                    EditorEntity {
                        name: def.name.clone(),
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
