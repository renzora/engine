use std::f32::consts::{PI, TAU};

use bevy::prelude::*;

use bevy_mod_outline::*;

#[bevy_main]
fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins((DefaultPlugins, OutlinePlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, (wobble, orbit))
        .run();
}

#[derive(Component)]
struct Wobbles;

#[derive(Component)]
struct Orbits;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Add cube with generated outline normals
    let mut cube_mesh = Cuboid::new(1.0, 1.0, 1.0).mesh().build();
    cube_mesh
        .generate_outline_normals(&GenerateOutlineNormalsSettings::default())
        .unwrap();
    commands.spawn((
        Mesh3d(meshes.add(cube_mesh)),
        MeshMaterial3d(materials.add(StandardMaterial::from(Color::srgb(0.1, 0.1, 0.9)))),
        Transform::from_xyz(0.0, 1.0, 0.0),
        OutlineVolume {
            visible: true,
            colour: Color::srgb(0.0, 1.0, 0.0),
            width: 25.0,
        },
        OutlineMode::FloodFlat,
        Wobbles,
    ));

    // Add torus using the regular surface normals for outlining
    commands.spawn((
        Mesh3d(
            meshes.add(
                Torus {
                    minor_radius: 0.1,
                    major_radius: 0.3,
                }
                .mesh()
                .minor_resolution(10)
                .major_resolution(20)
                .build(),
            ),
        ),
        MeshMaterial3d(materials.add(StandardMaterial::from(Color::srgb(0.9, 0.1, 0.1)))),
        Transform::from_xyz(0.0, 1.2, 2.0).with_rotation(Quat::from_rotation_x(0.5 * PI)),
        OutlineVolume {
            visible: true,
            colour: Color::srgba(1.0, 0.0, 1.0, 0.3),
            width: 15.0,
        },
        Orbits,
    ));

    // Add plane, light source, and camera
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::new(5.0, 5.0)).mesh().build())),
        MeshMaterial3d(materials.add(StandardMaterial::from(Color::srgb(0.3, 0.5, 0.3)))),
    ));
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        Msaa::Sample4,
    ));
}

fn wobble(mut query: Query<&mut Transform, With<Wobbles>>, timer: Res<Time>, mut t: Local<f32>) {
    let ta = *t;
    *t = (ta + 0.5 * timer.delta_secs()) % TAU;
    let tb = *t;
    let i1 = tb.cos() - ta.cos();
    let i2 = ta.sin() - tb.sin();
    for mut transform in query.iter_mut() {
        transform.rotate(Quat::from_rotation_z(TAU * 20.0 * i1 * timer.delta_secs()));
        transform.rotate(Quat::from_rotation_y(TAU * 20.0 * i2 * timer.delta_secs()));
    }
}

fn orbit(mut query: Query<&mut Transform, With<Orbits>>, timer: Res<Time>) {
    for mut transform in query.iter_mut() {
        transform.translate_around(Vec3::ZERO, Quat::from_rotation_y(0.4 * timer.delta_secs()))
    }
}
