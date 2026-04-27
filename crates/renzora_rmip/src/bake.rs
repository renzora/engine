//! Encoder for `.rmip` files.
//!
//! Decodes arbitrary image bytes (PNG/JPG/WebP/etc), generates a full
//! mipmap chain via Lanczos3 downsampling, and writes the binary
//! container described in the crate root. Used by `renzora_import` at
//! import time. Gated behind the `bake` feature so consumers that only
//! load `.rmip` (the editor + runtime) don't pull in the `image` crate.

use image::{imageops::FilterType, GenericImageView, ImageBuffer, Rgba};

use crate::{mip_count, RmipFormat, MAGIC, VERSION};

/// Decode arbitrary image bytes (PNG/JPG/etc) and bake them into a
/// `.rmip` byte vector. Lanczos3 is used for downsampling — slower than
/// box filter but visibly sharper, matches Godot's defaults.
///
/// `format` controls the sRGB vs. linear flag stored in the header. The
/// caller picks based on the texture's role (color vs. data map).
pub fn bake_from_image_bytes(
    bytes: &[u8],
    format: RmipFormat,
) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| format!("decode image: {e}"))?;
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();
    bake_from_rgba8(rgba.as_raw(), w, h, format)
}

/// Bake a `.rmip` from a pre-decoded RGBA8 buffer. `pixels.len()` must be
/// `width * height * 4`. Useful when the import pipeline already has
/// decoded pixels.
pub fn bake_from_rgba8(
    pixels: &[u8],
    width: u32,
    height: u32,
    format: RmipFormat,
) -> Result<Vec<u8>, String> {
    if width == 0 || height == 0 {
        return Err("zero-sized image".into());
    }
    let expected = (width as usize) * (height as usize) * 4;
    if pixels.len() != expected {
        return Err(format!(
            "pixel buffer size {} doesn't match {}x{}*4 = {}",
            pixels.len(),
            width,
            height,
            expected,
        ));
    }
    let buf: ImageBuffer<Rgba<u8>, _> =
        ImageBuffer::from_raw(width, height, pixels.to_vec())
            .ok_or_else(|| "ImageBuffer::from_raw failed".to_string())?;

    let mips = mip_count(width, height);
    let mut levels: Vec<Vec<u8>> = Vec::with_capacity(mips as usize);
    levels.push(buf.as_raw().clone());

    let mut current = buf;
    for _ in 1..mips {
        let new_w = (current.width() / 2).max(1);
        let new_h = (current.height() / 2).max(1);
        // Lanczos3 is the highest-quality default in `image::imageops`.
        // For a 1024² source the chain takes <1ms total — cost is paid
        // once at import, never at load.
        let resized = image::imageops::resize(&current, new_w, new_h, FilterType::Lanczos3);
        levels.push(resized.as_raw().clone());
        current = resized;
    }

    let header_len = 24;
    let body_len: usize = levels.iter().map(|l| l.len()).sum();
    let mut out = Vec::with_capacity(header_len + body_len);
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes());
    out.extend_from_slice(&mips.to_le_bytes());
    out.extend_from_slice(&(format as u32).to_le_bytes());
    for level in &levels {
        out.extend_from_slice(level);
    }
    Ok(out)
}
