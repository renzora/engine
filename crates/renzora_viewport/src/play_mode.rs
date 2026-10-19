//! Play and Stop: start the game as its own process, or end it.
//!
//! There is one way to play. The editor used to offer four (in the viewport,
//! in a window, in a VR headset, and a "Scripts" simulate mode), and three of
//! them ran the game inside the editor's own world, where a slow game made a
//! slow editor. Now the editor's `PlayModeState` stays `Editing` for good and
//! the game lives in the child process `external_runtime` manages. The request
//! flags on `PlayModeState` remain the way the Play button and shortcuts ask.

use bevy::prelude::*;
use renzora::core::PlayModeState;

use crate::external_runtime::{
    self, find_runtime_binary, replace_child, spawn_runtime, ExternalRuntime,
};

/// Handles Play/Stop requests each frame.
///
/// While the game is running, *every* request means Stop. That keeps a second
/// press of Play, or a shortcut that only knows "play", from starting a
/// second instance beside the first.
pub fn handle_play_mode_transitions(world: &mut World) {
    use renzora::core::console_log::*;

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
        .get_resource::<renzora::CurrentProject>()
        .map(|p| p.path.clone())
    else {
        console_error("PlayMode", "Play failed: no project is open");
        return;
    };

    // The game reads the scene from disk, so unsaved edits would otherwise be
    // invisible to it.
    world.trigger(renzora::core::SaveCurrentScene);

    console_info(
        "PlayMode",
        format!(
            "Starting the game: {} --no-editor --project {}",
            binary.display(),
            project_path.display(),
        ),
    );
    let embed_parent = crate::game_panel::embed_parent(world);
    match spawn_runtime(&binary, &project_path, embed_parent) {
        Ok(child) => {
            if let Some(mut runtime) = world.get_resource_mut::<ExternalRuntime>() {
                replace_child(&mut runtime, child);
                runtime.begin_preparing();
            }
            if embed_parent.is_some() {
                show_game_panel(world);
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

/// Bring the Game panel to the front, adding it beside the viewport when the
/// layout doesn't have one, so the game is on screen when it starts.
///
/// Adding the tab must also arm [`DockDirty`](renzora_ember::dock::DockDirty).
/// Without it the tree said the Game tab was the active one while the UI was
/// never rebuilt: the viewport, no longer its leaf's active tab, dropped to the
/// small stub target a hidden viewport renders at and showed as a blurry
/// square, and the Game panel the game's window is placed over never existed.
fn show_game_panel(world: &mut World) {
    use crate::game_panel::GAME_PANEL_ID;
    use renzora_ember::dock::{Dock, DockDirty, FixedDock, FocusPanelRequest};

    let present = world
        .get_resource::<Dock>()
        .is_some_and(|d| d.tree.contains_panel(GAME_PANEL_ID))
        || world
            .get_resource::<FixedDock>()
            .is_some_and(|f| f.tree.contains_panel(GAME_PANEL_ID));
    if present {
        if let Some(mut focus) = world.get_resource_mut::<FocusPanelRequest>() {
            focus.0 = Some(GAME_PANEL_ID.to_string());
        }
        return;
    }
    if let Some(mut dock) = world.get_resource_mut::<Dock>() {
        if !dock.tree.add_tab("viewport", GAME_PANEL_ID.to_string()) {
            dock.tree.focus_or_add_panel(GAME_PANEL_ID);
        }
    }
    if let Some(mut dirty) = world.get_resource_mut::<DockDirty>() {
        dirty.0 = true;
    }
}
