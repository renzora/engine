//! Timer system for scripts
//!
//! Updates all script timers each frame.

use bevy::prelude::*;
use crate::scripting::resources::ScriptTimers;
use crate::core::PlayModeState;

/// System to update all script timers
pub fn update_script_timers(
    time: Res<Time>,
    play_mode: Res<PlayModeState>,
    mut timers: ResMut<ScriptTimers>,
) {
    // Only tick timers during play mode
    if !play_mode.is_scripts_running() {
        return;
    }

    timers.tick_all(time.delta_secs());
}

/// System to clear timers when exiting play mode
pub fn clear_timers_on_stop(
    play_mode: Res<PlayModeState>,
    mut timers: ResMut<ScriptTimers>,
    mut last_playing: Local<bool>,
) {
    let currently_playing = play_mode.is_in_play_mode();

    // Detect transition from playing to editing
    if *last_playing && !currently_playing {
        timers.clear();
    }

    *last_playing = currently_playing;
}
