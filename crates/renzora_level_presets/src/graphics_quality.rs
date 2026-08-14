//! Graphics-quality enforcement — turns the user-selected [`GraphicsQuality`]
//! tier (Settings → Viewport → Performance) into actual render cost.
//!
//! Why this exists: on an empty scene the editor still spends ~30 ms of GPU per
//! frame, because the cost is **fullscreen, resolution-bound** passes on the
//! active camera (screen-space GI + auto-exposure + bloom + TAA), not geometry.
//! On a weak GPU or a high-DPI (Retina) display — where the pixel count is ~4× —
//! that stack drops to single-digit FPS regardless of what's in the scene. The
//! tier lets a user trade those passes for frame rate.
//!
//! ## Why it touches the *camera*, not the scene source
//!
//! Each effect is authored on a scene entity (GI on the "World Environment",
//! bloom/AE/TAA on the scene camera) and **`EffectRouting` fans it onto the
//! editor's viewport cameras**, which is where the pass actually runs. We force
//! the tier on those *routed copies* — the viewport cameras — and deliberately
//! leave the authored sources untouched. Two reasons:
//!
//! 1. **No save bleed.** The authored components serialize into the scene file;
//!    the viewport cameras carry `HideInHierarchy` and are excluded from saves
//!    (`renzora_engine::scene_io::save_scene`). Mutating the source would bake
//!    "GI off" into every scene saved on the default (Medium) tier — and silently
//!    strip it for anyone who later opens that scene at High. Mutating only the
//!    viewport copies can never reach disk.
//! 2. **Crash-safe.** We flip `RtLighting.enabled` (the same switch the Render
//!    Toggles debug panel uses) and add/remove the post-process components exactly
//!    as the routers themselves do. We never touch the atmosphere or the prepass
//!    bundle — their attachment layout is fixed at camera spawn and toggling them
//!    at runtime trips a wgpu validation crash (see `renzora_engine::camera`), so
//!    they stay resident at every tier.
//!
//! ## Restoring on a tier change
//!
//! Each router re-applies its effect from the source only when the source or
//! [`EffectRouting`] *changes* (e.g. `sync_lumen_lighting`'s `routing.is_changed()`
//! gate). So to bring an effect back when the tier is raised, we just bump
//! `EffectRouting` on any tier transition; every router then re-syncs from the
//! untouched source, and the per-frame force below immediately re-disables
//! whatever the new tier still forbids.

use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::light::DirectionalLightShadowMap;
use bevy::pbr::{AtmosphereMode, AtmosphereSettings, ScreenSpaceAmbientOcclusion};
use bevy::post_process::auto_exposure::AutoExposure;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;

use renzora::core::viewport_types::ViewportSettings;
use renzora::core::ViewportCamera;
use renzora::{
    EffectRouting, LumenLighting, LumenQuality, ResolvedGraphicsQuality, RtLighting, SplashState,
};

/// Remembers the last tier so we can re-poke `EffectRouting` exactly once per
/// change rather than every frame.
#[derive(Resource, Default)]
struct GraphicsQualityState {
    last: Option<renzora::core::viewport_types::GraphicsQuality>,
}

/// Suggest the `Low` tier once per session when running on an integrated GPU.
///
/// The tier defaults to `Medium`, and the users who most need `Low` are exactly
/// the ones least likely to go looking for Settings → Viewport → Performance —
/// the editor just feels slow and they have no reason to suspect a setting.
///
/// Deliberately a *hint*, not an action: nothing changes the user's tier for
/// them. A silently-applied override would be indistinguishable from the engine
/// misbehaving, and someone on integrated graphics may well have picked their
/// tier on purpose.
///
/// Needs no "already asked" flag on disk because the condition is
/// **self-clearing**: it only fires while the tier is not already `Low`, so
/// acting on it silences it permanently. Ignoring it costs one toast per launch,
/// which is the right pressure for a hint the user hasn't acted on.
fn suggest_low_tier_on_integrated_gpu(
    integrated: Option<Res<renzora::GpuIsIntegrated>>,
    settings: Option<Res<ViewportSettings>>,
    toasts: Option<ResMut<renzora_ui::Toasts>>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let (Some(integrated), Some(settings), Some(mut toasts)) = (integrated, settings, toasts) else {
        return;
    };
    if !integrated.yes {
        *done = true;
        return;
    }
    if settings.graphics_quality == renzora::core::viewport_types::GraphicsQuality::Low {
        *done = true;
        return;
    }
    toasts.info(renzora::lang::t("settings.hint.integrated_gpu"));
    *done = true;
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<GraphicsQualityState>();
    app.add_systems(
        PostUpdate,
        suggest_low_tier_on_integrated_gpu.run_if(in_state(SplashState::Editor)),
    );
    // PostUpdate so we run after the Update-stage effect routers, and the force
    // below has the last word over what they applied this frame.
    app.add_systems(
        PostUpdate,
        enforce_graphics_quality.run_if(in_state(SplashState::Editor)),
    );
}

