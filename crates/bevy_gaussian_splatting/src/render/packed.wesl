import bevy_gaussian_splatting::render::bindings::gaussian_uniforms;
import bevy_gaussian_splatting::render::bindings::points;
@if(GAUSSIAN_3D_STRUCTURE && PACKED_F32) import bevy_gaussian_splatting::render::bindings::Gaussian;
@if(BINARY_GAUSSIAN_OP) import bevy_gaussian_splatting::render::bindings::rhs_points;
@if(BINARY_GAUSSIAN_OP) import bevy_gaussian_splatting::render::bindings::out_points;
import bevy_gaussian_splatting::material::spherical_harmonics::spherical_harmonics_lookup;
import bevy_gaussian_splatting::material::spherical_harmonics::srgb_to_linear;


@if(PACKED_F32)
fn convert_sh_color_to_linear(color: vec3<f32>) -> vec3<f32> {
    if gaussian_uniforms.color_space == 1u {
        return color;
    }

    return srgb_to_linear(color);
}

@if(PACKED_F32)
fn gaussian_position(point: Gaussian) -> vec3<f32> {
    return point.position_visibility.xyz;
}

@if(PACKED_F32)
fn gaussian_color(point: Gaussian, ray_direction: vec3<f32>) -> vec3<f32> {
    let sh = gaussian_spherical_harmonics(point);
    let color = spherical_harmonics_lookup(ray_direction, sh);
    return convert_sh_color_to_linear(color);
}

@if(PACKED_F32)
fn gaussian_spherical_harmonics(point: Gaussian) -> array<f32, constants::SH_COEFF_COUNT> {
    return point.sh;
}

@if(PACKED_F32)
fn gaussian_rotation(point: Gaussian) -> vec4<f32> {
    return point.rotation;
}

@if(PACKED_F32)
fn gaussian_scale(point: Gaussian) -> vec3<f32> {
    return point.scale_opacity.xyz;
}

@if(PACKED_F32)
fn gaussian_opacity(point: Gaussian) -> f32 {
    return point.scale_opacity.w;
}

@if(PACKED_F32)
fn gaussian_visibility(point: Gaussian) -> f32 {
    return point.position_visibility.w;
}

@if(PACKED_F32)
fn get_position(index: u32) -> vec3<f32> {
    return gaussian_position(points[index]);
}

@if(PACKED_F32)
fn get_color(
    index: u32,
    ray_direction: vec3<f32>,
) -> vec3<f32> {
    return gaussian_color(points[index], ray_direction);
}

@if(PACKED_F32)
fn get_spherical_harmonics(index: u32) -> array<f32, constants::SH_COEFF_COUNT> {
    return gaussian_spherical_harmonics(points[index]);
}

@if(PACKED_F32)
fn get_rotation(index: u32) -> vec4<f32> {
    return gaussian_rotation(points[index]);
}

@if(PACKED_F32)
fn get_scale(index: u32) -> vec3<f32> {
    return gaussian_scale(points[index]);
}

@if(PACKED_F32)
fn get_opacity(index: u32) -> f32 {
    return gaussian_opacity(points[index]);
}

@if(PACKED_F32)
fn get_visibility(index: u32) -> f32 {
    return gaussian_visibility(points[index]);
}


@if(PACKED_F32 && BINARY_GAUSSIAN_OP)
fn get_rhs_position(index: u32) -> vec3<f32> {
    return gaussian_position(rhs_points[index]);
}

@if(PACKED_F32 && BINARY_GAUSSIAN_OP)
fn get_rhs_color(
    index: u32,
    ray_direction: vec3<f32>,
) -> vec3<f32> {
    return gaussian_color(rhs_points[index], ray_direction);
}

@if(PACKED_F32 && BINARY_GAUSSIAN_OP)
fn get_rhs_spherical_harmonics(index: u32) -> array<f32, constants::SH_COEFF_COUNT> {
    return gaussian_spherical_harmonics(rhs_points[index]);
}

@if(PACKED_F32 && BINARY_GAUSSIAN_OP)
fn get_rhs_rotation(index: u32) -> vec4<f32> {
    return gaussian_rotation(rhs_points[index]);
}

@if(PACKED_F32 && BINARY_GAUSSIAN_OP)
fn get_rhs_scale(index: u32) -> vec3<f32> {
    return gaussian_scale(rhs_points[index]);
}

@if(PACKED_F32 && BINARY_GAUSSIAN_OP)
fn get_rhs_opacity(index: u32) -> f32 {
    return gaussian_opacity(rhs_points[index]);
}

@if(PACKED_F32 && BINARY_GAUSSIAN_OP)
fn get_rhs_visibility(index: u32) -> f32 {
    return gaussian_visibility(rhs_points[index]);
}

@if(PACKED_F32 && BINARY_GAUSSIAN_OP)
fn set_output_position_visibility(
    index: u32,
    position: vec3<f32>,
    visibility: f32,
) {
    out_points[index].position_visibility = vec4<f32>(position, visibility);
}

@if(PACKED_F32 && BINARY_GAUSSIAN_OP)
fn set_output_spherical_harmonics(
    index: u32,
    sh: array<f32, constants::SH_COEFF_COUNT>,
) {
    out_points[index].sh = sh;
}

@if(PACKED_F32 && BINARY_GAUSSIAN_OP)
fn set_output_transform(
    index: u32,
    rotation: vec4<f32>,
    scale: vec3<f32>,
    opacity: f32,
) {
    out_points[index].rotation = rotation;
    out_points[index].scale_opacity = vec4<f32>(scale, opacity);
}
