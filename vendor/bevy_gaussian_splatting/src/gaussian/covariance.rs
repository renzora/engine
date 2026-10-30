use bevy::math::{Mat3, Vec3, Vec4};

#[allow(non_snake_case)]
pub fn compute_covariance_3d(rotation: Vec4, scale: Vec3) -> [f32; 6] {
    let S = Mat3::from_diagonal(scale);

    let r = rotation.x;
    let x = rotation.y;
    let y = rotation.z;
    let z = rotation.w;

    let R = Mat3::from_cols(
        Vec3::new(
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y - r * z),
            2.0 * (x * z + r * y),
        ),
        Vec3::new(
            2.0 * (x * y + r * z),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z - r * x),
        ),
        Vec3::new(
            2.0 * (x * z - r * y),
            2.0 * (y * z + r * x),
            1.0 - 2.0 * (x * x + y * y),
        ),
    );

    let M = S * R;
    let Sigma = M.transpose() * M;

    [
        Sigma.row(0).x,
        Sigma.row(0).y,
        Sigma.row(0).z,
        Sigma.row(1).y,
        Sigma.row(1).z,
        Sigma.row(2).z,
    ]
}
