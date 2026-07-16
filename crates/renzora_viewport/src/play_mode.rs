//! Play mode — switches from editor camera to game camera for in-editor playtesting.

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
use renzora::core::{DefaultCamera, PlayModeCamera, PlayModeState, PlayState, SceneCamera};
use renzora_editor_framework::EditorSettings;

use crate::external_runtime::{
    self, find_runtime_binary, replace_child, spawn_runtime, ExternalRuntime,
};

/// The viewport-maximize state before play maximized it (only present when the
/// "maximize viewport on play" setting drove the maximize), so [`exit_play_mode`]
/// restores exactly what the user had.
#[derive(Resource)]
struct PrePlayMaximized(Option<usize>);

/// Handles play mode transitions each frame.
pub fn handle_play_mode_transitions(world: &mut World) {
    // Try the external-runtime path first. If the Play target is set to
    // "Runtime Window" (the Play button's dropdown / the external_play_window
    // setting), route Play/Stop to spawning/killing the child instead of
    // doing the in-editor camera switch. The editor's own `PlayModeState`
    // stays in `Editing` while the runtime owns the game.
    //
    // The launcher prefers a packaged `renzora-runtime` sibling and otherwise
    // relaunches this binary with `--no-editor`, so it virtually always finds
    // something; on the rare failure it returns false and we fall through to
    // in-editor play, so Play always does *something*.
    if try_handle_external_runtime(world) {
        return;
    }

    let Some(mut play_mode) = world.remove_resource::<PlayModeState>() else {
        return;
    };

    if play_mode.request_play && play_mode.is_editing() {
        play_mode.request_play = false;
        enter_play_mode(world, &mut play_mode);
    } else if play_mode.request_simulate && play_mode.is_editing() {
        play_mode.request_simulate = false;
        enter_simulate_mode(world, &mut play_mode);
    } else if play_mode.request_stop
        && (play_mode.is_in_play_mode() || play_mode.is_simulating())
    {
        play_mode.request_stop = false;
        // Simulate and full Play tear down differently (Simulate restores the
        // scene snapshot and never touched the camera/chrome), so branch here.
        if play_mode.is_simulating() {
            exit_simulate_mode(world, &mut play_mode);
        } else {
            exit_play_mode(world, &mut play_mode);
        }
    } else if play_mode.request_pause {
        play_mode.request_pause = false;
        match play_mode.state {
            PlayState::Playing => play_mode.state = PlayState::Paused,
            PlayState::Paused => play_mode.state = PlayState::Playing,
            _ => {}
        }
    } else {
        play_mode.request_play = false;
        play_mode.request_simulate = false;
        play_mode.request_stop = false;
        play_mode.request_pause = false;
    }

    world.insert_resource(play_mode);
}

