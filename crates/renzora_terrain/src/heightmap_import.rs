//! Heightmap import — load PNG (8/16-bit grayscale) or RAW16 files into terrain chunks.

use crate::data::{TerrainChunkData, TerrainData};
use std::path::Path;

/// Supported heightmap file formats.
#[derive(Clone, Debug, Default)]
pub enum HeightmapFormat {
    /// Work it out from the bytes — see [`decode_heightmap`]. The default,
    /// because every caller that has a file picked by the user has no more
    /// information about it than the bytes do.
    #[default]
    Auto,
    /// PNG file (auto-detects 8 or 16-bit grayscale).
    Png,
    /// Raw 16-bit unsigned integers, row-major.
    Raw16 {
        width: u32,
        height: u32,
        big_endian: bool,
    },
}

/// A decoded heightmap: samples normalized to 0..1, row-major from the image's
/// top-left corner.
///
/// Carries its own sampler because the two consumers — the whole-terrain import
/// below and the Generate tool's heightmap source — need the same bilinear read
/// and had no business each writing one.
pub struct HeightmapImage {
    pub width: u32,
    pub height: u32,
    pub samples: Vec<f32>,
    /// The image's own value range, found once at decode.
    ///
    /// Real-world heightmaps rarely use the full 0..1 their container allows —
    /// a 16-bit DEM of a mountain range routinely spans something like 0.48 to
    /// 0.68, because 0 and 65535 are sea level and the highest point *on Earth*,
    /// not on the tile. Knowing the actual range is what lets the generator
    /// stretch it back out; scanning 16 million samples per frame to find it
    /// would not be.
    min: f32,
    max: f32,
}

/// Dimensions only. The samples are up to 16 million floats, and a `Debug` that
/// prints them turns one stray `dbg!` into a hung terminal.
impl std::fmt::Debug for HeightmapImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeightmapImage")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

impl HeightmapImage {
    /// Wrap decoded samples, scanning them once for their range.
    pub fn new(width: u32, height: u32, samples: Vec<f32>) -> Self {
        let mut min = f32::MAX;
        let mut max = f32::MIN;
        for &s in &samples {
            if s < min {
                min = s;
            }
            if s > max {
                max = s;
            }
        }
        if samples.is_empty() {
            min = 0.0;
            max = 1.0;
        }
        Self {
            width,
            height,
            samples,
            min,
            max,
        }
    }

    /// The lowest and highest sample in the image.
    pub fn range(&self) -> (f32, f32) {
        (self.min, self.max)
    }

    /// How much of the available 0..1 the image actually uses. Under about 0.5
    /// the file is worth levelling before it becomes terrain.
    pub fn coverage(&self) -> f32 {
        (self.max - self.min).max(0.0)
    }

    /// Bilinear read at normalized `u`/`v`, both clamped to the edge. Outside
    /// the image the nearest border pixel repeats, which is what keeps a
    /// heightmap laid over a region from tearing at the region's border.
    pub fn sample_uv(&self, u: f32, v: f32) -> f32 {
        if self.width == 0 || self.height == 0 {
            return 0.0;
        }
        let x = u.clamp(0.0, 1.0) * (self.width - 1) as f32;
        let z = v.clamp(0.0, 1.0) * (self.height - 1) as f32;
        bilinear_sample(&self.samples, self.width, self.height, x, z)
    }
}

/// PNG's 8-byte file signature. Sniffed rather than trusting the extension:
/// heightmaps get renamed, and a `.raw` that is really a PNG should still load.
const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// Decode heightmap bytes into a normalized image.
///
/// [`HeightmapFormat::Auto`] sniffs the PNG signature and otherwise treats the
/// file as RAW16 with a **square** side inferred from its length. That guess is
/// right for essentially every heightmap in the wild — terrain generators export
/// square power-of-two `.r16` — and being wrong is a clear error rather than a
/// silently skewed landscape, since a non-square byte count has no integer side.
pub fn decode_heightmap(
    data: &[u8],
    format: &HeightmapFormat,
) -> Result<HeightmapImage, String> {
    let (width, height, samples) = match format {
        HeightmapFormat::Png => load_png(data)?,
        HeightmapFormat::Raw16 {
            width,
            height,
            big_endian,
        } => load_raw16(data, *width, *height, *big_endian)?,
        HeightmapFormat::Auto => {
            if data.starts_with(&PNG_MAGIC) {
                load_png(data)?
            } else {
                // Little-endian: what every terrain tool writes, and the byte
                // order the format's users mean when they say "RAW16".
                let (w, h) = infer_raw16_size(data)?;
                load_raw16(data, w, h, false)?
            }
        }
    };
    Ok(HeightmapImage::new(width, height, samples))
}

