//! Model preview system - renders 3D model thumbnails for the asset browser
//!
//! Optimized for performance while maintaining visual quality:
//! - Fast LDR rendering (no HDR/bloom overhead)
//! - Minimal lighting (2 lights, no shadows)
//! - Quick capture with minimal frame delay
//! - Disk caching for instant loading on subsequent runs

use bevy::prelude::*;
use bevy::camera::RenderTarget;
use bevy::camera::primitives::Aabb;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::render::gpu_readback::Readback;
use bevy::scene::SceneRoot;
use bevy_egui::egui::TextureId;
use bevy_egui::EguiContexts;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::fs;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use std::process::{Child, Command, Stdio};

use crate::scene::EditorOnly;

/// Cache directory for thumbnails (relative to project root)
const THUMBNAIL_CACHE_DIR: &str = ".renzora/thumbnails";

/// Get the cache file path for a model
fn get_cache_path(model_path: &PathBuf) -> PathBuf {
    // Create a hash of the model path for the cache filename
    let mut hasher = DefaultHasher::new();
    model_path.hash(&mut hasher);
    let hash = hasher.finish();

    PathBuf::from(THUMBNAIL_CACHE_DIR).join(format!("{:016x}.png", hash))
}

/// Check if cached thumbnail is valid (exists and newer than source)
fn is_cache_valid(model_path: &PathBuf, cache_path: &PathBuf) -> bool {
    let cache_meta = match fs::metadata(cache_path) {
        Ok(m) => m,
        Err(_) => return false,
    };

    let source_meta = match fs::metadata(model_path) {
        Ok(m) => m,
        Err(_) => return false,
    };

    // Check if cache is newer than source
    match (cache_meta.modified(), source_meta.modified()) {
        (Ok(cache_time), Ok(source_time)) => cache_time >= source_time,
        _ => false,
    }
}

/// Save raw BGRA pixel data to PNG file
#[cfg(feature = "editor")]
fn save_thumbnail_data_to_cache(data: &[u8], width: u32, height: u32, cache_path: &PathBuf) -> Result<(), String> {
    // Ensure cache directory exists
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // Convert BGRA to RGBA for PNG
    let mut rgba_data = Vec::with_capacity(data.len());
    for chunk in data.chunks(4) {
        if chunk.len() == 4 {
            rgba_data.push(chunk[2]); // R (was B)
            rgba_data.push(chunk[1]); // G
            rgba_data.push(chunk[0]); // B (was R)
            rgba_data.push(chunk[3]); // A
        }
    }

    // Use image crate to save PNG
    let img = image::RgbaImage::from_raw(width, height, rgba_data)
        .ok_or("Failed to create image buffer")?;

    img.save(cache_path).map_err(|e| e.to_string())?;

    Ok(())
}

/// Preview image size for model thumbnails
const MODEL_PREVIEW_SIZE: u32 = 128;

/// Offset position for model preview rendering (far from main scene)
/// Using very large offset to avoid any interference with the main scene
const PREVIEW_AREA_OFFSET: Vec3 = Vec3::new(50000.0, 50000.0, 50000.0);

/// Resource that manages model preview generation
#[derive(Resource, Default)]
pub struct ModelPreviewCache {
    /// Queued model paths waiting to be rendered
    pub queue: VecDeque<PathBuf>,
    /// Completed preview textures ready for display
    pub textures: HashMap<PathBuf, Handle<Image>>,
    /// Texture IDs registered with egui
    pub texture_ids: HashMap<PathBuf, TextureId>,
    /// Paths that failed to load
    pub failed: std::collections::HashSet<PathBuf>,
    /// Paths that have been requested (to avoid duplicate requests)
    pub requested: std::collections::HashSet<PathBuf>,
    /// Background render processes (path -> child process)
    #[cfg(feature = "editor")]
    pub render_processes: Vec<(PathBuf, Child)>,
    /// Models being rendered in-process (fallback if subprocess fails)
    pub processing: HashMap<PathBuf, ModelPreviewState>,
    /// Frames since last render was started (for throttling)
    pub frames_since_render: u32,
}

