//! Screen-space indirect lighting (SSIL): the light that bounces off one
//! surface onto the ones beside it.
//!
//! Sunlight on a red wall should tint the white floor in front of it, and a
//! lit street should brighten the underside of an awning. Direct lighting and
//! the environment map do neither: the environment map is the sky, not the
//! scene. This pass gathers that bounce from the previous frame's lit image.
//!
//! It is Intel's ASSAO (MIT licence, 2016) as converted to gather light,
//! ported as it stands rather than reinvented, because an earlier version of
//! this crate that departed from it in the details (this frame instead of last,
//! full-resolution taps instead of a blurred mip, a sender cosine instead of a
//! pass-or-fail test, light added without any occlusion) came out an order of
//! magnitude too faint and read as haze when turned up:
//!
//! - a half-resolution gather on the ASSAO disk, each tap mirrored, rotated by
//!   one of twenty matrices chosen per pixel;
//! - every tap reads **last frame's** final colour, reprojected, from the
//!   sixth mip of a chain, so it stands for an area and already holds last
//!   frame's bounce (several bounces build up over frames);
//! - each tap compressed by `c / (1 + luma)` before averaging, uncompressed
//!   after; weighted by the receiver's obscurance and by a pass-or-fail test
//!   on the sender's normal;
//! - an occlusion term beside the light, which dims the ambient light the way
//!   the bounce replaces it;
//! - an edge-aware blur, two wide passes then two narrow ones, and a bilinear
//!   upsample.
//!
//! **Deferred only.** The composite reads each pixel's diffuse colour and
//! normal from the deferred G-buffer; Bevy's PBR shader has no hook to add the
//! light in a forward material. A camera without `DeferredPrepass` gets none.
//!
//! **Solari stands it down**, as it does SSR and SSAO: Solari traces its own
//! indirect light, and a screen-space bounce on top would count it twice.
//!
//! Split the usual way: [`sync_ssil`] decides in the main world which routed
//! cameras get it and with what, as an [`Ssil`] the render world copies; the
//! render world does the rest (`render.rs`, `ssil.wesl`).

mod render;

use bevy::core_pipeline::prepass::DeferredPrepass;
use bevy::light::EnvironmentMapLight;
use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use renzora_scene_api::EffectRouting;
use renzora_project_api::ResolvedGraphicsQuality;
use renzora_render_api::SolariLit;
use serde::{Deserialize, Serialize};

/// The authored SSIL settings, on a World Environment or a camera, routed to
/// the cameras that render it like every other effect.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SsilSettings {
    pub enabled: bool,
    /// How strong the bounce light is.
    pub intensity: f32,
    /// How strongly the sky light a surface's neighbours block is taken away.
    /// Separate from [`Self::intensity`] because the reference ties the two
    /// together, and under bright physical sky light the dimming outweighs a
    /// bounce gathered from mostly shaded surfaces: raising one setting for
    /// both darkened the scene. 0 leaves only the bounce; Bevy's own SSAO, if
    /// the camera has it, dims ambient light too.
    #[reflect(default = "default_occlusion")]
    #[serde(default = "default_occlusion")]
    pub occlusion: f32,
    /// How far light travels between surfaces, in metres.
    pub radius: f32,
    /// Taps per half-resolution pixel, each with a mirrored twin, so the
    /// texture reads are twice this. More is smoother and costs linearly.
    pub samples: u32,
    /// How firmly the blur stops at depth edges, 0 to 1. Lower blurs the
    /// bounce across silhouettes; higher keeps it to its own surface.
    pub sharpness: f32,
    /// How strictly a tap must face the pixel to send it light, 0 to 1. At 0
    /// the back of a wall lights what is in front of it.
    pub normal_rejection: f32,
    /// Distance in metres at which the bounce starts to fade out.
    pub fade_start: f32,
    /// Distance in metres beyond which there is no bounce at all.
    pub fade_end: f32,
}

/// Below the reference's 1. Tuned in the Bistro under a 120,000 lux sun: at 1
/// the dimming outweighed the bounce over a quarter of the frame, because the
/// environment light's own bright ground already stands in for much of the
/// light SSIL gathers, and Bevy's SSAO dims the same ambient light again. 0.3
/// keeps contact darkening in corners without darkening the scene overall.
fn default_occlusion() -> f32 {
    0.3
}

impl Default for SsilSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            intensity: 1.0,
            occlusion: default_occlusion(),
            radius: 5.0,
            samples: 12,
            sharpness: 0.98,
            normal_rejection: 1.0,
            fade_start: 50.0,
            fade_end: 300.0,
        }
    }
}

/// The bounds the inspector offers, applied again here so a hand-edited scene
/// cannot ask for more taps than the disk pattern holds.
pub const SAMPLES: std::ops::RangeInclusive<u32> = 3..=32;

