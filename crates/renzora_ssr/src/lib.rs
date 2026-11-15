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
fn sync_ssr(
    mut commands: Commands,
    sources: Query<Ref<SsrSettings>>,
    deferred_cameras: Query<(), With<DeferredPrepass>>,
    routing: Res<renzora::EffectRouting>,
    solari: Query<(), With<renzora::SolariLit>>,
    gained_solari: Query<(), Added<renzora::SolariLit>>,
    mut lost_solari: RemovedComponents<renzora::SolariLit>,
) {
    let routing_changed = routing.is_changed();
    let lost: Vec<Entity> = lost_solari.read().collect();
    for (target, source_list) in routing.iter() {
        let solari_changed = gained_solari.contains(*target) || lost.contains(target);
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
}
