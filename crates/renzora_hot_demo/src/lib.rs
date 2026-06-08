//! Hot-load demo plugin.
//!
//! A standalone distribution plugin that proves a dlopen'd cdylib gets the same
//! full Bevy ECS access an engine-internal plugin does — `Commands`, `Query`,
//! `Res`/`ResMut`, `Assets`, `Time`, the lot — when it's dropped into `plugins/`
//! *while the engine is running*.
//!
//! Build it, then copy the resulting dll/so/dylib into the running engine's
//! `plugins/` directory (see the crate README / chat instructions). The
//! `HotPluginPlugin` watcher builds it into the live world on the next frame;
//! you should see a green "loaded" toast, three rotating cubes appear at the
//! origin, and a once-a-second heartbeat in the console.
//!
//! Note: everything runs in `Update`, not `Startup` — a hot-loaded plugin joins
//! the session after the startup schedules have already executed, so `Startup`
//! systems would never fire. `spawn_demo_cubes` self-guards to run its spawn
//! exactly once instead.

use bevy::prelude::*;

#[derive(Default)]
pub struct HotDemoPlugin;

impl Plugin for HotDemoPlugin {
    fn build(&self, app: &mut App) {
        // Resource insertion, system registration — all the normal `&mut App`
        // surface, working from a dll built and loaded after `app.run()`.
        app.init_resource::<HotDemoState>().add_systems(
            Update,
            (spawn_demo_cubes, rotate_demo_cubes, heartbeat),
        );
        info!("[hot-demo] HotDemoPlugin::build ran — registered resource + 3 Update systems");
    }
}

/// Plugin-owned state. Proves a hot-loaded plugin can define and mutate its own
/// resources in the live world.
#[derive(Resource, Default)]
struct HotDemoState {
    spawned: bool,
    secs_since_log: f32,
    ticks: u32,
}

/// Marker so the rotate/heartbeat systems can query just our entities.
#[derive(Component)]
struct HotDemoCube;

/// Spawn three emissive cubes once, the first time this system runs after the
/// plugin is hot-loaded. Demonstrates `Commands` + `Assets` access live.
fn spawn_demo_cubes(
    mut state: ResMut<HotDemoState>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if state.spawned {
        return;
    }
    state.spawned = true;

    let mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let colors = [
        Color::srgb(0.95, 0.25, 0.25),
        Color::srgb(0.25, 0.9, 0.4),
        Color::srgb(0.3, 0.5, 1.0),
    ];

    for (i, color) in colors.into_iter().enumerate() {
        let material = materials.add(StandardMaterial {
            base_color: color,
            // Emissive so the cubes are clearly visible even if the current
            // scene has no lights set up.
            emissive: LinearRgba::from(color) * 0.6,
            ..Default::default()
        });
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            Transform::from_xyz((i as f32 - 1.0) * 2.0, 1.0, 0.0),
            HotDemoCube,
            Name::new(format!("HotDemo Cube {i}")),
        ));
    }

    info!("[hot-demo] spawned 3 cubes via Commands — ECS write access is live");
}

/// Rotate every demo cube each frame. Demonstrates a mutable `Query` filtered by
/// our marker component, driven by `Time`.
fn rotate_demo_cubes(time: Res<Time>, mut cubes: Query<&mut Transform, With<HotDemoCube>>) {
    let dt = time.delta_secs();
    for mut transform in &mut cubes {
        transform.rotate_y(dt * 1.5);
        transform.rotate_x(dt * 0.7);
    }
}

/// Log a heartbeat once a second so you can confirm the plugin's systems keep
/// ticking after the hot-load. Demonstrates `Res`/`ResMut` + a counting `Query`.
fn heartbeat(time: Res<Time>, mut state: ResMut<HotDemoState>, cubes: Query<(), With<HotDemoCube>>) {
    state.secs_since_log += time.delta_secs();
    if state.secs_since_log >= 1.0 {
        state.secs_since_log = 0.0;
        state.ticks += 1;
        info!(
            "[hot-demo] alive — tick {}, {} cube(s) under ECS control",
            state.ticks,
            cubes.iter().count()
        );
    }
}

// Runtime scope (default) → loads in the editor viewport AND an exported game.
renzora::add!(HotDemoPlugin);
