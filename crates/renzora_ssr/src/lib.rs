use bevy::core_pipeline::prepass::DeferredPrepass;
use bevy::pbr::ScreenSpaceReflections;
use bevy::prelude::*;
use bevy::render::camera::TemporalJitter;
use std::ops::Range;

/// The roughness range over which SSR fades out on a camera that nothing
/// resolves temporally (FXAA, SMAA or no anti-aliasing).
///
/// Bevy's SSR reflects a rough surface by taking ONE random sample of its
/// reflection lobe per pixel per frame, and relies on TAA or DLSS to average
/// those samples over time. Without one, every pixel shows its own random
/// reflection, which reads as grain over rough wood and stone (it appeared
/// as soon as Solari was switched off, because DLSS had been doing the
/// averaging). Up to about 0.3 roughness the lobe is narrow enough that a
/// single sample is nearly noise-free; above that, the environment map's
/// prefiltered reflection is what a surface gets, as it would without SSR.
const UNRESOLVED_MAX_ROUGHNESS: Range<f32> = 0.25..0.3;
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

/// Whether Solari lights a camera this SSR source is routed to, so the
/// inspector can show SSR off while it is stood down (see [`sync_ssr`]).
pub fn stood_down_by_solari(world: &World, source: Entity) -> bool {
    let Some(routing) = world.get_resource::<renzora::EffectRouting>() else {
        return false;
    };
    routing.iter().any(|(target, sources)| {
        sources.contains(&source) && world.get::<renzora::SolariLit>(*target).is_some()
    })
}

/// Drive `ScreenSpaceReflections` on every routed camera from its
/// [`SsrSettings`].
///
/// **Solari stands SSR down.** A camera carrying [`renzora::SolariLit`] gets no
/// SSR whatever the settings say: Solari traces its own reflections, and
/// screen-space reflections composited over them flicker as the two disagree
/// from frame to frame. The settings keep their value, so turning Solari off
/// brings SSR back, the same way SSAO stands aside.
///
/// **Rough reflections need a temporal pass.** A camera with no
/// `TemporalJitter` (which TAA and DLSS both bring) gets SSR on glossy
/// surfaces only; see [`UNRESOLVED_MAX_ROUGHNESS`].
#[allow(clippy::too_many_arguments)]
fn sync_ssr(
    mut commands: Commands,
    sources: Query<Ref<SsrSettings>>,
    deferred_cameras: Query<(), With<DeferredPrepass>>,
    routing: Res<renzora::EffectRouting>,
    solari: Query<(), With<renzora::SolariLit>>,
    gained_solari: Query<(), Added<renzora::SolariLit>>,
    mut lost_solari: RemovedComponents<renzora::SolariLit>,
    jittered: Query<(), With<TemporalJitter>>,
    gained_jitter: Query<(), Added<TemporalJitter>>,
    mut lost_jitter: RemovedComponents<TemporalJitter>,
) {
    let routing_changed = routing.is_changed();
    let lost: Vec<Entity> = lost_solari.read().collect();
    let lost_jitter: Vec<Entity> = lost_jitter.read().collect();
    for (target, source_list) in routing.iter() {
        let solari_changed = gained_solari.contains(*target)
            || lost.contains(target)
            || gained_jitter.contains(*target)
            || lost_jitter.contains(target);
        let mut found = false;
        for &src in source_list {
            if let Ok(settings) = sources.get(src) {
                if !routing_changed && !settings.is_changed() && !solari_changed {
                    found = true;
                    break;
                }
                if settings.enabled && !solari.contains(*target) {
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
                        let defaults = ScreenSpaceReflections::default();
                        let max_perceptual_roughness = if jittered.contains(*target) {
                            defaults.max_perceptual_roughness.clone()
                        } else {
                            UNRESOLVED_MAX_ROUGHNESS
                        };
                        commands.entity(*target).insert(ScreenSpaceReflections {
                            thickness: settings.thickness,
                            linear_steps: settings.linear_steps,
                            bisection_steps: settings.bisection_steps,
                            use_secant: settings.use_secant,
                            max_perceptual_roughness,
                            // The other ranges keep bevy's tuned defaults.
                            ..defaults
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

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_systems(Update, sync_ssr);
        let source = app.world_mut().spawn(SsrSettings::default()).id();
        let camera = app.world_mut().spawn(DeferredPrepass).id();
        app.insert_resource(renzora::EffectRouting {
            routes: vec![(camera, vec![source])],
        });
        (app, source, camera)
    }

    fn has_ssr(app: &App, camera: Entity) -> bool {
        app.world().get::<ScreenSpaceReflections>(camera).is_some()
    }

    /// Solari traces its own reflections: a ray-traced camera gets no SSR, and
    /// gets it back the moment Solari stops lighting it, with the settings
    /// untouched in between.
    #[test]
    fn solari_stands_ssr_down_and_hands_it_back() {
        let (mut app, source, camera) = app();
        app.update();
        assert!(has_ssr(&app, camera), "SSR is on by default");
        assert!(!stood_down_by_solari(app.world(), source));

        app.world_mut().entity_mut(camera).insert(renzora::SolariLit);
        app.update();
        assert!(!has_ssr(&app, camera), "Solari should stand SSR down");
        assert!(stood_down_by_solari(app.world(), source));
        assert!(app.world().get::<SsrSettings>(source).unwrap().enabled, "the setting is kept");

        app.world_mut().entity_mut(camera).remove::<renzora::SolariLit>();
        app.update();
        assert!(has_ssr(&app, camera), "SSR should come back without Solari");
    }

    fn max_roughness(app: &App, camera: Entity) -> Range<f32> {
        app.world().get::<ScreenSpaceReflections>(camera).unwrap().max_perceptual_roughness.clone()
    }

    /// Without a temporal pass SSR keeps to glossy surfaces, whose one sample
    /// per pixel is not grain; TAA or DLSS (both jitter) brings the full
    /// range back, and losing it takes it away again.
    #[test]
    fn rough_reflections_follow_the_temporal_pass() {
        let (mut app, _source, camera) = app();
        app.update();
        assert_eq!(max_roughness(&app, camera), UNRESOLVED_MAX_ROUGHNESS);

        app.world_mut().entity_mut(camera).insert(TemporalJitter::default());
        app.update();
        assert_eq!(
            max_roughness(&app, camera),
            ScreenSpaceReflections::default().max_perceptual_roughness
        );

        app.world_mut().entity_mut(camera).remove::<TemporalJitter>();
        app.update();
        assert_eq!(max_roughness(&app, camera), UNRESOLVED_MAX_ROUGHNESS);
    }
}
