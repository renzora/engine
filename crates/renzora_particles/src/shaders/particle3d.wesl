// 3D particles: one quad per particle, expanded here from its centre.
//
// Every vertex of a quad carries the particle's centre (location 0), its corner
// in -0.5..0.5 (location 1), colour, `vec4(size.x, size.y, rotation, frame)`, a
// stretch axis, `vec4(seed, life, 0, 0)` and its velocity (location 8). The CPU
// never sees the camera; the quad is turned to face it here, so the same mesh
// is right in every viewport that draws it.
//
// Facing (params.mode.y):
//   0  camera billboard: the quad lies in the view plane.
//   1  velocity aligned: height along the velocity, face turned toward the
//      camera around it (sparks, streaks, flame tongues).
//   2  fixed axis: as 1, around an axis from the effect file.
//   3  ribbon: each vertex is a point of a strip, the axis the strip's
//      direction there and size.x its width; the vertex is pushed across the
//      direction, toward the camera, and corner.y runs head (-0.5) to tail.
//   4  tube: a ribbon whose vertices go round the strip instead of across it,
//      corner.x the way round (`place_tube`).
//
// Blend (params.mode.x, read as an integer by `blend_mode`) decides what the
// fragment writes, to match the pipeline Bevy picked from the material's
// AlphaMode:
//   0  alpha:            straight colour, ALPHA_BLENDING
//   1  additive:         premultiplied colour with alpha 0, PREMULTIPLIED blending
//   2  premultiplied:    the texture is already premultiplied
//   3  opaque:           straight colour, alpha written as 1, depth written
//   4  alpha mask:       as opaque, discarding below params.surface.w
//   5  multiply:         premultiplied colour, Bevy's multiply blend state
//   6  alpha to coverage: the alpha sharpened around the cutoff, depth written
//
// Shading (params.shading.x): 0 sprite, 1 procedural flame, 2 procedural smoke.
//
// MESH_PARTICLES: each particle is a copy of a mesh instead of a quad. Every
// vertex of a copy carries the particle (as a quad corner does) plus its own
// position and normal in the source mesh (locations 6 and 7, or 1 and 2 on
// the GPU path), and its UV as the corner, so flipbooks and erosion run over
// the mesh's UVs. The mesh is turned by `mesh_orientation` (a random tumble,
// along the velocity, or about a fixed axis, picked through the facing value
// the renderer sends for it) and scaled by vec3(size.x, size.y, size.x).
//
// Lit (params.surface.x): the colour is shaded by Bevy's apply_pbr_lighting
// with the view's lights, shadows, ambient and environment map. A quad is a
// card facing the camera; a mesh is lit by its own normals.
//
// Screen-space size (params.shape.y): size is in physical pixels, turned into
// world units at the particle's depth, so it is constant on screen.
// The procedural code is shared with particle2d.wesl, between the SHARED
// markers below; tests/shaders.rs fails if the two copies drift apart.
//
// GPU_PARTICLES: the GPU backend draws with this same shader. Its mesh holds
// one quad per pool slot and nothing else (corner and slot in location 0),
// and `fetch` pulls the slot's particle out of the simulation's storage
// buffer and works out what the CPU path would have written into the
// vertex. From there on the two paths are one code path, which is the point:
// facing, flipbooks, procedural shading, soft particles and blending cannot
// differ between the backends. The fetch block is shared with
// particle2d.wesl too, between the GPU FETCH markers, and the Particle
// struct is checked against the simulation's own by tests/shaders.rs.
//
// PREPASS_PIPELINE: the depth, normal and motion-vector prepass and the shadow
// passes run this file too. The vertex expansion is the same functions (the
// VERTEX EXPANSION block, also copied into particle_surface.wesl, which draws a
// project `.material` on a particle), called with whatever view the pass
// renders, so a shadow view turns each quad toward the light. The prepass
// fragment discards what the main pass would and writes the normal and the
// motion vector. `globals` sits at binding 1 of the prepass view layout rather
// than 11, so it is declared here for that pipeline, as Bevy's own pbr.wesl
// and the material codegen's deferred header do.
//
// DEFERRED_PREPASS: the deferred G-buffer pass, which Bevy builds from the
// prepass pipeline for a solid particle on a deferred camera. The vertex is
// the prepass one; the fragment discards as the prepass does and packs the
// particle into the G-buffer (`particle_gbuffer`) for the deferred lighting
// pass, SSR and SSAO to read.

import bevy_pbr::render::mesh_functions;
import bevy_pbr::render::mesh_view_bindings::view;
@if(!PREPASS_PIPELINE) import bevy_pbr::render::mesh_view_bindings::globals;
@if(PREPASS_PIPELINE) import bevy_render::globals::Globals;
@if(PREPASS_PIPELINE && MOTION_VECTOR_PREPASS) import bevy_pbr::prepass::bindings::previous_view_uniforms;
@if(PREPASS_PIPELINE && PREPASS_FRAGMENT) import bevy_pbr::prepass::io::FragmentOutput;
@if(PREPASS_PIPELINE && DEFERRED_PREPASS) import bevy_pbr::deferred::functions::deferred_gbuffer_from_pbr_input;
import bevy_pbr::render::view_transformations;
import bevy_pbr::prepass::utils as prepass_utils;
import bevy_pbr::render::pbr_functions;
import bevy_pbr::render::pbr_types;
import bevy_pbr::render::mesh_types::MESH_FLAGS_SHADOW_RECEIVER_BIT;
@if(TONEMAP_IN_SHADER) import bevy_core_pipeline::tonemapping::tone_mapping;

@if(PREPASS_PIPELINE) @group(0) @binding(1) var<uniform> globals: Globals;

struct ParticleParams {
    // x columns, y rows of the sprite sheet, z 1 for a fixed-rate sheet
    sheet: vec4<f32>,
    // x blend, y facing, z texture (0 soft dot, 1 bound, 2 solid)
    mode: vec4<f32>,
    // x shading, y erosion, z soft-particle distance, w blend frames
    shading: vec4<f32>,
    // GPU only: the fixed axis
    axis: vec4<f32>,
    // GPU only: x draw sorted, y word index of the sorted list
    gpu: vec4<f32>,
    // x lit, y roughness, z metallic, w alpha-mask cutoff
    surface: vec4<f32>,
    // x mesh particles, y size in screen pixels
    shape: vec4<f32>,
    // x playback rate, y 1 for a world-space emitter (motion vectors)
    motion: vec4<f32>,
};

@group(3) @binding(0) var<uniform> params: ParticleParams;
@group(3) @binding(1) var particle_texture: texture_2d<f32>;
@group(3) @binding(2) var particle_sampler: sampler;

