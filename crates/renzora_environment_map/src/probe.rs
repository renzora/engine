//! Reflection-probe source handling: turn a user-assigned environment image
//! into the **power-of-two cube** texture that bevy's `GeneratedEnvironmentMapLight`
//! filter requires.
//!
//! Bevy's probe filter (`compute_mip_count`) asserts the source cubemap is a
//! power of two and reads its width to build the radiance mip chain. A `.ktx2`
//! cube container already satisfies that and is used as-is; an equirectangular
//! `.exr`/`.hdr`/`.png` is a flat 2D image and must be reprojected into a 6-face
//! POT cube (and into a *filterable* `Rgba16Float` format) first — otherwise GPU
//! validation rejects it.
//!
//! The authored value is the project-relative path
//! ([`renzora::core::ReflectionProbeSource`]); it persists in the scene and is
//! re-loaded + re-converted on load. The generated cube handle itself is
//! runtime-only.

use bevy::asset::{LoadState, RenderAssetUsages};
use bevy::image::Image;
use bevy::light::GeneratedEnvironmentMapLight;
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
};

use renzora::core::ReflectionProbeSource;

/// Per-probe bookkeeping so we only (re)load + convert when the authored path
/// actually changes — not every frame.
#[derive(Component, Default)]
pub struct ProbeSourceState {
    /// The path we last started loading.
    requested: String,
    /// Handle of the raw source image (equirect or cube) currently loading.
    pending: Option<Handle<Image>>,
}

/// Drive each probe's [`ReflectionProbeSource`] onto a `GeneratedEnvironmentMapLight`,
/// reprojecting equirect images and — crucially — only **attaching** the
/// `GeneratedEnvironmentMapLight` once a valid power-of-two cube exists.
///
/// Bevy's environment-map filter runs for any entity that *has* a
/// `GeneratedEnvironmentMapLight` whose handle resolves, including the 1×1
/// default placeholder, which fails GPU validation (mip math on a 1-pixel
/// "cube"). So an unconfigured probe must carry no `GeneratedEnvironmentMapLight`
/// at all; we add it only when the cube is ready and remove it when the source
/// is cleared.
pub fn apply_reflection_probe_source(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut q: Query<(
        Entity,
        &ReflectionProbeSource,
        Option<&mut GeneratedEnvironmentMapLight>,
        Option<&mut ProbeSourceState>,
    )>,
) {
    for (entity, source, gen_light, state) in &mut q {
        let path = source.path.trim().to_string();

        // First sighting — attach bookkeeping, pick it up next frame (keeps the
        // borrow checker happy vs. `commands`).
        let Some(mut st) = state else {
            commands.entity(entity).insert(ProbeSourceState::default());
            continue;
        };

        let has_light = gen_light.is_some();
        // Keep an already-attached light's intensity in sync with the inspector.
        if let Some(mut g) = gen_light {
            if g.intensity != source.intensity {
                g.intensity = source.intensity;
            }
        }

        if path.is_empty() {
            // No source → there must be no `GeneratedEnvironmentMapLight` (its
            // filter would run on the 1×1 default and fail). Remove it whenever
            // present — this also cleans up probes spawned before this was split
            // out (which shipped the light with an unset handle).
            if has_light {
                commands.entity(entity).remove::<GeneratedEnvironmentMapLight>();
            }
            st.requested.clear();
            st.pending = None;
            continue;
        }

        // Path changed → start a fresh load.
        if st.requested != path {
            st.requested = path.clone();
            st.pending = Some(asset_server.load::<Image>(path.clone()));
            continue;
        }

        // Already resolved (pending cleared) → nothing to do.
        let Some(pending) = st.pending.clone() else {
            continue;
        };

        match asset_server.load_state(pending.id()) {
            LoadState::Loaded => {
                let is_cube = images
                    .get(&pending)
                    .map(|i| i.texture_descriptor.size.depth_or_array_layers == 6)
                    .unwrap_or(false);
                let cube_handle = if is_cube {
                    // A real cube container (.ktx2/.dds) — feed it straight in.
                    Some(pending.clone())
                } else if let Some(cube) = images.get(&pending).and_then(equirect_to_pot_cube) {
                    Some(images.add(cube))
                } else {
                    warn!("reflection probe: couldn't convert '{path}' to a cubemap");
                    None
                };
                if let Some(cube_handle) = cube_handle {
                    // Attach (or replace) the GPU light now that the cube is valid.
                    commands.entity(entity).insert(GeneratedEnvironmentMapLight {
                        environment_map: cube_handle,
                        intensity: source.intensity,
                        ..default()
                    });
                }
                st.pending = None;
            }
            LoadState::Failed(err) => {
                warn!("reflection probe: failed to load '{path}': {err}");
                st.pending = None;
            }
            _ => {}
        }
    }
}

