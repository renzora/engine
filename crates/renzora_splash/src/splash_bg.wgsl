// Splash night sky for the r1-alpha6 terrain flyover: a deep twilight gradient
// with a faint last-light ember on the horizon, a glowing moon with soft maria,
// a sparse twinkling starfield, and a few birds gliding across the upper sky.
// It is the backmost layer — the moonlit terrain composites over it, and distant
// fully-fogged ridges melt into the horizon band (the terrain fog colour is tuned
// to match it). Deliberately dark/muted. Output is sRGB-encoded for the UI pass.

#import bevy_ui::ui_vertex_output::UiVertexOutput

struct BgUniforms {
    params: vec4<f32>, // x = time (s), y = aspect (w/h)
};

@group(1) @binding(0)
var<uniform> u: BgUniforms;

fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

// Smooth value noise → 0..1 (for the moon's maria + subtle sky variation).
fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn hsv2rgb(h: f32, s: f32, v: f32) -> vec3<f32> {
    let k = vec3<f32>(5.0, 3.0, 1.0);
    let p = abs(fract(vec3<f32>(h) + k / 6.0) * 6.0 - 3.0);
    return v * mix(vec3<f32>(1.0), clamp(p - 1.0, vec3<f32>(0.0), vec3<f32>(1.0)), s);
}

// Antialiased ~1px line at integer values of `coord` (screen-space derivative).
fn grid_line(coord: f32) -> f32 {
    let d = max(fwidth(coord), 1e-5);
    return 1.0 - min(abs(fract(coord - 0.5) - 0.5) / d, 1.0);
}

// Soft Gaussian glow around integer lines — fakes bloom on the wireframe (the sky
// is a plain UI layer, not the HDR bloom pipeline). `w` is the halo half-width in
// `coord` units.
fn grid_glow(coord: f32, w: f32) -> f32 {
    let dd = abs(fract(coord - 0.5) - 0.5);
    return exp(-(dd * dd) / (w * w));
}

// A retro wireframe sphere whose axis runs *through* the camera: we're flying into
// it, so the far pole is a vanishing point on the horizon ahead and the latitude
// rings expand outward past us (the near hemisphere is behind the camera). Drawn as
// concentric rings (latitudes) + radial spokes (meridians) around the vanishing
// point. `center` is the screen-space vanishing point; returns the neon colour.
fn grid_sphere(uv: vec2<f32>, center: vec2<f32>, aspect: f32, t: f32) -> vec3<f32> {
    let PI = 3.14159265;
    // View ray: screen centre looks at the vanishing point (forward = +Z).
    let ndc = vec2<f32>((uv.x - center.x) * aspect, center.y - uv.y);
    let fov = 1.25;
    let dir = normalize(vec3<f32>(ndc.x * fov, ndc.y * fov, 1.0));
    let theta = acos(clamp(dir.z, -1.0, 1.0)); // angle from the vanishing point (0 = dead ahead)
    let phi = atan2(dir.y, dir.x) + t * 0.04;   // very slow spin around the axis

    let rc = theta / PI * 16.0 - t * 0.16;      // latitude coord (rings drift slowly toward us)
    let sc = phi / PI * 10.0;                    // meridian coord
    let wire = max(grid_line(rc), grid_line(sc));               // crisp lines
    let glow = max(grid_glow(rc, 0.10), grid_glow(sc, 0.10));   // soft bloom halo
    let intensity = wire + glow * 0.6;

    let hue = fract(theta / PI + t * 0.015);
    return hsv2rgb(hue, 0.85, 1.0) * intensity;
}

// Distance from point `p` to segment `a`–`b` (for the bird silhouettes).
fn seg_dist(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    return length(pa - ba * h);
}

