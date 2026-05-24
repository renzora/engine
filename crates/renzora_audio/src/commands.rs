//! Audio command queue for decoupled audio control
//!
//! External systems (scripting, gameplay, UI) push commands into the queue,
//! and the audio systems drain them each frame.

use bevy::prelude::*;

use crate::components::AudioPlayer;

/// A single audio command to be processed by the audio systems.
#[derive(Clone, Debug)]
pub enum AudioCommand {
    PlaySound {
        path: String,
        volume: f32,
        looping: bool,
        bus: String,
        entity: Option<Entity>,
    },
    /// Play an entity's `AudioPlayer` with all its configured parameters
    /// (volume, pitch, panning, loop region, fade, bus, spatial settings).
    /// `position` is the emitter's world position, used when `spatial` is set.
    PlayEntity {
        entity: Entity,
        player: AudioPlayer,
        position: Vec3,
    },
    PlaySound3D {
        path: String,
        volume: f32,
        position: Vec3,
        bus: String,
        entity: Option<Entity>,
    },
    PlayMusic {
        path: String,
        volume: f32,
        fade_in: f32,
        bus: String,
    },
    StopMusic {
        fade_out: f32,
    },
    StopAllSounds,
    SetMasterVolume {
        volume: f32,
    },
    PauseSound {
        entity: Option<Entity>,
    },
    ResumeSound {
        entity: Option<Entity>,
    },
    SetSoundVolume {
        entity: Entity,
        volume: f32,
        fade: f32,
    },
    SetSoundPitch {
        entity: Entity,
        pitch: f32,
        fade: f32,
    },
    CrossfadeMusic {
        path: String,
        volume: f32,
        duration: f32,
        bus: String,
    },
}

/// Resource holding queued audio commands to be processed each frame.
#[derive(Resource, Default)]
pub struct AudioCommandQueue {
    commands: Vec<AudioCommand>,
}

impl AudioCommandQueue {
    pub fn push(&mut self, cmd: AudioCommand) {
        self.commands.push(cmd);
    }

    pub fn drain(&mut self) -> Vec<AudioCommand> {
        std::mem::take(&mut self.commands)
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}
