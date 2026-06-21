//! This example showcases how Hermite interpolation can be used to make `Transform` easing
//! more accurate and reliable.
//!
//! By default, `Transform` interpolation and extrapolation use *linear interpolation* (`lerp`)
//! for easing translation and scale, and *spherical linear interpolation* (`slerp`)
//! for easing rotation. This is computationally efficient and works well for most cases.
//!
//! However, for more accurate and reliable easing that works at arbitrary velocities,
//! it may be preferable to use *Hermite interpolation*. It uses both position and velocity information
//! to estimate the trajectories of entities, producing smoother results.

use bevy::{
    color::palettes::{
        css::WHITE,
        tailwind::{CYAN_400, LIME_400, RED_400},
    },
    ecs::query::QueryData,
    prelude::*,
};
use bevy_transform_interpolation::{
    VelocitySource,
    hermite::{RotationHermiteEasing, TransformHermiteEasingPlugin, TranslationHermiteEasing},
    prelude::*,
};

const MOVEMENT_SPEED: f32 = 250.0;
const ROTATION_SPEED: f32 = core::f32::consts::TAU * 3.0;

fn main() {
    let mut app = App::new();

    // Add the `TransformInterpolationPlugin` to the app to enable transform interpolation.
    // Add the `TransformHermiteEasingPlugin` to the app to enable Hermite interpolation for easing.
    app.add_plugins((
        DefaultPlugins,
        TransformInterpolationPlugin::default(),
        // We must specify "velocity sources" to tell the plugin how to extract velocity information.
        // These are implemented below this function.
        TransformHermiteEasingPlugin::<LinVelSource, AngVelSource>::default(),
    ));

    // Optional: Make the previous velocity components required for Hermite interpolation to insert them automatically.
    app.register_required_components::<TranslationHermiteEasing, PreviousLinearVelocity>();
    app.register_required_components::<RotationHermiteEasing, PreviousAngularVelocity>();

    // Set the fixed timestep to just 5 Hz for demonstration purposes.
    app.insert_resource(Time::<Fixed>::from_hz(5.0));

    // Setup the scene and UI, and update text in `Update`.
    app.add_systems(Startup, (setup, setup_text))
        .add_systems(Update, (change_timestep, update_timestep_text));

    // Update the previous velocity components in `FixedPreUpdate`.
    app.add_systems(FixedPreUpdate, update_previous_velocity);

    // Move entities in `FixedUpdate`. The movement should appear smooth for interpolated entities.
    app.add_systems(
        FixedUpdate,
        (flip_movement_direction.before(movement), movement, rotate),
    );

    // Run the app.
    app.run();
}

/// The linear velocity of an entity indicating its movement speed and direction.
#[derive(Component, Default, Deref, DerefMut)]
struct LinearVelocity(Vec2);

/// The previous linear velocity of an entity indicating its movement speed and direction during the previous frame.
#[derive(Component, Default, Deref, DerefMut)]
struct PreviousLinearVelocity(Vec2);

/// The angular velocity of an entity indicating its rotation speed.
#[derive(Component, Default, Deref, DerefMut)]
struct AngularVelocity(f32);

/// The previous angular velocity of an entity indicating its rotation speed during the previous frame.
#[derive(Component, Default, Deref, DerefMut)]
struct PreviousAngularVelocity(f32);

#[derive(QueryData)]
struct LinVelSource;

impl VelocitySource for LinVelSource {
    type Previous = PreviousLinearVelocity;
    type Current = LinearVelocity;

    fn previous(start: &Self::Previous) -> Vec3 {
        start.0.extend(0.0)
    }

    fn current(end: &Self::Current) -> Vec3 {
        end.0.extend(0.0)
    }
}

#[derive(QueryData)]
struct AngVelSource;

impl VelocitySource for AngVelSource {
    type Previous = PreviousAngularVelocity;
    type Current = AngularVelocity;

    fn previous(start: &Self::Previous) -> Vec3 {
        Vec3::Z * start.0
    }

    fn current(end: &Self::Current) -> Vec3 {
        Vec3::Z * end.0
    }
}

