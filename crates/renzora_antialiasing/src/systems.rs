//! Putting a camera's [`AntiAliasing`] onto the cameras that render it.

use bevy::anti_alias::fxaa::{Fxaa, Sensitivity};
use bevy::anti_alias::smaa::{Smaa, SmaaPreset};
use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::core_pipeline::oit::OrderIndependentTransparencySettings;
use bevy::core_pipeline::prepass::DeferredPrepass;
use bevy::pbr::{AtmosphereSettings, ScreenSpaceAmbientOcclusion};
use bevy::prelude::*;
use bevy::render::camera::{MipBias, TemporalJitter};
use renzora::{DlssMode, DlssSupport, EffectRouting, ResolvedGraphicsQuality};

use crate::compat::{blocker, CameraCaps, RunCaps};
use crate::settings::{AntiAliasing, AntiAliasingMethod};

#[cfg(feature = "dlss")]
pub(crate) type SuperResolution =
    bevy::anti_alias::dlss::Dlss<bevy::anti_alias::dlss::DlssSuperResolutionFeature>;
#[cfg(feature = "dlss")]
pub(crate) type RayReconstruction =
    bevy::anti_alias::dlss::Dlss<bevy::anti_alias::dlss::DlssRayReconstructionFeature>;

/// Never inserted: what the queries look for where Bevy's DLSS is not compiled.
#[cfg(not(feature = "dlss"))]
#[derive(Component)]
pub(crate) struct SuperResolution;
/// Never inserted, as [`SuperResolution`].
#[cfg(not(feature = "dlss"))]
#[derive(Component)]
pub(crate) struct RayReconstruction;

/// DLSS takes its render size from the camera's output, and an output a few
/// pixels across leaves it next to nothing to render at. The Game panel's
/// window starts at 1 x 1 until it is laid out, which is the case this guards;
/// Solari's Ray Reconstruction stays off below the same size for the same reason.
const DLSS_MIN_OUTPUT: u32 = 128;

/// On a rendering camera this plugin has given anti-aliasing to. Lets the
/// reconcile take it all away again when the camera stops being routed an
/// [`AntiAliasing`] (the component removed, or the camera deleted from the
/// scene), instead of leaving the last method running. That leftover is
/// exactly what fog and SSAO did when the World Environment was deleted.
#[derive(Component)]
pub(crate) struct AntiAliasingApplied;

/// On a camera this plugin gave TAA or DLSS: whether the jitter and mip bias
/// they require were already there. Both pull in `TemporalJitter` and
/// `MipBias`, and neither takes them away when it goes. Bevy still applies a
/// left-behind jitter to the view, frozen at whatever offset the last temporal
/// frame set (up to 3/8 of a pixel, since only TAA and DLSS advance it), and a
/// left-behind `MipBias(-1)` keeps sampling textures one mip sharper than they
/// should be, which shimmers with nothing to resolve it. They go when the last
/// temporal method goes, unless something else put them there.
#[derive(Component)]
pub(crate) struct TemporalByAntiAliasing {
    had_jitter: bool,
    had_mip_bias: bool,
}

/// What a camera should be running.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Want {
    msaa: Msaa,
    fxaa: Option<Sensitivity>,
    /// The preset's index, for [`smaa_preset`]: Bevy's `SmaaPreset` is not
    /// `Debug`, which the tests' comparisons need.
    smaa: Option<u32>,
    taa: bool,
    dlss: Option<DlssMode>,
}

impl Want {
    const NOTHING: Self = Self { msaa: Msaa::Off, fxaa: None, smaa: None, taa: false, dlss: None };
}

fn sensitivity(i: u32) -> Sensitivity {
    match i {
        0 => Sensitivity::Low,
        1 => Sensitivity::Medium,
        3 => Sensitivity::Ultra,
        4 => Sensitivity::Extreme,
        _ => Sensitivity::High,
    }
}

fn smaa_preset(i: u32) -> SmaaPreset {
    match i {
        0 => SmaaPreset::Low,
        1 => SmaaPreset::Medium,
        3 => SmaaPreset::Ultra,
        _ => SmaaPreset::High,
    }
}

fn want(aa: &AntiAliasing, cam: &CameraCaps, run: &RunCaps, big_enough: bool) -> Want {
    if !aa.enabled || blocker(aa.method, cam, run).is_some() {
        return Want::NOTHING;
    }
    let mut w = Want::NOTHING;
    match aa.method {
        AntiAliasingMethod::Msaa => w.msaa = aa.msaa(),
        AntiAliasingMethod::Fxaa => w.fxaa = Some(sensitivity(aa.fxaa_quality)),
        AntiAliasingMethod::Smaa => w.smaa = Some(aa.smaa_quality.min(3)),
        AntiAliasingMethod::Taa => w.taa = true,
        AntiAliasingMethod::Dlss => w.dlss = big_enough.then_some(aa.dlss_mode),
    }
    w
}