#[allow(clippy::too_many_arguments)]
fn enforce_graphics_quality(
    settings: Option<Res<ViewportSettings>>,
    mut state: ResMut<GraphicsQualityState>,
    mut resolved: ResMut<ResolvedGraphicsQuality>,
    routing: Option<ResMut<EffectRouting>>,
    mut commands: Commands,
    mut gi_rt: Query<&mut RtLighting, With<ViewportCamera>>,
    mut gi_lumen: Query<&mut LumenLighting, With<ViewportCamera>>,
    bloom_cams: Query<Entity, (With<ViewportCamera>, With<Bloom>)>,
    taa_cams: Query<Entity, (With<ViewportCamera>, With<TemporalAntiAliasing>)>,
    ae_cams: Query<Entity, (With<ViewportCamera>, With<AutoExposure>)>,
    ssao_cams: Query<Entity, (With<ViewportCamera>, With<ScreenSpaceAmbientOcclusion>)>,
    mut atmo: Query<&mut AtmosphereSettings, With<ViewportCamera>>,
    shadow_map: Option<ResMut<DirectionalLightShadowMap>>,
) {
    let Some(settings) = settings else {
        return;
    };
    let q = settings.graphics_quality;

    // Publish the live tier as the shared resource so downstream renderer crates
    // (clouds, environment-map IBL) apply the same tier in the editor viewport
    // that a shipped game applies from project config.
    if resolved.0 != q {
        resolved.0 = q;
    }

    // On a tier change, nudge the routers so any effect a lower tier had disabled
    // is re-applied from its (untouched) scene source. The per-frame force below
    // then re-strips whatever the new tier still forbids — so a downward change
    // costs at most a one-frame re-enable, and an upward change restores cleanly.
    if state.last != Some(q) {
        if let Some(mut routing) = routing {
            routing.set_changed();
        }
        state.last = Some(q);
    }

    // ── Screen-space GI (Lumen + RT) — the heaviest, most pixel-bound pass ──
    if !q.gi() {
        // SSGI renders off the camera's `RtLighting.enabled`; the reserved SDF
        // path reads `LumenLighting.quality`. Clear both so the GI channel is off
        // however it's routed. Reads go through `Deref` and only the assignment
        // hits `DerefMut`, so we don't re-flag the component every frame.
        for mut r in &mut gi_rt {
            if r.enabled {
                r.enabled = false;
            }
        }
        for mut l in &mut gi_lumen {
            if l.quality != LumenQuality::Off {
                l.quality = LumenQuality::Off;
            }
        }
    }

    // ── The post-process passes — remove the routed component on the camera ──
    // The `With<…>` filters mean each query only yields cameras that still carry
    // the component, so once removed they stop matching and we don't churn.
    if !q.bloom() {
        for e in &bloom_cams {
            commands.entity(e).remove::<Bloom>();
        }
    }
    if !q.taa() {
        for e in &taa_cams {
            commands.entity(e).remove::<TemporalAntiAliasing>();
        }
    }
    if !q.auto_exposure() {
        // DIAGNOSTIC (temporary): profiling shows the `auto_exposure` pass costing
        // the same at `Low` as above it, even though this branch should have
        // stripped the component. Note the pass's *zone* is expected to persist
        // either way — bevy registers `auto_exposure` as a `Core3d` graph system
        // with no run condition — so zone presence proves nothing; only the cost
        // does. This narrows it to one of two answers in a single launch:
        //   * logs once then stops  -> removal works, the cost is elsewhere
        //     (most likely bevy retaining the per-view buffer after the component
        //     goes away, the same bug as `prepare_uniform_components`);
        //   * logs every frame      -> something re-inserts it each frame and the
        //     gate is losing a fight it re-enters forever.
        // Delete once answered.
        let n = ae_cams.iter().count();
        if n > 0 {
            debug!("[graphics_quality] stripping AutoExposure from {n} viewport camera(s) at Low");
        }
        for e in &ae_cams {
            commands.entity(e).remove::<AutoExposure>();
        }
    }
    // SSAO — three full-res compute passes; dropped below `High` (the same
    // fullscreen, resolution-bound cost class as SSGI).
    if !q.ssao() {
        for e in &ssao_cams {
            commands.entity(e).remove::<ScreenSpaceAmbientOcclusion>();
        }
    }
    // Atmosphere — drop the raymarched sky (a 16-step per-pixel raymarch) to the
    // ~40× cheaper LookupTexture path below `High`. `AtmosphereMode` isn't
    // `PartialEq`, so gate the assignment on a `matches!` read to avoid
    // re-flagging the component every frame.
    if !q.atmosphere_raymarched() {
        for mut s in &mut atmo {
            if matches!(s.rendering_method, AtmosphereMode::Raymarched) {
                s.rendering_method = AtmosphereMode::LookupTexture;
            }
        }
    }
    // Directional shadow-map resolution — a global resource shared by all cascades
    // (the editor viewports render the same lights, so one setting covers them).
    // Gated on difference to avoid re-flagging every frame.
    if let Some(mut sm) = shadow_map {
        let target = q.shadow_map_size();
        if sm.size != target {
            sm.size = target;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use renzora::core::viewport_types::GraphicsQuality;

    /// A world holding one viewport camera carrying the full effect stack, plus
    /// every resource the system reads.
    ///
    /// The camera deliberately carries *everything*, because the property under
    /// test is subtractive: each tier is defined by what it strips, so a test
    /// that only spawned the components it expected to survive could not tell
    /// "correctly kept" from "never there".
    fn world_at(quality: GraphicsQuality) -> (World, Entity) {
        let mut world = World::new();

        world.insert_resource(ViewportSettings {
            graphics_quality: quality,
            ..default()
        });
        world.init_resource::<GraphicsQualityState>();
        world.init_resource::<ResolvedGraphicsQuality>();
        world.insert_resource(DirectionalLightShadowMap { size: 4096 });

        let camera = world
            .spawn((
                // The index is the multi-viewport slot; slot 0 is the focused
                // one, and the tier applies to every slot alike.
                ViewportCamera(0),
                (
                    Bloom::default(),
                    TemporalAntiAliasing::default(),
                    AutoExposure::default(),
                    ScreenSpaceAmbientOcclusion::default(),
                ),
                (
                    AtmosphereSettings {
                        rendering_method: AtmosphereMode::Raymarched,
                        ..default()
                    },
                    RtLighting { enabled: true, ..default() },
                    LumenLighting { quality: LumenQuality::ScreenSpace, ..default() },
                ),
            ))
            .id();

        (world, camera)
    }

    fn enforce(world: &mut World) {
        world.run_system_once(enforce_graphics_quality).unwrap();
    }

    #[test]
    fn high_keeps_the_whole_stack() {
        let (mut world, cam) = world_at(GraphicsQuality::High);
        enforce(&mut world);

        let e = world.entity(cam);
        assert!(e.get::<Bloom>().is_some());
        assert!(e.get::<TemporalAntiAliasing>().is_some());
        assert!(e.get::<AutoExposure>().is_some());
        assert!(e.get::<ScreenSpaceAmbientOcclusion>().is_some());
        assert!(e.get::<RtLighting>().unwrap().enabled);
        assert_eq!(e.get::<LumenLighting>().unwrap().quality, LumenQuality::ScreenSpace);
        assert!(matches!(
            e.get::<AtmosphereSettings>().unwrap().rendering_method,
            AtmosphereMode::Raymarched
        ));
    }

    /// Medium's whole purpose is killing the heaviest pixel-bound passes (SSGI,
    /// SSAO, the raymarched sky) while keeping the tonemapped look.
    #[test]
    fn medium_drops_the_screen_space_passes_but_keeps_the_look() {
        let (mut world, cam) = world_at(GraphicsQuality::Medium);
        enforce(&mut world);

        let e = world.entity(cam);
        assert!(!e.get::<RtLighting>().unwrap().enabled, "SSGI must be off below High");
        assert_eq!(e.get::<LumenLighting>().unwrap().quality, LumenQuality::Off);
        assert!(e.get::<ScreenSpaceAmbientOcclusion>().is_none());
        assert!(matches!(
            e.get::<AtmosphereSettings>().unwrap().rendering_method,
            AtmosphereMode::LookupTexture
        ));

        // The tonemapped look survives — that is what separates Medium from Low.
        assert!(e.get::<Bloom>().is_some());
        assert!(e.get::<TemporalAntiAliasing>().is_some());
        assert!(e.get::<AutoExposure>().is_some());
    }

    #[test]
    fn low_strips_every_fullscreen_pass() {
        let (mut world, cam) = world_at(GraphicsQuality::Low);
        enforce(&mut world);

        let e = world.entity(cam);
        assert!(e.get::<Bloom>().is_none());
        assert!(e.get::<TemporalAntiAliasing>().is_none());
        assert!(e.get::<AutoExposure>().is_none());
        assert!(e.get::<ScreenSpaceAmbientOcclusion>().is_none());
        assert!(!e.get::<RtLighting>().unwrap().enabled);
        assert_eq!(e.get::<LumenLighting>().unwrap().quality, LumenQuality::Off);
    }

    /// The atmosphere and prepass bundles must stay *resident* at every tier —
    /// their attachment layout is fixed at camera spawn and removing them at
    /// runtime trips a wgpu validation crash. Downgrading the rendering method
    /// is the cheap path; removing the component is the crash.
    #[test]
    fn the_atmosphere_component_is_never_removed() {
        for quality in [GraphicsQuality::Low, GraphicsQuality::Medium, GraphicsQuality::High] {
            let (mut world, cam) = world_at(quality);
            enforce(&mut world);
            assert!(
                world.entity(cam).get::<AtmosphereSettings>().is_some(),
                "{quality:?} removed AtmosphereSettings — that is a wgpu crash"
            );
        }
    }

    #[test]
    fn each_tier_sets_its_shadow_map_size() {
        for (quality, expected) in [
            (GraphicsQuality::Low, 512),
            (GraphicsQuality::Medium, 1024),
            (GraphicsQuality::High, 2048),
        ] {
            let (mut world, _) = world_at(quality);
            enforce(&mut world);
            assert_eq!(world.resource::<DirectionalLightShadowMap>().size, expected);
        }
    }

    /// Downstream renderer crates (clouds, environment-map IBL) read the tier
    /// from this resource rather than from `ViewportSettings`, so that a shipped
    /// game applying it from project config and the editor viewport agree.
    #[test]
    fn the_live_tier_is_published_for_other_crates() {
        let (mut world, _) = world_at(GraphicsQuality::Low);
        enforce(&mut world);
        assert_eq!(world.resource::<ResolvedGraphicsQuality>().0, GraphicsQuality::Low);
    }

    /// Raising the tier has to re-poke `EffectRouting`, because each router only
    /// re-applies its effect from the (untouched) scene source when routing
    /// changes. Without the poke, an effect a lower tier disabled would never
    /// come back.
    #[test]
    fn a_tier_change_nudges_the_routers_exactly_once() {
        let (mut world, _) = world_at(GraphicsQuality::Low);
        world.insert_resource(EffectRouting::default());

        enforce(&mut world);
        assert!(
            world.resource_ref::<EffectRouting>().is_changed(),
            "the first run is a tier change and must nudge the routers"
        );

        // A second run at the same tier must NOT keep re-poking: routers would
        // re-sync every frame, which is the churn this state exists to avoid.
        world.clear_trackers();
        enforce(&mut world);
        assert!(
            !world.resource_ref::<EffectRouting>().is_changed(),
            "an unchanged tier re-poked the routers"
        );

        // Raising the tier is a change again.
        world.clear_trackers();
        world.resource_mut::<ViewportSettings>().graphics_quality = GraphicsQuality::High;
        enforce(&mut world);
        assert!(world.resource_ref::<EffectRouting>().is_changed());
    }

    /// Scene-authored effects are what get serialized. The tier must only ever
    /// touch the routed copies on viewport cameras, or saving a scene on the
    /// default tier would bake "GI off" into the file for everyone.
    #[test]
    fn effects_on_non_viewport_entities_are_left_alone() {
        let (mut world, _) = world_at(GraphicsQuality::Low);
        let authored = world
            .spawn((
                Bloom::default(),
                AutoExposure::default(),
                RtLighting { enabled: true, ..default() },
            ))
            .id();

        enforce(&mut world);

        let e = world.entity(authored);
        assert!(e.get::<Bloom>().is_some(), "an authored source was mutated");
        assert!(e.get::<AutoExposure>().is_some());
        assert!(e.get::<RtLighting>().unwrap().enabled);
    }

    #[test]
    fn with_no_viewport_settings_nothing_is_touched() {
        let (mut world, cam) = world_at(GraphicsQuality::Low);
        world.remove_resource::<ViewportSettings>();
        enforce(&mut world);
        assert!(world.entity(cam).get::<Bloom>().is_some());
    }
}