/// Read and decode a heightmap file.
pub fn load_heightmap_file(
    path: &Path,
    format: &HeightmapFormat,
) -> Result<HeightmapImage, String> {
    let data = std::fs::read(path).map_err(|e| format!("Failed to read file: {e}"))?;
    decode_heightmap(&data, format)
}

/// The side length of a square RAW16 image with this many bytes.
fn infer_raw16_size(data: &[u8]) -> Result<(u32, u32), String> {
    if data.len() < 2 || !data.len().is_multiple_of(2) {
        return Err(format!(
            "Not a PNG, and {} bytes is not a whole number of 16-bit samples.",
            data.len()
        ));
    }
    let count = data.len() / 2;
    let side = (count as f64).sqrt().round() as usize;
    if side == 0 || side * side != count {
        return Err(format!(
            "Not a PNG, and {count} samples is not a square RAW16 image. \
             Non-square RAW16 needs its width and height given explicitly."
        ));
    }
    Ok((side as u32, side as u32))
}

/// Settings for heightmap import.
#[derive(Clone, Debug)]
pub struct HeightmapImportSettings {
    pub format: HeightmapFormat,
    /// Multiplier applied to normalized [0,1] heights.
    pub height_scale: f32,
    /// Offset added after scaling.
    pub height_offset: f32,
    /// Invert heights (1 - h).
    pub invert: bool,
}

impl Default for HeightmapImportSettings {
    fn default() -> Self {
        Self {
            format: HeightmapFormat::Auto,
            height_scale: 1.0,
            height_offset: 0.0,
            invert: false,
        }
    }
}

/// Load a heightmap file and produce per-chunk height arrays.
///
/// Returns a Vec of `(chunk_x, chunk_z, heights)` tuples, one per chunk.
pub fn import_heightmap(
    path: &Path,
    settings: &HeightmapImportSettings,
    terrain: &TerrainData,
) -> Result<Vec<(u32, u32, Vec<f32>)>, String> {
    let image = load_heightmap_file(path, &settings.format)?;
    let (src_width, src_height, normalized) = (image.width, image.height, image.samples);

    // Resample into per-chunk heightmaps
    let res = terrain.chunk_resolution;
    let total_verts_x = terrain.chunks_x * (res - 1) + 1;
    let total_verts_z = terrain.chunks_z * (res - 1) + 1;

    let mut result = Vec::new();

    for cz in 0..terrain.chunks_z {
        for cx in 0..terrain.chunks_x {
            let mut heights = Vec::with_capacity((res * res) as usize);

            for vz in 0..res {
                for vx in 0..res {
                    let global_vx = cx * (res - 1) + vx;
                    let global_vz = cz * (res - 1) + vz;

                    // Map to source image coordinates
                    let src_x =
                        global_vx as f32 / (total_verts_x - 1) as f32 * (src_width - 1) as f32;
                    let src_z =
                        global_vz as f32 / (total_verts_z - 1) as f32 * (src_height - 1) as f32;

                    let h = bilinear_sample(&normalized, src_width, src_height, src_x, src_z);
                    let h = if settings.invert { 1.0 - h } else { h };
                    let h = h * settings.height_scale + settings.height_offset;
                    heights.push(h.clamp(0.0, 1.0));
                }
            }

            result.push((cx, cz, heights));
        }
    }

    Ok(result)
}

/// Apply imported heights to existing terrain chunks.
pub fn apply_imported_heights(
    chunks: &mut [&mut TerrainChunkData],
    imported: &[(u32, u32, Vec<f32>)],
) {
    for (cx, cz, heights) in imported {
        if let Some(chunk) = chunks
            .iter_mut()
            .find(|c| c.chunk_x == *cx && c.chunk_z == *cz)
        {
            chunk.base_heights = heights.clone();
            chunk.dirty = true;
        }
    }
}

