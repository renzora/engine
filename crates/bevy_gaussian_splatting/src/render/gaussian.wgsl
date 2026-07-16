#import bevy_gaussian_splatting::bindings::{
    view,
    gaussian_uniforms,
    Entry,
}
#import bevy_gaussian_splatting::classification::class_to_rgb
#import bevy_gaussian_splatting::depth::depth_to_rgb
#import bevy_gaussian_splatting::optical_flow::{
    calculate_motion_vector,
    optical_flow_to_rgb,
}
#import bevy_gaussian_splatting::helpers::{
    get_rotation_matrix,
    get_scale_matrix,
}
#import bevy_gaussian_splatting::transform::{
    world_to_clip,
    in_frustum,
}

#ifdef GAUSSIAN_2D
    #import bevy_gaussian_splatting::gaussian_2d::{
        compute_cov2d_surfel,
        get_bounding_box_cov2d,
        surfel_fragment_power,
    }
#else ifdef GAUSSIAN_3D
    #import bevy_gaussian_splatting::gaussian_3d::{
        compute_cov2d_3dgs,
    }
    #import bevy_gaussian_splatting::helpers::{
        get_bounding_box_clip,
    }
#else ifdef GAUSSIAN_4D
    #import bevy_gaussian_splatting::gaussian_4d::{
        conditional_cov3d,
    }
    #import bevy_gaussian_splatting::helpers::{
        cov2d,
        get_bounding_box_clip,
    }
#endif

#ifdef PACKED
    #ifdef PRECOMPUTE_COVARIANCE_3D
        #import bevy_gaussian_splatting::packed::{
            get_position,
            get_color,
            get_visibility,
            get_opacity,
            get_cov3d,
        }
    #else
        #import bevy_gaussian_splatting::packed::{
            get_position,
            get_color,
            get_visibility,
            get_opacity,
            get_rotation,
            get_scale,
        }
    #endif
#else ifdef BUFFER_STORAGE
    #ifdef PRECOMPUTE_COVARIANCE_3D
        #import bevy_gaussian_splatting::planar::{
            get_position,
            get_color,
            get_visibility,
            get_opacity,
            get_cov3d,
        }
    #else
        #import bevy_gaussian_splatting::planar::{
            get_position,
            get_color,
            get_visibility,
            get_opacity,
            get_rotation,
            get_scale,
        }
    #endif
#else ifdef BUFFER_TEXTURE
    #ifdef PRECOMPUTE_COVARIANCE_3D
        #import bevy_gaussian_splatting::texture::{
            get_position,
            get_color,
            get_visibility,
            get_opacity,
            get_cov3d,
            location,
        }
    #else
        #import bevy_gaussian_splatting::texture::{
            get_position,
            get_color,
            get_visibility,
            get_opacity,
            get_rotation,
            get_scale,
            location,
        }
    #endif
#endif

#ifdef BUFFER_STORAGE
    @group(3) @binding(0) var<storage, read> sorted_entries: array<Entry>;
    fn get_entry(index: u32) -> Entry {
        return sorted_entries[index];
    }
#else ifdef BUFFER_TEXTURE
    @group(3) @binding(0) var sorted_entries: texture_2d<u32>;
    fn get_entry(index: u32) -> Entry {
        let sample = textureLoad(
            sorted_entries,
            location(index),
            0,
        );

        return Entry(
            sample.r,
            sample.g,
        );
    }
#endif

#ifdef WEBGL2
    struct GaussianVertexOutput {
        @builtin(position) position: vec4<f32>,
        @location(0) color: vec4<f32>,
        @location(1) uv: vec2<f32>,
    #ifdef GAUSSIAN_2D
        @location(2) local_to_pixel_u: vec3<f32>,
        @location(3) local_to_pixel_v: vec3<f32>,
        @location(4) local_to_pixel_w: vec3<f32>,
        @location(5) mean_2d: vec2<f32>,
        @location(6) radius: vec2<f32>,
    #else #ifdef GAUSSIAN_3D
        @location(2) conic: vec3<f32>,
        @location(3) major_minor: vec2<f32>,
    #else #ifdef GAUSSIAN_4D
        @location(2) conic: vec3<f32>,
        @location(3) major_minor: vec2<f32>,
    #endif
    };
