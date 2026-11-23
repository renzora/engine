//! Wind — one world-global wind, in the contract crate so every consumer agrees.
//!
//! Before this existed the engine had four unrelated winds: a hardcoded
//! `Vec2` in the grass renderer, per-cascade `wind_speed`/`wind_direction` on
//! the ocean, `speed` + `wind_direction` on the cloud deck, and `bevy_silk`'s
//! cloth `Winds` resource that nothing ever wrote. Each was authored
//! separately, so a scene could have grass leaning east under clouds drifting
//! west. [`WindSection`] is the single authored value and [`WindState`] is the
//! evaluated per-frame result every one of them now reads.
//!
//! # Why two types
//!
//! [`WindSection`] is *authored* — it lives on the `WorldEnvironment` entity,
//! serializes into the scene, and is what the inspector edits. [`WindState`] is
//! *derived* — a resource `renzora_wind` rewrites every frame, holding the
//! evaluated gust envelope and the smoothed sea state alongside the raw
//! parameters. Consumers read the resource so none of them has to re-derive the
//! gust curve (and so none of them can derive it *differently*, which is how the
//! grass and the clouds drifted apart in the first place).
//!
//! # Response times differ, deliberately
//!
//! Wind does not reach every system on the same clock, because the systems
//! genuinely do not respond on the same clock:
//!
//! * **Foliage** tracks the instantaneous value including gusts. A leaf responds
//!   to a gust in well under a second.
//! * **Cloth and particles** track the instantaneous value too, as a force.
//! * **The ocean** tracks [`WindState::sea_state_speed`], a heavily smoothed
//!   value. That is not a performance dodge — `wind_speed` is a JONSWAP spectrum
//!   input, so changing it rebuilds the cascade textures, and a real sea takes
//!   hours to build to a new wind. Smoothing is both cheaper *and* more correct.
//! * **The cloud deck** scales its drift off the sustained speed, ignoring
//!   gusts. Cloud features are kilometres across; a two-second gust does not
//!   move them.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Wind speed, in m/s, that foliage treats as "full deflection" — i.e. the
/// speed at which [`WindState::strength01`] reaches 1.0.
///
/// 12 m/s is a fresh-to-strong breeze (Beaufort 6): large branches in motion,
/// which is about as far as a tree bends before the motion stops reading as
/// wind and starts reading as a storm. Speeds above this are not clamped —
/// `strength01` keeps climbing — they simply push past the tuned range.
pub const REFERENCE_WIND_SPEED: f32 = 12.0;

/// The authored wind sub-section of `WorldEnvironment`.
///
/// `enabled` defaults to `true` with a light breeze: a scene with grass and
/// trees looks *wrong* dead still, so the useful default is gentle motion
/// rather than none. Setting `enabled = false` zeroes [`WindState`] rather than
/// freezing it at the last value — consumers read one number and never have to
/// check a flag.
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Serialize, Deserialize)]
pub struct WindSection {
    pub enabled: bool,
    /// Compass bearing the wind blows **toward**, in degrees. 0 = +X, 90 = +Z.
    ///
    /// "Toward", not "from" — meteorology reports the direction wind comes
    /// *from*, which is the opposite of what every shader wants and the source
    /// of a sign error every single time. The engine stores travel direction.
    pub direction: f32,
    /// Sustained wind speed in m/s, before gusts. See [`REFERENCE_WIND_SPEED`].
    pub speed: f32,
    /// Gust depth, 0–1. The fraction by which a gust lifts the speed above
    /// sustained: 0.5 means gusts peak at 1.5× `speed`. 0 = perfectly steady.
    pub gust_strength: f32,
    /// Gusts per second. Around 0.15 (a gust every ~7 s) reads as natural
    /// outdoor wind; above ~1 it stops being gusting and becomes buffeting.
    pub gust_frequency: f32,
    /// Chaotic cross-wind component, 0–1. Drives the perpendicular wobble that
    /// keeps foliage from all swinging along one axis like a metronome.
    pub turbulence: f32,
}

impl Default for WindSection {
    fn default() -> Self {
        Self {
            enabled: true,
            direction: 25.0,
            speed: 4.0,
            gust_strength: 0.45,
            gust_frequency: 0.15,
            turbulence: 0.35,
        }
    }
}

