//! The project's graphics-quality tier, and what each tier does to an effect.
//!
//! A project setting (`ProjectConfig::graphics_quality`), so it lives beside the
//! project config rather than with the viewport types it used to sit among.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Overall graphics-quality tier. A single user-facing switch that gates the
/// expensive, *fullscreen / resolution-bound* render passes — the ones whose
/// cost is per-pixel, not per-object, and so dominate on weak GPUs and high-DPI
/// (Retina) displays even on an empty scene.
///
/// Each tier maps to a set of crash-safe `enabled` toggles on the routed effect
/// sources (SSAO, auto-exposure, bloom, TAA); see
/// `renzora_engine::graphics_quality`. It deliberately does **not** touch
/// passes whose attachment layout is fixed at camera spawn (atmosphere / the
/// prepass bundle) — toggling those at runtime trips a wgpu validation crash, so
/// they stay resident regardless of tier.
///
/// The ladder removes the next-most-expensive pass at each step down:
/// - `High`   — everything on (the full authored look).
/// - `Medium`: SSAO off and the cheaper sky; the tonemapped look
///   (auto-exposure, bloom, TAA) is kept. **Default**: it's the best
///   out-of-box trade for the low-end / Retina machines that motivated this.
/// - `Low`: auto-exposure, bloom, TAA and clouds off too; the lightest path,
///   roughly a "compatibility" renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect, Serialize, Deserialize)]
#[reflect(Serialize, Deserialize)]
pub enum GraphicsQuality {
    Low,
    /// The shipping default: it sheds the heaviest fullscreen passes while
    /// keeping the tonemapped look, so the engine runs acceptably on the kind of older /
    /// integrated GPUs where the full stack drops to single-digit FPS. Capable
    /// machines can raise it to `High` in Settings > Project > Rendering.
    #[default]
    Medium,
    High,
}

impl GraphicsQuality {
    pub const ALL: &'static [GraphicsQuality] = &[Self::Low, Self::Medium, Self::High];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }

    /// Parse a persisted label back into a tier. Unknown / empty (an older config
    /// written before this field existed) resolves to the default `Medium`, so
    /// upgrading projects inherit the lighter default.
    pub fn from_label(label: &str) -> Self {
        match label {
            "Low" => Self::Low,
            "High" => Self::High,
            _ => Self::Medium,
        }
    }

    /// Auto-exposure histogram pass — on at `Medium` and `High`.
    pub fn auto_exposure(&self) -> bool {
        !matches!(self, Self::Low)
    }

    /// Bloom downsample/upsample chain — on at `Medium` and `High`.
    pub fn bloom(&self) -> bool {
        !matches!(self, Self::Low)
    }

    /// Temporal anti-aliasing — on at `Medium` and `High`.
    pub fn taa(&self) -> bool {
        !matches!(self, Self::Low)
    }

    /// Screen-space ambient occlusion (GTAO) — on only at `High`.
    ///
    /// Gated for the same reason as the rest: it is a fullscreen,
    /// resolution-bound pass, and profiling put it **second only to the deferred
    /// prepass** among GPU passes (0.46 ms of a 2.63 ms GPU frame on a discrete
    /// card — proportionally far worse on the integrated GPUs `Low` exists for).
    /// It was previously ungated, so picking `Low` explicitly for frame rate
    /// still paid for it.
    ///
    /// It is `High`-only rather than sitting with bloom/TAA at `Medium`
    /// because its three full-res compute passes are exactly the
    /// "fullscreen, resolution-bound" cost class `Medium` — the weak-machine
    /// default — exists to shed.
    pub fn ssao(&self) -> bool {
        matches!(self, Self::High)
    }

    /// Screen-space indirect lighting: on only at `High`.
    ///
    /// It is global illumination, the cost class `Medium` exists to shed: a
    /// half-resolution gather of a dozen taps per pixel, two blur passes and a
    /// full-resolution composite, every frame, whatever the scene holds.
    pub fn ssil(&self) -> bool {
        matches!(self, Self::High)
    }

