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

    // Fallback
    Unknown(u32),
}

impl Value {
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
            Value::Vec3dArray(v) => Some(v.iter().map(|d| [d[0] as f32, d[1] as f32, d[2] as f32]).collect()),
            _ => None,
        }
    }

    pub fn as_vec2f_array(&self) -> Option<Vec<[f32; 2]>> {
        match self {
            Value::Vec2fArray(v) => Some(v.clone()),
            Value::Vec2dArray(v) => Some(v.iter().map(|d| [d[0] as f32, d[1] as f32]).collect()),
            Value::HalfArray(v) => {
                // Pairs of f32 (already converted from half)
                let pairs: Vec<[f32; 2]> = v.chunks(2).filter(|c| c.len() == 2).map(|c| [c[0], c[1]]).collect();
                if pairs.is_empty() { None } else { Some(pairs) }
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

/// Known USDC value type IDs.
pub mod type_id {
    pub const BOOL: u32 = 1;
    pub const INT: u32 = 3;
    pub const INT64: u32 = 5;
    pub const HALF: u32 = 7;
    pub const FLOAT: u32 = 8;
    pub const DOUBLE: u32 = 9;
    pub const STRING: u32 = 13;
    pub const TOKEN: u32 = 14;
    pub const ASSET_PATH: u32 = 15;
    pub const MATRIX4D: u32 = 20;
    pub const QUATF: u32 = 21;
    pub const QUATD: u32 = 22;
    pub const VEC2F: u32 = 25;
    pub const VEC2D: u32 = 26;
    pub const VEC3F: u32 = 28;
    pub const VEC3D: u32 = 29;
    pub const VEC4F: u32 = 31;
    pub const VEC4D: u32 = 32;
    pub const PATH: u32 = 39;
    pub const PATH_LIST_OP: u32 = 40;
    pub const SPECIFIER: u32 = 36;
    pub const TOKEN_LIST_OP: u32 = 42;

    // Array flag -- OR'd with the base type
    pub const ARRAY_BIT: u32 = 1 << 31;

    pub fn is_array(type_id: u32) -> bool {
        type_id & ARRAY_BIT != 0
    }

    pub fn base_type(type_id: u32) -> u32 {
        type_id & !ARRAY_BIT
    }
}