// One gliding gull: a flapping "V" silhouette drifting across the upper sky.
fn bird(uv: vec2<f32>, aspect: f32, t: f32, seed: f32) -> f32 {
    let speed = 0.012 + hash21(vec2<f32>(seed, 1.0)) * 0.02;
    let dir = select(-1.0, 1.0, hash21(vec2<f32>(seed, 2.0)) > 0.5);
    let base_y = 0.10 + hash21(vec2<f32>(seed, 3.0)) * 0.26;
    let phase = hash21(vec2<f32>(seed, 4.0)) * 6.2832;
    let x = fract(hash21(vec2<f32>(seed, 5.0)) + t * speed * dir + 1.0);
    let cy = base_y + 0.012 * sin(t * 0.8 + phase);
    // Aspect-corrected local coords so wings keep their shape.
    let pl = vec2<f32>((uv.x - x) * aspect, uv.y - cy);
    let span = 0.016 + hash21(vec2<f32>(seed, 6.0)) * 0.010;
    let flap = 0.30 + 0.50 * sin(t * 6.0 + phase); // wings raise/lower
    let tip_l = vec2<f32>(-span, span * flap);
    let tip_r = vec2<f32>(span, span * flap);
    let d = min(seg_dist(pl, vec2<f32>(0.0), tip_l), seg_dist(pl, vec2<f32>(0.0), tip_r));
    return smoothstep(0.0024, 0.0, d);
}

// Rotate a 3D point around the X then Y axes.
fn rot3(v: vec3<f32>, ax: f32, ay: f32) -> vec3<f32> {
    let cx = cos(ax);
    let sx = sin(ax);
    let p = vec3<f32>(v.x, v.y * cx - v.z * sx, v.y * sx + v.z * cx);
    let cy = cos(ay);
    let sy = sin(ay);
    return vec3<f32>(p.x * cy + p.z * sy, p.y, -p.x * sy + p.z * cy);
}

// Shortest screen-space distance from `p` to the edges of a unit wireframe cube,
// rotated in 3D and projected with a mild perspective — so it reads as a real
// spinning 3D cube. `p` is in the same aspect-corrected space as the projection.
fn wire_cube(p: vec2<f32>, scale: f32, ax: f32, ay: f32) -> f32 {
    var V = array<vec3<f32>, 8>(
        vec3<f32>(-1.0, -1.0, -1.0), vec3<f32>(1.0, -1.0, -1.0),
        vec3<f32>(1.0, 1.0, -1.0), vec3<f32>(-1.0, 1.0, -1.0),
        vec3<f32>(-1.0, -1.0, 1.0), vec3<f32>(1.0, -1.0, 1.0),
        vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(-1.0, 1.0, 1.0),
    );
    var P: array<vec2<f32>, 8>;
    for (var i = 0; i < 8; i = i + 1) {
        let r = rot3(V[i], ax, ay);
        let s = scale / (1.0 - r.z * 0.28); // perspective: nearer (z+) draws larger
        P[i] = vec2<f32>(r.x, r.y) * s;
    }
    var E = array<vec2<i32>, 12>(
        vec2<i32>(0, 1), vec2<i32>(1, 2), vec2<i32>(2, 3), vec2<i32>(3, 0),
        vec2<i32>(4, 5), vec2<i32>(5, 6), vec2<i32>(6, 7), vec2<i32>(7, 4),
        vec2<i32>(0, 4), vec2<i32>(1, 5), vec2<i32>(2, 6), vec2<i32>(3, 7),
    );
    var d = 1e9;
    for (var i = 0; i < 12; i = i + 1) {
        let e = E[i];
        d = min(d, seg_dist(p, P[e.x], P[e.y]));
    }
    return d;
}

// Octahedron (a 3D diamond): 6 vertices on the axes, 12 edges.
fn wire_octa(p: vec2<f32>, scale: f32, ax: f32, ay: f32) -> f32 {
    var V = array<vec3<f32>, 6>(
        vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(-1.0, 0.0, 0.0),
        vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, -1.0, 0.0),
        vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 0.0, -1.0),
    );
    var P: array<vec2<f32>, 6>;
    for (var i = 0; i < 6; i = i + 1) {
        let r = rot3(V[i], ax, ay);
        let s = scale / (1.0 - r.z * 0.28);
        P[i] = vec2<f32>(r.x, r.y) * s;
    }
    var E = array<vec2<i32>, 12>(
        vec2<i32>(0, 2), vec2<i32>(0, 3), vec2<i32>(0, 4), vec2<i32>(0, 5),
        vec2<i32>(1, 2), vec2<i32>(1, 3), vec2<i32>(1, 4), vec2<i32>(1, 5),
        vec2<i32>(2, 4), vec2<i32>(2, 5), vec2<i32>(3, 4), vec2<i32>(3, 5),
    );
    var d = 1e9;
    for (var i = 0; i < 12; i = i + 1) {
        let e = E[i];
        d = min(d, seg_dist(p, P[e.x], P[e.y]));
    }
    return d;
}

