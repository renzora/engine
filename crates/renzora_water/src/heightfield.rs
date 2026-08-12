//! CPU mirror of the GPU wave simulation, for gameplay queries.
//!
//! The displacement maps the ocean is drawn from live in VRAM, which is the
//! wrong place for buoyancy: a dedicated server (`--server`) has no render
//! device at all, and reading a texture back stalls the frame. So this module
//! recomputes the *same* sea on the CPU at a much lower resolution.
//!
//! "Same" is meant literally, not statistically. The spectrum's random phases
//! come from a hash of the **frequency-grid index**, so this module indexes
//! that hash with the coordinate the GPU's larger grid would have used for the
//! identical wave vector ([`gpu_id`]). The CPU field is then exactly the GPU
//! field low-pass filtered to the frequencies a 64² grid can hold — same
//! amplitudes, same phases, same time. A boat bobs in step with the swell it is
//! visibly sitting on; what it misses is the chop, which is below the scale at
//! which buoyancy is meaningful anyway.

use bevy::prelude::*;

use crate::component::{WaterSurface, WaveCascade};

/// Resolution of the CPU field. 64² per cascade is ~0.2 ms to rebuild and small
/// enough to keep resident; the swell lives at these frequencies.
pub const CPU_MAP_SIZE: usize = 64;

const PI: f32 = std::f32::consts::PI;
const G: f32 = 9.81;

/// Complex number as `(re, im)`. `Vec2` keeps the arithmetic readable and
/// matches the `vec2<f32>` the shaders use for the same values.
type Complex = Vec2;

fn mul_complex(a: Complex, b: Complex) -> Complex {
    Vec2::new(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x)
}

fn exp_complex(x: f32) -> Complex {
    Vec2::new(x.cos(), x.sin())
}

fn conj(x: Complex) -> Complex {
    Vec2::new(x.x, -x.y)
}

// ── Spectrum (mirrors spectrum_compute.wgsl) ─────────────────────────────────

/// Integer hash from the reference (<https://www.shadertoy.com/view/Xt3cDn>),
/// reproduced bit-for-bit so CPU and GPU draw the same random numbers.
fn hash(x: u32, y: u32) -> Vec2 {
    let mut h32 = y
        .wrapping_add(374_761_393)
        .wrapping_add(x.wrapping_mul(3_266_489_917));
    h32 = 2_246_822_519u32.wrapping_mul(h32 ^ (h32 >> 15));
    h32 = 3_266_489_917u32.wrapping_mul(h32 ^ (h32 >> 13));
    let n = h32 ^ (h32 >> 16);
    let rz = [n, n.wrapping_mul(48271)];
    Vec2::new(
        ((rz[0] >> 1) & 0x7FFF_FFFF) as f32,
        ((rz[1] >> 1) & 0x7FFF_FFFF) as f32,
    ) / 0x7FFF_FFFFu32 as f32
}

/// Box-Muller: uniform pair → bivariate normal.
fn gaussian(x: Vec2) -> Vec2 {
    // The GPU takes log(x.x) unguarded; a zero draw there yields infinity and
    // one dead texel. Clamping costs nothing and keeps NaNs out of physics.
    let r = (-2.0 * x.x.max(1e-9).ln()).sqrt();
    let theta = 2.0 * PI * x.y;
    Vec2::new(r * theta.cos(), r * theta.sin())
}

/// Dispersion relation and its derivative with respect to `k`.
fn dispersion_relation(k: f32, depth: f32) -> Vec2 {
    let a = k * depth;
    let b = a.tanh();
    let w = (G * k * b).sqrt();
    let dw = 0.5 * G * (b + a * (1.0 - b * b)) / w.max(1e-9);
    Vec2::new(w, dw)
}

fn longuet_higgins_normalization(s: f32) -> f32 {
    let a = s.sqrt();
    if s < 0.4 {
        (0.5 / PI) + s * (0.220636 + s * (-0.109 + s * 0.090))
    } else {
        (1.0 / PI.sqrt()) * (a * 0.5 + (1.0 / a) * 0.0625)
    }
}

fn longuet_higgins_function(s: f32, theta: f32) -> f32 {
    longuet_higgins_normalization(s) * (theta * 0.5).cos().abs().powf(2.0 * s)
}

