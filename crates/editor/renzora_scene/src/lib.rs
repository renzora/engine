//! Renzora Scene — save/load scene entities to RON files using Bevy's DynamicScene.

use bevy::prelude::*;
use serde::de::DeserializeSeed;
use std::path::Path;

use renzora_core::{
    CurrentProject, EditorCamera, HideInHierarchy, MeshColor, MeshPrimitive, SaveSceneRequested,
};
use renzora_keybindings::{EditorAction, KeyBindings};
use renzora_lighting::SunData;
use renzora_splash::SplashState;

// ============================================================================
// Save
// ============================================================================

/// Save all scene entities to a RON file.
pub fn save_scene(world: &mut World, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let type_registry = world.resource::<AppTypeRegistry>().clone();

    // Collect scene entity IDs
    let mut entities: Vec<Entity> = Vec::new();
    let mut query = world.query_filtered::<Entity, (With<Name>, Without<HideInHierarchy>, Without<EditorCamera>)>();
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

    let scene = DynamicSceneBuilder::from_world(world)
        .deny_all()
        .allow_component::<Name>()
        .allow_component::<Transform>()
        .allow_component::<ChildOf>()
        .allow_component::<MeshPrimitive>()
        .allow_component::<MeshColor>()
        .allow_component::<DirectionalLight>()
        .allow_component::<PointLight>()
        .allow_component::<SpotLight>()
        .allow_component::<AmbientLight>()
        .allow_component::<SunData>()
        .allow_component::<Camera3d>()
        .allow_component::<Camera>()
        .extract_entities(entities.into_iter())
        .build();

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

/// Load a scene from a RON file.
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

    // Deserialize RON into DynamicScene
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

    // Write entities directly into the world (registry read guard must be dropped first)
    let mut entity_map = bevy::ecs::entity::EntityHashMap::default();
    match scene.write_to_world(world, &mut entity_map) {
        Ok(()) => {
            info!("Loaded scene from {} ({} entities mapped)", path.display(), entity_map.len());
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

fn rehydrate_meshes(
    mut commands: Commands,
    query: Query<(Entity, &MeshPrimitive, Option<&MeshColor>), Without<Mesh3d>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, primitive, color) in &query {
        let mesh = match primitive {
            MeshPrimitive::Cube => meshes.add(Cuboid::default()),
            MeshPrimitive::Sphere => meshes.add(Sphere::default()),
            MeshPrimitive::Plane { width, height } => {
                meshes.add(Plane3d::default().mesh().size(*width, *height))
            }
            MeshPrimitive::Cylinder => meshes.add(Cylinder::default()),
        };

        let base_color = color.map_or(Color::WHITE, |c| c.0);
        let material = materials.add(StandardMaterial {
            base_color,
            ..default()
        });

        commands.entity(entity).insert((Mesh3d(mesh), MeshMaterial3d(material)));
    }
}

fn rehydrate_suns(
    mut query: Query<(&SunData, &mut DirectionalLight, &mut Transform), Added<SunData>>,
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

// ============================================================================
// Keybinding-driven save
// ============================================================================

fn detect_save_keybinding(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<KeyBindings>,
) {
    if keybindings.just_pressed(EditorAction::SaveScene, &keyboard) {
        commands.insert_resource(SaveSceneRequested);
    }
}

fn save_scene_system(world: &mut World) {
    if world.remove_resource::<SaveSceneRequested>().is_some() {
        save_current_scene(world);
    }
}

// ============================================================================
// Load on entering editor
// ============================================================================

fn load_scene_on_enter(world: &mut World) {
    info!("load_scene_on_enter triggered");
    load_current_scene(world);
}

// ============================================================================
// Plugin
// ============================================================================

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<MeshPrimitive>()
            .register_type::<MeshColor>()
            .register_type::<SunData>()
            .add_systems(OnEnter(SplashState::Editor), load_scene_on_enter)
            .add_systems(
                Update,
                (rehydrate_meshes, rehydrate_suns, detect_save_keybinding, save_scene_system)
                    .run_if(in_state(SplashState::Editor)),
            );
    }
}
