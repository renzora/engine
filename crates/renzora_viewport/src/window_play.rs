//! Second-window play mode — runs the game in a separate OS window while the
//! editor stays fully live in the main window.
//!
//! Instead of taking over the primary window (the in-editor play path in
//! [`crate::play_mode`]) or spawning an external runtime *binary* (the
//! [`crate::external_runtime`] path — there's no runtime `dist/` directory now),
//! this spawns an in-process second [`Window`] and retargets the game's
//! `SceneCamera` to it via `RenderTarget::Window`. Gameplay (scripts/physics)
//! runs because `PlayModeState` enters `Playing`; the editor keeps rendering to
//! its own offscreen viewport in window 1.
//!
//! Gated by `EditorSettings.external_play_window` (on by default). Closing the
//! game window — or Stop / F5 — tears it down and returns to editing.

use bevy::camera::RenderTarget;
use bevy::core_pipeline::prepass::{
    DeferredPrepass, DepthPrepass, MotionVectorPrepass, NormalPrepass,
};
use bevy::light::AtmosphereEnvironmentMapLight;
use bevy::pbr::{Atmosphere, AtmosphereSettings, ScatteringMedium};
use bevy::prelude::*;
use bevy::render::view::Hdr;
use bevy::window::{Window, WindowCloseRequested, WindowRef};

use renzora::core::{DefaultCamera, PlayModeCamera, PlayModeState, PlayState, SceneCamera};
use renzora_editor::EditorSettings;

/// Marks the spawned OS window that hosts the running game.
#[derive(Component)]
pub struct GameWindow;

/// When `external_play_window` is on, route Play/Stop to opening/closing a second
/// OS window that renders the game camera. Returns `true` if it consumed the
/// request, so [`crate::play_mode::handle_play_mode_transitions`] bails out before
/// the in-editor camera-takeover path.
pub fn try_handle_window_play(world: &mut World) -> bool {
    let enabled = world
        .get_resource::<EditorSettings>()
        .map(|s| s.external_play_window)
        .unwrap_or(false);
    if !enabled {
        return false;
    }

    let (pending_play, pending_stop) = {
        let Some(pm) = world.get_resource::<PlayModeState>() else {
            return false;
        };
        (pm.request_play && pm.is_editing(), pm.request_stop)
    };
    let window_alive = {
        let mut q = world.query_filtered::<Entity, With<GameWindow>>();
        q.iter(world).next().is_some()
    };
    // While the game window is open, any play/stop request collapses to "stop"
    // (the title-bar Play button doesn't know window-play state).
    let want_stop = window_alive && (pending_play || pending_stop);
    let want_play = !window_alive && pending_play;
    if !want_play && !want_stop {
        return false;
    }

    if want_stop {
        exit_window_play(world);
        if let Some(mut pm) = world.get_resource_mut::<PlayModeState>() {
            pm.request_stop = false;
            pm.request_play = false;
        }
        return true;
    }

    enter_window_play(world);
    if let Some(mut pm) = world.get_resource_mut::<PlayModeState>() {
        pm.request_play = false;
    }
    true
}

fn enter_window_play(world: &mut World) {
    use renzora::core::console_log::*;
    console_info("PlayMode", "=== ENTERING WINDOW PLAY ===");

    // Save the scene first, mirroring the in-editor play path.
    world.trigger(renzora::core::SaveCurrentScene);

    // Pick the game camera: prefer DefaultCamera, else the first SceneCamera.
    let mut q = world.query_filtered::<(Entity, Option<&DefaultCamera>), With<SceneCamera>>();
    let candidates: Vec<(Entity, bool)> =
        q.iter(world).map(|(e, dc)| (e, dc.is_some())).collect();
    let game_camera = candidates
        .iter()
        .find(|(_, is_default)| *is_default)
        .map(|(e, _)| *e)
        .or_else(|| candidates.first().map(|(e, _)| *e));
    let Some(cam) = game_camera else {
        console_error("PlayMode", "No scene camera found — cannot enter window play");
        return;
    };
    let is_2d = world.get::<Camera2d>(cam).is_some();

    // Spawn the game window (default 1280x720). Bevy creates the OS window for
    // any `Window` entity automatically.
    let window = world
        .spawn((
            Window {
                title: "Renzora — Play".into(),
                ..default()
            },
            GameWindow,
            Name::new("game-window"),
        ))
        .id();

    // Configure the game camera to render into the new window.
    if !is_2d && world.get::<Camera3d>(cam).is_none() {
        world.entity_mut(cam).insert(Camera3d::default());
    }
    if world.get::<Camera>(cam).is_none() {
        world.entity_mut(cam).insert(Camera::default());
    }
    if let Some(mut c) = world.get_mut::<Camera>(cam) {
        c.is_active = true;
        c.order = 0;
    }
    world
        .entity_mut(cam)
        .insert(RenderTarget::Window(WindowRef::Entity(window)));

    if !is_2d {
        apply_camera_3d(world, cam);
    }
    world.entity_mut(cam).insert(PlayModeCamera);

    world.trigger(renzora::core::ResetScriptStates);
    world.trigger(renzora::core::UnpausePhysics);

    if let Some(mut pm) = world.get_resource_mut::<PlayModeState>() {
        pm.active_game_camera = Some(cam);
        pm.state = PlayState::Playing;
    }
    console_success("PlayMode", "=== WINDOW PLAY ACTIVE ===");
}

