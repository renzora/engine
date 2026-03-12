//! Drag-and-drop model spawning — detects asset drops on the viewport and
//! spawns GLTF/GLB models into the scene.

use std::path::PathBuf;

use bevy::camera::primitives::Aabb;
use bevy::prelude::*;
use bevy_egui::egui;

use renzora_core::{CurrentProject, EditorCamera, MeshInstanceData};
use renzora_editor::{EditorCommands, EditorSelection};
use renzora_ui::asset_drag::AssetDragPayload;

use crate::ViewportState;

/// Extensions accepted as droppable 3D models.
const MODEL_EXTENSIONS: &[&str] = &["glb", "gltf"];

/// Resource tracking pending GLTF loads that need to be spawned once ready.
#[derive(Resource, Default)]
pub struct PendingGltfLoads {
    pub loads: Vec<PendingLoad>,
}

pub struct PendingLoad {
    pub handle: Handle<Gltf>,
    pub name: String,
    pub asset_path: String,
    pub spawn_position: Vec3,
}

/// Marker component — entity needs its Y adjusted so the bottom sits on the ground.
#[derive(Component)]
pub struct NeedsGroundAlignment {
    pub target_y: f32,
}

/// Called from the viewport panel's `ui()` method (read-only `&World`).
///
/// Detects when a model asset is being dragged over the viewport and, on release,
/// queues a deferred command to initiate loading.
pub fn check_viewport_model_drop(ui: &mut egui::Ui, world: &World, viewport_rect: egui::Rect) {
    let Some(payload) = world.get_resource::<AssetDragPayload>() else {
        return;
    };
    if !payload.is_detached || !payload.matches_extensions(MODEL_EXTENSIONS) {
        return;
    }

    let pointer_pos = ui.ctx().pointer_latest_pos();
    let pointer_in_viewport = pointer_pos.map_or(false, |p| viewport_rect.contains(p));

    if !pointer_in_viewport {
        return;
    }

    // Check if the pointer was just released (= drop)
    let pointer_released = !ui.ctx().input(|i| i.pointer.any_down());
    if !pointer_released {
        return;
    }

    let path = payload.path.clone();
    let name = payload.name.clone();

    // Capture viewport info for ground-position computation in the deferred closure
    let screen_pos = pointer_pos.unwrap_or(viewport_rect.center());
    let vp_rect = viewport_rect;

    // Queue the spawn command (deferred — runs with &mut World)
    if let Some(commands) = world.get_resource::<EditorCommands>() {
        commands.push(move |world: &mut World| {
            let ground_pos = compute_ground_position(world, screen_pos, vp_rect)
                .unwrap_or(Vec3::ZERO);
            initiate_model_load(world, path, name, ground_pos);
        });
    }
}

/// Compute a world-space ground position (Y=0 plane) from a screen-space pointer.
fn compute_ground_position(
    world: &mut World,
    screen_pos: egui::Pos2,
    viewport_rect: egui::Rect,
) -> Option<Vec3> {
    // Query the editor camera
    let mut q = world.query_filtered::<(&GlobalTransform, &Camera), With<EditorCamera>>();
    let (camera_transform, camera) = q.iter(world).next()?;
    let camera_transform = *camera_transform;
    let camera = camera.clone();

    // Convert screen position to render-target coordinates
    let vp_state = world.get_resource::<ViewportState>()?;
    let vp_x = screen_pos.x - viewport_rect.min.x;
    let vp_y = screen_pos.y - viewport_rect.min.y;
    let render_x = vp_x / viewport_rect.width() * vp_state.current_size.x as f32;
    let render_y = vp_y / viewport_rect.height() * vp_state.current_size.y as f32;

    let ray = camera.viewport_to_world(&camera_transform, Vec2::new(render_x, render_y)).ok()?;

    // Intersect with Y=0 ground plane
    if ray.direction.y.abs() < 1e-6 {
        return Some(Vec3::new(ray.origin.x, 0.0, ray.origin.z));
    }
    let t = -ray.origin.y / ray.direction.y;
    if t < 0.0 {
        return Some(Vec3::ZERO);
    }
    let hit = ray.origin + ray.direction * t;
    Some(Vec3::new(hit.x, 0.0, hit.z))
}

