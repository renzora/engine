//! Studio Preview system - isolated 3D view with professional lighting
//!
//! Provides a separate render viewport with:
//! - Its own camera and render target
//! - Professional 3-point studio lighting
//! - Orbit controls for viewing selected objects
//! - Independent from main scene lighting/camera

use bevy::prelude::*;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy_egui::egui::TextureId;
use bevy_egui::{EguiContexts, EguiTextureHandle};

use crate::scene::EditorOnly;
use crate::core::{AppState, SelectionState, ViewportState};

/// Render layer for studio preview (isolated from main scene)
pub const STUDIO_RENDER_LAYER: usize = 5;

/// Resource holding the studio preview render texture
#[derive(Resource)]
pub struct StudioPreviewImage {
    pub handle: Handle<Image>,
    pub texture_id: Option<TextureId>,
    pub size: (u32, u32),
}

impl Default for StudioPreviewImage {
    fn default() -> Self {
        Self {
            handle: Handle::default(),
            texture_id: None,
            size: (512, 512),
        }
    }
}

/// Marker component for the studio preview camera
#[derive(Component)]
pub struct StudioPreviewCamera;

/// Marker component for studio preview lights
#[derive(Component)]
pub struct StudioPreviewLight;

/// Marker component for studio preview test geometry
#[derive(Component)]
pub struct StudioPreviewGeometry;

/// Marker component for entities cloned into studio preview
#[derive(Component)]
pub struct StudioPreviewClone {
    /// The original entity this was cloned from
    pub source: Entity,
}

/// State for studio preview orbit camera
#[derive(Resource)]
pub struct StudioPreviewOrbit {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: Vec3,
    /// Whether the panel is being interacted with
    pub is_active: bool,
    /// Auto-rotate when not interacting
    pub auto_rotate: bool,
    pub auto_rotate_speed: f32,
}

impl Default for StudioPreviewOrbit {
    fn default() -> Self {
        Self {
            yaw: 0.5,
            pitch: 0.3,
            distance: 3.0,
            target: Vec3::ZERO,
            is_active: false,
            auto_rotate: true,
            auto_rotate_speed: 0.2,
        }
    }
}

/// Tracks which entity is currently being shown in the studio preview
#[derive(Resource, Default)]
pub struct StudioPreviewSelection {
    /// The entity currently being previewed (from main scene)
    pub current_entity: Option<Entity>,
}

/// Sets up the studio preview render texture and camera
pub fn setup_studio_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut preview_image: ResMut<StudioPreviewImage>,
) {
    let size = Extent3d {
        width: 512,
        height: 512,
        depth_or_array_layers: 1,
    };

    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("studio_preview_texture"),
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bgra8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
        ..default()
    };
    image.resize(size);

    let image_handle = images.add(image);

    // Update the existing resource with the actual handle
    preview_image.handle = image_handle.clone();
    preview_image.size = (512, 512);

    // Spawn the studio preview camera
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.12, 0.12, 0.14)),
            order: -2, // Render before main camera
            ..default()
        },
        RenderTarget::Image(image_handle.into()),
        Transform::from_xyz(0.0, 1.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(STUDIO_RENDER_LAYER),
        StudioPreviewCamera,
        EditorOnly,
        Name::new("Studio Preview Camera"),
    ));

    // Spawn studio lights (3-point lighting)
    // Key light - main light from upper-right
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.98, 0.95),
            illuminance: 15000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.7, 0.5, 0.0)),
        RenderLayers::layer(STUDIO_RENDER_LAYER),
        StudioPreviewLight,
        EditorOnly,
        Name::new("Studio Key Light"),
    ));

    // Fill light - softer from left
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.85, 0.9, 1.0),
            illuminance: 6000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.2, -0.9, 0.0)),
        RenderLayers::layer(STUDIO_RENDER_LAYER),
        StudioPreviewLight,
        EditorOnly,
        Name::new("Studio Fill Light"),
    ));

    // Rim light - from behind
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 1.0, 1.0),
            illuminance: 8000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.3, 3.14, 0.0)),
        RenderLayers::layer(STUDIO_RENDER_LAYER),
        StudioPreviewLight,
        EditorOnly,
        Name::new("Studio Rim Light"),
    ));

    // Top light
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.95, 0.95, 1.0),
            illuminance: 4000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -1.5, 0.0, 0.0)),
        RenderLayers::layer(STUDIO_RENDER_LAYER),
        StudioPreviewLight,
        EditorOnly,
        Name::new("Studio Top Light"),
    ));

    // Ground plane for studio
    let ground_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.25, 0.28),
        perceptual_roughness: 0.95,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(10.0)))),
        MeshMaterial3d(ground_material),
        Transform::from_xyz(0.0, 0.0, 0.0),
        RenderLayers::layer(STUDIO_RENDER_LAYER),
        StudioPreviewGeometry,
        EditorOnly,
        Name::new("Studio Ground"),
    ));

    info!("Studio preview system initialized");
}

