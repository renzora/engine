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

#[derive(Debug, Clone, Reflect, Resource)]
struct MovingAnimation {
    pub base_entity: Option<Entity>,
    pub rotation_speed: f32,
}

impl Default for MovingAnimation {
    fn default() -> Self {
        Self {
            base_entity: None,
            rotation_speed: 1.0,
        }
    }
}

fn main() {
    App::new()
        .register_type::<MovingAnimation>()
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 500.0,
            ..default()
        })
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        .add_plugins(WorldInspectorPlugin::default())
        .add_plugins(ResourceInspectorPlugin::<ClothConfig>::new())
        .add_plugins(ResourceInspectorPlugin::<MovingAnimation>::new())
        .add_plugins(camera_plugin::CameraPlugin)
        .add_plugins(ClothPlugin)
        .add_systems(Startup, (spawn_cloth, setup))
        .add_systems(Update, animate_cube)
        .run();
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let mesh_handle = meshes.add(Cuboid::default());
    for (color, [x, z]) in [
        (Color::from(GREEN), [10.0, 0.0]),
        (Color::from(BLUE), [-10.0, 0.0]),
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
    asset_server: Res<AssetServer>,
) {
    let flag_texture = asset_server.load("France.png");
    let (size_x, size_y) = (20, 40);
    let mesh = rectangle_mesh((size_x, size_y), (Vec3::X * 0.1, -Vec3::Y * 0.1), Vec3::Z);
    let cloth = ClothBuilder::new().with_pinned_vertex_ids(0..size_x);
    let base_entity = Some(
        commands
            .spawn((
                Transform::from_xyz(0.0, 3.0, 0.0),
                Visibility::default(),
                Name::new("Cloth Controller"),
            ))
            .with_children(|b| {
                b.spawn((
                    Mesh3d(meshes.add(Cuboid::new(2.0, 2.0, 2.0))),
                    MeshMaterial3d(materials.add(Color::WHITE)),
                    Transform::from_xyz(10.0, 0.0, 0.0),
                    Name::new("Cube"),
                ))
                .with_children(|b2| {
                    b2.spawn((
                        Mesh3d(meshes.add(mesh)),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color_texture: Some(flag_texture),
                            cull_mode: None, // Option required to render back faces correctly
                            double_sided: true, /* Option required to render back faces
                                              * correctly */
                            ..Default::default()
                        })),
                        Transform::from_xyz(-1.0, 1.0, 1.01),
                        cloth,
                        Name::new("Cloth"),
                    ));
                });
            })
            .id(),
    );
    commands.insert_resource(MovingAnimation {
        base_entity,
        ..Default::default()
    });
}

fn animate_cube(
    animation: Res<MovingAnimation>,
    mut query: Query<&mut Transform>,
    time: Res<Time>,
) {
    let delta_time = time.delta_secs();
    let mut base_transform = query.get_mut(animation.base_entity.unwrap()).unwrap();
    base_transform.rotate(Quat::from_rotation_y(delta_time * animation.rotation_speed));
    base_transform.translation.y = 3.0 + (time.elapsed_secs() * 3.0).sin() * 2.0;
}
