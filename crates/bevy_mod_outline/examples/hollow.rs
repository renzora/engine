use std::f32::consts::{PI, TAU};

use bevy::{gltf::GltfPlugin, prelude::*, scene::SceneInstance};
use bevy_mod_outline::{
    AsyncSceneInheritOutline, OutlinePlugin, OutlineStencil, OutlineVolume,
    ATTRIBUTE_OUTLINE_NORMAL,
};

fn main() {
    App::new()
        // Register outline normal vertex attribute with glTF plugin
        .add_plugins(
            DefaultPlugins.build().set(
                GltfPlugin::default()
                    .add_custom_vertex_attribute("_OUTLINE_NORMAL", ATTRIBUTE_OUTLINE_NORMAL),
            ),
        )
        .add_plugins(OutlinePlugin)
        .insert_resource(AmbientLight::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (setup_scene_once_loaded, rotates_and_pulses, rotates_hue),
        )
        .run();
}

#[derive(Component)]
struct RotatesAndPulses;

#[derive(Component)]
struct RotatesHue;

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(20.0, 20.0, 30.0).looking_at(Vec3::new(0.0, 0.0, 0.0), Vec3::Y),
    ));

    // Light
    commands.spawn((
        Transform::from_rotation(Quat::from_euler(EulerRot::ZYX, 0.0, 1.0, -PI / 4.)),
        DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
    ));

    // Hollow
    commands.spawn((
        SceneRoot(asset_server.load("hollow.glb#Scene0")),
        RotatesAndPulses,
        OutlineVolume {
            visible: true,
            width: 0.0,
            colour: Color::srgb(0.0, 0.0, 1.0),
        },
        OutlineStencil {
            enabled: true,
            offset: 0.0,
        },
        AsyncSceneInheritOutline::default(),
    ));
}

// Once the scene is loaded, start the animation and add an outline
fn setup_scene_once_loaded(
    mut commands: Commands,
    scene_query: Query<&SceneInstance>,
    scene_manager: Res<SceneSpawner>,
    name_query: Query<&Name, With<Mesh3d>>,
    mut done: Local<bool>,
) {
    if !*done {
        if let Ok(scene) = scene_query.single() {
            if scene_manager.instance_is_ready(**scene) {
                for entity in scene_manager.iter_instance_entities(**scene) {
                    if let Ok(name) = name_query.get(entity) {
                        if name.as_str() == "inside" {
                            commands.entity(entity).insert(RotatesHue);
                        }
                    }
                }
                *done = true;
            }
        }
    }
}

fn rotates_and_pulses(
    mut query: Query<(&mut Transform, &mut OutlineVolume), With<RotatesAndPulses>>,
    timer: Res<Time>,
    mut t: Local<f32>,
) {
    *t = (*t + timer.delta_secs()) % TAU;
    let a = t.sin();
    let b = 10.0 * (3.0 * *t).cos().abs();
    for (mut transform, mut volume) in query.iter_mut() {
        *transform = Transform::from_rotation(Quat::from_rotation_y(a));
        volume.width = b;
    }
}

fn rotates_hue(
    query: Query<&mut MeshMaterial3d<StandardMaterial>, With<RotatesHue>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    timer: Res<Time>,
) {
    for handle in query.iter() {
        let material = materials.get_mut(handle.id()).unwrap();
        if let Color::Hsla(hsla) = material.base_color {
            material.base_color = Color::Hsla(Hsla {
                hue: (hsla.hue + 15.0 * timer.delta_secs()) % 360.0,
                ..hsla
            });
        }
    }
}