/// Register the studio preview texture with egui and sync to ViewportState
pub fn register_studio_preview_texture(
    mut contexts: EguiContexts,
    mut preview_image: ResMut<StudioPreviewImage>,
    viewport_state: Option<ResMut<ViewportState>>,
) {
    // Only register once we have a valid handle (set by setup_studio_preview)
    if preview_image.texture_id.is_none() && preview_image.handle != Handle::default() {
        let texture_id = contexts.add_image(EguiTextureHandle::Weak(preview_image.handle.id()));
        preview_image.texture_id = Some(texture_id);
    }

    // Sync texture_id to ViewportState for UI access (if available)
    if let Some(mut viewport) = viewport_state {
        if viewport.studio_preview_texture_id != preview_image.texture_id {
            viewport.studio_preview_texture_id = preview_image.texture_id;
            viewport.studio_preview_size = preview_image.size;
        }
    }
}

/// Update the studio preview camera based on orbit controls
pub fn update_studio_preview_camera(
    time: Res<Time>,
    mut orbit: ResMut<StudioPreviewOrbit>,
    mut camera: Query<&mut Transform, With<StudioPreviewCamera>>,
) {
    // Auto-rotate when not being interacted with
    if orbit.auto_rotate && !orbit.is_active {
        orbit.yaw += orbit.auto_rotate_speed * time.delta_secs();
    }

    // Update camera transform
    for mut transform in camera.iter_mut() {
        let x = orbit.distance * orbit.pitch.cos() * orbit.yaw.sin();
        let y = orbit.distance * orbit.pitch.sin();
        let z = orbit.distance * orbit.pitch.cos() * orbit.yaw.cos();

        transform.translation = orbit.target + Vec3::new(x, y, z);
        transform.look_at(orbit.target, Vec3::Y);
    }
}

