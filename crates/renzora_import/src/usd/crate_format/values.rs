#![allow(dead_code)] // USD Crate format reader — partial implementation, helpers staged.

//! USDC value types and decoding.
//!
//! USD supports ~50 value types. We implement the subset needed for
//! mesh/material/skeleton/animation/light/camera import.

/// A decoded USD value.
#[derive(Debug, Clone)]
pub enum Value {
    Bool(bool),
    Int(i32),
    Int64(i64),
    Float(f32),
    Double(f64),
    Half(f32), // stored as f32 after conversion
    String(String),
    Token(String),
    AssetPath(String),

    // Vector types
    Vec2f([f32; 2]),
    Vec3f([f32; 3]),
    Vec4f([f32; 4]),
    Vec2d([f64; 2]),
    Vec3d([f64; 3]),
    Vec4d([f64; 4]),

    // Matrix types
    Matrix4d([f64; 16]),

    // Quaternion
    Quatf([f32; 4]),
    Quatd([f64; 4]),

    // Array types
    IntArray(Vec<i32>),
    FloatArray(Vec<f32>),
    DoubleArray(Vec<f64>),
    Vec2fArray(Vec<[f32; 2]>),
    Vec3fArray(Vec<[f32; 3]>),
    Vec4fArray(Vec<[f32; 4]>),
    Vec2dArray(Vec<[f64; 2]>),
    Vec3dArray(Vec<[f64; 3]>),
    Matrix4dArray(Vec<[f64; 16]>),
    QuatfArray(Vec<[f32; 4]>),
    TokenArray(Vec<String>),
    StringArray(Vec<String>),
    PathArray(Vec<String>),
    HalfArray(Vec<f32>),

    // Path / relationship target
    Path(String),
    /// Indices into the crate's **path** table, as a relationship's targets are
    /// stored. Kept unresolved because the value decoder has the token table but
    /// not the path table — resolving them against `tokens` (which is what used
    /// to happen) yields an unrelated prim name, so `material:binding` pointed
    /// at nothing and every mesh rendered untextured.
    PathIndices(Vec<u32>),

    // Fallback
    Unknown(u32),
}

impl Value {
    /// Like [`Self::kind_name`] but naming the raw type enum for `Unknown`,
    /// which is what you need to map a file's numbering onto the constants.
    pub fn debug_kind(&self) -> String {
        match self {
            Value::Unknown(t) => format!("Unknown({t})"),
            other => other.kind_name().to_string(),
        }
    }

