use std::f32::consts::PI;

use bevy::{prelude::*, scene::SceneInstance};
use bevy_mod_outline::{
    AsyncSceneInheritOutline, AutoGenerateOutlineNormalsPlugin, OutlinePlugin, OutlineVolume,
};

#[derive(Resource)]
struct Fox(Handle<AnimationClip>);

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            OutlinePlugin,
            AutoGenerateOutlineNormalsPlugin::default(),
        ))
        .insert_resource(AmbientLight::default())
        .add_systems(Startup, setup)
        .add_systems(Update, setup_scene_once_loaded)
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Insert a resource with the current animation
    commands.insert_resource(Fox(asset_server.load("Fox.glb#Animation0")));

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(100.0, 100.0, 150.0).looking_at(Vec3::new(0.0, 20.0, 0.0), Vec3::Y),
    ));

    // Plane
    commands.spawn((
        Mesh3d(
            meshes.add(
                Plane3d::new(Vec3::Y, Vec2::new(500000.0, 500000.0))
                    .mesh()
                    .build(),
            ),
        ),
        MeshMaterial3d(materials.add(StandardMaterial::from(Color::srgb(0.3, 0.5, 0.3)))),
    ));

    // Light
    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::ZYX, 0.0, 1.0, -PI / 4.)),
    ));

    // Fox
    commands.spawn((
        SceneRoot(asset_server.load("Fox.glb#Scene0")),
        OutlineVolume {
            visible: true,
            width: 3.0,
            colour: Color::srgb(1.0, 0.0, 0.0),
        },
        AsyncSceneInheritOutline::default(),
    ));
}

// Once the scene is loaded, start the animation
fn setup_scene_once_loaded(
    mut commands: Commands,
    scene_query: Query<&SceneInstance>,
    scene_manager: Res<SceneSpawner>,
    mut player_query: Query<(Entity, &mut AnimationPlayer)>,
    animation: Res<Fox>,
    mut done: Local<bool>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    if !*done {
        if let (Ok(scene), Ok((entity, mut player))) =
            (scene_query.single(), player_query.single_mut())
        {
            if scene_manager.instance_is_ready(**scene) {
                let (graph, animation) = AnimationGraph::from_clip(animation.0.clone());
                commands
                    .entity(entity)
                    .insert(AnimationGraphHandle(graphs.add(graph)));
                player.play(animation).repeat();
                *done = true;
            }
        }
    }
}
