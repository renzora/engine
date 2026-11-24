//! Graphics-quality enforcement: the one implementation of the project's
//! [`GraphicsQuality`] tier, for a shipped game and the editor alike.
//!
//! ## Why it lives here, in the runtime
//!
//! The tier is a game setting (`[rendering] graphics_quality` in
//! `project.toml`), so the runtime stores and applies it, and the editor only
//! edits it. There used to be two copies: this one, registered only in a game,
//! and an editor one in `renzora_level_presets` that only touched the
//! viewport cameras. The editor copy was the only thing applying the tier
//! while editing, and the game copy read the setting once and treated it as
//! fixed, so a script that changed the tier in a running game changed nothing
//! anyone could see. Now one set of systems runs in every session and follows
//! [`CurrentProject`]'s config live, whoever wrote it: the Settings panel, a
//! script, or the project load.
//!
//! ## Which cameras it governs
//!
//! - **A game:** every 3D camera. The scene camera renders directly, and the
//!   effect routers put the post-process components on it.
//! - **An editor session:** only the [`ViewportCamera`]s. Each effect is
//!   authored on a scene entity and `EffectRouting` fans it onto the viewport
//!   cameras, which are where the passes run. The authored sources serialize
//!   into the scene file and the viewport cameras are save-excluded, so
//!   stripping only the viewport copies can never bake "bloom off" into a
//!   saved scene. The other 3D cameras in an editor (material preview,
//!   thumbnails) are not the game's view and keep what they asked for.
//!
//! [`IsolatedCamera`]s are never touched in either: they are offscreen
//! previews with their own lighting.
//!
//! ## Crash-safety
//!
//! Every mutation is one a router already performs dynamically, so none of it
//! grows a camera's bind-group layout after first render:
//! - Bloom / TAA / auto-exposure / SSAO: remove the routed component. Each
//!   router re-adds it from the untouched scene source when routing changes,
//!   which is how a tier raise brings an effect back (see below).
//! - Atmosphere: assign `AtmosphereSettings.rendering_method`, which
//!   `renzora_atmosphere::sync_atmosphere` already reassigns at runtime.
//! - Shadow-map resolution: a global resource Bevy reads every frame.
//!
//! ## The sky probe, and why a tier change rebuilds it over two frames
//!
//! The tier also sets the face size of the camera's atmosphere probe
//! (`AtmosphereEnvironmentMapLight::size`, see `GraphicsQuality::ibl_face_size`).
//! `renzora_environment_map::sync_environment_map` re-inserts the probe in
//! place when its size and the tier disagree, and Bevy answers an in-place
//! re-insert by dropping the derived `GeneratedEnvironmentMapLight` and
//! `EnvironmentMapLight` and making new ones. That is fine on its own. It
//! crashes when it lands in the same command flush as Bevy's
//! `generate_environment_map_light`, which runs unordered in `Update` and
//! allocates the filtered map's mip chain from the cube it sees: its insert of
//! the OLD-sized `EnvironmentMapLight` can apply after the re-insert, leaving a
//! new-sized cube beside an old-sized mip chain, and the filter then asks for a
//! mip that does not exist (`storage_view_mip_8 ... only has 8 total mip
//! levels`). A game set to High hit exactly that on startup when the tier was
//! resolved a frame after the camera was built.
//!
//! So the tier is resolved in `PostUpdate` (and once in `Startup`, before the
//! scene's cameras are built), and on the same frame a probe's size stops
//! matching, [`rebuild_mismatched_sky_probes`] takes the probe off the camera
//! entirely; the next frame's `PostUpdate` puts a fresh one back at the new
//! size. Nothing in `Update` ever sees a mismatched probe, so the in-place
//! re-insert never runs, and the add happens in a schedule where Bevy's
//! generator is not running. Removing the probe and adding it back is the same
//! round trip `renzora_atmosphere`'s `ParkedSky` makes whenever the sky is
//! switched off and on, which Bevy 0.20 handles cleanly. The cost is one frame
//! with no sky light on that camera, then a re-bake, which is what any tier
//! change that resizes the probe costs anyway.

use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::light::{AtmosphereEnvironmentMapLight, DirectionalLightShadowMap};
use bevy::pbr::{AtmosphereMode, AtmosphereSettings, ScreenSpaceAmbientOcclusion};
use bevy::post_process::auto_exposure::AutoExposure;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;

