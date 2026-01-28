//! Audio system resources
//!
//! Manages audio playback state and queued audio commands.

use bevy::prelude::*;
use std::collections::HashMap;

/// A queued audio command from a script
#[derive(Clone, Debug)]
pub enum AudioCommand {
    PlaySound {
        path: String,
        volume: f32,
        looping: bool,
    },
    PlaySound3D {
        path: String,
        volume: f32,
        position: Vec3,
    },
    PlayMusic {
        path: String,
        volume: f32,
        fade_in: f32,
    },
    StopMusic {
        fade_out: f32,
    },
    StopAllSounds,
    SetMasterVolume {
        volume: f32,
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

/// Marker component for music entities (only one music track at a time)
#[derive(Component)]
pub struct MusicTrack;

/// Marker component for sound effect entities
#[derive(Component)]
pub struct SoundEffect;

/// Marker component for 3D spatial audio entities
#[derive(Component)]
pub struct SpatialSound {
    /// The world position of the sound source
    pub position: Vec3,
}

/// Component for audio fade effects
#[derive(Component, Debug, Clone)]
pub struct AudioFade {
    /// The fade direction
    pub fade_type: FadeType,
    /// Starting volume (0.0 to 1.0)
    pub start_volume: f32,
    /// Target volume (0.0 to 1.0)
    pub target_volume: f32,
    /// Total fade duration in seconds
    pub duration: f32,
    /// Elapsed time in seconds
    pub elapsed: f32,
    /// Whether to despawn the entity when fade completes (for fade-out)
    pub despawn_on_complete: bool,
}

/// Type of audio fade
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadeType {
    FadeIn,
    FadeOut,
}

impl AudioFade {
    /// Create a new fade-in effect
    pub fn fade_in(target_volume: f32, duration: f32) -> Self {
        Self {
            fade_type: FadeType::FadeIn,
            start_volume: 0.0,
            target_volume,
            duration,
            elapsed: 0.0,
            despawn_on_complete: false,
        }
    }

    /// Create a new fade-out effect
    pub fn fade_out(current_volume: f32, duration: f32) -> Self {
        Self {
            fade_type: FadeType::FadeOut,
            start_volume: current_volume,
            target_volume: 0.0,
            duration,
            elapsed: 0.0,
            despawn_on_complete: true,
        }
    }

    /// Calculate the current volume based on elapsed time
    pub fn current_volume(&self) -> f32 {
        if self.duration <= 0.0 {
            return self.target_volume;
        }
        let t = (self.elapsed / self.duration).clamp(0.0, 1.0);
        self.start_volume + (self.target_volume - self.start_volume) * t
    }

    /// Check if the fade is complete
    pub fn is_complete(&self) -> bool {
        self.elapsed >= self.duration
    }

    /// Advance the fade timer
    pub fn tick(&mut self, delta: f32) {
        self.elapsed += delta;
    }
}

/// Resource to control global audio state
#[derive(Resource)]
pub struct AudioState {
    /// Master volume (0.0 to 1.0)
    pub master_volume: f32,
    /// Currently playing music entity
    pub current_music: Option<Entity>,
    /// Map of active sound handles for cleanup
    pub active_sounds: HashMap<Entity, String>,
}

impl Default for AudioState {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            current_music: None,
            active_sounds: HashMap::new(),
        }
    }
}

impl AudioState {
    pub fn effective_volume(&self, base_volume: f32) -> f32 {
        (base_volume * self.master_volume).clamp(0.0, 1.0)
    }
}