type Rendering<'a> = (
    Option<&'a mut Msaa>,
    Option<&'a Fxaa>,
    Option<&'a Smaa>,
    Has<TemporalAntiAliasing>,
    Option<&'a SuperResolution>,
    Has<AntiAliasingApplied>,
    Has<TemporalByAntiAliasing>,
    (
        Has<DeferredPrepass>,
        Has<ScreenSpaceAmbientOcclusion>,
        Has<AtmosphereSettings>,
        Has<OrderIndependentTransparencySettings>,
        Has<RayReconstruction>,
    ),
    (Has<TemporalJitter>, Has<MipBias>, Option<&'a Camera>),
);

/// Give every routed camera exactly the anti-aliasing its source asks for and
/// the rules allow, every frame, touching it only where it differs.
///
/// Declarative rather than change-driven, because most of what decides the
/// answer is not this component: SSAO or the sky coming on blocks MSAA, the
/// Low tier strips TAA, Solari's Ray Reconstruction arriving takes over from
/// TAA and DLSS. Each of those owners only writes on its own transitions, so a
/// change-driven router here would miss them, and the per-frame form agrees
/// with all of them instead of racing them.
///
/// In `PostUpdate`, after the `Update` systems that add or remove the things
/// MSAA cannot share a camera with, so the sky and MSAA never meet in a frame
/// that renders (that meeting is a wgpu validation crash).
#[allow(clippy::type_complexity)]
pub(crate) fn reconcile_anti_aliasing(
    mut commands: Commands,
    sources: Query<&AntiAliasing>,
    routing: Res<EffectRouting>,
    quality: Option<Res<ResolvedGraphicsQuality>>,
    dlss: Option<Res<DlssSupport>>,
    mut cameras: Query<Rendering>,
    applied: Query<Entity, With<AntiAliasingApplied>>,
    mut seen: Local<Vec<Entity>>,
) {
    let run = RunCaps {
        taa_off_by_quality: quality.is_some_and(|q| !q.0.taa()),
        dlss: dlss.as_deref().copied().unwrap_or_default(),
    };
    seen.clear();
    for (target, list) in routing.iter() {
        let source = list.iter().find_map(|&s| sources.get(s).ok());
        apply(&mut commands, &mut cameras, *target, source, &run);
        seen.push(*target);
    }
    // A camera that dropped out of the routing altogether keeps whatever it
    // last ran, unless it is taken away here.
    for cam in &applied {
        if !seen.contains(&cam) {
            apply(&mut commands, &mut cameras, cam, None, &run);
        }
    }
}

fn apply(
    commands: &mut Commands,
    cameras: &mut Query<Rendering>,
    target: Entity,
    source: Option<&AntiAliasing>,
    run: &RunCaps,
) {
    let Ok((msaa, fxaa, smaa, has_taa, sr, applied, temporal, caps, (jitter, mip_bias, camera))) =
        cameras.get_mut(target)
    else {
        return;
    };
    // Not ours to manage: a camera no anti-aliasing was ever routed to keeps
    // whatever its spawn gave it.
    if source.is_none() && !applied {
        return;
    }
    let (deferred, ssao, sky, oit, ray_reconstruction) = caps;
    let cam = CameraCaps { deferred, ssao, sky, oit, ray_reconstruction };
    let big_enough = camera
        .and_then(|c| c.physical_target_size())
        .is_some_and(|size| size.min_element() >= DLSS_MIN_OUTPUT);
    let w = source.map_or(Want::NOTHING, |aa| want(aa, &cam, run, big_enough));

    let mut ec = commands.entity(target);
    match msaa {
        Some(mut m) => {
            m.set_if_neq(w.msaa);
        }
        None if w.msaa != Msaa::Off => {
            ec.try_insert(w.msaa);
        }
        None => {}
    }

    let fxaa_now = fxaa.filter(|f| f.enabled && f.edge_threshold == f.edge_threshold_min);
    if fxaa_now.map(|f| f.edge_threshold) != w.fxaa {
        match w.fxaa {
            Some(s) => {
                ec.try_insert(Fxaa { enabled: true, edge_threshold: s, edge_threshold_min: s });
            }
            None => {
                ec.try_remove::<Fxaa>();
            }
        }
    }

    if smaa.map(|s| s.preset) != w.smaa.map(smaa_preset) {
        match w.smaa {
            Some(i) => {
                ec.try_insert(Smaa { preset: smaa_preset(i) });
            }
            None => {
                ec.try_remove::<Smaa>();
            }
        }
    }

    let starts_temporal = (w.taa && !has_taa) || (w.dlss.is_some() && sr.is_none());
    if starts_temporal && !temporal {
        ec.try_insert(TemporalByAntiAliasing { had_jitter: jitter, had_mip_bias: mip_bias });
    }

    if has_taa != w.taa {
        if w.taa {
            ec.try_insert(TemporalAntiAliasing::default());
        } else {
            ec.try_remove::<TemporalAntiAliasing>();
        }
    }

    apply_dlss(&mut ec, sr, w.dlss);

    match (source.is_some(), applied) {
        (true, false) => {
            ec.try_insert(AntiAliasingApplied);
        }
        (false, true) => {
            ec.try_remove::<AntiAliasingApplied>();
        }
        _ => {}
    }
}

