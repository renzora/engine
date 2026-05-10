// Stochastic average-log-luminance reducer.
//
// One workgroup per output. We sample a 16x16 stratified grid of the
// view target, accumulate log2(luminance) + 10.0 into a single fixed-point
// atomic (×1000). Main-world side reads that, divides by sample count,
// subtracts 10, and that's EV-100 of the scene.
//
// 256 samples × ~4 atomic ops = ~1024 atomic ops/frame. Cheap enough to
// dispatch every frame regardless of scene size.

@group(0) @binding(0) var scene: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> result: array<atomic<u32>, 2>;
//                                            [0] = sum of (log2(lum) + 10.0) × 1000
//                                            [1] = sample count

const SAMPLES_X: u32 = 16u;
const SAMPLES_Y: u32 = 16u;

@compute @workgroup_size(16, 16, 1)
fn reduce(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= SAMPLES_X || gid.y >= SAMPLES_Y) { return; }

    let dims = vec2<f32>(textureDimensions(scene));
    // Stratified sample at the center of each grid cell.
    let cell = vec2<f32>(f32(gid.x) + 0.5, f32(gid.y) + 0.5)
             / vec2<f32>(f32(SAMPLES_X), f32(SAMPLES_Y));
    let pixel = vec2<i32>(cell * dims);

    let color = textureLoad(scene, pixel, 0).rgb;
    let lum = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));

    // Shift log space to positive: pure black (lum=1e-4) → 10 + (-13.3) = -3.3
    // clamped to 0; bright sun (lum~1000) → 10 + 9.96 ≈ 20. 0..32 fits in u32.
    let log_lum = clamp(log2(max(lum, 1e-4)) + 10.0, 0.0, 32.0);
    let fixed = u32(log_lum * 1000.0);

    atomicAdd(&result[0], fixed);
    atomicAdd(&result[1], 1u);
}
