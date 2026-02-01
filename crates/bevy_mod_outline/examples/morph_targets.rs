//! Controls morph targets in a loaded scene.
//!
//! Illustrates:
//!
//! - How to access and modify individual morph target weights.
//!   See the [`update_weights`] system for details.
//! - How to read morph target names in [`name_morphs`].
//! - How to play morph target animations in [`setup_animations`].

use bevy::{prelude::*, scene::SceneInstance};
use bevy_mod_outline::{
    AutoGenerateOutlineNormalsPlugin, InheritOutline, OutlinePlugin, OutlineVolume,
};

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "morph targets".to_string(),
                    ..default()
                }),
                ..default()
            }),
            OutlinePlugin,
            AutoGenerateOutlineNormalsPlugin::default(),
        ))
        .insert_resource(AmbientLight::default())
        .add_systems(Startup, setup)
        .add_systems(Update, (name_morphs, setup_outlines, setup_animations))
        .run();
}

#[derive(Resource)]
struct MorphData {
    the_wave: Handle<AnimationClip>,
    mesh: Handle<Mesh>,
}

fn setup(asset_server: Res<AssetServer>, mut commands: Commands) {
    commands.insert_resource(MorphData {
        the_wave: asset_server.load("MorphStressTest.gltf#Animation2"),
        mesh: asset_server.load("MorphStressTest.gltf#Mesh0/Primitive0"),
    });
    commands.spawn((
        SceneRoot(asset_server.load("MorphStressTest.gltf#Scene0")),
        OutlineVolume {
            visible: true,
            width: 3.0,
            colour: Color::srgb(1.0, 0.0, 0.0),
        },
    ));
    commands.spawn(DirectionalLight {
        color: Color::WHITE,
        illuminance: 19350.0,
        ..default()
    });
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(3.0, 2.1, 5.2).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Adds outlines to the meshes.
fn setup_outlines(
    mut commands: Commands,
    mut has_setup: Local<bool>,
    scene_query: Query<&SceneInstance>,
    scene_manager: Res<SceneSpawner>,
) {
    if *has_setup {
        return;
    }
    if let Ok(scene) = scene_query.single() {
        if scene_manager.instance_is_ready(**scene) {
            for entity in scene_manager.iter_instance_entities(**scene) {
                commands.entity(entity).insert(InheritOutline);
                *has_setup = true;
            }
        }
    }
}

/// Plays an [`AnimationClip`] from the loaded [`Gltf`] on the [`AnimationPlayer`] created by the spawned scene.
fn setup_animations(
    mut has_setup: Local<bool>,
    mut commands: Commands,
    mut players: Query<(Entity, &Name, &mut AnimationPlayer)>,
    morph_data: Res<MorphData>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    if *has_setup {
        return;
    }
    for (entity, name, mut player) in &mut players {
        // The name of the entity in the GLTF scene containing the AnimationPlayer for our morph targets is "Main"
        if name.as_str() != "Main" {
            continue;
        }
        let (graph, animation) = AnimationGraph::from_clip(morph_data.the_wave.clone());
        commands
            .entity(entity)
            .insert(AnimationGraphHandle(graphs.add(graph)));
        player.play(animation).repeat();
        *has_setup = true;
    }
}

/// You can get the target names in their corresponding [`Mesh`].
/// They are in the order of the weights.
fn name_morphs(
    mut has_printed: Local<bool>,
    morph_data: Res<MorphData>,
    meshes: Res<Assets<Mesh>>,
) {
    if *has_printed {
        return;
    }

    let Some(mesh) = meshes.get(&morph_data.mesh) else {
        return;
    };
    let Some(names) = mesh.morph_target_names() else {
        return;
    };
    for name in names {
        println!("  {name}");
    }
    *has_printed = true;
}
