//! Bevy `AssetLoader` for `.rmip` files.

use bevy::asset::{io::Reader, AssetLoader, LoadContext, RenderAssetUsages};
use bevy::image::{Image, ImageLoaderSettings, ImageSampler};
use bevy::reflect::TypePath;
use bevy::render::render_resource::{
    Extent3d, TextureDataOrder, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use thiserror::Error;

use crate::{RmipFormat, HEADER_LEN, MAGIC, VERSION};

/// AssetLoader implementation for `.rmip` files. Registered via
/// `app.init_asset_loader::<RmipAssetLoader>()`. Bevy uploads
/// `Image::data` to wgpu honoring the descriptor's `mip_level_count`,
/// laying the levels out one after another in memory — exactly the on-
/// disk layout — so we don't have to do anything special at upload time.
#[derive(Default, TypePath)]
pub struct RmipAssetLoader;

#[derive(Debug, Error)]
pub enum RmipLoadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("file too small for header (got {0} bytes, need {1})")]
    TooSmall(usize, usize),
    #[error("bad magic; expected RMIP")]
    BadMagic,
    #[error("unsupported version {0}; this build expects {1}")]
    UnsupportedVersion(u32, u32),
    #[error("zero-sized image")]
    ZeroSize,
    #[error("unknown pixel format code {0}")]
    UnknownFormat(u32),
    #[error("mip data truncated: expected {expected} bytes, got {actual}")]
    Truncated { expected: usize, actual: usize },
}

impl AssetLoader for RmipAssetLoader {
    type Asset = Image;
    // We use `ImageLoaderSettings` rather than `()` so that Bevy's GLB
    // loader — which calls `load_context.load::<Image, ImageLoaderSettings>(...)`
    // for every embedded texture URI — can route through us without tripping
    // a settings-type-mismatch error. Old projects (pre-0921dc8) baked `.rmip`
    // URIs directly into the GLB JSON; with this loader's settings type
    // matching, the load goes through cleanly. Settings are otherwise advisory
    // — the format (sRGB vs linear) is baked into the `.rmip` header at import
    // time, so we ignore `is_srgb`/`format`/etc here.
    type Settings = ImageLoaderSettings;
    type Error = RmipLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &ImageLoaderSettings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        if bytes.len() < HEADER_LEN {
            return Err(RmipLoadError::TooSmall(bytes.len(), HEADER_LEN));
        }
        if bytes[0..4] != MAGIC {
            return Err(RmipLoadError::BadMagic);
        }

        // v1 was uncompressed-only; v2 added block-compressed formats. Both
        // share the same header + payload layout, so accept either.
        let version = u32_le(&bytes, 4);
        if version != 1 && version != VERSION {
            return Err(RmipLoadError::UnsupportedVersion(version, VERSION));
        }

        let width = u32_le(&bytes, 8);
        let height = u32_le(&bytes, 12);
        let mip_count = u32_le(&bytes, 16);
        let format_code = u32_le(&bytes, 20);

        if width == 0 || height == 0 || mip_count == 0 {
            return Err(RmipLoadError::ZeroSize);
        }

        let rmip_format =
            RmipFormat::from_code(format_code).ok_or(RmipLoadError::UnknownFormat(format_code))?;
        let format = wgpu_format(rmip_format);

        // Expected payload = sum of every mip level's byte size, computed with
        // block granularity for the compressed formats.
        let expected = rmip_format.payload_size(width, height, mip_count);
        let actual = bytes.len() - HEADER_LEN;
        if actual < expected {
            return Err(RmipLoadError::Truncated { expected, actual });
        }

        // Block-compressed textures must be uploaded with block-aligned base
        // dimensions or wgpu rejects the texture ("Width N is not a multiple of
        // <fmt>'s block width"). `aligned_upload` rounds the descriptor up to
        // the block size and caps the mip chain at the levels that still line up
        // with the stored data (see its doc comment). For files baked after the
        // bake-side alignment fix — and all uncompressed files — it's a no-op.
        let (aligned_w, aligned_h, usable_mips, used_bytes) =
            aligned_upload(rmip_format, width, height, mip_count);

        let pixels = bytes[HEADER_LEN..HEADER_LEN + used_bytes].to_vec();

        let image = Image {
            data: Some(pixels),
            // The `.rmip` payload lays mips out from largest to smallest,
            // which matches Bevy's default `MipMajor` order.
            data_order: TextureDataOrder::default(),
            texture_descriptor: TextureDescriptor {
                label: None,
                size: Extent3d {
                    width: aligned_w,
                    height: aligned_h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: usable_mips,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            },
            // `ImageSampler::Default` lets the global ImagePlugin sampler
            // (linear + anisotropic, configured at app build) apply on
            // first sample. Mipmaps + anisotropic together are what give
            // the "Godot-quality" look on oblique surfaces.
            sampler: ImageSampler::Default,
            texture_view_descriptor: None,
            asset_usage: RenderAssetUsages::default(),
            copy_on_resize: false,
        };

        Ok(image)
    }

    fn extensions(&self) -> &[&str] {
        &["rmip"]
    }
}

