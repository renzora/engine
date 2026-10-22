// Particles wearing a project `.material`: the vertex stage of
// `render::surface`'s ExtendedMaterial<StandardMaterial, _>.
//
// Only the vertex shaders live here. The fragment is the material's own,
// compiled from its graph (or StandardMaterial's pbr.wesl for a material
// simple enough to be a plain StandardMaterial), so a particle is lit,
// fogged, shadowed and tonemapped exactly as that material is on a mesh.
// This file places the particle: the same fetch (CPU columns or the GPU
// pool, under GPU_PARTICLES) and the same expansion (quads in every facing,
// ribbons, screen-space size, mesh particles in every orientation) as
// particle3d.wesl, copied between the GPU FETCH and VERTEX EXPANSION markers
// and held to that file by tests/shaders.rs.
//
// The main pass writes forward_io's VertexOutput locations (0 world position,
// 1 normal, 2 UV, 4 tangent, 5 colour, 6 instance index), which the
// pipeline's specialize turns on with VERTEX_UVS_A, VERTEX_TANGENTS and
// VERTEX_COLORS whatever the particle mesh's attributes, plus two of its own
// that a material graph's Particle nodes read under RENZORA_PARTICLE_SURFACE:
// location 10 `vec4(life, seed, frame, speed)` and 11 the world velocity. A
// fragment that does not read them (any mesh material, StandardMaterial's)
// simply leaves them unused.
//
// PREPASS_PIPELINE: the prepass, shadow and deferred G-buffer passes, writing
// prepass::io's VertexOutput locations (`PrepassOut`) for StandardMaterial's
// own prepass and G-buffer fragments, or a graph's deferred branch, with the
// previous position for motion vectors (see `previous_world`). Under
// DEFERRED_PREPASS it writes locations 10 and 11 as the main pass does, so a
// graph's Particle nodes read the same values in the G-buffer.

import bevy_pbr::render::mesh_functions;
import bevy_pbr::render::mesh_view_bindings::view;
@if(!PREPASS_PIPELINE) import bevy_pbr::render::mesh_view_bindings::globals;
@if(PREPASS_PIPELINE) import bevy_render::globals::Globals;

@if(PREPASS_PIPELINE) @group(0) @binding(1) var<uniform> globals: Globals;

// `ParticleMaterialParams`, as particle3d.wesl declares it.
struct ParticleParams {
    sheet: vec4<f32>,
    mode: vec4<f32>,
    shading: vec4<f32>,
    axis: vec4<f32>,
    gpu: vec4<f32>,
    surface: vec4<f32>,
    shape: vec4<f32>,
    motion: vec4<f32>,
};

// Past the graph material's slots (100..118), which the material's own
// fragment shader declares.
@group(3) @binding(119) var<uniform> params: ParticleParams;

@if(GPU_PARTICLES) {
@group(3) @binding(120) var<storage, read> particles: array<Particle>;
@group(3) @binding(121) var<storage, read> particle_state: array<u32>;
}

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
    @location(0) slot_corner: vec3<f32>,
};

@if(GPU_PARTICLES && MESH_PARTICLES)
struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) slot_corner: vec3<f32>,
    @location(1) local: vec3<f32>,
    @location(2) normal: vec3<f32>,
};

