#![allow(dead_code)] // USD Crate format reader — partial implementation, helpers staged.

//! USDC field and fieldset parsing.
//!
//! FIELDS section:
//!   u64: numFields
//!   [u64 compSize][compSize bytes]: integer-coded u32 token indices
//!   [u64 compSize][compSize bytes]: LZ4-compressed u64 value reps (raw LZ4, no integer coding)
//!
//! FIELDSETS section:
//!   u64: numFieldSets
//!   [u64 compSize][compSize bytes]: integer-coded u32 field indices (with 0-sentinel between sets)

use super::super::{UsdError, UsdResult};
use super::compression;
use super::sections::{TableOfContents, SECTION_FIELDS, SECTION_FIELDSETS};
use super::values::{type_id, Value};

#[derive(Debug, Clone)]
pub struct Field {
    pub token_index: u32,
    pub value: Value,
}

pub fn read_fields(data: &[u8], toc: &TableOfContents, tokens: &[String]) -> UsdResult<Vec<Field>> {
    let section = match toc.find(SECTION_FIELDS) {
        Some(s) => s,
        None => return Ok(Vec::new()),
    };

    // `offset + size`, both out of the file. See `sections::section_data` for
    // what writing that as a bare `+` used to do.
    let sec_start = section.offset as usize;
    let sec_data = super::read::slice(data, sec_start, section.size as usize)
        .ok_or_else(|| UsdError::Parse("FIELDS section extends beyond file".into()))?;

    if sec_data.len() < 8 {
        return Ok(Vec::new());
    }

    let num_fields = super::read::le_u64(sec_data, 0).unwrap_or(0) as usize;
    let mut pos = 8usize;

    if num_fields == 0 {
        return Ok(Vec::new());
    }

    // Token indices: integer-coded (delta + 2-bit + LZ4)
    let token_indices =
        compression::read_compressed_ints_with_count(sec_data, &mut pos, num_fields)?;

    // Value reps: raw LZ4 compressed u64 array (NOT integer coded)
    let reps_comp_size = super::read::le_u64(sec_data, pos)
        .ok_or_else(|| UsdError::Parse("FIELDS: value reps size truncated".into()))?
        as usize;
    pos += 8;

    // `num_fields * 8` is a file-derived count scaled by 8, so it overflows on
    // a large enough count and would ask the decompressor for a nonsense
    // buffer. A count that cannot be expressed is a corrupt one.
    let raw_size = num_fields
        .checked_mul(8)
        .ok_or_else(|| UsdError::Parse("FIELDS: implausible field count".into()))?;

    let value_reps = if let Some(compressed) =
        (reps_comp_size > 0).then(|| super::read::slice(sec_data, pos, reps_comp_size)).flatten()
    {
        let decompressed = compression::decompress_lz4_raw(compressed, raw_size)?;

        (0..num_fields)
            .map(|i| {
                // `i` is bounded by `num_fields`, whose × 8 is checked above, so
                // this cannot wrap; read through the helper regardless so a
                // short decompression is a zero rather than a panic.
                super::read::le_u64(&decompressed, i * 8).unwrap_or(0)
            })
            .collect()
    } else {
        vec![0u64; num_fields]
    };

    let mut fields = Vec::with_capacity(num_fields);
    for i in 0..num_fields {
        let tok_idx = token_indices.get(i).copied().unwrap_or(0);
        let value_rep = value_reps.get(i).copied().unwrap_or(0);
        let value = decode_value_rep(value_rep, data, tokens);
        fields.push(Field {
            token_index: tok_idx,
            value,
        });
    }

    log::debug!("Read {} fields", fields.len());
    Ok(fields)
}

