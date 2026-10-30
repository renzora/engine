//! The GPU copy of a terrain's weights: one atlas, updated in place.
//!
//! A terrain's chunks' weights are laid side by side in one RGBA8 2D array of
//! two slices (channels 0..3 and 4..7), `chunks_x * resolution` by
//! `chunks_z * resolution` texels, which the landscape shader reads by world
//! position. The image is built once, from every chunk, and after that a brush
//! stroke never touches it as an asset: each frame's changed texels go to the
//! render world as [`WeightPatch`]es and are written into the existing GPU
//! texture with `write_texture`. Re-uploading the whole image would move up to
//! 32 MB a frame for a brush that changed a few thousand bytes, which is the
//! kind of cost that once held overlay painting to two frames a second.
//!
//! The image is `RENDER_WORLD` only, so the main world keeps no second copy of
//! data the chunks already hold, and since the asset is never modified, Bevy
//! never re-prepares it over the patches.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};

use super::weights::{solid, Texel, TerrainChunkWeights, WEIGHT_CHANNELS};

/// Changed texels on their way to the GPU. Filled by the main world, drained
/// into the render world every frame.
#[derive(Resource, Default)]
pub struct TerrainWeightPatches(pub Vec<WeightPatch>);

/// One rectangle of an atlas, both slices.
#[derive(Clone, Debug)]
pub struct WeightPatch {
    pub image: AssetId<Image>,
    /// Texel origin in the atlas.
    pub origin: UVec2,
    pub size: UVec2,
    /// RGBA8 rows for slice 0 (channels 0..3) and slice 1 (channels 4..7).
    pub slices: [Vec<u8>; 2],
    /// Frames this patch has waited for its image to reach the GPU.
    pub age: u32,
}

/// The atlas for a `wide` x `deep` texel terrain, filled from `texel(x, z)`.
pub fn weight_atlas(wide: u32, deep: u32, texel: impl Fn(u32, u32) -> Texel) -> Image {
    let count = (wide * deep) as usize;
    let mut data = vec![0u8; count * WEIGHT_CHANNELS];
    let (first, second) = data.split_at_mut(count * 4);
    for z in 0..deep {
        for x in 0..wide {
            let t = texel(x, z);
            let i = ((z * wide + x) * 4) as usize;
            first[i..i + 4].copy_from_slice(&t[0..4]);
            second[i..i + 4].copy_from_slice(&t[4..8]);
        }
    }
    let mut image = Image::new(
        Extent3d { width: wide.max(1), height: deep.max(1), depth_or_array_layers: 2 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    image
}

/// The patch for texels `rect` (`[min_x, min_z, max_x, max_z]`, inclusive,
/// chunk-local) of the chunk at `(cx, cz)`.
pub fn chunk_patch(image: AssetId<Image>, resolution: u32, cx: u32, cz: u32, weights: &TerrainChunkWeights, rect: [u32; 4]) -> WeightPatch {
    let max = resolution.saturating_sub(1);
    let (x0, z0) = (rect[0].min(max), rect[1].min(max));
    let (x1, z1) = (rect[2].min(max).max(x0), rect[3].min(max).max(z0));
    let size = UVec2::new(x1 - x0 + 1, z1 - z0 + 1);
    let mut slices = [Vec::with_capacity((size.x * size.y * 4) as usize), Vec::with_capacity((size.x * size.y * 4) as usize)];
    for z in z0..=z1 {
        for x in x0..=x1 {
            let t = if weights.texels.is_empty() { solid(0) } else { weights.texel(x, z) };
            slices[0].extend_from_slice(&t[0..4]);
            slices[1].extend_from_slice(&t[4..8]);
        }
    }
    WeightPatch {
        image,
        origin: UVec2::new(cx * resolution + x0, cz * resolution + z0),
        size,
        slices,
        age: 0,
    }
}

/// The render-world half: take the main world's patches and write them into
/// their textures once those exist.
pub(crate) fn install_render_upload(app: &mut App) {
    use bevy::render::{ExtractSchedule, Render, RenderApp, RenderSystems};
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app
        .init_resource::<PendingWeightPatches>()
        .add_systems(ExtractSchedule, extract_weight_patches)
        .add_systems(Render, upload_weight_patches.in_set(RenderSystems::PrepareResources));
}

/// Patches waiting in the render world for their texture.
#[derive(Resource, Default)]
struct PendingWeightPatches(Vec<WeightPatch>);

/// Frames a patch waits for its texture before it is dropped: its atlas was
/// replaced (the terrain was resized), and the new one was built with the
/// patch's texels already in it.
const PATCH_PATIENCE: u32 = 30;

fn extract_weight_patches(mut main_world: ResMut<bevy::render::MainWorld>, mut pending: ResMut<PendingWeightPatches>) {
    if let Some(mut patches) = main_world.get_resource_mut::<TerrainWeightPatches>() {
        if !patches.0.is_empty() {
            pending.0.append(&mut patches.0);
        }
    }
}

fn upload_weight_patches(
    mut pending: ResMut<PendingWeightPatches>,
    images: Res<bevy::render::render_asset::RenderAssets<bevy::render::texture::GpuImage>>,
    queue: Res<bevy::render::renderer::RenderQueue>,
) {
    use bevy::render::render_resource::{Origin3d, TexelCopyBufferLayout, TexelCopyTextureInfo, TextureAspect};
    if pending.0.is_empty() {
        return;
    }
    pending.0.retain_mut(|patch| {
        let Some(gpu) = images.get(patch.image) else {
            patch.age += 1;
            return patch.age < PATCH_PATIENCE;
        };
        for (layer, bytes) in patch.slices.iter().enumerate() {
            queue.write_texture(
                TexelCopyTextureInfo {
                    texture: &gpu.texture,
                    mip_level: 0,
                    origin: Origin3d { x: patch.origin.x, y: patch.origin.y, z: layer as u32 },
                    aspect: TextureAspect::All,
                },
                bytes,
                TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(patch.size.x * 4),
                    rows_per_image: Some(patch.size.y),
                },
                Extent3d { width: patch.size.x, height: patch.size.y, depth_or_array_layers: 1 },
            );
        }
        false
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_atlas_splits_channels_into_two_slices() {
        let image = weight_atlas(2, 1, |x, _| if x == 0 { solid(1) } else { solid(6) });
        let data = image.data.as_ref().unwrap();
        assert_eq!(data.len(), 2 * 8);
        // Slice 0: texel 0 has channel 1, texel 1 nothing in 0..3.
        assert_eq!(&data[0..8], &[0, 255, 0, 0, 0, 0, 0, 0]);
        // Slice 1: texel 1 has channel 6 (the third of 4..7).
        assert_eq!(&data[8..16], &[0, 0, 0, 0, 0, 0, 255, 0]);
        assert_eq!(image.texture_descriptor.size.depth_or_array_layers, 2);
    }

    #[test]
    fn a_patch_lands_at_its_chunks_place_in_the_atlas() {
        let mut w = TerrainChunkWeights::default();
        w.set_texels(4, vec![solid(5); 16]);
        let p = chunk_patch(AssetId::default(), 4, 2, 1, &w, [1, 1, 2, 3]);
        assert_eq!(p.origin, UVec2::new(9, 5));
        assert_eq!(p.size, UVec2::new(2, 3));
        assert_eq!(p.slices[1].len(), 2 * 3 * 4);
        assert_eq!(&p.slices[1][0..4], &[0, 255, 0, 0]);
    }
}
