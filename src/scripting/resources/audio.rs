//! Audio system resources
//!
//! Manages audio playback state and queued audio commands.

use bevy::prelude::*;

/// A queued audio command from a script
#[derive(Clone, Debug)]
pub enum AudioCommand {
    PlaySound {
        path: String,
        volume: f32,
        looping: bool,
        bus: String,
        entity: Option<Entity>,
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

/// Resource to queue audio commands from scripts
#[derive(Resource, Default)]
pub struct AudioCommandQueue {
    pub commands: Vec<AudioCommand>,
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
