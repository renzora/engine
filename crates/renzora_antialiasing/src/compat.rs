//! Which anti-aliasing methods can run on a camera, and why not when they can't.
//!
//! One set of rules, read by both sides: [`crate::systems`] applies a method
//! only where [`blocker`] finds nothing in its way, and the inspector puts a
//! warning on every option [`blocker`] would refuse. Two lists would drift, and
//! the cost of drift is the exact thing the warning exists to prevent: a method
//! that reads as on and draws nothing.
//!
//! A blocked method is held off rather than forced through, because most of
//! these combinations do not fail politely. MSAA under the sky's depth binding
//! is a wgpu validation crash, and MSAA under deferred shading is a sample-count
//! mismatch wgpu rejects (see `renzora_engine::ensure_deferred_prepass_on_cameras`).
//! The component keeps the choice, and the method takes effect the moment the
//! thing in its way goes.

use bevy::core_pipeline::oit::OrderIndependentTransparencySettings;
use bevy::core_pipeline::prepass::DeferredPrepass;
use bevy::pbr::{AtmosphereSettings, ScreenSpaceAmbientOcclusion};
use bevy::prelude::*;
use renzora::{DlssSupport, EffectRouting};

use crate::settings::AntiAliasingMethod;

/// What about one rendering camera decides which methods it can run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CameraCaps {
    /// Renders through the deferred G-buffer: the project's Deferred mode, or
    /// Solari forcing it.
    pub deferred: bool,
    pub ssao: bool,
    /// The procedural sky is on this camera. Parked (sky off) does not count:
    /// the sky coming back is seen in the same frame, before anything renders.
    pub sky: bool,
    pub oit: bool,
    /// Solari's DLSS Ray Reconstruction is attached, which already
    /// anti-aliases the image.
    pub ray_reconstruction: bool,
}

/// What about the run as a whole decides it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunCaps {
    /// The tier forbids TAA (Low).
    pub taa_off_by_quality: bool,
    pub dlss: DlssSupport,
}

impl RunCaps {
    pub fn from_world(world: &World) -> Self {
        Self {
            taa_off_by_quality: world
                .get_resource::<renzora::ResolvedGraphicsQuality>()
                .is_some_and(|q| !q.0.taa()),
            dlss: world.get_resource::<DlssSupport>().copied().unwrap_or_default(),
        }
    }
}

/// Why a method cannot run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blocker {
    MsaaDeferred,
    MsaaSsao,
    MsaaSky,
    MsaaOit,
    TaaQualityTier,
    /// Ray Reconstruction is on the camera (TAA and DLSS alike step aside).
    RayReconstruction,
    DlssNotCompiled,
    DlssUnsupported,
}

impl Blocker {
    /// The tooltip behind the warning icon: what is in the way, and what to
    /// change to get the method back.
    pub fn message(self) -> &'static str {
        match self {
            Self::MsaaDeferred => {
                "MSAA only works with Forward rendering, and this camera renders Deferred. \
                 Switch Project Settings > Rendering to Forward, or pick another method."
            }
            Self::MsaaSsao => "MSAA cannot run while SSAO is on for this camera.",
            Self::MsaaSky => "MSAA cannot run while the sky (atmosphere) is on for this camera.",
            Self::MsaaOit => "MSAA cannot run with order-independent transparency on.",
            Self::TaaQualityTier => {
                "Graphics Quality Low turns TAA off. Raise it in Settings > Project > Rendering."
            }
            Self::RayReconstruction => {
                "Solari's DLSS Ray Reconstruction already anti-aliases this camera."
            }
            Self::DlssNotCompiled => "This build has no DLSS. Build with the DLSS SDK to use it.",
            Self::DlssUnsupported => "DLSS needs an NVIDIA RTX GPU on the Vulkan renderer.",
        }
    }
}

/// The first thing stopping `method` on a camera with `cam`, if any.
pub fn blocker(method: AntiAliasingMethod, cam: &CameraCaps, run: &RunCaps) -> Option<Blocker> {
    match method {
        AntiAliasingMethod::Msaa => {
            if cam.deferred {
                Some(Blocker::MsaaDeferred)
            } else if cam.ssao {
                Some(Blocker::MsaaSsao)
            } else if cam.sky {
                Some(Blocker::MsaaSky)
            } else if cam.oit {
                Some(Blocker::MsaaOit)
            } else {
                None
            }
        }
        // Post-process passes on the finished image: nothing in the way.
        AntiAliasingMethod::Fxaa | AntiAliasingMethod::Smaa => None,
        AntiAliasingMethod::Taa => {
            if cam.ray_reconstruction {
                Some(Blocker::RayReconstruction)
            } else if run.taa_off_by_quality {
                Some(Blocker::TaaQualityTier)
            } else {
                None
            }
        }
        AntiAliasingMethod::Dlss => {
            if !run.dlss.compiled {
                Some(Blocker::DlssNotCompiled)
            } else if !run.dlss.super_resolution {
                Some(Blocker::DlssUnsupported)
            } else if cam.ray_reconstruction {
                Some(Blocker::RayReconstruction)
            } else {
                None
            }
        }
    }
}