pub fn read_field_sets(data: &[u8], toc: &TableOfContents) -> UsdResult<Vec<u32>> {
    let section = match toc.find(SECTION_FIELDSETS) {
        Some(s) => s,
        None => return Ok(Vec::new()),
    };

    let sec_start = section.offset as usize;
    let sec_end = sec_start + section.size as usize;
    if sec_end > data.len() {
        return Err(UsdError::Parse(
            "FIELDSETS section extends beyond file".into(),
        ));
    }
    let sec_data = &data[sec_start..sec_end];

    if sec_data.len() < 8 {
        return Ok(Vec::new());
    }

    // u64 count + integer-coded compressed blob
    let mut pos = 0usize;
    let field_sets = compression::read_compressed_ints(sec_data, &mut pos)?;

    log::debug!("Read {} fieldset entries", field_sets.len());
    Ok(field_sets)
}

// ---------------------------------------------------------------------------
// Value rep decoding
// ---------------------------------------------------------------------------

/// `ValueRep` bit layout, from Pixar's `crateFile.h`:
///
/// ```text
/// bit  63     isArray
/// bit  62     isInlined
/// bit  61     isCompressed
/// bits 48..55 type enum
/// bits 0..47  payload
/// ```
///
/// This previously read the type from the *low* byte and the payload from bit
/// 10 upwards, so every field decoded as `Unknown` — including `typeName`,
/// which is why a fully-parsed stage reported no meshes.
const REP_IS_ARRAY: u64 = 1 << 63;
const REP_IS_INLINED: u64 = 1 << 62;
const REP_IS_COMPRESSED: u64 = 1 << 61;
const REP_PAYLOAD_MASK: u64 = (1 << 48) - 1;

fn decode_value_rep(rep: u64, file_data: &[u8], tokens: &[String]) -> Value {
    let type_enum = ((rep >> 48) & 0xFF) as u32;
    let is_inlined = rep & REP_IS_INLINED != 0;
    let is_array = rep & REP_IS_ARRAY != 0;
    let is_compressed = rep & REP_IS_COMPRESSED != 0;
    let payload = rep & REP_PAYLOAD_MASK;

    if is_inlined {
        return decode_inline_value(type_enum, is_array, payload, tokens);
    }

    let offset = payload as usize;
    if is_array {
        decode_out_of_line_array(type_enum, offset, file_data, tokens, is_compressed)
    } else {
        decode_out_of_line_scalar(type_enum, offset, file_data, tokens)
    }
}

fn decode_inline_value(type_enum: u32, is_array: bool, payload: u64, tokens: &[String]) -> Value {
    if is_array {
        return match type_enum {
            type_id::INT => Value::IntArray(Vec::new()),
            type_id::FLOAT => Value::FloatArray(Vec::new()),
            type_id::VEC3F => Value::Vec3fArray(Vec::new()),
            type_id::TOKEN => Value::TokenArray(Vec::new()),
            _ => Value::Unknown(type_enum),
        };
    }

    match type_enum {
        type_id::BOOL => Value::Bool(payload != 0),
        type_id::INT => Value::Int(payload as i32),
        type_id::INT64 => Value::Int64(payload as i64),
        type_id::FLOAT => Value::Float(f32::from_bits(payload as u32)),
        type_id::DOUBLE => Value::Double(f64::from_bits(payload)),
        type_id::TOKEN | type_id::SPECIFIER => {
            Value::Token(tokens.get(payload as usize).cloned().unwrap_or_default())
        }
        type_id::STRING => Value::String(tokens.get(payload as usize).cloned().unwrap_or_default()),
        type_id::ASSET_PATH => {
            Value::AssetPath(tokens.get(payload as usize).cloned().unwrap_or_default())
        }
        type_id::PATH_VECTOR | type_id::PATH_LIST_OP => {
            Value::PathIndices(vec![payload as u32])
        }
        type_id::TOKEN_LIST_OP => Value::TokenArray(vec![tokens
            .get(payload as usize)
            .cloned()
            .unwrap_or_default()]),
        _ => Value::Unknown(type_enum),
    }
}

