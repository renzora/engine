//! Contrast adaptive sharpening. Not anti-aliasing: it sharpens, mostly to
//! give back the detail TAA and DLSS soften, so it stays its own component and
//! sits on top of whichever method is chosen.

use bevy::anti_alias::contrast_adaptive_sharpening::ContrastAdaptiveSharpening;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct CasSettings {
    pub sharpening_strength: f32,
    pub denoise: bool,
    pub enabled: bool,
}

impl Default for CasSettings {
    fn default() -> Self {
        Self {
            sharpening_strength: 0.6,
            denoise: false,
            enabled: true,
        }
    }
}

pub(crate) fn sync_cas(
    mut commands: Commands,
    sources: Query<(Entity, Ref<CasSettings>)>,
    routing: Res<renzora::EffectRouting>,
) {
    let routing_changed = routing.is_changed();
    for (target, source_list) in routing.iter() {
        let mut found = false;
        for &src in source_list {
            if let Ok((_, settings)) = sources.get(src) {
                if !routing_changed && !settings.is_changed() {
                    found = true;
                    break;
                }
                if settings.enabled {
                    commands.entity(*target).insert(ContrastAdaptiveSharpening {
                        enabled: true,
                        sharpening_strength: settings.sharpening_strength,
                        denoise: settings.denoise,
                    });
                } else {
                    commands
                        .entity(*target)
                        .remove::<ContrastAdaptiveSharpening>();
                }
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<ContrastAdaptiveSharpening>();
            }
        }
    }
}
