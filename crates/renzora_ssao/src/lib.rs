//! SSAO — the slice-2 piece of the `WorldEnvironment` reconcile. SSAO is now a
//! section of [`renzora::WorldEnvironment`] (`ssao`); [`reconcile_ssao`] drives
//! `ScreenSpaceAmbientOcclusion` from it. See `docs/world-environment-spec.md`.

use bevy::pbr::ScreenSpaceAmbientOcclusion;
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
            ec.insert(ScreenSpaceAmbientOcclusion::default());
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