/// State for a model preview being generated
pub struct ModelPreviewState {
    /// Handle to the GLTF asset being loaded
    pub gltf_handle: Handle<Gltf>,
    /// Entity of the spawned scene (once loaded)
    pub scene_entity: Option<Entity>,
    /// Entity of the preview camera
    pub camera_entity: Option<Entity>,
    /// Handle to the render texture
    pub texture_handle: Handle<Image>,
    /// Entity of the preview light
    pub light_entity: Option<Entity>,
    /// Entity of the ground plane
    pub ground_entity: Option<Entity>,
    /// Frames waited for scene to settle
    pub frames_waited: u32,
    /// Whether camera has been positioned and is ready to render
    pub camera_positioned: bool,
    /// Frames waited after positioning camera (for render to complete)
    pub render_frames_waited: u32,
}

impl ModelPreviewCache {
    /// Request a model preview to be generated
    pub fn request_preview(&mut self, path: PathBuf) -> bool {
        if self.textures.contains_key(&path)
            || self.texture_ids.contains_key(&path)
            || self.processing.contains_key(&path)
            || self.queue.contains(&path)
            || self.failed.contains(&path)
            || self.requested.contains(&path)
        {
            return false;
        }
        self.requested.insert(path.clone());
        self.queue.push_back(path);
        true
    }

    /// Check if a preview is ready
    pub fn get_texture_id(&self, path: &PathBuf) -> Option<TextureId> {
        self.texture_ids.get(path).copied()
    }

    /// Check if a preview is loading
    pub fn is_loading(&self, path: &PathBuf) -> bool {
        self.processing.contains_key(path) || self.queue.contains(path)
    }

    /// Check if preview generation failed
    pub fn has_failed(&self, path: &PathBuf) -> bool {
        self.failed.contains(path)
    }
}

/// Marker component for model preview cameras
#[derive(Component)]
pub struct ModelPreviewCamera {
    pub model_path: PathBuf,
}

/// Marker component for model preview scenes
#[derive(Component)]
pub struct ModelPreviewScene {
    pub model_path: PathBuf,
}

/// Marker component for model preview lights
#[derive(Component)]
pub struct ModelPreviewLight {
    pub model_path: PathBuf,
}

/// Marker component for model preview ground planes
#[derive(Component)]
pub struct ModelPreviewGround {
    pub model_path: PathBuf,
}

/// Maximum concurrent subprocess renders
const MAX_SUBPROCESS_RENDERS: usize = 3;

