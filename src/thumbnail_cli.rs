//! Headless thumbnail renderer for model preview generation
//!
//! This module provides a CLI mode for rendering model thumbnails without
//! the full editor UI. It's spawned as a subprocess by the main editor
//! to avoid blocking the viewport rendering.

use bevy::prelude::*;
use bevy::asset::UnapprovedPathMode;
use bevy::camera::RenderTarget;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::render::gpu_readback::Readback;
use bevy::scene::SceneRoot;
use bevy::window::WindowMode;
use bevy::camera::primitives::Aabb;
use std::path::PathBuf;

const THUMBNAIL_SIZE: u32 = 128;

/// Run the headless thumbnail renderer
pub fn run_thumbnail_renderer(model_path: String, output_path: String) {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Thumbnail Renderer".to_string(),
                        resolution: (THUMBNAIL_SIZE, THUMBNAIL_SIZE).into(),
                        visible: false,
                        mode: WindowMode::Windowed,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    unapproved_path_mode: UnapprovedPathMode::Allow,
                    ..default()
                })
        )
        .insert_resource(ThumbnailTask {
            model_path: PathBuf::from(model_path),
            output_path: PathBuf::from(output_path),
            state: ThumbnailState::Loading,
            gltf_handle: None,
            scene_entity: None,
            camera_entity: None,
            texture_handle: None,
            frames_waited: 0,
        })
        .add_systems(Startup, setup_thumbnail_scene)
        .add_systems(Update, (
            wait_for_model_load,
            position_camera_and_capture,
            request_readback,
            handle_readback_complete,
        ).chain())
        .run();
}

#[derive(Resource)]
struct ThumbnailTask {
    model_path: PathBuf,
    output_path: PathBuf,
    state: ThumbnailState,
    gltf_handle: Option<Handle<Gltf>>,
    scene_entity: Option<Entity>,
    camera_entity: Option<Entity>,
    texture_handle: Option<Handle<Image>>,
    frames_waited: u32,
}

#[derive(Clone, Copy, PartialEq)]
enum ThumbnailState {
    Loading,
    WaitingForScene,
    WaitingForRender,  // Wait for GPU to render the frame
    #[allow(dead_code)]
    RequestingReadback,
    WaitingForReadback,
}

fn setup_thumbnail_scene(
    mut commands: Commands,
    mut task: ResMut<ThumbnailTask>,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
) {
    // Create render texture
    let size = Extent3d {
        width: THUMBNAIL_SIZE,
        height: THUMBNAIL_SIZE,
        depth_or_array_layers: 1,
    };

    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("thumbnail_texture"),
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bgra8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::COPY_SRC
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
        ..default()
    };
    image.resize(size);
    let texture_handle = images.add(image);
    task.texture_handle = Some(texture_handle.clone());

    // Load the model
    let gltf_handle: Handle<Gltf> = asset_server.load(task.model_path.clone());
    task.gltf_handle = Some(gltf_handle);

    // Lighting
    commands.spawn((
        DirectionalLight {
            illuminance: 12000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, 0.5, 0.0)),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 6000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.3, -1.2, 0.0)),
    ));

    // Camera (will be positioned later)
    let camera_entity = commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            ..default()
        },
        RenderTarget::Image(texture_handle.into()),
        Projection::Perspective(PerspectiveProjection {
            fov: 45.0_f32.to_radians(),
            aspect_ratio: 1.0,
            near: 0.01,
            far: 1000.0,
            ..default()
        }),
        Transform::from_translation(Vec3::new(2.0, 2.0, 2.0)).looking_at(Vec3::ZERO, Vec3::Y),
    )).id();

    task.camera_entity = Some(camera_entity);
}

fn wait_for_model_load(
    mut commands: Commands,
    mut task: ResMut<ThumbnailTask>,
    asset_server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
) {
    if task.state != ThumbnailState::Loading {
        return;
    }

    let Some(ref gltf_handle) = task.gltf_handle else {
        return;
    };

    use bevy::asset::LoadState;
    match asset_server.get_load_state(gltf_handle) {
        Some(LoadState::Loaded) => {
            if let Some(gltf) = gltfs.get(gltf_handle) {
                if let Some(scene_handle) = gltf.default_scene.clone().or_else(|| gltf.scenes.first().cloned()) {
                    let scene_entity = commands.spawn((
                        Transform::default(),
                        Visibility::default(),
                        SceneRoot(scene_handle),
                    )).id();
                    task.scene_entity = Some(scene_entity);
                    task.state = ThumbnailState::WaitingForScene;
                } else {
                    eprintln!("No scene in GLTF");
                    std::process::exit(1);
                }
            }
        }
        Some(LoadState::Failed(_)) => {
            eprintln!("Failed to load model");
            std::process::exit(1);
        }
        _ => {}
    }
}

