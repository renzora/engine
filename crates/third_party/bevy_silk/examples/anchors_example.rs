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
            brightness: 500.0,
            ..default()
        })
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        .add_plugins(ResourceInspectorPlugin::<ClothConfig>::new())
        .add_plugins(WorldInspectorPlugin::default())
        .add_plugins(camera_plugin::CameraPlugin)
        .add_plugins(ClothPlugin)
        .add_systems(Startup, (spawn_cloth, setup))
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
    let mesh_handle = meshes.add(Cuboid::default());
    for (color, [x, z]) in [
        (Color::from(BLUE), [-10.0, 0.0]),
        (Color::from(GREEN), [10.0, 0.0]),
        (Color::from(YELLOW), [0.0, -10.0]),
        (Color::from(RED), [0.0, 10.0]),
    ] {
        commands.spawn((
            Mesh3d(mesh_handle.clone()),
            Transform::from_xyz(x, 1.0, z),
            MeshMaterial3d(materials.add(color)),
        ));
    }
}

fn spawn_cloth(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
) {
    let flag_texture = asset_server.load("Bevy.png");
    let (size_x, size_y) = (60, 40);

    let anchor_mesh = meshes.add(Cuboid::default());
    let entity_a = commands
        .spawn((
            Mesh3d(anchor_mesh.clone()),
            MeshMaterial3d(materials.add(Color::from(RED))),
            Transform::from_xyz(-15.0, 15.0, 15.0),
            Name::new("Anchor RED"),
        ))
        .id();
    let entity_b = commands
        .spawn((
            Mesh3d(anchor_mesh),
            MeshMaterial3d(materials.add(Color::from(GREEN))),
            Transform::from_xyz(15.0, 15.0, 15.0),
            Name::new("Anchor GREEN"),
        ))
        .id();

    let mesh = rectangle_mesh((size_x, size_y), (Vec3::X * 0.5, -Vec3::Y * 0.5), -Vec3::Z);
    let cloth = ClothBuilder::new()
        .with_anchored_vertex_positions(
            |p| p.x <= 0.0,
            VertexAnchor {
                custom_target: Some(entity_a),
                ..Default::default()
            },
        )
        .with_anchored_vertex_positions(
            |p| p.x >= 29.5,
            VertexAnchor {
                custom_target: Some(entity_b),
                custom_offset: Some(-Vec3::X * 30.0),
                ..Default::default()
            },
        );
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
        Name::new("Cloth"),
    ));
}
