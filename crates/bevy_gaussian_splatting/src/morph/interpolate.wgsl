#define_import_path bevy_gaussian_splatting::morph::interpolate

#import bevy_gaussian_splatting::bindings::gaussian_uniforms

#ifdef PACKED_F32
    #import bevy_gaussian_splatting::packed::{
        get_opacity,
        get_position,
        get_rotation,
        get_scale,
        get_spherical_harmonics,
        get_visibility,
        get_rhs_opacity,
        get_rhs_position,
        get_rhs_rotation,
        get_rhs_scale,
        get_rhs_spherical_harmonics,
        get_rhs_visibility,
        set_output_position_visibility,
        set_output_spherical_harmonics,
        set_output_transform,
    };
#else
    #import bevy_gaussian_splatting::planar::{
        get_opacity,
        get_position,
        get_rotation,
        get_scale,
        get_spherical_harmonics,
        get_visibility,
        get_rhs_opacity,
        get_rhs_position,
        get_rhs_rotation,
        get_rhs_scale,
        get_rhs_spherical_harmonics,
        get_rhs_visibility,
        set_output_position_visibility,
        set_output_spherical_harmonics,
        set_output_transform,
    };

    #ifdef PRECOMPUTE_COVARIANCE_3D
        #import bevy_gaussian_splatting::planar::{
            get_cov3d,
            get_rhs_cov3d,
            set_output_covariance,
        };
    #endif
#endif

fn interpolation_factor() -> f32 {
    let duration = gaussian_uniforms.time_stop - gaussian_uniforms.time_start;
    if abs(duration) < 1e-6 {
        return select(0.0, 1.0, gaussian_uniforms.time >= gaussian_uniforms.time_stop);
    }
    return clamp((gaussian_uniforms.time - gaussian_uniforms.time_start) / duration, 0.0, 1.0);
}

fn normalize_quaternion(q: vec4<f32>) -> vec4<f32> {
    let length_squared = dot(q, q);
    if length_squared <= 0.0 {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    return q / sqrt(length_squared);
}

const WORKGROUP_SIZE: u32 = 256u;

@compute @workgroup_size(WORKGROUP_SIZE, 1, 1)
fn interpolate_gaussians(
    @builtin(global_invocation_id) global_id: vec3<u32>,
) {
    let index = global_id.x;
    if index >= gaussian_uniforms.count {
        return;
    }

    let t = interpolation_factor();
    let position_t = vec3<f32>(t);
    let rotation_t = vec4<f32>(t);

    let lhs_position = get_position(index);
    let rhs_position = get_rhs_position(index);
    let lhs_visibility = get_visibility(index);
    let rhs_visibility = get_rhs_visibility(index);

    let position = mix(lhs_position, rhs_position, position_t);
    let visibility = mix(lhs_visibility, rhs_visibility, t);
    set_output_position_visibility(index, position, visibility);

    var sh = get_spherical_harmonics(index);
    let rhs_sh = get_rhs_spherical_harmonics(index);
    for (var i = 0u; i < #{SH_COEFF_COUNT}; i = i + 1u) {
        sh[i] = mix(sh[i], rhs_sh[i], t);
    }
    set_output_spherical_harmonics(index, sh);

#ifdef PRECOMPUTE_COVARIANCE_3D
    var cov = get_cov3d(index);
    let rhs_cov = get_rhs_cov3d(index);
    for (var i = 0u; i < 6u; i = i + 1u) {
        cov[i] = mix(cov[i], rhs_cov[i], t);
    }
    let opacity = mix(get_opacity(index), get_rhs_opacity(index), t);
    set_output_covariance(index, cov, opacity);
#else
    let rotation = normalize_quaternion(
        mix(
            get_rotation(index),
            get_rhs_rotation(index),
            rotation_t,
        ),
    );

    let scale = mix(get_scale(index), get_rhs_scale(index), position_t);
    let opacity = mix(get_opacity(index), get_rhs_opacity(index), t);
    set_output_transform(index, rotation, scale, opacity);
#endif
}