    /// Whether the procedural cloud dome renders at all. Off at `Low` (its
    /// full-screen FBM shader is the single largest scene-independent raster
    /// cost on a weak GPU); on at `Medium` and `High`.
    pub fn clouds(&self) -> bool {
        !matches!(self, Self::Low)
    }

    /// Atmosphere render method: `true` = Bevy's `Raymarched` sky (a 16-step,
    /// ~80-fetch raymarch on every pixel), `false` = the `LookupTexture` sky
    /// (~2 fetches/pixel). Only `High` pays for the raymarch; `Low`/`Medium`
    /// get the ~40× cheaper lookup path, which for a static sky is visually
    /// near-identical.
    pub fn atmosphere_raymarched(&self) -> bool {
        matches!(self, Self::High)
    }

    /// Cube-face size of the atmosphere-derived IBL probe that Bevy re-bakes and
    /// re-prefilters **every frame** (there is no dirty check upstream, so the
    /// cost is fixed per frame and scales with the square of this number). The
    /// default `512` is far more than a blurry procedural-sky reflection needs;
    /// dropping the tiers cuts the dominant fixed GPU cost 4–16×.
    pub fn ibl_face_size(&self) -> u32 {
        match self {
            Self::High => 256,
            Self::Medium => 128,
            Self::Low => 64,
        }
    }

    /// Per-cascade directional shadow-map resolution (`DirectionalLightShadowMap`).
    /// Bevy's default is 2048, and each of the (up to 4) cascades allocates and
    /// clears a `size × size` depth target **every frame regardless of geometry**
    /// — pure bandwidth, which is exactly what a shared-memory iGPU chokes on.
    /// Halving the size quarters the per-cascade depth traffic; `High` keeps the
    /// crisp default.
    pub fn shadow_map_size(&self) -> usize {
        match self {
            Self::High => 2048,
            Self::Medium => 1024,
            Self::Low => 512,
        }
    }

    /// What this tier does to the effect behind the inspector section registered
    /// under `type_id`, if anything. Drives the amber warning the inspector puts
    /// in a gated component's header.
    ///
    /// It exists because every gate above is *silent* at the point the user looks
    /// for it. A tier that switches clouds off leaves `CloudsData` on the entity,
    /// enabled, with every slider live, and an empty sky: nothing in the panel
    /// says the tier took the dome away, so the effect reads as broken. The
    /// atmosphere is worse, because the tier writes back — set the sky to
    /// Raymarched below `High` and the dropdown snaps to LookupTexture on the
    /// next frame, which reads as the editor refusing the setting.
    ///
    /// The mapping is a table here rather than a field on `InspectorEntry`
    /// because a component does not get to decide this: the gates are already one
    /// central list on this enum (`bloom`, `ssao`, `clouds`,
    /// `auto_exposure`, `atmosphere_raymarched`), and a second copy spread across
    /// 180-odd registration sites would drift from it the first time a tier moved.
    /// Anything not listed is ungated and returns `None`.
    pub fn inspector_gate(&self, type_id: &str) -> Option<QualityGate> {
        // Not in the list below, and deliberately: Solari reads no tier at all,
        // so its section must not claim to be gated. Nor is anti-aliasing: the
        // tier stops only one of its methods (TAA), so the header must not say
        // the whole section is off; its Method dropdown warns on TAA instead.
        let on = match type_id {
            "clouds" => self.clouds(),
            "bloom" => self.bloom(),
            "auto_exposure" => self.auto_exposure(),
            "world_env_ssao" => self.ssao(),
            "ssil" => self.ssil(),
            // The one that survives in reduced form rather than going away.
            "atmosphere" => {
                return (!self.atmosphere_raymarched()).then_some(QualityGate::Downgraded)
            }
            _ => return None,
        };
        (!on).then_some(QualityGate::Disabled)
    }
}

/// What a [`GraphicsQuality`] tier does to an effect whose inspector section is
/// gated by it. See [`GraphicsQuality::inspector_gate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QualityGate {
    /// The tier switches the effect off. Its component stays on the entity and
    /// its settings still edit; nothing it draws reaches the viewport.
    Disabled,
    /// The tier keeps the effect but forces it onto a cheaper path, overwriting
    /// the setting that asked for the expensive one.
    Downgraded,
}