fn decode_out_of_line_scalar(
    type_enum: u32,
    offset: usize,
    data: &[u8],
    tokens: &[String],
) -> Value {
    match type_enum {
        type_id::INT => rv::<i32>(data, offset)
            .map(Value::Int)
            .unwrap_or(Value::Unknown(type_enum)),
        type_id::INT64 => rv::<i64>(data, offset)
            .map(Value::Int64)
            .unwrap_or(Value::Unknown(type_enum)),
        type_id::FLOAT => rv::<f32>(data, offset)
            .map(Value::Float)
            .unwrap_or(Value::Unknown(type_enum)),
        type_id::DOUBLE => rv::<f64>(data, offset)
            .map(Value::Double)
            .unwrap_or(Value::Unknown(type_enum)),
        type_id::TOKEN | type_id::SPECIFIER => rv::<u32>(data, offset)
            .map(|i| Value::Token(tokens.get(i as usize).cloned().unwrap_or_default()))
            .unwrap_or(Value::Unknown(type_enum)),
        type_id::STRING => rv::<u32>(data, offset)
            .map(|i| Value::String(tokens.get(i as usize).cloned().unwrap_or_default()))
            .unwrap_or(Value::Unknown(type_enum)),
        type_id::VEC2F => rv2f(data, offset)
            .map(Value::Vec2f)
            .unwrap_or(Value::Unknown(type_enum)),
        type_id::VEC3F => rv3f(data, offset)
            .map(Value::Vec3f)
            .unwrap_or(Value::Unknown(type_enum)),
        type_id::VEC3D => rv3d(data, offset)
            .map(Value::Vec3d)
            .unwrap_or(Value::Unknown(type_enum)),
        type_id::MATRIX4D => rmat(data, offset)
            .map(Value::Matrix4d)
            .unwrap_or(Value::Unknown(type_enum)),
        // A plain vector of paths: `[u64 count][count x u32]`.
        type_id::PATH_VECTOR => {
            let Some(count) = rv::<u64>(data, offset) else {
                return Value::Unknown(type_enum);
            };
            Value::PathIndices(ra::<u32>(data, offset + 8, count as usize))
        }
        // A list-op, which is a different shape entirely — see below.
        type_id::PATH_LIST_OP => decode_path_list_op(data, offset, type_enum),
        type_id::TOKEN_LIST_OP => {
            decode_out_of_line_array(type_id::TOKEN, offset, data, tokens, false)
        }
        _ => Value::Unknown(type_enum),
    }
}

/// Decode an `SdfPathListOp` — how USD stores a relationship's targets, and so
/// how `material:binding` names the material a mesh uses.
///
/// It is **not** a bare path index. Pixar writes a one-byte header of "which
/// sub-lists are present", then each present sub-list as `[u64 count][count x
/// u32 pathIndex]`, in the order explicit, added, prepended, appended, deleted,
/// ordered.
///
/// Reading the first four bytes as a `u32` — which is what used to happen —
/// picks up the header plus three bytes of the count. For the ordinary case of
/// one explicit target that is `03 01 00 00` = 259, a plausible-looking index
/// that resolves to an unrelated prim. And it is the *same* 259 for every mesh
/// in the file, which is exactly how this presented: 138 meshes all bound to
/// one arbitrary path, so none of them found a material and the model rendered
/// white while the material sphere — which reads the extracted material data
/// rather than the GLB — looked perfectly fine.
fn decode_path_list_op(data: &[u8], offset: usize, type_enum: u32) -> Value {
    const IS_EXPLICIT: u8 = 1 << 0;
    const HAS_EXPLICIT: u8 = 1 << 1;
    const HAS_ADDED: u8 = 1 << 2;
    const HAS_DELETED: u8 = 1 << 3;
    const HAS_ORDERED: u8 = 1 << 4;
    const HAS_PREPENDED: u8 = 1 << 5;
    const HAS_APPENDED: u8 = 1 << 6;

    let Some(&bits) = data.get(offset) else {
        return Value::Unknown(type_enum);
    };
    let _ = IS_EXPLICIT;
    let mut pos = offset + 1;

    // Sub-lists appear in write order, and every present one has to be walked
    // even if it is not the one we want, because they are laid out back to back.
    let mut targets: Vec<u32> = Vec::new();
    for (flag, is_target) in [
        (HAS_EXPLICIT, true),
        (HAS_ADDED, true),
        (HAS_PREPENDED, true),
        (HAS_APPENDED, true),
        (HAS_DELETED, false),
        (HAS_ORDERED, false),
    ] {
        if bits & flag == 0 {
            continue;
        }
        let Some(count) = rv::<u64>(data, pos) else {
            break;
        };
        pos += 8;
        let count = count as usize;
        if count > 10_000_000 || pos + count * 4 > data.len() {
            break;
        }
        let items = ra::<u32>(data, pos, count);
        pos += count * 4;
        // Deleted and ordered say what a target is *not*, or what order they go
        // in — neither adds a binding.
        if is_target && targets.is_empty() {
            targets = items;
        }
    }
    Value::PathIndices(targets)
}

