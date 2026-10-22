//! Blackbody colour: what a hot thing glows like at a given temperature.
//!
//! Fire is coloured by temperature, not by artistic choice: soot in a flame is
//! a blackbody radiator, so the white-yellow core, the orange body and the deep
//! red tips are one curve sampled at falling temperatures (Nguyen, Fedkiw and
//! Jensen, "Physically Based Modeling and Animation of Fire", 2002, render
//! their flames exactly this way). Painting those stops by hand is how a fire
//! ends up pink at the edges; sampling the physics is how it does not.
//!
//! The colour is computed properly once and tabulated: Planck's law integrated
//! against the CIE 1931 2-degree colour matching functions (the analytic
//! multi-lobe fit of Wyman, Sloan and Shirley, JCGT 2013), converted to linear
//! Rec.709 and normalised so the brightest channel is 1. Brightness is left to
//! the caller: a physical radiance spans orders of magnitude between an ember
//! and a core, which is a tonemapper's problem, and every effect wants its own
//! HDR scale anyway. Out-of-gamut negatives (a low temperature's blue) clamp to
//! zero.
//!
//! The table is uniform in reciprocal temperature (mireds), because
//! chromaticity changes evenly in 1/T: the same number of stops in kelvin would
//! spend most of them on near-identical blue-whites and leave the reds coarse.

use std::sync::OnceLock;

use crate::curve::GradientStop;

/// Coolest temperature the table covers. Anything colder reads as this deep
/// red; below about 800 K nothing is visible at all, so nothing is lost.
pub const MIN_KELVIN: f32 = 500.0;
/// Hottest temperature the table covers, a blue-white well past any flame.
pub const MAX_KELVIN: f32 = 40_000.0;
/// Stops in the table. Even in mireds each is about 15 apart, which keeps the
/// linear interpolation between them under any visible error.
pub const TABLE_STOPS: usize = 128;

/// `1e6 / MIN_KELVIN` and `1e6 / MAX_KELVIN`.
pub const MIRED_MAX: f32 = 1.0e6 / MIN_KELVIN;
pub const MIRED_MIN: f32 = 1.0e6 / MAX_KELVIN;

/// The table position of `kelvin`: 0 at [`MIN_KELVIN`], 1 at [`MAX_KELVIN`],
/// linear in mireds. The compiler emits this same arithmetic as IR.
pub fn table_t(kelvin: f32) -> f32 {
    let mired = 1.0e6 / kelvin.max(1.0);
    ((MIRED_MAX - mired) / (MIRED_MAX - MIRED_MIN)).clamp(0.0, 1.0)
}

/// One lobe of the Wyman fit: a Gaussian with a different width each side.
fn lobe(x: f64, mu: f64, s1: f64, s2: f64) -> f64 {
    let s = if x < mu { s1 } else { s2 };
    let t = (x - mu) / s;
    (-0.5 * t * t).exp()
}

/// CIE 1931 2-degree colour matching functions at `nm` nanometres.
fn cie_xyz(nm: f64) -> [f64; 3] {
    let x = 1.056 * lobe(nm, 599.8, 37.9, 31.0) + 0.362 * lobe(nm, 442.0, 16.0, 26.7)
        - 0.065 * lobe(nm, 501.1, 20.4, 26.2);
    let y = 0.821 * lobe(nm, 568.8, 46.9, 40.5) + 0.286 * lobe(nm, 530.9, 16.3, 31.1);
    let z = 1.217 * lobe(nm, 437.0, 11.8, 36.0) + 0.681 * lobe(nm, 459.0, 26.0, 13.8);
    [x, y, z]
}

/// Planck spectral radiance up to a constant factor, at `nm` and `kelvin`.
fn planck(nm: f64, kelvin: f64) -> f64 {
    // Second radiation constant hc/k, in metre-kelvin.
    const C2: f64 = 1.438_776_9e-2;
    let l = nm * 1e-9;
    let e = (C2 / (l * kelvin)).exp() - 1.0;
    if !e.is_finite() || e <= 0.0 {
        return 0.0;
    }
    1.0 / (l.powi(5) * e)
}

/// The linear Rec.709 colour of a blackbody at `kelvin`, brightest channel 1.
pub fn blackbody_rgb(kelvin: f32) -> [f32; 3] {
    let t = f64::from(kelvin.clamp(MIN_KELVIN, MAX_KELVIN));
    let mut xyz = [0.0f64; 3];
    let mut nm = 360.0;
    while nm <= 830.0 {
        let p = planck(nm, t);
        let c = cie_xyz(nm);
        for i in 0..3 {
            xyz[i] += p * c[i];
        }
        nm += 5.0;
    }
    let [x, y, z] = xyz;
    let rgb = [
        3.2406 * x - 1.5372 * y - 0.4986 * z,
        -0.9689 * x + 1.8758 * y + 0.0415 * z,
        0.0557 * x - 0.2040 * y + 1.0570 * z,
    ]
    .map(|c| c.max(0.0));
    let max = rgb.iter().copied().fold(0.0f64, f64::max);
    if max <= 0.0 || !max.is_finite() {
        return [1.0, 0.0, 0.0];
    }
    rgb.map(|c| (c / max) as f32)
}

/// The tabulated ramp `color/blackbody` samples: [`TABLE_STOPS`] stops uniform
/// in mireds from [`MIN_KELVIN`] to [`MAX_KELVIN`], alpha 1. Built once.
pub fn table() -> &'static [GradientStop] {
    static TABLE: OnceLock<Vec<GradientStop>> = OnceLock::new();
    TABLE.get_or_init(|| {
        (0..TABLE_STOPS)
            .map(|i| {
                let t = i as f32 / (TABLE_STOPS - 1) as f32;
                let mired = MIRED_MAX + (MIRED_MIN - MIRED_MAX) * t;
                let [r, g, b] = blackbody_rgb(1.0e6 / mired);
                GradientStop { t, color: [r, g, b, 1.0] }
            })
            .collect()
    })
}

/// A fire's colour over a particle's life, for the gradient editor's
/// Blackbody button: a 3800 K white-yellow cooling to an 800 K red, dimming as
/// it cools (radiance falls steeply with temperature) and fading out over the
/// last third. Kept within 0..1 so the colour field can edit every stop; scale
/// the particle's colour for HDR.
pub fn fire_gradient() -> Vec<GradientStop> {
    const STOPS: usize = 8;
    (0..STOPS)
        .map(|i| {
            let t = i as f32 / (STOPS - 1) as f32;
            let kelvin = 3800.0 + (800.0 - 3800.0) * t;
            let [r, g, b] = blackbody_rgb(kelvin);
            let bright = ((kelvin - 600.0) / (3800.0 - 600.0)).clamp(0.0, 1.0).powf(1.5);
            let alpha = if t < 0.65 { 1.0 } else { 1.0 - (t - 0.65) / 0.35 };
            GradientStop { t, color: [r * bright, g * bright, b * bright, alpha.clamp(0.0, 1.0)] }
        })
        .collect()
}