/// System that processes the model preview queue (checks disk cache first, spawns subprocesses)
pub fn process_model_preview_queue(
    mut preview_cache: ResMut<ModelPreviewCache>,
    mut images: ResMut<Assets<Image>>,
) {
    // Poll completed subprocess renders and load their results
    #[cfg(feature = "editor")]
    {
        let mut completed = Vec::new();
        for (i, (path, child)) in preview_cache.render_processes.iter_mut().enumerate() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    completed.push((i, path.clone(), status.success()));
                }
                Ok(None) => {} // Still running
                Err(_) => {
                    completed.push((i, path.clone(), false));
                }
            }
        }

        // Remove completed processes and load results (in reverse order to maintain indices)
        for (i, path, success) in completed.into_iter().rev() {
            preview_cache.render_processes.remove(i);

            if success {
                // Load the cached PNG immediately
                let cache_path = get_cache_path(&path);
                if let Ok(data) = fs::read(&cache_path) {
                    if let Ok(img) = image::load_from_memory(&data) {
                        let rgba = img.to_rgba8();
                        let (width, height) = rgba.dimensions();

                        let mut bgra_data = Vec::with_capacity((width * height * 4) as usize);
                        for pixel in rgba.pixels() {
                            bgra_data.push(pixel[2]);
                            bgra_data.push(pixel[1]);
                            bgra_data.push(pixel[0]);
                            bgra_data.push(pixel[3]);
                        }

                        let image = Image::new(
                            Extent3d { width, height, depth_or_array_layers: 1 },
                            TextureDimension::D2,
                            bgra_data,
                            TextureFormat::Bgra8UnormSrgb,
                            default(),
                        );

                        let handle = images.add(image);
                        preview_cache.textures.insert(path.clone(), handle);
                        continue;
                    }
                }
            }

            // Failed to load or process failed
            preview_cache.failed.insert(path);
        }
    }

    // Process cache loads (these are fast and don't affect FPS)
    let mut cache_loads = 0;
    while cache_loads < 30 {
        let Some(path) = preview_cache.queue.front().cloned() else {
            break;
        };

        // Check disk cache
        let cache_path = get_cache_path(&path);
        if is_cache_valid(&path, &cache_path) {
            preview_cache.queue.pop_front();
            if let Ok(data) = fs::read(&cache_path) {
                if let Ok(img) = image::load_from_memory(&data) {
                    let rgba = img.to_rgba8();
                    let (width, height) = rgba.dimensions();

                    // Convert RGBA to BGRA for Bevy
                    let mut bgra_data = Vec::with_capacity((width * height * 4) as usize);
                    for pixel in rgba.pixels() {
                        bgra_data.push(pixel[2]); // B
                        bgra_data.push(pixel[1]); // G
                        bgra_data.push(pixel[0]); // R
                        bgra_data.push(pixel[3]); // A
                    }

                    let image = Image::new(
                        Extent3d { width, height, depth_or_array_layers: 1 },
                        TextureDimension::D2,
                        bgra_data,
                        TextureFormat::Bgra8UnormSrgb,
                        default(),
                    );

                    let handle = images.add(image);
                    preview_cache.textures.insert(path, handle);
                    cache_loads += 1;
                    continue;
                }
            }
            // Cache read failed, put back for subprocess render
            preview_cache.queue.push_front(path);
        }
        break; // Not in cache, stop cache loading loop
    }

    // Spawn subprocess renders for items not in cache
    #[cfg(feature = "editor")]
    {
        while preview_cache.render_processes.len() < MAX_SUBPROCESS_RENDERS {
            let Some(path) = preview_cache.queue.pop_front() else {
                break;
            };

            // Double-check cache
            let cache_path = get_cache_path(&path);
            if is_cache_valid(&path, &cache_path) {
                preview_cache.queue.push_front(path);
                break;
            }

            // Get current executable path
            let exe_path = match std::env::current_exe() {
                Ok(p) => p,
                Err(_) => {
                    preview_cache.failed.insert(path);
                    continue;
                }
            };

            // Spawn subprocess
            let model_path_str = path.to_string_lossy().to_string();
            let cache_path_str = cache_path.to_string_lossy().to_string();

            match Command::new(&exe_path)
                .args(["--render-thumbnail", &model_path_str, &cache_path_str])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(child) => {
                    preview_cache.render_processes.push((path, child));
                }
                Err(e) => {
                    warn!("Failed to spawn thumbnail renderer: {}", e);
                    preview_cache.failed.insert(path);
                }
            }
        }
    }
}

/// System that spawns loaded models and their preview cameras
pub fn spawn_model_previews(
    mut commands: Commands,
    mut preview_cache: ResMut<ModelPreviewCache>,
    asset_server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    use bevy::asset::LoadState;

    let mut to_remove = Vec::new();
    let mut paths_to_spawn = Vec::new();

    // Check loading status for models in processing
    for (path, state) in preview_cache.processing.iter() {
        if state.scene_entity.is_some() {
            continue; // Already spawned
        }

        match asset_server.get_load_state(&state.gltf_handle) {
            Some(LoadState::Loaded) => {
                if let Some(gltf) = gltfs.get(&state.gltf_handle) {
                    if let Some(scene_handle) = gltf.default_scene.clone().or_else(|| gltf.scenes.first().cloned()) {
                        paths_to_spawn.push((path.clone(), scene_handle, state.texture_handle.clone()));
                    } else {
                        // No scene in GLTF
                        to_remove.push(path.clone());
                    }
                }
            }
            Some(LoadState::Failed(_)) => {
                to_remove.push(path.clone());
            }
            _ => {}
        }
    }

    // Mark failed loads
    for path in to_remove {
        preview_cache.processing.remove(&path);
        preview_cache.failed.insert(path);
    }

    // Spawn models with unique offsets
    for (i, (path, scene_handle, texture_handle)) in paths_to_spawn.into_iter().enumerate() {
        let offset = PREVIEW_AREA_OFFSET + Vec3::new(i as f32 * 100.0, 0.0, 0.0);

        // Spawn the model scene
        let scene_entity = commands
            .spawn((
                Transform::from_translation(offset),
                Visibility::default(),
                ModelPreviewScene { model_path: path.clone() },
                SceneRoot(scene_handle),
                EditorOnly,
            ))
            .id();

        // No ground plane - transparent background
        let ground_entity: Option<Entity> = None;

        // Simple 2-light setup for fast rendering
        // Key light from upper-right front
        let light_entity = commands
            .spawn((
                DirectionalLight {
                    illuminance: 12000.0,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, 0.5, 0.0)),
                ModelPreviewLight { model_path: path.clone() },
                EditorOnly,
            ))
            .id();

        // Fill light from opposite side to soften shadows
        commands.spawn((
            DirectionalLight {
                illuminance: 6000.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.3, -1.2, 0.0)),
            ModelPreviewLight { model_path: path.clone() },
            EditorOnly,
        ));

        // Spawn lightweight preview camera with transparent background
        let camera_entity = commands
            .spawn((
                Camera3d::default(),
                Camera {
                    clear_color: ClearColorConfig::Custom(Color::srgba(0.0, 0.0, 0.0, 0.0)),
                    order: -100,
                    ..default()
                },
                RenderTarget::Image(texture_handle.clone().into()),
                Projection::Perspective(PerspectiveProjection {
                    fov: 45.0_f32.to_radians(),
                    aspect_ratio: 1.0,
                    near: 0.01,
                    far: 1000.0,
                    ..default()
                }),
                Transform::from_translation(offset + Vec3::new(2.0, 2.0, 2.0))
                    .looking_at(offset, Vec3::Y),
                ModelPreviewCamera { model_path: path.clone() },
                EditorOnly,
            ))
            .id();

        if let Some(state) = preview_cache.processing.get_mut(&path) {
            state.scene_entity = Some(scene_entity);
            state.camera_entity = Some(camera_entity);
            state.light_entity = Some(light_entity);
            state.ground_entity = ground_entity;
        }
    }
}

