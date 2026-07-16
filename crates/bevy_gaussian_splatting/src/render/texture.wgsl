#define_import_path bevy_gaussian_splatting::texture

#ifdef PRECOMPUTE_COVARIANCE_3D
#import bevy_gaussian_splatting::bindings::{
    gaussian_uniforms,
    position_visibility,
    spherical_harmonics,
    covariance_3d_opacity,
};
#else
#import bevy_gaussian_splatting::bindings::{
    gaussian_uniforms,
    position_visibility,
    spherical_harmonics,
    rotation,
    rotation_scale_opacity,
    scale_opacity,
};
#endif

#import bevy_gaussian_splatting::spherical_harmonics::{
    shc,
    spherical_harmonics_lookup,
    srgb_to_linear,
}

fn location(index: u32) -> vec2<i32> {
    return vec2<i32>(
        i32(index) % i32(gaussian_uniforms.count_root_ceil),
        i32(index) / i32(gaussian_uniforms.count_root_ceil),
    );
}

fn convert_sh_color_to_linear(color: vec3<f32>) -> vec3<f32> {
    if gaussian_uniforms.color_space == 1u {
        return color;
    }

    return srgb_to_linear(color);
}

#ifdef PLANAR_TEXTURE_F16

fn get_position(index: u32) -> vec3<f32> {
    let sample = textureLoad(
        position_visibility,
        location(index),
        0,
    );

    return sample.xyz;
}

fn get_sh_vec(
    index: u32,
    plane: i32,
) -> vec4<u32> {
#if SH_VEC4_PLANES == 1
    return textureLoad(
        spherical_harmonics,
        location(index),
        plane,
    );
#else
    return textureLoad(
        spherical_harmonics,
        location(index),
        plane,
        0,
    );
#endif
}

#ifdef WEBGL2
fn get_color(
    index: u32,
    ray_direction: vec3<f32>,
) -> vec3<f32> {
    let s0 = get_sh_vec(index, 0);

    let v0 = unpack2x16float(s0.x);
    let v1 = unpack2x16float(s0.y);
    let v2 = unpack2x16float(s0.z);
    let v3 = unpack2x16float(s0.w);

    let rds = ray_direction * ray_direction;
    var color = vec3<f32>(0.5);

    color += shc[ 0] * vec3<f32>(
        v0.x,
        v0.y,
        v1.x,
    );

#if SH_COEFF_COUNT > 11
    let r1 = vec3<f32>(
        v1.y,
        v2.x,
        v2.y,
    );

    let s1 = get_sh_vec(index, 1);
    let v4 = unpack2x16float(s1.x);
    let v5 = unpack2x16float(s1.y);
    let v6 = unpack2x16float(s1.z);
    let v7 = unpack2x16float(s1.w);

    let r2 = vec3<f32>(
        v3.x,
        v3.y,
        v4.x,
    );

    let r3 = vec3<f32>(
        v4.y,
        v5.x,
        v5.y,
    );

    color += shc[ 1] * r1 * ray_direction.y;
    color += shc[ 2] * r2 * ray_direction.z;
    color += shc[ 3] * r3 * ray_direction.x;
#endif

#if SH_COEFF_COUNT > 26
    let r4 = vec3<f32>(
        v6.x,
        v6.y,
        v7.x,
    );

    let s2 = get_sh_vec(index, 2);
    let v8 = unpack2x16float(s2.x);
    let v9 = unpack2x16float(s2.y);
    let v10 = unpack2x16float(s2.z);
    let v11 = unpack2x16float(s2.w);

    let r5 = vec3<f32>(
        v7.y,
        v8.x,
        v8.y,
    );

    let r6 = vec3<f32>(
        v9.x,
        v9.y,
        v10.x,
    );

    let r7 = vec3<f32>(
        v10.y,
        v11.x,
        v11.y,
    );

    let s3 = get_sh_vec(index, 3);
    let v12 = unpack2x16float(s3.x);
    let v13 = unpack2x16float(s3.y);
    let v14 = unpack2x16float(s3.z);
    let v15 = unpack2x16float(s3.w);

    let r8 = vec3<f32>(
        v12.x,
        v12.y,
        v13.x,
    );

    color += shc[ 4] * r4 * ray_direction.x * ray_direction.y;
    color += shc[ 5] * r5 * ray_direction.y * ray_direction.z;
    color += shc[ 6] * r6 * (2.0 * rds.z - rds.x - rds.y);
    color += shc[ 7] * r7 * ray_direction.x * ray_direction.z;
    color += shc[ 8] * r8 * (rds.x - rds.y);
#endif

#if SH_COEFF_COUNT > 47
    let r9 = vec3<f32>(
        v13.y,
        v14.x,
        v14.y,
    );

    let s4 = get_sh_vec(index, 4);
    let v16 = unpack2x16float(s4.x);
    let v17 = unpack2x16float(s4.y);
    let v18 = unpack2x16float(s4.z);
    let v19 = unpack2x16float(s4.w);

    let r10 = vec3<f32>(
        v15.x,
        v15.y,
        v16.x,
    );

    let r11 = vec3<f32>(
        v16.y,
        v17.x,
        v17.y,
    );

    let r12 = vec3<f32>(
        v18.x,
        v18.y,
        v19.x,
    );

    let s5 = get_sh_vec(index, 5);
    let v20 = unpack2x16float(s5.x);
    let v21 = unpack2x16float(s5.y);
    let v22 = unpack2x16float(s5.z);
    let v23 = unpack2x16float(s5.w);

    let r13 = vec3<f32>(
        v19.y,
        v20.x,
        v20.y,
    );

    let r14 = vec3<f32>(
        v21.x,
        v21.y,
        v22.x,
    );

    let r15 = vec3<f32>(
        v22.y,
        v23.x,
        v23.y,
    );

    color += shc[ 9] * r9 * ray_direction.y * (3.0 * rds.x - rds.y);
    color += shc[10] * r10 * ray_direction.x * ray_direction.y * ray_direction.z;
    color += shc[11] * r11 * ray_direction.y * (4.0 * rds.z - rds.x - rds.y);
    color += shc[12] * r12 * ray_direction.z * (2.0 * rds.z - 3.0 * rds.x - 3.0 * rds.y);
    color += shc[13] * r13 * ray_direction.x * (4.0 * rds.z - rds.x - rds.y);
    color += shc[14] * r14 * ray_direction.z * (rds.x - rds.y);
    color += shc[15] * r15 * ray_direction.x * (rds.x - 3.0 * rds.y);
#endif

    return convert_sh_color_to_linear(color);
}
#else
fn get_spherical_harmonics(index: u32) -> array<f32, #{SH_COEFF_COUNT}> {
    var coefficients: array<f32, #{SH_COEFF_COUNT}>;

    for (var i = 0u; i < #{SH_VEC4_PLANES}u; i = i + 1u) {
        let sample = get_sh_vec(index, i32(i));

        let v0 = unpack2x16float(sample.x);
        let v1 = unpack2x16float(sample.y);
        let v2 = unpack2x16float(sample.z);
        let v3 = unpack2x16float(sample.w);

        let base_index = i * 8u;
        coefficients[base_index     ] = v0.x;
        coefficients[base_index + 1u] = v0.y;

        coefficients[base_index + 2u] = v1.x;
        coefficients[base_index + 3u] = v1.y;

        coefficients[base_index + 4u] = v2.x;
        coefficients[base_index + 5u] = v2.y;

        coefficients[base_index + 6u] = v3.x;
        coefficients[base_index + 7u] = v3.y;
    }

    return coefficients;
}