/// Read [`CameraCaps`] off an entity.
pub fn camera_caps(world: &World, camera: Entity) -> CameraCaps {
    let Ok(e) = world.get_entity(camera) else {
        return CameraCaps::default();
    };
    CameraCaps {
        deferred: e.contains::<DeferredPrepass>(),
        ssao: e.contains::<ScreenSpaceAmbientOcclusion>(),
        sky: e.contains::<AtmosphereSettings>(),
        oit: e.contains::<OrderIndependentTransparencySettings>(),
        ray_reconstruction: e.contains::<crate::systems::RayReconstruction>(),
    }
}

/// Why `method`, set on the camera `source`, will not run, or `None` if it
/// will. What decides it is the cameras the setting reaches: the editor's
/// viewport cameras when `source` is a scene camera routed onto them, or
/// `source` itself when nothing routes it (a game, where the scene camera is
/// the one rendering).
///
/// The first blocked camera answers. In practice every routed camera shares
/// the same render path and effects, so the first one speaks for all.
pub fn method_warning(world: &World, source: Entity, method: AntiAliasingMethod) -> Option<String> {
    let run = RunCaps::from_world(world);
    let mut targets = world
        .get_resource::<EffectRouting>()
        .map(|r| {
            r.iter()
                .filter(|(_, sources)| sources.contains(&source))
                .map(|(target, _)| *target)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if targets.is_empty() {
        targets.push(source);
    }
    targets
        .into_iter()
        .find_map(|t| blocker(method, &camera_caps(world, t), &run))
        .map(|b| b.message().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dlss_ok() -> RunCaps {
        RunCaps {
            dlss: DlssSupport { compiled: true, ray_reconstruction: true, super_resolution: true },
            ..default()
        }
    }

    #[test]
    fn msaa_needs_a_forward_camera_with_nothing_in_the_way() {
        let run = RunCaps::default();
        let clear = CameraCaps::default();
        assert_eq!(blocker(AntiAliasingMethod::Msaa, &clear, &run), None);
        for (cam, why) in [
            (CameraCaps { deferred: true, ..clear }, Blocker::MsaaDeferred),
            (CameraCaps { ssao: true, ..clear }, Blocker::MsaaSsao),
            (CameraCaps { sky: true, ..clear }, Blocker::MsaaSky),
            (CameraCaps { oit: true, ..clear }, Blocker::MsaaOit),
        ] {
            assert_eq!(blocker(AntiAliasingMethod::Msaa, &cam, &run), Some(why));
        }
    }

    #[test]
    fn post_process_methods_run_anywhere() {
        let busy = CameraCaps { deferred: true, ssao: true, sky: true, oit: true, ray_reconstruction: true };
        let run = RunCaps { taa_off_by_quality: true, ..default() };
        assert_eq!(blocker(AntiAliasingMethod::Fxaa, &busy, &run), None);
        assert_eq!(blocker(AntiAliasingMethod::Smaa, &busy, &run), None);
    }

    #[test]
    fn taa_steps_aside_for_ray_reconstruction_and_the_low_tier() {
        let run = RunCaps::default();
        let rr = CameraCaps { ray_reconstruction: true, ..default() };
        assert_eq!(blocker(AntiAliasingMethod::Taa, &rr, &run), Some(Blocker::RayReconstruction));
        let low = RunCaps { taa_off_by_quality: true, ..default() };
        assert_eq!(
            blocker(AntiAliasingMethod::Taa, &CameraCaps::default(), &low),
            Some(Blocker::TaaQualityTier)
        );
    }

    #[test]
    fn dlss_says_whether_it_is_missing_from_the_build_or_the_gpu() {
        let cam = CameraCaps::default();
        assert_eq!(
            blocker(AntiAliasingMethod::Dlss, &cam, &RunCaps::default()),
            Some(Blocker::DlssNotCompiled)
        );
        let no_gpu = RunCaps { dlss: DlssSupport { compiled: true, ..default() }, ..default() };
        assert_eq!(blocker(AntiAliasingMethod::Dlss, &cam, &no_gpu), Some(Blocker::DlssUnsupported));
        assert_eq!(blocker(AntiAliasingMethod::Dlss, &cam, &dlss_ok()), None);
    }
}
