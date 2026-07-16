#define_import_path bevy_gaussian_splatting::spherical_harmonics

const shc = array<f32, 16>(
    0.28209479177387814,
    -0.4886025119029199,
    0.4886025119029199,
    -0.4886025119029199,
    1.0925484305920792,
    -1.0925484305920792,
    0.31539156525252005,
    -1.0925484305920792,
    0.5462742152960396,
    -0.5900435899266435,
    2.890611442640554,
    -0.4570457994644658,
    0.3731763325901154,
    -0.4570457994644658,
    1.445305721320277,
    -0.5900435899266435,
);

fn srgb_to_linear(srgb_color: vec3<f32>) -> vec3<f32> {
    var linear_color: vec3<f32>;
    for (var i = 0u; i < 3u; i = i + 1u) {
        if (srgb_color[i] <= 0.04045) {
            linear_color[i] = srgb_color[i] / 12.92;
        } else {
            linear_color[i] = pow((srgb_color[i] + 0.055) / 1.055, 2.4);
        }
    }
    return linear_color;
}

fn spherical_harmonics_lookup(
    ray_direction: vec3<f32>,
    sh: array<f32, #{SH_COEFF_COUNT}>,
) -> vec3<f32> {
    let rds = ray_direction * ray_direction;
    var color = vec3<f32>(0.5);

    color += shc[ 0] * vec3<f32>(sh[0], sh[1], sh[2]);

#if SH_COEFF_COUNT > 11
    color += shc[ 1] * vec3<f32>(sh[ 3], sh[ 4], sh[ 5]) * ray_direction.y;
    color += shc[ 2] * vec3<f32>(sh[ 6], sh[ 7], sh[ 8]) * ray_direction.z;
    color += shc[ 3] * vec3<f32>(sh[ 9], sh[10], sh[11]) * ray_direction.x;
#endif

#if SH_COEFF_COUNT > 26
    color += shc[ 4] * vec3<f32>(sh[12], sh[13], sh[14]) * ray_direction.x * ray_direction.y;
    color += shc[ 5] * vec3<f32>(sh[15], sh[16], sh[17]) * ray_direction.y * ray_direction.z;
    color += shc[ 6] * vec3<f32>(sh[18], sh[19], sh[20]) * (2.0 * rds.z - rds.x - rds.y);
    color += shc[ 7] * vec3<f32>(sh[21], sh[22], sh[23]) * ray_direction.x * ray_direction.z;
    color += shc[ 8] * vec3<f32>(sh[24], sh[25], sh[26]) * (rds.x - rds.y);
#endif

#if SH_COEFF_COUNT > 47
    color += shc[ 9] * vec3<f32>(sh[27], sh[28], sh[29]) * ray_direction.y * (3.0 * rds.x - rds.y);
    color += shc[10] * vec3<f32>(sh[30], sh[31], sh[32]) * ray_direction.x * ray_direction.y * ray_direction.z;
    color += shc[11] * vec3<f32>(sh[33], sh[34], sh[35]) * ray_direction.y * (4.0 * rds.z - rds.x - rds.y);
    color += shc[12] * vec3<f32>(sh[36], sh[37], sh[38]) * ray_direction.z * (2.0 * rds.z - 3.0 * rds.x - 3.0 * rds.y);
    color += shc[13] * vec3<f32>(sh[39], sh[40], sh[41]) * ray_direction.x * (4.0 * rds.z - rds.x - rds.y);
    color += shc[14] * vec3<f32>(sh[42], sh[43], sh[44]) * ray_direction.z * (rds.x - rds.y);
    color += shc[15] * vec3<f32>(sh[45], sh[46], sh[47]) * ray_direction.x * (rds.x - 3.0 * rds.y);
#endif

    return color;
}
