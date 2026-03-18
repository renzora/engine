//! Heightmap import — load PNG (8/16-bit grayscale) or RAW16 files into terrain chunks.

use crate::data::{TerrainData, TerrainChunkData};
use std::path::Path;

/// Supported heightmap file formats.
#[derive(Clone, Debug)]
pub enum HeightmapFormat {
    /// PNG file (auto-detects 8 or 16-bit grayscale).
    Png,
    /// Raw 16-bit unsigned integers, row-major.
    Raw16 {
        width: u32,
        height: u32,
        big_endian: bool,
    },
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
            format: HeightmapFormat::Png,
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
    let data = std::fs::read(path).map_err(|e| format!("Failed to read file: {e}"))?;

    let (src_width, src_height, normalized) = match &settings.format {
        HeightmapFormat::Png => load_png(&data)?,
        HeightmapFormat::Raw16 { width, height, big_endian } => {
            load_raw16(&data, *width, *height, *big_endian)?
        }
    };

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
                    let src_x = global_vx as f32 / (total_verts_x - 1) as f32 * (src_width - 1) as f32;
                    let src_z = global_vz as f32 / (total_verts_z - 1) as f32 * (src_height - 1) as f32;

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
        if let Some(chunk) = chunks.iter_mut().find(|c| c.chunk_x == *cx && c.chunk_z == *cz) {
            chunk.heights = heights.clone();
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
        let mut writer = encoder.write_header().map_err(|e| format!("PNG encode error: {e}"))?;
        let bytes: Vec<u8> = pixels.iter().flat_map(|p| p.to_be_bytes()).collect();
        writer.write_image_data(&bytes).map_err(|e| format!("PNG write error: {e}"))?;
    }

    Ok(buf)
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn load_png(data: &[u8]) -> Result<(u32, u32, Vec<f32>), String> {
    let decoder = png::Decoder::new(std::io::Cursor::new(data));
    let mut reader = decoder.read_info().map_err(|e| format!("PNG decode error: {e}"))?;
    let info = reader.info().clone();
    let mut buf = vec![0u8; reader.output_buffer_size()];
    reader.next_frame(&mut buf).map_err(|e| format!("PNG frame error: {e}"))?;

    let w = info.width;
    let h = info.height;

    let normalized: Vec<f32> = match (info.color_type, info.bit_depth) {
        (png::ColorType::Grayscale, png::BitDepth::Eight) => {
            buf.iter().take((w * h) as usize).map(|&b| b as f32 / 255.0).collect()
        }
        (png::ColorType::Grayscale, png::BitDepth::Sixteen) => {
            buf.chunks_exact(2)
                .take((w * h) as usize)
                .map(|c| u16::from_be_bytes([c[0], c[1]]) as f32 / 65535.0)
                .collect()
        }
        (png::ColorType::Rgba, png::BitDepth::Eight) => {
            // Use red channel
            buf.chunks_exact(4)
                .take((w * h) as usize)
                .map(|c| c[0] as f32 / 255.0)
                .collect()
        }
        (png::ColorType::Rgb, png::BitDepth::Eight) => {
            // Use red channel
            buf.chunks_exact(3)
                .take((w * h) as usize)
                .map(|c| c[0] as f32 / 255.0)
                .collect()
        }
        _ => {
            return Err(format!(
                "Unsupported PNG format: {:?} {:?}. Use 8/16-bit grayscale or 8-bit RGB/RGBA.",
                info.color_type, info.bit_depth
            ));
        }
    };

    Ok((w, h, normalized))
}

fn load_raw16(data: &[u8], width: u32, height: u32, big_endian: bool) -> Result<(u32, u32, Vec<f32>), String> {
    let expected = (width * height * 2) as usize;
    if data.len() < expected {
        return Err(format!(
            "RAW16 file too small: expected {} bytes for {}x{}, got {}",
            expected, width, height, data.len()
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

    let get = |xi: u32, zi: u32| -> f32 {
        data.get((zi * width + xi) as usize).copied().unwrap_or(0.0)
    };

    let h00 = get(x0, z0);
    let h10 = get(x1, z0);
    let h01 = get(x0, z1);
    let h11 = get(x1, z1);

    let h0 = h00 * (1.0 - tx) + h10 * tx;
    let h1 = h01 * (1.0 - tx) + h11 * tx;
    h0 * (1.0 - tz) + h1 * tz
}
