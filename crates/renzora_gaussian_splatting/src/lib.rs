//! 3D Gaussian-splatting distribution plugin.
//!
//! Wraps the vendored [`bevy_gaussian_splatting`] renderer as a single
//! dlopen'd cdylib. Scenes author splats through the serializable
//! [`renzora::GaussianSplat`] contract component (a project-relative `.ply` /
//! `.gcloud` path plus per-cloud tuning); this plugin resolves it into the
//! live cloud handle + [`CloudSettings`] the renderer consumes — the same
//! path-in-component / handle-at-runtime split models, audio, and particles
//! use, so the raw asset `Handle` never has to survive the scene serializer.
//!
//! Runtime scope on purpose: the same plugin renders splats in the editor
//! viewport, in-editor play, and the shipped game. Without this plugin in
//! `plugins/`, `GaussianSplat` components ride along as inert data (the host
//! registers the type, see `renzora_engine`).
//!
//! Editor-only wiring (inspector entry, Add-Entity preset) is compile-gated
//! behind the `editor` feature; in a shipped game those registrations are
//! harmless no-ops because the editor registries they target don't exist.

use bevy::prelude::*;

use bevy::render::renderer::RenderDevice;
use bevy_gaussian_splatting::{
    CloudSettings, GaussianCamera, Planar, PlanarGaussian3d, PlanarGaussian3dHandle,
    sort::SortMode,
};
use renzora::GaussianSplat;

#[cfg(feature = "editor")]
mod editor;
mod sog;

/// Plugin-private bookkeeping for a resolved [`GaussianSplat`]. Records which
/// `source` the current handle was loaded from so path edits reload while
/// tuning-only edits (opacity/scale) don't. Not reflect-registered on purpose:
/// it must not serialize into scenes.
#[derive(Component)]
struct GaussianSplatSynced {
    source: String,
}

#[derive(Default)]
pub struct GaussianSplatPlugin;

impl Plugin for GaussianSplatPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] GaussianSplatPlugin (bevy_gaussian_splatting)");
        app.add_plugins(bevy_gaussian_splatting::GaussianSplattingPlugin);

        // Compressed-bundle support on top of the renderer's own .ply/.gcloud
        // loader: .sog (SuperSplat / PlayCanvas) decodes into the same cloud
        // asset. Registered after the plugin above so `PlanarGaussian3d` is
        // already an initialized asset type.
        app.register_asset_loader(sog::SogLoader);

        // Re-sort a moving camera every 100ms instead of upstream's 1000ms:
        // one full second of stale depth order reads as the cloud dissolving
        // while orbiting the editor camera. A rayon sort of a few hundred
        // thousand splats costs single-digit milliseconds, so 10Hz is cheap;
        // sorting still only happens while the camera actually moves.
        app.insert_resource(bevy_gaussian_splatting::sort::SortConfig { period_ms: 100 });

        app.add_systems(
            Update,
            (reject_oversized_clouds, sync_gaussian_splats, tag_gaussian_cameras),
        );

        #[cfg(feature = "editor")]
        editor::register(app);
    }
}

/// Refuse clouds whose GPU buffers can't exist on this device, BEFORE the
/// renderer tries to upload them. wgpu panics ("Buffer is invalid") when a
/// creation fails validation or allocation, killing the app — a multi-million
/// splat capture on a limit-constrained or VRAM-starved device (VR: stereo
/// eye buffers + compositor already resident) must instead degrade to a clear
/// error. The fattest per-splat plane is the spherical-harmonics buffer
/// (SH_COEFF_COUNT × f32 = 192 bytes at sh3); if that plane alone exceeds the
/// device's buffer/binding limits the whole cloud is unrenderable, so it is
/// replaced with an empty cloud (renders nothing, everything else lives on).
fn reject_oversized_clouds(
    mut clouds: ResMut<Assets<PlanarGaussian3d>>,
    mut events: MessageReader<AssetEvent<PlanarGaussian3d>>,
    device: Option<Res<RenderDevice>>,
) {
    let Some(device) = device else { return };
    let limits = device.limits();
    let max_bytes = limits
        .max_buffer_size
        .min(limits.max_storage_buffer_binding_size);
    const WORST_BYTES_PER_SPLAT: u64 = 192;

    for event in events.read() {
        let AssetEvent::Added { id } = event else {
            continue;
        };
        let Some(cloud) = clouds.get(*id) else {
            continue;
        };
        let count = cloud.len() as u64;
        let need = count * WORST_BYTES_PER_SPLAT;
        if need <= max_bytes {
            continue;
        }
        error!(
            "gaussian cloud rejected: {count} splats need {:.2} GiB per GPU \
             buffer but this device allows {:.2} GiB — decimate the capture \
             (SuperSplat can reduce splat count) or use a smaller scene",
            need as f64 / (1 << 30) as f64,
            max_bytes as f64 / (1 << 30) as f64,
        );
        if let Some(mut cloud) = clouds.get_mut(*id) {
            *cloud = PlanarGaussian3d::from_interleaved(vec![
                bevy_gaussian_splatting::Gaussian3d::default();
                32
            ]);
        }
    }
}