/// System that positions cameras based on model bounds and captures the preview
pub fn capture_model_previews(
    mut commands: Commands,
    mut preview_cache: ResMut<ModelPreviewCache>,
    mut camera_query: Query<&mut Transform, (With<ModelPreviewCamera>, Without<ModelPreviewScene>, Without<ModelPreviewGround>)>,
    mut projection_query: Query<&mut Projection, With<ModelPreviewCamera>>,
    mut ground_query: Query<&mut Transform, (With<ModelPreviewGround>, Without<ModelPreviewCamera>, Without<ModelPreviewScene>)>,
    scene_transform_query: Query<&GlobalTransform, With<ModelPreviewScene>>,
    mesh_query: Query<(&GlobalTransform, Option<&Aabb>), With<Mesh3d>>,
    children_query: Query<&Children>,
) {
    let mut completed = Vec::new();

    for (path, state) in preview_cache.processing.iter_mut() {
        let Some(scene_entity) = state.scene_entity else {
            continue;
        };
        let Some(camera_entity) = state.camera_entity else {
            continue;
        };

        // If camera is already positioned, just wait for render to complete
        if state.camera_positioned {
            state.render_frames_waited += 1;
            // Wait 2 frames for render (minimal for LDR without post-processing)
            if state.render_frames_waited >= 2 {
                completed.push(path.clone());
            }
            continue;
        }

        // Wait for the scene to fully load and transforms to propagate
        // Scenes are spawned asynchronously and need several frames to instantiate
        state.frames_waited += 1;
        if state.frames_waited < 10 {
            continue;
        }

        // Verify scene entity exists
        let Ok(scene_global_transform) = scene_transform_query.get(scene_entity) else {
            continue;
        };

        // Calculate world-space bounding box using mesh AABBs
        // This properly accounts for actual geometry size, not just positions
        let mut min_bound = Vec3::splat(f32::MAX);
        let mut max_bound = Vec3::splat(f32::MIN);
        let mut found_meshes = false;

        // Recursively find all meshes and compute world-space AABB
        fn find_meshes_recursive(
            entity: Entity,
            children_query: &Query<&Children>,
            mesh_query: &Query<(&GlobalTransform, Option<&Aabb>), With<Mesh3d>>,
            min_bound: &mut Vec3,
            max_bound: &mut Vec3,
            found_meshes: &mut bool,
        ) {
            if let Ok((global_transform, aabb)) = mesh_query.get(entity) {
                *found_meshes = true;

                // Get the world-space AABB by transforming the local AABB corners
                if let Some(aabb) = aabb {
                    // Transform AABB corners to world space
                    let center = aabb.center;
                    let half_extents = aabb.half_extents;

                    // Generate all 8 corners of the AABB
                    let corners = [
                        Vec3::new(center.x - half_extents.x, center.y - half_extents.y, center.z - half_extents.z),
                        Vec3::new(center.x + half_extents.x, center.y - half_extents.y, center.z - half_extents.z),
                        Vec3::new(center.x - half_extents.x, center.y + half_extents.y, center.z - half_extents.z),
                        Vec3::new(center.x + half_extents.x, center.y + half_extents.y, center.z - half_extents.z),
                        Vec3::new(center.x - half_extents.x, center.y - half_extents.y, center.z + half_extents.z),
                        Vec3::new(center.x + half_extents.x, center.y - half_extents.y, center.z + half_extents.z),
                        Vec3::new(center.x - half_extents.x, center.y + half_extents.y, center.z + half_extents.z),
                        Vec3::new(center.x + half_extents.x, center.y + half_extents.y, center.z + half_extents.z),
                    ];

                    // Transform each corner to world space
                    for corner in corners {
                        let world_corner = global_transform.transform_point(corner);
                        *min_bound = min_bound.min(world_corner);
                        *max_bound = max_bound.max(world_corner);
                    }
                } else {
                    // Fallback to just the position if no AABB available
                    let pos = global_transform.translation();
                    *min_bound = min_bound.min(pos);
                    *max_bound = max_bound.max(pos);
                }
            }

            if let Ok(children) = children_query.get(entity) {
                for child in children.iter() {
                    find_meshes_recursive(child, children_query, mesh_query, min_bound, max_bound, found_meshes);
                }
            }
        }

        find_meshes_recursive(scene_entity, &children_query, &mesh_query, &mut min_bound, &mut max_bound, &mut found_meshes);

        if !found_meshes {
            // No meshes found yet, wait more frames
            if state.frames_waited < 30 {
                continue;
            }
            // Give up after 30 frames
            completed.push(path.clone());
            continue;
        }

        // Calculate model center and size
        let center = (min_bound + max_bound) / 2.0;
        let size = max_bound - min_bound;

        // Ensure minimum size to avoid division issues
        let size = Vec3::new(
            size.x.max(0.001),
            size.y.max(0.001),
            size.z.max(0.001),
        );

        // Camera parameters
        let fov_y = 45.0_f32.to_radians();
        let aspect_ratio = 1.0;
        let fov_x = 2.0 * ((fov_y / 2.0).tan() * aspect_ratio).atan();

        // 3/4 view angle
        let view_dir = Vec3::new(0.7, 0.5, 1.0).normalize();

        // Calculate required distance to fit object in view
        // Based on the formula: distance = (size/2) / tan(fov/2)
        // We need to account for the projection of the bounding box onto the view plane

        // Project bounding box size onto the camera's view plane
        // Create orthonormal basis from view direction
        let forward = -view_dir;
        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward).normalize();

        // Calculate the apparent size in view space
        // For a box viewed from an angle, we need to consider all corners
        let half_size = size / 2.0;
        let corners = [
            Vec3::new(-half_size.x, -half_size.y, -half_size.z),
            Vec3::new( half_size.x, -half_size.y, -half_size.z),
            Vec3::new(-half_size.x,  half_size.y, -half_size.z),
            Vec3::new( half_size.x,  half_size.y, -half_size.z),
            Vec3::new(-half_size.x, -half_size.y,  half_size.z),
            Vec3::new( half_size.x, -half_size.y,  half_size.z),
            Vec3::new(-half_size.x,  half_size.y,  half_size.z),
            Vec3::new( half_size.x,  half_size.y,  half_size.z),
        ];

        // Find maximum extent in view-space X and Y
        let mut max_x: f32 = 0.0;
        let mut max_y: f32 = 0.0;
        for corner in corners {
            let view_x = corner.dot(right).abs();
            let view_y = corner.dot(up).abs();
            max_x = max_x.max(view_x);
            max_y = max_y.max(view_y);
        }

        // Also account for depth extent (distance along view direction)
        let mut max_depth: f32 = 0.0;
        for corner in corners {
            let depth = corner.dot(forward).abs();
            max_depth = max_depth.max(depth);
        }

        // Calculate distance needed to fit the object using FOV-based formula
        // distance = half_size / tan(half_fov) + half_depth
        let dist_x = max_x / (fov_x / 2.0).tan() + max_depth;
        let dist_y = max_y / (fov_y / 2.0).tan() + max_depth;

        // Use the larger distance with padding
        let camera_distance = dist_x.max(dist_y) * 1.1;
        let camera_pos = center + view_dir * camera_distance;

        if let Ok(mut camera_transform) = camera_query.get_mut(camera_entity) {
            *camera_transform = Transform::from_translation(camera_pos).looking_at(center, Vec3::Y);
        }

        // Update near/far planes
        if let Ok(mut projection) = projection_query.get_mut(camera_entity) {
            if let Projection::Perspective(ref mut persp) = *projection {
                persp.near = (camera_distance * 0.01).max(0.001);
                persp.far = camera_distance * 10.0;
            }
        }

        // Position checkered ground at bottom of model
        if let Some(ground_entity) = state.ground_entity {
            if let Ok(mut ground_transform) = ground_query.get_mut(ground_entity) {
                let ground_y = min_bound.y - 0.001;
                let model_footprint = size.x.max(size.z);
                // Scale tiles so model sits on ~4 tiles
                let tile_scale = (model_footprint / 4.0).max(0.1);
                *ground_transform = Transform::from_translation(Vec3::new(center.x, ground_y, center.z))
                    .with_scale(Vec3::splat(tile_scale));
            }
        }

        // Mark camera as positioned - now wait for render
        state.camera_positioned = true;
    }

    // Move completed previews to the textures map and cleanup entities
    for path in completed {
        if let Some(state) = preview_cache.processing.remove(&path) {
            // Spawn readback entity with observer to save to disk cache
            let cache_path = get_cache_path(&path);
            commands.spawn(Readback::texture(state.texture_handle.clone()))
                .observe(move |trigger: On<bevy::render::gpu_readback::ReadbackComplete>, mut commands: Commands| {
                    let data = &trigger.event().data;
                    if let Err(e) = save_thumbnail_data_to_cache(data, MODEL_PREVIEW_SIZE, MODEL_PREVIEW_SIZE, &cache_path) {
                        warn!("Failed to cache thumbnail: {}", e);
                    }
                    // Despawn self after saving (observer entity is the entity we spawned with Readback)
                    commands.entity(trigger.observer()).despawn();
                });

            // Store the texture handle
            preview_cache.textures.insert(path.clone(), state.texture_handle);

            // Despawn preview entities
            if let Some(entity) = state.scene_entity {
                commands.entity(entity).despawn();
            }
            if let Some(entity) = state.camera_entity {
                commands.entity(entity).despawn();
            }
            if let Some(entity) = state.light_entity {
                commands.entity(entity).despawn();
            }
            if let Some(entity) = state.ground_entity {
                commands.entity(entity).despawn();
            }
        }
    }

    // Also clean up any orphaned preview lights and grounds
    // (they're spawned separately and need to be tracked)
}