use renzora_project_api::{CurrentProject, EditorSession, ResolvedGraphicsQuality};
use renzora_scene_api::{EffectRouting, IsolatedCamera, ViewportCamera};
use renzora_viewport_api::GraphicsQuality;

/// Install the tier systems. Called by `RuntimePlugin` in every session.
pub(crate) fn register(app: &mut App) {
    app.init_resource::<PendingSkyProbes>();
    // `Startup`, so a game's tier is the project's before the first `Update`:
    // `rehydrate_cameras` sizes the game camera's sky probe from it there.
    app.add_systems(Startup, sync_graphics_quality);
    // `PostUpdate`, after the `Update`-stage effect routers, so the force has
    // the last word over what they applied this frame; and chained, so the
    // probe rebuild sees the tier the same frame it changes (see the module
    // doc for why that has to happen outside `Update`).
    app.add_systems(
        PostUpdate,
        (
            sync_graphics_quality,
            enforce_graphics_quality,
            rebuild_mismatched_sky_probes,
        )
            .chain(),
    );
}

/// Copy the project's `[rendering]` tier into [`ResolvedGraphicsQuality`],
/// the resource every renderer crate reads (clouds, environment-map IBL, SSIL,
/// antialiasing).
///
/// Runs every frame and assigns only when the value differs, so a change to
/// `CurrentProject.config` made by anything (Settings, a script, a project
/// switch) is picked up on the next frame and nothing is re-flagged otherwise.
/// With no project loaded the last tier is kept.
pub fn sync_graphics_quality(
    project: Option<Res<CurrentProject>>,
    mut resolved: ResMut<ResolvedGraphicsQuality>,
) {
    let Some(project) = project else {
        return;
    };
    let q = project.config.rendering.graphics_quality;
    if resolved.0 != q {
        resolved.0 = q;
        info!("[graphics] quality tier: {:?}", q);
    }
}

/// Whether a camera is one the tier governs in this session. See the module
/// doc: every 3D camera in a game, only the viewport cameras in the editor.
fn governed(editor: bool, is_viewport: bool, isolated: bool) -> bool {
    !isolated && (!editor || is_viewport)
}

/// A camera's view of the passes the tier can strip.
type TierCamera = (
    Entity,
    Has<ViewportCamera>,
    Has<IsolatedCamera>,
    Has<Bloom>,
    Has<TemporalAntiAliasing>,
    Has<AutoExposure>,
    Has<ScreenSpaceAmbientOcclusion>,
    Option<&'static mut AtmosphereSettings>,
);

/// Force the resolved tier onto the cameras it governs.
pub fn enforce_graphics_quality(
    quality: Res<ResolvedGraphicsQuality>,
    session: Option<Res<EditorSession>>,
    mut last: Local<Option<GraphicsQuality>>,
    routing: Option<ResMut<EffectRouting>>,
    mut commands: Commands,
    mut cameras: Query<TierCamera, With<Camera3d>>,
    shadow_map: Option<ResMut<DirectionalLightShadowMap>>,
) {
    let q = quality.0;
    let editor = session.is_some_and(|s| s.is_editor());

    // On a tier change, nudge the routers so an effect a lower tier had
    // stripped is re-applied from its (untouched) scene source; the force
    // below then re-strips whatever the new tier still forbids. A downward
    // change therefore costs at most a one-frame re-enable, and an upward one
    // restores cleanly.
    if *last != Some(q) {
        if let Some(mut routing) = routing {
            routing.set_changed();
        }
        *last = Some(q);
    }

    for (entity, is_viewport, isolated, bloom, taa, ae, ssao, atmosphere) in &mut cameras {
        if !governed(editor, is_viewport, isolated) {
            continue;
        }
        // Removing only what is present, so a camera already at the tier is
        // not handed a command every frame.
        if bloom && !q.bloom() {
            commands.entity(entity).remove::<Bloom>();
        }
        if taa && !q.taa() {
            commands.entity(entity).remove::<TemporalAntiAliasing>();
        }
        if ae && !q.auto_exposure() {
            commands.entity(entity).remove::<AutoExposure>();
        }
        if ssao && !q.ssao() {
            commands.entity(entity).remove::<ScreenSpaceAmbientOcclusion>();
        }
        // Drop the raymarched sky to the ~40x cheaper LookupTexture path below
        // `High`. `AtmosphereMode` is not `PartialEq`, so the assignment is
        // gated on a `matches!` read to avoid re-flagging every frame.
        if let Some(mut settings) = atmosphere {
            if !q.atmosphere_raymarched()
                && matches!(settings.rendering_method, AtmosphereMode::Raymarched)
            {
                settings.rendering_method = AtmosphereMode::LookupTexture;
            }
        }
    }

    // A global resource shared by every cascade of every directional light, so
    // it is set once rather than per camera. Read per frame by Bevy's shadow
    // prep; gated on difference so it is not re-flagged every frame.
    if let Some(mut sm) = shadow_map {
        let target = q.shadow_map_size();
        if sm.size != target {
            sm.size = target;
        }
    }
}