/// 3D render setup for the game camera — mirrors [`crate::play_mode`]'s entry so
/// the windowed game renders identically to the editor 3D camera (HDR, prepasses,
/// atmosphere, 100km far plane, deferred G-buffer when in Deferred mode).
fn apply_camera_3d(world: &mut World, cam: Entity) {
    let medium = world
        .resource_mut::<Assets<ScatteringMedium>>()
        .add(ScatteringMedium::default());

    if let Some(mut proj) = world.get_mut::<Projection>(cam) {
        if let Projection::Perspective(ref mut p) = *proj {
            p.far = 100_000.0;
        }
    } else {
        world
            .entity_mut(cam)
            .insert(Projection::Perspective(PerspectiveProjection {
                far: 100_000.0,
                ..default()
            }));
    }

    world.entity_mut(cam).insert((
        Hdr,
        NormalPrepass,
        DepthPrepass,
        MotionVectorPrepass,
        Atmosphere {
            bottom_radius: 6_360_000.0,
            top_radius: 6_460_000.0,
            ground_albedo: Vec3::splat(0.3),
            medium,
        },
        AtmosphereSettings::default(),
        AtmosphereEnvironmentMapLight {
            intensity: 0.0,
            ..default()
        },
        Msaa::Off,
    ));

    let deferred = world
        .get_resource::<renzora::ResolvedRenderingMode>()
        .map(|m| m.is_deferred())
        .unwrap_or(false);
    if deferred {
        world.entity_mut(cam).insert(DeferredPrepass);
    }
}

fn exit_window_play(world: &mut World) {
    use renzora::core::console_log::*;
    console_info("PlayMode", "=== EXITING WINDOW PLAY ===");

    // Strip the play camera back to its authored baseline. Deactivate + detach
    // from the (closing) window *before* the window despawns so nothing renders
    // to a dead surface.
    let mut q = world.query_filtered::<Entity, With<PlayModeCamera>>();
    let cams: Vec<Entity> = q.iter(world).collect();
    for cam in cams {
        let is_2d = world.get::<Camera2d>(cam).is_some();
        if let Some(mut c) = world.get_mut::<Camera>(cam) {
            c.is_active = false;
        }
        let mut e = world.entity_mut(cam);
        e.remove::<PlayModeCamera>();
        e.remove::<Camera>();
        e.insert(RenderTarget::default());
        if !is_2d {
            e.remove::<Camera3d>();
            e.remove::<Hdr>();
            e.remove::<NormalPrepass>();
            e.remove::<DepthPrepass>();
            e.remove::<MotionVectorPrepass>();
            e.remove::<DeferredPrepass>();
            e.remove::<Atmosphere>();
            e.remove::<AtmosphereSettings>();
            e.remove::<AtmosphereEnvironmentMapLight>();
            e.remove::<Msaa>();
        }
    }

    // Close any remaining game window(s). If the user closed it via the OS
    // titlebar, Bevy's `close_when_requested` may have despawned it already, so
    // the query simply returns nothing.
    let mut wq = world.query_filtered::<Entity, With<GameWindow>>();
    let windows: Vec<Entity> = wq.iter(world).collect();
    for w in windows {
        world.entity_mut(w).despawn();
    }

    world.trigger(renzora::core::PausePhysics);
    if let Some(mut pm) = world.get_resource_mut::<PlayModeState>() {
        pm.active_game_camera = None;
        pm.state = PlayState::Editing;
    }
    console_success("PlayMode", "=== WINDOW PLAY EXITED — back to editing ===");
}

/// When the user closes the game window via the OS titlebar, deactivate the game
/// camera *this frame* (so it never renders to the despawning surface) and queue
/// a stop — the exclusive transition handler completes the teardown next frame.
pub fn on_game_window_close(
    mut closed: MessageReader<WindowCloseRequested>,
    game: Query<(), With<GameWindow>>,
    mut cams: Query<&mut Camera, With<PlayModeCamera>>,
    play_mode: Option<ResMut<PlayModeState>>,
) {
    let closing = closed.read().any(|e| game.get(e.window).is_ok());
    if !closing {
        return;
    }
    for mut c in &mut cams {
        c.is_active = false;
    }
    if let Some(mut pm) = play_mode {
        pm.request_stop = true;
    }
}
