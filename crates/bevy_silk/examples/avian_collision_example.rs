use std::time::Duration;

use avian3d::prelude::*;
use bevy::{
    color::palettes::css::{BLUE, GREEN, RED, YELLOW},
    prelude::*,
    time::common_conditions::on_timer,
};
use bevy_inspector_egui::{
    bevy_egui::EguiPlugin,
    quick::{ResourceInspectorPlugin, WorldInspectorPlugin},
};
use bevy_silk::prelude::*;
use rand::{thread_rng, Rng};

mod camera_plugin;

#[derive(Debug, Resource)]
struct ClothMovement {
    sign: f32,
    t: f32,
}

fn main() {
    App::new()
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 100.0,
            ..default()
        })
        .add_plugins(DefaultPlugins)
        .add_plugins(PhysicsPlugins::default())
        .add_plugins(EguiPlugin::default())
        .add_plugins(ResourceInspectorPlugin::<ClothConfig>::new())
        .add_plugins(WorldInspectorPlugin::default())
        .add_plugins(ClothPlugin)
        .add_plugins(camera_plugin::CameraPlugin)
        .insert_resource(ClothMovement { sign: -1.0, t: 0.0 })
        .add_systems(Startup, (spawn_cloth, setup))
        .add_systems(
            Update,
            (
                shoot_balls.run_if(on_timer(Duration::from_secs(6))),
                move_cloth,
            ),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    commands.spawn((
        DirectionalLight::default(),
        Transform::from_rotation(Quat::from_rotation_y(5.0)),
    ));
    let mesh_handle = meshes.add(Cuboid::new(2.0, 2.0, 2.0));
    for (color, [x, z]) in [
        (Color::from(BLUE), [-10.0, 0.0]),
        (Color::from(GREEN), [10.0, 0.0]),
        (Color::from(YELLOW), [0.0, -10.0]),
        (Color::from(RED), [0.0, 10.0]),
    ] {
        commands.spawn((
            Mesh3d(mesh_handle.clone()),
            Transform::from_xyz(x, 1.0, z),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                double_sided: true,
                ..Default::default()
            })),
            Collider::cuboid(2.0, 2.0, 2.0),
            RigidBody::Static,
        ));
    }
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(24.0, 24.0, 24.0))),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Transform::from_xyz(0.0, -12.0, 0.0),
        Collider::cuboid(24.0, 24.0, 24.0),
        RigidBody::Static,
    ));
}

fn spawn_cloth(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
) {
    let flag_texture = asset_server.load("Bevy.png");
    let (size_x, size_y) = (120, 80);
    let mesh = rectangle_mesh(
        (size_x, size_y),
        (-Vec3::X * 0.25, -Vec3::Y * 0.25),
        Vec3::Z,
    );
    let cloth = ClothBuilder::new()
        .with_pinned_vertex_ids(0..size_x)
        .with_stick_generation(StickGeneration::Triangles);
    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(flag_texture),
            cull_mode: None,    // Option required to render back faces correctly
            double_sided: true, // Option required to render back faces correctly
            ..Default::default()
        })),
        Transform::from_xyz(15.0, 15.0, 15.0),
        cloth,
        ClothCollider {
            dampen_others: Some(0.02),
            ..Default::default()
        },
        Name::new("Cloth"),
    ));
}

fn move_cloth(
    time: Res<Time>,
    mut query: Query<&mut Transform, With<ClothBuilder>>,
    mut movement: ResMut<ClothMovement>,
) {
    let delta_time = time.delta_secs();
    for mut transform in query.iter_mut() {
        movement.t += delta_time * 2.0;
        transform.translation.z += movement.sign * delta_time * 2.0;
        if movement.t > 30.0 {
            movement.t = 0.0;
            movement.sign = -movement.sign;
        }
    }
}

fn shoot_balls(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut rng = thread_rng();
    let radius = rng.gen_range(1.0..3.0);
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(radius).mesh().ico(5).unwrap())),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Transform::from_xyz(0.0, 0.0, -20.0),
        LinearVelocity(Vec3::new(
            rng.gen_range(-5.0..5.0),
            rng.gen_range(10.0..15.0),
            rng.gen_range(20.0..30.0),
        )),
        RigidBody::Dynamic,
        Collider::sphere(radius),
        Name::new("Ball"),
    ));
}