@if(!GPU_PARTICLES && !MESH_PARTICLES)
struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) center: vec3<f32>,
    @location(1) corner: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) size_rot_frame: vec4<f32>,
    @location(4) axis: vec3<f32>,
    @location(5) extra: vec4<f32>,
    @location(8) velocity: vec3<f32>,
};

// A mesh particle's vertex from the CPU: the quad vertex plus where the
// vertex sits in the source mesh.
@if(!GPU_PARTICLES && MESH_PARTICLES)
struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) center: vec3<f32>,
    @location(1) corner: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) size_rot_frame: vec4<f32>,
    @location(4) axis: vec3<f32>,
    @location(5) extra: vec4<f32>,
    @location(6) local: vec3<f32>,
    @location(7) normal: vec3<f32>,
    @location(8) velocity: vec3<f32>,
};

@if(GPU_PARTICLES && !MESH_PARTICLES)
struct Vertex {
    @builtin(instance_index) instance_index: u32,
    // corner in xy, pool slot in z
    @location(0) slot_corner: vec3<f32>,
};

// The GPU pool mesh of mesh particles: one copy of the source mesh per slot,
// its UV as the corner.
@if(GPU_PARTICLES && MESH_PARTICLES)
struct Vertex {
    @builtin(instance_index) instance_index: u32,
    // corner (the UV) in xy, pool slot in z
    @location(0) slot_corner: vec3<f32>,
    @location(1) local: vec3<f32>,
    @location(2) normal: vec3<f32>,
};

@if(GPU_PARTICLES) {
@group(3) @binding(3) var<storage, read> particles: array<Particle>;
@group(3) @binding(4) var<storage, read> particle_state: array<u32>;
}

// What a vertex says about its particle, from whichever path made it.
struct ParticleVertex {
    center: vec3<f32>,
    corner: vec2<f32>,
    color: vec4<f32>,
    size_rot_frame: vec4<f32>,
    axis: vec3<f32>,
    extra: vec4<f32>,
    // mesh space; zero for a ribbon point
    velocity: vec3<f32>,
};

@if(!GPU_PARTICLES)
fn fetch(v: Vertex) -> ParticleVertex {
    return ParticleVertex(v.center, v.corner, v.color, v.size_rot_frame, v.axis, v.extra, v.velocity);
}

// BEGIN GPU FETCH

// BEGIN PARTICLE STRUCT
// One particle, 96 bytes. A slot is free while `lifetime` is negative; a
// live particle's lifetime is sanitized to zero or more at birth.
struct Particle {
    position: vec3<f32>,
    age: f32,
    velocity: vec3<f32>,
    lifetime: f32,
    color: vec4<f32>,
    base_color: vec4<f32>,
    size: vec2<f32>,
    base_size: vec2<f32>,
    rotation: f32,
    angular_velocity: f32,
    id: u32,
    seed: f32,
};
// END PARTICLE STRUCT

// The simulation-to-mesh transform the CPU writes into the state header.
@if(GPU_PARTICLES)
fn to_mesh() -> mat4x4<f32> {
    var c: array<vec4<f32>, 4>;
    for (var i = 0u; i < 4u; i = i + 1u) {
        let b = 16u + i * 4u;
        c[i] = vec4<f32>(
            bitcast<f32>(particle_state[b]),
            bitcast<f32>(particle_state[b + 1u]),
            bitcast<f32>(particle_state[b + 2u]),
            bitcast<f32>(particle_state[b + 3u]),
        );
    }
    return mat4x4<f32>(c[0], c[1], c[2], c[3]);
}

// render/mod.rs's `frame`: the sprite-sheet frame at this age.
@if(GPU_PARTICLES)
fn sheet_frame(age: f32, lifetime: f32) -> f32 {
    let frames = max(params.sheet.x, 1.0) * max(params.sheet.y, 1.0);
    if frames <= 1.0 {
        return 0.0;
    }
    var life = 0.0;
    if lifetime > 0.0 {
        life = clamp(age / lifetime, 0.0, 1.0);
    }
    let blend = params.shading.w > 0.5;
    if params.sheet.z < 0.5 {
        if blend {
            return life * (frames - 1.0);
        }
        return min(floor(life * frames), frames - 1.0);
    }
    let f = age * max(params.sheet.w, 0.0);
    if blend {
        return f - frames * floor(f / frames);
    }
    let k = floor(f);
    return k - frames * floor(k / frames);
}

// A ribbon on the GPU (the trail hook's fetch half): one vertex of a Trail
// strip, a pool slot's own strip through its history ring, or of a Connect
// segment between two ranks of the sorted list, worked out into exactly what
// render/ribbon.rs writes for the CPU's strip vertex, so the ribbon and tube
// expansion that follows cannot tell the backends apart. The ribbon pool mesh
// (gpu/runtime.rs's RibbonMesh) gives each vertex (across, row, strip):
// `across` the side of a flat strip or the way round a tube; for a Trail,
// `row` the point (0 the particle, then its ring newest first) and `strip`
// the slot; for a Connect ribbon, `row` 0 or 1 for the segment's two ends and
// `strip` the segment, joining ranks `strip` and `strip + 1`.
//
// params.gpu.z is the history's first word and params.gpu.w its points (0 for
// Connect); params.shape.z and .w the tail's width and alpha share; for
// Connect, params.axis.w the sort keys' first word (the strip in the top
// byte) and header words 32 on each strip's live count.

@if(GPU_PARTICLES)
fn ribbon_pcg(v: u32) -> u32 {
    let s = v * 747796405u + 2891336453u;
    let word = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (word >> 22u) ^ word;
}

// Point `k` of a trail: 0 the particle, then the ring newest first, past the
// `skip` newest points that are where the particle is, as render/ribbon.rs
// drops a zero-length first segment.
@if(GPU_PARTICLES)
fn ribbon_trail_point(tb: u32, head: u32, points: u32, skip: u32, here: vec3<f32>, k: u32) -> vec3<f32> {
    if k == 0u {
        return here;
    }
    let ring = (head + points - ((k - 1u + skip) % points)) % points;
    let w = tb + 4u + ring * 4u;
    return vec3<f32>(bitcast<f32>(particle_state[w]), bitcast<f32>(particle_state[w + 1u]), bitcast<f32>(particle_state[w + 2u]));
}