#else
    struct GaussianVertexOutput {
        @builtin(position) position: vec4<f32>,
        @location(0) @interpolate(flat) color: vec4<f32>,
        @location(1) @interpolate(linear) uv: vec2<f32>,
    #ifdef GAUSSIAN_2D
        @location(2) @interpolate(flat) local_to_pixel_u: vec3<f32>,
        @location(3) @interpolate(flat) local_to_pixel_v: vec3<f32>,
        @location(4) @interpolate(flat) local_to_pixel_w: vec3<f32>,
        @location(5) @interpolate(flat) mean_2d: vec2<f32>,
        @location(6) @interpolate(flat) radius: vec2<f32>,
    #else ifdef GAUSSIAN_3D
        @location(2) @interpolate(flat) conic: vec3<f32>,
        @location(3) @interpolate(linear) major_minor: vec2<f32>,
    #else ifdef GAUSSIAN_4D
        @location(2) @interpolate(flat) conic: vec3<f32>,
        @location(3) @interpolate(linear) major_minor: vec2<f32>,
    #endif
    };
#endif

fn world_to_local_direction(ray_direction_world: vec3<f32>, transform: mat4x4<f32>) -> vec3<f32> {
    let basis = mat3x3<f32>(
        transform[0].xyz,
        transform[1].xyz,
        transform[2].xyz,
    );
    let basis_x = normalize(basis[0]);
    let basis_y = normalize(basis[1]);
    let basis_z = normalize(basis[2]);

    let local = vec3<f32>(
        dot(basis_x, ray_direction_world),
        dot(basis_y, ray_direction_world),
        dot(basis_z, ray_direction_world),
    );

    return normalize(local);
}
@vertex
fn vs_points(
    @builtin(instance_index) instance_index: u32,
    @builtin(vertex_index) vertex_index: u32,
) -> GaussianVertexOutput {
    var output: GaussianVertexOutput;

    let entry = get_entry(instance_index);
    let splat_index = entry.value;

    var discard_quad = false;

    discard_quad |= entry.key == 0xFFFFFFFFu; // || splat_index == 0u;

    let position = vec4<f32>(get_position(splat_index), 1.0);

    var transformed_position = (gaussian_uniforms.transform * position).xyz;
    var previous_transformed_position = transformed_position;

#ifdef DRAW_SELECTED
    discard_quad |= get_visibility(splat_index) < 0.5;
#endif

#ifdef GAUSSIAN_4D
#else
    let projected_position = world_to_clip(transformed_position);
    discard_quad |= !in_frustum(projected_position.xyz);
#endif

    if (discard_quad) {
        output.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        output.position = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return output;
    }

    var quad_vertices = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
    );

    let quad_index = vertex_index % 4u;
    let quad_offset = quad_vertices[quad_index];

    var opacity = get_opacity(splat_index);

#ifdef OPACITY_ADAPTIVE_RADIUS
    let cutoff = sqrt(max(9.0 + 2.0 * log(opacity), 0.000001));
#else
    let cutoff = 3.0;
#endif

#ifdef GAUSSIAN_2D
    let surfel = compute_cov2d_surfel(
        transformed_position,
        splat_index,
        cutoff,
    );

    output.local_to_pixel_u = surfel.local_to_pixel[0];
    output.local_to_pixel_v = surfel.local_to_pixel[1];
    output.local_to_pixel_w = surfel.local_to_pixel[2];
    output.mean_2d = surfel.mean_2d;

    let bb = get_bounding_box_cov2d(
        surfel.extent,
        quad_offset,
        cutoff,
    );
    output.radius = bb.zw;