struct ParticleVertex {
    center: vec3<f32>,
    corner: vec2<f32>,
    color: vec4<f32>,
    size_rot_frame: vec4<f32>,
    axis: vec3<f32>,
    extra: vec4<f32>,
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

// Copies of particle3d.wesl's `sheet_uv` and `hash12` (its SHARED block holds
// texture reads this pipeline has no binding for, so the block itself cannot
// come along); tests/shaders.rs holds them to the originals.
fn sheet_uv(corner: vec2<f32>, frame: f32) -> vec2<f32> {
    let cols = max(params.sheet.x, 1.0);
    let rows = max(params.sheet.y, 1.0);
    let f = floor(frame);
    let row = floor(f / cols);
    let col = f - row * cols;
    let local = vec2<f32>(corner.x + 0.5, 0.5 - corner.y);
    return (vec2<f32>(col, row) + local) / vec2<f32>(cols, rows);
}

fn hash12(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

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

// The main pass's output: forward_io::VertexOutput's locations, which the
// material's fragment reads, and the particle's own data past them.
@if(!PREPASS_PIPELINE)
struct SurfaceOut {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(4) world_tangent: vec4<f32>,
    @location(5) color: vec4<f32>,
    @if(VERTEX_OUTPUT_INSTANCE_INDEX)
    @location(6) @interpolate(flat) instance_index: u32,
    @if(VISIBILITY_RANGE_DITHER)
    @location(7) @interpolate(flat) visibility_range_dither: i32,
    // life, seed, sheet frame, speed
    @location(10) particle: vec4<f32>,
    // world space
    @location(11) particle_velocity: vec3<f32>,
};

// The mesh copy's own vertex, or nothing for a quad.
fn mesh_vertex(v: Vertex) -> array<vec3<f32>, 2> {
    var out = array<vec3<f32>, 2>(vec3<f32>(0.0), vec3<f32>(0.0, 1.0, 0.0));
    @if(MESH_PARTICLES) {
        out[0] = v.local;
        out[1] = v.normal;
    }
    return out;
}

@if(!PREPASS_PIPELINE)
@vertex
fn vertex(v: Vertex) -> SurfaceOut {
    let pv = fetch(v);
    let m = mesh_vertex(v);
    let p = place_particle(pv, m[0], m[1], v.instance_index);
    var out: SurfaceOut;
    out.position = view.clip_from_world * vec4<f32>(p.world, 1.0);
    out.world_position = vec4<f32>(p.world, 1.0);
    out.world_normal = p.normal;
    out.uv = p.uv;
    out.world_tangent = vec4<f32>(p.tangent, 1.0);
    out.color = p.color;
    @if(VERTEX_OUTPUT_INSTANCE_INDEX) {
        out.instance_index = v.instance_index;
    }
    out.particle = vec4<f32>(p.extra.y, p.extra.x, p.extra.z, length(p.velocity));
    out.particle_velocity = p.velocity;
    return out;
}

// The prepass's output: prepass::io's VertexOutput, location for location and
// under the same defines (StandardMaterial's prepass and G-buffer fragments
// and a graph's deferred branch all read it as that struct), plus, in the
// G-buffer pass alone, the particle's own locations 10 and 11, which a
// graph's Particle nodes read there exactly as they do forward. They are left
// out of the depth prepass and shadows, whose fragments never read them.
// Inter-stage variables match by location, so a struct of our own feeds a
// fragment that declares Bevy's.
@if(PREPASS_PIPELINE)
struct PrepassOut {
    @builtin(position) position: vec4<f32>,
    @if(VERTEX_UVS_A)
    @location(0) uv: vec2<f32>,
    @if(VERTEX_UVS_B)
    @location(1) uv_b: vec2<f32>,
    @if(NORMAL_PREPASS_OR_DEFERRED_PREPASS)
    @location(2) world_normal: vec3<f32>,
    @if(NORMAL_PREPASS_OR_DEFERRED_PREPASS && VERTEX_TANGENTS)
    @location(3) world_tangent: vec4<f32>,
    @location(4) world_position: vec4<f32>,
    @if(MOTION_VECTOR_PREPASS)
    @location(5) previous_world_position: vec4<f32>,
    @if(UNCLIPPED_DEPTH_ORTHO_EMULATION)
    @location(6) unclipped_depth: f32,
    @if(VERTEX_OUTPUT_INSTANCE_INDEX)
    @location(7) @interpolate(flat) instance_index: u32,
    @if(VERTEX_COLORS)
    @location(8) color: vec4<f32>,
    @if(VISIBILITY_RANGE_DITHER)
    @location(9) @interpolate(flat) visibility_range_dither: i32,
    // life, seed, sheet frame, speed
    @if(DEFERRED_PREPASS)
    @location(10) particle: vec4<f32>,
    // world space
    @if(DEFERRED_PREPASS)
    @location(11) particle_velocity: vec3<f32>,
};

@if(PREPASS_PIPELINE)
@vertex
fn vertex(v: Vertex) -> PrepassOut {
    let pv = fetch(v);
    let m = mesh_vertex(v);
    let p = place_particle(pv, m[0], m[1], v.instance_index);
    var out: PrepassOut;
    out.position = view.clip_from_world * vec4<f32>(p.world, 1.0);
    @if(UNCLIPPED_DEPTH_ORTHO_EMULATION) {
        out.unclipped_depth = out.position.z;
        out.position.z = min(out.position.z, 1.0);
    }
    @if(VERTEX_UVS_A) {
        out.uv = p.uv;
    }
    @if(VERTEX_UVS_B) {
        out.uv_b = p.uv;
    }
    @if(NORMAL_PREPASS_OR_DEFERRED_PREPASS) {
        out.world_normal = p.normal;
    }
    @if(NORMAL_PREPASS_OR_DEFERRED_PREPASS && VERTEX_TANGENTS) {
        out.world_tangent = vec4<f32>(p.tangent, 1.0);
    }
    out.world_position = vec4<f32>(p.world, 1.0);
    @if(MOTION_VECTOR_PREPASS) {
        out.previous_world_position = vec4<f32>(previous_world(p.world, p.velocity, pv.center, v.instance_index), 1.0);
    }
    @if(VERTEX_OUTPUT_INSTANCE_INDEX) {
        out.instance_index = v.instance_index;
    }
    @if(VERTEX_COLORS) {
        out.color = p.color;
    }
    @if(VISIBILITY_RANGE_DITHER) {
        out.visibility_range_dither = 0;
    }
    @if(DEFERRED_PREPASS) {
        out.particle = vec4<f32>(p.extra.y, p.extra.x, p.extra.z, length(p.velocity));
        out.particle_velocity = p.velocity;
    }
    return out;
}