@if(GPU_PARTICLES)
fn ribbon_fetch(sc: vec3<f32>) -> ParticleVertex {
    var out: ParticleVertex;
    out.corner = vec2<f32>(sc.x, 0.0);
    let points = u32(params.gpu.w);
    let cap = arrayLength(&particles);
    var p: Particle;
    var pos = vec3<f32>(0.0);
    var prev = vec3<f32>(0.0);
    var next = vec3<f32>(0.0);
    var u = 0.0;
    var seed = 0.0;
    var frame = 0.0;
    if points > 0u {
        let slot = u32(sc.z);
        if slot >= cap {
            return out;
        }
        p = particles[slot];
        if p.lifetime < 0.0 {
            return out;
        }
        let tb = u32(params.gpu.z) + slot * 4u * (points + 1u);
        let head = particle_state[tb] % points;
        let len = min(particle_state[tb + 1u], points);
        let here = p.position;
        // Every newest point still where the particle is: a segment of no
        // length has no direction.
        var skip = 0u;
        loop {
            if skip >= len {
                break;
            }
            let d = ribbon_trail_point(tb, head, points, 0u, here, skip + 1u) - here;
            if dot(d, d) >= 1e-12 {
                break;
            }
            skip = skip + 1u;
        }
        let m = 1u + len - skip;
        if m < 2u {
            return out;
        }
        let j = min(u32(sc.y), m - 1u);
        pos = ribbon_trail_point(tb, head, points, skip, here, j);
        prev = ribbon_trail_point(tb, head, points, skip, here, max(j, 1u) - 1u);
        next = ribbon_trail_point(tb, head, points, skip, here, min(j + 1u, m - 1u));
        u = f32(j) / f32(m - 1u);
        seed = p.seed;
        frame = sheet_frame(p.age, p.lifetime);
    } else {
        let count = min(particle_state[0], cap);
        let segment = u32(sc.z);
        if segment + 1u >= count {
            return out;
        }
        let keys = u32(params.axis.w);
        let sorted = u32(params.gpu.y);
        let g = particle_state[keys + segment] >> 24u;
        // The segment from one strip's last particle to the next strip's
        // first is no segment at all.
        if (particle_state[keys + segment + 1u] >> 24u) != g || g >= 32u {
            return out;
        }
        var start = 0u;
        for (var i = 0u; i < g; i = i + 1u) {
            start = start + particle_state[32u + i];
        }
        let n = particle_state[32u + g];
        let rank = segment + u32(sc.y);
        if n < 2u || rank < start {
            return out;
        }
        let j = rank - start;
        p = particles[min(particle_state[sorted + rank], cap - 1u)];
        pos = p.position;
        prev = particles[min(particle_state[sorted + rank - select(0u, 1u, j > 0u)], cap - 1u)].position;
        next = particles[min(particle_state[sorted + rank + select(0u, 1u, j + 1u < n)], cap - 1u)].position;
        u = f32(j) / f32(n - 1u);
        // render/mod.rs's particle_seed of the strip, as the CPU seeds a
        // Connect strip by its group.
        seed = f32(ribbon_pcg(g ^ 0x9E3779B9u) >> 8u) * (1.0 / 16777216.0);
    }
    let m = to_mesh();
    var dir = (m * vec4<f32>(next - prev, 0.0)).xyz;
    // A repeated point has no chord; the particle's motion is the nearest
    // thing to the way the strip runs there.
    if dot(dir, dir) <= 1e-16 {
        dir = (m * vec4<f32>(p.velocity, 0.0)).xyz;
    }
    var life = 0.0;
    if p.lifetime > 0.0 {
        life = clamp(p.age / p.lifetime, 0.0, 1.0);
    }
    var color = p.color;
    color.w = color.w * (1.0 + (params.shape.w - 1.0) * u);
    out.center = (m * vec4<f32>(pos, 1.0)).xyz;
    out.corner = vec2<f32>(sc.x, u - 0.5);
    out.color = color;
    out.size_rot_frame = vec4<f32>(p.size.x * (1.0 + (params.shape.z - 1.0) * u), 0.0, 0.0, frame);
    out.axis = dir;
    out.extra = vec4<f32>(seed, life, 0.0, 0.0);
    return out;
}

// One quad per pool slot: a free slot, or one past the live count when
// drawing sorted, collapses to a point and draws nothing. A ribbon's vertex
// is the ribbon fetch's instead.
@if(GPU_PARTICLES)
fn fetch(v: Vertex) -> ParticleVertex {
    if params.mode.y > 2.5 {
        return ribbon_fetch(v.slot_corner);
    }
    var out = ParticleVertex(vec3<f32>(0.0), v.slot_corner.xy, vec4<f32>(0.0), vec4<f32>(0.0), vec3<f32>(0.0), vec4<f32>(0.0), vec3<f32>(0.0));
    var slot = u32(v.slot_corner.z);
    if params.gpu.x > 0.5 {
        if slot >= particle_state[0] {
            return out;
        }
        slot = particle_state[u32(params.gpu.y) + slot];
    }
    if slot >= arrayLength(&particles) {
        return out;
    }
    let p = particles[slot];
    if p.lifetime < 0.0 {
        return out;
    }
    let m = to_mesh();
    out.center = (m * vec4<f32>(p.position, 1.0)).xyz;
    out.velocity = (m * vec4<f32>(p.velocity, 0.0)).xyz;
    let facing = params.mode.y;
    var size = p.size;
    if facing > 0.5 && facing < 1.5 {
        out.axis = out.velocity;
        if params.mode.w > 0.0 {
            size.y = size.y + length(out.axis) * params.mode.w;
        }
    } else if facing > 1.5 && facing < 2.5 {
        out.axis = params.axis.xyz;
    }
    var life = 0.0;
    if p.lifetime > 0.0 {
        life = clamp(p.age / p.lifetime, 0.0, 1.0);
    }
    out.color = p.color;
    out.size_rot_frame = vec4<f32>(size, p.rotation, sheet_frame(p.age, p.lifetime));
    out.extra = vec4<f32>(p.seed, life, 0.0, 0.0);
    return out;
}

// END GPU FETCH

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) corner: vec2<f32>,
    @location(2) color: vec4<f32>,
    // seed, life, frame
    @location(3) extra: vec4<f32>,
    // For lighting: where the fragment is, and the way its surface faces.
    @location(4) world_position: vec4<f32>,
    @location(5) world_normal: vec3<f32>,
    // Where the vertex was a frame ago, for the motion-vector prepass.
    @if(PREPASS_PIPELINE && MOTION_VECTOR_PREPASS)
    @location(6) previous_world_position: vec4<f32>,
    // A shadow view without depth clip control clamps depth in the vertex
    // stage and writes the real one from the fragment, as Bevy's own
    // prepass does.
    @if(UNCLIPPED_DEPTH_ORTHO_EMULATION)
    @location(7) unclipped_depth: f32,
};

// BEGIN SHARED

// The blend code as an integer; see the table at the top of the file.
fn blend_mode() -> u32 {
    return u32(params.mode.x + 0.5);
}

// Straight colour in the form the blend's pipeline expects: premultiplied for
// additive (with alpha 0, so nothing behind darkens), premultiplied and
// multiply; straight for everything else.
fn blend_output(c: vec4<f32>) -> vec4<f32> {
    let blend = blend_mode();
    if blend == 1u {
        return vec4<f32>(c.rgb * c.a, 0.0);
    }
    if blend == 2u || blend == 5u {
        return vec4<f32>(c.rgb * c.a, c.a);
    }
    return c;
}