fn decode_out_of_line_array(
    type_enum: u32,
    offset: usize,
    data: &[u8],
    tokens: &[String],
    is_compressed: bool,
) -> Value {
    let Some(count) = super::read::le_u64(data, offset) else {
        return Value::Unknown(type_enum);
    };
    let count = count as usize;
    // Sound because `le_u64` above only answered after its own `checked_add`.
    let s = offset + 8;
    if count > 100_000_000 {
        return Value::Unknown(type_enum);
    }

    // USD integer-compresses int arrays above a size threshold (delta transform,
    // 2-bit classification, then LZ4). Reading one of those as raw `i32` yields
    // the *encoded* buffer: `faceVertexIndices` came back full of negatives,
    // which read as enormous `u32` values and made every face fail its bounds
    // check. Only small arrays escaped, which is why a car imported as four
    // two-triangle scraps.
    if is_compressed {
        let mut pos = s;
        return match type_enum {
            type_id::INT | type_id::UINT => {
                match compression::read_compressed_ints_with_count(data, &mut pos, count) {
                    Ok(v) => Value::IntArray(v.into_iter().map(|i| i as i32).collect()),
                    Err(e) => {
                        log::debug!("USDC: compressed int array failed: {e:?}");
                        Value::Unknown(type_enum)
                    }
                }
            }
            // 64-bit arrays land in `IntArray` too: nothing downstream reads a
            // 64-bit index or count, and USD writes these as 32-bit deltas
            // regardless.
            type_id::INT64 | type_id::UINT64 => {
                match compression::read_compressed_ints_with_count(data, &mut pos, count) {
                    Ok(v) => Value::IntArray(v.into_iter().map(|i| i as i32).collect()),
                    Err(e) => {
                        log::debug!("USDC: compressed int64 array failed: {e:?}");
                        Value::Unknown(type_enum)
                    }
                }
            }
            // Only integer arrays are compressed; anything else reaching here
            // is a format the writer version handles differently.
            _ => Value::Unknown(type_enum),
        };
    }

    match type_enum {
        type_id::INT => Value::IntArray(ra::<i32>(data, s, count)),
        type_id::FLOAT => Value::FloatArray(ra::<f32>(data, s, count)),
        type_id::DOUBLE => Value::DoubleArray(ra::<f64>(data, s, count)),
        type_id::VEC2F => {
            let f = ra::<f32>(data, s, count * 2);
            Value::Vec2fArray(
                f.chunks(2)
                    .filter(|c| c.len() == 2)
                    .map(|c| [c[0], c[1]])
                    .collect(),
            )
        }
        type_id::VEC3F => {
            let f = ra::<f32>(data, s, count * 3);
            Value::Vec3fArray(
                f.chunks(3)
                    .filter(|c| c.len() == 3)
                    .map(|c| [c[0], c[1], c[2]])
                    .collect(),
            )
        }
        type_id::VEC3D => {
            let d = ra::<f64>(data, s, count * 3);
            Value::Vec3dArray(
                d.chunks(3)
                    .filter(|c| c.len() == 3)
                    .map(|c| [c[0], c[1], c[2]])
                    .collect(),
            )
        }
        type_id::VEC4F | type_id::QUATF => {
            let f = ra::<f32>(data, s, count * 4);
            Value::Vec4fArray(
                f.chunks(4)
                    .filter(|c| c.len() == 4)
                    .map(|c| [c[0], c[1], c[2], c[3]])
                    .collect(),
            )
        }
        type_id::MATRIX4D => {
            let d = ra::<f64>(data, s, count * 16);
            Value::Matrix4dArray(
                d.chunks(16)
                    .filter(|c| c.len() == 16)
                    .map(|c| {
                        let mut m = [0.0f64; 16];
                        m.copy_from_slice(c);
                        m
                    })
                    .collect(),
            )
        }
        type_id::TOKEN => {
            let idx = ra::<u32>(data, s, count);
            Value::TokenArray(
                idx.iter()
                    .map(|&i| tokens.get(i as usize).cloned().unwrap_or_default())
                    .collect(),
            )
        }
        type_id::STRING => {
            let idx = ra::<u32>(data, s, count);
            Value::StringArray(
                idx.iter()
                    .map(|&i| tokens.get(i as usize).cloned().unwrap_or_default())
                    .collect(),
            )
        }
        type_id::PATH_VECTOR | type_id::PATH_LIST_OP => {
            Value::PathIndices(ra::<u32>(data, s, count))
        }
        type_id::HALF => {
            let h = ra::<u16>(data, s, count);
            Value::HalfArray(
                h.iter()
                    .map(|&b| half::f16::from_bits(b).to_f32())
                    .collect(),
            )
        }
        _ => Value::Unknown(type_enum),
    }
}

