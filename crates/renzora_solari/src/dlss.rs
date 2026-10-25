//! DLSS Ray Reconstruction on Solari cameras, with Renzora's own denoiser as
//! the fallback everywhere it cannot run.
//!
//! [`SolariGi::dlss`] is on by default. Whether it takes effect is decided
//! here, once a frame per routed camera: only when the binary was built with
//! the DLSS SDK (the `dlss` feature, which `cargo renzora` turns on by itself
//! when it finds the SDK), and the GPU, driver and renderer report Ray
//! Reconstruction support. Bevy's Solari skips the built-in denoiser on a
//! camera that carries `Dlss<DlssRayReconstructionFeature>`, so attaching that
//! component is the whole switch, and removing it hands the image straight
//! back to the denoiser.
//!
//! Without the `dlss` feature this module only reports [`DlssSupport`] as not
//! compiled, so the inspector can say why the toggle does nothing.

use bevy::prelude::*;
use renzora::DlssSupport;

pub(crate) fn register(app: &mut App) {
    #[cfg(feature = "dlss")]
    {
        app.add_systems(Startup, report_support)
            .add_systems(Update, imp::sync_dlss_cameras.after(crate::sync_solari_cameras));
    }
    #[cfg(not(feature = "dlss"))]
    app.insert_resource(DlssSupport::default());
}

/// Publish what DLSS can do on this run. Startup rather than `build`, because
/// Bevy's DLSS plugin decides support in its own `finish`, after every plugin
/// has built.
#[cfg(feature = "dlss")]
fn report_support(
    mut commands: Commands,
    rr: Option<Res<bevy::anti_alias::dlss::DlssRayReconstructionSupported>>,
) {
    let support = DlssSupport {
        compiled: true,
        ray_reconstruction: rr.is_some(),
    };
    if support.ray_reconstruction {
        info!("[solari] DLSS Ray Reconstruction is available");
    } else {
        info!(
            "[solari] DLSS Ray Reconstruction is not available on this GPU, driver or \
             renderer; Solari uses its built-in denoiser"
        );
    }
    commands.insert_resource(support);
}

#[cfg(feature = "dlss")]
mod imp {
    use bevy::anti_alias::dlss::{Dlss, DlssPerfQualityMode, DlssRayReconstructionFeature};
    use bevy::anti_alias::taa::TemporalAntiAliasing;
    use bevy::prelude::*;
    use bevy::render::camera::{MipBias, TemporalJitter};
    use bevy::solari::realtime::SolariLighting;
    use renzora::{DlssMode, DlssSupport, EffectRouting, SolariGi};

    type RayReconstruction = Dlss<DlssRayReconstructionFeature>;

    /// The smallest output, in pixels on its shorter side, DLSS is attached
    /// to. Well under any real viewport, and big enough that every quality
    /// mode still renders at a sensible size.
    const MIN_OUTPUT_SIZE: u32 = 128;

    /// On a camera this plugin gave DLSS: which of the components DLSS
    /// requires were already there. `Dlss` pulls in `TemporalJitter` and
    /// `MipBias`, and removing `Dlss` leaves them behind; a jitter nothing
    /// resolves shakes the whole image. So they go when DLSS goes, unless
    /// something else had put them there first.
    #[derive(Component)]
    pub(crate) struct DlssByRenzora {
        had_jitter: bool,
        had_mip_bias: bool,
    }

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

    /// Give each routed Solari camera DLSS Ray Reconstruction while it is
    /// wanted and supported, and take it away otherwise.
    ///
    /// TAA is removed from a camera while DLSS is on it: DLSS anti-aliases
    /// the image itself, and two passes resolving one jitter fight each
    /// other. The anti-aliasing plugin only re-applies TAA when its settings
    /// or the routing change, so taking DLSS away marks the routing changed,
    /// which puts TAA back where it is configured.
    #[allow(clippy::type_complexity)]
    pub(crate) fn sync_dlss_cameras(
        mut commands: Commands,
        support: Option<Res<DlssSupport>>,
        sources: Query<&SolariGi>,
        mut cameras: Query<(
            Has<SolariLighting>,
            Option<&mut RayReconstruction>,
            Option<&DlssByRenzora>,
            Has<TemporalAntiAliasing>,
            Has<TemporalJitter>,
            Has<MipBias>,
            Option<&Camera>,
        )>,
        mut routing: ResMut<EffectRouting>,
    ) {
        let supported = support.is_some_and(|s| s.ray_reconstruction);
        let mut dropped = false;
        let targets: Vec<(Entity, Option<(bool, DlssMode)>)> = routing
            .iter()
            .map(|(target, list)| {
                let gi = list.iter().find_map(|&s| sources.get(s).ok());
                (*target, gi.map(|gi| (gi.enabled && gi.dlss, gi.dlss_mode)))
            })
            .collect();
        for (target, wanted) in targets {
            let Ok((solari, dlss, ours, taa, jitter, mip_bias, camera)) = cameras.get_mut(target)
            else {
                continue;
            };
            let (want, mode) = wanted.unwrap_or((false, DlssMode::Auto));
            // DLSS takes its render size from the camera's output size, and an
            // output a few pixels across leaves it next to nothing to render
            // at. The game in the editor's Game panel starts like that: its
            // window is squashed to 1 x 1 until the panel is laid out, and
            // SSAO then failed to create its textures at that size and Play
            // crashed. Below this size the built-in denoiser runs instead.
            let big_enough = camera
                .and_then(|c| c.physical_target_size())
                .is_some_and(|size| size.min_element() >= MIN_OUTPUT_SIZE);
            let on = want && supported && solari && big_enough;
            match (on, dlss) {
                (true, Some(mut dlss)) => {
                    let mode = quality(mode);
                    if dlss.perf_quality_mode != mode {
                        dlss.perf_quality_mode = mode;
                        dlss.reset = true;
                    }
                    if taa {
                        commands.entity(target).try_remove::<TemporalAntiAliasing>();
                    }
                }
                (true, None) => {
                    let mut ec = commands.entity(target);
                    ec.try_insert(DlssByRenzora {
                        had_jitter: jitter,
                        had_mip_bias: mip_bias,
                    });
                    ec.try_insert(RayReconstruction {
                        perf_quality_mode: quality(mode),
                        ..default()
                    });
                    if taa {
                        ec.try_remove::<TemporalAntiAliasing>();
                    }
                }
                (false, Some(_)) => {
                    // Only DLSS this plugin attached; one put there by hand
                    // or by a script is left alone.
                    let Some(ours) = ours else { continue };
                    let mut ec = commands.entity(target);
                    ec.try_remove::<(RayReconstruction, DlssByRenzora)>();
                    if !ours.had_jitter {
                        ec.try_remove::<TemporalJitter>();
                    }
                    if !ours.had_mip_bias {
                        ec.try_remove::<MipBias>();
                    }
                    dropped = true;
                }
                (false, None) => {}
            }
        }
        if dropped {
            routing.set_changed();
        }
    }
}