// UV of `corner` within sprite-sheet cell `frame`, read left to right, top to
// bottom.
fn sheet_uv(corner: vec2<f32>, frame: f32) -> vec2<f32> {
    let cols = max(params.sheet.x, 1.0);
    let rows = max(params.sheet.y, 1.0);
    let f = floor(frame);
    let row = floor(f / cols);
    let col = f - row * cols;
    let local = vec2<f32>(corner.x + 0.5, 0.5 - corner.y);
    return (vec2<f32>(col, row) + local) / vec2<f32>(cols, rows);
}

// The texture at `frame`, crossfaded into the next frame by the fraction when
// the emitter blends frames. A fixed-rate sheet wraps to frame 0; one played
// over the particle's life holds its last frame.
fn sample_sprite(corner: vec2<f32>, frame: f32, uv: vec2<f32>) -> vec4<f32> {
    if params.shading.w < 0.5 {
        return textureSample(particle_texture, particle_sampler, uv);
    }
    let frames = max(params.sheet.x, 1.0) * max(params.sheet.y, 1.0);
    let f0 = floor(frame);
    var f1 = f0 + 1.0;
    if params.sheet.z > 0.5 {
        f1 = f1 - frames * floor(f1 / frames);
    } else {
        f1 = min(f1, frames - 1.0);
    }
    let a = textureSample(particle_texture, particle_sampler, sheet_uv(corner, f0));
    let b = textureSample(particle_texture, particle_sampler, sheet_uv(corner, f1));
    return mix(a, b, clamp(frame - f0, 0.0, 1.0));
}