/// Initiate loading a model file — called from a deferred `EditorCommands` closure.
fn initiate_model_load(world: &mut World, path: PathBuf, name: String, spawn_position: Vec3) {
    // Compute asset-relative path
    let asset_path = if let Some(project) = world.get_resource::<CurrentProject>() {
        let assets_dir = project.path.join("assets");
        let models_dir = assets_dir.join("models");

        // Copy to project assets/models/ if not already there
        let _ = std::fs::create_dir_all(&models_dir);

        let file_name = path.file_name().unwrap_or_default();
        let dest = models_dir.join(file_name);

        if !dest.exists() || path.canonicalize().ok() != dest.canonicalize().ok() {
            if let Err(e) = std::fs::copy(&path, &dest) {
                error!("Failed to copy model to project: {}", e);
            } else {
                info!("Copied model to project: {:?}", dest);
            }
        }

        project.make_asset_relative(&dest)
    } else {
        path.to_string_lossy().replace('\\', "/")
    };

    // Load via AssetServer
    let handle: Handle<Gltf> = world.resource::<AssetServer>().load(&asset_path);

    info!("Loading model '{}' from '{}'", name, asset_path);

    world
        .resource_mut::<PendingGltfLoads>()
        .loads
        .push(PendingLoad {
            handle,
            name,
            asset_path,
            spawn_position,
        });
}

/// System: poll pending GLTF loads, spawn entities when ready.
pub fn spawn_loaded_gltfs(
    mut commands: Commands,
    mut pending: ResMut<PendingGltfLoads>,
    gltf_assets: Res<Assets<Gltf>>,
    selection: Res<EditorSelection>,
) {
    let mut completed = Vec::new();

    for (index, load) in pending.loads.iter().enumerate() {
        let Some(gltf) = gltf_assets.get(&load.handle) else {
            continue;
        };

        // Pick the default scene, or the first scene
        let scene_handle = gltf
            .default_scene
            .clone()
            .or_else(|| gltf.scenes.first().cloned());

        let Some(scene) = scene_handle else {
            warn!("GLTF '{}' has no scenes", load.name);
            completed.push(index);
            continue;
        };

        let transform = Transform::from_translation(load.spawn_position);

        // Spawn the MeshInstance parent entity
        let parent = commands
            .spawn((
                Name::new(load.name.clone()),
                transform,
                Visibility::default(),
                MeshInstanceData {
                    model_path: Some(load.asset_path.clone()),
                },
            ))
            .id();

        // Spawn the GLTF scene as a child
        commands.spawn((
            bevy::scene::SceneRoot(scene),
            Transform::default(),
            Visibility::default(),
            ChildOf(parent),
        ));

        // Attach ground alignment marker
        commands.entity(parent).insert(NeedsGroundAlignment {
            target_y: load.spawn_position.y,
        });

        // Auto-select the new entity
        selection.set(Some(parent));

        info!("Spawned model '{}' at {:?}", load.name, load.spawn_position);
        completed.push(index);
    }

    // Remove completed loads in reverse order
    for index in completed.into_iter().rev() {
        pending.loads.remove(index);
    }
}

/// System: once child meshes have AABBs, offset the parent so its bottom sits on the ground.
pub fn align_models_to_ground(
    mut commands: Commands,
    query: Query<(Entity, &NeedsGroundAlignment, &Children)>,
    children_query: Query<&Children>,
    aabb_query: Query<(&Aabb, &GlobalTransform)>,
    mut transform_query: Query<&mut Transform>,
) {
    for (entity, alignment, children) in query.iter() {
        let mut lowest_y: Option<f32> = None;

        // Walk all descendants looking for AABBs
        let mut stack: Vec<Entity> = children.iter().collect();
        while let Some(child) = stack.pop() {
            if let Ok((aabb, global_transform)) = aabb_query.get(child) {
                let center = Vec3::from(aabb.center);
                let half = Vec3::from(aabb.half_extents);

                // Check all 8 AABB corners in world space
                for sx in [-1.0f32, 1.0] {
                    for sy in [-1.0f32, 1.0] {
                        for sz in [-1.0f32, 1.0] {
                            let corner: Vec3 = center + half * Vec3::new(sx, sy, sz);
                            let world_pos: Vec3 = global_transform.transform_point(corner);
                            lowest_y = Some(lowest_y.map_or(world_pos.y, |prev: f32| prev.min(world_pos.y)));
                        }
                    }
                }
            }

            if let Ok(grandchildren) = children_query.get(child) {
                stack.extend(grandchildren.iter());
            }
        }

        if let Some(lowest_world_y) = lowest_y {
            let offset = alignment.target_y - lowest_world_y;
            if let Ok(mut transform) = transform_query.get_mut(entity) {
                transform.translation.y += offset;
            }
            commands.entity(entity).remove::<NeedsGroundAlignment>();
        }
    }
}
