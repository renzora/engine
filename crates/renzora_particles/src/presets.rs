//! The built-in effects, compiled into the binary.
//!
//! Embedded rather than shipped under `assets/` so they exist in every build,
//! including an exported game that never copied the engine's asset folder, and
//! so a scene that references one keeps working wherever it is opened. Each is
//! registered as an embedded asset, which gives it a stable
//! `embedded://renzora_particles/presets/<file>` path the scene saves like any
//! other.

/// A built-in effect.
pub struct Preset {
    pub name: &'static str,
    /// The `embedded://` path a `ParticleSystem` references it by.
    pub path: &'static str,
    /// The RON source, for tests and for the editor to copy into a project.
    pub source: &'static str,
}

pub const PRESETS: &[Preset] = &[
    Preset {
        name: "Campfire",
        path: "embedded://renzora_particles/presets/campfire.vfx",
        source: include_str!("presets/campfire.vfx"),
    },
    Preset {
        name: "Sparks Burst",
        path: "embedded://renzora_particles/presets/sparks.vfx",
        source: include_str!("presets/sparks.vfx"),
    },
    Preset {
        name: "Smoke",
        path: "embedded://renzora_particles/presets/smoke.vfx",
        source: include_str!("presets/smoke.vfx"),
    },
    Preset {
        name: "Confetti",
        path: "embedded://renzora_particles/presets/confetti.vfx",
        source: include_str!("presets/confetti.vfx"),
    },
    Preset {
        name: "Magic Sparkles",
        path: "embedded://renzora_particles/presets/sparkles.vfx",
        source: include_str!("presets/sparkles.vfx"),
    },
    // Last, so the older presets keep their indices.
    Preset {
        name: "Realistic Fire",
        path: "embedded://renzora_particles/presets/realistic_fire.vfx",
        source: include_str!("presets/realistic_fire.vfx"),
    },
];

/// Register every preset with the embedded asset source.
pub fn embed(app: &mut bevy::app::App) {
    bevy::asset::embedded_asset!(app, "presets/campfire.vfx");
    bevy::asset::embedded_asset!(app, "presets/realistic_fire.vfx");
    bevy::asset::embedded_asset!(app, "presets/sparks.vfx");
    bevy::asset::embedded_asset!(app, "presets/smoke.vfx");
    bevy::asset::embedded_asset!(app, "presets/confetti.vfx");
    bevy::asset::embedded_asset!(app, "presets/sparkles.vfx");
}
