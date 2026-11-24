//! Play and Stop: start the game as its own process, or end it.
//!
//! There is one way to play. The editor used to offer four (in the viewport,
//! in a window, in a VR headset, and a "Scripts" simulate mode), and three of
//! them ran the game inside the editor's own world, where a slow game made a
//! slow editor. Now the editor's `PlayModeState` stays `Editing` for good and
//! the game lives in the child process `external_runtime` manages. The request
//! flags on `PlayModeState` remain the way the Play button and shortcuts ask.
//!
//! The game opens in a window of its own, exactly as an exported game runs. It
//! used to be placed inside the editor in a Game panel, and that made the game
//! something other than what ships: window size and mode were withheld there,
//! the editor's and the game's UI fought over focus and input, and the editor's
//! copy of the game's widgets fed back into the game. The editor still follows
//! and steers the running game over the live link (`renzora_live_link`).

use bevy::prelude::*;
use renzora_project_api::PlayModeState;

use crate::external_runtime::{
    self, find_runtime_binary, replace_child, spawn_runtime, ExternalRuntime,
};

/// Run condition for [`handle_play_mode_transitions`]: any request flag is set.
/// The system clears all four, so all four are here, not only the two it acts
/// on. Reading them through `Res` also stops the system from marking
/// `PlayModeState` changed on every frame, which taking it mutably did.
pub fn play_request_pending(play: Option<Res<PlayModeState>>) -> bool {
    play.is_some_and(|p| p.request_play || p.request_stop || p.request_simulate || p.request_pause)
}

/// Handles Play/Stop requests each frame.
///
/// While the game is running, *every* request means Stop. That keeps a second
/// press of Play, or a shortcut that only knows "play", from starting a
/// second instance beside the first.
pub fn handle_play_mode_transitions(world: &mut World) {
    use renzora_log_api::*;

    let Some(mut play_mode) = world.get_resource_mut::<PlayModeState>() else {
        return;
    };
    let requested_play = std::mem::take(&mut play_mode.request_play);
    let requested_stop = std::mem::take(&mut play_mode.request_stop);
    play_mode.request_simulate = false;
    play_mode.request_pause = false;
    if !requested_play && !requested_stop {
        return;
    }

    let alive = world
        .get_resource::<ExternalRuntime>()
        .is_some_and(|r| r.is_alive());
    if alive {
        if let Some(mut runtime) = world.get_resource_mut::<ExternalRuntime>() {
            if external_runtime::kill_runtime(&mut runtime) {
                console_info("PlayMode", "Game stopped");
            }
        }
        return;
    }
    if !requested_play {
        return;
    }

    let Some(binary) = find_runtime_binary() else {
        console_error("PlayMode", "Play failed: no game executable was found");
        return;
    };
    let Some(project_path) = world
        .get_resource::<renzora_project_api::CurrentProject>()
        .map(|p| p.path.clone())
    else {
        console_error("PlayMode", "Play failed: no project is open");
        return;
    };

    // The game reads the scene from disk, so unsaved edits would otherwise be
    // invisible to it.
    world.trigger(renzora_shell_api::SaveCurrentScene);

    console_info(
        "PlayMode",
        format!(
            "Starting the game: {} --no-editor --project {}",
            binary.display(),
            project_path.display(),
        ),
    );
    let live_link = world.get_resource::<renzora_project_api::LiveLinkEndpoint>().cloned();
    // The editor's own profiling key, so the companion can profile the game.
    let profiling_key = world
        .get_resource::<renzora_diagnostics_api::profiling::ProfilingAccess>()
        .map(|a| a.key.clone());
    match spawn_runtime(&binary, &project_path, live_link.as_ref(), profiling_key.as_deref()) {
        Ok(child) => {
            if let Some(mut runtime) = world.get_resource_mut::<ExternalRuntime>() {
                replace_child(&mut runtime, child);
                runtime.begin_preparing();
            }
        }
        Err(e) => console_error("PlayMode", format!("Failed to start the game: {e}")),
    }
}

/// Mirror whether the game process is alive into
/// [`PlayModeState::game_launched`], for crates that can't see
/// `ExternalRuntime`. Writes only on a change, so readers' change detection
/// means something.
pub fn sync_game_launched(runtime: Res<ExternalRuntime>, mut play_mode: ResMut<PlayModeState>) {
    let alive = runtime.is_alive();
    if play_mode.game_launched != alive {
        play_mode.game_launched = alive;
    }
}
