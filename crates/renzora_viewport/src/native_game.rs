//! Play-mode wiring for the shared viewport panel.
//!
//! Edit mode and play mode share the primary viewport panel AND its camera —
//! there is no separate "Game" panel or camera. In play mode `renzora_camera`
//! drives the editor viewport camera onto the active game camera's pose
//! (`drive_editor_camera_in_play`), so the viewport renders the running game with
//! the editor's exact pipeline. This module only handles the one piece of UX that
//! lives outside rendering: bringing the viewport tab to the foreground when play
//! starts, so the running game is actually on screen.

use bevy::prelude::*;

use renzora::core::PlayModeState;
use renzora_ember::dock::FocusPanelRequest;

pub fn register(app: &mut App) {
    use renzora_editor_framework::SplashState;
    app.add_systems(
        Update,
        focus_viewport_on_play.run_if(in_state(SplashState::Editor)),
    );
}

/// When play *starts*, bring the viewport tab to the foreground (if it's docked
/// but hidden behind another tab) so the running game is visible — covers every
/// play trigger (top-bar button, Play shortcut). Edge-triggered via a remembered
/// previous state so it only fires on the Editing→Playing transition, leaving the
/// user free to switch tabs afterwards.
fn focus_viewport_on_play(
    play_mode: Option<Res<PlayModeState>>,
    mut focus: ResMut<FocusPanelRequest>,
    mut was_playing: Local<bool>,
) {
    let playing = play_mode.is_some_and(|p| p.is_in_play_mode());
    if playing && !*was_playing {
        focus.0 = Some("viewport".to_string());
    }
    *was_playing = playing;
}