#else
    #ifdef GAUSSIAN_3D
        let gaussian_cov2d = compute_cov2d_3dgs(
            transformed_position,
            splat_index,
        );
    #else ifdef GAUSSIAN_4D
        let gaussian_4d = conditional_cov3d(
            transformed_position,
            splat_index,
            gaussian_uniforms.time,
        );

        if !gaussian_4d.mask {
            output.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
            output.position = vec4<f32>(0.0, 0.0, 0.0, 0.0);
            return output;
        }

        let position_t = vec4<f32>(position.xyz + gaussian_4d.delta_mean, 1.0);
        transformed_position = (gaussian_uniforms.transform * position_t).xyz;
        // TODO: set previous_transformed_position based on temporal position delta
        let projected_position = world_to_clip(transformed_position);

        if !in_frustum(projected_position.xyz) {
            output.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
            output.position = vec4<f32>(0.0, 0.0, 0.0, 0.0);
            return output;
        }

        opacity = opacity * gaussian_4d.opacity_modifier;

        let gaussian_cov2d = cov2d(
            transformed_position,
            gaussian_4d.cov3d,
        );
    #endif

    let bb = get_bounding_box_clip(
        gaussian_cov2d,
        quad_offset,
        cutoff,
    );

    #ifdef USE_AABB
        let det = gaussian_cov2d.x * gaussian_cov2d.z - gaussian_cov2d.y * gaussian_cov2d.y;
        let det_inv = 1.0 / det;
        let conic = vec3<f32>(
            gaussian_cov2d.z * det_inv,
            -gaussian_cov2d.y * det_inv,
            gaussian_cov2d.x * det_inv
        );
        output.conic = conic;
        output.major_minor = bb.zw;
    #endif
#endif

    var rgb = vec3<f32>(0.0);

// TODO: RASTERIZE_ACCELERATION
#ifdef RASTERIZE_CLASSIFICATION
    let ray_direction_world = normalize(transformed_position - view.world_position);
    let ray_direction_local = world_to_local_direction(ray_direction_world, gaussian_uniforms.transform);

    #ifdef GAUSSIAN_3D_STRUCTURE
        rgb = get_color(splat_index, ray_direction_local);
    #else ifdef GAUSSIAN_4D
        rgb = get_color(splat_index, gaussian_4d.dir_t, ray_direction_local);
    #endif

    rgb = class_to_rgb(
        get_visibility(splat_index),
        rgb,
    );
#else ifdef RASTERIZE_DEPTH
    // TODO: unbiased depth rendering, see: https://zju3dv.github.io/pgsr/
    let first_position = vec4<f32>(get_position(get_entry(1u).value), 1.0);
    let last_position = vec4<f32>(get_position(get_entry(gaussian_uniforms.count - 1u).value), 1.0);

    let min_position = (gaussian_uniforms.transform * last_position).xyz;
    let max_position = (gaussian_uniforms.transform * first_position).xyz;

    let camera_position = view.world_position;

    let min_distance = length(min_position - camera_position);
    let max_distance = length(max_position - camera_position);

    let depth = length(transformed_position - camera_position);
    rgb = depth_to_rgb(
        depth,
        min_distance,
        max_distance,
    );
#else ifdef RASTERIZE_NORMAL
    // TODO: support rotation decomposition for 4d gaussians
    let R = get_rotation_matrix(get_rotation(splat_index));
    let S = get_scale_matrix(get_scale(splat_index));
    let T = mat3x3<f32>(
        gaussian_uniforms.transform[0].xyz,
        gaussian_uniforms.transform[1].xyz,
        gaussian_uniforms.transform[2].xyz,
    );
    let L = T * S * R;

    let local_normal = vec4<f32>(L[2], 0.0);
    let world_normal = view.view_from_world * local_normal;

    let t = normalize(world_normal);

    rgb = vec3<f32>(
        0.5 * (t.x + 1.0),
        0.5 * (t.y + 1.0),
        0.5 * (t.z + 1.0)
    );
#else ifdef RASTERIZE_OPTICAL_FLOW
    let motion_vector = calculate_motion_vector(
        transformed_position,
        previous_transformed_position,
    );

    rgb = optical_flow_to_rgb(motion_vector);
#else ifdef RASTERIZE_POSITION
    rgb = (transformed_position - gaussian_uniforms.min.xyz) / (gaussian_uniforms.max.xyz - gaussian_uniforms.min.xyz);
#else ifdef RASTERIZE_VELOCITY
    let time_delta = 1e-3;
    let future_gaussian_4d = conditional_cov3d(
        transformed_position,
        splat_index,
        gaussian_uniforms.time + time_delta,
    );
    let position_delta = future_gaussian_4d.delta_mean - gaussian_4d.delta_mean;
    let velocity = position_delta / time_delta;
    let velocity_magnitude = length(velocity);
    let velocity_normalized = normalize(velocity);

    // TODO: magnitude normalization
    let min_magnitude = 1.0;
    let max_magnitude = 2.0;

    let scaled_mag = clamp(
        (velocity_magnitude - min_magnitude) / (max_magnitude - min_magnitude),
        0.0,
        1.0
    );

    if scaled_mag < 1e-2 {
        opacity = 0.0;
    }

    let base_color = 0.5 * (velocity_normalized + vec3<f32>(1.0, 1.0, 1.0));
    rgb = base_color * scaled_mag;