/// Largest power of two `<= n` (clamped so a probe cube stays a sane,
/// filter-friendly size regardless of the source HDRI resolution).
fn prev_pot(n: u32) -> u32 {
    if n < 2 {
        1
    } else {
        1u32 << (31 - n.leading_zeros())
    }
}

/// Reproject an equirectangular 2D image into a 6-face `Rgba16Float` cube whose
/// faces are a power of two. Mirrors the engine's skybox reprojection but pins
/// the output to a POT, GPU-filterable format (the two things bevy's probe
/// filter requires). Returns `None` if the source has no readable pixels.
fn equirect_to_pot_cube(src: &Image) -> Option<Image> {
    use std::f32::consts::PI;

    let (sw, sh) = (src.width(), src.height());
    if sw == 0 || sh == 0 {
        return None;
    }
    // Probe faces are derived from the equirect height; 128–1024 keeps the
    // one-time CPU reprojection cheap while leaving enough detail for glossy
    // reflections.
    let face = prev_pot(sh / 2).clamp(128, 1024);

    // One black `Rgba16Float` texel (8 bytes) tiled across all 6 faces; we then
    // overwrite each texel below. `set_color_at_3d` handles the f32→f16 encode.
    let mut cube = Image::new_fill(
        Extent3d {
            width: face,
            height: face,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0, 0, 0, 0, 0],
        TextureFormat::Rgba16Float,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );

    // +X, -X, +Y, -Y, +Z, -Z face bases (forward, up, right).
    let faces: [(Vec3, Vec3, Vec3); 6] = [
        (Vec3::X, Vec3::Y, Vec3::NEG_Z),
        (Vec3::NEG_X, Vec3::Y, Vec3::Z),
        (Vec3::Y, Vec3::NEG_Z, Vec3::X),
        (Vec3::NEG_Y, Vec3::Z, Vec3::X),
        (Vec3::Z, Vec3::Y, Vec3::X),
        (Vec3::NEG_Z, Vec3::Y, Vec3::NEG_X),
    ];

    let mut wrote_any = false;
    for (layer, (forward, up, right)) in faces.iter().enumerate() {
        for y in 0..face {
            for x in 0..face {
                let u = (x as f32 + 0.5) / face as f32 * 2.0 - 1.0;
                let v = (y as f32 + 0.5) / face as f32 * 2.0 - 1.0;
                let dir = (*forward + *right * u - *up * v).normalize();

                let theta = dir.z.atan2(dir.x);
                let phi = dir.y.asin();
                let eq_u = (theta + PI) / (2.0 * PI);
                let eq_v = (phi + PI / 2.0) / PI;

                let sx = ((eq_u * sw as f32) as u32).min(sw - 1);
                let sy = (((1.0 - eq_v) * sh as f32) as u32).min(sh - 1);

                if let Ok(color) = src.get_color_at(sx, sy) {
                    if cube.set_color_at_3d(x, y, layer as u32, color).is_ok() {
                        wrote_any = true;
                    }
                }
            }
        }
    }

    if !wrote_any {
        // Source had no CPU-readable data (e.g. stripped to GPU-only) — bail
        // rather than ship a black probe.
        return None;
    }

    cube.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    Some(cube)
}
