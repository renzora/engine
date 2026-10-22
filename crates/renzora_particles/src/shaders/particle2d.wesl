// 2D particles: one quad per particle in the XY plane, expanded here from its
// centre. The 2D twin of particle3d.wesl; see that file for the vertex layout,
// the blend conventions and the shading modes, which are identical. The block
// between the SHARED markers is a copy of the 3D one (tests/shaders.rs checks
// that it still is).
//
// Facing (params.mode.y) in 2D:
//   0  the quad is screen-aligned and turned by the particle's rotation.
//   1  velocity aligned: height along the velocity in the XY plane.
//   2  fixed axis: height along the axis from the effect file.
//   3  ribbon: a strip vertex, pushed across the strip's direction in the
//      XY plane by its width.
//
// The 2D pipeline only has alpha blending, so additive, premultiplied and
// multiply get their blend states from the material's `specialize`, and this
// shader writes premultiplied colour for all three, with alpha 0 for additive.
// Opaque (3) and alpha mask (4) are the 2D opaque and alpha-mask phases:
// straight colour, alpha 1, discarding below the cutoff for a mask. Alpha to
// coverage (6) is a mask here, since 2D has no coverage mode. 2D has no depth
// prepass, so there are no soft particles here, and no lights, so no lit
// shading. Mesh particles are 3D only.
//
// Screen-space size (params.shape.y) turns pixels into world units through
// the 2D camera's projection, so a zoomed camera keeps the particle's size.
//
// GPU_PARTICLES pulls each quad's particle from the GPU backend's storage
// buffer, exactly as in particle3d.wesl; the GPU FETCH block is a copy of
// that file's.

import bevy_sprite_render::mesh2d::functions;
import bevy_sprite_render::mesh2d::view_bindings::{view, globals};
@if(TONEMAP_IN_SHADER) import bevy_core_pipeline::tonemapping::tone_mapping;

struct ParticleParams {
    sheet: vec4<f32>,
    mode: vec4<f32>,
    shading: vec4<f32>,
    axis: vec4<f32>,
    gpu: vec4<f32>,
    // x lit and the rest of the surface are 3D only; w the alpha-mask cutoff
    surface: vec4<f32>,
    // x mesh particles (3D only), y size in screen pixels
    shape: vec4<f32>,
    // motion vectors, 3D only
    motion: vec4<f32>,
};

@group(2) @binding(0) var<uniform> params: ParticleParams;
@group(2) @binding(1) var particle_texture: texture_2d<f32>;
@group(2) @binding(2) var particle_sampler: sampler;

@if(!GPU_PARTICLES)
struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) center: vec3<f32>,
    @location(1) corner: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) size_rot_frame: vec4<f32>,
    @location(4) axis: vec3<f32>,
    @location(5) extra: vec4<f32>,
};

@if(GPU_PARTICLES) {
struct Vertex {
    @builtin(instance_index) instance_index: u32,
    // corner in xy, pool slot in z
    @location(0) slot_corner: vec3<f32>,
};

@group(2) @binding(3) var<storage, read> particles: array<Particle>;
@group(2) @binding(4) var<storage, read> particle_state: array<u32>;
}

// What a vertex says about its particle, from whichever path made it.
struct ParticleVertex {
    center: vec3<f32>,
    corner: vec2<f32>,
    color: vec4<f32>,
    size_rot_frame: vec4<f32>,
    axis: vec3<f32>,
    extra: vec4<f32>,
    // 3D only (motion vectors and material graphs); the shared fetch fills it
    velocity: vec3<f32>,
};

@if(!GPU_PARTICLES)
fn fetch(v: Vertex) -> ParticleVertex {
    return ParticleVertex(v.center, v.corner, v.color, v.size_rot_frame, v.axis, v.extra, vec3<f32>(0.0));
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

// World units per pixel at `center` when the emitter sizes in screen pixels,
// 1 otherwise. A clip-space height of 2 spans the viewport's height in pixels,
// and a world length L at the centre spans L * clip_from_view[1][1] / w of it.
fn pixel_scale(center: vec3<f32>) -> f32 {
    if params.shape.y < 0.5 {
        return 1.0;
    }
    let w = max((view.clip_from_world * vec4<f32>(center, 1.0)).w, 1e-4);
    return 2.0 * w / (view.clip_from_view[1][1] * max(view.viewport.w, 1.0));
}

@vertex
fn vertex(v: Vertex) -> VertexOut {
    let pv = fetch(v);
    let world_from_local = functions::get_world_from_local(v.instance_index);
    let center = world_from_local * vec4<f32>(pv.center, 1.0);

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
    offset = offset * pixel_scale(center.xyz);

    var right = vec2<f32>(1.0, 0.0);
    var up = vec2<f32>(0.0, 1.0);
    if params.mode.y > 0.5 {
        let axis_world = (world_from_local * vec4<f32>(pv.axis, 0.0)).xy;
        let axis_len = length(axis_world);
        if axis_len > 1e-6 {
            up = axis_world / axis_len;
            right = vec2<f32>(up.y, -up.x);
        }
    }

    let world = vec4<f32>(center.xy + right * offset.x + up * offset.y, center.z, 1.0);
    var out: VertexOut;
    out.position = functions::mesh2d_position_world_to_clip(world);
    out.uv = sheet_uv(pv.corner, pv.size_rot_frame.w);
    out.corner = pv.corner;
    out.color = pv.color;
    out.extra = vec4<f32>(pv.extra.x, pv.extra.y, pv.size_rot_frame.w, 0.0);
    return out;
}

@fragment
fn fragment(in: VertexOut) -> @location(0) vec4<f32> {
    var out = particle_color(in);
    // Opaque, alpha mask and alpha to coverage (a mask in 2D): the
    // depth-written phases want alpha 1, and a mask throws away what is under
    // its cutoff.
    let blend = blend_mode();
    if blend == 3u || blend == 4u || blend == 6u {
        if blend != 3u && out.a < params.surface.w {
            discard;
        }
        out.a = 1.0;
    }

    @if(TONEMAP_IN_SHADER) {
        out = tone_mapping(out, view.color_grading);
    }
    return out;
}