/// System that registers completed model preview textures with egui
pub fn register_model_preview_textures(
    mut contexts: EguiContexts,
    mut preview_cache: ResMut<ModelPreviewCache>,
    images: Res<Assets<Image>>,
) {
    use bevy_egui::EguiTextureHandle;

    let Ok(_ctx) = contexts.ctx_mut() else {
        return;
    };

    // Find textures that need to be registered with egui
    let to_register: Vec<(PathBuf, Handle<Image>)> = preview_cache
        .textures
        .iter()
        .filter(|(path, _)| !preview_cache.texture_ids.contains_key(*path))
        .map(|(path, handle)| (path.clone(), handle.clone()))
        .collect();

    for (path, handle) in to_register {
        // Verify the image exists
        if images.contains(&handle) {
            let texture_id = contexts.add_image(EguiTextureHandle::Weak(handle.id()));
            preview_cache.texture_ids.insert(path, texture_id);
        }
    }
}

/// Cleanup system for model preview entities when they're no longer needed
pub fn cleanup_orphaned_preview_entities(
    mut commands: Commands,
    preview_cache: Res<ModelPreviewCache>,
    lights: Query<(Entity, &ModelPreviewLight)>,
    grounds: Query<(Entity, &ModelPreviewGround)>,
) {
    for (entity, light) in lights.iter() {
        if !preview_cache.processing.contains_key(&light.model_path) {
            commands.entity(entity).despawn();
        }
    }
    for (entity, ground) in grounds.iter() {
        if !preview_cache.processing.contains_key(&ground.model_path) {
            commands.entity(entity).despawn();
        }
    }
}
