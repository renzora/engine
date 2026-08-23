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
//!
//! [`crate::wind::WindSection`] is the odd one out: it is authored here like
//! any other section, but it is not a camera-side render component and so it
//! has nothing to reconcile *onto*. `renzora_wind` reads it into the global
//! `WindState` resource instead — see that module's docs.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// The unified per-scene environment. Authored on the "World Environment" entity;
/// consumed by `reconcile_world_environment`.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize, Default)]
#[reflect(Component, Serialize, Deserialize)]
pub struct WorldEnvironment {
    pub fog: FogSection,
    pub ssao: SsaoSection,
    pub wind: crate::wind::WindSection,
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

/// Sample-count preset for [`SsaoSection`], mirroring Bevy's
/// `ScreenSpaceAmbientOcclusionQualityLevel`.
///
/// Kept as a plain fieldless enum rather than mirroring Bevy's `Custom { .. }`
/// variant: the inspector needs a stable dropdown index, and a variant that
/// carries data would make switching preset → custom → preset lose the counts
/// the user typed. The counts therefore live beside it on [`SsaoSection`] and
/// are simply ignored by every preset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
pub enum SsaoQuality {
    /// 4 samples per pixel.
    Low,
    /// 8 samples per pixel.
    Medium,
    /// 18 samples per pixel — Bevy's default.
    #[default]
    High,
    /// 54 samples per pixel.
    Ultra,
    /// Use [`SsaoSection::slice_count`] / [`SsaoSection::samples_per_slice_side`]
    /// directly instead of a preset pair.
    Custom,
}

impl SsaoQuality {
    /// Dropdown labels, in [`SsaoQuality::index`] order.
    pub const LABELS: [&'static str; 5] = ["Low", "Medium", "High", "Ultra", "Custom"];

    /// Position in [`SsaoQuality::LABELS`] — what the inspector dropdown binds to.
    pub fn index(self) -> usize {
        match self {
            Self::Low => 0,
            Self::Medium => 1,
            Self::High => 2,
            Self::Ultra => 3,
            Self::Custom => 4,
        }
    }

    /// Inverse of [`SsaoQuality::index`]. An out-of-range index falls back to
    /// the default rather than panicking — the dropdown is the only caller, but
    /// a scene file is not a trusted source.
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Self::Low,
            1 => Self::Medium,
            2 => Self::High,
            3 => Self::Ultra,
            4 => Self::Custom,
            _ => Self::default(),
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
///
/// Every field past `enabled` was added after the section shipped, so each
/// carries **both** `#[serde(default = ..)]` and `#[reflect(default = ..)]`:
/// the two load paths are separate, and a scene saved before the field existed
/// fails `FromReflect` outright (naming `WorldEnvironment`, not the field) if
/// only the serde one is present. The defaults are named functions rather than
/// bare `default` because the field types' `Default` is `0`, and silently
/// re-loading an old scene with a zero object thickness would change how it
/// looks.
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct SsaoSection {
    pub enabled: bool,
    /// Sample-count preset. `Custom` hands the two counts below to Bevy.
    #[serde(default)]
    #[reflect(default)]
    pub quality: SsaoQuality,
    /// Slices per pixel under [`SsaoQuality::Custom`]. More slices means less
    /// noise and a proportionally more expensive pass.
    #[serde(default = "default_slice_count")]
    #[reflect(default = "default_slice_count")]
    pub slice_count: u32,
    /// Samples per slice side under [`SsaoQuality::Custom`]. Bevy recommends
    /// leaving this at 2 or 3.
    #[serde(default = "default_samples_per_slice_side")]
    #[reflect(default = "default_samples_per_slice_side")]
    pub samples_per_slice_side: u32,
    /// Assumed thickness (world units) of the geometry behind the depth buffer.
    /// A ray passing further behind a surface than this is treated as missing it
    /// rather than being occluded by it, so raising it darkens thin geometry and
    /// lowering it thins out haloes behind foreground objects.
    #[serde(default = "default_object_thickness")]
    #[reflect(default = "default_object_thickness")]
    pub constant_object_thickness: f32,
}

fn default_slice_count() -> u32 {
    3
}

fn default_samples_per_slice_side() -> u32 {
    3
}

fn default_object_thickness() -> f32 {
    0.25
}

impl Default for SsaoSection {
    fn default() -> Self {
        Self {
            enabled: true,
            quality: SsaoQuality::default(),
            slice_count: default_slice_count(),
            samples_per_slice_side: default_samples_per_slice_side(),
            constant_object_thickness: default_object_thickness(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scenes reconstruct components through `FromReflect`, whose derive reads
    /// every field out of the dynamic struct with `?` — so a field absent from
    /// the saved data makes the *whole* reconstruction return `None` unless the
    /// field is `#[reflect(default = ..)]`. Bevy reports that as "couldn't
    /// create an instance of `WorldEnvironment`", naming the container and not
    /// the field, so it does not point at the change that caused it.
    ///
    /// This asserts the SSAO knobs added after the section shipped default
    /// instead of failing the load — and that they default to Bevy's own values,
    /// not to `0`, so an old scene keeps the look it was saved with.
    #[test]
    fn a_scene_without_the_new_ssao_fields_reconstructs_by_reflection() {
        use bevy::reflect::structs::DynamicStruct;
        use bevy::reflect::FromReflect;

        // What a scene saved before the quality knobs deserializes to: the one
        // field the section had, and nothing at all for the four added since.
        let mut old_ssao = DynamicStruct::default();
        old_ssao.insert("enabled", true);

        let ssao = SsaoSection::from_reflect(&old_ssao)
            .expect("missing SSAO knobs must default, not fail the load");
        assert!(ssao.enabled);
        assert_eq!(ssao.quality, SsaoQuality::High);
        assert_eq!(ssao.slice_count, 3);
        assert_eq!(ssao.samples_per_slice_side, 3);
        assert_eq!(ssao.constant_object_thickness, 0.25);
    }

    /// The dropdown binds to an index in both directions, so the two halves have
    /// to agree — and an index out of range (a hand-edited scene, a label list
    /// that grew) must land on the default rather than panic.
    #[test]
    fn quality_index_round_trips_and_clamps() {
        for q in [
            SsaoQuality::Low,
            SsaoQuality::Medium,
            SsaoQuality::High,
            SsaoQuality::Ultra,
            SsaoQuality::Custom,
        ] {
            assert_eq!(SsaoQuality::from_index(q.index()), q);
            assert!(q.index() < SsaoQuality::LABELS.len());
        }
        assert_eq!(SsaoQuality::from_index(99), SsaoQuality::default());
    }
}