/// Sky probes taken off their camera to be put back at a new size next frame.
///
/// Holds the probe each camera had, size already corrected, so everything
/// else about it (intensity, the unfiltered mode Solari asks for) survives the
/// round trip.
#[derive(Resource, Default)]
pub struct PendingSkyProbes(Vec<(Entity, AtmosphereEnvironmentMapLight)>);

/// Resize every sky probe whose face size no longer matches the tier, by
/// removing it this frame and adding it back next frame. See the module doc
/// for why it is not done in place.
///
/// Applies to every probe holder, not only the governed cameras, because
/// `sync_environment_map` forces the tier's size onto every holder too: a
/// probe this skipped would be resized by that in-place path instead.
pub fn rebuild_mismatched_sky_probes(
    quality: Res<ResolvedGraphicsQuality>,
    mut pending: ResMut<PendingSkyProbes>,
    mut commands: Commands,
    probes: Query<(Entity, &AtmosphereEnvironmentMapLight), Without<IsolatedCamera>>,
    exists: Query<Has<AtmosphereEnvironmentMapLight>>,
) {
    // Put back last frame's. A camera that gained a probe meanwhile (a scene
    // rehydrate, the sky being switched back on) already has one built at the
    // current tier, so it is left alone; a despawned camera is simply dropped.
    for (entity, probe) in std::mem::take(&mut pending.0) {
        if let Ok(false) = exists.get(entity) {
            commands.entity(entity).try_insert(probe);
        }
    }

    let size = UVec2::splat(quality.0.ibl_face_size());
    for (entity, probe) in &probes {
        if probe.size == size {
            continue;
        }
        info!(
            "[graphics] rebuilding sky probe on {entity}: {} -> {}",
            probe.size.x, size.x
        );
        // Removing the probe also removes the environment lighting Bevy
        // derived from it (its `on_remove` observer), so the camera holds no
        // half of the old mip chain when the new probe arrives.
        commands.entity(entity).remove::<AtmosphereEnvironmentMapLight>();
        pending.0.push((
            entity,
            AtmosphereEnvironmentMapLight {
                size,
                ..probe.clone()
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project_at(quality: GraphicsQuality) -> CurrentProject {
        let mut config = renzora_project_api::ProjectConfig::default();
        config.rendering.graphics_quality = quality;
        CurrentProject {
            path: Default::default(),
            config,
        }
    }

    fn set_tier(app: &mut App, quality: GraphicsQuality) {
        app.world_mut()
            .resource_mut::<CurrentProject>()
            .config
            .rendering
            .graphics_quality = quality;
    }

    /// An app running the real schedule registration, so the tests exercise
    /// the same `Startup` + chained `PostUpdate` wiring a session gets.
    fn app_at(quality: GraphicsQuality, editor: bool) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(EditorSession(editor));
        app.insert_resource(project_at(quality));
        app.init_resource::<ResolvedGraphicsQuality>();
        app.init_resource::<EffectRouting>();
        app.insert_resource(DirectionalLightShadowMap { size: 4096 });
        register(&mut app);
        app
    }

    /// The full effect stack. The property under test is subtractive (each tier
    /// is defined by what it strips), so a camera that only carried what was
    /// expected to survive could not tell "correctly kept" from "never there".
    fn full_stack() -> impl Bundle {
        (
            Camera3d::default(),
            (
                Bloom::default(),
                TemporalAntiAliasing::default(),
                AutoExposure::default(),
                ScreenSpaceAmbientOcclusion::default(),
            ),
            AtmosphereSettings {
                rendering_method: AtmosphereMode::Raymarched,
                ..default()
            },
        )
    }

    fn game_camera(app: &mut App) -> Entity {
        app.world_mut().spawn(full_stack()).id()
    }

    fn viewport_camera(app: &mut App) -> Entity {
        app.world_mut().spawn((full_stack(), ViewportCamera(0))).id()
    }

    #[test]
    fn high_keeps_the_whole_stack() {
        let mut app = app_at(GraphicsQuality::High, false);
        let cam = game_camera(&mut app);
        app.update();

        let e = app.world().entity(cam);
        assert!(e.contains::<Bloom>());
        assert!(e.contains::<TemporalAntiAliasing>());
        assert!(e.contains::<AutoExposure>());
        assert!(e.contains::<ScreenSpaceAmbientOcclusion>());
        assert!(matches!(
            e.get::<AtmosphereSettings>().unwrap().rendering_method,
            AtmosphereMode::Raymarched
        ));
    }

    /// Medium's whole purpose is killing the heaviest pixel-bound passes (SSAO,
    /// the raymarched sky) while keeping the tonemapped look.
    #[test]
    fn medium_drops_the_screen_space_passes_but_keeps_the_look() {
        let mut app = app_at(GraphicsQuality::Medium, false);
        let cam = game_camera(&mut app);
        app.update();

        let e = app.world().entity(cam);
        assert!(!e.contains::<ScreenSpaceAmbientOcclusion>(), "SSAO must be off below High");
        assert!(matches!(
            e.get::<AtmosphereSettings>().unwrap().rendering_method,
            AtmosphereMode::LookupTexture
        ));
        assert!(e.contains::<Bloom>());
        assert!(e.contains::<TemporalAntiAliasing>());
        assert!(e.contains::<AutoExposure>());
    }

    #[test]
    fn low_strips_every_fullscreen_pass() {
        let mut app = app_at(GraphicsQuality::Low, false);
        let cam = game_camera(&mut app);
        app.update();

        let e = app.world().entity(cam);
        assert!(!e.contains::<Bloom>());
        assert!(!e.contains::<TemporalAntiAliasing>());
        assert!(!e.contains::<AutoExposure>());
        assert!(!e.contains::<ScreenSpaceAmbientOcclusion>());
    }

    /// The atmosphere component must stay resident at every tier: downgrading
    /// its rendering method is the cheap path, removing it is not this
    /// system's call.
    #[test]
    fn the_atmosphere_component_is_never_removed() {
        for quality in [GraphicsQuality::Low, GraphicsQuality::Medium, GraphicsQuality::High] {
            let mut app = app_at(quality, false);
            let cam = game_camera(&mut app);
            app.update();
            assert!(
                app.world().entity(cam).contains::<AtmosphereSettings>(),
                "{quality:?} removed AtmosphereSettings"
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
            let mut app = app_at(quality, false);
            app.update();
            assert_eq!(app.world().resource::<DirectionalLightShadowMap>().size, expected);
        }
    }

    /// The point of the move: a tier written into the live config while the
    /// game runs (by a script, say) is applied, not only the one it started
    /// with.
    #[test]
    fn a_live_config_change_is_applied() {
        let mut app = app_at(GraphicsQuality::High, false);
        let cam = game_camera(&mut app);
        app.update();
        assert!(app.world().entity(cam).contains::<Bloom>());

        set_tier(&mut app, GraphicsQuality::Low);
        app.update();

        assert_eq!(app.world().resource::<ResolvedGraphicsQuality>().0, GraphicsQuality::Low);
        assert!(!app.world().entity(cam).contains::<Bloom>(), "the live tier was not applied");
        assert_eq!(app.world().resource::<DirectionalLightShadowMap>().size, 512);
    }

    /// The tier is resolved in `Startup`, so a game's first `Update` (where its
    /// cameras and their probes are built) already sees the project's tier.
    #[test]
    fn the_tier_is_resolved_before_the_first_update() {
        let mut app = app_at(GraphicsQuality::High, false);
        app.world_mut().run_schedule(Startup);
        assert_eq!(app.world().resource::<ResolvedGraphicsQuality>().0, GraphicsQuality::High);
    }

    /// Each router only re-applies its effect from the source when routing
    /// changes, so a raised tier has to poke `EffectRouting`, and an unchanged
    /// one must not (routers would re-sync every frame).
    #[test]
    fn a_tier_change_nudges_the_routers_exactly_once() {
        /// What a router sees each frame: its own `routing.is_changed()`. Read
        /// from a system rather than the `World`, because `App::update` clears
        /// the world's trackers on the way out.
        #[derive(Resource, Default)]
        struct Seen(Vec<bool>);

        let mut app = app_at(GraphicsQuality::Low, false);
        app.init_resource::<Seen>();
        app.add_systems(Last, |routing: Res<EffectRouting>, mut seen: ResMut<Seen>| {
            seen.0.push(routing.is_changed());
        });

        app.update();
        app.update();
        set_tier(&mut app, GraphicsQuality::High);
        app.update();
        app.update();

        assert_eq!(
            app.world().resource::<Seen>().0,
            // First frame: the first tier is a change. Second: unchanged, so no
            // poke (routers would re-sync every frame). Third: raised. Fourth:
            // settled again.
            vec![true, false, true, false]
        );
    }

    /// In the editor the tier lands on the viewport cameras only. Other 3D
    /// cameras there (the material preview, thumbnails) are not the game's
    /// view, and the scene's own camera carries the authored sources.
    #[test]
    fn an_editor_session_governs_only_the_viewport_cameras() {
        let mut app = app_at(GraphicsQuality::Low, true);
        let viewport = viewport_camera(&mut app);
        let other = game_camera(&mut app);
        app.update();

        assert!(!app.world().entity(viewport).contains::<Bloom>());
        assert!(app.world().entity(other).contains::<Bloom>(), "a non-viewport camera was stripped");
    }

    #[test]
    fn an_isolated_camera_is_never_touched() {
        let mut app = app_at(GraphicsQuality::Low, false);
        let cam = app.world_mut().spawn((full_stack(), IsolatedCamera)).id();
        app.update();
        assert!(app.world().entity(cam).contains::<Bloom>());
    }

    #[test]
    fn with_no_project_loaded_the_last_tier_is_kept() {
        let mut app = app_at(GraphicsQuality::Low, false);
        app.update();
        app.world_mut().remove_resource::<CurrentProject>();
        app.update();
        assert_eq!(app.world().resource::<ResolvedGraphicsQuality>().0, GraphicsQuality::Low);
    }

    /// A tier change that resizes the sky probe takes it off the camera for
    /// exactly one frame and puts it back at the new size, keeping everything
    /// else it had. Done in place, the resize can race Bevy's filtered-map
    /// allocation and crash on a missing mip; see the module doc.
    #[test]
    fn a_live_tier_change_rebuilds_the_sky_probe_over_two_frames() {
        let mut app = app_at(GraphicsQuality::Medium, false);
        let cam = app
            .world_mut()
            .spawn((
                Camera3d::default(),
                AtmosphereEnvironmentMapLight {
                    intensity: 0.25,
                    filtered: false,
                    size: UVec2::splat(GraphicsQuality::Medium.ibl_face_size()),
                    ..default()
                },
            ))
            .id();
        app.update();
        assert!(app.world().entity(cam).contains::<AtmosphereEnvironmentMapLight>());

        set_tier(&mut app, GraphicsQuality::High);
        app.update();
        assert!(
            !app.world().entity(cam).contains::<AtmosphereEnvironmentMapLight>(),
            "the probe must be off the camera for the frame between sizes"
        );

        app.update();
        let probe = app
            .world()
            .entity(cam)
            .get::<AtmosphereEnvironmentMapLight>()
            .expect("the probe was not put back");
        assert_eq!(probe.size, UVec2::splat(GraphicsQuality::High.ibl_face_size()));
        assert_eq!(probe.intensity, 0.25, "the probe's intensity was lost");
        assert!(!probe.filtered, "the probe's filtering mode was lost");

        // Settled: a further frame at the same tier leaves it alone.
        app.update();
        assert!(app.world().entity(cam).contains::<AtmosphereEnvironmentMapLight>());
        assert!(app.world().resource::<PendingSkyProbes>().0.is_empty());
    }

    /// A probe already at the tier's size is never touched, so a game whose
    /// cameras were built after `Startup` resolved the tier has nothing rebuilt.
    #[test]
    fn a_probe_at_the_right_size_is_left_alone() {
        let mut app = app_at(GraphicsQuality::High, false);
        let cam = app
            .world_mut()
            .spawn((
                Camera3d::default(),
                AtmosphereEnvironmentMapLight {
                    size: UVec2::splat(GraphicsQuality::High.ibl_face_size()),
                    ..default()
                },
            ))
            .id();
        app.update();
        assert!(app.world().entity(cam).contains::<AtmosphereEnvironmentMapLight>());
        assert!(app.world().resource::<PendingSkyProbes>().0.is_empty());
    }
}
