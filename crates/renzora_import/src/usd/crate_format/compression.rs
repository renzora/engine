#![allow(dead_code)] // USD Crate format reader — partial implementation, helpers staged.

//! USDC compression -- Pixar's integer coding + LZ4.
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

use super::super::{UsdError, UsdResult};

/// Largest block Pixar hands to LZ4 in one go, and therefore the size every
/// chunk but the last decompresses to. Mirrors `LZ4_MAX_INPUT_SIZE`.
const LZ4_MAX_INPUT_SIZE: usize = 0x7E00_0000;

/// Decompress a `TfFastCompression` buffer.
///
/// The layout is **not** a bare LZ4 block. Pixar prefixes a `u8` chunk count:
///
/// ```text
/// [u8 nChunks == 0]  [ LZ4 block ]                       // the common case
/// [u8 nChunks == N]  N x ( [i32 chunkSize] [LZ4 block] )  // > 2 GiB inputs
/// ```
///
/// This used to skip eight bytes here, reading them as an uncompressed-size
/// header — a header `TfFastCompression` does not write. That discarded the
/// chunk byte plus seven bytes of real payload, and LZ4 then failed with
/// "the offset to copy is not contained in the decompressed buffer" on every
/// USDC file whose sections are compressed. The sizes the callers need are
/// already in the section headers, which is why none is stored here.
///
/// `uncompressed_size` is an upper bound used to size the output; the result is
/// truncated to whatever actually decoded.
pub fn decompress_lz4(compressed: &[u8], uncompressed_size: usize) -> UsdResult<Vec<u8>> {
    let Some((&n_chunks, rest)) = compressed.split_first() else {
        return Err(UsdError::Parse("LZ4: empty compressed buffer".into()));
    };

    if n_chunks == 0 {
        return lz4_flex::decompress(rest, uncompressed_size)
            .map_err(|e| UsdError::Parse(format!("LZ4 decompression failed: {}", e)));
    }

    let mut out = Vec::with_capacity(uncompressed_size);
    let mut pos = 0usize;
    for chunk in 0..n_chunks as usize {
        let size = super::read::le_i32(rest, pos).ok_or_else(|| {
            UsdError::Parse(format!(
                "LZ4: truncated chunk header {} of {}",
                chunk + 1,
                n_chunks
            ))
        })?;
        pos += 4;
        let size = usize::try_from(size)
            .map_err(|_| UsdError::Parse("LZ4: negative chunk size".into()))?;
        // `pos + size` with a file-supplied `size`: wrapping it made an
        // over-long chunk look like it fit.
        if pos.checked_add(size).is_none_or(|end| end > rest.len()) {
            return Err(UsdError::Parse(format!(
                "LZ4: chunk {} claims {} bytes, {} remain",
                chunk + 1,
                size,
                rest.len() - pos
            )));
        }
        // Every chunk but the last fills a full block; the last takes whatever
        // of the total is left.
        let want = uncompressed_size
            .saturating_sub(out.len())
            .min(LZ4_MAX_INPUT_SIZE);
        let piece = lz4_flex::decompress(&rest[pos..pos + size], want)
            .map_err(|e| UsdError::Parse(format!("LZ4 chunk {}: {}", chunk + 1, e)))?;
        out.extend_from_slice(&piece);
        pos += size;
    }
    Ok(out)
}

/// Decompression working space size for N 32-bit integers.
fn working_space_size(num_ints: usize) -> usize {
    if num_ints == 0 {
        return 0;
    }
    // commonValue (4 bytes) + 2-bit codes + max int bytes
    4 + (num_ints * 2).div_ceil(8) + num_ints * 4
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
    let comp_size = super::read::le_u64(data, *pos)
        .ok_or_else(|| UsdError::Parse("Compressed ints: truncated size prefix".into()))?
        as usize;
    *pos += 8;

    if comp_size == 0 {
        return Ok(vec![0u32; num_ints]);
    }

    // `*pos + comp_size` with `comp_size` out of the file: one `read::slice`
    // does the checked add and the bounds test that used to be two lines that
    // could disagree.
    let compressed = super::read::slice(data, *pos, comp_size).ok_or_else(|| {
        UsdError::Parse(format!(
            "Compressed ints: need {} compressed bytes at {}, have {}",
            comp_size,
            *pos,
            data.len().saturating_sub(*pos)
        ))
    })?;
    *pos += comp_size;

    // Stage 1: LZ4 decompress
    let working_size = working_space_size(num_ints);
    let encoded = decompress_lz4(compressed, working_size)?;

    // Stage 2: Decode integer-coded buffer
    decode_integers_i32(&encoded, num_ints)
}

