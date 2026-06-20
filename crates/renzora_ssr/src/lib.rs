use bevy::core_pipeline::prepass::DeferredPrepass;
use bevy::pbr::ScreenSpaceReflections;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SsrSettings {
    pub enabled: bool,
    /// Assumed surface thickness for depth-buffer ray marching (world units).
    pub thickness: f32,
    /// Initial linear ray-march steps — quality vs. cost.
    pub linear_steps: u32,
    /// Binary-search refinement steps after the first hit — accuracy.
    pub bisection_steps: u32,
    /// Secant refinement for sharper, more accurate reflection hits.
    pub use_secant: bool,
}

impl Default for SsrSettings {
    // Mirrors bevy's `ScreenSpaceReflections::default()` so toggling on matches
    // the built-in physically-based SSR look.
    fn default() -> Self {
        Self {
            enabled: true,
            thickness: 0.25,
            linear_steps: 10,
            bisection_steps: 5,
            use_secant: true,
        }
    }
}

fn sync_ssr(
    mut commands: Commands,
    sources: Query<(Entity, Ref<SsrSettings>)>,
    deferred_cameras: Query<(), With<DeferredPrepass>>,
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
                    // Bevy 0.18's SSR is part of the deferred lighting
                    // path: inserting `ScreenSpaceReflections` switches
                    // the camera into deferred shading, which needs
                    // `DeferredPrepass` to supply the G-buffer
                    // (base color, normals, MR maps). On a forward-only
                    // camera the prepass isn't there, so shading
                    // collapses (shadows disappear, GI compositing
                    // breaks). Refuse to insert in that case.
                    if !deferred_cameras.contains(*target) {
                        warn!(
                            "SSR enabled but camera lacks DeferredPrepass; \
                             skipping (forward rendering path doesn't support SSR). \
                             Disable SSR in the inspector or wire up the deferred \
                             renderer."
                        );
                        commands.entity(*target).remove::<ScreenSpaceReflections>();
                    } else {
                        commands.entity(*target).insert(ScreenSpaceReflections {
                            thickness: settings.thickness,
                            linear_steps: settings.linear_steps,
                            bisection_steps: settings.bisection_steps,
                            use_secant: settings.use_secant,
                            // perceptual-roughness / fadeout ranges keep bevy's
                            // tuned defaults.
                            ..default()
                        });
                    }
                } else {
                    commands.entity(*target).remove::<ScreenSpaceReflections>();
                }
                found = true;
                break;
            }
        }
        if !found && routing_changed {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<ScreenSpaceReflections>();
            }
        }
    }
}

fn cleanup_ssr(
    mut commands: Commands,
    mut removed: RemovedComponents<SsrSettings>,
    routing: Res<renzora::EffectRouting>,
) {
    if removed.read().next().is_some() {
        for (target, _) in routing.iter() {
            if let Ok(mut ec) = commands.get_entity(*target) {
                ec.remove::<ScreenSpaceReflections>();
            }
        }
    }
}

#[derive(Default)]
pub struct SsrPlugin;

impl Plugin for SsrPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] SsrPlugin");
        app.register_type::<SsrSettings>();
        app.add_systems(Update, (sync_ssr, cleanup_ssr));
    }
}

renzora::add!(SsrPlugin);
