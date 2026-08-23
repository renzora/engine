//! SSAO — the slice-2 piece of the `WorldEnvironment` reconcile. SSAO is now a
//! section of [`renzora::WorldEnvironment`] (`ssao`); [`reconcile_ssao`] drives
//! `ScreenSpaceAmbientOcclusion` from it. See `docs/world-environment-spec.md`.

use bevy::pbr::{ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Legacy per-effect SSAO settings. Superseded by [`renzora::WorldEnvironment::ssao`];
/// kept (registered, no longer synced or shown in the inspector) so pre-unification
/// scenes still deserialize.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SsaoSettings {
    pub enabled: bool,
}

impl Default for SsaoSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Translate the authored [`renzora::SsaoQuality`] + custom counts into Bevy's
/// quality enum.
///
/// The `Custom` counts are clamped rather than passed through: Bevy multiplies
/// them into the per-pixel sample count of a full-resolution compute pass, so a
/// zero makes the pass produce nothing and a hand-edited scene file with a large
/// value stalls the GPU. The bounds match the ranges the inspector exposes.
fn quality_level(s: &renzora::SsaoSection) -> ScreenSpaceAmbientOcclusionQualityLevel {
    use ScreenSpaceAmbientOcclusionQualityLevel as Q;
    match s.quality {
        renzora::SsaoQuality::Low => Q::Low,
        renzora::SsaoQuality::Medium => Q::Medium,
        renzora::SsaoQuality::High => Q::High,
        renzora::SsaoQuality::Ultra => Q::Ultra,
        renzora::SsaoQuality::Custom => Q::Custom {
            slice_count: s.slice_count.clamp(1, 16),
            samples_per_slice_side: s.samples_per_slice_side.clamp(1, 8),
        },
    }
}

/// Slice-2 of the `WorldEnvironment` reconcile: drive SSAO on every routed
/// camera from `WorldEnvironment::ssao`.
///
/// Bevy's `ScreenSpaceAmbientOcclusion` has no "no-occlusion" value, so for now
/// this gates by **component presence** (insert when enabled, remove when not) —
/// "off" is therefore zero-cost (no SSAO compute). SSAO's pipeline key reads the
/// component directly (in sync with the bind group), so this toggle should not
/// hit the contact-shadows specialization race; if GPU testing shows otherwise,
/// it escalates to the resident white-AO gate. See `docs/world-environment-spec.md`.
fn reconcile_ssao(
    mut commands: Commands,
    sources: Query<Ref<renzora::WorldEnvironment>>,
    routing: Res<renzora::EffectRouting>,
) {
    let routing_changed = routing.is_changed();
    for (target, source_list) in routing.iter() {
        let Some(env) = source_list.iter().find_map(|&s| sources.get(s).ok()) else {
            continue;
        };
        if !routing_changed && !env.is_changed() {
            continue;
        }
        let Ok(mut ec) = commands.get_entity(*target) else {
            continue;
        };
        if env.ssao.enabled {
            ec.insert(ScreenSpaceAmbientOcclusion {
                quality_level: quality_level(&env.ssao),
                // Negative thickness would push the occlusion test in front of
                // the surface; the inspector clamps, but scenes are hand-editable.
                constant_object_thickness: env.ssao.constant_object_thickness.max(0.0),
            });
        } else {
            ec.remove::<ScreenSpaceAmbientOcclusion>();
        }
    }
}

#[derive(Default)]
pub struct SsaoPlugin;

impl Plugin for SsaoPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] SsaoPlugin");
        app.register_type::<SsaoSettings>();
        app.add_systems(Update, reconcile_ssao);
    }
}

renzora::add!(SsaoPlugin);