fn hasselmann_directional_spread(
    w: f32,
    w_p: f32,
    wind_speed: f32,
    theta: f32,
    angle: f32,
    swell: f32,
) -> f32 {
    let p = w / w_p;
    let s = if w <= w_p {
        6.97 * p.abs().powf(4.06)
    } else {
        9.77 * p.abs().powf(-2.33 - 1.45 * (wind_speed * w_p / G - 1.17))
    };
    let s_xi = 16.0 * (w_p / w).tanh() * swell * swell;
    longuet_higgins_function(s + s_xi, theta - angle)
}

fn tma_spectrum(w: f32, w_p: f32, alpha: f32, depth: f32) -> f32 {
    const BETA: f32 = 1.25;
    const GAMMA: f32 = 3.3;

    let sigma = if w <= w_p { 0.07 } else { 0.09 };
    let r = (-(w - w_p) * (w - w_p) / (2.0 * sigma * sigma * w_p * w_p)).exp();
    let jonswap = (alpha * G * G) / w.powi(5) * (-BETA * (w_p / w).powi(4)).exp() * GAMMA.powf(r);

    let w_h = (w * (depth / G).sqrt()).min(2.0);
    let depth_attenuation = if w_h <= 1.0 {
        0.5 * w_h * w_h
    } else {
        1.0 - 0.5 * (2.0 - w_h) * (2.0 - w_h)
    };

    jonswap * depth_attenuation
}

/// Split a user-facing `u32` seed into the 2D lattice offset the spectrum hash
/// takes, **per cascade**. Both simulations must agree on this, so it lives in
/// one place and `systems.rs` uploads whatever it returns.
///
/// The cascade index is mixed in, and that is load-bearing rather than tidy.
/// The spectrum's random phases come from `hash(grid_index + seed)`, so giving
/// every cascade the same seed makes them draw the *identical* Gaussian field:
/// different tile lengths and amplitudes, but the same realization rescaled.
/// Their crests then line up, which is exactly the beat that varying the tile
/// lengths and staggering the clocks by `π·i` exist to prevent. The reference
/// project rolls an independent seed per cascade for the same reason.
pub fn spectrum_seed(seed: u32, cascade_index: usize) -> IVec2 {
    // Two different odd multipliers, so neither axis of the lattice offset is a
    // scalar multiple of the other — a shared factor would put every cascade's
    // offset on one diagonal and correlate them again, just more subtly.
    let mixed = seed
        .wrapping_add((cascade_index as u32).wrapping_mul(0x9E37_79B9))
        .wrapping_mul(0x85EB_CA6B);
    IVec2::new(mixed as i32, mixed.wrapping_mul(2_654_435_761) as i32)
}

/// The frequency-grid index the GPU's `map_size²` grid uses for the wave vector
/// that CPU index `i` addresses. Both grids share `dk = 2π/tile_length`, so the
/// CPU's frequencies are a centred subset of the GPU's and this is a plain
/// shift — which is exactly what makes the two fields agree.
fn gpu_id(i: usize, cpu_size: usize, gpu_size: usize) -> i32 {
    i as i32 - (cpu_size as i32) / 2 + (gpu_size as i32) / 2
}

/// One texel of the initial spectrum: `h0(k)` and `conj(h0(-k))`, the same pair
/// the GPU packs into its spectrum texture.
#[derive(Clone, Copy, Default)]
struct SpectrumTexel {
    h0: Complex,
    h0_conj_neg: Complex,
}

/// Cached per-cascade state: the time-independent spectrum plus the parameters
/// it was built from, so it is only rebuilt when the sea state changes.
struct CascadeState {
    spectrum: Vec<SpectrumTexel>,
    tile_length: Vec2,
    displacement_scale: f32,
    depth: f32,
    /// Displacement field: `(x, height, z)` per texel, row-major.
    field: Vec<Vec3>,
}

/// Sampled wave surface, rebuilt on a throttle by
/// [`crate::systems::update_water_heightfield`].
#[derive(Resource, Default)]
pub struct WaterHeightField {
    cascades: Vec<CascadeState>,
    /// Set once a field has been computed; before that, sampling returns the
    /// flat plane rather than pretending there are waves.
    pub ready: bool,
}