// Tetrahedron (a 3D triangle/pyramid): 4 vertices, 6 edges.
fn wire_tetra(p: vec2<f32>, scale: f32, ax: f32, ay: f32) -> f32 {
    let k = scale * 0.85; // tetra verts have length sqrt(3); trim so sizes roughly match
    var V = array<vec3<f32>, 4>(
        vec3<f32>(1.0, 1.0, 1.0), vec3<f32>(1.0, -1.0, -1.0),
        vec3<f32>(-1.0, 1.0, -1.0), vec3<f32>(-1.0, -1.0, 1.0),
    );
    var P: array<vec2<f32>, 4>;
    for (var i = 0; i < 4; i = i + 1) {
        let r = rot3(V[i], ax, ay);
        let s = k / (1.0 - r.z * 0.28);
        P[i] = vec2<f32>(r.x, r.y) * s;
    }
    var E = array<vec2<i32>, 6>(
        vec2<i32>(0, 1), vec2<i32>(0, 2), vec2<i32>(0, 3),
        vec2<i32>(1, 2), vec2<i32>(1, 3), vec2<i32>(2, 3),
    );
    var d = 1e9;
    for (var i = 0; i < 6; i = i + 1) {
        let e = E[i];
        d = min(d, seg_dist(p, P[e.x], P[e.y]));
    }
    return d;
}

// A few spinning 3D wireframe shapes (cube / octahedron / tetrahedron) drifting in
// the sky, each with a soft glow halo to match the grid sphere.
fn floating_shapes(uv: vec2<f32>, aspect: f32, t: f32) -> vec3<f32> {
    var acc = vec3<f32>(0.0);
    for (var i = 0; i < 11; i = i + 1) {
        let fi = f32(i);
        let drift = 0.004 + hash21(vec2<f32>(fi, 21.0)) * 0.007;
        let dir = select(-1.0, 1.0, hash21(vec2<f32>(fi, 22.0)) > 0.5);
        let px = fract(hash21(vec2<f32>(fi, 23.0)) + t * drift * dir + 1.0);
        let py = 0.08 + hash21(vec2<f32>(fi, 24.0)) * 0.30;
        let bob = 0.012 * sin(t * 0.2 + fi);
        let scale = 0.016 + hash21(vec2<f32>(fi, 25.0)) * 0.022;
        let ax = t * (0.25 + hash21(vec2<f32>(fi, 26.0)) * 0.35) + fi;
        let ay = t * (0.20 + hash21(vec2<f32>(fi, 27.0)) * 0.35) + fi * 2.0;

        // Occasional glitch burst: ~12% of ~1.7s slots trigger a brief decaying
        // burst, giving horizontal jitter, a scale "pop", a blink and a hue jump.
        let gslot = floor(t * 0.6 + fi * 7.7);
        let burst = step(0.88, hash21(vec2<f32>(gslot, fi + 31.0)))
            * (1.0 - smoothstep(0.0, 0.20, fract(t * 0.6 + fi * 7.7)));
        let jitter = (hash21(vec2<f32>(floor(t * 45.0), fi)) - 0.5) * 0.05 * burst;

        let p = vec2<f32>((uv.x - px - jitter) * aspect, uv.y - (py + bob));
        let kind = i32(hash21(vec2<f32>(fi, 29.0)) * 3.0) % 3;
        let sc = scale * (1.0 + burst * 0.18 * sin(t * 60.0));
        var d: f32;
        if (kind == 0) {
            d = wire_cube(p, sc, ax, ay);
        } else if (kind == 1) {
            d = wire_octa(p, sc, ax, ay);
        } else {
            d = wire_tetra(p, sc, ax, ay);
        }

        let line = smoothstep(0.0035, 0.0, d);
        let glow = exp(-(d * d) / (0.010 * 0.010));
        // Random blink + hue jump while glitching.
        let blink = 1.0 - burst * step(0.5, hash21(vec2<f32>(floor(t * 40.0), fi + 2.0))) * 0.85;
        let hue = fract(hash21(vec2<f32>(fi, 28.0)) + t * 0.02
            + burst * (hash21(vec2<f32>(floor(t * 40.0), fi + 3.0)) - 0.5));
        acc = acc + hsv2rgb(hue, 0.8, 1.0) * (line + glow * 0.35) * blink;
    }
    return acc;
}

