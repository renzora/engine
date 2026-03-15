//! Plugin system integration — Bevy systems and UI rendering for the plugin host.
//!
//! This module bridges `plugin_host` (pure FFI/loading library) with the editor's
//! ECS and egui UI, matching the legacy `plugin_core` module pattern where the
//! plugin system was part of the editor crate.

use std::collections::HashMap;
use std::path::PathBuf;

use bevy::prelude::*;
use editor_plugin_api::abi::EntityId;
use editor_plugin_api::events::EditorEvent;
use plugin_host::abi::{self, EntityIdExt, PluginTransformExt};
use plugin_host::api::PendingOperation;
use plugin_host::{PluginHost, PluginRegistry};
use renzora_core::CurrentProject;
use renzora_splash::{AppConfig, SplashState};

use crate::EditorSelection;

/// Bevy plugin that manages the plugin host lifecycle.
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
                    .run_if(in_state(SplashState::Editor)),
            );
    }
}

/// Check if project changed and load/unload plugins accordingly.
fn check_project_plugins(
    mut plugin_host: ResMut<PluginHost>,
    current_project: Option<Res<CurrentProject>>,
    app_config: Option<Res<AppConfig>>,
    mut last_project_path: Local<Option<PathBuf>>,
    mut system_plugins_loaded: Local<bool>,
) {
    if !*system_plugins_loaded {
        *system_plugins_loaded = true;
        if let Some(config) = &app_config {
            plugin_host.set_user_disabled_ids(config.disabled_plugins.clone());
        }
        if let Err(e) = plugin_host.discover_and_load_system_plugins() {
            error!("Failed to load system plugins: {}", e);
        }
    }

    let current_path = current_project.as_ref().map(|p| p.path.clone());

    if *last_project_path != current_path {
        plugin_host.unload_project_plugins();

        if let Some(ref project_path) = current_path {
            let plugins_dir = project_path.join("plugins");
            plugin_host.set_plugin_dir(plugins_dir);

            if let Err(e) = plugin_host.discover_and_load_plugins() {
                error!("Failed to load project plugins: {}", e);
            }
        }

        *last_project_path = current_path;
    }

    plugin_host.check_for_changes();
}

/// Sync Bevy state to the plugin API before plugin update.
fn sync_bevy_to_plugins(
    mut plugin_host: ResMut<PluginHost>,
    selection: Res<EditorSelection>,
    current_project: Option<Res<CurrentProject>>,
    entities: Query<(
        Entity,
        Option<&Name>,
        &Transform,
        Option<&Visibility>,
        Option<&ChildOf>,
    )>,
    children_query: Query<&Children>,
) {
    let assets_path = current_project.as_ref().map(|p| p.path.join("assets"));
    plugin_host.api_mut().set_project_assets_path(assets_path);

    let selected = selection.get().map(EntityId::from_bevy);

    let mut transforms = HashMap::new();
    let mut names = HashMap::new();
    let mut visibility = HashMap::new();
    let mut parents = HashMap::new();
    let mut children_map: HashMap<EntityId, Vec<EntityId>> = HashMap::new();

    for (entity, name, transform, vis, child_of) in entities.iter() {
        let id = EntityId::from_bevy(entity);
        transforms.insert(id, PluginTransformExt::from_bevy(*transform));
        names.insert(
            id,
            name.map(|n| n.as_str().to_string())
                .unwrap_or_else(|| format!("Entity {:?}", entity)),
        );
        let is_visible = vis.map(|v| *v != Visibility::Hidden).unwrap_or(true);
        visibility.insert(id, is_visible);

        let parent_id = child_of.map(|c| EntityId::from_bevy(c.parent()));
        parents.insert(id, parent_id);

        if let Ok(children) = children_query.get(entity) {
            let child_ids: Vec<EntityId> =
                children.iter().map(|c| EntityId::from_bevy(c)).collect();
            children_map.insert(id, child_ids);
        } else {
            children_map.insert(id, Vec::new());
        }
    }

    let api = plugin_host.api_mut();
    api.sync_from_bevy(selected, transforms, names, visibility, parents, children_map);
}

/// Update all plugins (called every frame).
fn update_plugins(mut plugin_host: ResMut<PluginHost>, time: Res<Time>) {
    plugin_host.update(time.delta_secs());
}

/// Apply pending operations from plugins to Bevy world.
fn apply_plugin_operations(
    mut plugin_host: ResMut<PluginHost>,
    selection: Res<EditorSelection>,
    mut transforms: Query<&mut Transform>,
    mut names: Query<&mut Name>,
    mut visibilities: Query<&mut Visibility>,
    mut commands: Commands,
) {
    let operations = plugin_host.api_mut().take_pending_operations();

    for op in operations {
        match op {
            PendingOperation::SetSelectedEntity(entity_id) => {
                let bevy_entity = entity_id.and_then(|id| id.to_bevy());
                selection.set(bevy_entity);
            }
            PendingOperation::SetTransform { entity, transform } => {
                if let Some(bevy_entity) = entity.to_bevy() {
                    if let Ok(mut t) = transforms.get_mut(bevy_entity) {
                        *t = PluginTransformExt::to_bevy(&transform);
                    }
                }
            }
            PendingOperation::SetEntityName { entity, name } => {
                if let Some(bevy_entity) = entity.to_bevy() {
                    if let Ok(mut n) = names.get_mut(bevy_entity) {
                        n.set(name);
                    }
                }
            }
            PendingOperation::SetEntityVisible { entity, visible } => {
                if let Some(bevy_entity) = entity.to_bevy() {
                    if let Ok(mut vis) = visibilities.get_mut(bevy_entity) {
                        *vis = if visible {
                            Visibility::Inherited
                        } else {
                            Visibility::Hidden
                        };
                    }
                }
            }
            PendingOperation::SpawnEntity(def) => {
                let transform: Transform = abi::plugin_to_transform(def.transform);
                let mut entity_commands = commands.spawn((
                    transform,
                    Name::new(def.name.clone()),
                    Visibility::default(),
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
                }
            }
            PendingOperation::ReparentEntity { entity, new_parent } => {
                if let Some(bevy_entity) = entity.to_bevy() {
                    if let Some(parent_entity) = new_parent.and_then(|id| id.to_bevy()) {
                        commands.entity(bevy_entity).insert(ChildOf(parent_entity));
                    } else {
                        commands.entity(bevy_entity).remove::<ChildOf>();
                    }
                }
            }
            PendingOperation::LoadAsset(path) => {
                info!("Plugin requested asset load: {}", path);
            }
        }
    }
}

/// Track selection changes and dispatch events to plugins.
fn dispatch_selection_events(
    mut plugin_host: ResMut<PluginHost>,
    selection: Res<EditorSelection>,
    mut last_selection: Local<Option<Entity>>,
) {
    let current = selection.get();

    if current != *last_selection {
        if let Some(old) = *last_selection {
            plugin_host.queue_event(EditorEvent::EntityDeselected(EntityId::from_bevy(old)));
        }
        if let Some(new) = current {
            plugin_host.queue_event(EditorEvent::EntitySelected(EntityId::from_bevy(new)));
        }
        *last_selection = current;
    }
}
