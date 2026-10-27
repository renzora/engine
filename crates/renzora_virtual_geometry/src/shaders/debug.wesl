//! The virtual geometry debug view: colours every pixel virtual geometry drew
//! by cluster, triangle, level of detail or instance, and dims everything
//! else.
//!
//! Runs at the end of the main pass, before anti-aliasing (see
//! `render/debug.rs` for why), on editor viewports only. A pixel where an ordinary
//! mesh ended up in front still holds a cluster in the visibility buffer, so
//! the cluster's depth at that pixel is checked against the prepass depth (which
//! holds both kinds of geometry) and only the front surface is coloured.

import super::types::{VG_NOTHING, VG_SIGN_DETERMINANT};
import super::bindings::{vg_visibility, vg_visible, vg_instances, vg_clusters, vg_triangle_vertex, vg_world_position};
import bevy_render::view::View;
import bevy_render::maths::affine3_to_square;

struct DebugParams {
    /// 1 clusters, 2 triangles, 3 level of detail, 4 instances, 5 residency.
    mode: u32,
    /// The cull pass's error threshold, in pixels.
    lod_threshold: f32,
    // Scalars, not a vec2: keeps this 16 bytes, as the binding declares.
    _pad1: u32,
    _pad2: u32,
}

/// The cull pass's projection of an error to pixels (`cull.wesl`), so the
/// residency view sees the cut exactly as culling chose it.
fn projected_error(sphere: vec4<f32>, error: f32, world_from_local: mat4x4<f32>, scale: f32) -> f32 {
    if error >= 3.0e38 {
        return 3.0e38;
    }
    let center = (world_from_local * vec4(sphere.xyz, 1.0)).xyz;
    let radius = sphere.w * scale;
    let world_error = error * scale;
    let proj = view.clip_from_view[1][1];
    let height = view.viewport.w;
    if view.clip_from_view[3][3] == 1.0 {
        return world_error * proj * 0.5 * height;
    }
    let znear = view.clip_from_view[3][2];
    let distance = max(length(center - view.world_position) - radius, znear);
    return world_error / distance * proj * 0.5 * height;
}

@group(1) @binding(0) var<uniform> view: View;
@group(1) @binding(1) var scene: texture_2d<f32>;
@group(1) @binding(2) var<uniform> params: DebugParams;
@if(DEPTH_TEST)
@group(1) @binding(3) var prepass_depth: texture_depth_2d;

fn hash(x: u32) -> u32 {
    var h = x * 747796405u + 2891336453u;
    h = ((h >> ((h >> 28u) + 4u)) ^ h) * 277803737u;
    return (h >> 22u) ^ h;
}

