use bevy::{
    core_pipeline::{
        experimental::taa::{TemporalAntiAliasPlugin, TemporalAntiAliasing},
        fxaa::Fxaa,
        smaa::{Smaa, SmaaPreset},
    },
    prelude::*,
};

use bevy_mod_outline::*;

#[bevy_main]
fn main() {
    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins((DefaultPlugins, TemporalAntiAliasPlugin, OutlinePlugin))
        .insert_state(AAMode::NoAA)
        .add_systems(Startup, setup)
        .add_systems(Update, (bounce, highlight, interaction))
        .add_systems(
            OnEnter(AAMode::MSAAx4),
            |mut query: Query<Entity, With<TheCamera>>, mut commands: Commands| {
                commands
                    .entity(query.single().unwrap())
                    .insert(Msaa::Sample4);
            },
        )
        .add_systems(
            OnExit(AAMode::MSAAx4),
            |mut query: Query<Entity, With<TheCamera>>, mut commands: Commands| {
                commands.entity(query.single().unwrap()).insert(Msaa::Off);
            },
        )
        .add_systems(
            OnEnter(AAMode::FXAA),
            |mut query: Query<Entity, With<TheCamera>>, mut commands: Commands| {
                commands
                    .entity(query.single().unwrap())
                    .insert(Fxaa::default());
            },
        )
        .add_systems(
            OnExit(AAMode::FXAA),
            |mut query: Query<Entity, With<TheCamera>>, mut commands: Commands| {
                commands.entity(query.single().unwrap()).remove::<Fxaa>();
            },
        )
        .add_systems(
            OnEnter(AAMode::SMAA),
            |mut query: Query<Entity, With<TheCamera>>, mut commands: Commands| {
                commands.entity(query.single().unwrap()).insert(Smaa {
                    preset: SmaaPreset::Ultra,
                });
            },
        )
        .add_systems(
            OnExit(AAMode::SMAA),
            |mut query: Query<Entity, With<TheCamera>>, mut commands: Commands| {
                commands.entity(query.single().unwrap()).remove::<Smaa>();
            },
        )
        .add_systems(
            OnEnter(AAMode::TAA),
            |mut query: Query<Entity, With<TheCamera>>, mut commands: Commands| {
                commands
                    .entity(query.single().unwrap())
                    .insert(TemporalAntiAliasing::default());
            },
        )
        .add_systems(
            OnExit(AAMode::TAA),
            |mut query: Query<Entity, With<TheCamera>>, mut commands: Commands| {
                commands
                    .entity(query.single().unwrap())
                    .remove::<TemporalAntiAliasing>();
            },
        )
        .run();
}

#[derive(Component)]
struct Bounce;

#[derive(Component)]
struct TheCamera;

#[derive(Copy, Clone, States, Component, Debug, PartialEq, Eq, Hash)]
enum AAMode {
    NoAA,
    MSAAx4,
    FXAA,
    SMAA,
    TAA,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Add spheres
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(1.0).mesh().ico(9).unwrap())),
        MeshMaterial3d(materials.add(StandardMaterial::from(Color::srgb(0.5, 0.5, 0.5)))),
        Transform::from_translation(Vec3::new(-1.5, 0.0, 0.0)),
        OutlineVolume {
            visible: true,
            width: 25.0,
            colour: Color::srgb(1.0, 1.0, 0.0),
        },
        Bounce,
    ));
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(1.0).mesh().ico(20).unwrap())),
        MeshMaterial3d(materials.add(StandardMaterial::from(Color::srgb(0.5, 0.5, 0.5)))),
        Transform::from_translation(Vec3::new(1.5, 0.0, 0.0)),
        Bounce,
    ));

    // Add buttons
    commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|parent| {
            for mode in [
                AAMode::NoAA,
                AAMode::MSAAx4,
                AAMode::FXAA,
                AAMode::SMAA,
                AAMode::TAA,
            ] {
                parent
                    .spawn((
                        Button,
                        Node {
                            margin: UiRect::all(Val::Px(5.0)),
                            padding: UiRect::all(Val::Px(10.0)),
                            border: UiRect::all(Val::Px(5.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BorderColor(Color::BLACK),
                        BorderRadius::MAX,
                        BackgroundColor(Color::srgb(0.2, 0.2, 0.2)),
                        mode,
                    ))
                    .with_children(|parent| {
                        parent
                            .spawn(Text::new(format!("{:?}", mode)))
                            .insert(TextColor(Color::WHITE));
                    });
            }
        });

    // Add light source and camera
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        Msaa::Off,
        TheCamera,
    ));
}

fn bounce(mut query: Query<&mut Transform, With<Bounce>>, timer: Res<Time>, mut t: Local<f32>) {
    *t = (*t + timer.delta_secs()) % 4.0;
    let y = (*t - 2.0).abs() - 1.0;
    for mut transform in query.iter_mut() {
        transform.translation.y = y;
    }
}

fn highlight(mut query: Query<(&mut BorderColor, &AAMode)>, state: Res<State<AAMode>>) {
    for (mut border, m) in query.iter_mut() {
        *border = if m == state.get() {
            BorderColor(Color::srgb(0.0, 0.0, 1.0))
        } else {
            BorderColor(Color::BLACK)
        };
    }
}

fn interaction(
    query: Query<(&Interaction, &AAMode)>,
    mut armed: Local<bool>,
    mut state: ResMut<NextState<AAMode>>,
) {
    let mut any_pressed = false;
    for (i, m) in query.iter() {
        if *i == Interaction::Pressed {
            any_pressed = true;
            if *armed {
                state.set(*m);
            }
        }
    }
    *armed = !any_pressed;
}
