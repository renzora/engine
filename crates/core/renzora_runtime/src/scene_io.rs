//! Shared scene load/save and rehydration — used by both editor and runtime.

use bevy::ecs::world::FilteredEntityRef;
use bevy::prelude::*;
use renzora_core::{CurrentProject, DefaultCamera, EditorCamera, HideInHierarchy, MeshColor, MeshPrimitive, SceneCamera, ShapeRegistry};
use renzora_lighting::Sun;
use serde::de::DeserializeSeed;
use std::path::Path;

// ============================================================================
// Save
// ============================================================================

/// Save specific entities to a RON file.
pub fn save_scene(world: &mut World, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let type_registry = world.resource::<AppTypeRegistry>().clone();

    let mut entities: Vec<Entity> = Vec::new();
    let mut query = world.query_filtered::<Entity, (With<Name>, Without<HideInHierarchy>, Without<EditorCamera>, Without<bevy::input::gamepad::Gamepad>)>();
    for entity in query.iter(world) {
        entities.push(entity);
    }

    if entities.is_empty() {
        let content = "(entities: {}, resources: {})";
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        info!("Saved empty scene to {}", path.display());
        return Ok(());
    }

    let mut scene = DynamicSceneBuilder::from_world(world)
        .deny_all_resources()
        .deny_component::<Mesh3d>()
        .deny_component::<MeshMaterial3d<StandardMaterial>>()
        .deny_component::<Camera3d>()
        .deny_component::<Camera>()
        .deny_component::<GlobalTransform>()
        .deny_component::<Visibility>()
        .deny_component::<InheritedVisibility>()
        .deny_component::<ViewVisibility>()
        .deny_component::<Children>()
        .deny_component::<bevy::transform::components::TransformTreeChanged>()
        .deny_component::<bevy::camera::primitives::Aabb>()
        .deny_component::<bevy::render::sync_world::SyncToRenderWorld>()
        .deny_component::<bevy::input::gamepad::Gamepad>()
        .deny_component::<bevy::input::gamepad::GamepadSettings>()
        .extract_entities(entities.into_iter())
        .build();

    // Strip components that can't be serialized to avoid hard failures.
    // We trial-serialize each component and drop any that fail.
    {
        let registry = type_registry.read();
        for entity in &mut scene.entities {
            entity.components.retain(|component| {
                let serializer = bevy::reflect::serde::TypedReflectSerializer::new(
                    component.as_partial_reflect(),
                    &registry,
                );
                // Try serializing to a throwaway RON value
                ron::ser::to_string(&serializer).is_ok()
            });
        }
    }

    let registry = type_registry.read();
    let serialized = scene
        .serialize(&registry)
        .map_err(|e| format!("Scene serialization failed: {e}"))?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, &serialized)?;
    info!("Saved scene to {} ({} entities)", path.display(), scene.entities.len());
    Ok(())
}

/// Save the current project's main scene.
pub fn save_current_scene(world: &mut World) {
    let Some(project) = world.get_resource::<CurrentProject>() else {
        warn!("No project open — cannot save scene");
        return;
    };
    let path = project.main_scene_path();
    if let Err(e) = save_scene(world, &path) {
        error!("Failed to save scene: {}", e);
    }
}

// ============================================================================
// Load
// ============================================================================

/// Load a scene from a RON file into the world.
pub fn load_scene(world: &mut World, path: &Path) {
    if !path.exists() {
        info!("Scene file does not exist yet: {}", path.display());
        return;
    }

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to read scene file {}: {}", path.display(), e);
            return;
        }
    };

    let trimmed = content.trim();
    if trimmed.is_empty() || trimmed == "(entities: {}, resources: {})" {
        info!("Scene is empty: {}", path.display());
        return;
    }

    let scene = {
        let type_registry = world.resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();

        let scene_deserializer = bevy::scene::serde::SceneDeserializer {
            type_registry: &registry,
        };

        let mut ron_deserializer = match ron::Deserializer::from_str(&content) {
            Ok(d) => d,
            Err(e) => {
                error!("Failed to parse RON {}: {}", path.display(), e);
                return;
            }
        };

        match scene_deserializer.deserialize(&mut ron_deserializer) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to deserialize scene {}: {}", path.display(), e);
                return;
            }
        }
    };

    let mut entity_map = bevy::ecs::entity::EntityHashMap::default();
    match scene.write_to_world(world, &mut entity_map) {
        Ok(()) => {
            info!("Loaded scene from {} ({} entities mapped)", path.display(), entity_map.len());

            // Bevy's write_to_world inserts ChildOf via reflection, which may not
            // trigger the on_insert hooks that maintain the parent's Children component.
            // Re-insert ChildOf on each child to force the hooks to fire.
            let children_with_parents: Vec<(Entity, Entity)> = entity_map
                .values()
                .filter_map(|&entity| {
                    world.entity(entity).get::<ChildOf>().map(|c| (entity, c.parent()))
                })
                .collect();

            for (child, parent) in children_with_parents {
                // Remove and re-insert ChildOf to trigger hooks
                world.entity_mut(child).remove::<ChildOf>();
                world.entity_mut(child).insert(ChildOf(parent));
            }
        }
        Err(e) => {
            error!("Failed to write scene to world {}: {}", path.display(), e);
        }
    }
}