fn position_camera_and_capture(
    _commands: Commands,
    mut task: ResMut<ThumbnailTask>,
    mut camera_query: Query<&mut Transform, With<Camera3d>>,
    mut projection_query: Query<&mut Projection, With<Camera3d>>,
    mesh_query: Query<(&GlobalTransform, Option<&Aabb>), With<Mesh3d>>,
    children_query: Query<&Children>,
) {
    if task.state != ThumbnailState::WaitingForScene {
        return;
    }

    task.frames_waited += 1;
    if task.frames_waited < 10 {
        return;
    }

    let Some(scene_entity) = task.scene_entity else {
        return;
    };

    // Calculate bounding box
    let mut min_bound = Vec3::splat(f32::MAX);
    let mut max_bound = Vec3::splat(f32::MIN);
    let mut found_meshes = false;

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
            if let Some(aabb) = aabb {
                let center = aabb.center;
                let half_extents = aabb.half_extents;
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
                for corner in corners {
                    let world_corner = global_transform.transform_point(corner);
                    *min_bound = min_bound.min(world_corner);
                    *max_bound = max_bound.max(world_corner);
                }
            } else {
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
        if task.frames_waited < 30 {
            return;
        }
        eprintln!("No meshes found in model");
        std::process::exit(1);
    }

    // Position camera
    let center = (min_bound + max_bound) / 2.0;
    let size = (max_bound - min_bound).max(Vec3::splat(0.001));

    let fov_y = 45.0_f32.to_radians();
    let fov_x = 2.0 * ((fov_y / 2.0).tan()).atan();
    let view_dir = Vec3::new(0.7, 0.5, 1.0).normalize();

    let forward = -view_dir;
    let right = forward.cross(Vec3::Y).normalize();
    let up = right.cross(forward).normalize();

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

    let mut max_x: f32 = 0.0;
    let mut max_y: f32 = 0.0;
    let mut max_depth: f32 = 0.0;
    for corner in corners {
        max_x = max_x.max(corner.dot(right).abs());
        max_y = max_y.max(corner.dot(up).abs());
        max_depth = max_depth.max(corner.dot(forward).abs());
    }

    let dist_x = max_x / (fov_x / 2.0).tan() + max_depth;
    let dist_y = max_y / (fov_y / 2.0).tan() + max_depth;
    let camera_distance = dist_x.max(dist_y) * 1.1;
    let camera_pos = center + view_dir * camera_distance;

    if let Ok(mut camera_transform) = camera_query.single_mut() {
        *camera_transform = Transform::from_translation(camera_pos).looking_at(center, Vec3::Y);
    }

    if let Ok(mut projection) = projection_query.single_mut() {
        if let Projection::Perspective(ref mut persp) = *projection {
            persp.near = (camera_distance * 0.01).max(0.001);
            persp.far = camera_distance * 10.0;
        }
    }

    // Wait for render before requesting readback
    task.state = ThumbnailState::WaitingForRender;
    task.frames_waited = 0;
}

fn request_readback(
    mut commands: Commands,
    mut task: ResMut<ThumbnailTask>,
) {
    if task.state != ThumbnailState::WaitingForRender {
        return;
    }

    task.frames_waited += 1;

    // Wait 5 frames for the GPU to render
    if task.frames_waited < 5 {
        return;
    }

    // Request readback
    if let Some(texture_handle) = task.texture_handle.clone() {
        let output_path = task.output_path.clone();
        commands.spawn(Readback::texture(texture_handle))
            .observe(move |trigger: On<bevy::render::gpu_readback::ReadbackComplete>, mut exit: MessageWriter<AppExit>| {
                let data = &trigger.event().data;
                if let Err(e) = save_thumbnail(data, THUMBNAIL_SIZE, THUMBNAIL_SIZE, &output_path) {
                    eprintln!("Failed to save thumbnail: {}", e);
                    exit.write(AppExit::Error(1u8.try_into().unwrap()));
                } else {
                    exit.write(AppExit::Success);
                }
            });
    }

    task.state = ThumbnailState::WaitingForReadback;
    task.frames_waited = 0;
}

// Readback completion is handled by the observer - this system is just a placeholder
fn handle_readback_complete() {
    // The observer on the Readback entity handles exit
}

fn save_thumbnail(data: &[u8], width: u32, height: u32, output_path: &PathBuf) -> Result<(), String> {
    use std::fs;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // Convert BGRA to RGBA
    let mut rgba_data = Vec::with_capacity(data.len());
    for chunk in data.chunks(4) {
        if chunk.len() == 4 {
            rgba_data.push(chunk[2]); // R
            rgba_data.push(chunk[1]); // G
            rgba_data.push(chunk[0]); // B
            rgba_data.push(chunk[3]); // A
        }
    }

    let img = image::RgbaImage::from_raw(width, height, rgba_data)
        .ok_or("Failed to create image buffer")?;

    img.save(output_path).map_err(|e| e.to_string())?;

    Ok(())
}
