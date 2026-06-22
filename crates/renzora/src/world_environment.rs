//! `WorldEnvironment` — the unified, owned environment contract type.
//!
//! One entity's worth of "the world's look": background, ambient/IBL, and the
//! shading-coupled screen-space effects (SSAO, SSR, fog, GI). It lives in the
//! shared `renzora` dylib so the host, the reconcile systems, the editor
//! inspector, and `renzora_level_presets` all share ONE `TypeId` across the
//! dlopen boundary.
//!
//! **Residency model (see `docs/world-environment-spec.md`).** Each sub-section
//! carries its own `enabled`. A single `reconcile_world_environment` writer
//! translates these into the **resident** camera-side render components —
//! it NEVER adds/removes them, because they live in PBR's shared mesh-view bind
//! group and toggling their presence at runtime restructures that layout and
//! crashes wgpu. "Off" = the component stays resident but the writer sets a
//! no-op value (and, for the heavier effects, skips the work passes). The layout
//! never changes, so toggling can't crash.
//!
//! Slice 1 ships only [`FogSection`]; the remaining sections (`background`,
//! `ibl`, `ssao`, `ssr`, `gi`) land in later slices.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// The unified per-scene environment. Authored on the "World Environment" entity;
/// consumed by `reconcile_world_environment`.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize, Default)]
#[reflect(Component, Serialize, Deserialize)]
pub struct WorldEnvironment {
    pub fog: FogSection,
    pub ssao: SsaoSection,
    // Future sections — each `{ enabled, ...params }`, resident + gated:
    //   pub background: Background,   // Color | Procedural(atmosphere) | Skybox
    //   pub ibl: IblSection,
    //   pub ssr: SsrSection,
    //   pub gi: GiSection,
}

/// Distance-fog sub-section.
///
/// `enabled` defaults to `false` so a freshly-spawned `WorldEnvironment` matches
/// the stock scene (which ships fog-less). When disabled the reconcile keeps the
/// `DistanceFog` binding resident but sets a no-op falloff — the layout is
/// identical on and off.
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct FogSection {
    pub enabled: bool,
    /// Fog tint (linear, 0..1).
    pub color: [f32; 3],
    /// Directional in-scatter tint (used by the Atmospheric falloff).
    pub directional_light_color: [f32; 3],
    pub directional_light_exponent: f32,
    /// 0 = Linear, 1 = Exponential, 2 = ExponentialSquared, 3 = Atmospheric.
    pub mode: u32,
    pub start: f32,
    pub end: f32,
    pub density: f32,
    pub extinction: [f32; 3],
    pub inscattering: [f32; 3],
}

impl Default for FogSection {
    fn default() -> Self {
        Self {
            enabled: false,
            color: [0.72, 0.78, 0.9],
            directional_light_color: [1.0, 0.92, 0.75],
            directional_light_exponent: 12.0,
            mode: 3,
            start: 50.0,
            end: 800.0,
            density: 0.005,
            extinction: [0.006, 0.005, 0.004],
            inscattering: [0.008, 0.01, 0.014],
        }
    }
}

/// Screen-space ambient occlusion sub-section.
///
/// `enabled` defaults to `true` (the stock scene ships SSAO on). Slice 2 gates
/// it by toggling `ScreenSpaceAmbientOcclusion`'s presence — Bevy has no
/// "no-occlusion" knob, so a resident-and-neutral version (white AO texture +
/// skipped compute) is deferred until/unless toggling proves to crash. See
/// `docs/world-environment-spec.md`.
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct SsaoSection {
    pub enabled: bool,
}

impl Default for SsaoSection {
    fn default() -> Self {
        Self { enabled: true }
    }
}
