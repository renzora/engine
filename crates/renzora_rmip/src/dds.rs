//! DDS → `.rmip` transcoding.
//!
//! Most real-world FBX ships its textures as external `.dds` files that are
//! *already* GPU block-compressed with a full mip chain — which is precisely
//! what a `.rmip` holds. So this is a repack, not a bake: the block data is
//! copied across untouched, and clamping to a maximum size is done by dropping
//! whole mip levels off the front of the chain.
//!
//! That matters for more than speed. Round-tripping a BC-compressed texture
//! through RGBA and back re-quantizes every block, so a decode/re-encode would
//! lose quality for no reason, and re-encoding a gigabyte of 2K maps to BC7
//! takes minutes. Copying the blocks is lossless and runs at disk speed.
//!
//! Getting the source into `.rmip` is also what puts it under the engine's
//! texture budget at all: `.rmip` is the only texture format the loader
//! publishes a `#low` mip-tail subasset for, which is what
//! `renzora_engine::texture_stream` swaps to when a material drifts far from
//! the camera. A raw `.dds` sitting in a project is outside all of that — it
//! loads once, at full resolution, and stays there.

use crate::{RmipFormat, MAGIC, VERSION};

/// Header sizes fixed by the DDS spec.
const DDS_MAGIC: &[u8; 4] = b"DDS ";
const DDS_HEADER_LEN: usize = 124;
/// Offset of the first byte of surface data for a plain (non-DX10) file:
/// 4-byte magic + 124-byte header.
const DDS_DATA_OFFSET: usize = 4 + DDS_HEADER_LEN;
/// A `DX10` FourCC adds a 20-byte extended header before the surface data.
const DDS_DX10_HEADER_LEN: usize = 20;

/// `dwCaps2` bits that mean the file holds more than one plain 2D surface.
/// `.rmip` has no way to express either, so those files are left alone.
const DDSCAPS2_CUBEMAP: u32 = 0x200;
const DDSCAPS2_VOLUME: u32 = 0x20_0000;

/// What a DDS file turned into.
pub struct Transcoded {
    pub bytes: Vec<u8>,
    pub format: RmipFormat,
    /// Mip 0 dimensions after clamping.
    pub width: u32,
    pub height: u32,
    pub mips: u32,
    /// Mip levels dropped off the front to satisfy `max_size`.
    pub levels_dropped: u32,
}

/// Repack a DDS file as a `.rmip`, dropping leading mip levels until the base
/// is at most `max_size` on its longest side (`0` disables clamping).
///
/// Returns `Err` with a short reason for anything this can't represent — an
/// uncompressed or exotic pixel format, a cubemap, a volume texture. Callers
/// are expected to fall back to copying the file verbatim rather than treating
/// it as a failure.
///
/// `srgb` picks between the sRGB and linear variants of the block format. DDS
/// FourCC codes carry no colour-space information, so the caller has to supply
/// it from the texture's role in the material (base colour is sRGB; normal,
/// metallic-roughness and occlusion maps are linear).
/// What a DDS header says, without touching the surface data.
#[derive(Clone, Copy, Debug)]
pub struct Description {
    pub format: RmipFormat,
    pub width: u32,
    pub height: u32,
    pub mips: u32,
}

impl Description {
    /// Whether the block format carries a usable alpha channel — the signal an
    /// importer needs to tell a cutout material from an opaque one.
    pub fn has_alpha(&self) -> bool {
        self.format.has_alpha()
    }

    /// Bytes this texture would occupy as a `.rmip` clamped to `max_size`
    /// (`0` = no clamp), which is also what it will occupy in VRAM: block data
    /// uploads one-for-one.
    ///
    /// Exact rather than estimated, and computed from the header alone — so a
    /// caller can total up a whole model's texture set and pick a cap before
    /// reading a single surface.
    pub fn size_at(&self, max_size: u32) -> usize {
        let (mut w, mut h, mut mips) = (self.width, self.height, self.mips.max(1));
        while max_size > 0 && mips > 1 && (w > max_size || h > max_size) {
            w = (w / 2).max(1);
            h = (h / 2).max(1);
            mips -= 1;
        }
        self.format.payload_size(w, h, mips)
    }
}

