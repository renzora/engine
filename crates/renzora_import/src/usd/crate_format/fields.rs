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
use super::values::{Value, type_id};

#[derive(Debug, Clone)]
pub struct Field {
    pub token_index: u32,
    pub value: Value,
}

pub fn read_fields(
    data: &[u8],
    toc: &TableOfContents,
    tokens: &[String],
) -> UsdResult<Vec<Field>> {
    let section = match toc.find(SECTION_FIELDS) {
        Some(s) => s,
        None => return Ok(Vec::new()),
    };

    let sec_start = section.offset as usize;
    let sec_end = sec_start + section.size as usize;
    if sec_end > data.len() {
        return Err(UsdError::Parse("FIELDS section extends beyond file".into()));
    }
    let sec_data = &data[sec_start..sec_end];

    if sec_data.len() < 8 {
        return Ok(Vec::new());
    }

    let num_fields = u64::from_le_bytes(sec_data[0..8].try_into().unwrap()) as usize;
    let mut pos = 8usize;

    if num_fields == 0 {
        return Ok(Vec::new());
    }

    // Token indices: integer-coded (delta + 2-bit + LZ4)
    let token_indices = compression::read_compressed_ints_with_count(sec_data, &mut pos, num_fields)?;

    // Value reps: raw LZ4 compressed u64 array (NOT integer coded)
    if pos + 8 > sec_data.len() {
        return Err(UsdError::Parse("FIELDS: value reps size truncated".into()));
    }
    let reps_comp_size = u64::from_le_bytes(sec_data[pos..pos + 8].try_into().unwrap()) as usize;
    pos += 8;

    let value_reps = if reps_comp_size > 0 && pos + reps_comp_size <= sec_data.len() {
        let compressed = &sec_data[pos..pos + reps_comp_size];
        let raw_size = num_fields * 8;
        let decompressed = compression::decompress_lz4_raw(compressed, raw_size)?;

        (0..num_fields)
            .map(|i| {
                let off = i * 8;
                if off + 8 <= decompressed.len() {
                    u64::from_le_bytes(decompressed[off..off + 8].try_into().unwrap())
                } else {
                    0
                }
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
        fields.push(Field { token_index: tok_idx, value });
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
        return Err(UsdError::Parse("FIELDSETS section extends beyond file".into()));
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

fn decode_value_rep(rep: u64, file_data: &[u8], tokens: &[String]) -> Value {
    let type_enum = (rep & 0xFF) as u32;
    let is_inlined = (rep >> 8) & 1 != 0;
    let is_array = (rep >> 9) & 1 != 0;
    let payload = rep >> 10;

    if is_inlined {
        return decode_inline_value(type_enum, is_array, payload, tokens);
    }

    let offset = payload as usize;
    if is_array {
        decode_out_of_line_array(type_enum, offset, file_data, tokens)
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
        type_id::STRING => {
            Value::String(tokens.get(payload as usize).cloned().unwrap_or_default())
        }
        type_id::ASSET_PATH => {
            Value::AssetPath(tokens.get(payload as usize).cloned().unwrap_or_default())
        }
        type_id::PATH | type_id::PATH_LIST_OP => {
            Value::Path(tokens.get(payload as usize).cloned().unwrap_or_default())
        }
        type_id::TOKEN_LIST_OP => {
            Value::TokenArray(vec![tokens.get(payload as usize).cloned().unwrap_or_default()])
        }
        _ => Value::Unknown(type_enum),
    }
}

fn decode_out_of_line_scalar(type_enum: u32, offset: usize, data: &[u8], tokens: &[String]) -> Value {
    match type_enum {
        type_id::INT => rv::<i32>(data, offset).map(Value::Int).unwrap_or(Value::Unknown(type_enum)),
        type_id::INT64 => rv::<i64>(data, offset).map(Value::Int64).unwrap_or(Value::Unknown(type_enum)),
        type_id::FLOAT => rv::<f32>(data, offset).map(Value::Float).unwrap_or(Value::Unknown(type_enum)),
        type_id::DOUBLE => rv::<f64>(data, offset).map(Value::Double).unwrap_or(Value::Unknown(type_enum)),
        type_id::TOKEN | type_id::SPECIFIER => {
            rv::<u32>(data, offset)
                .map(|i| Value::Token(tokens.get(i as usize).cloned().unwrap_or_default()))
                .unwrap_or(Value::Unknown(type_enum))
        }
        type_id::STRING => {
            rv::<u32>(data, offset)
                .map(|i| Value::String(tokens.get(i as usize).cloned().unwrap_or_default()))
                .unwrap_or(Value::Unknown(type_enum))
        }
        type_id::VEC2F => rv2f(data, offset).map(Value::Vec2f).unwrap_or(Value::Unknown(type_enum)),
        type_id::VEC3F => rv3f(data, offset).map(Value::Vec3f).unwrap_or(Value::Unknown(type_enum)),
        type_id::VEC3D => rv3d(data, offset).map(Value::Vec3d).unwrap_or(Value::Unknown(type_enum)),
        type_id::MATRIX4D => rmat(data, offset).map(Value::Matrix4d).unwrap_or(Value::Unknown(type_enum)),
        type_id::PATH | type_id::PATH_LIST_OP => {
            rv::<u32>(data, offset)
                .map(|i| Value::Path(tokens.get(i as usize).cloned().unwrap_or_default()))
                .unwrap_or(Value::Unknown(type_enum))
        }
        type_id::TOKEN_LIST_OP => decode_out_of_line_array(type_id::TOKEN, offset, data, tokens),
        _ => Value::Unknown(type_enum),
    }
}

fn decode_out_of_line_array(type_enum: u32, offset: usize, data: &[u8], tokens: &[String]) -> Value {
    if offset + 8 > data.len() { return Value::Unknown(type_enum); }
    let count = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap()) as usize;
    let s = offset + 8;
    if count > 100_000_000 { return Value::Unknown(type_enum); }

    match type_enum {
        type_id::INT => Value::IntArray(ra::<i32>(data, s, count)),
        type_id::FLOAT => Value::FloatArray(ra::<f32>(data, s, count)),
        type_id::DOUBLE => Value::DoubleArray(ra::<f64>(data, s, count)),
        type_id::VEC2F => {
            let f = ra::<f32>(data, s, count * 2);
            Value::Vec2fArray(f.chunks(2).filter(|c| c.len() == 2).map(|c| [c[0], c[1]]).collect())
        }
        type_id::VEC3F => {
            let f = ra::<f32>(data, s, count * 3);
            Value::Vec3fArray(f.chunks(3).filter(|c| c.len() == 3).map(|c| [c[0], c[1], c[2]]).collect())
        }
        type_id::VEC3D => {
            let d = ra::<f64>(data, s, count * 3);
            Value::Vec3dArray(d.chunks(3).filter(|c| c.len() == 3).map(|c| [c[0], c[1], c[2]]).collect())
        }
        type_id::VEC4F | type_id::QUATF => {
            let f = ra::<f32>(data, s, count * 4);
            Value::Vec4fArray(f.chunks(4).filter(|c| c.len() == 4).map(|c| [c[0], c[1], c[2], c[3]]).collect())
        }
        type_id::MATRIX4D => {
            let d = ra::<f64>(data, s, count * 16);
            Value::Matrix4dArray(d.chunks(16).filter(|c| c.len() == 16).map(|c| { let mut m = [0.0f64; 16]; m.copy_from_slice(c); m }).collect())
        }
        type_id::TOKEN => {
            let idx = ra::<u32>(data, s, count);
            Value::TokenArray(idx.iter().map(|&i| tokens.get(i as usize).cloned().unwrap_or_default()).collect())
        }
        type_id::STRING => {
            let idx = ra::<u32>(data, s, count);
            Value::StringArray(idx.iter().map(|&i| tokens.get(i as usize).cloned().unwrap_or_default()).collect())
        }
        type_id::PATH | type_id::PATH_LIST_OP => {
            let idx = ra::<u32>(data, s, count);
            Value::PathArray(idx.iter().map(|&i| tokens.get(i as usize).cloned().unwrap_or_default()).collect())
        }
        type_id::HALF => {
            let h = ra::<u16>(data, s, count);
            Value::HalfArray(h.iter().map(|&b| half::f16::from_bits(b).to_f32()).collect())
        }
        _ => Value::Unknown(type_enum),
    }
}

// Helpers
fn rv<T: LeRead>(d: &[u8], o: usize) -> Option<T> { T::at(d, o) }
fn ra<T: LeRead>(d: &[u8], o: usize, n: usize) -> Vec<T> {
    let sz = std::mem::size_of::<T>();
    (0..n).filter_map(|i| T::at(d, o + i * sz)).collect()
}
fn rv2f(d: &[u8], o: usize) -> Option<[f32; 2]> { Some([f32::at(d, o)?, f32::at(d, o+4)?]) }
fn rv3f(d: &[u8], o: usize) -> Option<[f32; 3]> { Some([f32::at(d, o)?, f32::at(d, o+4)?, f32::at(d, o+8)?]) }
fn rv3d(d: &[u8], o: usize) -> Option<[f64; 3]> { Some([f64::at(d, o)?, f64::at(d, o+8)?, f64::at(d, o+16)?]) }
fn rmat(d: &[u8], o: usize) -> Option<[f64; 16]> { let mut m = [0.0; 16]; for i in 0..16 { m[i] = f64::at(d, o+i*8)?; } Some(m) }

trait LeRead: Sized { fn at(data: &[u8], offset: usize) -> Option<Self>; }
macro_rules! impl_le { ($t:ty, $n:expr) => { impl LeRead for $t {
    fn at(d: &[u8], o: usize) -> Option<Self> {
        if o + $n <= d.len() { Some(<$t>::from_le_bytes(d[o..o+$n].try_into().unwrap())) } else { None }
    }
}}}
impl_le!(u16, 2); impl_le!(u32, 4); impl_le!(i32, 4); impl_le!(i64, 8); impl_le!(f32, 4); impl_le!(f64, 8);