#[cfg(feature = "dlss")]
fn apply_dlss(ec: &mut EntityCommands, sr: Option<&SuperResolution>, want: Option<DlssMode>) {
    use bevy::anti_alias::dlss::DlssPerfQualityMode;
    fn quality(mode: DlssMode) -> DlssPerfQualityMode {
        match mode {
            DlssMode::Auto => DlssPerfQualityMode::Auto,
            DlssMode::Dlaa => DlssPerfQualityMode::Dlaa,
            DlssMode::Quality => DlssPerfQualityMode::Quality,
            DlssMode::Balanced => DlssPerfQualityMode::Balanced,
            DlssMode::Performance => DlssPerfQualityMode::Performance,
            DlssMode::UltraPerformance => DlssPerfQualityMode::UltraPerformance,
        }
    }
    match (want, sr) {
        (Some(mode), Some(now)) if now.perf_quality_mode == quality(mode) => {}
        (Some(mode), _) => {
            // A changed mode is a fresh insert with a reset, so DLSS does not
            // blend the old render size's history into the new one.
            ec.try_insert(SuperResolution {
                perf_quality_mode: quality(mode),
                reset: sr.is_some(),
                ..default()
            });
        }
        (None, Some(_)) => {
            ec.try_remove::<SuperResolution>();
        }
        (None, None) => {}
    }
}

/// Without Bevy's DLSS nothing can ask for it: [`blocker`] refuses the method
/// in a build that did not compile it.
#[cfg(not(feature = "dlss"))]
fn apply_dlss(_: &mut EntityCommands, _: Option<&SuperResolution>, _: Option<DlssMode>) {}

/// Take away the jitter and mip bias TAA or DLSS brought, once neither is on
/// the camera any more. See [`TemporalByAntiAliasing`].
///
/// Covers every way the temporal method goes, not only this plugin's: the Low
/// graphics tier strips TAA by itself, and before this ran, that left the
/// viewport offset by its last jitter and sampling at TAA's mip bias.
#[allow(clippy::type_complexity)]
pub(crate) fn release_temporal_jitter(
    mut commands: Commands,
    cameras: Query<(
        Entity,
        &TemporalByAntiAliasing,
        Has<TemporalAntiAliasing>,
        Has<SuperResolution>,
        Has<RayReconstruction>,
    )>,
) {
    for (cam, by_us, taa, sr, rr) in &cameras {
        if taa || sr || rr {
            continue;
        }
        let mut ec = commands.entity(cam);
        ec.try_remove::<TemporalByAntiAliasing>();
        if !by_us.had_jitter {
            ec.try_remove::<TemporalJitter>();
        }
        if !by_us.had_mip_bias {
            ec.try_remove::<MipBias>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run() -> RunCaps {
        RunCaps::default()
    }

    #[test]
    fn disabled_wants_nothing() {
        let aa = AntiAliasing { enabled: false, ..default() };
        assert_eq!(want(&aa, &CameraCaps::default(), &run(), true), Want::NOTHING);
    }

    #[test]
    fn a_blocked_method_wants_nothing_rather_than_something_else() {
        let aa = AntiAliasing { method: AntiAliasingMethod::Msaa, ..default() };
        let deferred = CameraCaps { deferred: true, ..default() };
        assert_eq!(want(&aa, &deferred, &run(), true), Want::NOTHING);
        assert_eq!(want(&aa, &CameraCaps::default(), &run(), true).msaa, Msaa::Sample4);
    }

    #[test]
    fn only_the_selected_method_is_wanted() {
        let aa = AntiAliasing { method: AntiAliasingMethod::Smaa, smaa_quality: 3, ..default() };
        let w = want(&aa, &CameraCaps::default(), &run(), true);
        assert_eq!(w, Want { smaa: Some(3), ..Want::NOTHING });
    }
}
