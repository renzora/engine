//! The particle components a scene holds and a script drives.
//!
//! A layer of the contract hierarchy, so a Rust script or a native plugin can
//! place, play and burst an effect by naming these types: they reach it as
//! `renzora::particles`. Before this layer existed the components lived only
//! in `renzora_particles`, which nothing outside the engine can link, so a
//! script could poke an effect already in the scene through the `vfx_*`
//! actions but could never spawn one, which is most of what a game wants from
//! particles (a hit spark, an explosion, a trail on a projectile).
//!
//! Only authored data lives here. The loaded effect asset, the compiled
//! program and the simulation are runtime state that `renzora_particles` keeps
//! on components of its own, built from these the way a physics backend
//! builds its bodies from `PhysicsBodyData`.
//!
//! The types keep the `renzora_particles::component` type path they were saved
//! under, so scenes written before the move load unchanged.
//!
//! ```ignore
//! use renzora::particles::{ParticlePlayback, ParticleSystem};
//!
//! ctx.world().spawn((
//!     Name::new("hit_sparks"),
//!     Transform::from_translation(position),
//!     ParticleSystem::from_path("particles/hit_sparks.vfx"),
//!     // Plays once, then despawns the entity.
//!     ParticlePlayback::one_shot(),
//! ));
//! ```

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// The effect a new Particle System gets: the built-in campfire.
pub const DEFAULT_EFFECT: &str = "embedded://renzora_particles/presets/campfire.vfx";

/// A particle effect on an entity, by the project-relative path of its `.vfx`.
///
/// This is all a scene stores about an effect. An old `.particle` works here
/// too: the runtime loads it through the legacy converter.
///
/// Requires a [`ParticlePlayback`], so spawning this alone plays the effect.
#[derive(Component, Clone, Debug, PartialEq, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize, Default)]
#[type_path = "renzora_particles::component"]
#[require(Transform, Visibility, ParticlePlayback)]
#[serde(default)]
pub struct ParticleSystem {
    /// Project-relative path of the `.vfx`. Empty when the runtime was handed
    /// an effect built in code instead.
    pub path: String,
}

impl Default for ParticleSystem {
    fn default() -> Self {
        Self { path: DEFAULT_EFFECT.to_string() }
    }
}

impl ParticleSystem {
    pub fn from_path(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
#[type_path = "renzora_particles::component"]
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
#[type_path = "renzora_particles::component"]
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
///
/// The authored settings (state, time scale, the one-shot rule) are saved.
/// The requests (restart, burst, seek) are transient: they are queued here and
/// the runtime takes them on its next update.
///
/// Build one with `default()` or [`ParticlePlayback::one_shot`] and set the
/// public fields; the queued requests are private, so struct-update syntax
/// (`..default()`) does not compile outside this crate.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize, Default)]
#[type_path = "renzora_particles::component"]
#[serde(default)]
pub struct ParticlePlayback {
    pub state: PlayState,
    /// Multiplies the frame delta. 0 freezes, 2 runs double speed.
    pub time_scale: f32,
    pub one_shot: OneShot,
    #[reflect(ignore)]
    #[serde(skip)]
    restart_requested: bool,
    #[reflect(ignore)]
    #[serde(skip)]
    pending_bursts: u32,
    #[reflect(ignore)]
    #[serde(skip)]
    seek_to: Option<f32>,
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
    /// A playing effect that despawns its entity when it finishes: the usual
    /// setting for an effect spawned by gameplay.
    pub fn one_shot() -> Self {
        Self { one_shot: OneShot::Despawn, ..default() }
    }

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

    /// For the runtime: whether a restart was asked for since it last looked,
    /// clearing the request.
    pub fn take_restart(&mut self) -> bool {
        std::mem::take(&mut self.restart_requested)
    }

    /// For the runtime: the bursts asked for since it last looked, clearing
    /// them.
    pub fn take_bursts(&mut self) -> u32 {
        std::mem::take(&mut self.pending_bursts)
    }

    /// For the runtime: the seek asked for since it last looked, clearing it.
    pub fn take_seek(&mut self) -> Option<f32> {
        self.seek_to.take()
    }

    /// Whether a request is waiting, without taking it. Lets the runtime skip
    /// a mutable borrow (and its change tick) on the frames nothing was asked.
    pub fn has_requests(&self) -> bool {
        self.restart_requested || self.pending_bursts > 0 || self.seek_to.is_some()
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

/// Live numbers for one effect, refreshed every frame by the runtime. Not
/// saved.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ParticleStats {
    pub alive: u32,
    /// Spawns refused because an emitter was full, since the last restart.
    pub dropped: u64,
    /// Seconds since the effect started.
    pub time: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Saved scenes name these types by path; the move into this layer must
    /// not change what they are called on disk.
    #[test]
    fn type_paths_are_the_ones_scenes_were_saved_with() {
        assert_eq!(
            <ParticleSystem as TypePath>::type_path(),
            "renzora_particles::component::ParticleSystem"
        );
        assert_eq!(
            <ParticlePlayback as TypePath>::type_path(),
            "renzora_particles::component::ParticlePlayback"
        );
        assert_eq!(<PlayState as TypePath>::type_path(), "renzora_particles::component::PlayState");
        assert_eq!(<OneShot as TypePath>::type_path(), "renzora_particles::component::OneShot");
    }

    #[test]
    fn requests_are_taken_once() {
        let mut p = ParticlePlayback::default();
        assert!(!p.has_requests());
        p.burst(3);
        p.burst(2);
        p.restart();
        p.seek(1.5);
        assert!(p.has_requests());
        assert_eq!(p.take_bursts(), 5);
        assert!(p.take_restart());
        assert_eq!(p.take_seek(), Some(1.5));
        assert!(!p.has_requests());
        assert_eq!(p.take_bursts(), 0);
    }
}