fn get_color(
    index: u32,
    ray_direction: vec3<f32>,
) -> vec3<f32> {
    let sh = get_spherical_harmonics(index);
    let color = spherical_harmonics_lookup(ray_direction, sh);
    return convert_sh_color_to_linear(color);
}
#endif

#ifdef PRECOMPUTE_COVARIANCE_3D
    fn get_cov3d(index: u32) -> array<f32, 6> {
        let sample = textureLoad(
            covariance_3d_opacity,
            location(index),
            0,
        );

        let c0 = unpack2x16float(sample.x);
        let c1 = unpack2x16float(sample.y);
        let c2 = unpack2x16float(sample.z);

        var cov3d: array<f32, 6>;

        cov3d[0] = c0.y;
        cov3d[1] = c0.x;
        cov3d[2] = c1.y;
        cov3d[3] = c1.x;
        cov3d[4] = c2.y;
        cov3d[5] = c2.x;

        return cov3d;
    }
#else
    fn get_rotation(index: u32) -> vec4<f32> {
        let sample = textureLoad(
            rotation_scale_opacity,
            location(index),
            0,
        );

        let q0 = unpack2x16float(sample.x);
        let q1 = unpack2x16float(sample.y);

        return vec4<f32>(
            q0.yx,
            q1.yx,
        );
    }

    fn get_scale(index: u32) -> vec3<f32> {
        let sample = textureLoad(
            rotation_scale_opacity,
            location(index),
            0,
        );

        let s0 = unpack2x16float(sample.z);
        let s1 = unpack2x16float(sample.w);

        return vec3<f32>(
            s0.yx,
            s1.y,
        );
    }
#endif

fn get_opacity(index: u32) -> f32 {
#ifdef PRECOMPUTE_COVARIANCE_3D
    let sample = textureLoad(
        covariance_3d_opacity,
        location(index),
        0,
    );

    return unpack2x16float(sample.w).y;
#else
    let sample = textureLoad(
        rotation_scale_opacity,
        location(index),
        0,
    );

    return unpack2x16float(sample.w).x;
#endif
}

fn get_visibility(index: u32) -> f32 {
    let sample = textureLoad(
        position_visibility,
        location(index),
        0,
    );

    return sample.w;
}
#endif

// TODO: support f32
#ifdef PLANAR_TEXTURE_F32
fn get_position(index: u32) -> vec3<f32> {
    return position_visibility[index].xyz;
}

fn get_spherical_harmonics(index: u32) -> array<f32, #{SH_COEFF_COUNT}> {
    return spherical_harmonics[index];
}

fn get_rotation(index: u32) -> vec4<f32> {
    return rotation[index];
}

fn get_scale(index: u32) -> vec3<f32> {
    return scale_opacity[index].xyz;
}

fn get_opacity(index: u32) -> f32 {
    return scale_opacity[index].w;
}

fn get_visibility(index: u32) -> f32 {
    return position_visibility[index].w;
}
#endif
