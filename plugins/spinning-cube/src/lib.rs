//! A native plugin that renders a spinning cube.
//!
//! Ordinary Bevy throughout — meshes, materials, a `Transform` rotated each
//! frame. The only Renzora-specific line is `renzora::plugin!` at the bottom.
//! Nothing here is aware it is being loaded from a library rather than compiled
//! into the engine, which is the point.
//!
//! # `Name`, and when to spawn
//!
//! Two editor behaviours hang off `Name`, and they pull in opposite directions.
//!
//! The hierarchy panel queries `(Entity, &Name)` — no name, no row, and nothing
//! to click, so no selection or gizmo either. But `despawn_scene_entities`
//! selects on `With<Name>` too: a named entity is a *scene* entity, saved with
//! the scene and cleared on every scene switch, including the one that runs when
//! a project first opens.
//!
//! So a named cube spawned in `Startup` appears and then vanishes a moment
//! later, because the project-load teardown comes after. The answer is not to
//! drop the name — it is to spawn at the right time. `OnEnter(SplashState::
//! Editor)` runs once the project is open and the teardown has been and gone,
//! which is where anything contributing scene content belongs.
//!
//! `SplashState` is in the contract crate, so a plugin can hook it without
//! depending on any editor crate.

use bevy::prelude::*;
use renzora::SplashState;

/// Marks the cube so the spin system finds it and nothing else.
#[derive(Component, Clone, Default)]
pub struct SpinningCube;

pub struct SpinningCubePlugin;

impl Plugin for SpinningCubePlugin {
    fn build(&self, app: &mut App) {
        info!("[spinning-cube] build()");
        // NOT `Startup` — see the module doc. That runs before the project-load
        // teardown, which despawns everything with a `Name`.
        app.add_systems(OnEnter(SplashState::Editor), spawn)
            .add_systems(Update, spin);
    }
}

fn spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        // Required for the hierarchy panel, which queries `(Entity, &Name)`.
        // Without it the cube renders but cannot be listed, clicked or selected.
        Name::new("Spinning Cube"),
        SpinningCube,
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb_u8(124, 144, 255),
            // Emissive so the cube is visible in an empty scene with no lights.
            // A demo that only shows up once someone adds a light is a demo that
            // looks broken.
            emissive: LinearRgba::rgb(0.2, 0.3, 0.9),
            ..default()
        })),
        Transform::from_xyz(0.0, 1.0, 0.0),
    ));
    info!("[spinning-cube] spawned a cube at (0, 1, 0)");
}

/// Rotates on two axes so it reads as a cube rather than a flat hexagon.
fn spin(time: Res<Time>, mut q: Query<&mut Transform, With<SpinningCube>>) {
    let dt = time.delta_secs();
    for mut t in &mut q {
        t.rotate_y(dt * 0.8);
        t.rotate_x(dt * 0.3);
    }
}

renzora::plugin!(SpinningCubePlugin);
