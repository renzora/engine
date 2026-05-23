//! `.rmip` — Renzora Mipmapped Texture.
//!
//! A small custom container holding decoded RGBA8 pixel data plus a full
//! mipmap chain, baked at import time so runtime never has to decode or
//! downsample. Textures referenced by imported models live as `.rmip`
//! files alongside the model and load directly into `bevy_image::Image`
//! with `mip_level_count > 1`.
//!
//! Why a custom format instead of KTX2: the `ktx2` crate Bevy uses is
//! read-only and the Khronos spec requires a few hundred lines of Data
//! Format Descriptor encoding for a correct writer. We control producer
//! and consumer here, so a purpose-built container is a fraction of the
//! code with the same visual result.
//!
//! ## On-disk layout
//!
//! ```text
//! Offset  Size  Field
//! ------  ----  -----
//! 0       4     Magic "RMIP" (0x52 0x4D 0x49 0x50)
//! 4       4     Version u32 LE — currently 2 (1 still accepted; v1 only
//!               ever used the uncompressed RGBA8 format codes 0/1)
//! 8       4     Width u32 LE — mip 0 width in pixels
//! 12      4     Height u32 LE
//! 16      4     Mip count u32 LE — number of mip levels (≥ 1)
//! 20      4     Format u32 LE — see `RmipFormat`
//! 24      ...   Mip data, mip 0 first then halving (clamped to 1×1).
//!               Each level is `RmipFormat::level_byte_size(w_n, h_n)` bytes:
//!               `w*h*4` for RGBA8, or `blocks_x*blocks_y*bytes_per_block`
//!               for block-compressed formats (mip dims < 4 px round up to a
//!               full 4×4 block, matching wgpu's upload expectation).
//! ```
//!
//! ## Crate layout
//!
//! - `RmipFormat`, `MAGIC`, `VERSION` — the spec.
//! - `RmipAssetLoader` — Bevy `AssetLoader` for runtime/editor.
//! - `bake::*` (feature `bake`) — encoder used by the import pipeline.

mod loader;
pub use loader::{RmipAssetLoader, RmipLoadError};

#[cfg(feature = "bake")]
pub mod bake;

/// GPU pixel format used in a `.rmip` file. Mirrors the `Format` field in
/// the header. Codes 0/1 are the original uncompressed RGBA8 variants;
/// 2+ are GPU block-compressed formats added in version 2, which cut VRAM
/// 4–8× and upload directly without a CPU decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum RmipFormat {
    /// sRGB-encoded color, uncompressed. GPU gamma-decodes on sample.
    Rgba8UnormSrgb = 0,
    /// Linear data, uncompressed (normal/MR/occlusion). No gamma.
    Rgba8Unorm = 1,
    /// BC7 sRGB color — highest-quality RGBA block format, 1 byte/px.
    Bc7RgbaUnormSrgb = 2,
    /// BC7 linear data (multi-channel maps like metallic-roughness), 1 byte/px.
    Bc7RgbaUnorm = 3,
    /// BC5 two-channel (R,G) linear — normal maps. Bevy reconstructs Z. 1 byte/px.
    Bc5RgUnorm = 4,
    /// BC1 sRGB color, opaque (or 1-bit alpha), 0.5 byte/px.
    Bc1RgbaUnormSrgb = 5,
    /// BC1 linear data, 0.5 byte/px.
    Bc1RgbaUnorm = 6,
    /// BC3 sRGB color with full alpha, 1 byte/px.
    Bc3RgbaUnormSrgb = 7,
    /// BC3 linear data with full alpha, 1 byte/px.
    Bc3RgbaUnorm = 8,
    /// BC4 single-channel (R) linear — packed roughness/metallic/AO, 0.5 byte/px.
    Bc4RUnorm = 9,
}

