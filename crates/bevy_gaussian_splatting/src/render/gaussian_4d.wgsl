#define_import_path bevy_gaussian_splatting::gaussian_4d

#import bevy_gaussian_splatting::bindings::{
    view,
    globals,
    gaussian_uniforms,
}

#ifdef BUFFER_STORAGE
    #import bevy_gaussian_splatting::planar::{
        get_isotropic_rotations,
        get_scale,
        get_timestamp,
        get_time_scale,
    }
#endif

struct DecomposedGaussian4d {
    cov3d: array<f32, 6>,
    delta_mean: vec3<f32>,
    opacity_modifier: f32,
    dir_t: f32,
    mask: bool,
}

fn outer_product(
    a: vec3<f32>,
    b: vec3<f32>,
) -> mat3x3<f32> {
    return mat3x3<f32>(
        a.x * b.x, a.x * b.y, a.x * b.z,
        a.y * b.x, a.y * b.y, a.y * b.z,
        a.z * b.x, a.z * b.y, a.z * b.z,
    );
}

fn conditional_cov3d(
    position: vec3<f32>,
    index: u32,
    time: f32,
) -> DecomposedGaussian4d {
    let isotropic_rotations = get_isotropic_rotations(index);
    let rotation = isotropic_rotations[0];
    let rotation_r = isotropic_rotations[1];
    let scale = get_scale(index);

    let dt = time - get_timestamp(index);

    let S = mat4x4<f32>(
        gaussian_uniforms.global_scale * scale.x, 0.0, 0.0, 0.0,
        0.0, gaussian_uniforms.global_scale * scale.y, 0.0, 0.0,
        0.0, 0.0, gaussian_uniforms.global_scale * scale.z, 0.0,
        0.0, 0.0, 0.0, get_time_scale(index),
    );

    let w = rotation.x;
    let x = rotation.y;
    let y = rotation.z;
    let z = rotation.w;

    let wr = rotation_r.x;
    let xr = rotation_r.y;
    let yr = rotation_r.z;
    let zr = rotation_r.w;

    let M_l = mat4x4<f32>(
        w, -x, -y, -z,
        x,  w, -z,  y,
        y,  z,  w, -x,
        z, -y,  x,  w,
    );

    let M_r = mat4x4<f32>(
        wr, -xr, -yr, -zr,
        xr,  wr,  zr, -yr,
        yr, -zr,  wr,  xr,
        zr,  yr, -xr,  wr,
    );

    let R = M_r * M_l;
    let M = R * S;
    let Sigma = transpose(M) * M;

    let cov_t = Sigma[3][3];
    let marginal_t = exp(-0.5 * dt * dt / cov_t);

    let mask = marginal_t > 0.05;
    if !mask {
        return DecomposedGaussian4d(
            array<f32, 6>(0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            vec3<f32>(0.0, 0.0, 0.0),
            0.0,
            dt,
            mask,
        );
    }

    let opacity_modifier = marginal_t;

    let cov11 = mat3x3<f32>(
        Sigma[0][0], Sigma[0][1], Sigma[0][2],
        Sigma[1][0], Sigma[1][1], Sigma[1][2],
        Sigma[2][0], Sigma[2][1], Sigma[2][2],
    );
    let cov12 = vec3<f32>(Sigma[0][3], Sigma[1][3], Sigma[2][3]);
    let cov_op = outer_product(cov12, cov12);
    let cov_op_t = mat3x3<f32>(
        cov_op[0].x / cov_t, cov_op[0].y / cov_t, cov_op[0].z / cov_t,
        cov_op[1].x / cov_t, cov_op[1].y / cov_t, cov_op[1].z / cov_t,
        cov_op[2].x / cov_t, cov_op[2].y / cov_t, cov_op[2].z / cov_t,
    );
    let cov3d_condition = cov11 - cov_op_t;

    let delta_mean = (cov12 / cov_t) * dt;

    return DecomposedGaussian4d(
        array<f32, 6>(
            cov3d_condition[0][0],
            cov3d_condition[0][1],
            cov3d_condition[0][2],
            cov3d_condition[1][1],
            cov3d_condition[1][2],
            cov3d_condition[2][2]
        ),
        delta_mean,
        opacity_modifier,
        dt,
        mask,
    );
}
