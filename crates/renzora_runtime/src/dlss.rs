//! NVIDIA DLSS Ray Reconstruction as Solari's denoiser, behind the `dlss`
//! feature (`cargo renzora --dlss`).
//!
//! # Why this is a build switch and not part of the normal build
//!
//! DLSS is not a DLL you can drop in. `dlss_wgpu` links NVIDIA's static NGX
//! library and generates bindings from the SDK headers at compile time, so the
//! build needs the DLSS SDK on disk (`DLSS_SDK`) plus the Vulkan SDK
//! (`VULKAN_SDK`) and clang. The SDK's licence forbids bundling it, so neither CI
//! nor the cross-compile containers have it. The two DLLs (`nvngx_dlss.dll`,
//! `nvngx_dlssd.dll`) are only what ships beside the executable; xtask copies them
//! there when it builds with the switch.
//!
//! It also only works on an RTX card, on the **Vulkan** backend, on Windows or
//! Linux. Anywhere else Bevy simply never inserts
//! [`DlssRayReconstructionSupported`], and everything here stays idle.
//!
//! # Why it lives in the runtime and not in `renzora_solari`
//!
//! The `dlss` feature, and the `DlssInitPlugin` that must be registered before
//! `RenderPlugin`, both belong to this crate, and naming the `Dlss` types in a
//! non-DLSS build is a compile error. Keeping the attach/detach beside them means
//! one crate owns the whole `cfg`, keyed only on Bevy's own `SolariLighting`.
//! (This also dates from when Solari was a native plugin, compiled with no cargo
//! features at all and so unable to `cfg` on DLSS.)
//!
//! # TAA
//!
//! Ray Reconstruction is a temporal anti-aliaser as well as a denoiser, and
//! Bevy's TAA knows nothing about it. Leaving both on a camera stacks two
//! temporal passes, so TAA is set aside while DLSS is attached and handed back
//! when Solari switches off.

use bevy::anti_alias::dlss::{
    Dlss, DlssProjectId, DlssRayReconstructionFeature, DlssRayReconstructionSupported,
};
use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::prelude::*;
use bevy::solari::realtime::SolariLighting;

/// Renzora's DLSS project id. NVIDIA keys driver-side profiles on it, so it has
/// to stay the same across releases; it is not a secret.
const PROJECT_ID: bevy::asset::uuid::Uuid =
    bevy::asset::uuid::uuid!("3f9c2a7e-5b1d-4e86-9a4f-7c2e81d0b6a5");

/// Insert the project id. Must run before `DefaultPlugins`, whose
/// `DlssInitPlugin` panics without it when the feature is on.
pub(crate) fn insert_project_id(app: &mut App) {
    app.insert_resource(DlssProjectId(PROJECT_ID));
}

/// The camera's TAA, held while DLSS Ray Reconstruction stands in for it.
#[derive(Component)]
struct TaaSetAside(TemporalAntiAliasing);

pub(crate) struct DlssSolariPlugin;

impl Plugin for DlssSolariPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, report_support)
            .add_systems(Update, (attach_ray_reconstruction, detach_ray_reconstruction));
    }
}

/// Say once whether Ray Reconstruction is available, because when it is not the
/// only symptom is a noisier image and the reason (wrong backend, no RTX card,
/// missing DLL) is otherwise invisible.
fn report_support(supported: Option<Res<DlssRayReconstructionSupported>>) {
    if supported.is_some() {
        info!("[dlss] Ray Reconstruction available: Solari will be denoised by DLSS");
    } else {
        warn!(
            "[dlss] built with DLSS, but Ray Reconstruction is unavailable on this device. \
             It needs an NVIDIA RTX GPU, the Vulkan backend (Settings > Renderer), and \
             nvngx_dlssd.dll beside the executable. Solari falls back to ReSTIR."
        );
    }
}

/// Give every Solari camera DLSS Ray Reconstruction, setting its TAA aside.
fn attach_ray_reconstruction(
    supported: Option<Res<DlssRayReconstructionSupported>>,
    cameras: Query<
        (Entity, Option<&TemporalAntiAliasing>),
        (With<SolariLighting>, Without<Dlss<DlssRayReconstructionFeature>>),
    >,
    mut commands: Commands,
) {
    if supported.is_none() {
        return;
    }
    for (entity, taa) in &cameras {
        let mut camera = commands.entity(entity);
        camera.insert(Dlss::<DlssRayReconstructionFeature>::default());
        if let Some(taa) = taa {
            camera
                .remove::<TemporalAntiAliasing>()
                .insert(TaaSetAside(taa.clone()));
        }
    }
}

/// Take DLSS off cameras that no longer run Solari, and give TAA back.
///
/// `TemporalJitter` and `MipBias` stay, unlike in Bevy's example: TAA requires
/// them too, and it is about to be restored.
///
/// Also sets aside any TAA that effect routing put back while DLSS was on, so
/// the two never run together.
fn detach_ray_reconstruction(
    dlss_off: Query<
        (Entity, Option<&TaaSetAside>),
        (With<Dlss<DlssRayReconstructionFeature>>, Without<SolariLighting>),
    >,
    taa_back: Query<
        (Entity, &TemporalAntiAliasing),
        (With<Dlss<DlssRayReconstructionFeature>>, With<SolariLighting>),
    >,
    mut commands: Commands,
) {
    for (entity, set_aside) in &dlss_off {
        let mut camera = commands.entity(entity);
        camera.remove::<(Dlss<DlssRayReconstructionFeature>, TaaSetAside)>();
        if let Some(TaaSetAside(taa)) = set_aside {
            camera.insert(taa.clone());
        }
    }
    // Effect routing re-inserts TAA whenever the World Environment's
    // anti-aliasing changes. While DLSS is on, keep the newer settings for later
    // rather than letting both run.
    for (entity, taa) in &taa_back {
        commands
            .entity(entity)
            .remove::<TemporalAntiAliasing>()
            .insert(TaaSetAside(taa.clone()));
    }
}