/// A saturated colour for an id: a hashed hue at fixed saturation and value,
/// so neighbouring ids never come out as two near-greys.
fn id_colour(id: u32) -> vec3<f32> {
    let h = hash(id);
    let hue = f32(h & 1023u) / 1024.0;
    let value = 0.75 + 0.25 * f32((h >> 10u) & 3u) / 3.0;
    let rgb = clamp(abs(fract(hue + vec3(0.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - 3.0) - 1.0, vec3(0.0), vec3(1.0));
    return srgb_to_linear(mix(vec3(1.0), rgb, 0.85) * value);
}

/// The frame is linear at this point and encoded for display afterwards, so a
/// colour picked by eye is converted first or it comes out washed out.
fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return pow(c, vec3(2.2));
}

/// Level 0 (full detail) green, through yellow and orange, to red and purple
/// for the coarsest.
fn level_colour(level: u32) -> vec3<f32> {
    var palette = array<vec3<f32>, 8>(
        vec3(0.15, 0.85, 0.3),
        vec3(0.6, 0.9, 0.2),
        vec3(0.95, 0.9, 0.2),
        vec3(1.0, 0.65, 0.15),
        vec3(1.0, 0.35, 0.15),
        vec3(0.9, 0.15, 0.25),
        vec3(0.75, 0.2, 0.7),
        vec3(0.45, 0.3, 0.95),
    );
    return srgb_to_linear(palette[min(level, 7u)]);
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
}

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> VertexOutput {
    let uv = vec2<f32>(f32(index >> 1u), f32(index & 1u)) * 2.0;
    return VertexOutput(vec4(uv * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0));
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let coord = vec2<u32>(in.position.xy);
    let colour = textureLoad(scene, coord, 0);
    let dimmed = vec4(vec3(dot(colour.rgb, vec3(0.2126, 0.7152, 0.0722)) * 0.3), colour.a);

    let stored = textureLoad(vg_visibility, coord, 0).r;
    if stored == VG_NOTHING {
        // Nothing at all was drawn here (depth is still the cleared far
        // plane): it is sky, not ordinary geometry, so it keeps its colour
        // rather than being greyed out with the meshes.
        @if(DEPTH_TEST) {
            if textureLoad(prepass_depth, coord, 0) == 0.0 {
                return colour;
            }
        }
        return dimmed;
    }
    let packed = stored - 1u;
    let triangle = packed & 127u;
    let entry = vg_visible[packed >> 7u];
    let instance = vg_instances[entry.x];
    let cluster = vg_clusters[entry.y];

    var world: array<vec3<f32>, 3>;
    var ndc: array<vec3<f32>, 3>;
    for (var i = 0u; i < 3u; i++) {
        let w = vg_world_position(instance, vg_triangle_vertex(cluster, triangle, i));
        let clip = view.clip_from_world * vec4(w, 1.0);
        world[i] = w;
        ndc[i] = clip.xyz / clip.w;
    }

    @if(DEPTH_TEST) {
        // Depth in normalized device coordinates is affine across a
        // triangle on screen, so 2D barycentrics give it exactly.
        let p = (in.position.xy - view.viewport.xy) / view.viewport.zw * vec2(2.0, -2.0) + vec2(-1.0, 1.0);
        let e0 = ndc[1].xy - ndc[0].xy;
        let e1 = ndc[2].xy - ndc[0].xy;
        let d = p - ndc[0].xy;
        let det = e0.x * e1.y - e0.y * e1.x;
        if abs(det) > 1e-12 {
            let b1 = (d.x * e1.y - d.y * e1.x) / det;
            let b2 = (e0.x * d.y - e0.y * d.x) / det;
            let depth = ndc[0].z + b1 * (ndc[1].z - ndc[0].z) + b2 * (ndc[2].z - ndc[0].z);
            // Reversed z: nearer is larger. Something in front means the
            // prepass depth is clearly larger than this surface's.
            let front = textureLoad(prepass_depth, coord, 0);
            if front > depth + max(depth, 1e-7) * 1e-3 {
                return dimmed;
            }
        }
    }

    var base: vec3<f32>;
    switch params.mode {
        case 2u: { base = id_colour(entry.y * 131u + triangle); }
        case 3u: { base = level_colour(cluster.level); }
        case 4u: { base = id_colour(entry.x + 7919u); }
        case 5u: {
            // Drawn although its own error is over the threshold: it can only
            // be standing in for finer clusters that are not resident.
            let world_from_local = affine3_to_square(instance.world_from_local);
            let error = projected_error(cluster.lod, cluster.lod_error, world_from_local, instance.max_scale);
            base = srgb_to_linear(select(vec3(0.2, 0.85, 0.3), vec3(0.95, 0.2, 0.15), error > params.lod_threshold));
        }
        default: { base = id_colour(entry.y); }
    }

    // Shade by how squarely the face looks at the camera, so the form reads.
    var normal = normalize(cross(world[1] - world[0], world[2] - world[0]));
    if (instance.flags & VG_SIGN_DETERMINANT) == 0u {
        normal = -normal;
    }
    let facing = abs(dot(normal, normalize(view.world_position - world[0])));
    return vec4(base * (0.35 + 0.65 * facing), colour.a);
}
