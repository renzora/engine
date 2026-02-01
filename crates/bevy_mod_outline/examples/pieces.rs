use std::f32::consts::TAU;

use bevy::prelude::*;

use bevy_mod_outline::*;

#[bevy_main]
fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins((DefaultPlugins, OutlinePlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, rotates)
        .run();
}

#[derive(Component)]
struct Rotates;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Add sphere with child meshes sticking out of it
    commands
        .spawn((
            Mesh3d(meshes.add(Sphere::new(0.75).mesh().uv(30, 30))),
            MeshMaterial3d(materials.add(StandardMaterial::from(Color::srgb(0.9, 0.1, 0.1)))),
            Transform::from_translation(Vec3::new(0.0, 1.0, 0.0)),
            OutlineVolume {
                visible: true,
                colour: Color::WHITE,
                width: 10.0,
            },
            OutlineStencil {
                offset: 5.0,
                ..default()
            },
        ))
        .insert(Rotates)
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(
                    meshes.add(
                        Capsule3d::new(0.2, 1.0)
                            .mesh()
                            .rings(15)
                            .latitudes(15)
                            .longitudes(15)
                            .build(),
                    ),
                ),
                MeshMaterial3d(materials.add(StandardMaterial::from(Color::srgb(0.1, 0.1, 0.9)))),
                Transform::from_rotation(Quat::from_axis_angle(Vec3::X, TAU / 4.0))
                    .with_translation(Vec3::new(0.0, 0.0, 0.75)),
                InheritOutline,
            ));
            parent.spawn((
                Mesh3d(
                    meshes.add(
                        Torus {
                            minor_radius: 0.1,
                            major_radius: 0.5,
                        }
                        .mesh()
                        .minor_resolution(15)
                        .major_resolution(30)
                        .build(),
                    ),
                ),
                MeshMaterial3d(materials.add(StandardMaterial::from(Color::srgb(0.1, 0.1, 0.9)))),
                Transform::from_rotation(Quat::from_axis_angle(Vec3::Z, TAU / 4.0))
                    .with_translation(Vec3::new(0.0, 0.0, -0.75)),
                InheritOutline,
            ));
        });

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

fn rotates(mut query: Query<&mut Transform, With<Rotates>>, timer: Res<Time>) {
    for mut transform in query.iter_mut() {
        transform.rotate_axis(Dir3::Y, 0.75 * timer.delta_secs());
    }
}
