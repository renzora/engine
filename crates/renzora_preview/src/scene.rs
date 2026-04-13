//! Shared preview scene — camera, lighting, orbit controls, mesh primitives.

use bevy::prelude::*;
use crate::bridge::{PreviewCommand, PreviewCommandQueue};

// ── Components ──────────────────────────────────────────────────────────────

#[derive(Component)]
pub struct PreviewCamera;

#[derive(Component)]
pub struct PreviewSubject;

#[derive(Component)]
pub struct PreviewGround;

/// Orbit camera state.
#[derive(Resource)]
pub struct OrbitState {
    pub azimuth: f32,
    pub elevation: f32,
    pub distance: f32,
    pub auto_rotate: f32,
}

impl Default for OrbitState {
    fn default() -> Self {
        Self {
            azimuth: 0.0,
            elevation: 0.4,
            distance: 3.0,
            auto_rotate: 0.3,
        }
    }
}

// ── Systems ─────────────────────────────────────────────────────────────────

pub fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 1.5, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
        PreviewCamera,
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, 0.4, 0.0)),
    ));

    commands.spawn((
        PointLight {
            color: Color::srgb(0.6, 0.7, 1.0),
            intensity: 3_000.0,
            range: 20.0,
            ..default()
        },
        Transform::from_xyz(-3.0, 2.0, -1.0),
    ));

    // Grid floor — generated procedurally as thin line meshes
    let grid_size = 20;
    let grid_spacing = 0.5;
    let half = grid_size as f32 * grid_spacing * 0.5;
    let grid_color = Color::srgba(0.2, 0.2, 0.25, 0.3);

    for i in 0..=grid_size {
        let offset = i as f32 * grid_spacing - half;

        // Line along X axis
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(half * 2.0, 0.003, 0.003))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: grid_color,
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..default()
            })),
            Transform::from_xyz(0.0, -1.0, offset),
            PreviewGround,
        ));

        // Line along Z axis
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.003, 0.003, half * 2.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: grid_color,
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..default()
            })),
            Transform::from_xyz(offset, -1.0, 0.0),
            PreviewGround,
        ));
    }

    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.2, 1.2, 1.2))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.7, 0.7, 0.7),
            ..default()
        })),
        Transform::IDENTITY,
        PreviewSubject,
    ));
}

pub fn update_orbit_camera(
    time: Res<Time>,
    mut orbit: ResMut<OrbitState>,
    mut camera_q: Query<&mut Transform, With<PreviewCamera>>,
) {
    if orbit.auto_rotate != 0.0 {
        orbit.azimuth += orbit.auto_rotate * time.delta_secs();
    }

    let Ok(mut transform) = camera_q.single_mut() else { return };

    let x = orbit.distance * orbit.elevation.cos() * orbit.azimuth.sin();
    let y = orbit.distance * orbit.elevation.sin();
    let z = orbit.distance * orbit.elevation.cos() * orbit.azimuth.cos();

    transform.translation = Vec3::new(x, y, z);
    transform.look_at(Vec3::ZERO, Vec3::Y);
}

pub fn handle_scene_commands(
    mut queue: ResMut<PreviewCommandQueue>,
    mut orbit: ResMut<OrbitState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut subject_q: Query<&mut Mesh3d, With<PreviewSubject>>,
) {
    // Process camera orbit and mesh commands, leave others for mode systems
    let mut remaining = Vec::new();

    for cmd in queue.commands.drain(..) {
        match cmd {
            PreviewCommand::CameraOrbit(c) => {
                orbit.azimuth = c.azimuth;
                orbit.elevation = c.elevation;
                orbit.distance = c.distance;
                orbit.auto_rotate = 0.0;
            }
            PreviewCommand::SetMesh(c) => {
                let Ok(mut mesh_handle) = subject_q.single_mut() else { continue };
                let new_mesh = match c.shape.as_str() {
                    "sphere" => Sphere::new(0.8).mesh().ico(5).unwrap(),
                    "cube" => Cuboid::new(1.2, 1.2, 1.2).into(),
                    "plane" => Plane3d::new(Vec3::Y, Vec2::splat(1.5)).into(),
                    "cylinder" => Cylinder::new(0.6, 1.5).into(),
                    "torus" => Torus::new(0.4, 0.8).into(),
                    _ => continue,
                };
                mesh_handle.0 = meshes.add(new_mesh);
            }
            other => remaining.push(other),
        }
    }

    queue.commands = remaining;
}

// ── Plugin ──────────────────────────────────────────────────────────────────

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OrbitState>()
            .add_systems(Startup, setup_scene)
            .add_systems(Update, update_orbit_camera)
            .add_systems(Update, handle_scene_commands);
    }
}