impl WaterHeightField {
    /// Rebuild every cascade's displacement field for the given sea state and
    /// per-cascade times. `cascade_times` must be the same values fed to the
    /// GPU or the CPU surface will drift out of phase with the rendered one.
    pub fn update(&mut self, surface: &WaterSurface, cascade_times: &[f32]) {
        let cascades = surface.active_cascades();
        let gpu_size = surface.clamped_map_size() as usize;
        self.cascades.truncate(cascades.len());

        for (i, cascade) in cascades.iter().enumerate() {
            let time = cascade_times.get(i).copied().unwrap_or(0.0);

            if i >= self.cascades.len() {
                self.cascades.push(CascadeState {
                    spectrum: Vec::new(),
                    tile_length: Vec2::ZERO,
                    displacement_scale: cascade.displacement_scale,
                    depth: surface.sea_depth,
                    field: vec![Vec3::ZERO; CPU_MAP_SIZE * CPU_MAP_SIZE],
                });
            }

            let state = &mut self.cascades[i];
            let stale = state.spectrum.is_empty()
                || state.tile_length != cascade.tile_length
                || state.depth != surface.sea_depth;
            if stale {
                state.spectrum =
                    build_spectrum(cascade, surface.sea_depth, surface.seed, i, gpu_size);
                state.tile_length = cascade.tile_length;
                state.depth = surface.sea_depth;
            }
            state.displacement_scale = cascade.displacement_scale;

            modulate_and_transform(state, time);
        }

        self.ready = !self.cascades.is_empty();
    }

    /// Invalidate the cached spectra so the next update rebuilds them. Cheap
    /// enough to call on any parameter edit.
    pub fn invalidate(&mut self) {
        for cascade in &mut self.cascades {
            cascade.spectrum.clear();
        }
    }

    /// Total surface displacement at a world XZ position: `(x, height, z)`.
    ///
    /// Note this is the displacement *of* the grid point at `xz`, not a
    /// solution of "which grid point ends up above `xz`" — the same
    /// approximation the vertex shader makes.
    pub fn sample_displacement(&self, xz: Vec2) -> Vec3 {
        let mut total = Vec3::ZERO;
        for cascade in &self.cascades {
            if cascade.displacement_scale <= 0.0 || cascade.tile_length.x <= 0.0 {
                continue;
            }
            let uv = xz / cascade.tile_length;
            total += bilinear(&cascade.field, uv) * cascade.displacement_scale;
        }
        total
    }

    /// Surface height at a world XZ position, relative to the water plane.
    pub fn sample_height(&self, xz: Vec2) -> f32 {
        self.sample_displacement(xz).y
    }
}

/// Bilinear sample of a wrapping `CPU_MAP_SIZE²` field, `uv` in tile units.
fn bilinear(field: &[Vec3], uv: Vec2) -> Vec3 {
    let n = CPU_MAP_SIZE as f32;
    let x = uv.x * n;
    let y = uv.y * n;
    let x0 = x.floor();
    let y0 = y.floor();
    let fx = x - x0;
    let fy = y - y0;

    let wrap = |v: f32| (v as i64).rem_euclid(CPU_MAP_SIZE as i64) as usize;
    let ix0 = wrap(x0);
    let iy0 = wrap(y0);
    let ix1 = wrap(x0 + 1.0);
    let iy1 = wrap(y0 + 1.0);

    let s00 = field[iy0 * CPU_MAP_SIZE + ix0];
    let s10 = field[iy0 * CPU_MAP_SIZE + ix1];
    let s01 = field[iy1 * CPU_MAP_SIZE + ix0];
    let s11 = field[iy1 * CPU_MAP_SIZE + ix1];

    let top = s00.lerp(s10, fx);
    let bottom = s01.lerp(s11, fx);
    top.lerp(bottom, fy)
}

