use bevy::core_pipeline::oit::OrderIndependentTransparencySettings;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct OitSettings {
    pub layer_count: i32,
    pub alpha_threshold: f32,
    pub enabled: bool,
}

impl Default for OitSettings {
    fn default() -> Self {
        Self {
            layer_count: 8,
            alpha_threshold: 0.0,
            enabled: true,
        }
    }
}

fn sync_oit(
    mut commands: Commands,
    sources: Query<(Entity, Ref<OitSettings>)>,
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
                // Never on the web. OIT adds its own bindings to the transparent
                // pass, and that pass is already at WebGPU's ceiling: with it on,
                // `alpha_blend_mesh_pipeline` needs 13 uniform buffers in the
                // fragment stage against a per-stage limit of 12, so the pipeline
                // fails to create and NOTHING transparent draws — a strictly
                // worse outcome than sorted alpha blending.
                //
                // Bevy declines OIT itself on WebGL (no FRAGMENT_WRITABLE_STORAGE).
                // WebGPU does have that flag, so it accepts OIT and then runs out
                // of binding slots instead; this is the equivalent refusal.
                if settings.enabled && !cfg!(target_arch = "wasm32") {
                    commands.entity(*target).insert(Msaa::Off).insert(
                        OrderIndependentTransparencySettings {
                            // 0.19: `layer_count` → `sorted_fragment_max_count`;
                            // the new `fragments_per_pixel_average` takes its default.
                            sorted_fragment_max_count: settings.layer_count as u32,
                            alpha_threshold: settings.alpha_threshold,
                            ..default()
                        },
                    );
                } else {
                    commands
                        .entity(*target)
                        .remove::<OrderIndependentTransparencySettings>();
                }
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<OrderIndependentTransparencySettings>();
            }
        }
    }
}

fn cleanup_oit(
    mut commands: Commands,
    mut removed: RemovedComponents<OitSettings>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<OrderIndependentTransparencySettings>();
            }
        }
    }
}

#[derive(Default)]
pub struct OitPlugin;

impl Plugin for OitPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] OitPlugin");
        app.register_type::<OitSettings>();
        app.add_systems(Update, (sync_oit, cleanup_oit));
    }
}

renzora::add!(OitPlugin);
