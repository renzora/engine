@if(GAUSSIAN_3D) import bevy_gaussian_splatting::render::bindings::view;
@if(GAUSSIAN_3D) import bevy_gaussian_splatting::render::bindings::gaussian_uniforms;
@if(GAUSSIAN_3D) import bevy_gaussian_splatting::render::helpers::cov2d;
@if(GAUSSIAN_3D) import bevy_gaussian_splatting::render::helpers::get_rotation_matrix;
@if(GAUSSIAN_3D) import bevy_gaussian_splatting::render::helpers::get_scale_matrix;
@if(GAUSSIAN_3D && PACKED && PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::packed::get_cov3d;
@if(GAUSSIAN_3D && PACKED && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::packed::get_rotation;
@if(GAUSSIAN_3D && PACKED && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::packed::get_scale;
@if(GAUSSIAN_3D && !PACKED && BUFFER_STORAGE && PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::planar::get_cov3d;
@if(GAUSSIAN_3D && !PACKED && BUFFER_STORAGE && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::planar::get_rotation;
@if(GAUSSIAN_3D && !PACKED && BUFFER_STORAGE && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::planar::get_scale;
@if(GAUSSIAN_3D && !PACKED && !BUFFER_STORAGE && BUFFER_TEXTURE && PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::texture::get_cov3d;
@if(GAUSSIAN_3D && !PACKED && !BUFFER_STORAGE && BUFFER_TEXTURE && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::texture::get_rotation;
@if(GAUSSIAN_3D && !PACKED && !BUFFER_STORAGE && BUFFER_TEXTURE && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::texture::get_scale;



@if(GAUSSIAN_3D)
fn compute_cov3d(scale: vec3<f32>, rotation: vec4<f32>) -> array<f32, 6> {
    let S = get_scale_matrix(scale);

    let T = mat3x3<f32>(
        gaussian_uniforms.transform[0].xyz,
        gaussian_uniforms.transform[1].xyz,
        gaussian_uniforms.transform[2].xyz,
    );

    let R = get_rotation_matrix(rotation);

    let M = S * R;
    let Sigma = transpose(M) * M;
    let TS = T * Sigma * transpose(T);

    return array<f32, 6>(
        TS[0][0],
        TS[0][1],
        TS[0][2],
        TS[1][1],
        TS[1][2],
        TS[2][2],
    );
}

@if(GAUSSIAN_3D)
fn compute_cov2d_3dgs(
    position: vec3<f32>,
    index: u32,
) -> vec3<f32> {
    @if(PRECOMPUTE_COVARIANCE_3D) let cov3d = get_cov3d(index);
    @if(!PRECOMPUTE_COVARIANCE_3D) let rotation = get_rotation(index);
    @if(!PRECOMPUTE_COVARIANCE_3D) let scale = get_scale(index);

    @if(!PRECOMPUTE_COVARIANCE_3D) let cov3d = compute_cov3d(scale, rotation);

    return cov2d(position, cov3d);
}