/// Read a DDS header without pulling the surface chain into memory, and report
/// whether [`transcode`] can handle it.
///
/// Lets a caller decide what extension a texture will be written under, and how
/// big it will end up, when a single model references close to a gigabyte of
/// them.
pub fn probe(header: &[u8], srgb: bool) -> Result<Description, String> {
    parse_header(header, srgb).map(|parsed| Description {
        format: parsed.format,
        width: parsed.width,
        height: parsed.height,
        mips: parsed.mips,
    })
}

struct Header {
    format: RmipFormat,
    data_offset: usize,
    width: u32,
    height: u32,
    mips: u32,
}

fn parse_header(bytes: &[u8], srgb: bool) -> Result<Header, String> {
    if bytes.len() < DDS_DATA_OFFSET || &bytes[0..4] != DDS_MAGIC {
        return Err("not a DDS file".into());
    }
    let read = |offset: usize| -> u32 {
        u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ])
    };
    if read(4) as usize != DDS_HEADER_LEN {
        return Err("unexpected DDS header size".into());
    }

    let height = read(12);
    let width = read(16);
    // `dwMipMapCount` is 0 on files that never declared a chain; treat that as
    // the single base level it actually is.
    let declared_mips = read(28).max(1);
    let caps2 = read(112);
    if caps2 & (DDSCAPS2_CUBEMAP | DDSCAPS2_VOLUME) != 0 {
        return Err("cubemap/volume DDS".into());
    }
    if width == 0 || height == 0 {
        return Err("zero-sized DDS".into());
    }

    let four_cc = &bytes[84..88];
    let (format, data_offset) = if four_cc == b"DX10" {
        let ext = DDS_DATA_OFFSET;
        if bytes.len() < ext + DDS_DX10_HEADER_LEN {
            return Err("truncated DX10 header".into());
        }
        let dxgi = read(ext);
        (
            format_from_dxgi(dxgi, srgb).ok_or_else(|| format!("unsupported DXGI format {dxgi}"))?,
            ext + DDS_DX10_HEADER_LEN,
        )
    } else {
        (
            format_from_four_cc(four_cc, srgb).ok_or_else(|| {
                format!(
                    "unsupported DDS pixel format '{}'",
                    String::from_utf8_lossy(four_cc)
                )
            })?,
            DDS_DATA_OFFSET,
        )
    };

    Ok(Header {
        format,
        data_offset,
        width,
        height,
        mips: declared_mips,
    })
}

/// Repack a DDS file as a `.rmip` — see the module docs for why this is a copy
/// rather than a decode.
pub fn transcode(bytes: &[u8], srgb: bool, max_size: u32) -> Result<Transcoded, String> {
    let Header {
        format,
        data_offset,
        width,
        height,
        mips: declared_mips,
    } = parse_header(bytes, srgb)?;

    // Walk the chain, skipping levels that are still over the cap. A file with
    // no mips can't be clamped this way — there's nothing to fall back to — so
    // it comes through at its native size.
    let mut level_w = width;
    let mut level_h = height;
    let mut offset = data_offset;
    let mut levels_dropped = 0u32;
    let mut remaining = declared_mips;
    while max_size > 0
        && remaining > 1
        && (level_w > max_size || level_h > max_size)
    {
        offset += format.level_byte_size(level_w, level_h);
        level_w = (level_w / 2).max(1);
        level_h = (level_h / 2).max(1);
        levels_dropped += 1;
        remaining -= 1;
    }

    // The declared mip count is not to be trusted — plenty of exporters write
    // a chain length the file doesn't actually contain. Copy only the levels
    // that are really there.
    let mut payload = Vec::new();
    let mut mips = 0u32;
    let (mut w, mut h) = (level_w, level_h);
    for _ in 0..remaining {
        let size = format.level_byte_size(w, h);
        let end = offset + size;
        if end > bytes.len() {
            break;
        }
        payload.extend_from_slice(&bytes[offset..end]);
        offset = end;
        mips += 1;
        if w == 1 && h == 1 {
            break;
        }
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }
    if mips == 0 {
        return Err("DDS surface data is truncated".into());
    }

    let mut out = Vec::with_capacity(crate::HEADER_LEN + payload.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&level_w.to_le_bytes());
    out.extend_from_slice(&level_h.to_le_bytes());
    out.extend_from_slice(&mips.to_le_bytes());
    out.extend_from_slice(&format.code().to_le_bytes());
    out.extend_from_slice(&payload);

    Ok(Transcoded {
        bytes: out,
        format,
        width: level_w,
        height: level_h,
        mips,
        levels_dropped,
    })
}

