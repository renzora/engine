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
//! 4       4     Version u32 LE — currently 1
//! 8       4     Width u32 LE — mip 0 width in pixels
//! 12      4     Height u32 LE
//! 16      4     Mip count u32 LE — number of mip levels (≥ 1)
//! 20      4     Format u32 LE: 0 = Rgba8UnormSrgb, 1 = Rgba8Unorm
//! 24      ...   Mip data, mip 0 first then halving (clamped to 1×1).
//!               Each level is `width_n * height_n * 4` raw bytes.
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

/// Pixel format used in a `.rmip` file. Mirrors the `Format` field in the
/// header. Only RGBA8 variants for now — covers every PBR texture we
/// extract today.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RmipFormat {
    /// sRGB-encoded color textures (base color, emissive). GPU does the
    /// gamma decode on sample.
    Rgba8UnormSrgb = 0,
    /// Linear data textures (normal maps, metallic-roughness, occlusion).
    /// No gamma applied.
    Rgba8Unorm = 1,
}

/// Magic bytes at the start of the file. Just "RMIP" in ASCII.
pub const MAGIC: [u8; 4] = *b"RMIP";

/// Format version. Bump when the layout changes; the loader checks this
/// to refuse old files rather than silently misinterpreting.
pub const VERSION: u32 = 1;

/// Header size in bytes. Pixel data starts immediately after.
pub const HEADER_LEN: usize = 24;

/// Number of mip levels for an image of `(width, height)`. Goes down to 1×1.
pub fn mip_count(width: u32, height: u32) -> u32 {
    32 - width.max(height).max(1).leading_zeros()
}
