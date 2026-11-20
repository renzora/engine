import bevy_pbr::deferred::types::unpack_24bit_normal;
import bevy_pbr::render::rgb9e5::rgb9e5_to_vec3_;
import bevy_render::utils::octahedral_decode;
import bevy_render::view::{View, depth_ndc_to_view_z};
import package::scene::bindings::{MIN_PERCEPTUAL_ROUGHNESS, ResolvedMaterial};

struct ResolvedGPixel {
    world_position: vec3<f32>,
    world_normal: vec3<f32>,
    material: ResolvedMaterial,
}

/// RENZORA PATCH: how far, in pixels, a G-buffer surface is lifted off what was
/// drawn before rays leave it (see `gpixel_resolve`). Twice virtual geometry's
/// default level-of-detail error of one pixel.
const SURFACE_LIFT_PIXELS = 2.0;

fn gpixel_resolve(gpixel: vec4<u32>, depth: f32, pixel_id: vec2<u32>, view_size: vec2<f32>, world_from_clip: mat4x4<f32>) -> ResolvedGPixel {
    let drawn_position = reconstruct_world_position(pixel_id, depth, view_size, world_from_clip);
    let world_normal = octahedral_decode(unpack_24bit_normal(gpixel.a));

    // RENZORA PATCH: the surface the camera sees is not always the one the rays
    // hit. Virtual geometry draws a mesh at whatever simplified level keeps it
    // within a pixel of the full detail, while the ray-traced scene holds the
    // full detail. Where that sits above the drawn surface (the grooves between
    // paving stones seen from low down, a curved awning from far away), a ray
    // leaving the drawn surface starts underneath it, hits it at once, and the
    // pixel is shadowed: black patches that grow with distance. The gap is
    // bounded by the pixel's own footprint, measured here as the distance to the
    // neighbouring pixel at the same depth, so lifting the surface by a couple of
    // footprints along its normal clears it at any distance. Ordinary meshes,
    // drawn from the triangles that are traced, lose at most a pixel or two of
    // contact shadow.
    //
    // Not for a sky pixel (depth 0), which reuse passes resolve too, as a
    // neighbour: its position is infinite, and `pixel_dissimilar` rejects it
    // only because comparisons with infinity come out true. A lift computed from
    // two infinities is NaN, every comparison with NaN is false, and the sky's
    // neighbour passed as the same surface, spreading NaN over the whole image
    // (everything but emissive surfaces went black).
    let neighbour = reconstruct_world_position_at(vec2<f32>(pixel_id) + vec2(1.5, 0.5), depth, view_size, world_from_clip);
    let lift = select(0.0, distance(drawn_position, neighbour) * SURFACE_LIFT_PIXELS, depth > 0.0);
    let world_position = drawn_position + world_normal * lift;

    let base_rough = unpack4x8unorm(gpixel.r);
    let base_color = pow(base_rough.rgb, vec3(2.2));
    // RENZORA PATCH: floored like a ray hit's; see MIN_PERCEPTUAL_ROUGHNESS.
    let perceptual_roughness = clamp(base_rough.a, MIN_PERCEPTUAL_ROUGHNESS, 1.0);
    let roughness = perceptual_roughness * perceptual_roughness;
    let props = unpack4x8unorm(gpixel.b);
    let reflectance = props.r;
    let metallic = props.g;
    let emissive = rgb9e5_to_vec3_(gpixel.g);
    let material = ResolvedMaterial(base_color, emissive, reflectance, perceptual_roughness, roughness, metallic, false);

    return ResolvedGPixel(world_position, world_normal, material);
}

fn reconstruct_world_position(pixel_id: vec2<u32>, depth: f32, view_size: vec2<f32>, world_from_clip: mat4x4<f32>) -> vec3<f32> {
    return reconstruct_world_position_at(vec2<f32>(pixel_id) + 0.5, depth, view_size, world_from_clip);
}

/// RENZORA PATCH: `reconstruct_world_position` at any point on the screen, in
/// pixels, not only a pixel's centre.
fn reconstruct_world_position_at(pixel: vec2<f32>, depth: f32, view_size: vec2<f32>, world_from_clip: mat4x4<f32>) -> vec3<f32> {
    let uv = pixel / view_size;
    let xy_ndc = (uv - vec2(0.5)) * vec2(2.0, -2.0);
    let world_pos = world_from_clip * vec4(xy_ndc, depth, 1.0);
    return world_pos.xyz / world_pos.w;
}

/// Reject if tangent plane differs more than 0.3% or angle between normals more than 90 degrees
fn pixel_dissimilar(depth: f32, world_position: vec3<f32>, other_world_position: vec3<f32>, normal: vec3<f32>, other_normal: vec3<f32>, view: View) -> bool {
    // https://developer.download.nvidia.com/video/gputechconf/gtc/2020/presentations/s22699-fast-denoising-with-self-stabilizing-recurrent-blurs.pdf#page=45
    let tangent_plane_distance = abs(dot(normal, other_world_position - world_position));
    let view_z = -depth_ndc_to_view_z(depth, view.clip_from_view, view.view_from_clip);

    return tangent_plane_distance / view_z > 0.003 || dot(normal, other_normal) < 0.0;
}

fn permute_pixel(pixel_id: vec2<u32>, frame_index: u32, view_size: vec2<f32>) -> vec2<u32> {
    let r = frame_index;
    let offset = vec2<i32>(i32(r & 3u), i32((r >> 2u) & 3u));
    var shifted_pixel_id = vec2<i32>(pixel_id) + offset;
    shifted_pixel_id ^= vec2<i32>(3);
    shifted_pixel_id -= offset;
    if any(shifted_pixel_id < vec2<i32>(0)) || any(shifted_pixel_id >= vec2<i32>(view_size)) {
        return pixel_id;
    }
    return vec2<u32>(shifted_pixel_id);
}
