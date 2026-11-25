//! Flickers a light's intensity over time, for torches, campfires and faulty
//! bulbs: a script animating the light's own intensity, so no engine support is
//! needed. The range can pulse with it, reaching farther on flares and
//! shorter on dips.
//!
//! Scripts only run in Play (or with the script's own play toggle), so there
//! is no flicker while editing.
//!
//! Styles (`STYLE`):
//! - `Campfire`: a lively fire, slow swell plus fast flicker plus random flares
//!   and dips. The only style that also flares above the base.
//! - `Torch`: layered sines at incommensurate rates, a steady organic flicker.
//! - `Pulse`: gentle sine breathing, for magic or sci-fi.
//! - `Flicker`: sharp random levels with occasional deep dips, a broken bulb.
//!
//! Setup: attach to the light entity (a point or spot light). Which one is
//! detected; set `FORCE_LIGHT` to pick one when an entity has both.

use bevy::prelude::*;
use renzora::ScriptCtx;

#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum Style {
    Campfire,
    Torch,
    Pulse,
    Flicker,
}

#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum LightKind {
    Point,
    Spot,
}

const STYLE: Style = Style::Campfire;
/// `None` detects the light on this entity.
const FORCE_LIGHT: Option<LightKind> = None;
/// Flicker depth 0..1: how far intensity dips. Try 0.5 for a campfire, 0.8 for
/// a broken bulb.
const AMOUNT: f32 = 0.5;
const SPEED: f32 = 12.0;
/// Lower clamp on the intensity multiplier, so the light never goes fully dark.
const MIN_FACTOR: f32 = 0.2;
/// Overall multiplier; raise it for a much brighter fire.
const BRIGHTNESS: f32 = 1.0;
/// Base intensity; 0 captures the light's own value on start.
const BASE_INTENSITY: f32 = 0.0;
/// Range during dips and flares. `RANGE_MAX` of 0 leaves the range alone.
const RANGE_MIN: f32 = 4.5;
const RANGE_MAX: f32 = 6.0;
/// Used when there is no base to capture.
const FALLBACK_INTENSITY: f32 = 1000.0;

#[derive(Component)]
struct Flicker {
    kind: LightKind,
    base: f32,
    t: f32,
    /// Seconds (in flicker time) left holding the current random target.
    hold: f32,
    cur: f32,
    target: f32,
    seed: u64,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    if !ctx.has::<Flicker>() {
        // Resolved lazily, so a light added after the script started is found.
        let Some(kind) = resolve(ctx) else { return };
        let authored = intensity(ctx, kind).unwrap_or(0.0);
        let base = if BASE_INTENSITY > 0.0 {
            BASE_INTENSITY
        } else if authored > 0.0 {
            authored
        } else {
            FALLBACK_INTENSITY
        };
        let seed = ctx.entity().to_bits() ^ ((ctx.elapsed().to_bits() as u64) << 32);
        ctx.insert(Flicker { kind, base, t: 0.0, hold: 0.0, cur: 1.0, target: 1.0, seed });
    }
    let Some(mut f) = ctx.get_mut::<Flicker>() else { return };
    f.t += dt * SPEED;
    let t = f.t;

    // Noise in 0..1, shaped per style.
    let n = match STYLE {
        Style::Campfire => {
            // Mostly fast flicker and frequent random flares or dips, with just
            // a hint of slow swell underneath.
            let slow = 0.5 + 0.5 * (t * 0.5).sin();
            let fast = 0.5 + 0.5 * (0.6 * t.sin() + 0.4 * (t * 2.9 + 1.3).sin());
            f.hold -= dt * SPEED;
            if f.hold <= 0.0 {
                f.target = rand01(&mut f.seed);
                f.hold = 0.06 + rand01(&mut f.seed) * 0.22;
            }
            f.cur += (f.target - f.cur) * (dt * SPEED * 6.0).min(1.0);
            0.45 * fast + 0.15 * slow + 0.40 * f.cur
        }
        Style::Pulse => 0.5 + 0.5 * t.sin(),
        Style::Flicker => {
            f.hold -= dt * SPEED;
            if f.hold <= 0.0 {
                f.target = rand01(&mut f.seed);
                if rand01(&mut f.seed) < 0.2 {
                    f.target *= 0.15;
                }
                f.hold = 0.4 + rand01(&mut f.seed) * 0.8;
            }
            f.cur += (f.target - f.cur) * (dt * SPEED * 3.0).min(1.0);
            f.cur
        }
        Style::Torch => {
            0.5 + 0.5 * (0.55 * t.sin() + 0.30 * (t * 2.3 + 1.7).sin() + 0.15 * (t * 5.1 + 4.2).sin())
        }
    };
    let (kind, base) = (f.kind, f.base);
    drop(f);

    // Campfire is bipolar (dips and flares around the base); the others only
    // dim from it.
    let m = if STYLE == Style::Campfire {
        1.0 + AMOUNT * (n * 2.0 - 1.0)
    } else {
        (1.0 - AMOUNT) + AMOUNT * n
    }
    .max(MIN_FACTOR);
    let value = base * BRIGHTNESS * m;
    let range = (RANGE_MAX > 0.0).then(|| RANGE_MIN + (RANGE_MAX - RANGE_MIN) * n);

    match kind {
        LightKind::Point => {
            if let Some(mut light) = ctx.get_mut::<PointLight>() {
                light.intensity = value;
                if let Some(range) = range {
                    light.range = range;
                }
            }
        }
        LightKind::Spot => {
            if let Some(mut light) = ctx.get_mut::<SpotLight>() {
                light.intensity = value;
                if let Some(range) = range {
                    light.range = range;
                }
            }
        }
    }
}

fn resolve(ctx: &ScriptCtx) -> Option<LightKind> {
    let has = |kind| match kind {
        LightKind::Point => ctx.has::<PointLight>(),
        LightKind::Spot => ctx.has::<SpotLight>(),
    };
    if let Some(kind) = FORCE_LIGHT.filter(|k| has(*k)) {
        return Some(kind);
    }
    [LightKind::Point, LightKind::Spot].into_iter().find(|k| has(*k))
}

fn intensity(ctx: &ScriptCtx, kind: LightKind) -> Option<f32> {
    match kind {
        LightKind::Point => ctx.get::<PointLight>().map(|l| l.intensity),
        LightKind::Spot => ctx.get::<SpotLight>().map(|l| l.intensity),
    }
}

/// A uniform value in 0..1 from a SplitMix64 step; scripts have no rand crate.
fn rand01(seed: &mut u64) -> f32 {
    *seed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *seed;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 24) as f32
}

renzora::script!(update);
