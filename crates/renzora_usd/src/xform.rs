//! USD transform (Xform) processing.
//!
//! USD transforms are defined by an ordered list of xformOps.
//! This module collapses them into a single 4x4 matrix.

use crate::crate_format::Value;

/// Identity matrix (column-major).
pub fn identity() -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ]
}

/// Extract a combined transform from a set of fields.
///
/// Looks for `xformOp:transform` (full matrix) or individual
/// `xformOp:translate`, `xformOp:rotateXYZ`, `xformOp:scale` ops.
pub fn extract_transform(fields: &[(String, Value)]) -> Option<[f32; 16]> {
    // Check for a direct 4x4 matrix first
    for (key, value) in fields.iter() {
        let key: &str = key.as_str();
        let value: &Value = value;
        if key == "xformOp:transform" {
            if let Value::Matrix4d(m) = value {
                let mut out = [0.0f32; 16];
                for i in 0..16 {
                    out[i] = m[i] as f32;
                }
                return Some(out);
            }
        }
    }

    // Otherwise compose from individual ops
    let mut has_any = false;
    let mut translate = [0.0f32; 3];
    let mut rotate_xyz = [0.0f32; 3]; // degrees
    let mut scale = [1.0f32; 3];

    for (key, value) in fields.iter() {
        let key: &str = key.as_str();
        match key {
            "xformOp:translate" => {
                if let Some(v) = value.as_vec3f() {
                    translate = v;
                    has_any = true;
                } else if let Value::Vec3d(v) = value {
                    translate = [v[0] as f32, v[1] as f32, v[2] as f32];
                    has_any = true;
                }
            }
            "xformOp:rotateXYZ" | "xformOp:rotateX" | "xformOp:rotateY" | "xformOp:rotateZ" => {
                if let Some(v) = value.as_vec3f() {
                    rotate_xyz = v;
                    has_any = true;
                } else if let Some(v) = value.as_float() {
                    match key {
                        "xformOp:rotateX" => rotate_xyz[0] = v,
                        "xformOp:rotateY" => rotate_xyz[1] = v,
                        "xformOp:rotateZ" => rotate_xyz[2] = v,
                        _ => {}
                    }
                    has_any = true;
                }
            }
            "xformOp:scale" => {
                if let Some(v) = value.as_vec3f() {
                    scale = v;
                    has_any = true;
                }
            }
            _ => {}
        }
    }

    if !has_any {
        return None;
    }

    Some(compose_trs(&translate, &rotate_xyz, &scale))
}

/// Compose a TRS (translate, rotate XYZ in degrees, scale) into a column-major 4x4 matrix.
fn compose_trs(t: &[f32; 3], r_deg: &[f32; 3], s: &[f32; 3]) -> [f32; 16] {
    let rx = r_deg[0].to_radians();
    let ry = r_deg[1].to_radians();
    let rz = r_deg[2].to_radians();

    let (sx, cx) = (rx.sin(), rx.cos());
    let (sy, cy) = (ry.sin(), ry.cos());
    let (sz, cz) = (rz.sin(), rz.cos());

    // R = Rz * Ry * Rx (extrinsic XYZ rotation)
    let r00 = cy * cz;
    let r01 = sx * sy * cz - cx * sz;
    let r02 = cx * sy * cz + sx * sz;
    let r10 = cy * sz;
    let r11 = sx * sy * sz + cx * cz;
    let r12 = cx * sy * sz - sx * cz;
    let r20 = -sy;
    let r21 = sx * cy;
    let r22 = cx * cy;

    // Column-major: column 0 = [r00, r10, r20, 0], etc.
    [
        r00 * s[0], r10 * s[0], r20 * s[0], 0.0,
        r01 * s[1], r11 * s[1], r21 * s[1], 0.0,
        r02 * s[2], r12 * s[2], r22 * s[2], 0.0,
        t[0],       t[1],       t[2],       1.0,
    ]
}