/// The camera's diffuse environment light, which the composite needs to know
/// how much ambient light the occlusion term should take away.
#[derive(Clone, Debug, PartialEq)]
pub struct SsilAmbient {
    pub diffuse_map: Handle<Image>,
    pub intensity: f32,
    pub rotation: Quat,
}

/// What one camera renders, resolved from its routed [`SsilSettings`]. Present
/// exactly while the camera should draw the pass.
#[derive(Component, Clone, Debug, PartialEq, ExtractComponent)]
#[extract_app(bevy::render::RenderApp)]
pub struct Ssil {
    pub intensity: f32,
    pub occlusion: f32,
    pub radius: f32,
    pub samples: u32,
    pub sharpness: f32,
    pub normal_rejection: f32,
    pub fade_start: f32,
    pub fade_end: f32,
    /// The camera's environment light, if it has one. Without it the bounce is
    /// still added, but nothing is dimmed.
    pub ambient: Option<SsilAmbient>,
}

impl Ssil {
    fn from_settings(settings: &SsilSettings, ambient: Option<&EnvironmentMapLight>) -> Self {
        let fade_start = settings.fade_start.max(0.0);
        Self {
            intensity: settings.intensity.max(0.0),
            occlusion: settings.occlusion.max(0.0),
            radius: settings.radius.max(0.05),
            samples: settings.samples.clamp(*SAMPLES.start(), *SAMPLES.end()),
            sharpness: settings.sharpness.clamp(0.0, 1.0),
            normal_rejection: settings.normal_rejection.clamp(0.0, 1.0),
            fade_start,
            fade_end: settings.fade_end.max(fade_start + 0.01),
            ambient: ambient.map(|env| SsilAmbient {
                diffuse_map: env.diffuse_map.clone(),
                intensity: env.intensity,
                rotation: env.rotation,
            }),
        }
    }
}

/// Whether Solari lights a camera this SSIL source is routed to, so the
/// inspector can show SSIL off while it is stood down.
pub fn stood_down_by_solari(world: &World, source: Entity) -> bool {
    let Some(routing) = world.get_resource::<EffectRouting>() else {
        return false;
    };
    routing
        .iter()
        .any(|(target, sources)| sources.contains(&source) && world.get::<SolariLit>(*target).is_some())
}

/// Give every routed camera the [`Ssil`] its settings ask for, and take it off
/// every camera that should not draw it.
///
/// A camera draws it when its first routed source with [`SsilSettings`] has it
/// enabled, the quality tier allows it, the camera renders deferred, and
/// Solari is not lighting it. The component is only rewritten when the answer
/// changes, so a still scene does not re-extract it every frame.
#[allow(clippy::type_complexity)]
pub fn sync_ssil(
    mut commands: Commands,
    sources: Query<&SsilSettings>,
    routing: Option<Res<EffectRouting>>,
    quality: Option<Res<ResolvedGraphicsQuality>>,
    cameras: Query<(
        Option<&Ssil>,
        Has<DeferredPrepass>,
        Has<SolariLit>,
        Option<&EnvironmentMapLight>,
    )>,
    drawing: Query<Entity, With<Ssil>>,
) {
    let tier_allows = quality.is_none_or(|q| q.0.ssil());
    let mut routed = Vec::new();
    if let Some(routing) = routing.as_deref() {
        for (target, source_list) in routing.iter() {
            let Ok((current, deferred, solari, ambient)) = cameras.get(*target) else {
                continue;
            };
            routed.push(*target);
            let want = source_list
                .iter()
                .find_map(|&s| sources.get(s).ok())
                .filter(|s| s.enabled && tier_allows && deferred && !solari)
                .map(|s| Ssil::from_settings(s, ambient));
            match want {
                Some(ssil) if current != Some(&ssil) => {
                    commands.entity(*target).try_insert(ssil);
                }
                None if current.is_some() => {
                    commands.entity(*target).try_remove::<Ssil>();
                }
                _ => {}
            }
        }
    }
    for camera in &drawing {
        if !routed.contains(&camera) {
            commands.entity(camera).try_remove::<Ssil>();
        }
    }
}

#[derive(Default)]
pub struct SsilPlugin;

