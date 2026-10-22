//! Light from particles: what an emitter's [`ParticleLight`] lights the scene
//! with, worked out from its live particles each frame.
//!
//! The lights themselves are ordinary Bevy `PointLight`s on hidden children of
//! the effect entity (`crate::systems` spawns and moves them). This module is
//! the part that decides what they should be, kept free of the ECS so it can
//! be tested on a particle buffer alone:
//!
//! - [`aggregate`] turns the particles into target lights. Each particle
//!   weighs its colour's luminance times its alpha (clamped to 0..1), which is
//!   how much it adds to an additive image. `PerEmitter` sums the weights into
//!   one light at the emitter whose colour is the weighted average colour;
//!   `BrightestN` puts one light on each of the heaviest particles.
//! - [`LightRig`] eases the lights toward those targets over the setting's
//!   smoothing time, so a fire that spawns and kills particles every frame
//!   does not strobe, and matches each light to the nearest target from one
//!   frame to the next, so a `BrightestN` light follows a particle rather than
//!   jumping to whichever is now third brightest.
//! - [`flicker`] dims the result by a smooth noise of the effect's time.
//!
//! Everything runs on the simulation's own clock, so a paused effect's light
//! holds still and a scrubbed one is where the frame puts it.
//!
//! ## Solari
//!
//! Solari ignores `PointLight`s, so `renzora_solari` stands every visible
//! point light on render layer 0 in as an emissive sphere of the light's
//! radius and power (its `sync_light_proxies`). These lights are plain
//! `PointLight`s with inherited visibility, so they get that treatment with
//! nothing done here: a particle fire lights a Solari scene too. Each
//! flickering light changes its proxy material's emissive every frame, which
//! Solari re-uploads; at a few lights that is nothing, and it is one more
//! reason `BrightestN` is capped at [`crate::asset::MAX_PARTICLE_LIGHTS`].

use bevy::math::{Affine3A, Vec3, Vec4};

use crate::asset::{LightMode, ParticleLight};
use crate::sim::ParticleBuffer;

/// One light, where and how bright it should be. Positions are in the effect
/// entity's local space, which is the light children's space.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LightSample {
    pub position: Vec3,
    /// Linear RGB, brightest channel 1 (or black).
    pub color: Vec3,
    /// Lumens.
    pub intensity: f32,
}

/// How much a particle adds to the light: its colour's luminance times its
/// alpha, which is what it adds to an additive image.
pub fn weight(color: Vec4) -> f32 {
    let lum = 0.2126 * color.x.max(0.0) + 0.7152 * color.y.max(0.0) + 0.0722 * color.z.max(0.0);
    let w = lum * color.w.clamp(0.0, 1.0);
    if w.is_finite() {
        w
    } else {
        0.0
    }
}

/// `rgb` scaled so its brightest channel is 1, then tinted.
fn chroma(rgb: Vec3, tint: Vec3) -> Vec3 {
    let m = rgb.max_element();
    let c = if m > 1e-6 && m.is_finite() { rgb / m } else { Vec3::ONE };
    c * tint
}

fn tint(light: &ParticleLight) -> Vec3 {
    let t = Vec3::from_array(light.color);
    if t.is_finite() {
        t.max(Vec3::ZERO)
    } else {
        Vec3::ONE
    }
}

/// The lights `light` asks for over `particles`, before smoothing and
/// flicker, into `out` (cleared first). `to_local` takes a particle's
/// simulation-space position into the entity's local space: identity for a
/// `Local` emitter, the inverse of the entity's transform for a `World` one.
pub fn aggregate(particles: &ParticleBuffer, light: &ParticleLight, to_local: Affine3A, out: &mut Vec<LightSample>) {
    out.clear();
    let intensity = if light.intensity.is_finite() { light.intensity.max(0.0) } else { 0.0 };
    match light.mode {
        LightMode::Off => {}
        LightMode::PerEmitter => {
            let mut total = 0.0;
            let mut rgb = Vec3::ZERO;
            for &c in &particles.color {
                let w = weight(c);
                total += w;
                rgb += c.truncate().max(Vec3::ZERO) * c.w.clamp(0.0, 1.0);
            }
            out.push(per_emitter(light, total, rgb));
        }
        LightMode::BrightestN(_) => {
            let n = light.light_count();
            // The n heaviest, heaviest first; ties broken by id so the pick is
            // the same on every replay.
            let mut order: Vec<(f32, u32, usize)> = (0..particles.len())
                .map(|i| (weight(particles.color[i]), particles.id[i], i))
                .filter(|(w, _, _)| *w > 0.0)
                .collect();
            order.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
            for &(w, _, i) in order.iter().take(n) {
                let position = to_local.transform_point3(particles.position[i]);
                if !position.is_finite() {
                    continue;
                }
                out.push(LightSample {
                    position,
                    color: chroma(particles.color[i].truncate().max(Vec3::ZERO), tint(light)),
                    intensity: w * intensity,
                });
            }
        }
    }
}

