// The sky probe's compute entry: marches the cloud deck into the environment
// cubemap, on top of the sky the atmosphere baked into it.
//
// Not a module of its own. `probe.rs` appends this to `clouds.wesl`, so the
// march here is the very function the dome draws with, and the reflections can
// never show a different deck from the one in the sky. That file's material
// bindings are moved to group 0 on the way; the probe's own are group 1.

struct CloudProbe {
    // xyz = the camera's eye in world metres, w unused.
    eye: vec4<f32>,
    // The first cube face to write this dispatch; one dispatch covers
    // `workgroups.z` faces from here.
    first_face: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

@group(1) @binding(0) var<uniform> probe: CloudProbe;
// The atmosphere's cubemap, as six layers.
@group(1) @binding(1) var sky_in: texture_2d_array<f32>;
@group(1) @binding(2) var sky_out: texture_storage_2d_array<rgba16float, write>;

// The cube-face direction for a texel, matching the atmosphere bake
// (`bevy_pbr::render::utils::sample_cube_dir`, then z flipped for cubemaps
// being left-handed), so each texel holds cloud from the direction its sky
// came from.
fn probe_cube_dir(uv: vec2<f32>, face: u32) -> vec3<f32> {
    let c = 2.0 * uv - 1.0;
    var dir: vec3<f32>;
    switch face {
        case 0u: { dir = vec3<f32>(1.0, -c.y, -c.x); }
        case 1u: { dir = vec3<f32>(-1.0, -c.y, c.x); }
        case 2u: { dir = vec3<f32>(c.x, 1.0, c.y); }
        case 3u: { dir = vec3<f32>(c.x, -1.0, -c.y); }
        case 4u: { dir = vec3<f32>(c.x, -c.y, 1.0); }
        default: { dir = vec3<f32>(-c.x, -c.y, -1.0); }
    }
    dir.z = -dir.z;
    return normalize(dir);
}

@compute @workgroup_size(8, 8, 1)
fn probe_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let dims = textureDimensions(sky_out);
    let face = probe.first_face + id.z;
    if id.x >= dims.x || id.y >= dims.y || face >= 6u {
        return;
    }

    let uv = (vec2<f32>(id.xy) + 0.5) / vec2<f32>(dims);
    let dir = probe_cube_dir(uv, face);
    let sky = textureLoad(sky_in, vec2<i32>(id.xy), i32(face), 0);

    // A fixed dither, not the dome's moving one: nothing averages this
    // texture over frames, and what reads it (a blurred reflection, a filtered
    // sky light, a denoised ray) spreads the grain out anyway.
    let dither = fract(52.9829189 * fract(dot(vec2<f32>(id.xy), vec2<f32>(0.06711056, 0.00583715))));
    let c = march_clouds(probe.eye.xyz, dir, dither);

    // The cubemap holds sky radiance with no exposure applied, so the cloud
    // goes in at that scale too: its authored colour is a value seen at
    // `REFERENCE_EXPOSURE`. Premultiplied, so the sky behind is dimmed by the
    // cloud's coverage and the cloud's own light added.
    let cloud = c.rgb / REFERENCE_EXPOSURE;
    textureStore(sky_out, vec2<i32>(id.xy), i32(face), vec4<f32>(sky.rgb * (1.0 - c.a) + cloud, 1.0));
}