/// Export terrain heights to a 16-bit PNG buffer.
pub fn export_heightmap_png16(
    terrain: &TerrainData,
    chunks: &[&TerrainChunkData],
) -> Result<Vec<u8>, String> {
    let res = terrain.chunk_resolution;
    let total_w = terrain.chunks_x * (res - 1) + 1;
    let total_h = terrain.chunks_z * (res - 1) + 1;

    let mut pixels = vec![0u16; (total_w * total_h) as usize];

    for chunk in chunks {
        for vz in 0..res {
            for vx in 0..res {
                let global_x = chunk.chunk_x * (res - 1) + vx;
                let global_z = chunk.chunk_z * (res - 1) + vz;
                let h = chunk.get_height(vx, vz, res);
                let pixel = (h * 65535.0).round() as u16;
                pixels[(global_z * total_w + global_x) as usize] = pixel;
            }
        }
    }

    // Encode as 16-bit grayscale PNG
    let mut buf = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut buf, total_w, total_h);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Sixteen);
        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("PNG encode error: {e}"))?;
        let bytes: Vec<u8> = pixels.iter().flat_map(|p| p.to_be_bytes()).collect();
        writer
            .write_image_data(&bytes)
            .map_err(|e| format!("PNG write error: {e}"))?;
    }

    Ok(buf)
}

// ── Internal helpers ─────────────────────────────────────────────────────────

/// Public wrapper for stamp brush loading.
pub fn load_png_public(data: &[u8]) -> Result<(u32, u32, Vec<f32>), String> {
    load_png(data)
}

/// Decode any PNG into normalized heights.
///
/// Deliberately generic over colour type and bit depth rather than matching a
/// list of them. The list version rejected 16-bit RGB — which is what a lot of
/// terrain and texture tools write, since "16-bit heightmap" and "RGB" are
/// orthogonal choices to them — and every such rejection is a file the user can
/// plainly see is a heightmap being refused for a reason about its container.
/// There is no colour layout a height can't be read out of, so there is nothing
/// here worth failing on.
fn load_png(data: &[u8]) -> Result<(u32, u32, Vec<f32>), String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(data));
    // Expand palettes to RGB and sub-byte grayscale up to 8-bit, so what comes
    // out is always whole 8- or 16-bit samples and the channel walk below holds.
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("PNG decode error: {e}"))?;

    let w = reader.info().width;
    let h = reader.info().height;
    // *Output* colour type, not the file's: after EXPAND they differ.
    let (color, depth) = reader.output_color_type();
    let channels = color.samples();

    let mut buf = vec![0u8; reader.output_buffer_size()];
    reader
        .next_frame(&mut buf)
        .map_err(|e| format!("PNG frame error: {e}"))?;

    let pixels = (w as usize) * (h as usize);

    // Colour channels collapse to luminance rather than to the red channel. A
    // real grayscale-in-RGB heightmap has R == G == B, so the weights sum to 1
    // and this is exactly the old red-channel read; a heightmap that is *tinted*
    // — an AI-generated or hillshaded one — reads as the greyscale a human sees
    // instead of as its red separation, which for those is the difference
    // between terrain and noise.
    let luma = |c: &[f32]| -> f32 {
        match channels {
            1 | 2 => c[0],
            _ => 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2],
        }
    };

    let normalized: Vec<f32> = match depth {
        png::BitDepth::Eight => buf
            .chunks_exact(channels)
            .take(pixels)
            .map(|px| {
                let s: Vec<f32> = px.iter().map(|&b| b as f32 / 255.0).collect();
                luma(&s)
            })
            .collect(),
        png::BitDepth::Sixteen => buf
            .chunks_exact(channels * 2)
            .take(pixels)
            .map(|px| {
                let s: Vec<f32> = px
                    .chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]) as f32 / 65535.0)
                    .collect();
                luma(&s)
            })
            .collect(),
        // EXPAND lifts 1/2/4-bit to 8, so this is unreachable in practice. It
        // stays an error rather than an unwrap because "unreachable" is a claim
        // about a dependency's behaviour, not about ours.
        other => {
            return Err(format!(
                "Unsupported PNG bit depth {other:?}. Use 8- or 16-bit."
            ));
        }
    };

    if normalized.len() < pixels {
        return Err(format!(
            "PNG is truncated: {}x{} needs {pixels} pixels, decoded {}.",
            w,
            h,
            normalized.len()
        ));
    }

    Ok((w, h, normalized))
}

