//! USDC compression — Pixar's integer coding + LZ4.
//!
//! The compression is a two-stage pipeline:
//! 1. Custom integer encoding: delta transform + 2-bit classification + variable-width storage
//! 2. LZ4 block compression of the encoded buffer
//!
//! On disk, each compressed integer blob is stored as:
//!   u64: compressedSize (bytes of LZ4-compressed data)
//!   compressedSize bytes: LZ4-compressed data
//!
//! After LZ4 decompression, the buffer contains:
//!   i32: commonDelta (most frequent delta value)
//!   ceil(numInts * 2 / 8) bytes: 2-bit codes packed 4-per-byte
//!   variable bytes: non-common delta values (i8, i16, or i32 depending on code)

use crate::{UsdError, UsdResult};

/// LZ4 decompress (TfFastCompression format: 8-byte uncompressed size header + raw LZ4 block).
pub fn decompress_lz4(compressed: &[u8], max_output: usize) -> UsdResult<Vec<u8>> {
    if compressed.len() < 8 {
        return Err(UsdError::Parse("LZ4: data too small for header".into()));
    }

    // TfFastCompression prepends i64 uncompressed size
    let uncompressed_size =
        i64::from_le_bytes(compressed[0..8].try_into().unwrap()) as usize;
    let lz4_data = &compressed[8..];

    let output_size = uncompressed_size.min(max_output);

    lz4_flex::decompress(lz4_data, output_size)
        .map_err(|e| UsdError::Parse(format!("LZ4 decompression failed: {}", e)))
}

/// Decompression working space size for N 32-bit integers.
fn working_space_size(num_ints: usize) -> usize {
    if num_ints == 0 {
        return 0;
    }
    // commonValue (4 bytes) + 2-bit codes + max int bytes
    4 + (num_ints * 2 + 7) / 8 + num_ints * 4
}

/// Read a compressed u32 array from data at `pos`.
///
/// Format on disk: `[u64 compressedSize] [compressedSize bytes of LZ4 data]`
///
/// The LZ4 data decompresses to the integer-coded buffer which is then decoded.
pub fn read_compressed_ints_with_count(
    data: &[u8],
    pos: &mut usize,
    num_ints: usize,
) -> UsdResult<Vec<u32>> {
    if num_ints == 0 {
        return Ok(Vec::new());
    }

    // Read u64 compressed size prefix
    if *pos + 8 > data.len() {
        return Err(UsdError::Parse("Compressed ints: truncated size prefix".into()));
    }

    let comp_size = u64::from_le_bytes(data[*pos..*pos + 8].try_into().unwrap()) as usize;
    *pos += 8;

    if comp_size == 0 {
        return Ok(vec![0u32; num_ints]);
    }

    if *pos + comp_size > data.len() {
        return Err(UsdError::Parse(format!(
            "Compressed ints: need {} compressed bytes at {}, have {}",
            comp_size, *pos, data.len() - *pos
        )));
    }

    let compressed = &data[*pos..*pos + comp_size];
    *pos += comp_size;

    // Stage 1: LZ4 decompress
    let working_size = working_space_size(num_ints);
    let encoded = decompress_lz4(compressed, working_size)?;

    // Stage 2: Decode integer-coded buffer
    decode_integers_i32(&encoded, num_ints)
}

/// Read a compressed u32 array with a u64 count prefix followed by a u64 size prefix.
pub fn read_compressed_ints(data: &[u8], pos: &mut usize) -> UsdResult<Vec<u32>> {
    if *pos + 8 > data.len() {
        return Err(UsdError::Parse("Compressed ints: truncated count".into()));
    }

    let num_ints = u64::from_le_bytes(data[*pos..*pos + 8].try_into().unwrap()) as usize;
    *pos += 8;

    read_compressed_ints_with_count(data, pos, num_ints)
}

/// Read compressed i32 array (same format but returns i32).
pub fn read_compressed_signed_ints(
    data: &[u8],
    pos: &mut usize,
    num_ints: usize,
) -> UsdResult<Vec<i32>> {
    let unsigned = read_compressed_ints_with_count(data, pos, num_ints)?;
    Ok(unsigned.into_iter().map(|v| v as i32).collect())
}

/// Decode the custom integer-coded buffer into u32 values.
///
/// Buffer format:
///   i32: commonDelta
///   ceil(numInts * 2 / 8) bytes: 2-bit codes (4 per byte, LSB first)
///     00 = Common (use commonDelta)
///     01 = Small (read i8)
///     10 = Medium (read i16)
///     11 = Large (read i32)
///   variable bytes: non-common delta values
fn decode_integers_i32(encoded: &[u8], num_ints: usize) -> UsdResult<Vec<u32>> {
    if encoded.len() < 4 {
        return Err(UsdError::Parse("Integer decode: buffer too small".into()));
    }

    // Read common delta value
    let common_delta = i32::from_le_bytes(encoded[0..4].try_into().unwrap());

    // Read 2-bit codes
    let num_code_bytes = (num_ints * 2 + 7) / 8;
    let codes_start = 4;
    let vints_start = codes_start + num_code_bytes;

    if codes_start + num_code_bytes > encoded.len() {
        return Err(UsdError::Parse("Integer decode: codes truncated".into()));
    }

    let codes = &encoded[codes_start..codes_start + num_code_bytes];
    let vints = if vints_start < encoded.len() {
        &encoded[vints_start..]
    } else {
        &[]
    };

    let mut result = Vec::with_capacity(num_ints);
    let mut prev: i32 = 0;
    let mut vint_pos = 0usize;

    for i in 0..num_ints {
        let byte_idx = i / 4;
        let bit_shift = (i % 4) * 2;
        let code = if byte_idx < codes.len() {
            (codes[byte_idx] >> bit_shift) & 0x03
        } else {
            0
        };

        let delta: i32 = match code {
            0 => common_delta, // Common
            1 => {
                // Small: i8
                if vint_pos < vints.len() {
                    let v = vints[vint_pos] as i8 as i32;
                    vint_pos += 1;
                    v
                } else {
                    0
                }
            }
            2 => {
                // Medium: i16
                if vint_pos + 2 <= vints.len() {
                    let v = i16::from_le_bytes(
                        vints[vint_pos..vint_pos + 2].try_into().unwrap(),
                    ) as i32;
                    vint_pos += 2;
                    v
                } else {
                    0
                }
            }
            3 => {
                // Large: i32
                if vint_pos + 4 <= vints.len() {
                    let v = i32::from_le_bytes(
                        vints[vint_pos..vint_pos + 4].try_into().unwrap(),
                    );
                    vint_pos += 4;
                    v
                } else {
                    0
                }
            }
            _ => 0,
        };

        prev = prev.wrapping_add(delta);
        result.push(prev as u32);
    }

    Ok(result)
}

/// Decompress raw LZ4 data with TfFastCompression header (used for value reps).
pub fn decompress_lz4_raw(compressed: &[u8], max_output: usize) -> UsdResult<Vec<u8>> {
    decompress_lz4(compressed, max_output)
}