// Helpers
fn rv<T: LeRead>(d: &[u8], o: usize) -> Option<T> {
    T::at(d, o)
}
fn ra<T: LeRead>(d: &[u8], o: usize, n: usize) -> Vec<T> {
    let sz = std::mem::size_of::<T>();
    (0..n).filter_map(|i| T::at(d, o + i * sz)).collect()
}
fn rv2f(d: &[u8], o: usize) -> Option<[f32; 2]> {
    Some([f32::at(d, o)?, f32::at(d, o + 4)?])
}
fn rv3f(d: &[u8], o: usize) -> Option<[f32; 3]> {
    Some([f32::at(d, o)?, f32::at(d, o + 4)?, f32::at(d, o + 8)?])
}
fn rv3d(d: &[u8], o: usize) -> Option<[f64; 3]> {
    Some([f64::at(d, o)?, f64::at(d, o + 8)?, f64::at(d, o + 16)?])
}
fn rmat(d: &[u8], o: usize) -> Option<[f64; 16]> {
    let mut m = [0.0; 16];
    for (i, slot) in m.iter_mut().enumerate() {
        *slot = f64::at(d, o + i * 8)?;
    }
    Some(m)
}

trait LeRead: Sized {
    fn at(data: &[u8], offset: usize) -> Option<Self>;
}
macro_rules! impl_le {
    ($t:ty, $n:expr) => {
        impl LeRead for $t {
            fn at(d: &[u8], o: usize) -> Option<Self> {
                // `o + $n` rather than `o.checked_add($n)` was the same wrap the
                // rest of this module carried: `o` walks a file-derived buffer,
                // and near the top of the range the sum came back small enough
                // to pass `<= d.len()` and then panic in the index.
                let end = o.checked_add($n)?;
                let bytes: [u8; $n] = d.get(o..end)?.try_into().ok()?;
                Some(<$t>::from_le_bytes(bytes))
            }
        }
    };
}
impl_le!(u16, 2);
impl_le!(u32, 4);
impl_le!(i32, 4);
impl_le!(i64, 8);
impl_le!(u64, 8);
impl_le!(f32, 4);
impl_le!(f64, 8);