/// Map a `.rmip` storage format to its wgpu `TextureFormat`. The BC formats
/// require the adapter's `TEXTURE_COMPRESSION_BC` feature, which Bevy enables
/// by default on any desktop GPU that supports it.
fn wgpu_format(format: RmipFormat) -> TextureFormat {
    match format {
        RmipFormat::Rgba8UnormSrgb => TextureFormat::Rgba8UnormSrgb,
        RmipFormat::Rgba8Unorm => TextureFormat::Rgba8Unorm,
        RmipFormat::Bc7RgbaUnormSrgb => TextureFormat::Bc7RgbaUnormSrgb,
        RmipFormat::Bc7RgbaUnorm => TextureFormat::Bc7RgbaUnorm,
        RmipFormat::Bc5RgUnorm => TextureFormat::Bc5RgUnorm,
        RmipFormat::Bc1RgbaUnormSrgb => TextureFormat::Bc1RgbaUnormSrgb,
        RmipFormat::Bc1RgbaUnorm => TextureFormat::Bc1RgbaUnorm,
        RmipFormat::Bc3RgbaUnormSrgb => TextureFormat::Bc3RgbaUnormSrgb,
        RmipFormat::Bc3RgbaUnorm => TextureFormat::Bc3RgbaUnorm,
        RmipFormat::Bc4RUnorm => TextureFormat::Bc4RUnorm,
    }
}

fn u32_le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

/// Decide how to upload a stored `.rmip` of logical `width × height` with
/// `mip_count` levels in `format`, given that wgpu requires block-compressed
/// textures to have block-aligned base dimensions.
///
/// Returns `(aligned_width, aligned_height, usable_mips, used_bytes)`:
/// - the base dimensions rounded up to the format's block size (a no-op for
///   uncompressed formats and for already-aligned bakes);
/// - how many leading mip levels still line up between the stored layout
///   (`logical >> level`) and the layout wgpu derives from the aligned base
///   (`aligned >> level`) — they can diverge deep in the chain for legacy
///   non-aligned files, and those trailing levels must be dropped;
/// - the payload byte count for exactly those usable levels.
///
/// Level 0 always matches by construction, so `usable_mips >= 1`.
pub(crate) fn aligned_upload(
    format: RmipFormat,
    width: u32,
    height: u32,
    mip_count: u32,
) -> (u32, u32, u32, usize) {
    let (bw, bh) = format.block_dim();
    let aligned_w = width.div_ceil(bw) * bw;
    let aligned_h = height.div_ceil(bh) * bh;
    let mut usable_mips = 0u32;
    let mut used_bytes = 0usize;
    for level in 0..mip_count {
        let lw = (width >> level).max(1);
        let lh = (height >> level).max(1);
        let stored = (lw.div_ceil(bw), lh.div_ceil(bh));
        let aligned = (
            (aligned_w >> level).max(1).div_ceil(bw),
            (aligned_h >> level).max(1).div_ceil(bh),
        );
        if stored != aligned {
            break;
        }
        usable_mips += 1;
        used_bytes += format.level_byte_size(lw, lh);
    }
    (aligned_w, aligned_h, usable_mips, used_bytes)
}

#[cfg(test)]
mod tests {
    use super::aligned_upload;
    use crate::{mip_count, RmipFormat};

    #[test]
    fn aligned_bc_file_is_unchanged() {
        // 256×256 BC5 is already block-aligned: full mip chain, full payload.
        let mips = mip_count(256, 256);
        let (aw, ah, usable, bytes) = aligned_upload(RmipFormat::Bc5RgUnorm, 256, 256, mips);
        assert_eq!((aw, ah), (256, 256));
        assert_eq!(usable, mips);
        assert_eq!(bytes, RmipFormat::Bc5RgUnorm.payload_size(256, 256, mips));
    }

    #[test]
    fn uncompressed_is_never_aligned_or_capped() {
        // Uncompressed has 1×1 blocks: any size is valid, nothing is dropped.
        let mips = mip_count(517, 300);
        let (aw, ah, usable, bytes) = aligned_upload(RmipFormat::Rgba8Unorm, 517, 300, mips);
        assert_eq!((aw, ah), (517, 300));
        assert_eq!(usable, mips);
        assert_eq!(bytes, RmipFormat::Rgba8Unorm.payload_size(517, 300, mips));
    }

    #[test]
    fn legacy_non_aligned_bc_rounds_up_and_caps_chain() {
        // The crash case: a 517-wide BC5 normal map. The base rounds 517→520;
        // the chain diverges at the first level where 520>>l and 517>>l round to
        // different block counts (level 3: 17 vs 16 blocks wide), so the deepest
        // mips are dropped — but the texture now uploads instead of crashing.
        let mips = mip_count(517, 300); // height 300 is already block-aligned
        let (aw, ah, usable, bytes) = aligned_upload(RmipFormat::Bc5RgUnorm, 517, 300, mips);
        assert_eq!((aw, ah), (520, 300));
        assert_eq!(aw % 4, 0);
        assert_eq!(ah % 4, 0);
        assert_eq!(usable, 3);
        // used_bytes covers exactly the usable leading levels, by logical dims.
        let expect: usize = (0..usable)
            .map(|l| RmipFormat::Bc5RgUnorm.level_byte_size((517 >> l).max(1), (300 >> l).max(1)))
            .sum();
        assert_eq!(bytes, expect);
    }

    #[test]
    fn usable_mips_at_least_one_for_tiny_odd_bc() {
        // A 1×1 BC texture: rounds up to 4×4, single usable level, never panics.
        let (aw, ah, usable, bytes) = aligned_upload(RmipFormat::Bc7RgbaUnorm, 1, 1, 1);
        assert_eq!((aw, ah), (4, 4));
        assert_eq!(usable, 1);
        assert_eq!(bytes, RmipFormat::Bc7RgbaUnorm.level_byte_size(1, 1));
    }
}