/// Sync the selected entity from the hierarchy to the studio preview
pub fn sync_selection_to_preview(
    mut commands: Commands,
    selection: Res<SelectionState>,
    mut preview_selection: ResMut<StudioPreviewSelection>,
    mut orbit: ResMut<StudioPreviewOrbit>,
    existing_clones: Query<Entity, With<StudioPreviewClone>>,
    // Query for meshes on the selected entity and its children
    mesh_query: Query<(
        &Mesh3d,
        Option<&MeshMaterial3d<StandardMaterial>>,
        &GlobalTransform,
    )>,
    // Query for Aabb component (computed by Bevy)
    aabb_query: Query<&bevy::camera::primitives::Aabb>,
    // Query for skinned meshes
    skinned_query: Query<&SkinnedMesh>,
    // Query for transforms (needed for cloning joints)
    transform_query: Query<(&Transform, &GlobalTransform)>,
    children_query: Query<&Children>,
) {
    // Check if selection changed
    if selection.selected_entity == preview_selection.current_entity {
        return;
    }

    // Update tracked selection
    preview_selection.current_entity = selection.selected_entity;

    // Despawn all existing preview clones
    for entity in existing_clones.iter() {
        commands.entity(entity).despawn();
    }

    // If nothing selected, we're done
    let Some(selected) = selection.selected_entity else {
        return;
    };

    // Collect all entities to check (selected + all descendants)
    let mut entities_to_check = vec![selected];
    let mut i = 0;
    while i < entities_to_check.len() {
        let entity = entities_to_check[i];
        if let Ok(children) = children_query.get(entity) {
            for child in children.iter() {
                entities_to_check.push(child);
            }
        }
        i += 1;
    }

    // Calculate bounds of all meshes (including skinned meshes)
    let mut min_bounds = Vec3::splat(f32::MAX);
    let mut max_bounds = Vec3::splat(f32::MIN);
    let mut has_meshes = false;

    for entity in &entities_to_check {
        if let Ok((_, _, global_transform)) = mesh_query.get(*entity) {
            // Use Aabb component if available, otherwise use a default size
            let (center, half_extents) = if let Ok(aabb) = aabb_query.get(*entity) {
                (Vec3::from(aabb.center), Vec3::from(aabb.half_extents))
            } else {
                // Default bounding box for meshes without Aabb
                (Vec3::ZERO, Vec3::splat(0.5))
            };

            let world_center = global_transform.transform_point(center);
            let scale = global_transform.compute_transform().scale;
            let scaled_half = half_extents * scale;

            min_bounds = min_bounds.min(world_center - scaled_half);
            max_bounds = max_bounds.max(world_center + scaled_half);
            has_meshes = true;
        }
    }

    if !has_meshes {
        return;
    }

    // Calculate center and size of bounds
    let bounds_center = (min_bounds + max_bounds) * 0.5;
    let bounds_size = max_bounds - min_bounds;
    let max_dimension = bounds_size.x.max(bounds_size.y).max(bounds_size.z);
    let ground_offset = min_bounds.y - bounds_center.y;

    // Collect all joint entities that need to be cloned for skinned meshes
    let mut joint_mapping: std::collections::HashMap<Entity, Entity> = std::collections::HashMap::new();

    for entity in &entities_to_check {
        if let Ok(skinned_mesh) = skinned_query.get(*entity) {
            for &joint_entity in &skinned_mesh.joints {
                if !joint_mapping.contains_key(&joint_entity) {
                    // Clone the joint entity
                    if let Ok((transform, _global_transform)) = transform_query.get(joint_entity) {
                        let cloned_joint = commands.spawn((
                            *transform,
                            GlobalTransform::default(),
                            RenderLayers::layer(STUDIO_RENDER_LAYER),
                            StudioPreviewClone { source: joint_entity },
                            EditorOnly,
                            Name::new("Studio Joint Clone"),
                        )).id();
                        joint_mapping.insert(joint_entity, cloned_joint);
                    }
                }
            }
        }
    }

    // Clone meshes to studio preview layer, centered at origin
    for entity in &entities_to_check {
        if let Ok((mesh3d, material, global_transform)) = mesh_query.get(*entity) {
            // Calculate local position relative to bounds center, then offset so model sits on ground
            let world_pos = global_transform.translation();
            let local_pos = world_pos - bounds_center;
            let preview_pos = Vec3::new(local_pos.x, local_pos.y - ground_offset, local_pos.z);

            let mut entity_commands = commands.spawn((
                Mesh3d(mesh3d.0.clone()),
                Transform::from_translation(preview_pos)
                    .with_rotation(global_transform.compute_transform().rotation)
                    .with_scale(global_transform.compute_transform().scale),
                RenderLayers::layer(STUDIO_RENDER_LAYER),
                StudioPreviewClone { source: *entity },
                EditorOnly,
                Name::new("Studio Preview Clone"),
            ));

            // Add material if present
            if let Some(mat) = material {
                entity_commands.insert(MeshMaterial3d(mat.0.clone()));
            }

            // Handle skinned mesh - clone with remapped joints
            if let Ok(skinned_mesh) = skinned_query.get(*entity) {
                let new_joints: Vec<Entity> = skinned_mesh.joints
                    .iter()
                    .map(|&old_joint| {
                        *joint_mapping.get(&old_joint).unwrap_or(&old_joint)
                    })
                    .collect();

                entity_commands.insert(SkinnedMesh {
                    inverse_bindposes: skinned_mesh.inverse_bindposes.clone(),
                    joints: new_joints,
                });
            }
        }
    }

    // Adjust camera distance based on model size
    let camera_distance = (max_dimension * 1.5).max(2.0);
    orbit.distance = camera_distance;
    orbit.target = Vec3::new(0.0, bounds_size.y * 0.4, 0.0); // Look at center-ish of model
    orbit.pitch = 0.3; // Reset pitch for new model
}

/// Sync cloned joint transforms from the original joints each frame (for animation)
pub fn sync_preview_joint_transforms(
    mut clones: Query<(&StudioPreviewClone, &mut Transform), Without<Mesh3d>>,
    source_transforms: Query<&Transform, Without<StudioPreviewClone>>,
) {
    for (clone, mut transform) in clones.iter_mut() {
        if let Ok(source_transform) = source_transforms.get(clone.source) {
            *transform = *source_transform;
        }
    }
}

/// Plugin for the studio preview system
pub struct StudioPreviewPlugin;

impl Plugin for StudioPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StudioPreviewOrbit>();
        app.init_resource::<StudioPreviewSelection>();
        // Initialize with default so UI can access it before setup runs
        app.init_resource::<StudioPreviewImage>();
        // Setup when entering Editor state (not during splash)
        app.add_systems(OnEnter(AppState::Editor), setup_studio_preview);
        // Only run these systems in Editor state
        app.add_systems(Update, (
            register_studio_preview_texture,
            update_studio_preview_camera,
            sync_selection_to_preview,
            sync_preview_joint_transforms,
        ).run_if(in_state(AppState::Editor)));
    }
}