/// Re-emit a DDS with the same clamping [`transcode`] applies, still as a DDS.
///
/// The import pipeline writes two files per texture: the `.rmip` the material
/// graph samples, and a copy in a format Bevy's own GLB image loader can read,
/// because the intermediate GLB's materials have to resolve for the scene to
/// load at all. Handing that second file back at full resolution would undo the
/// clamp — the point is that *neither* copy is oversized.
///
/// Only the header's dimensions, mip count and linear size change; the
/// surviving block data is copied straight through.
pub fn clamp(bytes: &[u8], max_size: u32) -> Result<Vec<u8>, String> {
    let header = parse_header(bytes, false)?;
    // The colour-space hint doesn't affect layout, so `false` above is fine —
    // this only reuses the parse.
    let trimmed = transcode(bytes, false, max_size)?;
    if trimmed.levels_dropped == 0 {
        return Ok(bytes.to_vec());
    }

    let mut out = bytes[..header.data_offset].to_vec();
    let put = |out: &mut Vec<u8>, offset: usize, value: u32| {
        out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    };
    put(&mut out, 12, trimmed.height);
    put(&mut out, 16, trimmed.width);
    // `dwPitchOrLinearSize` is the top level's byte size for a compressed
    // surface; a stale value confuses strict readers.
    put(
        &mut out,
        20,
        header
            .format
            .level_byte_size(trimmed.width, trimmed.height) as u32,
    );
    put(&mut out, 28, trimmed.mips);
    out.extend_from_slice(&trimmed.bytes[crate::HEADER_LEN..]);
    Ok(out)
}

/// Map a legacy FourCC to the matching `.rmip` block format.
///
/// `ATI2` is the one that earns its keep: it's how essentially every DCC tool
/// writes two-channel tangent-space normal maps, and it is *not* something the
/// `image` crate can decode — so a decode-and-re-encode pipeline would have to
/// skip exactly the maps a scene has most of.
fn format_from_four_cc(four_cc: &[u8], srgb: bool) -> Option<RmipFormat> {
    Some(match four_cc {
        b"DXT1" => {
            if srgb {
                RmipFormat::Bc1RgbaUnormSrgb
            } else {
                RmipFormat::Bc1RgbaUnorm
            }
        }
        b"DXT5" => {
            if srgb {
                RmipFormat::Bc3RgbaUnormSrgb
            } else {
                RmipFormat::Bc3RgbaUnorm
            }
        }
        b"ATI1" | b"BC4U" => RmipFormat::Bc4RUnorm,
        b"ATI2" | b"BC5U" => RmipFormat::Bc5RgUnorm,
        // DXT2/DXT3 are BC2, which `.rmip` has no code for, and the signed BC4/
        // BC5 variants would need a different sampler setup.
        _ => return None,
    })
}