impl Plugin for SsilPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] SsilPlugin");
        app.register_type::<SsilSettings>();
        app.add_systems(Update, sync_ssil);
        render::build(app);
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_systems(Update, sync_ssil);
        let source = app.world_mut().spawn(SsilSettings::default()).id();
        let camera = app.world_mut().spawn(DeferredPrepass).id();
        app.insert_resource(EffectRouting {
            routes: vec![(camera, vec![source])],
        });
        (app, source, camera)
    }

    fn drawing(app: &App, camera: Entity) -> bool {
        app.world().get::<Ssil>(camera).is_some()
    }

    #[test]
    fn a_routed_deferred_camera_draws_it() {
        let (mut app, _, camera) = app();
        app.update();
        assert!(drawing(&app, camera));
    }

    /// The composite reads the G-buffer, so a forward camera gets nothing
    /// rather than a pass that would read a texture it does not have.
    #[test]
    fn a_forward_camera_does_not() {
        let (mut app, _, camera) = app();
        app.world_mut().entity_mut(camera).remove::<DeferredPrepass>();
        app.update();
        assert!(!drawing(&app, camera));
    }

    /// Solari traces its own bounce: SSIL stands down while it lights the
    /// camera and comes back after, with the setting untouched in between.
    #[test]
    fn solari_stands_it_down_and_hands_it_back() {
        let (mut app, source, camera) = app();
        app.world_mut().entity_mut(camera).insert(SolariLit);
        app.update();
        assert!(!drawing(&app, camera));
        assert!(stood_down_by_solari(app.world(), source));
        app.world_mut().entity_mut(camera).remove::<SolariLit>();
        app.update();
        assert!(drawing(&app, camera));
    }

    #[test]
    fn the_quality_tier_gates_it() {
        use renzora_viewport_api::GraphicsQuality;
        let (mut app, _, camera) = app();
        app.insert_resource(ResolvedGraphicsQuality(GraphicsQuality::Medium));
        app.update();
        assert!(!drawing(&app, camera));
        app.insert_resource(ResolvedGraphicsQuality(GraphicsQuality::High));
        app.update();
        assert!(drawing(&app, camera));
    }

    #[test]
    fn disabling_or_unrouting_takes_it_away() {
        let (mut app, source, camera) = app();
        app.update();
        app.world_mut().get_mut::<SsilSettings>(source).unwrap().enabled = false;
        app.update();
        assert!(!drawing(&app, camera));
        app.world_mut().get_mut::<SsilSettings>(source).unwrap().enabled = true;
        app.update();
        assert!(drawing(&app, camera));
        app.insert_resource(EffectRouting::default());
        app.update();
        assert!(!drawing(&app, camera));
    }

    /// The camera's environment light rides along, so the composite can dim
    /// the ambient light the occlusion term says is blocked.
    #[test]
    fn the_environment_light_is_carried() {
        let (mut app, _, camera) = app();
        app.update();
        assert!(app.world().get::<Ssil>(camera).unwrap().ambient.is_none());
        app.world_mut().entity_mut(camera).insert(EnvironmentMapLight {
            intensity: 0.5,
            ..default()
        });
        app.update();
        let ambient = app.world().get::<Ssil>(camera).unwrap().ambient.clone();
        assert_eq!(ambient.map(|a| a.intensity), Some(0.5));
    }

    /// A scene saved before Occlusion was its own setting still loads, with
    /// the default strength. Scenes load through `FromReflect`, which
    /// fails the whole component on a missing field unless it has a default:
    /// here the struct's `#[reflect(Default)]` supplies it (the field's own
    /// `reflect(default)` is the project's belt and braces).
    #[test]
    fn a_scene_without_the_occlusion_field_reconstructs_by_reflection() {
        use bevy::reflect::structs::DynamicStruct;
        use bevy::reflect::FromReflect;
        let mut old = DynamicStruct::default();
        old.insert("enabled", true);
        old.insert("intensity", 2.0_f32);
        old.insert("radius", 5.0_f32);
        old.insert("samples", 12_u32);
        old.insert("sharpness", 0.98_f32);
        old.insert("normal_rejection", 1.0_f32);
        old.insert("fade_start", 50.0_f32);
        old.insert("fade_end", 300.0_f32);
        let settings = SsilSettings::from_reflect(&old).expect("old scene loads");
        assert_eq!(settings.intensity, 2.0);
        assert_eq!(settings.occlusion, default_occlusion());
    }

    /// Out-of-range values from a hand-edited scene are brought into range
    /// before they reach the GPU.
    #[test]
    fn settings_are_clamped() {
        let wild = SsilSettings {
            samples: 10_000,
            radius: 0.0,
            sharpness: 3.0,
            normal_rejection: -2.0,
            fade_start: 10.0,
            fade_end: 5.0,
            intensity: -1.0,
            occlusion: -1.0,
            ..default()
        };
        let ssil = Ssil::from_settings(&wild, None);
        assert_eq!(ssil.occlusion, 0.0);
        assert_eq!(ssil.samples, *SAMPLES.end());
        assert!(ssil.radius > 0.0);
        assert_eq!(ssil.sharpness, 1.0);
        assert_eq!(ssil.normal_rejection, 0.0);
        assert!(ssil.fade_end > ssil.fade_start);
        assert_eq!(ssil.intensity, 0.0);
    }
}
