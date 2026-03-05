//! Runtime camera spawning, render target syncing, and test scene.

use bevy::prelude::*;
use bevy::camera::RenderTarget;

use crate::{RuntimeCamera, ViewportRenderTarget};

/// Spawns the main 3D game camera.
///
/// If `ViewportRenderTarget` already has an image (editor mode),
/// the camera renders to it. Otherwise it renders to the window.
pub fn spawn_runtime_camera(mut commands: Commands, render_target: Res<ViewportRenderTarget>) {
    let mut entity = commands.spawn((
        Camera3d::default(),
        Camera {
            order: -1,
            ..default()
        },
        Transform::from_xyz(5.0, 4.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        RuntimeCamera,
        Name::new("Runtime Camera"),
    ));

    if let Some(ref image) = render_target.image {
        entity.insert(RenderTarget::Image(image.clone().into()));
    }
}

/// Watches for changes to `ViewportRenderTarget` and updates the camera accordingly.
///
/// Only acts when an image handle is set (editor mode). When `None`, the camera
/// keeps its default window target — we never remove `RenderTarget`.
pub fn sync_camera_render_target(
    render_target: Res<ViewportRenderTarget>,
    cameras: Query<Entity, With<RuntimeCamera>>,
    mut commands: Commands,
) {
    if !render_target.is_changed() {
        return;
    }

    if let Some(ref image) = render_target.image {
        for entity in &cameras {
            commands
                .entity(entity)
                .insert(RenderTarget::Image(image.clone().into()));
        }
    }
}

/// Spawns a simple test scene: ground plane, cube, and lights.
pub fn spawn_test_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Ambient light so the scene is never pitch-black
    commands.spawn(AmbientLight {
        color: Color::WHITE,
        brightness: 300.0,
        affects_lightmapped_meshes: true,
    });

    // Ground plane
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(5.0)).mesh())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.35, 0.35),
            ..default()
        })),
        Transform::default(),
        Name::new("Ground"),
    ));

    // Cube
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0).mesh())),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.8, 0.3, 0.2),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.5, 0.0),
        Name::new("Cube"),
    ));

    // Directional light (sun)
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, 0.4, 0.0)),
        Name::new("Sun"),
    ));
}