/// Load the current project's main scene.
pub fn load_current_scene(world: &mut World) {
    let Some(project) = world.get_resource::<CurrentProject>() else {
        warn!("load_current_scene: no CurrentProject resource");
        return;
    };
    let path = project.main_scene_path();
    info!("load_current_scene: loading from {}", path.display());
    load_scene(world, &path);
}

// ============================================================================
// Rehydration
// ============================================================================

/// Rehydrate mesh primitives — spawns `Mesh3d` + `MeshMaterial3d` for entities that have
/// `MeshPrimitive` but no `Mesh3d` yet (e.g. after scene deserialization).
pub fn rehydrate_meshes(
    mut commands: Commands,
    query: Query<(Entity, &MeshPrimitive, Option<&MeshColor>), Without<Mesh3d>>,
    registry: Res<ShapeRegistry>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, primitive, color) in &query {
        let Some(mesh) = registry.create_mesh(&primitive.0, &mut meshes) else {
            warn!("Unknown shape ID '{}' — skipping rehydration", primitive.0);
            continue;
        };

        let base_color = color.map_or(Color::WHITE, |c| c.0);
        let material = materials.add(StandardMaterial {
            base_color,
            ..default()
        });

        commands.entity(entity).insert((Mesh3d(mesh), MeshMaterial3d(material)));
    }
}

/// Ensure parent entities have `Visibility` so transform/visibility propagation works.
/// Fixes groups/empty parents that were saved without `Visibility`.
pub fn rehydrate_visibility(
    mut commands: Commands,
    query: Query<Entity, (With<Children>, Without<Visibility>)>,
) {
    for entity in &query {
        commands.entity(entity).insert(Visibility::default());
    }
}

/// Rehydrate scene cameras — spawns `Camera3d` for entities that have `SceneCamera` but no `Camera3d`.
///
/// In runtime mode (no editor), the `DefaultCamera` is active; if none is marked,
/// the first scene camera wins. All others are inactive.
/// In editor mode, all scene cameras are inactive (the editor camera renders).
pub fn rehydrate_cameras(
    mut commands: Commands,
    query: Query<(Entity, Option<&DefaultCamera>), (With<SceneCamera>, Without<Camera3d>)>,
    editor_camera: Query<(), With<EditorCamera>>,
) {
    if query.is_empty() { return; }

    let is_editor = !editor_camera.is_empty();

    // Find which entity should be the active camera in runtime mode
    let default_entity = query.iter()
        .find(|(_, dc)| dc.is_some())
        .or_else(|| query.iter().next())
        .map(|(e, _)| e);

    for (entity, _) in &query {
        let is_active = !is_editor && default_entity == Some(entity);

        commands.entity(entity).insert((
            Camera3d::default(),
            Camera {
                is_active,
                ..default()
            },
        ));
    }
}

/// Ensures only the default scene camera is active in runtime mode.
///
/// Runs every frame (cheap — early-exits if no changes). Handles cameras that
/// were deserialized with `Camera3d` already present (so `rehydrate_cameras` skipped them).
pub fn enforce_single_active_camera(
    mut cameras: Query<(Entity, &mut Camera, Option<&DefaultCamera>), With<SceneCamera>>,
    editor_camera: Query<(), With<EditorCamera>>,
) {
    if !editor_camera.is_empty() { return; }
    if cameras.is_empty() { return; }

    // Find which entity should be active: DefaultCamera > first
    let default_entity = cameras.iter()
        .find(|(_, _, dc)| dc.is_some())
        .or_else(|| cameras.iter().next())
        .map(|(e, _, _)| e);

    for (entity, mut camera, _) in &mut cameras {
        let should_be_active = default_entity == Some(entity);
        if camera.is_active != should_be_active {
            camera.is_active = should_be_active;
        }
    }
}

fn should_sync(type_path: &str) -> bool {
    type_path.ends_with("Settings")
}