/// External-runtime mode handler. Returns true if it consumed the
/// play/stop request — caller should bail out without falling through to
/// the in-editor path. Returns false if the feature is off or
/// inapplicable (no binary found, no project loaded, no pending request).
fn try_handle_external_runtime(world: &mut World) -> bool {
    use renzora::core::console_log::*;

    // Inspect the request flags before mutating anything else.
    let Some(play_mode) = world.get_resource::<PlayModeState>() else {
        return false;
    };
    let runtime_alive = world
        .get_resource::<ExternalRuntime>()
        .map(|r| r.is_alive())
        .unwrap_or(false);
    // Target gate: only *spawn* when the Play target is "Runtime Window".
    // Stopping a live child is deliberately NOT gated — the user can flip
    // the target back to "Viewport" (Play dropdown) while a runtime is
    // running, and Stop must still kill it rather than fall through to the
    // in-editor path (which would clear the request without doing anything,
    // leaving the child orphaned behind a stuck Stop button).
    // "Window" and "VR Headset" both run the external-process path; VR just
    // adds `--vr` to the child's arguments.
    let (enabled, vr) = world
        .get_resource::<EditorSettings>()
        .map(|s| (s.external_play_window || s.play_launch_vr, s.play_launch_vr))
        .unwrap_or((false, false));
    if !enabled && !runtime_alive {
        return false;
    }
    let pending_play = play_mode.request_play && play_mode.is_editing();
    let pending_stop = play_mode.request_stop;
    // While a child runtime is alive, *every* play/stop request collapses
    // to "stop." That keeps the title bar's Play button (which doesn't
    // know about external runtime state) from accidentally spawning a
    // second instance.
    let want_stop = runtime_alive && (pending_play || pending_stop);
    let want_play = enabled && !runtime_alive && pending_play;

    if !want_play && !want_stop {
        return false;
    }

    if want_stop {
        if let Some(mut runtime) = world.get_resource_mut::<ExternalRuntime>() {
            if external_runtime::kill_runtime(&mut runtime) {
                console_info("PlayMode", "External runtime killed");
            }
        }
        if let Some(mut pm) = world.get_resource_mut::<PlayModeState>() {
            pm.request_stop = false;
            pm.request_play = false;
        }
        return true;
    }

    // want_play — try to locate and spawn the runtime. If we can't, log
    // and let the caller fall through to the in-editor path so Play still
    // does something.
    let Some(binary) = find_runtime_binary() else {
        console_info(
            "PlayMode",
            "External play requested but runtime binary not found — falling back to in-editor play",
        );
        return false;
    };

    let project_path = world
        .get_resource::<renzora::CurrentProject>()
        .map(|p| p.path.clone());
    let Some(project_path) = project_path else {
        console_error(
            "PlayMode",
            "External play requested but no project is loaded",
        );
        // Eat the request so we don't repeatedly attempt this — the user
        // hasn't loaded a project, the in-editor path can't help either.
        if let Some(mut pm) = world.get_resource_mut::<PlayModeState>() {
            pm.request_play = false;
        }
        return true;
    };

    // Save scene before launching, mirroring `enter_play_mode`. The
    // runtime reads from disk, so unsaved edits would otherwise be
    // invisible to it.
    world.trigger(renzora::core::SaveCurrentScene);

    console_info(
        "PlayMode",
        format!(
            "Spawning external runtime: {} --no-editor --project {}{}",
            binary.display(),
            project_path.display(),
            if vr { " --vr" } else { "" }
        ),
    );

    match spawn_runtime(&binary, &project_path, vr) {
        Ok(child) => {
            if let Some(mut runtime) = world.get_resource_mut::<ExternalRuntime>() {
                replace_child(&mut runtime, child);
                // Raise the "Preparing export runtime" overlay and pause the
                // editor until the runtime window closes.
                runtime.begin_preparing();
            }
            if let Some(mut pm) = world.get_resource_mut::<PlayModeState>() {
                pm.request_play = false;
            }
            true
        }
        Err(e) => {
            console_error(
                "PlayMode",
                format!("Failed to spawn runtime: {} — falling back", e),
            );
            // Don't eat the request; let in-editor play take over.
            false
        }
    }
}

fn enter_play_mode(world: &mut World, play_mode: &mut PlayModeState) {
    use renzora::core::console_log::*;
    console_info("PlayMode", "=== ENTERING PLAY MODE ===");

    // Save scene before entering play mode (observed by renzora_engine)
    world.trigger(renzora::core::SaveCurrentScene);
    console_info("PlayMode", "Scene saved before play mode");

    // Find the game camera: prefer DefaultCamera, then first SceneCamera.
    let mut q = world.query_filtered::<(Entity, Option<&DefaultCamera>), With<SceneCamera>>();
    let (mut def, mut first) = (None, None);
    for (e, dc) in q.iter(world) {
        if first.is_none() {
            first = Some(e);
        }
        if dc.is_some() && def.is_none() {
            def = Some(e);
        }
    }
    let Some(cam_entity) = def.or(first) else {
        console_error("PlayMode", "No scene camera found — cannot enter play mode");
        warn!("Play mode: no scene camera found");
        return;
    };

    // Mark the authored scene camera as the play camera — kept for compatibility
    // (scene_io's scene-transition handling, the camera controller's exclusions,
    // debug overlays all key off `PlayModeCamera`). We DELIBERATELY do not touch
    // its render pipeline, render target, or `is_active`, and we leave the editor +
    // UI cameras alone. Rendering is handled entirely by `renzora_camera`'s
    // `drive_editor_camera_in_play`, which points the existing editor viewport
    // camera at this camera's pose — so the game renders through the editor's exact
    // pipeline and play↔stop changes nothing on the GPU (no per-toggle pipeline
    // rebuild = none of the recurring wgpu surface/buffer crashes).
    world.entity_mut(cam_entity).insert(PlayModeCamera);

    // Reset script states and unpause physics (via decoupled events).
    world.trigger(renzora::core::ResetScriptStates);
    world.trigger(renzora::core::UnpausePhysics);

    play_mode.active_game_camera = Some(cam_entity);
    play_mode.state = PlayState::Playing;

    // Clear the editor selection so no gizmo / selection outline draws over the
    // running game.
    if let Some(sel) = world.get_resource::<renzora_editor_framework::EditorSelection>() {
        sel.clear();
    }

    // Optionally maximize the viewport for a clean, full-panel game view. Remember
    // the pre-play maximize state so Stop restores exactly what the user had (in
    // case they'd manually maximized already).
    let maximize = world
        .get_resource::<EditorSettings>()
        .map(|s| s.maximize_viewport_on_play)
        .unwrap_or(false);
    if maximize {
        let was = world
            .get_resource::<renzora_ui::ViewportMaximized>()
            .and_then(|m| m.0);
        world.insert_resource(PrePlayMaximized(was));
        // Maximize the focused viewport for the game view.
        let focused = world
            .get_resource::<renzora::core::viewport_types::Viewports>()
            .map(|v| v.focused)
            .unwrap_or(0);
        world
            .get_resource_or_insert_with(renzora_ui::ViewportMaximized::default)
            .0 = Some(focused);
    }

    let is_2d = world.get::<Camera2d>(cam_entity).is_some();
    if is_2d {
        console_info(
            "PlayMode",
            "Game camera is 2D — the in-panel view is driven by the 3D editor camera, so 2D scenes may not render correctly in-panel yet.",
        );
    }
    console_success(
        "PlayMode",
        format!("=== PLAY MODE ACTIVE (camera: {:?}) ===", cam_entity),
    );
    info!("Entered play mode (camera: {:?})", cam_entity);
}