/// A `PerEmitter` light from its two sums: the particles' total [`weight`]
/// and their colours weighted by alpha. The GPU backend reduces exactly these
/// two on the device and reads them back, so its light is this function of
/// the same numbers, a frame late.
pub fn per_emitter(light: &ParticleLight, total_weight: f32, weighted_rgb: Vec3) -> LightSample {
    let intensity = if light.intensity.is_finite() { light.intensity.max(0.0) } else { 0.0 };
    let offset = Vec3::from_array(light.offset);
    let total = if total_weight.is_finite() { total_weight.max(0.0) } else { 0.0 };
    LightSample {
        position: if offset.is_finite() { offset } else { Vec3::ZERO },
        color: chroma(weighted_rgb, tint(light)),
        intensity: total * intensity,
    }
}

/// A smooth 0..1 noise of `t`: two octaves of value noise, seeded per
/// emitter so two fires beside each other do not flicker in step.
fn flicker_noise(t: f32, seed: u64) -> f32 {
    let hash = |i: i64| -> f32 {
        let h = crate::random::pcg((i as u32) ^ crate::random::pcg(seed as u32 ^ (seed >> 32) as u32 ^ 0xF11C_4E25));
        crate::random::unit_float(h)
    };
    let octave = |t: f32| -> f32 {
        let i = t.floor();
        let f = t - i;
        let u = f * f * (3.0 - 2.0 * f);
        let a = hash(i as i64);
        let b = hash(i as i64 + 1);
        a + (b - a) * u
    };
    (octave(t) * 0.65 + octave(t * 2.31 + 17.0) * 0.35).clamp(0.0, 1.0)
}

/// The flicker multiplier at effect time `t`: 1 with no flicker, dipping by up
/// to `light.flicker` below it.
pub fn flicker(light: &ParticleLight, seed: u64, t: f32) -> f32 {
    let amount = if light.flicker.is_finite() { light.flicker.clamp(0.0, 1.0) } else { 0.0 };
    if amount <= 0.0 || !t.is_finite() {
        return 1.0;
    }
    let rate = if light.flicker_rate.is_finite() { light.flicker_rate.max(0.0) } else { 0.0 };
    1.0 - amount * flicker_noise(t * rate, seed)
}

/// The lights of one emitter as they are drawn, eased toward their targets.
#[derive(Clone, Debug, Default)]
pub struct LightRig {
    /// One per light entity, in entity order.
    pub lights: Vec<LightSample>,
    /// The effect time the rig last moved to; `None` until the first frame,
    /// which snaps rather than eases.
    last_time: Option<f32>,
}

impl LightRig {
    pub fn new(count: usize) -> Self {
        Self { lights: vec![LightSample::default(); count], last_time: None }
    }

    /// Move toward `targets` for the effect's clock now reading `time`. Time
    /// going backwards (a restart or a seek) snaps, since easing from where
    /// the light was would be easing from a moment that no longer happened.
    pub fn update(&mut self, targets: &[LightSample], time: f32, smoothing: f32) {
        let dt = match self.last_time {
            Some(last) if time >= last => time - last,
            _ => f32::INFINITY,
        };
        self.last_time = Some(time);
        let k = if smoothing.is_finite() && smoothing > 0.0 { 1.0 - (-dt / smoothing).exp() } else { 1.0 };
        let k = if k.is_finite() { k.clamp(0.0, 1.0) } else { 1.0 };

        // Greedy nearest match: each light, in order, takes the nearest target
        // not yet taken. With a handful of lights that is as good as an
        // optimal assignment and far simpler.
        let mut taken = vec![false; targets.len()];
        for light in &mut self.lights {
            let best = targets
                .iter()
                .enumerate()
                .filter(|(j, _)| !taken[*j])
                .min_by(|(_, a), (_, b)| {
                    a.position.distance_squared(light.position).total_cmp(&b.position.distance_squared(light.position))
                })
                .map(|(j, _)| j);
            match best {
                Some(j) => {
                    taken[j] = true;
                    let t = targets[j];
                    if light.intensity <= 1e-6 {
                        // A light coming on appears where it is needed instead
                        // of sliding in from where it last went out.
                        light.position = t.position;
                        light.color = t.color;
                    } else {
                        light.position += (t.position - light.position) * k;
                        light.color += (t.color - light.color) * k;
                    }
                    light.intensity += (t.intensity - light.intensity) * k;
                }
                None => light.intensity -= light.intensity * k,
            }
        }
    }
}
