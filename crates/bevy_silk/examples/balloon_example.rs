use bevy::{
    color::palettes::css::{BLUE, GREEN, RED, YELLOW},
    prelude::*,
};
use bevy_inspector_egui::{
    bevy_egui::EguiPlugin,
    quick::{ResourceInspectorPlugin, WorldInspectorPlugin},
};
use bevy_silk::prelude::*;

mod camera_plugin;

fn main() {
    App::new()
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 100.0,
            ..default()
        })
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        .add_plugins(WorldInspectorPlugin::default())
        .add_plugins(ResourceInspectorPlugin::<ClothConfig>::new())
        .add_plugins(camera_plugin::CameraPlugin)
        .add_plugins(ClothPlugin)
        .insert_resource(ClothConfig {
            friction: 0.1,
            ..Default::default()
        })
        .add_systems(Startup, (spawn_cloth, setup))
        .run();
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    commands.spawn(DirectionalLight::default());
    let mesh_handle = meshes.add(Cuboid::default());
    for (color, [x, z]) in [
        (Color::from(BLUE), [-10.0, 0.0]),
        (Color::from(GREEN), [10.0, 0.0]),
        (Color::from(YELLOW), [0.0, -10.0]),
        (Color::from(RED), [0.0, 10.0]),
    ] {
        commands.spawn((
            Mesh3d(mesh_handle.clone()),
            Transform::from_xyz(x, 0.0, z),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                double_sided: true,
                ..Default::default()
            })),
        ));
    }
}

fn spawn_cloth(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(5.).mesh().ico(10).unwrap())),
        MeshMaterial3d(materials.add(Color::from(YELLOW))),
        Transform::from_xyz(0.0, 2.0, 0.0),
        ClothBuilder::new().with_pinned_vertex_ids(0..=0),
        Name::new("Balloon"),
    ));
}