impl RmipFormat {
    /// Decode the header's format code. Returns `None` for unknown codes.
    pub fn from_code(code: u32) -> Option<Self> {
        Some(match code {
            0 => Self::Rgba8UnormSrgb,
            1 => Self::Rgba8Unorm,
            2 => Self::Bc7RgbaUnormSrgb,
            3 => Self::Bc7RgbaUnorm,
            4 => Self::Bc5RgUnorm,
            5 => Self::Bc1RgbaUnormSrgb,
            6 => Self::Bc1RgbaUnorm,
            7 => Self::Bc3RgbaUnormSrgb,
            8 => Self::Bc3RgbaUnorm,
            9 => Self::Bc4RUnorm,
            _ => return None,
        })
    }

    /// The numeric code written to the header.
    pub fn code(self) -> u32 {
        self as u32
    }

    /// `true` for sRGB-encoded color formats (GPU applies gamma decode).
    pub fn is_srgb(self) -> bool {
        matches!(
            self,
            Self::Rgba8UnormSrgb | Self::Bc7RgbaUnormSrgb | Self::Bc1RgbaUnormSrgb | Self::Bc3RgbaUnormSrgb
        )
    }

    /// `true` for the GPU block-compressed formats (4×4 block granularity).
    pub fn is_block_compressed(self) -> bool {
        !matches!(self, Self::Rgba8UnormSrgb | Self::Rgba8Unorm)
    }

    /// Block dimensions in texels. `(1, 1)` for uncompressed, `(4, 4)` for BC.
    pub fn block_dim(self) -> (u32, u32) {
        if self.is_block_compressed() {
            (4, 4)
        } else {
            (1, 1)
        }
    }

    /// Bytes per block (per texel for uncompressed). BC1/BC4 are 8 bytes per
    /// 4×4 block (0.5 byte/px); BC3/BC5/BC7 are 16 (1 byte/px); RGBA8 is 4.
    pub fn bytes_per_block(self) -> usize {
        match self {
            Self::Rgba8UnormSrgb | Self::Rgba8Unorm => 4,
            Self::Bc1RgbaUnormSrgb | Self::Bc1RgbaUnorm | Self::Bc4RUnorm => 8,
            Self::Bc3RgbaUnormSrgb
            | Self::Bc3RgbaUnorm
            | Self::Bc5RgUnorm
            | Self::Bc7RgbaUnormSrgb
            | Self::Bc7RgbaUnorm => 16,
        }
    }

    /// On-disk / on-GPU byte size of a single mip level of logical size
    /// `(w, h)`. Block-compressed levels round each dimension up to a full
    /// block — exactly what wgpu expects when uploading a BC mip chain.
    pub fn level_byte_size(self, w: u32, h: u32) -> usize {
        let (bw, bh) = self.block_dim();
        let blocks_x = w.div_ceil(bw) as usize;
        let blocks_y = h.div_ceil(bh) as usize;
        blocks_x * blocks_y * self.bytes_per_block()
    }

    /// Total payload size across the whole mip chain for a `(width, height)`
    /// base image with `mips` levels (each halving, clamped to 1×1).
    pub fn payload_size(self, width: u32, height: u32, mips: u32) -> usize {
        (0..mips)
            .map(|level| {
                let w = (width >> level).max(1);
                let h = (height >> level).max(1);
                self.level_byte_size(w, h)
            })
            .sum()
    }
}

/// Magic bytes at the start of the file. Just "RMIP" in ASCII.
pub const MAGIC: [u8; 4] = *b"RMIP";

/// Current format version. Bumped to 2 when block-compressed formats were
/// added. The loader still accepts version 1 (uncompressed-only) files.
pub const VERSION: u32 = 2;

/// Header size in bytes. Pixel data starts immediately after.
pub const HEADER_LEN: usize = 24;

/// Number of mip levels for an image of `(width, height)`. Goes down to 1×1.
pub fn mip_count(width: u32, height: u32) -> u32 {
    32 - width.max(height).max(1).leading_zeros()
}
