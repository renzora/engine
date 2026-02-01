use std::f32::consts::{PI, TAU};

use bevy::{
    core_pipeline::bloom::{Bloom, BloomCompositeMode},
    prelude::*,
};

use bevy_mod_outline::*;

#[bevy_main]
fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins((DefaultPlugins, OutlinePlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, (rotates, pulses))
        .run();
}

#[derive(Component)]
struct Rotates;

#[derive(Component)]
struct Pulses(f32);

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Add emissive sphere
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.75).mesh().uv(50, 50))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            ..default()
        })),
        Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),
        OutlineVolume {
            visible: true,
            colour: Color::WHITE,
            width: 5.0,
        },
        Pulses(0.0),
    ));

    // Add satellite
    commands
        .spawn((Transform::default(), InheritedVisibility::default()))
        .insert(Rotates)
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(meshes.add(Sphere::new(0.25).mesh().uv(25, 25))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(1.0, 0.0, 0.0),
                    emissive: LinearRgba::rgb(100.0, 0.0, 0.0),
                    ..default()
                })),
                Transform::from_translation(Vec3::new(0.0, 0.0, 1.25)),
                OutlineVolume {
                    visible: true,
                    colour: Color::WHITE,
                    width: 5.0,
                },
                Pulses(PI),
            ));
        });

    // Add HDR camera
    commands.spawn((
        Camera3d::default(),
        Camera {
            hdr: true,
            ..default()
        },
        Transform::from_xyz(0.0, 3.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
        Msaa::Sample4,
        Bloom {
            intensity: 1.0,
            low_frequency_boost: 0.5,
            low_frequency_boost_curvature: 0.5,
            high_pass_frequency: 0.5,
            composite_mode: BloomCompositeMode::Additive,
            ..default()
        },
    ));
}

fn rotates(mut query: Query<&mut Transform, With<Rotates>>, timer: Res<Time>) {
    for mut transform in query.iter_mut() {
        transform.rotate_axis(Dir3::Y, 0.75 * timer.delta_secs());
    }
}

fn pulses(
    mut query: Query<(&mut OutlineVolume, &Pulses)>,
    timer: Res<Time>,
    mut state: Local<f32>,
) {
    *state = (*state + 0.3 * timer.delta_secs()) % TAU;
    for (mut outline, phase) in query.iter_mut() {
        let t = (*state + phase.0).sin().max(0.0);
        outline.width = (15.0 * t).min(7.5);
    }
}
