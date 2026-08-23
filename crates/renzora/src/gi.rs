//! Global-illumination contract types.
//!
//! The GI settings components (`RtLighting`, `LumenLighting`) and their enums
//! live here in the shared `renzora` dylib so the host, the GI distribution
//! plugin (`renzora_lumen`), the editor inspectors, and `renzora_level_presets`
//! all share ONE `TypeId` across the dlopen boundary. Authoring crates insert
//! these components; the GI plugin's systems consume them.
//!
//! Also defines `LumenDiagState`, the flat per-frame diagnostics snapshot the
//! GI plugin produces (editor builds) and the debugger's Lumen panel reads —
//! same boundary-crossing reason.

use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::sync_component::SyncComponent;
use serde::{Deserialize, Serialize};

// ── RT (screen-space GI) ──────────────────────────────────────────────────

/// Output mode for the SSGI pass. Drives a uniform the shader branches on at
/// composite time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize, Default)]
pub enum RtDebugMode {
    /// scene + indirect — normal output.
    #[default]
    Composite,
    /// Indirect contribution only — no scene.
    IndirectOnly,
}

impl RtDebugMode {
    pub fn as_u32(self) -> u32 {
        match self {
            RtDebugMode::Composite => 0,
            RtDebugMode::IndirectOnly => 1,
        }
    }
}

/// Screen-space global illumination settings. Authored on a source entity and
/// routed onto the active cameras via `EffectRouting`.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct RtLighting {
    pub enabled: bool,
    pub intensity: f32,
    pub debug: RtDebugMode,
}

impl Default for RtLighting {
    fn default() -> Self {
        Self {
            enabled: true,
            intensity: 1.0,
            debug: RtDebugMode::Composite,
        }
    }
}

// 0.19: `ExtractComponent` now requires `SyncComponent` (ensures the entity is
// synced to the render world). `Target` is what gets removed from the render
// world when this component is removed — for a self-extracting component, Self.
impl SyncComponent for RtLighting {
    type Target = RtLighting;
}

impl ExtractComponent for RtLighting {
    type QueryData = &'static RtLighting;
    type QueryFilter = ();
    type Out = RtLighting;

    fn extract_component(
        item: bevy::ecs::query::QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        Some(item.clone())
    }
}

/// Marker placed on a target camera to tell the RT sync systems to leave its
/// `RtLighting` alone — set by the Lumen `ScreenSpace` tier when it owns the
/// channel. Insert alongside `RtLighting`; remove together when releasing it.
#[derive(Component, Clone, Debug, Default)]
pub struct RtLightingExternallyManaged;

// ── Lumen GI ──────────────────────────────────────────────────────────────

/// Quality tier for Lumen GI. Phase 1 implements only `Off` and `ScreenSpace`;
/// higher tiers parse but currently render the same as `Off`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize, Default)]
pub enum LumenQuality {
    Off,
    #[default]
    ScreenSpace,
    /// Reserved — Phase 5+: SDF tracing, low-res voxel cache.
    SdfLow,
    /// Reserved — Phase 5+: SDF tracing, full-res voxel cache.
    SdfHigh,
    /// Reserved — Phase 10: hardware ray tracing backend.
    Hwrt,
}

/// Debug visualization mode for Lumen GI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect, Serialize, Deserialize, Default)]
pub enum LumenDebug {
    #[default]
    None,
    /// Show only the indirect-light contribution, without the scene composite.
    IndirectOnly,
    /// Visualize the voxel radiance cache.
    VoxelCache,
}

/// Lumen global-illumination settings. Authored on a non-camera entity
/// (typically "World Environment"); the GI plugin routes the chosen tier onto
/// the active cameras. Mutually exclusive with a hand-attached `RtLighting`.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct LumenLighting {
    pub quality: LumenQuality,
    pub intensity: f32,
    /// Multiplier on the specular voxel-cone trace contribution.
    pub specular_intensity: f32,
    pub debug: LumenDebug,
}

impl Default for LumenLighting {
    fn default() -> Self {
        Self {
            quality: LumenQuality::ScreenSpace,
            intensity: 0.4,
            specular_intensity: 1.0,
            debug: LumenDebug::None,
        }
    }
}

impl SyncComponent for LumenLighting {
    type Target = LumenLighting;
}

impl ExtractComponent for LumenLighting {
    type QueryData = &'static LumenLighting;
    type QueryFilter = ();
    type Out = LumenLighting;

    fn extract_component(
        item: bevy::ecs::query::QueryItem<'_, '_, Self::QueryData>,
    ) -> Option<Self::Out> {
        Some(item.clone())
    }
}

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
/// [`EffectRouting`], mirroring [`LumenLighting`]. The `renzora_solari` plugin
/// consumes it: while enabled it attaches Bevy's `SolariLighting` (and the HDR +
/// prepass components it requires) to each routed camera and mirrors conforming
/// meshes into the ray-tracing scene.
///
/// Solari is a *different* GI backend from Lumen — fully dynamic hardware path
/// tracing, no voxel/SDF cache — and the two are mutually exclusive per camera.
/// Don't author both on the same World Environment.
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
        }
    }
}

// ── Diagnostics snapshot (GI plugin → debugger Lumen panel) ────────────────

/// Flat snapshot of the Lumen CPU-bake throttle. The GI plugin (editor builds)
/// copies its internal bake stats into this each frame; the debugger's Lumen
/// panel renders it. Plain primitives only — no render handles — so it crosses
/// the dlopen boundary cleanly.
#[derive(Clone, Default)]
pub struct LumenBakeSnapshot {
    pub last_bake_dur: std::time::Duration,
    pub avg_bake_dur: std::time::Duration,
    pub max_bake_dur: std::time::Duration,
    pub bakes_last_frame: usize,
    pub total_bakes: u64,
    pub total_samples_baked: u64,
    pub bake_budget_per_frame: usize,
}

/// One camera's voxel-cache view flags, for the debugger Lumen panel.
#[derive(Clone)]
pub struct LumenCameraEntry {
    pub camera_name: String,
    pub inject_active: bool,
    pub debug_active: bool,
}

/// Per-frame Lumen diagnostics snapshot. Produced by the GI plugin (editor
/// builds), consumed by the debugger's Lumen panel. Lives in the contract so
/// producer and consumer share one `TypeId` across the dlopen boundary.
#[derive(Resource, Default, Clone)]
pub struct LumenDiagState {
    pub cameras: Vec<LumenCameraEntry>,
    pub mesh_voxel_samples_entities: usize,
    pub has_sky_cubemap: bool,
    pub bake: LumenBakeSnapshot,
}
