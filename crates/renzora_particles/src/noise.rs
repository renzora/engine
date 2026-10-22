//! Curl noise for `force/curl_noise`.
//!
//! Three channels of 3D value noise act as a vector potential, and the force is
//! its curl. A curl has zero divergence, which is the whole reason to use one:
//! particles pushed by it swirl and fold without collecting at sinks or
//! thinning out at sources, so smoke stays smoke-shaped instead of clumping.
//!
//! The noise returns its analytic gradient alongside its value (quintic
//! interpolation, after Inigo Quilez's derivation), so the curl costs three
//! noise evaluations rather than the eighteen central differences would. The
//! lattice is hashed with the same PCG mixer as particle randoms, so a GPU
//! backend can reproduce it exactly.

use bevy::math::Vec3;

use crate::random::pcg;

/// A lattice value in -1..1 for one integer corner of one channel.
#[inline]
fn lattice(x: i32, y: i32, z: i32, channel: u32) -> f32 {
    let h = pcg((x as u32) ^ pcg((y as u32) ^ pcg((z as u32) ^ pcg(channel))));
    (h >> 8) as f32 * (2.0 / 16_777_216.0) - 1.0
}

/// Value noise and its gradient at `p`.
pub fn value_noise_grad(p: Vec3, channel: u32) -> (f32, Vec3) {
    let i = p.floor();
    let f = p - i;
    let (ix, iy, iz) = (i.x as i32, i.y as i32, i.z as i32);

    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let du = 30.0 * f * f * (f * (f - 2.0) + 1.0);

    let a = lattice(ix, iy, iz, channel);
    let b = lattice(ix + 1, iy, iz, channel);
    let c = lattice(ix, iy + 1, iz, channel);
    let d = lattice(ix + 1, iy + 1, iz, channel);
    let e = lattice(ix, iy, iz + 1, channel);
    let f1 = lattice(ix + 1, iy, iz + 1, channel);
    let g = lattice(ix, iy + 1, iz + 1, channel);
    let h = lattice(ix + 1, iy + 1, iz + 1, channel);

    let k0 = a;
    let k1 = b - a;
    let k2 = c - a;
    let k3 = e - a;
    let k4 = a - b - c + d;
    let k5 = a - c - e + g;
    let k6 = a - b - e + f1;
    let k7 = -a + b + c - d + e - f1 - g + h;

    let value = k0
        + k1 * u.x
        + k2 * u.y
        + k3 * u.z
        + k4 * u.x * u.y
        + k5 * u.y * u.z
        + k6 * u.z * u.x
        + k7 * u.x * u.y * u.z;
    let grad = du
        * Vec3::new(
            k1 + k4 * u.y + k6 * u.z + k7 * u.y * u.z,
            k2 + k5 * u.z + k4 * u.x + k7 * u.z * u.x,
            k3 + k6 * u.x + k5 * u.y + k7 * u.x * u.y,
        );
    (value, grad)
}

/// Curl of the three-channel potential at `p`.
pub fn curl(p: Vec3) -> Vec3 {
    if !p.is_finite() {
        return Vec3::ZERO;
    }
    let (_, g1) = value_noise_grad(p, 0x1);
    let (_, g2) = value_noise_grad(p, 0x2);
    let (_, g3) = value_noise_grad(p, 0x3);
    Vec3::new(g3.y - g2.z, g1.z - g3.x, g2.x - g1.y)
}

/// The most octaves [`curl_fbm`] sums. Past six the finest octave's swirls are
/// smaller than any particle anyone would push with them; the cap is what
/// keeps the loop bounded for a GPU backend and a typo from costing a frame.
pub const MAX_OCTAVES: f32 = 8.0;

/// How far each octave's lattice is shifted from the one before. Without it
/// every octave has a lattice corner at the origin, where all of their values
/// are correlated, and the field visibly pinches there. Octave 0 is unshifted
/// so one octave is exactly [`curl`].
const OCTAVE_SHIFT: Vec3 = Vec3::new(17.13, 5.71, 11.37);

/// Fractal curl noise: `octaves` octaves of [`curl`] (the fraction fading the
/// last one in), each `lacunarity` times the frequency and `persistence` times
/// the amplitude of the one before, starting at amplitude 1 and summed without
/// normalising. That is the same sum as Hanabi's noise turbulence (which fixed
/// the persistence at 0.5), so a converted effect keeps its strength: two
/// octaves are up to half again as strong as one, not the same.
///
/// The sum of curls is a curl (of the summed potential), so the result is
/// still divergence-free.
pub fn curl_fbm(p: Vec3, octaves: f32, lacunarity: f32, persistence: f32) -> Vec3 {
    if !p.is_finite() || !octaves.is_finite() {
        return Vec3::ZERO;
    }
    let octaves = octaves.clamp(0.0, MAX_OCTAVES);
    let lacunarity = if lacunarity.is_finite() { lacunarity } else { 2.0 };
    let persistence = if persistence.is_finite() { persistence } else { 0.5 };
    let mut sum = Vec3::ZERO;
    let mut freq = 1.0;
    let mut amp = 1.0;
    let mut i = 0.0;
    while i < octaves {
        let weight = (octaves - i).min(1.0);
        sum += curl(p * freq + OCTAVE_SHIFT * i) * (amp * weight);
        freq *= lacunarity;
        amp *= persistence;
        i += 1.0;
    }
    sum
}