fn load_raw16(
    data: &[u8],
    width: u32,
    height: u32,
    big_endian: bool,
) -> Result<(u32, u32, Vec<f32>), String> {
    let expected = (width * height * 2) as usize;
    if data.len() < expected {
        return Err(format!(
            "RAW16 file too small: expected {} bytes for {}x{}, got {}",
            expected,
            width,
            height,
            data.len()
        ));
    }

    let normalized: Vec<f32> = data
        .chunks_exact(2)
        .take((width * height) as usize)
        .map(|c| {
            let val = if big_endian {
                u16::from_be_bytes([c[0], c[1]])
            } else {
                u16::from_le_bytes([c[0], c[1]])
            };
            val as f32 / 65535.0
        })
        .collect();

    Ok((width, height, normalized))
}

fn bilinear_sample(data: &[f32], width: u32, height: u32, x: f32, z: f32) -> f32 {
    let x0 = (x.floor() as u32).min(width - 1);
    let z0 = (z.floor() as u32).min(height - 1);
    let x1 = (x0 + 1).min(width - 1);
    let z1 = (z0 + 1).min(height - 1);
    let tx = x.fract();
    let tz = z.fract();

    let get =
        |xi: u32, zi: u32| -> f32 { data.get((zi * width + xi) as usize).copied().unwrap_or(0.0) };

    let h00 = get(x0, z0);
    let h10 = get(x1, z0);
    let h01 = get(x0, z1);
    let h11 = get(x1, z1);

    let h0 = h00 * (1.0 - tx) + h10 * tx;
    let h1 = h01 * (1.0 - tx) + h11 * tx;
    h0 * (1.0 - tz) + h1 * tz
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode `samples` (row-major, `channels` interleaved per pixel) as a
    /// 16-bit PNG of the given colour type.
    fn encode16(width: u32, height: u32, color: png::ColorType, samples: &[u16]) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut encoder = png::Encoder::new(&mut buf, width, height);
        encoder.set_color(color);
        encoder.set_depth(png::BitDepth::Sixteen);
        let mut writer = encoder.write_header().unwrap();
        let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_be_bytes()).collect();
        writer.write_image_data(&bytes).unwrap();
        drop(writer);
        buf
    }

    fn encode8(width: u32, height: u32, color: png::ColorType, samples: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut encoder = png::Encoder::new(&mut buf, width, height);
        encoder.set_color(color);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(samples).unwrap();
        drop(writer);
        buf
    }

    /// The format that started this: 16-bit RGB is what a lot of terrain and
    /// texture tools write, and the old enumerate-the-cases decoder rejected it
    /// outright. A grey ramp in three 16-bit channels has to come back as a ramp.
    #[test]
    fn sixteen_bit_rgb_decodes() {
        let (w, h) = (4u32, 2u32);
        let mut samples = Vec::new();
        for _ in 0..h {
            for x in 0..w {
                let v = (x as f32 / (w - 1) as f32 * 65535.0) as u16;
                samples.extend_from_slice(&[v, v, v]);
            }
        }
        let png = encode16(w, h, png::ColorType::Rgb, &samples);
        let (dw, dh, values) = load_png(&png).expect("16-bit RGB must decode");
        assert_eq!((dw, dh), (w, h));
        assert_eq!(values.len(), (w * h) as usize);
        assert!(values[0] < 1e-3);
        assert!((values[3] - 1.0).abs() < 1e-3);
    }

    /// The same heights written four ways must decode to the same heights.
    /// Colour layout is a container detail; it must not change the terrain.
    #[test]
    fn every_colour_layout_agrees_on_a_grey_image() {
        let (w, h) = (4u32, 4u32);
        let grey: Vec<u16> = (0..w * h)
            .map(|i| (i as f32 / (w * h - 1) as f32 * 65535.0) as u16)
            .collect();

        let gray = load_png(&encode16(w, h, png::ColorType::Grayscale, &grey)).unwrap();
        let rgb = load_png(&encode16(
            w,
            h,
            png::ColorType::Rgb,
            &grey.iter().flat_map(|&v| [v, v, v]).collect::<Vec<_>>(),
        ))
        .unwrap();
        let rgba = load_png(&encode16(
            w,
            h,
            png::ColorType::Rgba,
            &grey
                .iter()
                .flat_map(|&v| [v, v, v, u16::MAX])
                .collect::<Vec<_>>(),
        ))
        .unwrap();
        let ga = load_png(&encode16(
            w,
            h,
            png::ColorType::GrayscaleAlpha,
            &grey.iter().flat_map(|&v| [v, u16::MAX]).collect::<Vec<_>>(),
        ))
        .unwrap();

        for (label, other) in [("rgb", &rgb), ("rgba", &rgba), ("grayscale+alpha", &ga)] {
            for (i, (a, b)) in gray.2.iter().zip(other.2.iter()).enumerate() {
                assert!((a - b).abs() < 1e-4, "{label} drifted at pixel {i}");
            }
        }
    }

    #[test]
    fn eight_bit_layouts_still_decode() {
        let (w, h) = (2u32, 2u32);
        let gray = load_png(&encode8(w, h, png::ColorType::Grayscale, &[0, 85, 170, 255])).unwrap();
        assert_eq!(gray.2.len(), 4);
        assert!(gray.2[0] < 1e-3);
        assert!((gray.2[3] - 1.0).abs() < 1e-3);

        let rgb = load_png(&encode8(
            w,
            h,
            png::ColorType::Rgb,
            &[0, 0, 0, 85, 85, 85, 170, 170, 170, 255, 255, 255],
        ))
        .unwrap();
        for (a, b) in gray.2.iter().zip(rgb.2.iter()) {
            assert!((a - b).abs() < 1e-4);
        }
    }

    /// Auto sniffs the signature rather than trusting a name, so a `.raw` that
    /// is really a PNG still loads.
    #[test]
    fn auto_detects_png_from_its_signature() {
        let png = encode8(2, 2, png::ColorType::Grayscale, &[0, 85, 170, 255]);
        let image = decode_heightmap(&png, &HeightmapFormat::Auto).unwrap();
        assert_eq!((image.width, image.height), (2, 2));
        assert!((image.sample_uv(1.0, 1.0) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn auto_infers_a_square_raw16_side() {
        // 64 x 64 x 2 bytes.
        let data = vec![0u8; 64 * 64 * 2];
        let image = decode_heightmap(&data, &HeightmapFormat::Auto).unwrap();
        assert_eq!((image.width, image.height), (64, 64));
    }

    /// Guessing wrong would be a silently skewed landscape, so a byte count with
    /// no integer side has to be an error the user can act on.
    #[test]
    fn a_non_square_raw16_is_an_error() {
        let data = vec![0u8; 64 * 63 * 2];
        assert!(decode_heightmap(&data, &HeightmapFormat::Auto).is_err());
    }

    #[test]
    fn the_range_is_measured_at_decode() {
        let (w, h) = (4u32, 1u32);
        // 0.25 .. 0.75 of the 16-bit range.
        let samples: Vec<u16> = (0..w)
            .map(|x| (16384.0 + x as f32 / (w - 1) as f32 * 32768.0) as u16)
            .flat_map(|v| [v, v, v])
            .collect();
        let image = decode_heightmap(
            &encode16(w, h, png::ColorType::Rgb, &samples),
            &HeightmapFormat::Auto,
        )
        .unwrap();
        let (lo, hi) = image.range();
        assert!((lo - 0.25).abs() < 1e-3, "lo was {lo}");
        assert!((hi - 0.75).abs() < 1e-3, "hi was {hi}");
        assert!((image.coverage() - 0.5).abs() < 1e-3);
    }

    /// `sample_uv` clamps rather than wrapping — a heightmap laid over a region
    /// must not tear at the region's border.
    #[test]
    fn sampling_clamps_at_the_edges() {
        let image = HeightmapImage::new(2, 2, vec![0.0, 1.0, 0.0, 1.0]);
        assert_eq!(image.sample_uv(-5.0, 0.5), image.sample_uv(0.0, 0.5));
        assert_eq!(image.sample_uv(5.0, 0.5), image.sample_uv(1.0, 0.5));
    }
}