/// Mark a mesh as wind-animated.
///
/// `renzora_wind` swaps the entity's `StandardMaterial` for a wind-animated
/// variant (the same material plus a vertex stage); removing this component
/// swaps it back. Lives here rather than in `renzora_wind` because the crates
/// that *tag* geometry — the procedural tree generator, importers, gameplay
/// code — must not each link the wind plugin to do it.
///
/// How far a given vertex moves comes from the mesh's `UV_1` attribute
/// (`x` = sway weight, `y` = leaf flutter weight); the procedural tree
/// generator writes those. A mesh without `UV_1` falls back to a height ramp
/// over [`pivot_height`](Self::pivot_height), which is good enough for a bush
/// and wrong for anything with a long horizontal branch.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct WindSway {
    pub enabled: bool,
    /// Multiplies the world wind for this mesh. A stiff shrub is ~0.4, a
    /// willow ~1.6. This is the knob for "same wind, different plant".
    pub response: f32,
    /// Scales the high-frequency leaf flutter independently of the branch
    /// sway. 0 on woody geometry that should only bend.
    pub flutter: f32,
    /// How far the floppiest vertex travels at reference wind, in metres.
    /// Amplitude is authored in world units rather than derived from the
    /// mesh's bounds, because bounds change when a tree regenerates and the
    /// motion should not.
    pub amplitude: f32,
    /// Fallback only: height above the object origin at which a mesh with no
    /// `UV_1` weights is considered fully flexible.
    pub pivot_height: f32,
}

impl Default for WindSway {
    fn default() -> Self {
        Self {
            enabled: true,
            response: 1.0,
            flutter: 1.0,
            amplitude: 0.35,
            pivot_height: 3.0,
        }
    }
}

/// The evaluated wind for this frame. Written by `renzora_wind`, read by
/// everything that moves.
///
/// `Copy` on purpose: consumers take it as `Option<Res<WindState>>` and fall
/// back to [`WindState::default`] (dead calm) when the wind plugin isn't in the
/// build, so a stripped lean export doesn't need every foliage system guarded.
#[derive(Resource, Clone, Copy, Debug, Reflect)]
#[reflect(Resource)]
pub struct WindState {
    /// Unit travel direction on the XZ plane.
    pub direction: Vec2,
    /// Sustained speed, m/s. Already zeroed when the section is disabled.
    pub speed: f32,
    /// Gust depth, 0–1 (see [`WindSection::gust_strength`]).
    pub gust_strength: f32,
    /// Gusts per second.
    pub gust_frequency: f32,
    /// Cross-wind chaos, 0–1.
    pub turbulence: f32,
    /// This frame's gust envelope, 0–1. CPU consumers (cloth, particles) use
    /// this; GPU consumers re-derive an equivalent curve in the shader from
    /// `globals.time` so the motion stays smooth between uniform writes.
    pub gust: f32,
    /// Sustained speed smoothed over minutes, for the ocean spectrum. See the
    /// module docs on why this one lags.
    pub sea_state_speed: f32,
}

impl Default for WindState {
    /// Dead calm — the value consumers see when no wind plugin is present.
    fn default() -> Self {
        Self {
            direction: Vec2::X,
            speed: 0.0,
            gust_strength: 0.0,
            gust_frequency: 0.0,
            turbulence: 0.0,
            gust: 0.0,
            sea_state_speed: 0.0,
        }
    }
}

impl WindState {
    /// Sustained strength normalized so [`REFERENCE_WIND_SPEED`] maps to 1.0.
    /// This is the number shader-side foliage scales its deflection by.
    pub fn strength01(&self) -> f32 {
        self.speed / REFERENCE_WIND_SPEED
    }

    /// Instantaneous strength including this frame's gust — the CPU-side
    /// equivalent of what the foliage shaders compute per-vertex.
    pub fn gusting_strength01(&self) -> f32 {
        self.strength01() * (1.0 + self.gust * self.gust_strength)
    }

    /// Instantaneous wind velocity in m/s as a world vector (always horizontal).
    /// This is the force input for cloth, hair and particles.
    pub fn velocity(&self) -> Vec3 {
        let s = self.speed * (1.0 + self.gust * self.gust_strength);
        Vec3::new(self.direction.x, 0.0, self.direction.y) * s
    }

    /// Bearing in degrees, the inverse of [`WindSection::direction`]. Consumers
    /// that store an angle rather than a vector (the cloud deck) use this so
    /// they don't each re-derive `atan2` with a different axis convention.
    pub fn direction_degrees(&self) -> f32 {
        self.direction.y.atan2(self.direction.x).to_degrees()
    }
}