/// Sync post-process (and other reflected) components from SceneCamera entities to the EditorCamera.
///
/// In editor mode the viewport renders through the EditorCamera, but users attach
/// effects to the SceneCamera entity. This system mirrors those components so they
/// take effect during editing.
pub fn sync_scene_camera_to_editor_camera(world: &mut World) {
    // Find the scene camera and editor camera entities.
    let mut scene_cam = None;
    let mut editor_cam = None;
    let mut q = world.query_filtered::<Entity, With<SceneCamera>>();
    for e in q.iter(world) {
        scene_cam = Some(e);
        break;
    }
    let mut q = world.query_filtered::<Entity, With<EditorCamera>>();
    for e in q.iter(world) {
        editor_cam = Some(e);
        break;
    }
    let Some(dst) = editor_cam else {
        return;
    };

    // If no scene camera exists, remove all synced components from the editor camera.
    let Some(src) = scene_cam else {
        let type_registry = world.resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let mut to_remove: Vec<bevy::ecs::reflect::ReflectComponent> = Vec::new();
        let editor_ref = world.entity(dst);
        for reg in registry.iter() {
            let Some(reflect_component) = reg.data::<bevy::ecs::reflect::ReflectComponent>() else {
                continue;
            };
            let type_path = reg.type_info().type_path();
            if !should_sync(type_path) {
                continue;
            }
            if reflect_component.contains(FilteredEntityRef::from(editor_ref)) {
                to_remove.push(reflect_component.clone());
            }
        }
        drop(registry);
        for reflect_component in &to_remove {
            reflect_component.remove(&mut world.entity_mut(dst));
        }
        return;
    };

    if src == dst {
        return;
    }

    let type_registry = world.resource::<AppTypeRegistry>().clone();
    let registry = type_registry.read();

    // Collect reflected component data from the scene camera.
    let mut components_to_sync: Vec<(
        bevy::ecs::reflect::ReflectComponent,
        Box<dyn Reflect>,
    )> = Vec::new();
    let mut synced_type_paths: Vec<&'static str> = Vec::new();

    let entity_ref = world.entity(src);
    for reg in registry.iter() {
        let Some(reflect_component) = reg.data::<bevy::ecs::reflect::ReflectComponent>() else {
            continue;
        };
        let type_path = reg.type_info().type_path();
        if !should_sync(type_path) {
            continue;
        }
        if let Some(reflected) = reflect_component.reflect(FilteredEntityRef::from(entity_ref)) {
            if let Ok(cloned) = reflected.reflect_clone() {
                components_to_sync.push((reflect_component.clone(), cloned));
                synced_type_paths.push(type_path);
            }
        }
    }
    drop(registry);

    // Apply collected components to the editor camera.
    {
        let registry = type_registry.read();
        for (reflect_component, value) in &components_to_sync {
            let mut entity_mut = world.entity_mut(dst);
            if reflect_component.contains(entity_mut.as_readonly()) {
                reflect_component.apply(entity_mut, value.as_partial_reflect());
            } else {
                reflect_component.insert(&mut entity_mut, value.as_partial_reflect(), &registry);
            }
        }
    }

    // Remove components from editor camera that were removed from scene camera.
    let registry = type_registry.read();
    let mut to_remove: Vec<bevy::ecs::reflect::ReflectComponent> = Vec::new();
    let editor_ref = world.entity(dst);
    for reg in registry.iter() {
        let Some(reflect_component) = reg.data::<bevy::ecs::reflect::ReflectComponent>() else {
            continue;
        };
        let type_path = reg.type_info().type_path();
        if !should_sync(type_path) {
            continue;
        }
        if reflect_component.contains(FilteredEntityRef::from(editor_ref))
            && !synced_type_paths.contains(&type_path)
        {
            to_remove.push(reflect_component.clone());
        }
    }
    drop(registry);

    for reflect_component in &to_remove {
        reflect_component.remove(&mut world.entity_mut(dst));
    }
}

/// Rehydrate sun entities — syncs `DirectionalLight` + `Transform` from `Sun` on newly added entities.
pub fn rehydrate_suns(
    mut query: Query<(&Sun, &mut DirectionalLight, &mut Transform), Added<Sun>>,
) {
    for (sun, mut light, mut transform) in &mut query {
        light.color = Color::srgb(sun.color.x, sun.color.y, sun.color.z);
        light.illuminance = sun.illuminance;
        light.shadows_enabled = sun.shadows_enabled;
        *transform = Transform::from_rotation(
            Quat::from_rotation_arc(Vec3::NEG_Z, sun.direction()),
        );
    }
}
