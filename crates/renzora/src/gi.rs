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
