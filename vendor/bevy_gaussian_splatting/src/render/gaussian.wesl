import bevy_gaussian_splatting::render::bindings::view;
import bevy_gaussian_splatting::render::bindings::gaussian_uniforms;
import bevy_gaussian_splatting::render::bindings::Entry;
import bevy_gaussian_splatting::material::classification::class_to_rgb;
import bevy_gaussian_splatting::material::depth::depth_to_rgb;
import bevy_gaussian_splatting::material::optical_flow::calculate_motion_vector;
import bevy_gaussian_splatting::material::optical_flow::optical_flow_to_rgb;
import bevy_gaussian_splatting::render::helpers::get_rotation_matrix;
import bevy_gaussian_splatting::render::helpers::get_scale_matrix;
import bevy_gaussian_splatting::render::transform::world_to_clip;
import bevy_gaussian_splatting::render::transform::in_frustum;
@if(GAUSSIAN_2D) import bevy_gaussian_splatting::render::gaussian_2d::compute_cov2d_surfel;
@if(GAUSSIAN_2D) import bevy_gaussian_splatting::render::gaussian_2d::get_bounding_box_cov2d;
@if(GAUSSIAN_2D) import bevy_gaussian_splatting::render::gaussian_2d::surfel_fragment_power;
@if(!GAUSSIAN_2D && GAUSSIAN_3D) import bevy_gaussian_splatting::render::gaussian_3d::compute_cov2d_3dgs;
@if(!GAUSSIAN_2D && GAUSSIAN_3D) import bevy_gaussian_splatting::render::helpers::get_bounding_box_clip;
@if(!GAUSSIAN_2D && !GAUSSIAN_3D && GAUSSIAN_4D) import bevy_gaussian_splatting::render::gaussian_4d::conditional_cov3d;
@if(!GAUSSIAN_2D && !GAUSSIAN_3D && GAUSSIAN_4D) import bevy_gaussian_splatting::render::helpers::cov2d;
@if(!GAUSSIAN_2D && !GAUSSIAN_3D && GAUSSIAN_4D) import bevy_gaussian_splatting::render::helpers::get_bounding_box_clip;
@if(PACKED && PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::packed::get_position;
@if(PACKED && PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::packed::get_color;
@if(PACKED && PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::packed::get_visibility;
@if(PACKED && PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::packed::get_opacity;
@if(PACKED && PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::packed::get_cov3d;
@if(PACKED && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::packed::get_position;
@if(PACKED && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::packed::get_color;
@if(PACKED && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::packed::get_visibility;
@if(PACKED && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::packed::get_opacity;
@if(PACKED && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::packed::get_rotation;
@if(PACKED && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::packed::get_scale;
@if(!PACKED && BUFFER_STORAGE && PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::planar::get_position;
@if(!PACKED && BUFFER_STORAGE && PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::planar::get_color;
@if(!PACKED && BUFFER_STORAGE && PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::planar::get_visibility;
@if(!PACKED && BUFFER_STORAGE && PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::planar::get_opacity;
@if(!PACKED && BUFFER_STORAGE && PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::planar::get_cov3d;
@if(!PACKED && BUFFER_STORAGE && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::planar::get_position;
@if(!PACKED && BUFFER_STORAGE && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::planar::get_color;
@if(!PACKED && BUFFER_STORAGE && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::planar::get_visibility;
@if(!PACKED && BUFFER_STORAGE && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::planar::get_opacity;
@if(!PACKED && BUFFER_STORAGE && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::planar::get_rotation;
@if(!PACKED && BUFFER_STORAGE && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::planar::get_scale;
@if(!PACKED && !BUFFER_STORAGE && BUFFER_TEXTURE && PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::texture::get_position;
@if(!PACKED && !BUFFER_STORAGE && BUFFER_TEXTURE && PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::texture::get_color;
@if(!PACKED && !BUFFER_STORAGE && BUFFER_TEXTURE && PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::texture::get_visibility;
@if(!PACKED && !BUFFER_STORAGE && BUFFER_TEXTURE && PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::texture::get_opacity;
@if(!PACKED && !BUFFER_STORAGE && BUFFER_TEXTURE && PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::texture::get_cov3d;
@if(!PACKED && !BUFFER_STORAGE && BUFFER_TEXTURE && PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::texture::location;
@if(!PACKED && !BUFFER_STORAGE && BUFFER_TEXTURE && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::texture::get_position;
@if(!PACKED && !BUFFER_STORAGE && BUFFER_TEXTURE && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::texture::get_color;
@if(!PACKED && !BUFFER_STORAGE && BUFFER_TEXTURE && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::texture::get_visibility;
@if(!PACKED && !BUFFER_STORAGE && BUFFER_TEXTURE && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::texture::get_opacity;
@if(!PACKED && !BUFFER_STORAGE && BUFFER_TEXTURE && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::texture::get_rotation;
@if(!PACKED && !BUFFER_STORAGE && BUFFER_TEXTURE && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::texture::get_scale;
@if(!PACKED && !BUFFER_STORAGE && BUFFER_TEXTURE && !PRECOMPUTE_COVARIANCE_3D) import bevy_gaussian_splatting::render::texture::location;


// Each `#ifdef A ... #else ifdef B` chain of the original became statements
// guarded by `@if(A)`, `@if(!A && B)` and so on: WESL attaches conditions to
// whole statements and declarations, not to spans of lines.

@if(BUFFER_STORAGE)
@group(3) @binding(0) var<storage, read> sorted_entries: array<Entry>;
@if(BUFFER_STORAGE)
fn get_entry(index: u32) -> Entry {
    return sorted_entries[index];
}

@if(!BUFFER_STORAGE && BUFFER_TEXTURE)
@group(3) @binding(0) var sorted_entries: texture_2d<u32>;
@if(!BUFFER_STORAGE && BUFFER_TEXTURE)
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

@if(WEBGL2)
struct GaussianVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @if(GAUSSIAN_2D) @location(2) local_to_pixel_u: vec3<f32>,
    @if(GAUSSIAN_2D) @location(3) local_to_pixel_v: vec3<f32>,
    @if(GAUSSIAN_2D) @location(4) local_to_pixel_w: vec3<f32>,
    @if(GAUSSIAN_2D) @location(5) mean_2d: vec2<f32>,
    @if(GAUSSIAN_2D) @location(6) radius: vec2<f32>,
    @if(!GAUSSIAN_2D && GAUSSIAN_3D) @location(2) conic: vec3<f32>,
    @if(!GAUSSIAN_2D && GAUSSIAN_3D) @location(3) major_minor: vec2<f32>,
    @if(!GAUSSIAN_2D && !GAUSSIAN_3D && GAUSSIAN_4D) @location(2) conic: vec3<f32>,
    @if(!GAUSSIAN_2D && !GAUSSIAN_3D && GAUSSIAN_4D) @location(3) major_minor: vec2<f32>,
};

@if(!WEBGL2)
struct GaussianVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) color: vec4<f32>,
    @location(1) @interpolate(linear) uv: vec2<f32>,
    @if(GAUSSIAN_2D) @location(2) @interpolate(flat) local_to_pixel_u: vec3<f32>,
    @if(GAUSSIAN_2D) @location(3) @interpolate(flat) local_to_pixel_v: vec3<f32>,
    @if(GAUSSIAN_2D) @location(4) @interpolate(flat) local_to_pixel_w: vec3<f32>,
    @if(GAUSSIAN_2D) @location(5) @interpolate(flat) mean_2d: vec2<f32>,
    @if(GAUSSIAN_2D) @location(6) @interpolate(flat) radius: vec2<f32>,
    @if(!GAUSSIAN_2D && GAUSSIAN_3D) @location(2) @interpolate(flat) conic: vec3<f32>,
    @if(!GAUSSIAN_2D && GAUSSIAN_3D) @location(3) @interpolate(linear) major_minor: vec2<f32>,
    @if(!GAUSSIAN_2D && !GAUSSIAN_3D && GAUSSIAN_4D) @location(2) @interpolate(flat) conic: vec3<f32>,
    @if(!GAUSSIAN_2D && !GAUSSIAN_3D && GAUSSIAN_4D) @location(3) @interpolate(linear) major_minor: vec2<f32>,
};

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

    @if(DRAW_SELECTED) discard_quad |= get_visibility(splat_index) < 0.5;

    @if(!GAUSSIAN_4D) let projected_position = world_to_clip(transformed_position);
    @if(!GAUSSIAN_4D) discard_quad |= !in_frustum(projected_position.xyz);

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

    @if(OPACITY_ADAPTIVE_RADIUS) let cutoff = sqrt(max(9.0 + 2.0 * log(opacity), 0.000001));
    @if(!OPACITY_ADAPTIVE_RADIUS) let cutoff = 3.0;

    @if(GAUSSIAN_2D) let surfel = compute_cov2d_surfel(
        transformed_position,
        splat_index,
        cutoff,
    );

    @if(GAUSSIAN_2D) output.local_to_pixel_u = surfel.local_to_pixel[0];
    @if(GAUSSIAN_2D) output.local_to_pixel_v = surfel.local_to_pixel[1];
    @if(GAUSSIAN_2D) output.local_to_pixel_w = surfel.local_to_pixel[2];
    @if(GAUSSIAN_2D) output.mean_2d = surfel.mean_2d;

    @if(GAUSSIAN_2D) let bb = get_bounding_box_cov2d(
        surfel.extent,
        quad_offset,
        cutoff,
    );
    @if(GAUSSIAN_2D) output.radius = bb.zw;

    @if(!GAUSSIAN_2D && GAUSSIAN_3D) let gaussian_cov2d = compute_cov2d_3dgs(
        transformed_position,
        splat_index,
    );

    @if(!GAUSSIAN_2D && !GAUSSIAN_3D && GAUSSIAN_4D) let gaussian_4d = conditional_cov3d(
        transformed_position,
        splat_index,
        gaussian_uniforms.time,
    );

    @if(!GAUSSIAN_2D && !GAUSSIAN_3D && GAUSSIAN_4D) if !gaussian_4d.mask {
        output.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        output.position = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return output;
    }

    @if(!GAUSSIAN_2D && !GAUSSIAN_3D && GAUSSIAN_4D) let position_t = vec4<f32>(position.xyz + gaussian_4d.delta_mean, 1.0);
    @if(!GAUSSIAN_2D && !GAUSSIAN_3D && GAUSSIAN_4D) transformed_position = (gaussian_uniforms.transform * position_t).xyz;
    // TODO: set previous_transformed_position based on temporal position delta
    @if(!GAUSSIAN_2D && !GAUSSIAN_3D && GAUSSIAN_4D) let projected_position = world_to_clip(transformed_position);

    @if(!GAUSSIAN_2D && !GAUSSIAN_3D && GAUSSIAN_4D) if !in_frustum(projected_position.xyz) {
        output.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        output.position = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return output;
    }

    @if(!GAUSSIAN_2D && !GAUSSIAN_3D && GAUSSIAN_4D) opacity = opacity * gaussian_4d.opacity_modifier;

    @if(!GAUSSIAN_2D && !GAUSSIAN_3D && GAUSSIAN_4D) let gaussian_cov2d = cov2d(
        transformed_position,
        gaussian_4d.cov3d,
    );

    @if(!GAUSSIAN_2D) let bb = get_bounding_box_clip(
        gaussian_cov2d,
        quad_offset,
        cutoff,
    );

    @if(!GAUSSIAN_2D && USE_AABB) let det = gaussian_cov2d.x * gaussian_cov2d.z - gaussian_cov2d.y * gaussian_cov2d.y;
    @if(!GAUSSIAN_2D && USE_AABB) let det_inv = 1.0 / det;
    @if(!GAUSSIAN_2D && USE_AABB) let conic = vec3<f32>(
        gaussian_cov2d.z * det_inv,
        -gaussian_cov2d.y * det_inv,
        gaussian_cov2d.x * det_inv
    );
    @if(!GAUSSIAN_2D && USE_AABB) output.conic = conic;
    @if(!GAUSSIAN_2D && USE_AABB) output.major_minor = bb.zw;

    var rgb = vec3<f32>(0.0);

    // TODO: RASTERIZE_ACCELERATION
    @if(RASTERIZE_CLASSIFICATION) let ray_direction_world = normalize(transformed_position - view.world_position);
    @if(RASTERIZE_CLASSIFICATION) let ray_direction_local = world_to_local_direction(ray_direction_world, gaussian_uniforms.transform);

    @if(RASTERIZE_CLASSIFICATION && GAUSSIAN_3D_STRUCTURE) rgb = get_color(splat_index, ray_direction_local);
    @if(RASTERIZE_CLASSIFICATION && !GAUSSIAN_3D_STRUCTURE && GAUSSIAN_4D) rgb = get_color(splat_index, gaussian_4d.dir_t, ray_direction_local);

    @if(RASTERIZE_CLASSIFICATION) rgb = class_to_rgb(
        get_visibility(splat_index),
        rgb,
    );

    // TODO: unbiased depth rendering, see: https://zju3dv.github.io/pgsr/
    @if(!RASTERIZE_CLASSIFICATION && RASTERIZE_DEPTH) let first_position = vec4<f32>(get_position(get_entry(1u).value), 1.0);
    @if(!RASTERIZE_CLASSIFICATION && RASTERIZE_DEPTH) let last_position = vec4<f32>(get_position(get_entry(gaussian_uniforms.count - 1u).value), 1.0);

    @if(!RASTERIZE_CLASSIFICATION && RASTERIZE_DEPTH) let min_position = (gaussian_uniforms.transform * last_position).xyz;
    @if(!RASTERIZE_CLASSIFICATION && RASTERIZE_DEPTH) let max_position = (gaussian_uniforms.transform * first_position).xyz;

    @if(!RASTERIZE_CLASSIFICATION && RASTERIZE_DEPTH) let camera_position = view.world_position;

    @if(!RASTERIZE_CLASSIFICATION && RASTERIZE_DEPTH) let min_distance = length(min_position - camera_position);
    @if(!RASTERIZE_CLASSIFICATION && RASTERIZE_DEPTH) let max_distance = length(max_position - camera_position);

    @if(!RASTERIZE_CLASSIFICATION && RASTERIZE_DEPTH) let depth = length(transformed_position - camera_position);
    @if(!RASTERIZE_CLASSIFICATION && RASTERIZE_DEPTH) rgb = depth_to_rgb(
        depth,
        min_distance,
        max_distance,
    );

    // TODO: support rotation decomposition for 4d gaussians
    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && RASTERIZE_NORMAL) let R = get_rotation_matrix(get_rotation(splat_index));
    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && RASTERIZE_NORMAL) let S = get_scale_matrix(get_scale(splat_index));
    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && RASTERIZE_NORMAL) let T = mat3x3<f32>(
        gaussian_uniforms.transform[0].xyz,
        gaussian_uniforms.transform[1].xyz,
        gaussian_uniforms.transform[2].xyz,
    );
    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && RASTERIZE_NORMAL) let L = T * S * R;

    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && RASTERIZE_NORMAL) let local_normal = vec4<f32>(L[2], 0.0);
    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && RASTERIZE_NORMAL) let world_normal = view.view_from_world * local_normal;

    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && RASTERIZE_NORMAL) let t = normalize(world_normal);

    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && RASTERIZE_NORMAL) rgb = vec3<f32>(
        0.5 * (t.x + 1.0),
        0.5 * (t.y + 1.0),
        0.5 * (t.z + 1.0)
    );

    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && !RASTERIZE_NORMAL && RASTERIZE_OPTICAL_FLOW) let motion_vector = calculate_motion_vector(
        transformed_position,
        previous_transformed_position,
    );

    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && !RASTERIZE_NORMAL && RASTERIZE_OPTICAL_FLOW) rgb = optical_flow_to_rgb(motion_vector);

    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && !RASTERIZE_NORMAL && !RASTERIZE_OPTICAL_FLOW && RASTERIZE_POSITION) rgb = (transformed_position - gaussian_uniforms.min.xyz) / (gaussian_uniforms.max.xyz - gaussian_uniforms.min.xyz);

    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && !RASTERIZE_NORMAL && !RASTERIZE_OPTICAL_FLOW && !RASTERIZE_POSITION && RASTERIZE_VELOCITY) let time_delta = 1e-3;
    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && !RASTERIZE_NORMAL && !RASTERIZE_OPTICAL_FLOW && !RASTERIZE_POSITION && RASTERIZE_VELOCITY) let future_gaussian_4d = conditional_cov3d(
        transformed_position,
        splat_index,
        gaussian_uniforms.time + time_delta,
    );
    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && !RASTERIZE_NORMAL && !RASTERIZE_OPTICAL_FLOW && !RASTERIZE_POSITION && RASTERIZE_VELOCITY) let position_delta = future_gaussian_4d.delta_mean - gaussian_4d.delta_mean;
    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && !RASTERIZE_NORMAL && !RASTERIZE_OPTICAL_FLOW && !RASTERIZE_POSITION && RASTERIZE_VELOCITY) let velocity = position_delta / time_delta;
    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && !RASTERIZE_NORMAL && !RASTERIZE_OPTICAL_FLOW && !RASTERIZE_POSITION && RASTERIZE_VELOCITY) let velocity_magnitude = length(velocity);
    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && !RASTERIZE_NORMAL && !RASTERIZE_OPTICAL_FLOW && !RASTERIZE_POSITION && RASTERIZE_VELOCITY) let velocity_normalized = normalize(velocity);

    // TODO: magnitude normalization
    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && !RASTERIZE_NORMAL && !RASTERIZE_OPTICAL_FLOW && !RASTERIZE_POSITION && RASTERIZE_VELOCITY) let min_magnitude = 1.0;
    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && !RASTERIZE_NORMAL && !RASTERIZE_OPTICAL_FLOW && !RASTERIZE_POSITION && RASTERIZE_VELOCITY) let max_magnitude = 2.0;

    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && !RASTERIZE_NORMAL && !RASTERIZE_OPTICAL_FLOW && !RASTERIZE_POSITION && RASTERIZE_VELOCITY) let scaled_mag = clamp(
        (velocity_magnitude - min_magnitude) / (max_magnitude - min_magnitude),
        0.0,
        1.0
    );

    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && !RASTERIZE_NORMAL && !RASTERIZE_OPTICAL_FLOW && !RASTERIZE_POSITION && RASTERIZE_VELOCITY) if scaled_mag < 1e-2 {
        opacity = 0.0;
    }

    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && !RASTERIZE_NORMAL && !RASTERIZE_OPTICAL_FLOW && !RASTERIZE_POSITION && RASTERIZE_VELOCITY) let base_color = 0.5 * (velocity_normalized + vec3<f32>(1.0, 1.0, 1.0));
    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && !RASTERIZE_NORMAL && !RASTERIZE_OPTICAL_FLOW && !RASTERIZE_POSITION && RASTERIZE_VELOCITY) rgb = base_color * scaled_mag;

    // TODO: verify color benefit for ray_direction computed at quad verticies instead of gaussian center (same as current complexity)
    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && !RASTERIZE_NORMAL && !RASTERIZE_OPTICAL_FLOW && !RASTERIZE_POSITION && !RASTERIZE_VELOCITY && RASTERIZE_COLOR) let ray_direction_world = normalize(transformed_position - view.world_position);
    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && !RASTERIZE_NORMAL && !RASTERIZE_OPTICAL_FLOW && !RASTERIZE_POSITION && !RASTERIZE_VELOCITY && RASTERIZE_COLOR) let ray_direction_local = world_to_local_direction(ray_direction_world, gaussian_uniforms.transform);

    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && !RASTERIZE_NORMAL && !RASTERIZE_OPTICAL_FLOW && !RASTERIZE_POSITION && !RASTERIZE_VELOCITY && RASTERIZE_COLOR && GAUSSIAN_3D_STRUCTURE) rgb = get_color(splat_index, ray_direction_local);
    @if(!RASTERIZE_CLASSIFICATION && !RASTERIZE_DEPTH && !RASTERIZE_NORMAL && !RASTERIZE_OPTICAL_FLOW && !RASTERIZE_POSITION && !RASTERIZE_VELOCITY && RASTERIZE_COLOR && !GAUSSIAN_3D_STRUCTURE && GAUSSIAN_4D) rgb = get_color(splat_index, gaussian_4d.dir_t, ray_direction_local);

    output.color = vec4<f32>(
        rgb,
        opacity * gaussian_uniforms.global_opacity,
    );

    @if(HIGHLIGHT_SELECTED) if (get_visibility(splat_index) > 0.5) {
        output.color = vec4<f32>(0.3, 1.0, 0.1, 1.0);
    }

    output.uv = quad_offset;
    output.position = vec4<f32>(
        projected_position.xy + bb.xy,
        projected_position.zw,
    );

    return output;
}

@fragment
fn fs_main(input: GaussianVertexOutput) -> @location(0) vec4<f32> {
    @if(USE_AABB && GAUSSIAN_2D) let radius = input.radius;
    @if(USE_AABB && GAUSSIAN_2D) let mean_2d = input.mean_2d;
    @if(USE_AABB && GAUSSIAN_2D) let aspect = vec2<f32>(
        1.0,
        view.viewport.z / view.viewport.w,
    );
    @if(USE_AABB && GAUSSIAN_2D) let pixel_coord = input.uv * radius * aspect + mean_2d;

    @if(USE_AABB && GAUSSIAN_2D) let power = surfel_fragment_power(
        mat3x3<f32>(
            input.local_to_pixel_u,
            input.local_to_pixel_v,
            input.local_to_pixel_w,
        ),
        pixel_coord,
        mean_2d,
    );
    @if(USE_AABB && !GAUSSIAN_2D && GAUSSIAN_3D) let d = -input.major_minor;
    @if(USE_AABB && !GAUSSIAN_2D && GAUSSIAN_3D) let conic = input.conic;
    @if(USE_AABB && !GAUSSIAN_2D && GAUSSIAN_3D) let power = -0.5 * (conic.x * d.x * d.x + conic.z * d.y * d.y) + conic.y * d.x * d.y;
    @if(USE_AABB && !GAUSSIAN_2D && !GAUSSIAN_3D && GAUSSIAN_4D) let d = -input.major_minor;
    @if(USE_AABB && !GAUSSIAN_2D && !GAUSSIAN_3D && GAUSSIAN_4D) let conic = input.conic;
    @if(USE_AABB && !GAUSSIAN_2D && !GAUSSIAN_3D && GAUSSIAN_4D) let power = -0.5 * (conic.x * d.x * d.x + conic.z * d.y * d.y) + conic.y * d.x * d.y;

    @if(USE_AABB) if (power > 0.0) {
        discard;
    }

    @if(USE_OBB) let sigma = 1.0 / 3.0;
    @if(USE_OBB) let sigma_squared = 2.0 * sigma * sigma;
    @if(USE_OBB) let distance_squared = dot(input.uv, input.uv);

    @if(USE_OBB) let power = -distance_squared / sigma_squared;

    @if(USE_OBB) if (distance_squared > 3.0 * 3.0) {
        discard;
    }

    @if(VISUALIZE_BOUNDING_BOX) let uv = input.uv * 0.5 + 0.5;
    @if(VISUALIZE_BOUNDING_BOX) let edge_width = 0.08;
    @if(VISUALIZE_BOUNDING_BOX) if (
        (uv.x < edge_width || uv.x > 1.0 - edge_width) ||
        (uv.y < edge_width || uv.y > 1.0 - edge_width)
    ) {
        return vec4<f32>(0.3, 1.0, 0.1, 1.0);
    }

    let alpha = min(exp(power) * input.color.a, 0.999);

    // TODO: round alpha to terminate depth test?

    // renzora patch(depth-write): depth-only pass: keep depth writes to the
    // solid core of each splat and discard the soft fringe, so depth-reading
    // post passes (volumetric fog, DoF) see the splat surface instead of the
    // far plane, without cutting hard quad silhouettes out of the sky. Color
    // writes are masked off at the pipeline level; the return value is unused.
    // 0.3 keeps faded splats (low CloudSettings::global_opacity) from writing
    // depth once they are mostly transparent.
    @if(DEPTH_WRITE_PASS) if (alpha < 0.3) {
        discard;
    }
    @if(DEPTH_WRITE_PASS) return vec4<f32>(0.0);
    @if(!DEPTH_WRITE_PASS) return vec4<f32>(
        input.color.rgb * alpha,
        alpha,
    );
}