fn update_previous_velocity(
    mut lin_vel_query: Query<(&LinearVelocity, &mut PreviousLinearVelocity)>,
    mut ang_vel_query: Query<(&AngularVelocity, &mut PreviousAngularVelocity)>,
) {
    for (lin_vel, mut prev_lin_vel) in &mut lin_vel_query {
        prev_lin_vel.0 = lin_vel.0;
    }

    for (ang_vel, mut prev_ang_vel) in &mut ang_vel_query {
        prev_ang_vel.0 = ang_vel.0;
    }
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // Spawn a camera.
    commands.spawn(Camera2d);

    let mesh = meshes.add(Rectangle::from_length(60.0));

    // This entity uses linear interpolation (`lerp` and `slerp`).
    // Notice how it can cause entities to rotate in the wrong direction at high speeds,
    // because `slerp` always takes the shortest path and doesn't account for full revolutions.
    commands.spawn((
        Name::new("Lerp + Slerp"),
        Mesh2d(mesh.clone()),
        MeshMaterial2d(materials.add(Color::from(CYAN_400)).clone()),
        Transform::from_xyz(-500.0, 120.0, 0.0),
        TransformInterpolation,
        LinearVelocity(Vec2::new(MOVEMENT_SPEED, 0.0)),
        AngularVelocity(ROTATION_SPEED),
    ));

    // This entity uses Hermite interpolation.
    commands.spawn((
        Name::new("Hermite Interpolation"),
        Mesh2d(mesh.clone()),
        MeshMaterial2d(materials.add(Color::from(LIME_400)).clone()),
        Transform::from_xyz(-500.0, 0.0, 0.0),
        // Note: `TransformHermiteEasing` on its own does not perform interpolation.
        //       Either `TransformInterpolation` or `TransformExtrapolation` must be present.
        TransformInterpolation,
        TransformHermiteEasing,
        LinearVelocity(Vec2::new(MOVEMENT_SPEED, 0.0)),
        AngularVelocity(ROTATION_SPEED),
    ));

    // This entity is simulated in `FixedUpdate` without any smoothing.
    commands.spawn((
        Name::new("No Interpolation"),
        Mesh2d(mesh.clone()),
        MeshMaterial2d(materials.add(Color::from(RED_400)).clone()),
        Transform::from_xyz(-500.0, -120.0, 0.0),
        LinearVelocity(Vec2::new(MOVEMENT_SPEED, 0.0)),
        AngularVelocity(ROTATION_SPEED),
    ));
}

/// Flips the movement directions of objects when they reach the left or right side of the screen.
fn flip_movement_direction(mut query: Query<(&Transform, &mut LinearVelocity)>) {
    for (transform, mut lin_vel) in &mut query {
        if transform.translation.x > 500.0 && lin_vel.0.x > 0.0 {
            lin_vel.0 = Vec2::new(-MOVEMENT_SPEED, 0.0);
        } else if transform.translation.x < -500.0 && lin_vel.0.x < 0.0 {
            lin_vel.0 = Vec2::new(MOVEMENT_SPEED, 0.0);
        }
    }
}

/// Changes the timestep of the simulation when the up or down arrow keys are pressed.
fn change_timestep(mut time: ResMut<Time<Fixed>>, keyboard_input: Res<ButtonInput<KeyCode>>) {
    if keyboard_input.pressed(KeyCode::ArrowUp) {
        let new_timestep = (time.delta_secs_f64() * 0.9).max(1.0 / 255.0);
        time.set_timestep_seconds(new_timestep);
    }
    if keyboard_input.pressed(KeyCode::ArrowDown) {
        let new_timestep = (time.delta_secs_f64() * 1.1).min(1.0);
        time.set_timestep_seconds(new_timestep);
    }
}

/// Moves entities based on their `LinearVelocity`.
fn movement(mut query: Query<(&mut Transform, &LinearVelocity)>, time: Res<Time>) {
    let delta_secs = time.delta_secs();

    for (mut transform, lin_vel) in &mut query {
        transform.translation += lin_vel.extend(0.0) * delta_secs;
    }
}

/// Rotates entities based on their `AngularVelocity`.
fn rotate(mut query: Query<(&mut Transform, &AngularVelocity)>, time: Res<Time>) {
    let delta_secs = time.delta_secs();

    for (mut transform, ang_vel) in &mut query {
        transform.rotate_local_z(ang_vel.0 * delta_secs);
    }
}

#[derive(Component)]
struct TimestepText;

fn setup_text(mut commands: Commands) {
    let font = TextFont {
        font_size: FontSize::Px(20.0),
        ..default()
    };

    commands
        .spawn((
            Text::new("Fixed Hz: "),
            TextColor::from(WHITE),
            font.clone(),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                left: Val::Px(10.0),
                ..default()
            },
        ))
        .with_child((TimestepText, TextSpan::default()));

    commands.spawn((
        Text::new("Change Timestep With Up/Down Arrow"),
        TextColor::from(WHITE),
        font.clone(),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(10.0),
            ..default()
        },
    ));

    commands.spawn((
        Text::new("Lerp + Slerp"),
        TextColor::from(CYAN_400),
        font.clone(),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(50.0),
            left: Val::Px(10.0),
            ..default()
        },
    ));

    commands.spawn((
        Text::new("Hermite Interpolation"),
        TextColor::from(LIME_400),
        font.clone(),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(75.0),
            left: Val::Px(10.0),
            ..default()
        },
    ));

    commands.spawn((
        Text::new("No Interpolation"),
        TextColor::from(RED_400),
        font.clone(),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(100.0),
            left: Val::Px(10.0),
            ..default()
        },
    ));
}

fn update_timestep_text(
    mut text: Single<&mut TextSpan, With<TimestepText>>,
    time: Res<Time<Fixed>>,
) {
    let timestep = time.timestep().as_secs_f32().recip();
    text.0 = format!("{timestep:.2}");
}