/// Read a compressed u32 array with a u64 count prefix followed by a u64 size prefix.
pub fn read_compressed_ints(data: &[u8], pos: &mut usize) -> UsdResult<Vec<u32>> {
    let num_ints = super::read::le_u64(data, *pos)
        .ok_or_else(|| UsdError::Parse("Compressed ints: truncated count".into()))?
        as usize;
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
    let common_delta = super::read::le_i32(encoded, 0)
        .ok_or_else(|| UsdError::Parse("Integer decode: buffer too small".into()))?;

    // Read 2-bit codes. `num_ints` is a count out of the file, so doubling it
    // can wrap: a count near `usize::MAX / 2` produced a tiny `num_code_bytes`,
    // which then passed the length test and decoded from the wrong window.
    let num_code_bytes = num_ints
        .checked_mul(2)
        .map(|bits| bits.div_ceil(8))
        .ok_or_else(|| UsdError::Parse("Integer decode: implausible count".into()))?;
    let codes_start = 4;
    let vints_start = codes_start + num_code_bytes;

    let codes = super::read::slice(encoded, codes_start, num_code_bytes)
        .ok_or_else(|| UsdError::Parse("Integer decode: codes truncated".into()))?;
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
            // Small: i8
            1 if vint_pos < vints.len() => {
                let v = vints[vint_pos] as i8 as i32;
                vint_pos += 1;
                v
            }
            // Medium: i16. `vint_pos` only ever advances within `vints`, so
            // these cannot wrap; through the helper anyway so the guard and the
            // read are one expression rather than two that can drift.
            2 if super::read::le_i16(vints, vint_pos).is_some() => {
                let v = super::read::le_i16(vints, vint_pos).unwrap_or(0) as i32;
                vint_pos += 2;
                v
            }
            // Large: i32
            3 if super::read::le_i32(vints, vint_pos).is_some() => {
                let v = super::read::le_i32(vints, vint_pos).unwrap_or(0);
                vint_pos += 4;
                v
            }
            // Out-of-range codes and truncated payloads decode as no delta.
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

#[cfg(test)]
mod lz4_tests {
    use super::*;

    /// Wrap a payload the way `TfFastCompression` writes a single chunk.
    fn single_chunk(data: &[u8]) -> Vec<u8> {
        let mut out = vec![0u8];
        out.extend_from_slice(&lz4_flex::compress(data));
        out
    }

    #[test]
    fn round_trips_a_single_chunk() {
        let payload: Vec<u8> = (0..4000u32).map(|i| (i % 251) as u8).collect();
        let packed = single_chunk(&payload);
        let out = decompress_lz4(&packed, payload.len()).expect("decompresses");
        assert_eq!(out, payload);
    }

    #[test]
    fn does_not_skip_a_size_header_that_is_not_there() {
        // The regression: eight bytes used to be discarded up front, which ate
        // the chunk byte and seven bytes of the LZ4 block.
        let payload = b"usd tokens: a short but real payload".to_vec();
        let packed = single_chunk(&payload);
        assert_eq!(packed[0], 0, "leading byte is the chunk count");
        let out = decompress_lz4(&packed, payload.len()).unwrap();
        assert_eq!(out, payload);
    }

    #[test]
    fn reads_a_multi_chunk_buffer() {
        let a: Vec<u8> = (0..1000u32).map(|i| (i % 97) as u8).collect();
        let b: Vec<u8> = (0..1000u32).map(|i| (i % 89) as u8).collect();
        let mut packed = vec![2u8];
        for part in [&a, &b] {
            let c = lz4_flex::compress(part);
            packed.extend_from_slice(&(c.len() as i32).to_le_bytes());
            packed.extend_from_slice(&c);
        }
        let out = decompress_lz4(&packed, a.len() + b.len()).expect("decompresses");
        assert_eq!(out.len(), a.len() + b.len());
        assert_eq!(&out[..a.len()], &a[..]);
        assert_eq!(&out[a.len()..], &b[..]);
    }

    #[test]
    fn an_empty_buffer_is_an_error_not_a_panic() {
        assert!(decompress_lz4(&[], 16).is_err());
    }

    #[test]
    fn a_truncated_chunk_header_is_reported() {
        // Claims two chunks, provides half a header.
        let packed = vec![2u8, 0x10, 0x00];
        let err = decompress_lz4(&packed, 64).unwrap_err();
        assert!(format!("{err:?}").contains("truncated"), "{err:?}");
    }
}
