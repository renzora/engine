//! Global-illumination contract types.
//!
//! Solari (Bevy's hardware-raytraced lighting) is the engine's only GI. Its
//! settings component and the GPU capability flags live here in the shared
//! `renzora` dylib so the host, the Solari plugin and the editor all share ONE
//! `TypeId` across the dlopen boundary.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

// ── Solari (hardware-raytraced GI) ─────────────────────────────────────────

/// GPU ray-tracing capability flag, decided ONCE by the host at startup.
///
/// Bevy's `bevy_solari` needs ray-tracing wgpu features (`EXPERIMENTAL_RAY_QUERY`
/// plus acceleration structures) enabled on the `RenderDevice` *at creation time* —
/// which is frozen before any dlopen plugin's `build()` runs. So the host
/// (`renzora_runtime`) probes the GPU adapter at boot, requests those features
/// when supported, and records the verdict here. The `renzora_solari` plugin
/// reads it in `build()` (before the device exists) to decide whether installing
/// `SolariPlugins` is safe: adding ray-tracing render nodes on a GPU that can't
/// create them would crash the engine, so when this is absent/`false` the plugin
/// stays inert and the engine boots normally on non-RT GPUs.
///
/// Lives in the contract dylib so the host (producer) and the plugin (consumer)
/// share one `TypeId` across the dlopen boundary.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct GpuRaytracing {
    pub enabled: bool,
}

/// Whether the GPU the renderer picked is integrated (or a software fallback)
/// rather than a discrete card — probed once at boot by `renzora_runtime`
/// alongside [`GpuRaytracing`], from the same `wgpu` adapter request.
///
/// The editor's cost is dominated by **fullscreen, resolution-bound** passes
/// (SSGI, SSAO, bloom, auto-exposure, TAA), which is exactly what an integrated
/// GPU is worst at — the same scene that runs comfortably on a discrete card can
/// sit in single digits. The Graphics Quality tier exists to trade those passes
/// away, but it defaults to `Medium` and users generally never find it, so this
/// flag lets the editor *point them at it* once.
///
/// Deliberately only a hint: nothing changes a user's tier on their behalf.
///
/// Lives in the contract dylib so the host (producer) and the editor plugins
/// (consumers) share one `TypeId` across the dlopen boundary.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct GpuIsIntegrated {
    pub yes: bool,
}

/// Solari raytraced-GI settings. Authored on a non-camera source entity
/// (typically "World Environment") and routed onto the active cameras via
/// [`EffectRouting`]. The Solari plugin consumes it: while enabled it attaches
/// Bevy's `SolariLighting` (and the HDR + prepass components it requires) to
/// each routed camera and mirrors conforming meshes into the ray-tracing scene.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SolariGi {
    pub enabled: bool,
    /// Stop rendering shadow maps while Solari is lighting the scene.
    ///
    /// Solari replaces shadow mapping with traced visibility, and its camera
    /// carries `SkipDeferredLighting` — which removes the only pass that would
    /// ever *read* a shadow map. Every sun cascade and every point-light cubemap
    /// is therefore rendered and then thrown away, which in a scene with many
    /// lamps is a large amount of wasted GPU time. Bevy's own guidance is to
    /// turn shadow maps off on all lights while Solari is on.
    ///
    /// The suppression is applied in the *render world*, to the extracted
    /// lights, so your `PointLight`/`DirectionalLight` components are never
    /// touched and a scene saved while Solari is active doesn't silently
    /// persist shadows-off. Turn this off if you want to compare against, or
    /// keep, raster shadows.
    #[serde(default = "shadow_map_suppression_default")]
    #[reflect(default = "shadow_map_suppression_default")]
    pub suppress_shadow_maps: bool,
    /// Give point and spot lights an invisible emissive sphere in the traced
    /// scene, so they light it at all.
    ///
    /// Solari samples exactly two kinds of light: directional, and emissive
    /// meshes. Point and spot lights contribute *nothing* — and because a Solari
    /// camera carries `SkipDeferredLighting`, Bevy's clustered-light pass (the
    /// only other thing that would apply them) never runs either. A lamp-lit
    /// scene therefore renders almost black, and nothing gets bright enough to
    /// bloom.
    ///
    /// With this on, each point/spot light gets a small sphere in the
    /// ray-tracing scene whose emissive radiance is derived from the light's
    /// luminous power, which Solari samples as a real area light — soft shadows
    /// and colour bleed included. The sphere is traced-only, never rasterized,
    /// so nothing new appears in the viewport.
    ///
    /// Caveats: a spot light loses its cone (the sphere emits in all
    /// directions), and `AmbientLight` cannot be represented this way at all.
    #[serde(default = "light_proxies_default")]
    #[reflect(default = "light_proxies_default")]
    pub light_proxies: bool,
    /// How much work Solari spends per frame, traded against noise. See
    /// [`SolariQuality`]. A scene saved before this existed loads as `High`,
    /// which is what it rendered as before.
    #[serde(default)]
    #[reflect(default)]
    pub quality: SolariQuality,
}

/// Solari's cost/noise preset, mapped by the Solari plugin onto the sample
/// counts and bounce depth of Bevy's `SolariLighting`.
///
/// The step that matters most is ReSTIR, which `Low` turns off. It reuses
/// samples across neighbouring pixels and past frames, and it is the only
/// denoising this build has: Bevy's other one is DLSS Ray Reconstruction, which
/// needs NVIDIA's SDK at build time and is not enabled. So `Low` is the fastest
/// and visibly the grainiest; it is meant for moving around a scene, not for
/// judging lighting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Reflect, Serialize, Deserialize)]
#[reflect(Default)]
pub enum SolariQuality {
    /// No ReSTIR, one bounce, fewest samples.
    Low,
    /// ReSTIR on, two bounces, half of Bevy's sample counts.
    Medium,
    /// ReSTIR on with Bevy's own defaults (three bounces).
    #[default]
    High,
    /// ReSTIR on, four bounces, double the samples. For stills.
    Ultra,
}

impl SolariQuality {
    /// Every preset, in the order the inspector lists them.
    pub const ALL: [Self; 4] = [Self::Low, Self::Medium, Self::High, Self::Ultra];

    /// The inspector dropdown labels, index for index with [`ALL`](Self::ALL).
    pub const LABELS: [&'static str; 4] = ["Low", "Medium", "High", "Ultra"];

    /// The inspector dropdown label.
    pub fn label(self) -> &'static str {
        Self::LABELS[self as usize]
    }

    /// The preset with this [`label`](Self::label), if any.
    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|q| q.label() == label)
    }
}

/// Default for [`SolariGi::suppress_shadow_maps`], used both by `serde` and by
/// reflection so a scene saved before the field existed loads with suppression
/// on rather than with `bool::default()` (`false`).
fn shadow_map_suppression_default() -> bool {
    true
}

/// Default for [`SolariGi::light_proxies`]. On, because without it point and
/// spot lights are simply absent from a Solari render.
fn light_proxies_default() -> bool {
    true
}

impl Default for SolariGi {
    fn default() -> Self {
        Self {
            enabled: true,
            suppress_shadow_maps: shadow_map_suppression_default(),
            light_proxies: light_proxies_default(),
            quality: SolariQuality::default(),
        }
    }
}