fn build_spectrum(
    cascade: &WaveCascade,
    depth: f32,
    seed: u32,
    cascade_index: usize,
    gpu_size: usize,
) -> Vec<SpectrumTexel> {
    let n = CPU_MAP_SIZE;
    let alpha = cascade.jonswap_alpha();
    let peak_frequency = cascade.jonswap_peak_frequency();
    let dk = Vec2::splat(2.0 * PI) / cascade.tile_length.max(Vec2::splat(1e-3));

    // The seed offsets the hash lattice exactly as the GPU's `seed` does.
    let seed = spectrum_seed(seed, cascade_index);
    let (seed_x, seed_y) = (seed.x, seed.y);

    let amplitude = |id: IVec2| -> Complex {
        let k_vec = Vec2::new(id.x as f32, id.y as f32) * dk;
        let k = k_vec.length() + 1e-6;
        let theta = k_vec.x.atan2(k_vec.y);

        let dispersion = dispersion_relation(k, depth);
        let w = dispersion.x;
        let w_norm = dispersion.y / k * dk.x * dk.y;
        let s = tma_spectrum(w, peak_frequency, alpha, depth);
        let isotropic = 0.5 / PI;
        let directional = hasselmann_directional_spread(
            w,
            peak_frequency,
            cascade.wind_speed,
            theta,
            cascade.wind_direction,
            cascade.swell,
        );
        let blend = 1.0 - cascade.spread;
        let d = (isotropic + (directional - isotropic) * blend)
            * (-(1.0 - cascade.detail) * (1.0 - cascade.detail) * k * k).exp();

        // Hash by the *GPU* grid coordinate so both simulations draw the same
        // random phase for this wave vector.
        let gpu = IVec2::new(
            id.x + (gpu_size as i32) / 2 + seed_x,
            id.y + (gpu_size as i32) / 2 + seed_y,
        );
        gaussian(hash(gpu.x as u32, gpu.y as u32)) * (2.0 * s * d * w_norm).max(0.0).sqrt()
    };

    let mut out = vec![SpectrumTexel::default(); n * n];
    for y in 0..n {
        for x in 0..n {
            // Centred frequency index — identical in both grids.
            let id = IVec2::new(
                gpu_id(x, n, gpu_size) - (gpu_size as i32) / 2,
                gpu_id(y, n, gpu_size) - (gpu_size as i32) / 2,
            );
            out[y * n + x] = SpectrumTexel {
                h0: amplitude(id),
                h0_conj_neg: conj(amplitude(-id)),
            };
        }
    }
    out
}

/// Propagate the spectrum to `time` and inverse-transform it into the
/// displacement field. Two real signals ride in one complex transform: the
/// x-displacement in the real part, the height in the imaginary part.
fn modulate_and_transform(state: &mut CascadeState, time: f32) {
    let n = CPU_MAP_SIZE;
    let dk = Vec2::splat(2.0 * PI) / state.tile_length.max(Vec2::splat(1e-3));

    let mut packed_xy = vec![Complex::ZERO; n * n];
    let mut packed_z = vec![Complex::ZERO; n * n];

    for y in 0..n {
        for x in 0..n {
            let id = IVec2::new(x as i32 - (n as i32) / 2, y as i32 - (n as i32) / 2);
            let k_vec = Vec2::new(id.x as f32, id.y as f32) * dk;
            let k = k_vec.length() + 1e-6;
            let k_unit = k_vec / k;

            let texel = state.spectrum[y * n + x];
            let dispersion = dispersion_relation(k, state.depth).x * time;
            let modulation = exp_complex(dispersion);
            let h = mul_complex(texel.h0, modulation)
                + mul_complex(texel.h0_conj_neg, conj(modulation));
            let h_inv = Vec2::new(-h.y, h.x);

            let hx = h_inv * k_unit.y;
            let hy = h;
            let hz = h_inv * k_unit.x;

            // hx + i*hy, exactly as the GPU packs FFT layer 0.
            packed_xy[y * n + x] = Vec2::new(hx.x - hy.y, hx.y + hy.x);
            packed_z[y * n + x] = hz;
        }
    }

    ifft2(&mut packed_xy, n);
    ifft2(&mut packed_z, n);

    for y in 0..n {
        for x in 0..n {
            // Multiplying by (-1)^(x+y) is an ifftshift — the spectrum was
            // built centred, so the transform lands offset by half a tile
            // without it.
            let sign = if (x ^ y) & 1 == 1 { -1.0 } else { 1.0 };
            let xy = packed_xy[y * n + x];
            let z = packed_z[y * n + x];
            // Transposed on purpose. The GPU runs rows -> transpose -> rows and
            // then *skips* the second transpose (a 90-degree rotation of a wave
            // field is invisible, and it saves a pass), so its maps are the
            // transpose of a textbook 2D inverse FFT. `ifft2` above does the
            // textbook version, so storing it transposed is what puts the CPU
            // surface in the same orientation as the one being drawn. Undo this
            // and buoyancy quietly runs on a sea rotated 90 degrees from the
            // visible one.
            state.field[x * n + y] = Vec3::new(xy.x, xy.y, z.x) * sign;
        }
    }
}

// ── Radix-2 inverse FFT ──────────────────────────────────────────────────────

/// In-place 2D inverse transform: rows, then columns. Unnormalised, matching
/// the GPU — the spectrum amplitudes already carry the `dk` normalisation.
fn ifft2(data: &mut [Complex], n: usize) {
    let mut row = vec![Complex::ZERO; n];

    for y in 0..n {
        row.copy_from_slice(&data[y * n..y * n + n]);
        ifft(&mut row);
        data[y * n..y * n + n].copy_from_slice(&row);
    }

    for x in 0..n {
        for (y, slot) in row.iter_mut().enumerate() {
            *slot = data[y * n + x];
        }
        ifft(&mut row);
        for (y, slot) in row.iter().enumerate() {
            data[y * n + x] = *slot;
        }
    }
}