    /// Variant name, for diagnostics — enough to tell "the field is there but
    /// decoded as the wrong type" from "the field is missing".
    pub fn kind_name(&self) -> &'static str {
        match self {
            Value::Bool(_) => "Bool",
            Value::Int(_) => "Int",
            Value::Int64(_) => "Int64",
            Value::Float(_) => "Float",
            Value::Double(_) => "Double",
            Value::Half(_) => "Half",
            Value::String(_) => "String",
            Value::Token(_) => "Token",
            Value::AssetPath(_) => "AssetPath",
            _ => "Other",
        }
    }

    // Accessor helpers for common conversions

    pub fn as_token(&self) -> Option<&str> {
        match self {
            Value::Token(s) | Value::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i32> {
        match self {
            Value::Int(v) => Some(*v),
            Value::Int64(v) => Some(*v as i32),
            Value::Float(v) => Some(*v as i32),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f32> {
        match self {
            Value::Float(v) | Value::Half(v) => Some(*v),
            Value::Double(v) => Some(*v as f32),
            Value::Int(v) => Some(*v as f32),
            _ => None,
        }
    }

    pub fn as_vec2f(&self) -> Option<[f32; 2]> {
        match self {
            Value::Vec2f(v) => Some(*v),
            Value::Vec2d(v) => Some([v[0] as f32, v[1] as f32]),
            _ => None,
        }
    }

    pub fn as_vec3f(&self) -> Option<[f32; 3]> {
        match self {
            Value::Vec3f(v) => Some(*v),
            Value::Vec3d(v) => Some([v[0] as f32, v[1] as f32, v[2] as f32]),
            _ => None,
        }
    }

    pub fn as_vec3f_array(&self) -> Option<Vec<[f32; 3]>> {
        match self {
            Value::Vec3fArray(v) => Some(v.clone()),
            Value::Vec3dArray(v) => Some(
                v.iter()
                    .map(|d| [d[0] as f32, d[1] as f32, d[2] as f32])
                    .collect(),
            ),
            _ => None,
        }
    }

    pub fn as_vec2f_array(&self) -> Option<Vec<[f32; 2]>> {
        match self {
            Value::Vec2fArray(v) => Some(v.clone()),
            Value::Vec2dArray(v) => Some(v.iter().map(|d| [d[0] as f32, d[1] as f32]).collect()),
            Value::HalfArray(v) => {
                // Pairs of f32 (already converted from half)
                let pairs: Vec<[f32; 2]> = v
                    .chunks(2)
                    .filter(|c| c.len() == 2)
                    .map(|c| [c[0], c[1]])
                    .collect();
                if pairs.is_empty() {
                    None
                } else {
                    Some(pairs)
                }
            }
            _ => None,
        }
    }

    pub fn as_int_array(&self) -> Option<Vec<i32>> {
        match self {
            Value::IntArray(v) => Some(v.clone()),
            _ => None,
        }
    }

    pub fn as_float_array(&self) -> Option<Vec<f32>> {
        match self {
            Value::FloatArray(v) => Some(v.clone()),
            Value::DoubleArray(v) => Some(v.iter().map(|&d| d as f32).collect()),
            Value::HalfArray(v) => Some(v.clone()),
            _ => None,
        }
    }

    pub fn as_token_array(&self) -> Option<Vec<String>> {
        match self {
            Value::TokenArray(v) | Value::StringArray(v) => Some(v.clone()),
            _ => None,
        }
    }

    pub fn as_matrix4d_array(&self) -> Option<Vec<[f32; 16]>> {
        match self {
            Value::Matrix4dArray(v) => Some(
                v.iter()
                    .map(|m| {
                        let mut out = [0.0f32; 16];
                        for i in 0..16 {
                            out[i] = m[i] as f32;
                        }
                        out
                    })
                    .collect(),
            ),
            _ => None,
        }
    }

    pub fn as_path_or_token(&self) -> Option<String> {
        match self {
            Value::Path(s) | Value::Token(s) | Value::String(s) | Value::AssetPath(s) => {
                Some(s.clone())
            }
            Value::PathArray(v) => v.first().cloned(),
            Value::TokenArray(v) => v.first().cloned(),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// USDC value type IDs (from OpenUSD's SdfValueTypeNames)
// ---------------------------------------------------------------------------

pub mod type_id {
    //! Known USDC value type IDs — `TypeEnum` from Pixar's `crateDataTypes.h`.
    //!
    //! These are the values USD actually writes, confirmed against a real file:
    //! a prim's `typeName` and `kind` fields both report 11, which is `Token`.
    //! The previous table was offset from about `String` onwards (it had
    //! `Token = 14`), so every token-valued field decoded as `Unknown` — and a
    //! stage whose structure had parsed perfectly reported no meshes, because
    //! nothing could be identified as one.
    //!
    //! Array-ness is *not* encoded here: it is a separate bit in the
    //! `ValueRep`, so an array of `Vec3f` is still type 24.
    pub const INVALID: u32 = 0;
    pub const BOOL: u32 = 1;
    pub const UCHAR: u32 = 2;
    pub const INT: u32 = 3;
    pub const UINT: u32 = 4;
    pub const INT64: u32 = 5;
    pub const UINT64: u32 = 6;
    pub const HALF: u32 = 7;
    pub const FLOAT: u32 = 8;
    pub const DOUBLE: u32 = 9;
    pub const STRING: u32 = 10;
    pub const TOKEN: u32 = 11;
    pub const ASSET_PATH: u32 = 12;
    pub const MATRIX2D: u32 = 13;
    pub const MATRIX3D: u32 = 14;
    pub const MATRIX4D: u32 = 15;
    pub const QUATD: u32 = 16;
    pub const QUATF: u32 = 17;
    pub const QUATH: u32 = 18;
    pub const VEC2D: u32 = 19;
    pub const VEC2F: u32 = 20;
    pub const VEC2H: u32 = 21;
    pub const VEC2I: u32 = 22;
    pub const VEC3D: u32 = 23;
    pub const VEC3F: u32 = 24;
    pub const VEC3H: u32 = 25;
    pub const VEC3I: u32 = 26;
    pub const VEC4D: u32 = 27;
    pub const VEC4F: u32 = 28;
    pub const VEC4H: u32 = 29;
    pub const VEC4I: u32 = 30;
    pub const DICTIONARY: u32 = 31;
    pub const TOKEN_LIST_OP: u32 = 32;
    pub const STRING_LIST_OP: u32 = 33;
    pub const PATH_LIST_OP: u32 = 34;
    pub const REFERENCE_LIST_OP: u32 = 35;
    pub const INT_LIST_OP: u32 = 36;
    pub const INT64_LIST_OP: u32 = 37;
    pub const UINT_LIST_OP: u32 = 38;
    pub const UINT64_LIST_OP: u32 = 39;
    pub const PAYLOAD_LIST_OP: u32 = 40;
    // Observed directly in a real file: `primChildren` and `properties` are
    // both `TfTokenVector` and report 41; `specifier` reports 42. The ordering
    // below follows from those two anchors.
    pub const TOKEN_VECTOR: u32 = 41;
    pub const SPECIFIER: u32 = 42;
    pub const PERMISSION: u32 = 43;
    pub const VARIABILITY: u32 = 44;
    pub const VARIANT_SELECTION_MAP: u32 = 45;
    pub const TIME_SAMPLES: u32 = 46;
    pub const PAYLOAD: u32 = 47;
    pub const DOUBLE_VECTOR: u32 = 48;
    pub const LAYER_OFFSET_VECTOR: u32 = 49;
    pub const STRING_VECTOR: u32 = 50;
    pub const VALUE_BLOCK: u32 = 51;
    pub const VALUE: u32 = 52;
    pub const UNREGISTERED_VALUE: u32 = 53;
    pub const UNREGISTERED_VALUE_LIST_OP: u32 = 54;
    pub const PATH_VECTOR: u32 = 55;
    pub const TIME_CODE: u32 = 56;

    /// Kept for callers that still pass a combined id; array-ness now travels
    /// in the `ValueRep` bit instead.
    pub const ARRAY_BIT: u32 = 1 << 31;

    pub fn is_array(type_id: u32) -> bool {
        type_id & ARRAY_BIT != 0
    }

    pub fn base_type(type_id: u32) -> u32 {
        type_id & !ARRAY_BIT
    }
}
