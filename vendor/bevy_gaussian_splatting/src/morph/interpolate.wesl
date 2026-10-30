import bevy_gaussian_splatting::render::bindings::gaussian_uniforms;
@if(PACKED_F32) import bevy_gaussian_splatting::render::packed::get_opacity;
@if(PACKED_F32) import bevy_gaussian_splatting::render::packed::get_position;
@if(PACKED_F32) import bevy_gaussian_splatting::render::packed::get_rotation;
@if(PACKED_F32) import bevy_gaussian_splatting::render::packed::get_scale;
@if(PACKED_F32) import bevy_gaussian_splatting::render::packed::get_spherical_harmonics;
@if(PACKED_F32) import bevy_gaussian_splatting::render::packed::get_visibility;
@if(PACKED_F32) import bevy_gaussian_splatting::render::packed::get_rhs_opacity;
@if(PACKED_F32) import bevy_gaussian_splatting::render::packed::get_rhs_position;
@if(PACKED_F32) import bevy_gaussian_splatting::render::packed::get_rhs_rotation;
@if(PACKED_F32) import bevy_gaussian_splatting::render::packed::get_rhs_scale;
@if(PACKED_F32) import bevy_gaussian_splatting::render::packed::get_rhs_spherical_harmonics;
@if(PACKED_F32) import bevy_gaussian_splatting::render::packed::get_rhs_visibility;
@if(PACKED_F32) import bevy_gaussian_splatting::render::packed::set_output_position_visibility;
@if(PACKED_F32) import bevy_gaussian_splatting::render::packed::set_output_spherical_harmonics;
@if(PACKED_F32) import bevy_gaussian_splatting::render::packed::set_output_transform;
@if(!PACKED_F32) import bevy_gaussian_splatting::render::planar::get_opacity;
@if(!PACKED_F32) import bevy_gaussian_splatting::render::planar::get_position;
@if(!PACKED_F32) import bevy_gaussian_splatting::render::planar::get_rotation;
@if(!PACKED_F32) import bevy_gaussian_splatting::render::planar::get_scale;
@if(!PACKED_F32) import bevy_gaussian_splatting::render::planar::get_spherical_harmonics;
@if(!PACKED_F32) import bevy_gaussian_splatting::render::planar::get_visibility;
@if(!PACKED_F32) import bevy_gaussian_splatting::render::planar::get_rhs_opacity;
@if(!PACKED_F32) import bevy_gaussian_splatting::render::planar::get_rhs_position;
@if(!PACKED_F32) import bevy_gaussian_splatting::render::planar::get_rhs_rotation;
@if(!PACKED_F32) import bevy_gaussian_splatting::render::planar::get_rhs_scale;
@if(!PACKED_F32) import bevy_gaussian_splatting::render::planar::get_rhs_spherical_harmonics;
@if(!PACKED_F32) import bevy_gaussian_splatting::render::planar::get_rhs_visibility;
@if(!PACKED_F32) import bevy_gaussian_splatting::render::planar::set_output_position_visibility;
@if(!PACKED_F32) import bevy_gaussian_splatting::render::planar::set_output_spherical_harmonics;
@if(!PACKED_F32) import bevy_gaussian_splatting::render::planar::set_output_transform;
@if(!PACKED_F32 && PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::planar::get_cov3d;
@if(!PACKED_F32 && PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::planar::get_rhs_cov3d;
@if(!PACKED_F32 && PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::planar::set_output_covariance;




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
    for (var i = 0u; i < constants::SH_COEFF_COUNT; i = i + 1u) {
        sh[i] = mix(sh[i], rhs_sh[i], t);
    }
    set_output_spherical_harmonics(index, sh);

    @if(PRECOMPUTE_COVARIANCE_3D) var cov = get_cov3d(index);
    @if(PRECOMPUTE_COVARIANCE_3D) let rhs_cov = get_rhs_cov3d(index);
    @if(PRECOMPUTE_COVARIANCE_3D) for (var i = 0u; i < 6u; i = i + 1u) {
        cov[i] = mix(cov[i], rhs_cov[i], t);
    }
    @if(PRECOMPUTE_COVARIANCE_3D) let opacity = mix(get_opacity(index), get_rhs_opacity(index), t);
    @if(PRECOMPUTE_COVARIANCE_3D) set_output_covariance(index, cov, opacity);
    @if(!PRECOMPUTE_COVARIANCE_3D) let rotation = normalize_quaternion(
        mix(
            get_rotation(index),
            get_rhs_rotation(index),
            rotation_t,
        ),
    );

    @if(!PRECOMPUTE_COVARIANCE_3D) let scale = mix(get_scale(index), get_rhs_scale(index), position_t);
    @if(!PRECOMPUTE_COVARIANCE_3D) let opacity = mix(get_opacity(index), get_rhs_opacity(index), t);
    @if(!PRECOMPUTE_COVARIANCE_3D) set_output_transform(index, rotation, scale, opacity);
}
