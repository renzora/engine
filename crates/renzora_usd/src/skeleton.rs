//! Skeleton processing utilities.

use crate::scene::UsdSkeleton;

/// Compute inverse bind matrices from bind transforms.
/// Bind transforms are world-space; inverse bind matrices are used for skinning.
pub fn compute_inverse_bind_matrices(skeleton: &UsdSkeleton) -> Vec<[f32; 16]> {
    skeleton
        .bind_transforms
        .iter()
        .map(|m| invert_4x4(m))
        .collect()
}

/// Invert a column-major 4x4 matrix.
fn invert_4x4(m: &[f32; 16]) -> [f32; 16] {
    // Cofactor expansion for 4x4 matrix inverse
    let a = m[0] * m[5] - m[1] * m[4];
    let b = m[0] * m[6] - m[2] * m[4];
    let c = m[0] * m[7] - m[3] * m[4];
    let d = m[1] * m[6] - m[2] * m[5];
    let e = m[1] * m[7] - m[3] * m[5];
    let f = m[2] * m[7] - m[3] * m[6];
    let g = m[8] * m[13] - m[9] * m[12];
    let h = m[8] * m[14] - m[10] * m[12];
    let i = m[8] * m[15] - m[11] * m[12];
    let j = m[9] * m[14] - m[10] * m[13];
    let k = m[9] * m[15] - m[11] * m[13];
    let l = m[10] * m[15] - m[11] * m[14];

    let det = a * l - b * k + c * j + d * i - e * h + f * g;
    if det.abs() < 1e-12 {
        return identity();
    }
    let inv_det = 1.0 / det;

    [
        ( m[5] * l - m[6] * k + m[7] * j) * inv_det,
        (-m[1] * l + m[2] * k - m[3] * j) * inv_det,
        ( m[13] * f - m[14] * e + m[15] * d) * inv_det,
        (-m[9] * f + m[10] * e - m[11] * d) * inv_det,
        (-m[4] * l + m[6] * i - m[7] * h) * inv_det,
        ( m[0] * l - m[2] * i + m[3] * h) * inv_det,
        (-m[12] * f + m[14] * c - m[15] * b) * inv_det,
        ( m[8] * f - m[10] * c + m[11] * b) * inv_det,
        ( m[4] * k - m[5] * i + m[7] * g) * inv_det,
        (-m[0] * k + m[1] * i - m[3] * g) * inv_det,
        ( m[12] * e - m[13] * c + m[15] * a) * inv_det,
        (-m[8] * e + m[9] * c - m[11] * a) * inv_det,
        (-m[4] * j + m[5] * h - m[6] * g) * inv_det,
        ( m[0] * j - m[1] * h + m[2] * g) * inv_det,
        (-m[12] * d + m[13] * b - m[14] * a) * inv_det,
        ( m[8] * d - m[9] * b + m[10] * a) * inv_det,
    ]
}

fn identity() -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ]
}