// Dave Hoskins' sine-free hash: the same on every GPU, where fract(sin(x))
// loses precision differently per vendor and bands on some.
fn hash12(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash12(i);
    let b = hash12(i + vec2<f32>(1.0, 0.0));
    let c = hash12(i + vec2<f32>(0.0, 1.0));
    let d = hash12(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Fractal value noise, normalised to 0..1 (mean 0.5). Each octave is turned
// and shifted so the lattices of successive octaves never line up into grid
// artefacts. A few octaves is all a particle a few hundred pixels tall needs.
fn fbm(p_in: vec2<f32>, octaves: i32) -> f32 {
    var p = p_in;
    var sum = 0.0;
    var amp = 0.5;
    var norm = 0.0;
    for (var i = 0; i < octaves; i = i + 1) {
        sum = sum + amp * value_noise(p);
        norm = norm + amp;
        p = mat2x2<f32>(1.6, 1.2, -1.2, 1.6) * p + vec2<f32>(5.3, 1.7);
        amp = amp * 0.5;
    }
    return sum / norm;
}

// Heat (0 cold, 1 a flame's core) to HDR linear colour along a blackbody
// curve, 900 K at 0 to 3600 K at 1. The chromaticity is a two-line fit to the
// Planckian locus in linear Rec.709 normalised to red = 1 (green rises about
// 0.2 per 1000 K from 0.05 at 1000 K; blue starts near 2000 K), within a few
// percent of the exact table `color/blackbody` uses. Brightness goes with heat
// squared: a tempered stand-in for radiance's T^4, which would make a core
// thousands of times an ember and leave nothing for the tonemapper to show.
fn blackbody_ramp(heat: f32) -> vec3<f32> {
    let h = max(heat, 0.0);
    let kk = 0.9 + 2.7 * h;
    let g = clamp(0.2 * kk - 0.15, 0.0, 1.0);
    let b = clamp(0.15 * (kk - 2.0), 0.0, 1.0);
    return vec3<f32>(1.0, g, b) * (h * h * 2.0);
}

// A flame tongue filling the quad, base at the bottom (corner.y = -0.5), tip
// at the top, which a velocity-aligned quad points along the motion. Returns
// straight colour and coverage; the emitted light is their product.
//
// After the Niagara flame recipe: a soft teardrop mask, UVs distorted by noise
// (the lick, growing toward the tip, where real flames whip most), alpha eroded
// by noise scrolling upward with the gas against a threshold that rises with
// life and height, so a young tongue is whole and an old one breaks into
// wisps from the top down. The temperature is the eroded density times the
// colour's alpha, so the core is the hottest and the ragged edges and torn-off
// wisps the coolest, which is the gradient Nguyen, Fedkiw and Jensen's
// blackbody flames show. Their blue core is the faint blue at the base, where
// the gas is burning cleanly before soot forms.
fn flame_shade(corner: vec2<f32>, seed: f32, life: f32, color: vec4<f32>, time: f32) -> vec4<f32> {
    let y = corner.y + 0.5;
    let t = time + seed * 61.0;
    let s = seed * 97.0;
    let lick = fbm(vec2<f32>(y * 2.2 - t * 1.9, s), 2) - 0.5;
    let jitter = fbm(vec2<f32>(corner.x * 4.0 + s, y * 4.0 - t * 3.1), 2) - 0.5;
    let x = corner.x + lick * 0.55 * y * y + jitter * 0.08;
    // Width: a rounded base swelling to its widest a fifth of the way up,
    // then tapering to a point.
    let w = 0.46 * pow(1.0 - min(y, 1.0), 1.15) * (0.3 + 0.7 * sqrt(clamp(y / 0.22, 0.0, 1.0)));
    let d = abs(x) / max(w, 1e-3);
    let across = clamp(1.0 - d * d, 0.0, 1.0);
    let along = smoothstep(0.0, 0.1, y) * sqrt(clamp(1.0 - y, 0.0, 1.0));
    let shape = across * along;
    let n = fbm(vec2<f32>(x * 3.4 + s, y * 2.6 - t * 2.4), 4);
    let density = shape + (n - 0.5) * (0.6 + 1.2 * y);
    let erode = 0.06 + 0.5 * life * life + 0.18 * y + params.shading.y * life;
    let coverage = smoothstep(erode, erode + 0.22, density);
    let heat = clamp(density, 0.0, 1.2) * color.a;
    let blue = (1.0 - smoothstep(0.02, 0.16, y)) * across * color.a * 0.35;
    let rgb = blackbody_ramp(heat) * color.rgb + vec3<f32>(0.08, 0.16, 0.6) * blue;
    return vec4<f32>(rgb, coverage);
}

// A soft, billowing puff. The disc is domain-warped by low-frequency noise so
// its outline bulges rather than reading as a circle, its density carries a
// finer noise that erodes away as the puff ages, and it is lit from above by
// comparing the density with the density a little higher up: where it thins
// upward the surface faces the sky and is brighter, where it thickens it is in
// its own shadow. Returns straight colour and coverage.
fn smoke_shade(corner: vec2<f32>, seed: f32, life: f32, color: vec4<f32>, time: f32) -> vec4<f32> {
    let s = seed * 97.0;
    let t = time * 0.25 + seed * 13.0;
    let p = corner * 2.0;
    let warp = vec2<f32>(fbm(p * 1.3 + vec2<f32>(s, t), 3), fbm(p * 1.3 + vec2<f32>(t, s + 4.1), 3)) - 0.5;
    let q = p + warp * 0.6;
    let body = 1.0 - smoothstep(0.35, 1.0, length(q));
    let drift = vec2<f32>(s, -t * 1.5);
    let n = fbm(q * 1.8 + drift, 4);
    let density = body * (0.45 + 1.1 * n);
    let erode = 0.08 + 0.55 * life + params.shading.y * life * 0.5;
    let coverage = smoothstep(erode, erode + 0.45, density);
    let n_up = fbm((q + vec2<f32>(0.0, 0.18)) * 1.8 + drift, 4);
    let lit = clamp(0.62 + (n - n_up) * 2.2 + q.y * 0.18, 0.35, 1.15);
    return vec4<f32>(color.rgb * lit, coverage * color.a);
}

// The emitter's erosion on a sprite's alpha: noise against a threshold that
// rises from just below zero at birth (nothing eaten) to the erosion amount at
// death, so a particle dissolves into holes and wisps instead of fading flat.
fn erode_sprite(alpha: f32, corner: vec2<f32>, seed: f32, life: f32) -> f32 {
    let e = params.shading.y;
    if e <= 0.0 {
        return alpha;
    }
    let n = clamp((fbm(corner * 3.0 + vec2<f32>(seed * 97.0, life * 1.5), 3) - 0.2) / 0.6, 0.0, 1.0);
    let threshold = e * life * 1.12 - 0.12;
    return alpha * smoothstep(threshold, threshold + 0.12, n);
}

// The fragment's colour before tonemapping and soft fading, in the form the
// blend mode's pipeline expects.
fn particle_color(in: VertexOut) -> vec4<f32> {
    let blend = blend_mode();
    let shading = params.shading.x;
    let seed = in.extra.x;
    let life = in.extra.y;
    if shading > 0.5 {
        var c: vec4<f32>;
        if shading < 1.5 {
            c = flame_shade(in.corner, seed, life, in.color, globals.time);
        } else {
            c = smoke_shade(in.corner, seed, life, in.color, globals.time);
        }
        return blend_output(c);
    }

    let premultiplied = blend == 2u;
    var tex = vec4<f32>(1.0);
    if params.mode.z > 0.5 && params.mode.z < 1.5 {
        tex = sample_sprite(in.corner, in.extra.z, in.uv);
    } else if params.mode.z < 0.5 {
        // No texture: a soft round dot, which is what a particle with no art
        // should look like rather than a hard square. (An untextured mesh,
        // mode.z 2, stays solid white.)
        let d = length(in.corner) * 2.0;
        let a = 1.0 - smoothstep(0.0, 1.0, d);
        tex = vec4<f32>(1.0, 1.0, 1.0, a * a);
        // Premultiplied mode treats the texture as premultiplied; so must the
        // stand-in, or its corners come out as a solid square.
        if premultiplied {
            tex = vec4<f32>(tex.a, tex.a, tex.a, tex.a);
        }
    }
    if params.shading.y > 0.0 {
        let a = erode_sprite(tex.a, in.corner, seed, life);
        // A premultiplied texture's colour shrinks with its alpha.
        if premultiplied && tex.a > 1e-5 {
            tex = vec4<f32>(tex.rgb * (a / tex.a), a);
        } else {
            tex.a = a;
        }
    }

    if premultiplied {
        return vec4<f32>(tex.rgb * in.color.rgb * in.color.a, tex.a * in.color.a);
    }
    return blend_output(tex * in.color);
}

// END SHARED

// BEGIN VERTEX EXPANSION

// A particle vertex placed in the world, which every pass of both 3D
// pipelines (this file's and particle_surface.wesl's) builds its output from.
//
// The trail hook: a vertex path that expands particles some other way (a GPU
// trail's history points, a tube's ring) fills one of these and hands it to
// the same output code, the way `place_quad` and `place_mesh` do, so its
// strips are lit, shaded, shadowed and given motion vectors like any quad.
struct Placed {
    world: vec3<f32>,
    normal: vec3<f32>,
    // Along the quad's width (a mesh's turned perpendicular), for normal maps.
    tangent: vec3<f32>,
    uv: vec2<f32>,
    corner: vec2<f32>,
    color: vec4<f32>,
    // seed, life, frame, 0
    extra: vec4<f32>,
    // world space
    velocity: vec3<f32>,
};

// World units per pixel at `center` when the emitter sizes in screen pixels,
// 1 otherwise. A clip-space height of 2 spans the viewport's height in pixels,
// and a world length L at the centre spans L * clip_from_view[1][1] / w of it,
// so this is right for a perspective camera (w is the depth) and an
// orthographic one (w is 1) alike. The same as Hanabi's screen-space size.
fn pixel_scale(center: vec3<f32>) -> f32 {
    if params.shape.y < 0.5 {
        return 1.0;
    }
    let w = max((view.clip_from_world * vec4<f32>(center, 1.0)).w, 1e-4);
    return 2.0 * w / (view.clip_from_view[1][1] * max(view.viewport.w, 1.0));
}

// Toward the viewer from `at`. An orthographic view (a directional light's
// shadow cascade, an orthographic camera) looks along its own axis, and its
// position is no point any quad should turn toward.
fn toward_view(at: vec3<f32>) -> vec3<f32> {
    if view.clip_from_view[3].w == 1.0 {
        return view.world_from_view[2].xyz;
    }
    return view.world_position - at;
}

// A quad (or a ribbon vertex) turned to face the view around its centre.
fn place_quad(pv: ParticleVertex, world_from_local: mat4x4<f32>, center: vec3<f32>) -> Placed {
    let size = pv.size_rot_frame.xy;
    let rotation = pv.size_rot_frame.z;
    let c = cos(rotation);
    let s = sin(rotation);
    var offset = vec2<f32>(pv.corner.x * c - pv.corner.y * s, pv.corner.x * s + pv.corner.y * c) * size;
    // Ribbon: a strip vertex sits on its point and moves only across the
    // strip, by the width there; corner.y is how far along the strip it is,
    // which only the UVs and the shading read.
    if params.mode.y > 2.5 {
        offset = vec2<f32>(pv.corner.x * size.x, 0.0);
    }
    offset = offset * pixel_scale(center);

    var right = view.world_from_view[0].xyz;
    var up = view.world_from_view[1].xyz;
    if params.mode.y > 0.5 {
        let axis_world = (world_from_local * vec4<f32>(pv.axis, 0.0)).xyz;
        let axis_len = length(axis_world);
        let to_view = toward_view(center);
        if axis_len > 1e-6 && length(to_view) > 1e-6 {
            let along = axis_world / axis_len;
            let side = cross(along, normalize(to_view));
            let side_len = length(side);
            // Looking straight down the axis there is no side to turn toward;
            // keep the view-facing basis rather than collapse the quad.
            if side_len > 1e-4 {
                right = side / side_len;
                up = along;
            }
        }
    }

    var p: Placed;
    p.world = center + right * offset.x + up * offset.y;
    // A quad is a card facing the view, whichever way its winding runs.
    var normal = cross(right, up);
    if dot(normal, toward_view(p.world)) < 0.0 {
        normal = -normal;
    }
    p.normal = normal;
    p.tangent = right;
    p.uv = sheet_uv(pv.corner, pv.size_rot_frame.w);
    p.corner = pv.corner;
    p.color = pv.color;
    p.extra = vec4<f32>(pv.extra.x, pv.extra.y, pv.size_rot_frame.w, 0.0);
    return p;
}

// The orientation math below has a Rust twin in render/mesh_particles.rs
// (`mesh_rotation`). Keep the two the same.

// A stable 0..1 per particle and stream `k`, from the particle's seed.
fn seed_random(seed: f32, k: f32) -> f32 {
    return hash12(vec2<f32>(seed * 4096.0, k));
}

// A uniformly random unit vector from streams `k` and `k + 1`.
fn random_axis(seed: f32, k: f32) -> vec3<f32> {
    let z = seed_random(seed, k) * 2.0 - 1.0;
    let phi = seed_random(seed, k + 1.0) * 6.2831853;
    let r = sqrt(max(1.0 - z * z, 0.0));
    return vec3<f32>(r * cos(phi), r * sin(phi), z);
}

// Rotation by `angle` about the unit `axis` (Rodrigues), as glam's
// Mat3::from_axis_angle builds it.
fn axis_angle(a: vec3<f32>, angle: f32) -> mat3x3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    let k = 1.0 - c;
    return mat3x3<f32>(
        vec3<f32>(c + a.x * a.x * k, a.y * a.x * k + a.z * s, a.z * a.x * k - a.y * s),
        vec3<f32>(a.x * a.y * k - a.z * s, c + a.y * a.y * k, a.z * a.y * k + a.x * s),
        vec3<f32>(a.x * a.z * k + a.y * s, a.y * a.z * k - a.x * s, c + a.z * a.z * k),
    );
}

// A rotation taking +Y to the unit `dir`, with no roll of its own.
fn align_y(dir: vec3<f32>) -> mat3x3<f32> {
    var helper = vec3<f32>(1.0, 0.0, 0.0);
    if abs(dir.x) > 0.9 {
        helper = vec3<f32>(0.0, 0.0, 1.0);
    }
    let z = normalize(cross(helper, dir));
    let x = cross(dir, z);
    return mat3x3<f32>(x, dir, z);
}

// A mesh particle's rotation in mesh space, from what the facing value says
// the vertex carries: nothing (0, a tumble), the velocity (1) or a fixed axis
// (2) in `axis`. A tumble starts each particle at a random orientation, so a
// burst of debris is not a row of identical cubes, and spins it about a
// random axis by its rotation, so an angular velocity makes it tumble.
fn mesh_orientation(axis: vec3<f32>, seed: f32, rotation: f32) -> mat3x3<f32> {
    if params.mode.y < 0.5 {
        let base = axis_angle(random_axis(seed, 1.0), seed_random(seed, 3.0) * 6.2831853);
        return axis_angle(random_axis(seed, 4.0), rotation) * base;
    }
    var dir = vec3<f32>(0.0, 1.0, 0.0);
    let len = length(axis);
    if len > 1e-6 {
        dir = axis / len;
    }
    return align_y(dir) * axis_angle(vec3<f32>(0.0, 1.0, 0.0), rotation);
}

// Any unit vector perpendicular to `n`, as `any_tangent` in
// render/mesh_particles.rs picks it: a mesh copy carries no tangent of its
// own, and a normal-mapped material still needs a frame.
fn perpendicular(n: vec3<f32>) -> vec3<f32> {
    var helper = vec3<f32>(0.0, 1.0, 0.0);
    if abs(n.y) >= 0.99 {
        helper = vec3<f32>(1.0, 0.0, 0.0);
    }
    let t = cross(helper, n);
    let len = length(t);
    if len < 1e-6 {
        return vec3<f32>(1.0, 0.0, 0.0);
    }
    return t / len;
}

// A vertex of a mesh copy: turned by the particle's orientation and the
// entity's rotation, sized by vec3(size.x, size.y, size.x) in world units (or
// pixels), and placed on the particle. The entity's scale is left out, as it
// is for quads, so a size means the same on both.
fn place_mesh(pv: ParticleVertex, local: vec3<f32>, normal: vec3<f32>, world_from_local: mat4x4<f32>, center: vec3<f32>) -> Placed {
    let scale = vec3<f32>(pv.size_rot_frame.x, pv.size_rot_frame.y, pv.size_rot_frame.x) * pixel_scale(center);
    let entity = mat3x3<f32>(
        world_from_local[0].xyz / max(length(world_from_local[0].xyz), 1e-6),
        world_from_local[1].xyz / max(length(world_from_local[1].xyz), 1e-6),
        world_from_local[2].xyz / max(length(world_from_local[2].xyz), 1e-6),
    );
    let turn = entity * mesh_orientation(pv.axis, pv.extra.x, pv.size_rot_frame.z);
    var p: Placed;
    p.world = center + turn * (local * scale);
    // Normals scale inversely, so a flattened sphere is lit as a flat one.
    let n = turn * (normal / max(abs(scale), vec3<f32>(1e-6)));
    p.normal = n / max(length(n), 1e-6);
    let t = turn * (perpendicular(normal) * scale);
    p.tangent = t / max(length(t), 1e-6);
    p.uv = sheet_uv(pv.corner, pv.size_rot_frame.w);
    p.corner = pv.corner;
    p.color = pv.color;
    p.extra = vec4<f32>(pv.extra.x, pv.extra.y, pv.size_rot_frame.w, 0.0);
    return p;
}

// A tube ribbon's vertex (facing 4, the trail hook's expansion half): the
// strip point pushed out round the strip's direction by half the width,
// `corner.x` the way round (-0.5 to 0.5 once; render/ribbon.rs and the GPU's
// ribbon fetch both give the ring that). The ring's frame is turned to the
// view about the direction, so its seam sits the same way from every view
// and no frame has to be carried from point to point, which a vertex that
// sees one point at a time could not do; a round tube looks the same either
// way. It is real geometry: its normal is the way out, for lighting and the
// prepass, and its tangent runs round it.
fn place_tube(pv: ParticleVertex, world_from_local: mat4x4<f32>, center: vec3<f32>) -> Placed {
    let radius = 0.5 * pv.size_rot_frame.x * pixel_scale(center);
    var along = (world_from_local * vec4<f32>(pv.axis, 0.0)).xyz;
    if dot(along, along) < 1e-12 {
        along = view.world_from_view[1].xyz;
    }
    along = normalize(along);
    var side = cross(along, toward_view(center));
    if dot(side, side) < 1e-12 {
        // Looking straight down the tube: any way across will do.
        side = perpendicular(along);
    }
    side = normalize(side);
    let toward = cross(side, along);
    let angle = (pv.corner.x + 0.5) * 6.2831853;
    let out_dir = side * cos(angle) + toward * sin(angle);
    var p: Placed;
    p.world = center + out_dir * radius;
    p.normal = out_dir;
    p.tangent = normalize(cross(along, out_dir));
    p.uv = sheet_uv(pv.corner, pv.size_rot_frame.w);
    p.corner = pv.corner;
    p.color = pv.color;
    p.extra = vec4<f32>(pv.extra.x, pv.extra.y, pv.size_rot_frame.w, 0.0);
    return p;
}

// The vertex of `pv` in the world: a quad, a ribbon point, a tube's ring
// vertex or a mesh copy, by the pipeline's defs and the facing. `local` and
// `normal` are the mesh copy's vertex (unused for quads).
fn place_particle(pv: ParticleVertex, local: vec3<f32>, normal: vec3<f32>, instance_index: u32) -> Placed {
    let world_from_local = mesh_functions::get_world_from_local(instance_index);
    let center = (world_from_local * vec4<f32>(pv.center, 1.0)).xyz;
    var p: Placed;
    @if(MESH_PARTICLES) {
        p = place_mesh(pv, local, normal, world_from_local, center);
    }
    @if(!MESH_PARTICLES) {
        if params.mode.y > 3.5 {
            p = place_tube(pv, world_from_local, center);
        } else {
            p = place_quad(pv, world_from_local, center);
        }
    }
    p.velocity = (world_from_local * vec4<f32>(pv.velocity, 0.0)).xyz;
    return p;
}

// Where `world` was a frame ago: back along the particle's velocity by the
// frame's simulated time (the playback rate scales the frame's delta, so a
// paused effect is still), and for a `Local` emitter also back with the
// entity, through its previous transform at the particle's centre. A `World`
// emitter's particles stay where they were born whatever the entity does.
fn previous_world(world: vec3<f32>, velocity: vec3<f32>, center_local: vec3<f32>, instance_index: u32) -> vec3<f32> {
    let back = world - velocity * (globals.delta_time * max(params.motion.x, 0.0));
    if params.motion.y > 0.5 {
        return back;
    }
    let now = mesh_functions::get_world_from_local(instance_index) * vec4<f32>(center_local, 1.0);
    let before = mesh_functions::get_previous_world_from_local(instance_index) * vec4<f32>(center_local, 1.0);
    return back + (before.xyz - now.xyz);
}

// END VERTEX EXPANSION

@vertex
fn vertex(v: Vertex) -> VertexOut {
    let pv = fetch(v);
    var local = vec3<f32>(0.0);
    var normal = vec3<f32>(0.0, 1.0, 0.0);
    @if(MESH_PARTICLES) {
        local = v.local;
        normal = v.normal;
    }
    let p = place_particle(pv, local, normal, v.instance_index);
    var out: VertexOut;
    out.position = view.clip_from_world * vec4<f32>(p.world, 1.0);
    out.uv = p.uv;
    out.corner = p.corner;
    out.color = p.color;
    out.extra = p.extra;
    out.world_position = vec4<f32>(p.world, 1.0);
    out.world_normal = p.normal;
    @if(PREPASS_PIPELINE && MOTION_VECTOR_PREPASS) {
        out.previous_world_position = vec4<f32>(previous_world(p.world, p.velocity, pv.center, v.instance_index), 1.0);
    }
    @if(UNCLIPPED_DEPTH_ORTHO_EMULATION) {
        out.unclipped_depth = out.position.z;
        out.position.z = min(out.position.z, 1.0);
    }
    return out;
}

// Soft particles: 0..1, fading the fragment out as it nears the opaque scene
// behind it. Only with the view's depth prepass texture, which Bevy binds in
// this pass under DEPTH_PREPASS (a copy, so reading it here is safe); see the
// material module docs for why WebGL2 is left out.
fn soft_fade(frag: vec4<f32>) -> f32 {
    @if(DEPTH_PREPASS && !WEBGL2 && !PREPASS_PIPELINE) {
        let distance = params.shading.z;
        if distance > 0.0 {
            let scene = view_transformations::depth_ndc_to_view_z(prepass_utils::prepass_depth(frag, 0u));
            let here = view_transformations::depth_ndc_to_view_z(frag.z);
            // View z is negative ahead of the camera, so the gap to the
            // surface behind is here - scene.
            return clamp((here - scene) / distance, 0.0, 1.0);
        }
    }
    return 1.0;
}

// The particle's colour shaded by the scene's lights, through Bevy's own PBR
// lighting: directional, point and spot lights with their shadows, ambient
// and the environment map. `c` is the colour in the form the blend expects;
// the renderer never sets lit for additive or multiply, whose colour is
// light or a filter, not a surface. A premultiplied colour is lit as the
// straight colour it holds.
fn light(in: VertexOut, c: vec4<f32>, front: bool) -> vec4<f32> {
    let premultiplied = blend_mode() == 2u;
    var rgb = c.rgb;
    if premultiplied {
        rgb = c.rgb / max(c.a, 1e-4);
    }
    var n = in.world_normal / max(length(in.world_normal), 1e-6);
    // A mesh seen from behind (a leaf card) is lit as the side being seen.
    // A quad's normal already faces the camera.
    if params.shape.x > 0.5 && !front {
        n = -n;
    }
    let ortho = view.clip_from_view[3].w == 1.0;
    var pbr_input = pbr_types::pbr_input_new();
    pbr_input.material.base_color = vec4<f32>(rgb, 1.0);
    pbr_input.material.perceptual_roughness = params.surface.y;
    pbr_input.material.metallic = params.surface.z;
    pbr_input.frag_coord = in.position;
    pbr_input.world_position = in.world_position;
    pbr_input.world_normal = n;
    pbr_input.N = n;
    pbr_input.V = pbr_functions::calculate_view(in.world_position, ortho);
    pbr_input.is_orthographic = ortho;
    // Particles stand in the sun's shadow (and solid ones cast their own).
    pbr_input.flags = MESH_FLAGS_SHADOW_RECEIVER_BIT;
    let lit = pbr_functions::apply_pbr_lighting(pbr_input).rgb;
    if premultiplied {
        return vec4<f32>(lit * c.a, c.a);
    }
    return vec4<f32>(lit, c.a);
}

// Whether the blend writes depth: opaque, mask and alpha to coverage.
fn writes_depth() -> bool {
    let blend = blend_mode();
    return blend == 3u || blend == 4u || blend == 6u;
}

@if(!PREPASS_PIPELINE)
@fragment
fn fragment(in: VertexOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    var out = particle_color(in);
    if params.surface.x > 0.5 {
        out = light(in, out, front);
    }
    // Taken before any branch: a derivative needs uniform control flow.
    let edge = fwidth(out.a);
    let blend = blend_mode();
    if writes_depth() {
        // The depth-written blends draw with the scene's solid geometry:
        // nothing to fade, and a mask throws away what is under its cutoff.
        // Coverage instead hands the MSAA resolve an alpha that crosses the
        // cutoff within about a pixel, which it turns into a smooth edge.
        if blend == 4u && out.a < params.surface.w {
            discard;
        }
        if blend == 6u {
            out.a = clamp((out.a - params.surface.w) / max(edge, 1e-4) + 0.5, 0.0, 1.0);
        } else {
            out.a = 1.0;
        }
    } else {
        let fade = soft_fade(in.position);
        if blend == 0u {
            // Straight alpha: fading the alpha alone fades the particle.
            out.a = out.a * fade;
        } else {
            out = out * fade;
        }
    }

    @if(TONEMAP_IN_SHADER) {
        out = tone_mapping(out, view.color_grading);
    }
    return out;
}

// How much of the particle is there, whatever form its colour takes: the
// alpha, or for additive (written with alpha 0) the brightest channel.
fn coverage(c: vec4<f32>) -> f32 {
    if blend_mode() == 1u {
        return clamp(max(c.r, max(c.g, c.b)), 0.0, 1.0);
    }
    return c.a;
}

// What the prepass and the shadow passes throw away: everything the main
// pass would not draw solid. An opaque particle keeps every fragment; the
// rest are cut at the cutoff, which for a blended particle casting a shadow
// is where its shadow's edge falls.
@if(PREPASS_PIPELINE)
fn prepass_discard(in: VertexOut) {
    if blend_mode() == 3u {
        return;
    }
    if coverage(particle_color(in)) < params.surface.w {
        discard;
    }
}

// The surface's normal as the forward lighting sees it: the card's, which
// already faces the view, or the mesh's, flipped on a back face (a leaf).
@if(PREPASS_PIPELINE)
fn seen_normal(in: VertexOut, front: bool) -> vec3<f32> {
    var n = in.world_normal / max(length(in.world_normal), 1e-6);
    if params.shape.x > 0.5 && !front {
        n = -n;
    }
    return n;
}

// The particle as the G-buffer holds it. What the forward path would light
// (`light`) becomes the base colour, roughness, metallic and normal, for the
// deferred lighting pass to light with the same Bevy PBR it calls forward;
// what it would draw as is (unlit) carries Bevy's unlit flag, which that pass
// draws unshaded from the emissive channel at full HDR.
//
// The base colour is 8 bits per channel, so a colour past 1 (the Emission pin
// above 1, an HDR gradient) is split: the colour scaled into range is the
// surface and the overflow is emissive, so the brightest particles still glow
// where they would have clipped. A procedural flame is light rather than a
// surface, so its whole colour is emissive over a black base. The colour is
// straight here: only the depth-written blends (3, 4 and 6) reach this pass,
// and `particle_color` returns them unpremultiplied. Fog stays off, as the
// forward particle shader never applies it either; particles receive shadows.
@if(PREPASS_PIPELINE && DEFERRED_PREPASS)
fn particle_gbuffer(in: VertexOut, front: bool) -> vec4<u32> {
    let rgb = max(particle_color(in).rgb, vec3<f32>(0.0));
    let n = seen_normal(in, front);
    let ortho = view.clip_from_view[3].w == 1.0;
    var pbr_input = pbr_types::pbr_input_new();
    pbr_input.frag_coord = in.position;
    pbr_input.world_position = in.world_position;
    pbr_input.world_normal = n;
    pbr_input.N = n;
    pbr_input.V = pbr_functions::calculate_view(in.world_position, ortho);
    pbr_input.is_orthographic = ortho;
    pbr_input.flags = MESH_FLAGS_SHADOW_RECEIVER_BIT;
    pbr_input.material.flags = 0u;
    pbr_input.material.perceptual_roughness = params.surface.y;
    pbr_input.material.metallic = params.surface.z;
    if params.surface.x < 0.5 {
        pbr_input.material.flags = pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT;
        pbr_input.material.base_color = vec4<f32>(rgb, 1.0);
    } else if params.shading.x > 0.5 && params.shading.x < 1.5 {
        pbr_input.material.base_color = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        pbr_input.material.emissive = vec4<f32>(rgb, 1.0);
    } else {
        let base = rgb / max(1.0, max(rgb.r, max(rgb.g, rgb.b)));
        pbr_input.material.base_color = vec4<f32>(base, 1.0);
        pbr_input.material.emissive = vec4<f32>(rgb - base, 1.0);
    }
    return deferred_gbuffer_from_pbr_input(pbr_input);
}

@if(PREPASS_PIPELINE && PREPASS_FRAGMENT)
@fragment
fn fragment(in: VertexOut, @builtin(front_facing) front: bool) -> FragmentOutput {
    prepass_discard(in);
    var out: FragmentOutput;
    @if(NORMAL_PREPASS) {
        let n = seen_normal(in, front);
        out.normal = vec4<f32>(n * 0.5 + vec3<f32>(0.5), 1.0);
    }
    @if(DEFERRED_PREPASS) {
        out.deferred = particle_gbuffer(in, front);
        // Bevy's PBR lighting pass, the one every StandardMaterial starts on.
        out.deferred_lighting_pass_id = pbr_types::standard_material_new().deferred_lighting_pass_id;
    }
    @if(MOTION_VECTOR_PREPASS) {
        let clip_t = view.unjittered_clip_from_world * in.world_position;
        let clip = clip_t.xy / clip_t.w;
        let previous_t = previous_view_uniforms.unjittered_clip_from_world * in.previous_world_position;
        let previous = previous_t.xy / previous_t.w;
        // Bevy's encoding: UV offsets in -1..1, v pointing down.
        out.motion_vector = (clip - previous) * vec2<f32>(0.5, -0.5);
    }
    @if(UNCLIPPED_DEPTH_ORTHO_EMULATION) {
        out.frag_depth = in.unclipped_depth;
    }
    return out;
}

@if(PREPASS_PIPELINE && !PREPASS_FRAGMENT)
@fragment
fn fragment(in: VertexOut) {
    prepass_discard(in);
}
