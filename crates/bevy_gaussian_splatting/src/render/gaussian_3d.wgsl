#define_import_path bevy_gaussian_splatting::gaussian_3d

#ifdef GAUSSIAN_3D
#import bevy_gaussian_splatting::bindings::{
    view,
    gaussian_uniforms,
}
#import bevy_gaussian_splatting::helpers::{
    cov2d,
    get_rotation_matrix,
    get_scale_matrix,
}

#ifdef PACKED
    #ifdef PRECOMPUTE_COVARIANCE_3D
        #import bevy_gaussian_splatting::packed::{
            get_cov3d,
        }
    #else
        #import bevy_gaussian_splatting::packed::{
            get_rotation,
            get_scale,
        }
    #endif
#else ifdef BUFFER_STORAGE
    #ifdef PRECOMPUTE_COVARIANCE_3D
        #import bevy_gaussian_splatting::planar::{
            get_cov3d,
        }
    #else
        #import bevy_gaussian_splatting::planar::{
            get_rotation,
            get_scale,
        }
    #endif
#else ifdef BUFFER_TEXTURE
    #ifdef PRECOMPUTE_COVARIANCE_3D
        #import bevy_gaussian_splatting::texture::{
            get_cov3d,
        }
    #else
        #import bevy_gaussian_splatting::texture::{
            get_rotation,
            get_scale,
        }
    #endif
#endif

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

fn compute_cov2d_3dgs(
    position: vec3<f32>,
    index: u32,
) -> vec3<f32> {
#ifdef PRECOMPUTE_COVARIANCE_3D
    let cov3d = get_cov3d(index);
#else
    let rotation = get_rotation(index);
    let scale = get_scale(index);

    let cov3d = compute_cov3d(scale, rotation);
#endif

    return cov2d(position, cov3d);
}

#endif  // GAUSSIAN_3D