fn format_from_dxgi(dxgi: u32, srgb: bool) -> Option<RmipFormat> {
    Some(match dxgi {
        // BC1
        71 => {
            if srgb {
                RmipFormat::Bc1RgbaUnormSrgb
            } else {
                RmipFormat::Bc1RgbaUnorm
            }
        }
        72 => RmipFormat::Bc1RgbaUnormSrgb,
        // BC3
        77 => {
            if srgb {
                RmipFormat::Bc3RgbaUnormSrgb
            } else {
                RmipFormat::Bc3RgbaUnorm
            }
        }
        78 => RmipFormat::Bc3RgbaUnormSrgb,
        // BC4 / BC5 (unsigned only)
        80 => RmipFormat::Bc4RUnorm,
        83 => RmipFormat::Bc5RgUnorm,
        // BC7
        98 => {
            if srgb {
                RmipFormat::Bc7RgbaUnormSrgb
            } else {
                RmipFormat::Bc7RgbaUnorm
            }
        }
        99 => RmipFormat::Bc7RgbaUnormSrgb,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal DDS with `mips` levels of block data for `format`.
    fn dds(four_cc: &[u8; 4], width: u32, height: u32, mips: u32, format: RmipFormat) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(DDS_MAGIC);
        let mut header = vec![0u8; DDS_HEADER_LEN];
        let put = |h: &mut Vec<u8>, offset: usize, value: u32| {
            h[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        };
        // Header offsets are relative to the header itself, i.e. file offset
        // minus the 4-byte magic.
        put(&mut header, 0, DDS_HEADER_LEN as u32);
        put(&mut header, 8, height);
        put(&mut header, 12, width);
        put(&mut header, 24, mips);
        header[80..84].copy_from_slice(four_cc);
        out.extend_from_slice(&header);

        // Surface data: each level filled with its own level number so the
        // test can tell which levels survived a trim.
        let (mut w, mut h) = (width, height);
        for level in 0..mips {
            out.extend(
                std::iter::repeat(level as u8 + 1).take(format.level_byte_size(w, h)),
            );
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }
        out
    }

    fn header_of(bytes: &[u8]) -> (u32, u32, u32, u32) {
        let r = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
        (r(8), r(12), r(16), r(20))
    }

    #[test]
    fn repacks_dxt1_without_clamping() {
        let src = dds(b"DXT1", 64, 64, 7, RmipFormat::Bc1RgbaUnormSrgb);
        let out = transcode(&src, true, 0).expect("transcode");

        assert_eq!(out.format, RmipFormat::Bc1RgbaUnormSrgb);
        assert_eq!((out.width, out.height, out.mips), (64, 64, 7));
        assert_eq!(out.levels_dropped, 0);

        let (width, height, mips, code) = header_of(&out.bytes);
        assert_eq!((width, height, mips), (64, 64, 7));
        assert_eq!(code, RmipFormat::Bc1RgbaUnormSrgb.code());
        // Payload is the source's surface data copied verbatim.
        assert_eq!(&out.bytes[crate::HEADER_LEN..], &src[DDS_DATA_OFFSET..]);
    }

    #[test]
    fn clamping_drops_leading_mip_levels() {
        // 256² down to a 64 cap = two levels off the front.
        let src = dds(b"DXT5", 256, 256, 9, RmipFormat::Bc3RgbaUnormSrgb);
        let out = transcode(&src, true, 64).expect("transcode");

        assert_eq!(out.levels_dropped, 2);
        assert_eq!((out.width, out.height, out.mips), (64, 64, 7));
        // The surviving base level is the source's level 2 (filled with 3).
        assert_eq!(out.bytes[crate::HEADER_LEN], 3);
    }

    #[test]
    fn normal_maps_keep_their_two_channel_format() {
        // ATI2 is what DCC tools write for tangent-space normals, and it must
        // survive as BC5 regardless of the sRGB hint.
        let src = dds(b"ATI2", 32, 32, 6, RmipFormat::Bc5RgUnorm);
        for srgb in [false, true] {
            let out = transcode(&src, srgb, 0).expect("transcode");
            assert_eq!(out.format, RmipFormat::Bc5RgUnorm);
        }
    }

    #[test]
    fn srgb_hint_selects_the_colour_space() {
        let src = dds(b"DXT1", 16, 16, 5, RmipFormat::Bc1RgbaUnorm);
        assert_eq!(
            transcode(&src, true, 0).unwrap().format,
            RmipFormat::Bc1RgbaUnormSrgb
        );
        assert_eq!(
            transcode(&src, false, 0).unwrap().format,
            RmipFormat::Bc1RgbaUnorm
        );
    }

    #[test]
    fn a_mipless_file_is_left_at_its_native_size() {
        // Nothing to fall back to, so the cap can't be honoured — but it must
        // still come through rather than erroring.
        let src = dds(b"DXT1", 256, 256, 1, RmipFormat::Bc1RgbaUnormSrgb);
        let out = transcode(&src, true, 64).expect("transcode");
        assert_eq!((out.width, out.height, out.mips), (256, 256, 1));
        assert_eq!(out.levels_dropped, 0);
    }

    #[test]
    fn a_lying_mip_count_is_truncated_to_what_is_there() {
        // Claim 9 levels, supply 3. Trusting the header would read past the
        // end of the file.
        let mut src = dds(b"DXT1", 256, 256, 3, RmipFormat::Bc1RgbaUnormSrgb);
        src[4 + 24..4 + 28].copy_from_slice(&9u32.to_le_bytes());
        let out = transcode(&src, true, 0).expect("transcode");
        assert_eq!(out.mips, 3);
    }

    #[test]
    fn size_at_matches_what_transcode_actually_writes() {
        // The budget maths must agree with the repack byte-for-byte, or a
        // caller picking a cap from `size_at` gets a different answer than it
        // ends up writing.
        let src = dds(b"DXT5", 256, 256, 9, RmipFormat::Bc3RgbaUnormSrgb);
        for cap in [0, 1024, 256, 64, 16] {
            let described = probe(&src, true).unwrap().size_at(cap);
            let written = transcode(&src, true, cap).unwrap().bytes.len() - crate::HEADER_LEN;
            assert_eq!(described, written, "cap {cap}");
        }
    }

    #[test]
    fn clamp_rewrites_the_dds_header_and_keeps_it_parseable() {
        let src = dds(b"DXT5", 256, 256, 9, RmipFormat::Bc3RgbaUnormSrgb);
        let clamped = clamp(&src, 64).expect("clamp");

        // Still a DDS, and one that describes itself correctly.
        let described = probe(&clamped, true).expect("re-parse");
        assert_eq!((described.width, described.height), (64, 64));
        assert_eq!(described.mips, 7);

        // Same surviving block data as the `.rmip` route.
        let via_rmip = transcode(&src, true, 64).unwrap();
        assert_eq!(
            &clamped[4 + DDS_HEADER_LEN..],
            &via_rmip.bytes[crate::HEADER_LEN..]
        );
    }

    #[test]
    fn clamp_is_a_passthrough_when_nothing_needs_dropping() {
        let src = dds(b"DXT1", 64, 64, 7, RmipFormat::Bc1RgbaUnormSrgb);
        assert_eq!(clamp(&src, 2048).unwrap(), src);
        assert_eq!(clamp(&src, 0).unwrap(), src);
    }

    #[test]
    fn unsupported_inputs_are_reported_not_guessed() {
        // BC2 has no `.rmip` code; a cubemap can't be expressed at all.
        let bc2 = dds(b"DXT3", 16, 16, 1, RmipFormat::Bc3RgbaUnormSrgb);
        assert!(transcode(&bc2, true, 0).is_err());

        let mut cube = dds(b"DXT1", 16, 16, 1, RmipFormat::Bc1RgbaUnormSrgb);
        cube[4 + 108..4 + 112].copy_from_slice(&DDSCAPS2_CUBEMAP.to_le_bytes());
        assert!(transcode(&cube, true, 0).is_err());

        assert!(transcode(b"not a dds file at all", true, 0).is_err());
    }
}
