//! The components a scene holds: which effect, and how it is playing.
//!
//! [`ParticleSystem`] persists the effect by **path**: a `Handle` means
//! nothing in a saved scene, so the handle field is skipped by serde and by
//! reflection, and the
//! runtime loads it from the path on every scene load (see
//! [`crate::systems::resolve_handles`]). Code that builds an effect in memory
//! can still assign a handle directly and leave the path empty.
//!
//! [`ParticlePlayback`] persists the authored playback settings (whether it
//! plays, its time scale, what happens when a one-shot ends). The requests that
//! drive it (restart, burst, seek) are transient and are not saved.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::asset::VfxAsset;

/// The effect a new Particle System gets: the built-in campfire.
pub const DEFAULT_EFFECT: &str = "embedded://renzora_particles/presets/campfire.vfx";

/// A particle effect on an entity.
///
/// Requires a [`renzora::PickProxy`] because nothing about an effect can be hit
/// by the viewport's mesh raycast: without it a particle system was visible but
/// could not be clicked.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize, Default)]
#[require(Transform, Visibility, ParticlePlayback, renzora::PickProxy = renzora::PickProxy { radius: 0.3 })]
#[serde(default)]
pub struct ParticleSystem {
    /// Project-relative path of the `.vfx`. This is what a scene stores. An old
    /// `.particle` works here too: it loads through the legacy converter
    /// ([`crate::legacy`]).
    pub path: String,
    /// The loaded effect, resolved from `path`. Assign it directly (with an
    /// empty `path`) for an effect built in code.
    #[reflect(ignore)]
    #[serde(skip)]
    pub asset: Handle<VfxAsset>,
}

impl Default for ParticleSystem {
    fn default() -> Self {
        Self {
            path: DEFAULT_EFFECT.to_string(),
            asset: Handle::default(),
        }
    }
}

impl ParticleSystem {
    pub fn from_path(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            asset: Handle::default(),
        }
    }

    pub fn from_handle(asset: Handle<VfxAsset>) -> Self {
        Self {
            path: String::new(),
            asset,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum PlayState {
    #[default]
    Playing,
    /// Frozen where it is; particles stay on screen.
    Paused,
    /// Cleared and not emitting.
    Stopped,
}

/// What a non-looping effect does once its duration has passed and its last
/// particle has died.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum OneShot {
    /// Nothing: it stays finished until restarted.
    #[default]
    Keep,
    /// Switch to `Stopped`.
    Deactivate,
    /// Despawn the entity. For fire-and-forget effects spawned by gameplay.
    Despawn,
}

/// How an effect is playing.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ParticlePlayback {
    pub state: PlayState,
    /// Multiplies the frame delta. 0 freezes, 2 runs double speed.
    pub time_scale: f32,
    pub one_shot: OneShot,
    #[reflect(ignore)]
    #[serde(skip)]
    pub(crate) restart_requested: bool,
    #[reflect(ignore)]
    #[serde(skip)]
    pub(crate) pending_bursts: u32,
    #[reflect(ignore)]
    #[serde(skip)]
    pub(crate) seek_to: Option<f32>,
    /// Set by an editor that drives the effect's time itself (the timeline's
    /// playhead): `Some(rate)` plays it at that rate whatever `state` and
    /// `time_scale` say, `Some(0.0)` holds it still for seeks, and the one-shot
    /// rule waits. Transient and never saved, which is the point: the editor
    /// takes the clock over without touching what the scene stores, so a scene
    /// saved mid-scrub still plays its effects.
    #[reflect(ignore)]
    #[serde(skip)]
    pub editor_clock: Option<f32>,
}

impl Default for ParticlePlayback {
    fn default() -> Self {
        Self {
            state: PlayState::Playing,
            time_scale: 1.0,
            one_shot: OneShot::Keep,
            restart_requested: false,
            pending_bursts: 0,
            seek_to: None,
            editor_clock: None,
        }
    }
}

impl ParticlePlayback {
    /// Resume, or start from where a stop left it (the beginning).
    pub fn play(&mut self) {
        self.state = PlayState::Playing;
    }

    pub fn pause(&mut self) {
        self.state = PlayState::Paused;
    }

    /// Clear every particle and stop emitting.
    pub fn stop(&mut self) {
        self.state = PlayState::Stopped;
    }

    /// Clear every particle and play from the beginning.
    pub fn restart(&mut self) {
        self.state = PlayState::Playing;
        self.restart_requested = true;
    }

    /// Spawn `count` extra particles from every emitter next frame.
    pub fn burst(&mut self, count: u32) {
        self.pending_bursts = self.pending_bursts.saturating_add(count);
    }

    /// Jump to `seconds` by replaying deterministically from the start.
    pub fn seek(&mut self, seconds: f32) {
        self.seek_to = Some(seconds.max(0.0));
    }
}

/// Values a script (or game code) has given the effect's Parameter nodes, by
/// name: four lanes each, laid out as an inline value of the same shape (a
/// float fills every lane, a Vec3 has alpha 1).
///
/// A component rather than a write into the running simulation because the
/// simulation is rebuilt when its file changes on disk, and a hot reload must
/// not quietly drop what the game set. The runtime copies these into the
/// simulation every frame, so the latest value always wins and a removed one
/// lets the parameter fall back to its keys or its inline value. Not saved: a
/// scene starts with the effect as authored.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct ParticleParams {
    pub values: std::collections::BTreeMap<String, Vec4>,
}

impl ParticleParams {
    pub fn set(&mut self, name: impl Into<String>, lanes: Vec4) {
        self.values.insert(name.into(), lanes);
    }

    pub fn set_float(&mut self, name: impl Into<String>, v: f32) {
        self.set(name, Vec4::splat(v));
    }

    pub fn set_vec3(&mut self, name: impl Into<String>, v: Vec3) {
        self.set(name, v.extend(1.0));
    }

    /// Linear RGBA.
    pub fn set_color(&mut self, name: impl Into<String>, c: Vec4) {
        self.set(name, c);
    }

    pub fn clear(&mut self, name: &str) {
        self.values.remove(name);
    }
}

/// Live numbers for one effect, refreshed every frame. Not saved.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ParticleStats {
    pub alive: u32,
    /// Spawns refused because an emitter was full, since the last restart.
    pub dropped: u64,
    /// Seconds since the effect started.
    pub time: f32,
}