/// Resolve added/changed [`GaussianSplat`] components into the live renderer
/// components, and strip those from entities whose `GaussianSplat` was removed.
///
/// Loading goes through the `AssetServer` with the stored project-relative
/// path (the asset root IS the project directory, same as model/audio
/// rehydration), so a scene loaded in the editor, in play mode, or in a
/// shipped game all resolve identically.
fn sync_gaussian_splats(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    changed: Query<(Entity, &GaussianSplat, Option<&GaussianSplatSynced>), Changed<GaussianSplat>>,
    mut settings: Query<&mut CloudSettings>,
    orphaned: Query<Entity, (With<GaussianSplatSynced>, Without<GaussianSplat>)>,
) {
    for (entity, splat, synced) in changed.iter() {
        if splat.source.is_empty() {
            // Cleared (or not yet assigned) source: drop any previously
            // resolved cloud instead of asking the loader for "".
            if synced.is_some() {
                commands
                    .entity(entity)
                    .remove::<(PlanarGaussian3dHandle, CloudSettings, GaussianSplatSynced)>();
            }
            continue;
        }

        let needs_load = synced.is_none_or(|s| s.source != splat.source);
        if needs_load {
            // Bevy hands back the same handle for an already-loaded path, so a
            // duplicated cloud is a refcount bump, not a second parse.
            let handle: Handle<PlanarGaussian3d> = asset_server.load(splat.source.clone());
            commands.entity(entity).try_insert((
                PlanarGaussian3dHandle(handle),
                GaussianSplatSynced {
                    source: splat.source.clone(),
                },
            ));
        }

        // Mutate settings in place when they exist so a tuning edit doesn't
        // clobber renderer-managed fields; first sync inserts fresh defaults.
        if let Ok(mut cloud_settings) = settings.get_mut(entity) {
            cloud_settings.global_opacity = splat.opacity;
            cloud_settings.global_scale = splat.splat_scale;
        } else {
            commands.entity(entity).try_insert(CloudSettings {
                global_opacity: splat.opacity,
                global_scale: splat.splat_scale,
                // CPU sort, NOT the upstream default Radix: the GPU radix path
                // sorts into chunk 0 of the sorted-entries buffer only (bind
                // group offset 0), while drawing reads the chunk at the view's
                // camera_index — correct only in a single-camera app. The
                // editor always has several gaussian cameras (viewport slots,
                // camera preview, play mode), where radix makes every extra
                // view render with another camera's depth order — splats
                // dissolve into mush as the camera orbits. The rayon sorter
                // fills every per-camera chunk correctly.
                sort_mode: SortMode::Rayon,
                ..Default::default()
            });
        }
    }

    for entity in orphaned.iter() {
        commands
            .entity(entity)
            .remove::<(PlanarGaussian3dHandle, CloudSettings, GaussianSplatSynced)>();
    }
}

/// The upstream renderer only draws clouds through views that carry its
/// [`GaussianCamera`] marker. Scene tooling shouldn't have to know that, so
/// every **active** 3D camera is tagged automatically once the scene actually
/// contains a cloud — editor viewport cameras and game cameras alike.
/// `IsolatedCamera`s (material preview, thumbnail renders, ...) are excluded:
/// they render curated content on private layers and shouldn't pay the
/// per-view sort.
///
/// Inactive cameras are UNtagged, not just skipped: every `GaussianCamera`
/// gets a per-camera chunk in every cloud's sort buffer and a CPU re-sort
/// whenever it moves, so leaving the marker on disabled cameras (the scene's
/// game camera while editing, undocked viewport slots) taxes every sort for
/// views that never draw.
///
/// `warmup: true` mirrors upstream's multi-camera example — the first frame
/// after tagging renders nothing for that view while its sort buffers fill,
/// avoiding a garbage-splat flash.
fn tag_gaussian_cameras(
    mut commands: Commands,
    clouds: Query<(), With<GaussianSplat>>,
    untagged: Query<
        (Entity, &Camera),
        (
            With<Camera3d>,
            Without<GaussianCamera>,
            Without<renzora::IsolatedCamera>,
        ),
    >,
    tagged: Query<(Entity, &Camera), With<GaussianCamera>>,
) {
    if clouds.is_empty() {
        return;
    }
    for (entity, camera) in untagged.iter() {
        if camera.is_active {
            commands
                .entity(entity)
                .try_insert(GaussianCamera { warmup: true });
        }
    }
    for (entity, camera) in tagged.iter() {
        if !camera.is_active {
            // SortTrigger comes off with the marker: the sorter densifies
            // camera indices over SortTrigger holders while sizing chunk
            // buffers over GaussianCamera holders — leaving one behind would
            // let the index space outgrow the chunk space.
            commands
                .entity(entity)
                .remove::<(GaussianCamera, bevy_gaussian_splatting::sort::SortTrigger)>();
        }
    }
}

renzora::add!(GaussianSplatPlugin);