#else ifdef RASTERIZE_COLOR
    // TODO: verify color benefit for ray_direction computed at quad verticies instead of gaussian center (same as current complexity)
    let ray_direction_world = normalize(transformed_position - view.world_position);
    let ray_direction_local = world_to_local_direction(ray_direction_world, gaussian_uniforms.transform);

    #ifdef GAUSSIAN_3D_STRUCTURE
        rgb = get_color(splat_index, ray_direction_local);
    #else ifdef GAUSSIAN_4D
        rgb = get_color(splat_index, gaussian_4d.dir_t, ray_direction_local);
    #endif
#endif

    output.color = vec4<f32>(
        rgb,
        opacity * gaussian_uniforms.global_opacity,
    );

#ifdef HIGHLIGHT_SELECTED
    if (get_visibility(splat_index) > 0.5) {
        output.color = vec4<f32>(0.3, 1.0, 0.1, 1.0);
    }
#endif

    output.uv = quad_offset;
    output.position = vec4<f32>(
        projected_position.xy + bb.xy,
        projected_position.zw,
    );

    return output;
}

@fragment
fn fs_main(input: GaussianVertexOutput) -> @location(0) vec4<f32> {
#ifdef USE_AABB
#ifdef GAUSSIAN_2D
    let radius = input.radius;
    let mean_2d = input.mean_2d;
    let aspect = vec2<f32>(
        1.0,
        view.viewport.z / view.viewport.w,
    );
    let pixel_coord = input.uv * radius * aspect + mean_2d;

    let power = surfel_fragment_power(
        mat3x3<f32>(
            input.local_to_pixel_u,
            input.local_to_pixel_v,
            input.local_to_pixel_w,
        ),
        pixel_coord,
        mean_2d,
    );
#else ifdef GAUSSIAN_3D
    let d = -input.major_minor;
    let conic = input.conic;
    let power = -0.5 * (conic.x * d.x * d.x + conic.z * d.y * d.y) + conic.y * d.x * d.y;
#else ifdef GAUSSIAN_4D
    let d = -input.major_minor;
    let conic = input.conic;
    let power = -0.5 * (conic.x * d.x * d.x + conic.z * d.y * d.y) + conic.y * d.x * d.y;
#endif

    if (power > 0.0) {
        discard;
    }
#endif

#ifdef USE_OBB
    let sigma = 1.0 / 3.0;
    let sigma_squared = 2.0 * sigma * sigma;
    let distance_squared = dot(input.uv, input.uv);

    let power = -distance_squared / sigma_squared;

    if (distance_squared > 3.0 * 3.0) {
        discard;
    }
#endif

#ifdef VISUALIZE_BOUNDING_BOX
    let uv = input.uv * 0.5 + 0.5;
    let edge_width = 0.08;
    if (
        (uv.x < edge_width || uv.x > 1.0 - edge_width) ||
        (uv.y < edge_width || uv.y > 1.0 - edge_width)
    ) {
        return vec4<f32>(0.3, 1.0, 0.1, 1.0);
    }
#endif

    let alpha = min(exp(power) * input.color.a, 0.999);

    // TODO: round alpha to terminate depth test?

#ifdef DEPTH_WRITE_PASS
    // renzora patch(depth-write): depth-only pass — keep depth writes to the
    // solid core of each splat and discard the soft fringe, so depth-reading
    // post passes (volumetric fog, DoF) see the splat surface instead of the
    // far plane, without cutting hard quad silhouettes out of the sky. Color
    // writes are masked off at the pipeline level; the return value is unused.
    // 0.3 keeps faded splats (low CloudSettings::global_opacity) from writing
    // depth once they are mostly transparent.
    if (alpha < 0.3) {
        discard;
    }
    return vec4<f32>(0.0);
#else
    return vec4<f32>(
        input.color.rgb * alpha,
        alpha,
    );
#endif
}
