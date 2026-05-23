//! Encoder for `.rmip` files.
//!
//! Decodes arbitrary image bytes (PNG/JPG/WebP/etc), optionally clamps the
//! resolution, generates a full mipmap chain via Lanczos3 downsampling, and
//! GPU-block-compresses each level (BC1/BC3/BC5/BC7) before writing the
//! binary container described in the crate root. Used by `renzora_import` at
//! import time. Gated behind the `bake` feature so consumers that only load
//! `.rmip` (the editor + runtime) don't pull in `image`/`intel_tex_2`.
//!
//! The format-selection logic mirrors Godot's texture importer: sRGB color →
//! BC7 (or BC1/BC3 in the non-"high quality" path), tangent-space normals →
//! BC5 with renormalized mips, linear multi-channel data → BC7/BC1.

use image::{imageops::FilterType, GenericImageView, ImageBuffer, Rgba};
use intel_tex_2 as intel_tex;

use crate::{mip_count, RmipFormat, HEADER_LEN, MAGIC, VERSION};

/// Semantic role of a texture. Drives the sRGB-vs-linear choice and which
/// GPU block format the data is compressed to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureRole {
    /// sRGB color (base color, emissive). Keeps full RGBA.
    Color,
    /// Tangent-space normal map. Linear; stored as BC5 (R,G) with the mip
    /// chain renormalized. Bevy reconstructs Z from the two channels.
    NormalMap,
    /// Linear multi-channel data (metallic-roughness, occlusion,
    /// spec-glossiness). Keeps full RGBA, no gamma.
    LinearData,
}

impl TextureRole {
    fn is_srgb(self) -> bool {
        matches!(self, TextureRole::Color)
    }
}

/// Controls how an image is baked into a `.rmip`.
#[derive(Clone, Copy, Debug)]
pub struct BakeParams {
    /// Semantic role — see [`TextureRole`].
    pub role: TextureRole,
    /// Use GPU block compression. When `false`, stores uncompressed RGBA8
    /// (still mipmapped) — useful as an escape hatch or for tiny textures.
    pub compress: bool,
    /// Prefer BC7 (1 byte/px, best quality) over BC1/BC3 (0.5–1 byte/px,
    /// cheaper, visible block artifacts on gradients). Ignored for normal
    /// maps, which always use BC5.
    pub high_quality: bool,
    /// Clamp the longest side to at most this many texels before baking
    /// (`0` disables clamping). Downsampling at import is the biggest single
    /// win for over-authored 4K texture sets.
    pub max_size: u32,
}

impl Default for BakeParams {
    fn default() -> Self {
        Self {
            role: TextureRole::Color,
            compress: true,
            high_quality: true,
            max_size: 2048,
        }
    }
}

impl BakeParams {
    /// Pick the on-disk GPU format for this role given the source's alpha and
    /// the compression settings. Mirrors Godot's importer decision tree.
    fn select_format(&self, has_alpha: bool) -> RmipFormat {
        if !self.compress {
            return if self.role.is_srgb() {
                RmipFormat::Rgba8UnormSrgb
            } else {
                RmipFormat::Rgba8Unorm
            };
        }
        match self.role {
            TextureRole::NormalMap => RmipFormat::Bc5RgUnorm,
            TextureRole::Color => {
                if self.high_quality {
                    RmipFormat::Bc7RgbaUnormSrgb
                } else if has_alpha {
                    RmipFormat::Bc3RgbaUnormSrgb
                } else {
                    RmipFormat::Bc1RgbaUnormSrgb
                }
            }
            TextureRole::LinearData => {
                if self.high_quality {
                    RmipFormat::Bc7RgbaUnorm
                } else if has_alpha {
                    RmipFormat::Bc3RgbaUnorm
                } else {
                    RmipFormat::Bc1RgbaUnorm
                }
            }
        }
    }
}

/// Decode arbitrary image bytes (PNG/JPG/etc) and bake them into a `.rmip`
/// byte vector according to `params`.
pub fn bake_image(bytes: &[u8], params: BakeParams) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("decode image: {e}"))?;
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();
    bake_rgba8(rgba.as_raw(), w, h, params)
}

