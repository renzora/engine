#define_import_path bevy_gaussian_splatting::gaussian_2d

#ifdef GAUSSIAN_2D
#import bevy_gaussian_splatting::bindings::{
    view,
    gaussian_uniforms,
}
#import bevy_gaussian_splatting::helpers::{
    get_rotation_matrix,
    get_scale_matrix,
    intrinsic_matrix,
}

#ifdef PACKED
    #import bevy_gaussian_splatting::packed::{
        get_position,
        get_color,
        get_visibility,
        get_opacity,
        get_rotation,
        get_scale,
    }
#else ifdef BUFFER_STORAGE
    #import bevy_gaussian_splatting::planar::{
        get_position,
        get_color,
        get_visibility,
        get_opacity,
        get_rotation,
        get_scale,
    }
#else BUFFER_TEXTURE
    #import bevy_gaussian_splatting::texture::{
        get_position,
        get_color,
        get_visibility,
        get_opacity,
        get_rotation,
        get_scale,
    }
#endif

struct Surfel {
    local_to_pixel: mat3x3<f32>,
    mean_2d: vec2<f32>,
    extent: vec2<f32>,
};

fn get_bounding_box_cov2d(
    extent: vec2<f32>,
    direction: vec2<f32>,
    cutoff: f32,
) -> vec4<f32> {
    let filter_size = 0.707106;

    if extent.x < 1.e-4 || extent.y < 1.e-4 {
        return vec4<f32>(0.0);
    }

    let radius = sqrt(extent);
    let max_radius = vec2<f32>(max(
        max(radius.x, radius.y),
        cutoff * filter_size,
    ));

    // TODO: verify OBB capability
    let radius_ndc = vec2<f32>(
        max_radius / view.viewport.zw,
    );

    return vec4<f32>(
        radius_ndc * direction,
        max_radius,
    );
}

fn compute_cov2d_surfel(
    gaussian_position: vec3<f32>,
    index: u32,
    cutoff: f32,
) -> Surfel {
    var output: Surfel;

    let rotation = get_rotation(index);
    let scale = get_scale(index);

    let T_r = mat3x3<f32>(
        gaussian_uniforms.transform[0].xyz,
        gaussian_uniforms.transform[1].xyz,
        gaussian_uniforms.transform[2].xyz,
    );

    let S = get_scale_matrix(scale);
    let R = get_rotation_matrix(rotation);

    let L = T_r * transpose(R) * S;

    let world_from_local = mat3x4<f32>(
        vec4<f32>(L[0], 0.0),
        vec4<f32>(L[1], 0.0),
        vec4<f32>(gaussian_position, 1.0),
    );

    let ndc_from_world = transpose(view.clip_from_world);
    let pixels_from_ndc = intrinsic_matrix();

    let T = transpose(world_from_local) * ndc_from_world * pixels_from_ndc;

    let test = vec3<f32>(cutoff * cutoff, cutoff * cutoff, -1.0);
    let d = dot(test * T[2], T[2]);
    if abs(d) < 1.0e-4 {
        output.extent = vec2<f32>(0.0);
        return output;
    }

    let f = (1.0 / d) * test;
    let mean_2d = vec2<f32>(
        dot(f, T[0] * T[2]),
        dot(f, T[1] * T[2]),
    );

    let t = vec2<f32>(
        dot(f * T[0], T[0]),
        dot(f * T[1], T[1]),
    );
    let extent = mean_2d * mean_2d - t;

    output.local_to_pixel = T;
    output.mean_2d = mean_2d;
    output.extent = extent;
    return output;
}

fn surfel_fragment_power(
    local_to_pixel: mat3x3<f32>,
    pixel_coord: vec2<f32>,
    mean_2d: vec2<f32>,
) -> f32 {
    let deltas = mean_2d - pixel_coord;

    let hu = pixel_coord.x * local_to_pixel[2] - local_to_pixel[0];
    let hv = pixel_coord.y * local_to_pixel[2] - local_to_pixel[1];

    let p = cross(hu, hv);

    let us = p.x / p.z;
    let vs = p.y / p.z;

    let sigmas_3d = us * us + vs * vs;
    let sigmas_2d = 2.0 * (deltas.x * deltas.x + deltas.y * deltas.y);

    let sigmas = 0.5 * min(sigmas_3d, sigmas_2d);
    let power = -sigmas;

    return power;
}

#endif  // GAUSSIAN_2D