/// In-place radix-2 decimation-in-time inverse DFT (positive exponent, no
/// 1/N scaling). `data.len()` must be a power of two.
fn ifft(data: &mut [Complex]) {
    let n = data.len();
    debug_assert!(n.is_power_of_two());

    // Bit-reversal permutation.
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            data.swap(i, j);
        }
    }

    let mut len = 2usize;
    while len <= n {
        let angle = 2.0 * PI / len as f32; // positive => inverse transform
        let step = exp_complex(angle);
        for chunk in data.chunks_mut(len) {
            let mut w = Vec2::new(1.0, 0.0);
            for i in 0..len / 2 {
                let u = chunk[i];
                let v = mul_complex(chunk[i + len / 2], w);
                chunk[i] = u + v;
                chunk[i + len / 2] = u - v;
                w = mul_complex(w, step);
            }
        }
        len <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Straight O(n²) inverse DFT with the same convention, as ground truth.
    fn naive_idft(data: &[Complex]) -> Vec<Complex> {
        let n = data.len();
        (0..n)
            .map(|k| {
                let mut acc = Complex::ZERO;
                for (i, x) in data.iter().enumerate() {
                    let angle = 2.0 * PI * (k * i) as f32 / n as f32;
                    acc += mul_complex(*x, exp_complex(angle));
                }
                acc
            })
            .collect()
    }

    #[test]
    fn ifft_matches_naive_transform() {
        for n in [8usize, 16, 64] {
            let mut data: Vec<Complex> = (0..n)
                .map(|i| {
                    let f = i as f32;
                    Vec2::new((f * 0.7).sin() * 1.3, (f * 0.31).cos() * 0.6)
                })
                .collect();
            let expected = naive_idft(&data);
            ifft(&mut data);
            for (got, want) in data.iter().zip(expected.iter()) {
                assert!(
                    (*got - *want).length() < 1e-2,
                    "n={n}: {got:?} != {want:?}"
                );
            }
        }
    }

    #[test]
    fn ifft_of_impulse_is_constant() {
        // A single DC bin must transform to a flat signal; catches a wrong
        // butterfly stride or a botched bit reversal.
        let mut data = vec![Complex::ZERO; 16];
        data[0] = Vec2::new(2.0, 0.0);
        ifft(&mut data);
        for value in data {
            assert!((value - Vec2::new(2.0, 0.0)).length() < 1e-4);
        }
    }

    #[test]
    fn zero_wind_surface_is_flat() {
        // No energy in the spectrum must mean no displacement at all — a
        // buoyant body on a dead calm sea should not twitch.
        let mut surface = WaterSurface::default();
        for cascade in &mut surface.cascades {
            cascade.wind_speed = 1e-4;
        }
        let mut field = WaterHeightField::default();
        field.update(&surface, &[0.0, 1.0, 2.0]);
        for xz in [Vec2::ZERO, Vec2::new(13.0, -7.5), Vec2::new(-99.0, 42.0)] {
            assert!(
                field.sample_height(xz).abs() < 0.05,
                "height {} at {xz:?} on a calm sea",
                field.sample_height(xz)
            );
        }
    }

    #[test]
    fn height_field_is_finite_and_bounded() {
        // Guards the whole chain: a NaN anywhere in the spectrum (log(0),
        // divide by k=0) would end up shoving physics bodies to infinity.
        let surface = WaterSurface::default();
        let mut field = WaterHeightField::default();
        field.update(&surface, &[120.0, 123.1, 126.3]);
        for i in 0..200 {
            let xz = Vec2::new(i as f32 * 3.7 - 300.0, i as f32 * -2.3 + 150.0);
            let d = field.sample_displacement(xz);
            assert!(d.is_finite(), "non-finite displacement {d:?} at {xz:?}");
            assert!(d.y.abs() < 50.0, "implausible height {} at {xz:?}", d.y);
        }
    }

    #[test]
    fn height_field_is_tile_periodic() {
        // Sampling one tile length away must give the same height; if it does
        // not, the UV wrap or the field indexing is wrong.
        let mut surface = WaterSurface::default();
        surface.cascades.truncate(1);
        let tile = surface.cascades[0].tile_length;
        let mut field = WaterHeightField::default();
        field.update(&surface, &[120.0]);

        for xz in [Vec2::new(3.0, 5.0), Vec2::new(-11.0, 2.5)] {
            let a = field.sample_height(xz);
            let b = field.sample_height(xz + Vec2::new(tile.x, 0.0));
            let c = field.sample_height(xz + Vec2::new(0.0, tile.y));
            assert!((a - b).abs() < 1e-3, "x-periodicity: {a} vs {b}");
            assert!((a - c).abs() < 1e-3, "z-periodicity: {a} vs {c}");
        }
    }

    #[test]
    fn waves_run_along_the_wind_axis() {
        // Pins the field's orientation, which is the one thing about this
        // module that cannot be checked against the GPU directly. With the wind
        // at angle 0 the spectrum peaks at `atan2(k.x, k.y) == 0`, i.e. a wave
        // vector along +ky — and because the maps are stored transposed (see
        // `modulate_and_transform`) that shows up as variation along world X.
        // If someone "fixes" the transpose, this flips to Z and buoyancy stops
        // matching the rendered waves.
        let mut surface = WaterSurface::default();
        surface.cascades.truncate(1);
        surface.cascades[0].wind_direction = 0.0;
        surface.cascades[0].spread = 0.0;
        surface.cascades[0].swell = 2.0;
        let mut field = WaterHeightField::default();
        field.update(&surface, &[120.0]);

        let tile = surface.cascades[0].tile_length;
        let steps = 128;
        // Measure *variation* along each axis (consecutive differences), not
        // the magnitude of the height — a wave travelling along X still has a
        // large |h| everywhere along Z, it just doesn't change there.
        let mut along_x = 0.0f32;
        let mut along_z = 0.0f32;
        let mut prev_x = field.sample_height(Vec2::ZERO);
        let mut prev_z = prev_x;
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let hx = field.sample_height(Vec2::new(t * tile.x, 0.0));
            let hz = field.sample_height(Vec2::new(0.0, t * tile.y));
            along_x += (hx - prev_x).abs();
            along_z += (hz - prev_z).abs();
            prev_x = hx;
            prev_z = hz;
        }
        // 1.5x, not a tight bound: this is an orientation check, and how
        // anisotropic any one realization comes out varies. A transposed field
        // inverts the ratio, which this catches easily.
        assert!(
            along_x > along_z * 1.5,
            "expected variation along X (got {along_x} vs {along_z} along Z)"
        );
    }

    #[test]
    fn cascade_seeds_are_decorrelated() {
        // Two cascades with *identical* parameters must still produce different
        // random realizations. They used to share one seed, which made every
        // cascade the same Gaussian field rescaled — so their crests lined up
        // and the layering bought nothing but amplitude.
        let cascade = WaveCascade::default();
        let a = build_spectrum(&cascade, 20.0, 1234, 0, 512);
        let b = build_spectrum(&cascade, 20.0, 1234, 1, 512);

        let identical = a
            .iter()
            .zip(b.iter())
            .filter(|(x, y)| (x.h0 - y.h0).length() < 1e-9)
            .count();
        // A handful of texels can coincide by chance (both are zero where the
        // spectrum has no energy); the whole field matching is the failure.
        assert!(
            identical * 4 < a.len(),
            "{identical}/{} texels identical across cascades — seeds are correlated",
            a.len()
        );
    }

    #[test]
    fn same_cascade_index_is_reproducible() {
        // The flip side: the seed must still be deterministic, or a scene would
        // reload as a different ocean and the CPU mirror would disagree with
        // the GPU (which derives its seed from the same function).
        let cascade = WaveCascade::default();
        let a = build_spectrum(&cascade, 20.0, 1234, 2, 512);
        let b = build_spectrum(&cascade, 20.0, 1234, 2, 512);
        assert!(a.iter().zip(b.iter()).all(|(x, y)| x.h0 == y.h0));
    }

    #[test]
    fn height_field_moves_with_time() {
        // Two different simulation times must give different surfaces,
        // otherwise the modulation stage is dropping the time term.
        let surface = WaterSurface::default();
        let mut a = WaterHeightField::default();
        let mut b = WaterHeightField::default();
        a.update(&surface, &[120.0, 123.1, 126.3]);
        b.update(&surface, &[123.0, 126.1, 129.3]);
        let differs = (0..64).any(|i| {
            let xz = Vec2::new(i as f32 * 4.0, i as f32 * 1.5);
            (a.sample_height(xz) - b.sample_height(xz)).abs() > 1e-3
        });
        assert!(differs, "surface did not change over 3 seconds");
    }
}

