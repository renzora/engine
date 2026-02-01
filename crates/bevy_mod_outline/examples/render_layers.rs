use std::f32::consts::PI;

use bevy::{
    prelude::*,
    render::{camera::Viewport, view::RenderLayers},
    window::PrimaryWindow,
};
use bevy_mod_outline::{OutlinePlugin, OutlineRenderLayers, OutlineVolume};

#[bevy_main]
fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins((DefaultPlugins, OutlinePlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, set_camera_viewports)
        .run();
}

const OBJECT_LAYER_ID: usize = 1;
const OUTLINE_LAYER_ID: usize = 2;

#[derive(Copy, Clone, Component)]
struct CameraMode {
    object_layer: bool,
    outline_layer: bool,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Add torus using the regular surface normals for outlining
    commands.spawn((
        Mesh3d(
            meshes.add(
                Torus {
                    minor_radius: 0.2,
                    major_radius: 0.6,
                }
                .mesh()
                .minor_resolution(20)
                .major_resolution(40)
                .build(),
            ),
        ),
        MeshMaterial3d(materials.add(StandardMaterial::from(Color::srgb(0.1, 0.1, 0.9)))),
        Transform::from_rotation(Quat::from_rotation_x(0.5 * PI)).with_translation(0.8 * Vec3::Y),
        OutlineVolume {
            visible: true,
            colour: Color::WHITE,
            width: 10.0,
        },
        RenderLayers::layer(OBJECT_LAYER_ID),
        OutlineRenderLayers(RenderLayers::layer(OUTLINE_LAYER_ID)),
    ));

    // Add plane and light source
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

    // Add cameras for different combinations of render-layers
    for i in 0..4 {
        let object_layer = i & 1 == 1;
        let outline_layer = (i >> 1) & 1 == 1;
        let mut layers = RenderLayers::default();
        if object_layer {
            layers = layers.with(OBJECT_LAYER_ID);
        }
        if outline_layer {
            layers = layers.with(OUTLINE_LAYER_ID);
        }
        commands.spawn((
            Camera3d::default(),
            Camera {
                order: i,
                clear_color: if i > 0 {
                    ClearColorConfig::None
                } else {
                    ClearColorConfig::Default
                },
                ..default()
            },
            Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
            Msaa::Sample4,
            CameraMode {
                object_layer,
                outline_layer,
            },
            layers,
        ));
    }
}

fn set_camera_viewports(
    win_query: Query<Ref<Window>, With<PrimaryWindow>>,
    mut query: Query<(&mut Camera, &CameraMode)>,
) {
    let win = win_query.single().unwrap();
    if win.is_changed() {
        // Divide window into quadrants
        let size = UVec2::new(win.physical_width() / 2, win.physical_height() / 2);
        for (mut camera, mode) in query.iter_mut() {
            let offset = UVec2::new(
                if mode.object_layer { size.x } else { 0 },
                if mode.outline_layer { size.y } else { 0 },
            );
            camera.viewport = Some(Viewport {
                physical_position: offset,
                physical_size: size,
                ..default()
            });
        }
    }
}
