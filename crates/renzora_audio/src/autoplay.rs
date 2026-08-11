//! AudioPlayer autoplay driver.
//!
//! Bridges the `AudioPlayer` component to actual playback: when the game starts
//! (play mode in the editor, or immediately in a standalone/exported runtime),
//! every entity whose `AudioPlayer.autoplay` is set gets its clip played with
//! the component's configured volume/pitch/loop/bus/spatial settings.
//!
//! Without this system the component is inert data — this is what makes
//! "drop an AudioPlayer on an entity, tick autoplay, set a clip" actually
//! produce sound.

use bevy::prelude::*;

use crate::commands::{AudioCommand, AudioCommandQueue};
use crate::components::AudioPlayer;
use crate::link::AudioLink;

/// Marker inserted once an entity's `AudioPlayer` has been auto-started this
/// play session. Removed when play mode stops so the next play restarts it.
#[derive(Component)]
pub struct AudioAutoplayed;

/// Returns whether the game is "running" (scripts/gameplay active). In a
/// standalone runtime there is no `PlayModeState`, so it's always running.
fn is_running(play_mode: &Option<Res<renzora::PlayModeState>>) -> bool {
    play_mode
        .as_ref()
        .is_none_or(|pm| pm.is_scripts_running())
}

/// Start autoplay clips on play, and stop them when play mode ends.
pub fn audio_player_autoplay(
    mut commands: Commands,
    play_mode: Option<Res<renzora::PlayModeState>>,
    link: Res<AudioLink>,
    pending: Query<(Entity, &AudioPlayer, Option<&GlobalTransform>), Without<AudioAutoplayed>>,
    started: Query<Entity, With<AudioAutoplayed>>,
    mut queue: ResMut<AudioCommandQueue>,
    mut was_running: Local<bool>,
) {
    let running = is_running(&play_mode);

    // Autoplay is a one-shot per session: the marker below is what stops it
    // firing again, and it goes on whether or not the sound actually started.
    // With no backend yet, `PlayEntity` is dropped when the queue is drained —
    // so a frame where the plugin has not been adopted would mark every emitter
    // as done and leave the scene permanently silent. That is not hypothetical:
    // a shipped runtime autoplays on the very first frame, alongside the frame
    // the backend is adopted on. Wait for ears instead of spending the shot.
    if running && !link.is_active() {
        *was_running = running;
        return;
    }

    if running {
        // Play any not-yet-started autoplay emitters. Entities that load in
        // mid-session are picked up here too (the marker, not a one-shot edge,
        // gates replay).
        for (entity, player, xform) in &pending {
            if !player.autoplay || player.clip.is_empty() {
                continue;
            }
            let position = xform.map(|t| t.translation()).unwrap_or(Vec3::ZERO);
            queue.push(AudioCommand::PlayEntity {
                entity,
                player: player.clone(),
                position,
            });
            commands.entity(entity).insert(AudioAutoplayed);
        }
    } else if *was_running {
        // Just left play mode: stop everything and clear markers so a
        // subsequent play restarts the autoplay clips from the top.
        for entity in &started {
            commands.entity(entity).remove::<AudioAutoplayed>();
        }
        queue.push(AudioCommand::StopAllSounds);
    }

    *was_running = running;
}
