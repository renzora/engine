use bevy::prelude::*;
use bevy::render::render_asset::RenderAssetUsages;
use bevy_mod_outline::*;
use std::f32::consts::TAU;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.1, 0.1, 0.1)))
        .add_plugins((DefaultPlugins, OutlinePlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, pulse_outline_thickness)
        .run();
}

#[derive(Component)]
struct PulsingOutline;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    // Create a checkerboard pattern image for the alpha mask
    let mut alpha_mask = Image::new_fill(
        bevy::render::render_resource::Extent3d {
            width: 256,
            height: 256,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        &[0],
        bevy::render::render_resource::TextureFormat::R8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );

    // Generate a checkerboard pattern
    for y in 0..256 {
        for x in 0..256 {
            let checker = (x / 32 + y / 32) % 2 == 0;
            let index = y * 256 + x;

            if let Some(data) = &mut alpha_mask.data {
                if index < data.len() {
                    let alpha = if checker { 255 } else { 0 };
                    data[index] = alpha;
                }
            }
        }
    }

    let alpha_mask_handle = images.add(alpha_mask);

    // Create a square (flat cube) with the alpha mask outline
    let square_mesh = Rectangle::new(1.0, 1.0).mesh().build();

    commands.spawn((
        Mesh3d(meshes.add(square_mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.3, 0.3),
            ..default()
        })),
        OutlineVolume {
            visible: true,
            colour: Color::srgb(1.0, 1.0, 0.0),
            width: 0.0,
        },
        OutlineMode::FloodFlat,
        OutlineAlphaMask {
            texture: Some(alpha_mask_handle),
            channel: TextureChannel::R,
            threshold: 0.5,
        },
        PulsingOutline,
    ));

    // Add light source and camera
    commands.spawn((
        PointLight {
            color: Color::WHITE,
            intensity: 1500.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
        Msaa::Off,
    ));
}

fn pulse_outline_thickness(
    mut query: Query<&mut OutlineVolume, With<PulsingOutline>>,
    timer: Res<Time>,
    mut t: Local<f32>,
) {
    *t = (*t + timer.delta_secs()) % TAU;
    let pulse = (*t * 8.0).sin() * 5.0 + 7.5;

    for mut outline in query.iter_mut() {
        outline.width = pulse;
    }
}
