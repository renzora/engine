//! Bevy `AssetLoader` for `.rmip` files.

use bevy::asset::{io::Reader, AssetLoader, LoadContext, RenderAssetUsages};
use bevy::image::{Image, ImageLoaderSettings, ImageSampler};
use bevy::reflect::TypePath;
use bevy::render::render_resource::{
    Extent3d, TextureDataOrder, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use thiserror::Error;

use crate::{HEADER_LEN, MAGIC, VERSION};

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

        let version = u32_le(&bytes, 4);
        if version != VERSION {
            return Err(RmipLoadError::UnsupportedVersion(version, VERSION));
        }

        let width = u32_le(&bytes, 8);
        let height = u32_le(&bytes, 12);
        let mip_count = u32_le(&bytes, 16);
        let format_code = u32_le(&bytes, 20);

        if width == 0 || height == 0 || mip_count == 0 {
            return Err(RmipLoadError::ZeroSize);
        }

        let format = match format_code {
            0 => TextureFormat::Rgba8UnormSrgb,
            1 => TextureFormat::Rgba8Unorm,
            other => return Err(RmipLoadError::UnknownFormat(other)),
        };

        // Compute expected payload size — sum of every mip level's byte size.
        let mut expected: usize = 0;
        for level in 0..mip_count {
            let w = (width >> level).max(1) as usize;
            let h = (height >> level).max(1) as usize;
            expected += w * h * 4;
        }
        let actual = bytes.len() - HEADER_LEN;
        if actual < expected {
            return Err(RmipLoadError::Truncated { expected, actual });
        }

        let pixels = bytes[HEADER_LEN..HEADER_LEN + expected].to_vec();

        let image = Image {
            data: Some(pixels),
            // The `.rmip` payload lays mips out from largest to smallest,
            // which matches Bevy's default `MipMajor` order.
            data_order: TextureDataOrder::default(),
            texture_descriptor: TextureDescriptor {
                label: None,
                size: Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: mip_count,
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

fn u32_le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}