/// Bake a `.rmip` from a pre-decoded RGBA8 buffer. `pixels.len()` must be
/// `width * height * 4`.
pub fn bake_rgba8(
    pixels: &[u8],
    width: u32,
    height: u32,
    params: BakeParams,
) -> Result<Vec<u8>, String> {
    if width == 0 || height == 0 {
        return Err("zero-sized image".into());
    }
    let expected = (width as usize) * (height as usize) * 4;
    if pixels.len() != expected {
        return Err(format!(
            "pixel buffer size {} doesn't match {width}x{height}*4 = {expected}",
            pixels.len(),
        ));
    }

    let mut base: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, pixels.to_vec())
            .ok_or_else(|| "ImageBuffer::from_raw failed".to_string())?;

    // ── Resolution clamp ────────────────────────────────────────────────
    // Downsample the longest side to `max_size`, preserving aspect ratio.
    if params.max_size > 0 && width.max(height) > params.max_size {
        let scale = params.max_size as f32 / width.max(height) as f32;
        let new_w = ((width as f32 * scale).round() as u32).max(1);
        let new_h = ((height as f32 * scale).round() as u32).max(1);
        base = image::imageops::resize(&base, new_w, new_h, FilterType::Lanczos3);
    }
    let width = base.width();
    let height = base.height();

    let renormalize = params.role == TextureRole::NormalMap;
    let has_alpha = !renormalize && base.pixels().any(|p| p.0[3] != 255);
    let format = params.select_format(has_alpha);

    // ── Mip chain (Lanczos3, renormalized for normal maps) ──────────────
    let mips = mip_count(width, height);
    let mut levels: Vec<ImageBuffer<Rgba<u8>, Vec<u8>>> = Vec::with_capacity(mips as usize);
    levels.push(base.clone());
    let mut current = base;
    for _ in 1..mips {
        let new_w = (current.width() / 2).max(1);
        let new_h = (current.height() / 2).max(1);
        let mut resized = image::imageops::resize(&current, new_w, new_h, FilterType::Lanczos3);
        if renormalize {
            renormalize_normals(&mut resized);
        }
        levels.push(resized.clone());
        current = resized;
    }

    // ── Encode each level to the chosen GPU format ──────────────────────
    let mut payload =
        Vec::with_capacity(format.payload_size(width, height, mips));
    for level in &levels {
        encode_level(format, level, &mut payload);
    }

    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes());
    out.extend_from_slice(&mips.to_le_bytes());
    out.extend_from_slice(&format.code().to_le_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

/// Renormalize a normal map level in place: treat each texel's RGB as a
/// tangent-space vector, normalize it, and re-encode. Box/Lanczos averaging
/// shortens normals near detail; renormalizing keeps lighting stable across
/// the mip chain (matches Godot's `generate_mipmaps(renormalize=true)`).
fn renormalize_normals(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>) {
    for px in img.pixels_mut() {
        let x = px.0[0] as f32 / 127.5 - 1.0;
        let y = px.0[1] as f32 / 127.5 - 1.0;
        let z = px.0[2] as f32 / 127.5 - 1.0;
        let len = (x * x + y * y + z * z).sqrt();
        if len > 1e-6 {
            let inv = 1.0 / len;
            px.0[0] = (((x * inv) * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8;
            px.0[1] = (((y * inv) * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8;
            px.0[2] = (((z * inv) * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// Compress one mip level into `out`. For block formats the level is first
/// padded (edge-replicated) to a multiple of 4 in each dimension, then
/// compressed; the resulting block count matches
/// `RmipFormat::level_byte_size`, which is what wgpu expects on upload.
fn encode_level(format: RmipFormat, img: &ImageBuffer<Rgba<u8>, Vec<u8>>, out: &mut Vec<u8>) {
    let (w, h) = (img.width(), img.height());

    if !format.is_block_compressed() {
        out.extend_from_slice(img.as_raw());
        return;
    }

    // Pad to a multiple of 4 by clamping to the edge texel.
    let pw = w.div_ceil(4) * 4;
    let ph = h.div_ceil(4) * 4;
    let padded = pad_rgba_clamp(img, pw, ph);
    let surface = intel_tex::RgbaSurface {
        data: &padded,
        width: pw,
        height: ph,
        stride: pw * 4,
    };

    match format {
        RmipFormat::Bc7RgbaUnormSrgb | RmipFormat::Bc7RgbaUnorm => {
            // Alpha-aware settings only matter for textures that actually use
            // alpha; opaque settings are faster and avoid wasting a mode bit.
            let has_alpha = img.pixels().any(|p| p.0[3] != 255);
            let settings = if has_alpha {
                intel_tex::bc7::alpha_basic_settings()
            } else {
                intel_tex::bc7::opaque_basic_settings()
            };
            out.extend_from_slice(&intel_tex::bc7::compress_blocks(&settings, &surface));
        }
        RmipFormat::Bc1RgbaUnormSrgb | RmipFormat::Bc1RgbaUnorm => {
            out.extend_from_slice(&intel_tex::bc1::compress_blocks(&surface));
        }
        RmipFormat::Bc3RgbaUnormSrgb | RmipFormat::Bc3RgbaUnorm => {
            out.extend_from_slice(&intel_tex::bc3::compress_blocks(&surface));
        }
        RmipFormat::Bc5RgUnorm => {
            // BC5 takes a tightly-packed 2-channel (R,G) surface.
            let rg = extract_channels::<2>(&padded);
            let rg_surface = intel_tex::RgSurface {
                data: &rg,
                width: pw,
                height: ph,
                stride: pw * 2,
            };
            out.extend_from_slice(&intel_tex::bc5::compress_blocks(&rg_surface));
        }
        RmipFormat::Bc4RUnorm => {
            let r = extract_channels::<1>(&padded);
            let r_surface = intel_tex::RSurface {
                data: &r,
                width: pw,
                height: ph,
                stride: pw,
            };
            out.extend_from_slice(&intel_tex::bc4::compress_blocks(&r_surface));
        }
        RmipFormat::Rgba8UnormSrgb | RmipFormat::Rgba8Unorm => unreachable!("handled above"),
    }
}

/// Build a `pw × ph` RGBA8 buffer from `img`, replicating the right/bottom
/// edge texels into the padding region.
fn pad_rgba_clamp(img: &ImageBuffer<Rgba<u8>, Vec<u8>>, pw: u32, ph: u32) -> Vec<u8> {
    let (w, h) = (img.width(), img.height());
    if w == pw && h == ph {
        return img.as_raw().clone();
    }
    let src = img.as_raw();
    let mut out = vec![0u8; (pw * ph * 4) as usize];
    for y in 0..ph {
        let sy = y.min(h - 1);
        for x in 0..pw {
            let sx = x.min(w - 1);
            let s = ((sy * w + sx) * 4) as usize;
            let d = ((y * pw + x) * 4) as usize;
            out[d..d + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
    out
}

/// Take the first `N` channels of an RGBA8 buffer into a tightly-packed
/// `N`-channel buffer (for BC4's R / BC5's RG surfaces).
fn extract_channels<const N: usize>(rgba: &[u8]) -> Vec<u8> {
    let texels = rgba.len() / 4;
    let mut out = Vec::with_capacity(texels * N);
    for t in 0..texels {
        out.extend_from_slice(&rgba[t * 4..t * 4 + N]);
    }
    out
}

// ── Backwards-compatible thin wrappers ──────────────────────────────────
// Older call sites passed an `RmipFormat` purely to signal sRGB vs linear.
// These map that onto the new role-based API with compression enabled.

/// Deprecated shim: bake with default params, inferring the role from a
/// storage format flag (`Rgba8Unorm` → linear data, else color).
pub fn bake_from_image_bytes(bytes: &[u8], format: RmipFormat) -> Result<Vec<u8>, String> {
    let role = if format.is_srgb() {
        TextureRole::Color
    } else {
        TextureRole::LinearData
    };
    bake_image(
        bytes,
        BakeParams {
            role,
            ..Default::default()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Solid-color RGBA8 buffer of `w × h`, with optional alpha.
    fn solid(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        rgba.iter().copied().cycle().take((w * h * 4) as usize).collect()
    }

    /// Parse the `.rmip` header into (version, width, height, mips, format).
    fn header(bytes: &[u8]) -> (u32, u32, u32, u32, RmipFormat) {
        assert_eq!(&bytes[0..4], &MAGIC, "magic");
        let r = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
        (
            r(4),
            r(8),
            r(12),
            r(16),
            RmipFormat::from_code(r(20)).expect("known format"),
        )
    }

    /// The payload length must equal the format's computed mip-chain size.
    fn assert_payload_matches(bytes: &[u8]) {
        let (_, w, h, mips, fmt) = header(bytes);
        assert_eq!(
            bytes.len() - HEADER_LEN,
            fmt.payload_size(w, h, mips),
            "payload size mismatch for {fmt:?} {w}x{h} mips={mips}",
        );
    }

    #[test]
    fn format_block_math() {
        // BC7: 1 byte/px → 16 bytes per 4×4 block.
        assert_eq!(RmipFormat::Bc7RgbaUnorm.level_byte_size(8, 8), 64);
        // Mip levels below 4px round up to a single full block.
        assert_eq!(RmipFormat::Bc7RgbaUnorm.level_byte_size(2, 2), 16);
        assert_eq!(RmipFormat::Bc7RgbaUnorm.level_byte_size(1, 1), 16);
        // 8×8 BC7 chain = 64 + 16 + 16 + 16.
        assert_eq!(RmipFormat::Bc7RgbaUnorm.payload_size(8, 8, 4), 112);
        // BC1/BC4 are half the bytes per block.
        assert_eq!(RmipFormat::Bc1RgbaUnorm.level_byte_size(8, 8), 32);
        // Uncompressed is plain w*h*4.
        assert_eq!(RmipFormat::Rgba8Unorm.level_byte_size(8, 8), 256);
    }

    #[test]
    fn color_high_quality_is_bc7_srgb() {
        let px = solid(8, 8, [200, 100, 50, 255]);
        let out = bake_rgba8(&px, 8, 8, BakeParams::default()).unwrap();
        let (ver, w, h, mips, fmt) = header(&out);
        assert_eq!(ver, VERSION);
        assert_eq!((w, h), (8, 8));
        assert_eq!(mips, 4);
        assert_eq!(fmt, RmipFormat::Bc7RgbaUnormSrgb);
        assert!(fmt.is_srgb());
        assert_payload_matches(&out);
    }

    #[test]
    fn normal_map_is_bc5() {
        let px = solid(16, 16, [128, 128, 255, 255]);
        let out = bake_rgba8(
            &px,
            16,
            16,
            BakeParams {
                role: TextureRole::NormalMap,
                ..Default::default()
            },
        )
        .unwrap();
        let (_, _, _, _, fmt) = header(&out);
        assert_eq!(fmt, RmipFormat::Bc5RgUnorm);
        assert!(!fmt.is_srgb());
        assert_payload_matches(&out);
    }

    #[test]
    fn linear_data_low_quality_picks_bc1_or_bc3_on_alpha() {
        // Opaque → BC1.
        let opaque = solid(8, 8, [10, 20, 30, 255]);
        let out = bake_rgba8(
            &opaque,
            8,
            8,
            BakeParams {
                role: TextureRole::LinearData,
                high_quality: false,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(header(&out).4, RmipFormat::Bc1RgbaUnorm);

        // Has alpha → BC3.
        let alpha = solid(8, 8, [10, 20, 30, 128]);
        let out = bake_rgba8(
            &alpha,
            8,
            8,
            BakeParams {
                role: TextureRole::LinearData,
                high_quality: false,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(header(&out).4, RmipFormat::Bc3RgbaUnorm);
    }

    #[test]
    fn uncompressed_path_roundtrips_sizes() {
        let px = solid(8, 8, [1, 2, 3, 4]);
        let out = bake_rgba8(
            &px,
            8,
            8,
            BakeParams {
                role: TextureRole::Color,
                compress: false,
                ..Default::default()
            },
        )
        .unwrap();
        let (_, _, _, _, fmt) = header(&out);
        assert_eq!(fmt, RmipFormat::Rgba8UnormSrgb);
        assert_payload_matches(&out);
    }

    #[test]
    fn max_size_clamps_longest_side() {
        let px = solid(32, 16, [255, 0, 0, 255]);
        let out = bake_rgba8(
            &px,
            32,
            16,
            BakeParams {
                max_size: 8,
                ..Default::default()
            },
        )
        .unwrap();
        let (_, w, h, _, _) = header(&out);
        assert_eq!((w, h), (8, 4), "aspect-preserving clamp to longest side");
        assert_payload_matches(&out);
    }

    #[test]
    fn renormalize_keeps_unit_length() {
        // A skewed normal that isn't unit length should be pushed back toward
        // the unit sphere after renormalization.
        let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(2, 2, Rgba([200, 200, 200, 255]));
        renormalize_normals(&mut img);
        let p = img.get_pixel(0, 0).0;
        let x = p[0] as f32 / 127.5 - 1.0;
        let y = p[1] as f32 / 127.5 - 1.0;
        let z = p[2] as f32 / 127.5 - 1.0;
        let len = (x * x + y * y + z * z).sqrt();
        assert!((len - 1.0).abs() < 0.02, "len {len} not ~1");
    }
}