const HORIZON: f32 = 0.52;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;                       // 0..1, y down
    let t = u.params.x;
    let aspect = max(u.params.y, 0.0001);

    // ── Retro synthwave gradient: deep indigo overhead → purple → magenta horizon ──
    let c_top = vec3<f32>(0.06, 0.02, 0.14);   // deep indigo
    let c_mid = vec3<f32>(0.20, 0.04, 0.26);   // purple
    let c_hor = vec3<f32>(0.40, 0.08, 0.24);   // magenta near the horizon
    var col = mix(c_top, c_mid, smoothstep(0.0, 0.55, uv.y));
    col = mix(col, c_hor, smoothstep(0.30, HORIZON, uv.y));
    col = mix(col, vec3<f32>(0.10, 0.03, 0.11), smoothstep(HORIZON, 1.0, uv.y)); // darker below (under terrain)

    // ── Retro horizon glow: hot-pink band + a warmer orange core ──
    let band = exp(-pow((uv.y - HORIZON) / 0.10, 2.0));
    col = col + vec3<f32>(0.55, 0.16, 0.30) * band * 0.5;
    let core = exp(-pow((uv.y - HORIZON) / 0.05, 2.0));
    col = col + vec3<f32>(0.50, 0.22, 0.10) * core * 0.4;

    // ── Retro wireframe sphere we're flying *into* ──
    // Its axis runs through the camera: the far pole is a vanishing point on the
    // horizon ahead, latitude rings expand outward past us, and the near hemisphere
    // is behind the camera. (Terrain hides the lower half.)
    col = col + grid_sphere(uv, vec2<f32>(0.5, HORIZON), aspect, t) * 0.45;

    // ── Floating neon wireframe shapes drifting in the sky ──
    col = col + floating_shapes(uv, aspect, t) * 0.45;

    // ── Moon: soft halo + bright disk with darker maria, up-right of centre ──
    let moon_c = vec2<f32>((uv.x - 0.70) * aspect, uv.y - 0.22);
    let md = length(moon_c);
    let halo = exp(-md * 7.0);
    col = col + vec3<f32>(0.55, 0.62, 0.80) * halo * 0.45;
    let moon_r = 0.050;
    let disk = smoothstep(moon_r, moon_r - 0.004, md);
    let maria = 0.78 + 0.22 * vnoise(moon_c * 60.0);
    let moon_col = vec3<f32>(0.90, 0.93, 1.0) * maria;
    col = mix(col, moon_col, disk);

    // ── Sparse steady stars in the upper sky (no twinkle; dimmer toward the horizon) ──
    if (uv.y < HORIZON) {
        let g = vec2<f32>(uv.x * aspect, uv.y) * 240.0;
        let cell = floor(g);
        let h = hash21(cell);
        if (h > 0.982) {
            let d = length(fract(g) - 0.5);
            let s = 1.0 - smoothstep(0.0, 0.14, d);
            let fade = clamp(1.0 - uv.y / HORIZON, 0.0, 1.0);
            col = col + vec3<f32>(0.80, 0.85, 1.0) * s * fade * 0.6;
        }
    }

    // ── A small flock gliding across the upper sky ──
    var flock = 0.0;
    for (var i = 0; i < 6; i = i + 1) {
        flock = max(flock, bird(uv, aspect, t, f32(i) * 7.0 + 1.0));
    }
    col = mix(col, vec3<f32>(0.04, 0.04, 0.06), flock * 0.85);

    // ── Bottom vignette so the launcher's bottom bar stays legible ──
    let vig = smoothstep(0.80, 1.0, uv.y);
    col = mix(col, vec3<f32>(0.014, 0.016, 0.032), vig * 0.7);

    // Faint film grain over the whole sky.
    col = col + (hash21(uv * 950.0 + vec2<f32>(t, t * 1.3)) - 0.5) * 0.014;

    return vec4<f32>(pow(col, vec3<f32>(2.2)), 1.0);
}