fn exit_play_mode(world: &mut World, play_mode: &mut PlayModeState) {
    use renzora::core::console_log::*;
    console_info("PlayMode", "=== EXITING PLAY MODE ===");

    // Drop the `PlayModeCamera` marker from the scene camera(s). We never mutated
    // their render pipeline or activation, so there is nothing to tear down — and
    // `drive_editor_camera_in_play` stops driving the editor camera the moment play
    // state clears, so `renzora_camera`'s normal systems snap it back to the editor
    // pose. No GPU teardown on stop = no pipelined-render race = no crash.
    let mut play_cam_q = world.query_filtered::<Entity, With<PlayModeCamera>>();
    let play_cams: Vec<Entity> = play_cam_q.iter(world).collect();
    for entity in play_cams {
        world.entity_mut(entity).remove::<PlayModeCamera>();
    }

    // Restore the pre-play viewport-maximize state (only if play maximized it).
    if let Some(prev) = world.remove_resource::<PrePlayMaximized>() {
        world
            .get_resource_or_insert_with(renzora_ui::ViewportMaximized::default)
            .0 = prev.0;
    }

    // Re-pause physics (via decoupled event).
    world.trigger(renzora::core::PausePhysics);

    // Restore cursor (a gameplay script may have grabbed it).
    let mut cursor_q = world.query::<&mut CursorOptions>();
    if let Ok(mut cursor) = cursor_q.single_mut(world) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }

    play_mode.active_game_camera = None;
    play_mode.state = PlayState::Editing;

    console_success("PlayMode", "=== PLAY MODE EXITED — back to editing ===");
    info!("Exited play mode");
}

/// Enter Simulate mode: run the simulation (scripts + physics + animation) while
/// leaving the editor fully live. Unlike [`enter_play_mode`] this DELIBERATELY
/// does not touch the camera, selection, viewport maximize, or editor chrome —
/// the whole point is to keep editing while things move. The scene is snapshotted
/// first so [`exit_simulate_mode`] can revert any mutation the simulation makes.
fn enter_simulate_mode(world: &mut World, play_mode: &mut PlayModeState) {
    use renzora::core::console_log::*;
    console_info("Simulate", "=== ENTERING SIMULATE MODE ===");

    // Snapshot the scene in-memory (observed by renzora_engine) so Stop restores
    // it. We do this BEFORE unpausing physics so the captured state is the
    // untouched, pre-simulate pose.
    world.trigger(renzora::core::SnapshotSceneForSimulate);

    // Run scripts (and the physics/animation they drive) — same wake-up as Play,
    // minus the camera/chrome changes.
    world.trigger(renzora::core::ResetScriptStates);
    world.trigger(renzora::core::UnpausePhysics);

    play_mode.state = PlayState::Simulating;
    console_success(
        "Simulate",
        "=== SIMULATE ACTIVE (editor stays live; press Stop/Esc to revert) ===",
    );
    info!("Entered simulate mode");
}

/// Exit Simulate mode: re-pause physics, restore the pre-simulate scene snapshot,
/// and return to editing. The editor camera/chrome were never changed, so there
/// is nothing to undo there.
fn exit_simulate_mode(world: &mut World, play_mode: &mut PlayModeState) {
    use renzora::core::console_log::*;
    console_info("Simulate", "=== EXITING SIMULATE MODE ===");

    world.trigger(renzora::core::PausePhysics);

    // Restore the cursor in case a gameplay script grabbed/hid it.
    let mut cursor_q = world.query::<&mut CursorOptions>();
    if let Ok(mut cursor) = cursor_q.single_mut(world) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }

    // Revert every mutation the simulation made (observed by renzora_engine).
    world.trigger(renzora::core::RestoreSimulateSnapshot);

    play_mode.state = PlayState::Editing;
    console_success("Simulate", "=== SIMULATE EXITED — scene restored ===");
    info!("Exited simulate mode");
}
