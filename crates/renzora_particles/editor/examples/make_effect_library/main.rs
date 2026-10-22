//! Build a curated library of `.vfx` effects into a project folder.
//!
//! ```sh
//! cargo run --profile dist -p renzora_particles_editor --example make_effect_library -- <out_dir> [--force] [--only <category>]
//! ```
//!
//! Every effect is built through the public API (the converter's
//! [`GraphBuilder`], so each graph gets the same tidy column layout a converted
//! effect gets in the ember graph view), compiled with the real compiler and
//! simulated for three seconds on the CPU, once in calm air and once in a
//! breeze. An effect is written only when every emitter compiles, nothing goes
//! NaN, every emitter has had live particles, none dropped a spawn for lack of
//! capacity, and its RON survives a round trip through the editor's own
//! serializer. If any effect fails, nothing at all is written, so a library is
//! never left half generated.
//!
//! Files land in `<out_dir>/<category>/<name>.vfx`. An existing file is never
//! overwritten without `--force`: the user may have tuned it since.
//!
//! Scale is real-world metres throughout (pixels for the 2D tests). Each
//! builder carries a comment on why it is shaped the way it is.
//!
//! The shared helpers and the original sets live here; the `popular` and
//! `stress` sets are their own modules, so this file stays readable.

mod popular;
mod stress;

use std::f32::consts::TAU;
use std::path::PathBuf;

use bevy::math::{Affine3A, Vec3};
use renzora_particles::asset::{
    parse_vfx, write_vfx, Backend, BlendMode, Dimension, DrawOrder, Emitter, Facing, LightMode, Lighting, MeshOrient,
    ParticleLight, ParticleMesh, ParticleShadows, PinValue, RenderSettings, Ribbon, RibbonKind, RibbonShape, Shading,
    SheetMode, SimulationSpace, SpriteSheet, SubEmitter, SubEmitterTrigger, VfxAsset, VfxGraph,
};
use renzora_particles::curve::{CurvePoint, Ease, GradientStop};
use renzora_particles::light::{aggregate, LightSample};
use renzora_particles::params::{KeyInterp, ParamKey, ParamTarget, ParamTrack};
use renzora_particles::sim::{EffectSim, FrameInput};
use renzora_particles_editor::convert::build::{GraphBuilder, Src, OUTPUT_ID};

type B = GraphBuilder;
use SimulationSpace::{Local, World};

// ── Value helpers ───────────────────────────────────────────────────────────

fn f(v: f32) -> Src {
    Src::float(v)
}

fn v3(x: f32, y: f32, z: f32) -> Src {
    Src::vec3([x, y, z])
}

fn v2(x: f32, y: f32) -> Src {
    Src::Value(PinValue::Vec2([x, y]))
}

fn col(c: [f32; 4]) -> Src {
    Src::Value(PinValue::Color(c))
}

fn rand(b: &mut B, min: f32, max: f32) -> Src {
    b.op("random/range", &[("min", f(min)), ("max", f(max))], "value")
}

fn rand3(b: &mut B, min: [f32; 3], max: [f32; 3]) -> Src {
    b.op("random/vec3_range", &[("min", Src::vec3(min)), ("max", Src::vec3(max))], "value")
}

fn rand_color(b: &mut B, a: [f32; 4], c: [f32; 4]) -> Src {
    b.op("random/color_between", &[("a", col(a)), ("b", col(c))], "color")
}

/// A shape node, returning its (position, direction) pins.
fn shape(b: &mut B, ty: &str, inputs: &[(&str, PinValue)]) -> (Src, Src) {
    let id = b.node(ty);
    for (pin, v) in inputs {
        b.set(id, pin, v.clone());
    }
    (Src::Pin(id, "position"), Src::Pin(id, "direction"))
}

fn bed(b: &mut B, size: [f32; 3]) -> (Src, Src) {
    shape(b, "shape/box", &[("size", PinValue::Vec3(size))])
}

fn lift(b: &mut B, pos: Src, y: f32) -> Src {
    if y == 0.0 {
        pos
    } else {
        b.add(pos, v3(0.0, y, 0.0))
    }
}

fn curve(b: &mut B, points: &[(f32, f32, Ease)]) -> Src {
    let id = b.node("curve/curve");
    let points = points.iter().map(|&(t, value, ease)| CurvePoint { t, value, ease }).collect();
    b.set(id, "points", PinValue::Curve(points));
    Src::Pin(id, "value")
}

fn gradient(b: &mut B, stops: &[(f32, [f32; 4])]) -> Src {
    let id = b.node("curve/gradient");
    let stops = stops.iter().map(|&(t, color)| GradientStop { t, color }).collect();
    b.set(id, "stops", PinValue::Gradient(stops));
    Src::Pin(id, "color")
}

fn curl(b: &mut B, strength: f32, frequency: f32, scroll: [f32; 3]) -> Src {
    b.op(
        "force/curl_noise",
        &[("strength", f(strength)), ("frequency", f(frequency)), ("scroll", Src::vec3(scroll))],
        "force",
    )
}

fn wind(b: &mut B, coupling: f32, height_falloff: f32) -> Src {
    let id = b.node("force/wind");
    b.set(id, "coupling", PinValue::Float(coupling));
    if height_falloff > 0.0 {
        b.set(id, "height_falloff", PinValue::Float(height_falloff));
    }
    Src::Pin(id, "force")
}

fn drag(b: &mut B, linear: f32, quadratic: f32) -> Src {
    b.op("force/drag", &[("linear", f(linear)), ("quadratic", f(quadratic))], "force")
}

/// Wire the sum of `forces` into the output's acceleration.
fn accelerate(b: &mut B, forces: Vec<Src>) {
    if let Some(total) = b.sum(forces) {
        b.output("acceleration", total);
    }
}

/// A ground plane through the effect's origin. Acts by being in the graph.
fn ground(b: &mut B, bounce: f32, friction: f32, kill: bool) {
    let id = b.node("collide/plane");
    b.set(id, "bounce", PinValue::Float(bounce));
    b.set(id, "friction", PinValue::Float(friction));
    if kill {
        b.set(id, "kill", PinValue::Bool(true));
    }
}

/// `sin(age * rate + phase)`, rate and phase random per particle.
fn wave(b: &mut B, rate: (f32, f32)) -> Src {
    let age = b.op("input/age", &[], "value");
    let r = rand(b, rate.0, rate.1);
    let t = b.mul(age, r);
    let phase = rand(b, 0.0, TAU);
    let x = b.add(t, phase);
    b.op("math/sin", &[("value", x)], "value")
}

/// A per-particle brightness flicker between `lo` and `hi`.
fn flicker(b: &mut B, rate: (f32, f32), lo: f32, hi: f32) -> Src {
    let s = wave(b, rate);
    b.op(
        "math/remap",
        &[("value", s), ("in_min", f(-1.0)), ("in_max", f(1.0)), ("out_min", f(lo)), ("out_max", f(hi))],
        "value",
    )
}

fn random_spin(b: &mut B, spin: f32) {
    let r = rand(b, 0.0, TAU);
    b.output("rotation", r);
    if spin > 0.0 {
        let s = rand(b, -spin, spin);
        b.output("angular_velocity", s);
    }
}

fn ease(t: f32, value: f32) -> (f32, f32, Ease) {
    (t, value, Ease::Linear)
}

fn ease_out(t: f32, value: f32) -> (f32, f32, Ease) {
    (t, value, Ease::EaseOut)
}

// ── Render helpers ──────────────────────────────────────────────────────────

fn flame_render(stretch: f32) -> RenderSettings {
    RenderSettings {
        blend: BlendMode::Additive,
        facing: Facing::VelocityAligned,
        shading: Shading::ProceduralFlame,
        stretch,
        soft_distance: 0.05,
        ..Default::default()
    }
}

fn smoke_render(soft_distance: f32, erosion: f32) -> RenderSettings {
    RenderSettings {
        blend: BlendMode::Alpha,
        facing: Facing::CameraBillboard,
        shading: Shading::ProceduralSmoke,
        soft_distance,
        erosion,
        sort: true,
        ..Default::default()
    }
}

fn streak_render(blend: BlendMode, stretch: f32) -> RenderSettings {
    RenderSettings { blend, facing: Facing::VelocityAligned, stretch, ..Default::default() }
}

fn dot_render(blend: BlendMode) -> RenderSettings {
    RenderSettings { blend, ..Default::default() }
}

fn emitter(name: &str, capacity: u32, space: SimulationSpace, seed: u64, graph: VfxGraph, render: RenderSettings) -> Emitter {
    Emitter { name: name.to_string(), capacity, space, seed, graph, render, ..Default::default() }
}

fn effect(name: &str, duration: f32, looping: bool, emitters: Vec<Emitter>) -> VfxAsset {
    VfxAsset { name: name.to_string(), dimension: Dimension::ThreeD, duration, looping, emitters, ..Default::default() }
}

fn warm_light(intensity: f32, range: f32, flicker: f32, height: f32) -> ParticleLight {
    ParticleLight {
        mode: LightMode::PerEmitter,
        color: [1.0, 0.52, 0.2],
        intensity,
        range,
        radius: 0.15,
        smoothing: 0.15,
        flicker,
        flicker_rate: 5.0,
        offset: [0.0, height, 0.0],
        ..Default::default()
    }
}

// ── Shared layers ───────────────────────────────────────────────────────────

/// How a layer moves through the air.
#[derive(Clone, Copy)]
enum Air {
    /// Drag through the world wind (outdoors): coupling and height falloff.
    Wind(f32, f32),
    /// Drag through still air (indoors), so a scene's wind does not reach it.
    Still(f32),
}

fn air(b: &mut B, a: Air) -> Src {
    match a {
        Air::Wind(c, h) => wind(b, c, h),
        Air::Still(l) => drag(b, l, 0.0),
    }
}

/// White-hot core: the heat (alpha) peaks above 1 early and cools to the tip.
const HOT: &[(f32, [f32; 4])] = &[
    (0.0, [1.0, 1.0, 1.0, 0.55]),
    (0.08, [1.0, 1.0, 1.0, 1.05]),
    (0.45, [1.0, 0.97, 0.93, 0.95]),
    (0.8, [1.0, 0.9, 0.8, 0.6]),
    (1.0, [1.0, 0.85, 0.7, 0.0]),
];

/// The orange body of a fire: cooler throughout.
const WARM: &[(f32, [f32; 4])] = &[
    (0.0, [1.0, 1.0, 1.0, 0.5]),
    (0.1, [1.0, 1.0, 1.0, 0.95]),
    (0.5, [1.0, 0.95, 0.9, 0.8]),
    (0.85, [1.0, 0.85, 0.75, 0.45]),
    (1.0, [1.0, 0.8, 0.7, 0.0]),
];

/// One layer of procedural flame tongues. See `realistic_fire.vfx` for the
/// reasoning behind the shape: the rise settles at buoyancy over the air's
/// drag, curl noise stays weak so an updraft never outpulls the buoyancy, and
/// the tongue is velocity-stretched so it lengthens as it speeds up.
struct Flame {
    name: &'static str,
    seed: u64,
    capacity: u32,
    space: SimulationSpace,
    rate: f32,
    bed: [f32; 3],
    lift: f32,
    vel: ([f32; 3], [f32; 3]),
    life: (f32, f32),
    size: (f32, f32),
    buoyancy: f32,
    tint: [f32; 4],
    stops: &'static [(f32, [f32; 4])],
    curl: (f32, f32, f32),
    air: Air,
    stretch: f32,
}

fn flame(p: Flame) -> Emitter {
    let mut b = B::default();
    b.output("spawn_rate", f(p.rate));
    b.output("gravity", v3(0.0, p.buoyancy, 0.0));
    b.output("color", col(p.tint));
    let (pos, _) = bed(&mut b, p.bed);
    let pos = lift(&mut b, pos, p.lift);
    b.output("position", pos);
    let vel = rand3(&mut b, p.vel.0, p.vel.1);
    b.output("velocity", vel);
    let life = rand(&mut b, p.life.0, p.life.1);
    b.output("lifetime", life);
    let size = rand(&mut b, p.size.0, p.size.1);
    b.output("size", size);
    let grow = curve(&mut b, &[ease_out(0.0, 0.55), ease(0.18, 1.0), ease(0.7, 0.9), ease(1.0, 0.55)]);
    b.output("size_over_life", grow);
    let fade = gradient(&mut b, p.stops);
    b.output("color_over_life", fade);
    let swirl = curl(&mut b, p.curl.0, p.curl.1, [0.0, p.curl.2, 0.0]);
    let a = air(&mut b, p.air);
    accelerate(&mut b, vec![swirl, a]);
    emitter(p.name, p.capacity, p.space, p.seed, b.finish(), flame_render(p.stretch))
}

/// A layer of procedural smoke puffs that swell, spin slowly and fade.
struct Smoke {
    name: &'static str,
    seed: u64,
    capacity: u32,
    space: SimulationSpace,
    rate: f32,
    burst: f32,
    bed: [f32; 3],
    lift: f32,
    vel: ([f32; 3], [f32; 3]),
    life: (f32, f32),
    size: (f32, f32),
    grow: f32,
    tint: [f32; 4],
    /// Colour over life multiplier; its alpha shapes the fade.
    stops: &'static [(f32, [f32; 4])],
    buoyancy: f32,
    curl: (f32, f32, f32),
    forces: &'static [Air],
    ground: Option<(f32, f32)>,
    soft: f32,
    erosion: f32,
    shading: Shading,
}

/// A puff that fades in, holds and thins out.
const PUFF: &[(f32, [f32; 4])] = &[
    (0.0, [1.0, 1.0, 1.0, 0.0]),
    (0.12, [1.0, 1.0, 1.0, 1.0]),
    (0.5, [1.0, 1.0, 1.0, 0.7]),
    (1.0, [1.0, 1.0, 1.0, 0.0]),
];

/// Smoke over a fire: lit warm from below at first, cooling to grey.
const FIRE_SMOKE: &[(f32, [f32; 4])] = &[
    (0.0, [1.25, 1.05, 0.8, 0.0]),
    (0.12, [1.15, 1.0, 0.82, 1.0]),
    (0.45, [0.95, 0.92, 0.9, 0.75]),
    (1.0, [0.85, 0.85, 0.88, 0.0]),
];

fn smoke(p: Smoke) -> Emitter {
    let mut b = B::default();
    b.output("spawn_rate", f(p.rate));
    if p.burst > 0.0 {
        b.output("burst_count", f(p.burst));
    }
    b.output("gravity", v3(0.0, p.buoyancy, 0.0));
    b.output("color", col(p.tint));
    let (pos, _) = bed(&mut b, p.bed);
    let pos = lift(&mut b, pos, p.lift);
    b.output("position", pos);
    let vel = rand3(&mut b, p.vel.0, p.vel.1);
    b.output("velocity", vel);
    let life = rand(&mut b, p.life.0, p.life.1);
    b.output("lifetime", life);
    let size = rand(&mut b, p.size.0, p.size.1);
    b.output("size", size);
    let grow = curve(&mut b, &[ease_out(0.0, 0.5), ease(0.3, 1.0), ease(1.0, p.grow)]);
    b.output("size_over_life", grow);
    let fade = gradient(&mut b, p.stops);
    b.output("color_over_life", fade);
    random_spin(&mut b, 0.3);
    let mut forces = Vec::new();
    if p.curl.0 > 0.0 {
        forces.push(curl(&mut b, p.curl.0, p.curl.1, [0.0, p.curl.2, 0.0]));
    }
    for &a in p.forces {
        forces.push(air(&mut b, a));
    }
    accelerate(&mut b, forces);
    if let Some((bounce, friction)) = p.ground {
        ground(&mut b, bounce, friction, false);
    }
    let mut render = smoke_render(p.soft, p.erosion);
    render.shading = p.shading;
    emitter(p.name, p.capacity, p.space, p.seed, b.finish(), render)
}

/// White-hot embers or sparks thrown up from a fire, each with its own
/// weight (some ride the plume, some arc back down), flickering and drifting
/// on the wind, optionally skipping off the ground.
struct Embers {
    name: &'static str,
    seed: u64,
    capacity: u32,
    rate: f32,
    bed: [f32; 3],
    lift: f32,
    vel: ([f32; 3], [f32; 3]),
    life: (f32, f32),
    size: [f32; 2],
    color: [f32; 4],
    weight: (f32, f32),
    coupling: f32,
    curl: f32,
    ground: Option<(f32, f32)>,
    stretch: f32,
}

fn embers(p: Embers) -> Emitter {
    let mut b = B::default();
    b.output("spawn_rate", f(p.rate));
    b.output("size", v2(p.size[0], p.size[1]));
    b.output("color", col(p.color));
    let (pos, _) = bed(&mut b, p.bed);
    let pos = lift(&mut b, pos, p.lift);
    b.output("position", pos);
    let vel = rand3(&mut b, p.vel.0, p.vel.1);
    b.output("velocity", vel);
    let life = rand(&mut b, p.life.0, p.life.1);
    b.output("lifetime", life);
    let weight = rand(&mut b, p.weight.0, p.weight.1);
    let g = b.op("math/compose_vec3", &[("y", weight)], "value");
    b.output("gravity", g);
    let cool = gradient(
        &mut b,
        &[
            (0.0, [1.0, 1.0, 1.0, 1.0]),
            (0.55, [0.85, 0.6, 0.45, 1.0]),
            (0.85, [0.6, 0.3, 0.15, 0.8]),
            (1.0, [0.4, 0.12, 0.04, 0.0]),
        ],
    );
    let flick = flicker(&mut b, (14.0, 24.0), 0.35, 1.0);
    let c = b.mul(cool, flick);
    b.output("color_over_life", c);
    let swirl = curl(&mut b, p.curl, 1.4, [0.0, -0.6, 0.0]);
    let w = wind(&mut b, p.coupling, 0.0);
    accelerate(&mut b, vec![swirl, w]);
    if let Some((bounce, friction)) = p.ground {
        ground(&mut b, bounce, friction, false);
    }
    emitter(p.name, p.capacity, World, p.seed, b.finish(), streak_render(BlendMode::Additive, p.stretch))
}

/// The glowing bed under a fire: wide, low soft ovals that breathe slowly.
#[allow(clippy::too_many_arguments)]
fn coals(name: &str, seed: u64, capacity: u32, rate: f32, area: [f32; 3], oval: [f32; 2], color: [f32; 4], drift: Air) -> Emitter {
    let mut b = B::default();
    b.output("spawn_rate", f(rate));
    b.output("velocity", v3(0.0, 0.02, 0.0));
    b.output("color", col(color));
    let (pos, _) = bed(&mut b, area);
    let pos = lift(&mut b, pos, 0.035);
    b.output("position", pos);
    let life = rand(&mut b, 0.6, 1.1);
    b.output("lifetime", life);
    let scale = rand(&mut b, 0.7, 1.3);
    let size = b.mul(v2(oval[0], oval[1]), scale);
    b.output("size", size);
    let fade = gradient(
        &mut b,
        &[
            (0.0, [1.0, 1.0, 1.0, 0.0]),
            (0.3, [1.0, 1.0, 1.0, 1.0]),
            (0.7, [0.9, 0.8, 0.7, 0.8]),
            (1.0, [0.8, 0.6, 0.5, 0.0]),
        ],
    );
    let flick = flicker(&mut b, (6.0, 11.0), 0.6, 1.0);
    let c = b.mul(fade, flick);
    b.output("color_over_life", c);
    let w = air(&mut b, drift);
    b.output("acceleration", w);
    let render = RenderSettings { blend: BlendMode::Additive, soft_distance: 0.05, ..Default::default() };
    emitter(name, capacity, Local, seed, b.finish(), render)
}

// ── fire/ ───────────────────────────────────────────────────────────────────

/// A ring-of-logs campfire about 0.6 m across, origin on the ground at its
/// centre. Four layers: a white-hot core and a ring of cooler orange tongues
/// (ProceduralFlame), a thin warm-to-grey smoke haze born above the tips, and
/// embers thrown up that either ride the plume or arc down and skip off a
/// ground plane. The outer flames carry one warm flickering point light at
/// flame height: about 33 tongues alive at a light weight near 0.8 each, so
/// 3600 lm a tongue is roughly 85000 lm in full flow, dimming with the fire.
/// Flames are Local (they stay on the logs); smoke and embers are World so
/// they trail off a moving fire.
fn campfire() -> VfxAsset {
    let core = flame(Flame {
        name: "flame core",
        seed: 101,
        capacity: 48,
        space: Local,
        rate: 20.0,
        bed: [0.28, 0.04, 0.28],
        lift: 0.1,
        vel: ([-0.06, 0.3, -0.06], [0.06, 0.55, 0.06]),
        life: (0.5, 0.8),
        size: (0.15, 0.22),
        buoyancy: 0.7,
        tint: [2.1, 1.9, 1.7, 1.0],
        stops: HOT,
        curl: (0.6, 2.2, -1.6),
        air: Air::Wind(0.5, 0.5),
        stretch: 0.14,
    });
    let mut outer = flame(Flame {
        name: "outer flames",
        seed: 202,
        capacity: 80,
        space: Local,
        rate: 45.0,
        bed: [0.55, 0.04, 0.55],
        lift: 0.08,
        vel: ([-0.1, 0.25, -0.1], [0.1, 0.45, 0.1]),
        life: (0.55, 0.9),
        size: (0.1, 0.17),
        buoyancy: 0.75,
        tint: [1.45, 1.3, 1.15, 0.8],
        stops: WARM,
        curl: (0.8, 2.8, -1.9),
        air: Air::Wind(0.6, 0.45),
        stretch: 0.16,
    });
    outer.light = warm_light(3600.0, 8.0, 0.35, 0.3);
    let haze = smoke(Smoke {
        name: "smoke",
        seed: 303,
        capacity: 48,
        space: World,
        rate: 6.0,
        burst: 0.0,
        bed: [0.35, 0.06, 0.35],
        lift: 0.55,
        vel: ([-0.06, 0.25, -0.06], [0.06, 0.5, 0.06]),
        life: (3.0, 4.5),
        size: (0.2, 0.3),
        grow: 2.6,
        tint: [0.44, 0.4, 0.36, 0.28],
        stops: FIRE_SMOKE,
        buoyancy: 0.32,
        curl: (0.7, 1.1, -0.35),
        forces: &[Air::Wind(1.4, 0.0)],
        ground: None,
        soft: 0.3,
        erosion: 0.0,
        shading: Shading::ProceduralSmoke,
    });
    let sparks = embers(Embers {
        name: "embers",
        seed: 404,
        capacity: 40,
        rate: 8.0,
        bed: [0.45, 0.05, 0.45],
        lift: 0.15,
        vel: ([-0.35, 1.0, -0.35], [0.35, 2.2, 0.35]),
        life: (1.4, 2.6),
        size: [0.012, 0.02],
        color: [6.0, 2.1, 0.45, 1.0],
        weight: (-1.4, 0.2),
        coupling: 0.8,
        curl: 2.4,
        ground: Some((0.35, 0.6)),
        stretch: 0.035,
    });
    effect("Campfire", 0.0, true, vec![core, outer, haze, sparks])
}

/// A hand torch. The origin is the top of the shaft, where the pitch-soaked
/// head burns: the flame starts there and licks about 0.25 m up. Tongues are
/// World space and short lived (under 0.55 s), so a swung torch leaves a
/// brief lick of flame behind it the way a real one does. A dark wisp of
/// smoke and a few sparks come off the top. One warm light 0.12 m up, range
/// 4 m: about 15 tongues alive at weight near 1, so 2000 lm a tongue is about
/// 30000 lm, flickering 30 % at 7 Hz.
fn torch() -> VfxAsset {
    let mut tongues = flame(Flame {
        name: "flame",
        seed: 111,
        capacity: 32,
        space: World,
        rate: 28.0,
        bed: [0.07, 0.03, 0.07],
        lift: 0.02,
        vel: ([-0.03, 0.25, -0.03], [0.03, 0.45, 0.03]),
        life: (0.35, 0.55),
        size: (0.07, 0.1),
        buoyancy: 0.8,
        tint: [1.8, 1.6, 1.4, 0.95],
        stops: HOT,
        curl: (0.9, 4.0, -2.2),
        air: Air::Wind(0.6, 0.1),
        stretch: 0.12,
    });
    tongues.light = ParticleLight { flicker_rate: 7.0, ..warm_light(2000.0, 4.0, 0.3, 0.12) };
    let wisp = smoke(Smoke {
        name: "smoke",
        seed: 222,
        capacity: 16,
        space: World,
        rate: 4.0,
        burst: 0.0,
        bed: [0.05, 0.02, 0.05],
        lift: 0.25,
        vel: ([-0.03, 0.3, -0.03], [0.03, 0.5, 0.03]),
        life: (1.5, 2.5),
        size: (0.06, 0.1),
        grow: 3.0,
        tint: [0.3, 0.28, 0.26, 0.25],
        stops: FIRE_SMOKE,
        buoyancy: 0.4,
        curl: (0.5, 2.0, -0.5),
        forces: &[Air::Wind(1.5, 0.0)],
        ground: None,
        soft: 0.1,
        erosion: 0.0,
        shading: Shading::ProceduralSmoke,
    });
    let sparks = embers(Embers {
        name: "sparks",
        seed: 333,
        capacity: 12,
        rate: 3.0,
        bed: [0.06, 0.03, 0.06],
        lift: 0.08,
        vel: ([-0.2, 0.6, -0.2], [0.2, 1.4, 0.2]),
        life: (0.8, 1.4),
        size: [0.008, 0.014],
        color: [6.0, 2.2, 0.5, 1.0],
        weight: (-0.6, 0.2),
        coupling: 0.8,
        curl: 2.0,
        ground: None,
        stretch: 0.03,
    });
    effect("Torch", 0.0, true, vec![tongues, wisp, sparks])
}

/// A candle flame. The origin is the wick tip. A dozen overlapping tongues
/// born on the wick in still air (drag 2/s against a buoyancy of 0.25 m/s^2,
/// so a rise that settles at 0.12 m/s) build one steady teardrop about 4 cm
/// tall: indoors, so it drags through still air rather than the world wind.
/// Curl noise is almost nil so the flame barely wavers. The light is small
/// and warm (about 1200 lm over 1.8 m) with a gentle 8 % flicker at 2.5 Hz,
/// smoothed over 0.2 s so it breathes rather than flickers.
fn candle() -> VfxAsset {
    let mut f1 = flame(Flame {
        name: "flame",
        seed: 121,
        capacity: 16,
        space: Local,
        rate: 14.0,
        bed: [0.004, 0.0, 0.004],
        lift: 0.004,
        vel: ([-0.002, 0.03, -0.002], [0.002, 0.05, 0.002]),
        life: (0.5, 0.6),
        size: (0.013, 0.016),
        buoyancy: 0.25,
        tint: [1.6, 1.45, 1.25, 0.85],
        stops: WARM,
        curl: (0.03, 6.0, -1.0),
        air: Air::Still(2.0),
        stretch: 0.2,
    });
    f1.render.soft_distance = 0.0;
    f1.light = ParticleLight {
        radius: 0.02,
        smoothing: 0.2,
        flicker_rate: 2.5,
        color: [1.0, 0.6, 0.28],
        ..warm_light(180.0, 1.8, 0.08, 0.02)
    };
    effect("Candle", 0.0, true, vec![f1])
}

/// A hearth fire about 1 m wide and 0.3 m deep, origin on the hearth floor.
/// Wide and low: many small orange tongues (70/s) across the whole grate,
/// fewer hot ones along its middle, and a breathing bed of coals. Indoors,
/// so every layer drags through still air (a scene's outdoor wind does not
/// blow into a fireplace). Lower buoyancy than a campfire keeps the flames
/// to about 0.4 m. One warm light at 0.25 m, about 60000 lm over 7 m,
/// flickering 25 %.
fn fireplace() -> VfxAsset {
    let core = flame(Flame {
        name: "hot tongues",
        seed: 131,
        capacity: 32,
        space: Local,
        rate: 25.0,
        bed: [0.6, 0.03, 0.12],
        lift: 0.08,
        vel: ([-0.05, 0.25, -0.03], [0.05, 0.4, 0.03]),
        life: (0.45, 0.7),
        size: (0.12, 0.17),
        buoyancy: 0.55,
        tint: [2.0, 1.85, 1.6, 1.0],
        stops: HOT,
        curl: (0.6, 2.4, -1.6),
        air: Air::Still(0.5),
        stretch: 0.14,
    });
    let mut tongues = flame(Flame {
        name: "tongues",
        seed: 132,
        capacity: 64,
        space: Local,
        rate: 70.0,
        bed: [0.9, 0.03, 0.25],
        lift: 0.06,
        vel: ([-0.08, 0.2, -0.05], [0.08, 0.4, 0.05]),
        life: (0.45, 0.75),
        size: (0.08, 0.14),
        buoyancy: 0.55,
        tint: [1.5, 1.3, 1.1, 0.85],
        stops: WARM,
        curl: (0.8, 3.0, -1.9),
        air: Air::Still(0.6),
        stretch: 0.16,
    });
    tongues.light = ParticleLight { flicker_rate: 4.0, ..warm_light(1900.0, 7.0, 0.25, 0.25) };
    let bed = coals("coals", 133, 48, 30.0, [1.0, 0.0, 0.3], [0.24, 0.09], [1.7, 0.38, 0.06, 0.35], Air::Still(0.5));
    effect("Fireplace", 0.0, true, vec![core, tongues, bed])
}

/// A fire in an iron bowl about 0.5 m across. The origin is the bowl's rim:
/// flames rise from a 0.22 m disc there, sparks pour up out of it on the
/// heat (fast, light, most of them riding the plume a metre or two up) and a
/// little smoke follows. Every layer is wind-coupled by weight: smoke 1.5,
/// sparks 0.9, flames 0.6 (sheltered by the bowl below 0.3 m). One warm light
/// at 0.3 m, about 50000 lm over 7 m.
fn brazier() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(40.0));
    b.output("gravity", v3(0.0, 0.75, 0.0));
    b.output("color", col([1.7, 1.5, 1.3, 0.9]));
    let (pos, dir) = shape(&mut b, "shape/cone", &[("angle", PinValue::Float(12.0)), ("radius", PinValue::Float(0.22))]);
    b.output("position", pos);
    let speed = rand(&mut b, 0.3, 0.6);
    let vel = b.mul(dir, speed);
    b.output("velocity", vel);
    let life = rand(&mut b, 0.5, 0.85);
    b.output("lifetime", life);
    let size = rand(&mut b, 0.11, 0.18);
    b.output("size", size);
    let grow = curve(&mut b, &[ease_out(0.0, 0.55), ease(0.18, 1.0), ease(0.7, 0.9), ease(1.0, 0.5)]);
    b.output("size_over_life", grow);
    let fade = gradient(&mut b, WARM);
    b.output("color_over_life", fade);
    let swirl = curl(&mut b, 0.8, 2.6, [0.0, -1.8, 0.0]);
    let w = wind(&mut b, 0.6, 0.3);
    accelerate(&mut b, vec![swirl, w]);
    let mut flames = emitter("flames", 64, Local, 141, b.finish(), flame_render(0.15));
    flames.light = warm_light(2100.0, 7.0, 0.3, 0.3);

    let sparks = embers(Embers {
        name: "sparks",
        seed: 142,
        capacity: 40,
        rate: 14.0,
        bed: [0.3, 0.05, 0.3],
        lift: 0.1,
        vel: ([-0.4, 1.5, -0.4], [0.4, 3.0, 0.4]),
        life: (1.0, 2.0),
        size: [0.01, 0.018],
        color: [6.0, 2.4, 0.6, 1.0],
        weight: (-0.3, 0.4),
        coupling: 0.9,
        curl: 3.0,
        ground: None,
        stretch: 0.03,
    });
    let haze = smoke(Smoke {
        name: "smoke",
        seed: 143,
        capacity: 32,
        space: World,
        rate: 5.0,
        burst: 0.0,
        bed: [0.25, 0.05, 0.25],
        lift: 0.6,
        vel: ([-0.05, 0.3, -0.05], [0.05, 0.5, 0.05]),
        life: (2.5, 4.0),
        size: (0.18, 0.26),
        grow: 2.6,
        tint: [0.4, 0.37, 0.34, 0.25],
        stops: FIRE_SMOKE,
        buoyancy: 0.35,
        curl: (0.7, 1.1, -0.35),
        forces: &[Air::Wind(1.5, 0.0)],
        ground: None,
        soft: 0.25,
        erosion: 0.0,
        shading: Shading::ProceduralSmoke,
    });
    effect("Brazier", 0.0, true, vec![flames, sparks, haze])
}

// ── smoke/ ──────────────────────────────────────────────────────────────────

/// A slow chimney plume. The origin is the chimney pot. Puffs leave at about
/// 0.75 m/s, then rise at buoyancy over coupling (0.6 / 1.8, 0.33 m/s), so a
/// plume climbs about 3 m in its 6 to 9 s while swelling 4x. The coupling of
/// 1.8 is strong: in any breeze the plume lies over and streams downwind
/// within a second, which is what chimney smoke does. World space, sorted,
/// lightly eroded so its edges break into wisps.
fn chimney_smoke() -> VfxAsset {
    let plume = smoke(Smoke {
        name: "plume",
        seed: 201,
        capacity: 64,
        space: World,
        rate: 5.0,
        burst: 0.0,
        bed: [0.3, 0.05, 0.3],
        lift: 0.05,
        vel: ([-0.05, 0.6, -0.05], [0.05, 0.9, 0.05]),
        life: (6.0, 9.0),
        size: (0.35, 0.5),
        grow: 4.0,
        tint: [0.55, 0.53, 0.52, 0.45],
        stops: PUFF,
        buoyancy: 0.6,
        curl: (0.5, 0.6, -0.3),
        forces: &[Air::Wind(1.8, 0.0)],
        ground: None,
        soft: 0.5,
        erosion: 0.25,
        shading: Shading::ProceduralSmoke,
    });
    effect("Chimney Smoke", 0.0, true, vec![plume])
}

/// Low ground fog over a 12 x 12 m area, origin on the ground at its centre.
/// Big (2.5 to 4 m), very faint (opacity 0.12) soft sprites that live 10 to
/// 14 s and drift at a few cm/s, lightly coupled to the wind (0.15) so a
/// breeze slowly herds the bank along. A burst of 60 at the start fills the
/// area at once instead of fading in over ten seconds. Soft particles (1 m)
/// are what stop each sheet showing a hard line where it meets the ground.
fn ground_fog() -> VfxAsset {
    let bank = smoke(Smoke {
        name: "fog",
        seed: 211,
        capacity: 256,
        space: World,
        rate: 10.0,
        burst: 60.0,
        bed: [12.0, 0.3, 12.0],
        lift: 0.35,
        vel: ([-0.08, -0.01, -0.08], [0.08, 0.01, 0.08]),
        life: (10.0, 14.0),
        size: (2.5, 4.0),
        grow: 1.4,
        tint: [0.78, 0.8, 0.84, 0.12],
        stops: &[(0.0, [1.0, 1.0, 1.0, 0.0]), (0.25, [1.0, 1.0, 1.0, 1.0]), (0.75, [1.0, 1.0, 1.0, 1.0]), (1.0, [1.0, 1.0, 1.0, 0.0])],
        buoyancy: 0.0,
        curl: (0.03, 0.15, 0.0),
        forces: &[Air::Wind(0.15, 0.0)],
        ground: None,
        soft: 1.0,
        erosion: 0.0,
        shading: Shading::Sprite,
    });
    effect("Ground Fog", 0.0, true, vec![bank])
}

/// A steam vent: a fast white jet (5 to 7 m/s in a 6 degree cone) that
/// quadratic drag brakes within half a metre or so, swelling tenfold and
/// thinning to nothing as it slows, then drifting off on the wind. Opacity
/// is front-loaded so the jet is dense at the nozzle and gone by its end.
fn steam_vent() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(60.0));
    b.output("gravity", v3(0.0, 0.4, 0.0));
    b.output("color", col([0.95, 0.96, 1.0, 0.5]));
    let (pos, dir) = shape(&mut b, "shape/cone", &[("angle", PinValue::Float(6.0)), ("radius", PinValue::Float(0.03))]);
    b.output("position", pos);
    let speed = rand(&mut b, 5.0, 7.0);
    let vel = b.mul(dir, speed);
    b.output("velocity", vel);
    let life = rand(&mut b, 1.2, 1.8);
    b.output("lifetime", life);
    let size = rand(&mut b, 0.05, 0.08);
    b.output("size", size);
    let grow = curve(&mut b, &[ease_out(0.0, 1.0), ease(0.4, 6.0), ease(1.0, 10.0)]);
    b.output("size_over_life", grow);
    let fade = gradient(
        &mut b,
        &[(0.0, [1.0, 1.0, 1.0, 0.0]), (0.05, [1.0, 1.0, 1.0, 1.0]), (0.35, [1.0, 1.0, 1.0, 0.45]), (1.0, [1.0, 1.0, 1.0, 0.0])],
    );
    b.output("color_over_life", fade);
    random_spin(&mut b, 0.6);
    let brake = drag(&mut b, 1.2, 0.25);
    let w = wind(&mut b, 0.4, 0.0);
    let swirl = curl(&mut b, 0.4, 1.5, [0.0, -0.8, 0.0]);
    accelerate(&mut b, vec![brake, w, swirl]);
    let jet = emitter("jet", 160, World, 221, b.finish(), smoke_render(0.2, 0.2));
    effect("Steam Vent", 0.0, true, vec![jet])
}

/// A one-shot cloud of dust kicked up by something heavy landing, origin on
/// the ground. 40 puffs thrown out and up at a few m/s, braked hard by drag,
/// then settling slowly (a weak -0.4 m/s^2) onto a ground plane that stops
/// them dead, while they swell 2.5x and thin out over 2.5 to 4 s. A second
/// layer of grit falls back under full gravity and skids to a stop. Sandy
/// colour; sorted and soft.
fn dust_cloud() -> VfxAsset {
    let cloud = smoke(Smoke {
        name: "dust",
        seed: 231,
        capacity: 64,
        space: World,
        rate: 0.0,
        burst: 40.0,
        bed: [1.0, 0.2, 1.0],
        lift: 0.15,
        vel: ([-2.5, 0.3, -2.5], [2.5, 1.8, 2.5]),
        life: (2.5, 4.0),
        size: (0.4, 0.7),
        grow: 2.5,
        tint: [0.62, 0.54, 0.43, 0.55],
        stops: PUFF,
        buoyancy: -0.4,
        curl: (0.3, 0.8, 0.0),
        forces: &[Air::Still(1.5), Air::Wind(0.6, 0.0)],
        ground: Some((0.0, 0.8)),
        soft: 0.3,
        erosion: 0.15,
        shading: Shading::ProceduralSmoke,
    });
    let mut b = B::default();
    b.output("spawn_rate", f(0.0));
    b.output("burst_count", f(30.0));
    b.output("gravity", v3(0.0, -9.81, 0.0));
    b.output("size", f(0.015));
    b.output("color", col([0.42, 0.36, 0.28, 1.0]));
    let (pos, _) = bed(&mut b, [0.6, 0.1, 0.6]);
    let pos = lift(&mut b, pos, 0.05);
    b.output("position", pos);
    let vel = rand3(&mut b, [-2.0, 1.0, -2.0], [2.0, 3.0, 2.0]);
    b.output("velocity", vel);
    let life = rand(&mut b, 1.0, 1.6);
    b.output("lifetime", life);
    let fade = gradient(&mut b, &[(0.0, [1.0, 1.0, 1.0, 1.0]), (0.8, [1.0, 1.0, 1.0, 1.0]), (1.0, [1.0, 1.0, 1.0, 0.0])]);
    b.output("color_over_life", fade);
    ground(&mut b, 0.2, 0.8, false);
    let grit = emitter("grit", 48, World, 232, b.finish(), dot_render(BlendMode::Alpha));
    effect("Dust Cloud", 1.0, false, vec![cloud, grit])
}

// ── water/ ──────────────────────────────────────────────────────────────────

/// A garden fountain. The origin is the basin floor; the nozzle is 0.6 m up.
/// Drops leave at 4.2 to 4.8 m/s within 20 degrees of vertical, so they peak
/// about 1 m over the nozzle and land some 1.5 m out a second later, where a
/// ground plane kills them. Each death fires the splash emitter (OnDeath,
/// two droplets each), which hop 5 to 10 cm and die on the same plane.
/// Velocity-aligned so a drop reads as a short streak along its arc.
fn fountain() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(220.0));
    b.output("gravity", v3(0.0, -9.81, 0.0));
    b.output("size", v2(0.02, 0.05));
    b.output("color", col([0.72, 0.84, 1.0, 0.6]));
    b.output("lifetime", f(2.0));
    let (pos, dir) = shape(&mut b, "shape/cone", &[("angle", PinValue::Float(20.0)), ("radius", PinValue::Float(0.02))]);
    let pos = lift(&mut b, pos, 0.6);
    b.output("position", pos);
    let speed = rand(&mut b, 4.2, 4.8);
    let vel = b.mul(dir, speed);
    b.output("velocity", vel);
    ground(&mut b, 0.0, 0.0, true);
    let mut jet = emitter("jet", 512, World, 301, b.finish(), streak_render(BlendMode::Alpha, 0.03));
    jet.sub_emitters = vec![SubEmitter {
        trigger: SubEmitterTrigger::OnDeath,
        target: 1,
        count: 2,
        inherit_velocity: 0.1,
        inherit_color: false,
    }];
    effect("Fountain", 0.0, true, vec![jet, splash_droplets("splash", 302, 256, (0.6, 1.4), 0.6)])
}

/// The droplets a falling drop throws up where it lands: a sub-emitter
/// target, so its own spawn pins are unused and its position is an offset
/// from the drop that died.
fn splash_droplets(name: &str, seed: u64, capacity: u32, up: (f32, f32), spread: f32) -> Emitter {
    let mut b = B::default();
    b.output("spawn_rate", f(0.0));
    b.output("gravity", v3(0.0, -9.81, 0.0));
    b.output("size", f(0.012));
    b.output("color", col([0.82, 0.9, 1.0, 0.5]));
    b.output("position", v3(0.0, 0.01, 0.0));
    let vel = rand3(&mut b, [-spread, up.0, -spread], [spread, up.1, spread]);
    b.output("velocity", vel);
    let life = rand(&mut b, 0.25, 0.4);
    b.output("lifetime", life);
    let fade = gradient(&mut b, &[(0.0, [1.0, 1.0, 1.0, 1.0]), (1.0, [1.0, 1.0, 1.0, 0.0])]);
    b.output("color_over_life", fade);
    ground(&mut b, 0.0, 0.0, true);
    emitter(name, capacity, World, seed, b.finish(), dot_render(BlendMode::Alpha))
}

/// The spray cloud at the foot of a waterfall, spread over 4 m along the
/// pool's edge. Big soft puffs (0.8 to 1.4 m, opacity 0.18) billow up and
/// out of the pool (+Z, away from the falls) and ride the wind fully
/// (coupling 1), swelling 2.5x over 3 to 5 s. Soft particles and sorting
/// keep the overlapping sheets from showing edges.
fn waterfall_mist() -> VfxAsset {
    let mist = smoke(Smoke {
        name: "mist",
        seed: 311,
        capacity: 160,
        space: World,
        rate: 25.0,
        burst: 0.0,
        bed: [4.0, 0.3, 1.5],
        lift: 0.3,
        vel: ([-0.3, 0.4, 0.2], [0.3, 1.0, 0.8]),
        life: (3.0, 5.0),
        size: (0.8, 1.4),
        grow: 2.5,
        tint: [0.88, 0.92, 0.96, 0.18],
        stops: PUFF,
        buoyancy: 0.05,
        curl: (0.6, 0.7, -0.3),
        forces: &[Air::Wind(1.0, 0.0)],
        ground: None,
        soft: 0.6,
        erosion: 0.2,
        shading: Shading::ProceduralSmoke,
    });
    effect("Waterfall Mist", 0.0, true, vec![mist])
}

/// A one-shot splash, origin on the water's surface. 60 droplets thrown out
/// in a crown (a ring 8 cm across, 1.2 to 2 m/s outward and 1.8 to 2.8 m/s
/// up, so they reach 15 to 40 cm and land half a metre out) plus a central
/// column of 12 thrown straight up. Both die on the surface plane.
fn splash() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(0.0));
    b.output("burst_count", f(60.0));
    b.output("gravity", v3(0.0, -9.81, 0.0));
    b.output("size", v2(0.018, 0.035));
    b.output("color", col([0.78, 0.88, 1.0, 0.65]));
    b.output("lifetime", f(1.5));
    let angle = rand(&mut b, 0.0, TAU);
    let s = b.op("math/sin", &[("value", angle.clone())], "value");
    let c = b.op("math/cos", &[("value", angle)], "value");
    let rx = b.mul(c.clone(), f(0.04));
    let rz = b.mul(s.clone(), f(0.04));
    let pos = b.op("math/compose_vec3", &[("x", rx), ("y", f(0.01)), ("z", rz)], "value");
    b.output("position", pos);
    let out = rand(&mut b, 1.2, 2.0);
    let vx = b.mul(c, out.clone());
    let vz = b.mul(s, out);
    let vy = rand(&mut b, 1.8, 2.8);
    let vel = b.op("math/compose_vec3", &[("x", vx), ("y", vy), ("z", vz)], "value");
    b.output("velocity", vel);
    ground(&mut b, 0.0, 0.0, true);
    let crown = emitter("crown", 96, World, 321, b.finish(), streak_render(BlendMode::Alpha, 0.025));

    let mut b = B::default();
    b.output("spawn_rate", f(0.0));
    b.output("burst_count", f(12.0));
    b.output("gravity", v3(0.0, -9.81, 0.0));
    b.output("size", v2(0.022, 0.045));
    b.output("color", col([0.8, 0.9, 1.0, 0.7]));
    b.output("lifetime", f(1.5));
    b.output("position", v3(0.0, 0.01, 0.0));
    let vel = rand3(&mut b, [-0.15, 2.6, -0.15], [0.15, 3.6, 0.15]);
    b.output("velocity", vel);
    ground(&mut b, 0.0, 0.0, true);
    let column = emitter("column", 24, World, 322, b.finish(), streak_render(BlendMode::Alpha, 0.025));
    effect("Splash", 1.0, false, vec![crown, column])
}

// ── weather/ ────────────────────────────────────────────────────────────────

/// Rain over a 16 x 16 m area, origin on the ground (parent it under the
/// camera rig to keep it overhead). Drops are born 9 m up and fall under
/// gravity against a wind coupling of 1.1, which makes their terminal speed
/// g / 1.1 = 8.9 m/s (a real raindrop's) and means they drift with the wind
/// at its full speed, as rain does. Velocity-aligned thin streaks. A ground
/// plane kills each drop and its death throws two tiny droplets (the
/// splash emitter). World space, so a moving rig does not drag the rain.
fn rain() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(900.0));
    b.output("gravity", v3(0.0, -9.81, 0.0));
    b.output("size", v2(0.008, 0.18));
    b.output("color", col([0.7, 0.78, 0.9, 0.35]));
    b.output("lifetime", f(1.6));
    let (pos, _) = bed(&mut b, [16.0, 0.0, 16.0]);
    let pos = lift(&mut b, pos, 9.0);
    b.output("position", pos);
    let vel = rand3(&mut b, [-0.1, -9.0, -0.1], [0.1, -8.6, 0.1]);
    b.output("velocity", vel);
    let w = wind(&mut b, 1.1, 0.0);
    b.output("acceleration", w);
    ground(&mut b, 0.0, 0.0, true);
    let mut drops = emitter("drops", 1280, World, 401, b.finish(), streak_render(BlendMode::Alpha, 0.015));
    drops.sub_emitters = vec![SubEmitter {
        trigger: SubEmitterTrigger::OnDeath,
        target: 1,
        count: 2,
        inherit_velocity: 0.0,
        inherit_color: false,
    }];
    let mut splashes = splash_droplets("splashes", 402, 640, (0.5, 1.1), 0.5);
    splashes.render = dot_render(BlendMode::Alpha);
    effect("Rain", 0.0, true, vec![drops, splashes])
}

/// Snow over a 16 x 16 m area, born 8 m up. A flake's terminal speed is
/// about 1 m/s, so gravity is -1 against a wind coupling of 1: it falls at
/// 1 m/s and drifts fully with any wind. Curl noise (0.4 at 0.8 /m) gives
/// the side-to-side flutter. Flakes die on the ground plane; lifetime 10 s
/// is a backstop. 150/s keeps about 1200 alive once the fall has filled.
fn snow() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(150.0));
    b.output("gravity", v3(0.0, -1.0, 0.0));
    b.output("color", col([1.0, 1.0, 1.0, 0.85]));
    b.output("lifetime", f(10.0));
    let (pos, _) = bed(&mut b, [16.0, 0.0, 16.0]);
    let pos = lift(&mut b, pos, 8.0);
    b.output("position", pos);
    let vel = rand3(&mut b, [-0.1, -1.1, -0.1], [0.1, -0.8, 0.1]);
    b.output("velocity", vel);
    let size = rand(&mut b, 0.02, 0.035);
    b.output("size", size);
    let fade = gradient(&mut b, &[(0.0, [1.0, 1.0, 1.0, 0.0]), (0.05, [1.0, 1.0, 1.0, 1.0]), (1.0, [1.0, 1.0, 1.0, 1.0])]);
    b.output("color_over_life", fade);
    let flutter = curl(&mut b, 0.4, 0.8, [0.0, 0.3, 0.0]);
    let w = wind(&mut b, 1.0, 0.0);
    accelerate(&mut b, vec![flutter, w]);
    ground(&mut b, 0.0, 0.0, true);
    let flakes = emitter("flakes", 1600, World, 411, b.finish(), dot_render(BlendMode::Alpha));
    effect("Snow", 0.0, true, vec![flakes])
}

/// Leaves falling from a canopy 5.5 to 6.5 m up over a 6 x 6 m area. A leaf
/// falls at about 1.8 m/s (gravity -1.4 against a wind coupling of 0.8)
/// and drifts on the wind; curl noise swings it from side to side, and a
/// random spin of up to 4 rad/s tumbles it. Lifetime is matched to the
/// fall (3.0 to 3.5 s, fading out over the last 15 %), so a leaf is gone as
/// it reaches the ground; a ground plane kills any that land early. Autumn
/// colours, picked per leaf.
fn falling_leaves() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(3.0));
    b.output("gravity", v3(0.0, -1.4, 0.0));
    let (pos, _) = bed(&mut b, [6.0, 1.0, 6.0]);
    let pos = lift(&mut b, pos, 6.0);
    b.output("position", pos);
    let vel = rand3(&mut b, [-0.2, -1.9, -0.2], [0.2, -1.6, 0.2]);
    b.output("velocity", vel);
    let life = rand(&mut b, 3.0, 3.5);
    b.output("lifetime", life);
    let scale = rand(&mut b, 0.8, 1.2);
    let size = b.mul(v2(0.07, 0.045), scale);
    b.output("size", size);
    let tint = rand_color(&mut b, [0.85, 0.38, 0.08, 1.0], [0.72, 0.62, 0.14, 1.0]);
    b.output("color", tint);
    let fade = gradient(&mut b, &[(0.0, [1.0, 1.0, 1.0, 0.0]), (0.05, [1.0, 1.0, 1.0, 1.0]), (0.85, [0.9, 0.85, 0.8, 1.0]), (1.0, [0.85, 0.8, 0.75, 0.0])]);
    b.output("color_over_life", fade);
    random_spin(&mut b, 4.0);
    let swing = curl(&mut b, 1.2, 0.7, [0.0, 0.5, 0.0]);
    let w = wind(&mut b, 0.8, 0.0);
    accelerate(&mut b, vec![swing, w]);
    ground(&mut b, 0.0, 1.0, true);
    let leaves = emitter("leaves", 32, World, 421, b.finish(), dot_render(BlendMode::Alpha));
    effect("Falling Leaves", 0.0, true, vec![leaves])
}

// ── ambient/ ────────────────────────────────────────────────────────────────

/// Dust motes in a room's air, a 4 x 2.5 x 4 m volume standing on the
/// origin. Tiny (6 to 12 mm), dim, warm specks that hang for 8 to 12 s,
/// wandering on a very weak curl field in still air (indoors, so no wind).
/// Additive so they catch the eye only where they overlap light, and
/// fading in and out so none pops. A burst of 40 fills the room at once.
fn dust_motes() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(6.0));
    b.output("burst_count", f(40.0));
    b.output("color", col([1.0, 0.95, 0.85, 0.35]));
    let (pos, _) = bed(&mut b, [4.0, 2.5, 4.0]);
    let pos = lift(&mut b, pos, 1.25);
    b.output("position", pos);
    let vel = rand3(&mut b, [-0.02, -0.01, -0.02], [0.02, 0.01, 0.02]);
    b.output("velocity", vel);
    let life = rand(&mut b, 8.0, 12.0);
    b.output("lifetime", life);
    let size = rand(&mut b, 0.006, 0.012);
    b.output("size", size);
    let fade = gradient(&mut b, &[(0.0, [1.0, 1.0, 1.0, 0.0]), (0.2, [1.0, 1.0, 1.0, 1.0]), (0.8, [1.0, 1.0, 1.0, 1.0]), (1.0, [1.0, 1.0, 1.0, 0.0])]);
    let glint = flicker(&mut b, (0.8, 2.0), 0.5, 1.0);
    let c = b.mul(fade, glint);
    b.output("color_over_life", c);
    let drift = curl(&mut b, 0.05, 0.5, [0.0, 0.05, 0.0]);
    let still = drag(&mut b, 0.6, 0.0);
    accelerate(&mut b, vec![drift, still]);
    let motes = emitter("motes", 128, Local, 501, b.finish(), dot_render(BlendMode::Additive));
    effect("Dust Motes", 0.0, true, vec![motes])
}

/// Fireflies over a 6 x 6 m meadow, 0.4 to 1.6 m up. Each wanders on a slow
/// curl field (braked by drag so it meanders rather than races) for 6 to
/// 10 s and blinks: a smoothstep of a sine sharpens each pulse into a short
/// yellow-green flash with long dark gaps. The three brightest carry real
/// point lights (BrightestN), each about 120 lm at the top of its flash over
/// 2 m, smoothed only 0.08 s so the light blinks with the bug.
fn fireflies() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(3.0));
    b.output("burst_count", f(12.0));
    b.output("size", f(0.03));
    b.output("color", col([2.2, 3.2, 0.5, 1.0]));
    let (pos, _) = bed(&mut b, [6.0, 1.2, 6.0]);
    let pos = lift(&mut b, pos, 1.0);
    b.output("position", pos);
    let vel = rand3(&mut b, [-0.2, -0.05, -0.2], [0.2, 0.05, 0.2]);
    b.output("velocity", vel);
    let life = rand(&mut b, 6.0, 10.0);
    b.output("lifetime", life);
    let s = wave(&mut b, (1.2, 2.2));
    let blink = b.op("math/smoothstep", &[("edge0", f(0.55)), ("edge1", f(0.95)), ("value", s)], "value");
    let glow = b.op("math/max", &[("a", blink), ("b", f(0.03))], "value");
    let fade = gradient(&mut b, &[(0.0, [1.0, 1.0, 1.0, 0.0]), (0.1, [1.0, 1.0, 1.0, 1.0]), (0.9, [1.0, 1.0, 1.0, 1.0]), (1.0, [1.0, 1.0, 1.0, 0.0])]);
    let c = b.mul(fade, glow);
    b.output("color_over_life", c);
    let wander = curl(&mut b, 0.35, 0.4, [0.1, 0.0, 0.0]);
    let brake = drag(&mut b, 0.8, 0.0);
    accelerate(&mut b, vec![wander, brake]);
    let mut bugs = emitter("fireflies", 48, Local, 511, b.finish(), dot_render(BlendMode::Additive));
    bugs.light = ParticleLight {
        mode: LightMode::BrightestN(3),
        color: [0.8, 1.0, 0.4],
        intensity: 40.0,
        range: 2.0,
        radius: 0.02,
        smoothing: 0.08,
        ..Default::default()
    };
    effect("Fireflies", 0.0, true, vec![bugs])
}

// ── impacts/ ────────────────────────────────────────────────────────────────

/// A footstep or landing puff, origin on the ground under the foot. 14
/// small puffs pushed out along the ground (up to 1.2 m/s, barely upward),
/// braked hard so they spread only 20 to 40 cm, swelling 2.2x and gone in
/// about a second. Fire it with `vfx_restart()` on each footfall.
fn dust_puff() -> VfxAsset {
    let puff = smoke(Smoke {
        name: "puff",
        seed: 601,
        capacity: 24,
        space: World,
        rate: 0.0,
        burst: 14.0,
        bed: [0.2, 0.02, 0.2],
        lift: 0.04,
        vel: ([-1.2, 0.1, -1.2], [1.2, 0.5, 1.2]),
        life: (0.8, 1.4),
        size: (0.1, 0.18),
        grow: 2.2,
        tint: [0.6, 0.53, 0.44, 0.45],
        stops: PUFF,
        buoyancy: -0.3,
        curl: (0.0, 0.0, 0.0),
        forces: &[Air::Still(3.0)],
        ground: Some((0.0, 0.8)),
        soft: 0.15,
        erosion: 0.2,
        shading: Shading::ProceduralSmoke,
    });
    effect("Dust Puff", 0.5, false, vec![puff])
}

/// A one-shot burst of debris, origin on the ground at the impact. 24 chunks
/// (3 to 7 cm, stone colours) thrown out and up at 3 to 6 m/s under full
/// gravity, spinning, bouncing off the ground (restitution 0.35) and sliding
/// to a stop on friction 0.5, then fading out at 2.5 to 3.5 s. A few slow
/// dust puffs mark the impact point.
fn debris_burst() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(0.0));
    b.output("burst_count", f(24.0));
    b.output("gravity", v3(0.0, -9.81, 0.0));
    let (pos, _) = bed(&mut b, [0.2, 0.1, 0.2]);
    let pos = lift(&mut b, pos, 0.08);
    b.output("position", pos);
    let vel = rand3(&mut b, [-3.0, 3.0, -3.0], [3.0, 6.0, 3.0]);
    b.output("velocity", vel);
    let life = rand(&mut b, 2.5, 3.5);
    b.output("lifetime", life);
    let size = rand(&mut b, 0.03, 0.07);
    b.output("size", size);
    let tint = rand_color(&mut b, [0.32, 0.28, 0.24, 1.0], [0.52, 0.47, 0.4, 1.0]);
    b.output("color", tint);
    let fade = gradient(&mut b, &[(0.0, [1.0, 1.0, 1.0, 1.0]), (0.85, [1.0, 1.0, 1.0, 1.0]), (1.0, [1.0, 1.0, 1.0, 0.0])]);
    b.output("color_over_life", fade);
    random_spin(&mut b, 8.0);
    let air_drag = drag(&mut b, 0.0, 0.05);
    b.output("acceleration", air_drag);
    ground(&mut b, 0.35, 0.5, false);
    let chunks = emitter("chunks", 32, World, 611, b.finish(), dot_render(BlendMode::Alpha));
    let dust = smoke(Smoke {
        name: "dust",
        seed: 612,
        capacity: 16,
        space: World,
        rate: 0.0,
        burst: 10.0,
        bed: [0.4, 0.1, 0.4],
        lift: 0.15,
        vel: ([-0.8, 0.2, -0.8], [0.8, 0.9, 0.8]),
        life: (1.5, 2.5),
        size: (0.25, 0.4),
        grow: 2.5,
        tint: [0.55, 0.5, 0.44, 0.4],
        stops: PUFF,
        buoyancy: -0.2,
        curl: (0.2, 0.8, 0.0),
        forces: &[Air::Still(2.0)],
        ground: Some((0.0, 0.8)),
        soft: 0.2,
        erosion: 0.15,
        shading: Shading::ProceduralSmoke,
    });
    effect("Debris Burst", 1.0, false, vec![chunks, dust])
}

// ── test/ ───────────────────────────────────────────────────────────────────

/// The plain dot every feature test starts from: a soft white-to-orange
/// fading dot, so whatever differs between tests is the feature itself.
fn test_fade(b: &mut B) {
    let fade = gradient(b, &[(0.0, [1.0, 1.0, 1.0, 1.0]), (0.7, [1.0, 0.75, 0.4, 0.8]), (1.0, [1.0, 0.5, 0.2, 0.0])]);
    b.output("color_over_life", fade);
}

/// Bursts only: 40 particles every second from a point, no spawn rate.
fn test_burst() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(0.0));
    b.output("burst_count", f(40.0));
    b.output("burst_interval", f(1.0));
    b.output("burst_cycles", f(0.0));
    b.output("lifetime", f(0.9));
    b.output("size", f(0.05));
    b.output("position", v3(0.0, 1.0, 0.0));
    let dir = b.op("random/unit_vector", &[], "value");
    let vel = b.mul(dir, f(1.5));
    b.output("velocity", vel);
    test_fade(&mut b);
    effect("Test Burst", 0.0, true, vec![emitter("burst", 64, Local, 701, b.finish(), dot_render(BlendMode::Alpha))])
}

/// A steady 20 per second, nothing else: a column rising at 0.5 m/s.
fn test_rate() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(20.0));
    b.output("lifetime", f(2.0));
    b.output("size", f(0.05));
    b.output("velocity", v3(0.0, 0.5, 0.0));
    test_fade(&mut b);
    effect("Test Rate", 0.0, true, vec![emitter("rate", 64, Local, 702, b.finish(), dot_render(BlendMode::Alpha))])
}

/// Wind only: dots float up on a little buoyancy against a Wind Force of
/// coupling 1. In calm air they rise at 0.3 m/s; in wind they stream away
/// downwind at the wind's speed.
fn test_wind() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(30.0));
    b.output("lifetime", f(4.0));
    b.output("size", f(0.05));
    b.output("gravity", v3(0.0, 0.3, 0.0));
    b.output("position", v3(0.0, 0.5, 0.0));
    let w = wind(&mut b, 1.0, 0.0);
    b.output("acceleration", w);
    test_fade(&mut b);
    effect("Test Wind", 0.0, true, vec![emitter("wind", 160, World, 703, b.finish(), dot_render(BlendMode::Alpha))])
}

/// Balls dropped from 2 m onto a ground plane (restitution 0.6, friction
/// 0.2) and a solid sphere 0.4 m across sitting on it, so both a plane and
/// a sphere collider are in view.
fn test_collision() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(8.0));
    b.output("lifetime", f(4.0));
    b.output("size", f(0.05));
    b.output("gravity", v3(0.0, -9.81, 0.0));
    let (pos, _) = shape(&mut b, "shape/sphere", &[("radius", PinValue::Float(0.1))]);
    let pos = lift(&mut b, pos, 2.0);
    b.output("position", pos);
    let vel = rand3(&mut b, [-0.4, 0.0, -0.4], [0.4, 0.5, 0.4]);
    b.output("velocity", vel);
    test_fade(&mut b);
    ground(&mut b, 0.6, 0.2, false);
    let ball = b.node("collide/sphere");
    b.set(ball, "center", PinValue::Vec3([0.0, 0.2, 0.0]));
    b.set(ball, "radius", PinValue::Float(0.2));
    b.set(ball, "bounce", PinValue::Float(0.6));
    b.set(ball, "friction", PinValue::Float(0.2));
    effect("Test Collision", 0.0, true, vec![emitter("balls", 48, World, 704, b.finish(), dot_render(BlendMode::Alpha))])
}

/// Ribbon Trail: each particle drags its own 24-point tail. They leave a
/// point 1 m up and a vortex curls their paths so the trails visibly bend.
fn test_ribbon_trail() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(6.0));
    b.output("lifetime", f(2.5));
    b.output("size", f(0.05));
    b.output("position", v3(0.0, 1.0, 0.0));
    let dir = b.op("random/unit_vector", &[], "value");
    let vel = b.mul(dir, f(1.5));
    b.output("velocity", vel);
    let spin = b.op("force/vortex", &[("center", v3(0.0, 1.0, 0.0)), ("strength", f(2.0))], "force");
    let brake = drag(&mut b, 0.3, 0.0);
    accelerate(&mut b, vec![spin, brake]);
    test_fade(&mut b);
    let render = RenderSettings {
        blend: BlendMode::Additive,
        facing: Facing::Ribbon,
        ribbon: Ribbon { kind: RibbonKind::Trail, points: 24, min_distance: 0.03, ..Default::default() },
        ..Default::default()
    };
    effect("Test Ribbon Trail", 0.0, true, vec![emitter("trails", 24, World, 705, b.finish(), render)])
}

/// Ribbon Connect: one strip through the particles in spawn order. The spawn
/// point circles 0.5 m around the axis (from Emitter Time) while particles
/// rise at 0.4 m/s, so the strip is a helix even with the emitter still.
fn test_ribbon_connect() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(20.0));
    b.output("lifetime", f(3.0));
    b.output("size", f(0.06));
    b.output("velocity", v3(0.0, 0.4, 0.0));
    let t = b.op("input/emitter_time", &[], "value");
    let a = b.mul(t, f(2.0));
    let s = b.op("math/sin", &[("value", a.clone())], "value");
    let c = b.op("math/cos", &[("value", a)], "value");
    let x = b.mul(c, f(0.5));
    let z = b.mul(s, f(0.5));
    let pos = b.op("math/compose_vec3", &[("x", x), ("y", f(0.2)), ("z", z)], "value");
    b.output("position", pos);
    test_fade(&mut b);
    let render = RenderSettings {
        blend: BlendMode::Additive,
        facing: Facing::Ribbon,
        ribbon: Ribbon { kind: RibbonKind::Connect, groups: 1, tail_width: 0.2, tail_alpha: 0.0, ..Default::default() },
        ..Default::default()
    };
    effect("Test Ribbon Connect", 0.0, true, vec![emitter("strip", 80, World, 706, b.finish(), render)])
}

/// Sub-emitters, both triggers: a shell rises and drops a trail dot every
/// 0.05 s (Interval, target 2), then bursts into 24 children when it dies
/// (OnDeath, target 1), which inherit its colour.
fn test_sub_emitter() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(1.5));
    b.output("lifetime", f(1.1));
    b.output("size", f(0.08));
    b.output("gravity", v3(0.0, -3.0, 0.0));
    let tint = rand_color(&mut b, [1.0, 0.6, 0.3, 1.0], [0.4, 0.7, 1.0, 1.0]);
    b.output("color", tint);
    let vel = rand3(&mut b, [-0.4, 4.0, -0.4], [0.4, 5.0, 0.4]);
    b.output("velocity", vel);
    let mut shell = emitter("shell", 8, World, 707, b.finish(), dot_render(BlendMode::Additive));
    shell.sub_emitters = vec![
        SubEmitter { trigger: SubEmitterTrigger::OnDeath, target: 1, count: 24, inherit_velocity: 0.0, inherit_color: true },
        SubEmitter { trigger: SubEmitterTrigger::Interval(0.05), target: 2, count: 1, inherit_velocity: 0.0, inherit_color: false },
    ];

    let mut b = B::default();
    b.output("gravity", v3(0.0, -2.0, 0.0));
    b.output("size", f(0.04));
    let (_, dir) = shape(&mut b, "shape/sphere", &[("radius", PinValue::Float(0.05)), ("surface", PinValue::Bool(true))]);
    let speed = rand(&mut b, 1.0, 2.0);
    let vel = b.mul(dir, speed);
    b.output("velocity", vel);
    let life = rand(&mut b, 0.8, 1.2);
    b.output("lifetime", life);
    test_fade(&mut b);
    let children = emitter("burst children", 96, World, 708, b.finish(), dot_render(BlendMode::Additive));

    let mut b = B::default();
    b.output("size", f(0.03));
    b.output("lifetime", f(0.5));
    b.output("color", col([0.8, 0.8, 0.8, 0.6]));
    test_fade(&mut b);
    let trail = emitter("trail", 96, World, 709, b.finish(), dot_render(BlendMode::Alpha));
    effect("Test Sub Emitter", 0.0, true, vec![shell, children, trail])
}

/// One PerEmitter light: a steady orange glow ball 0.5 m up with its light
/// at its centre, about 35000 lm over 6 m. No flicker, so the light level
/// itself is what is being checked.
fn test_light_per_emitter() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(30.0));
    b.output("size", f(0.12));
    b.output("color", col([2.0, 0.9, 0.35, 1.0]));
    let (pos, dir) = shape(&mut b, "shape/sphere", &[("radius", PinValue::Float(0.15))]);
    let pos = lift(&mut b, pos, 0.5);
    b.output("position", pos);
    let vel = b.mul(dir, f(0.2));
    b.output("velocity", vel);
    let life = rand(&mut b, 0.6, 1.0);
    b.output("lifetime", life);
    let fade = gradient(&mut b, &[(0.0, [1.0, 1.0, 1.0, 0.0]), (0.2, [1.0, 1.0, 1.0, 1.0]), (1.0, [1.0, 1.0, 1.0, 0.0])]);
    b.output("color_over_life", fade);
    let mut glow = emitter("glow", 48, Local, 710, b.finish(), dot_render(BlendMode::Additive));
    glow.light = ParticleLight {
        mode: LightMode::PerEmitter,
        intensity: 2500.0,
        range: 6.0,
        radius: 0.15,
        offset: [0.0, 0.5, 0.0],
        ..Default::default()
    };
    effect("Test Light Per Emitter", 0.0, true, vec![glow])
}

/// BrightestN(4): a handful of drifting orbs, the four brightest carrying a
/// light each (about 1500 lm over 3 m), which should follow them around.
fn test_light_brightest() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(1.5));
    b.output("burst_count", f(4.0));
    b.output("lifetime", f(4.0));
    b.output("size", f(0.08));
    b.output("color", col([0.6, 1.2, 2.4, 1.0]));
    let (pos, _) = bed(&mut b, [2.0, 0.6, 2.0]);
    let pos = lift(&mut b, pos, 0.8);
    b.output("position", pos);
    let fade = gradient(&mut b, &[(0.0, [1.0, 1.0, 1.0, 0.0]), (0.15, [1.0, 1.0, 1.0, 1.0]), (0.85, [1.0, 1.0, 1.0, 1.0]), (1.0, [1.0, 1.0, 1.0, 0.0])]);
    b.output("color_over_life", fade);
    let wander = curl(&mut b, 0.4, 0.5, [0.1, 0.0, 0.0]);
    let brake = drag(&mut b, 0.8, 0.0);
    accelerate(&mut b, vec![wander, brake]);
    let mut orbs = emitter("orbs", 16, Local, 711, b.finish(), dot_render(BlendMode::Additive));
    orbs.light = ParticleLight {
        mode: LightMode::BrightestN(4),
        intensity: 1300.0,
        range: 3.0,
        radius: 0.05,
        ..Default::default()
    };
    effect("Test Light Brightest", 0.0, true, vec![orbs])
}

/// Keyed tracks over a 4 s loop: emitter 0's spawn rate ramps 5 to 80 and
/// back (Smooth), with keyed bursts of 40 at 1 s and 3 s; emitter 1 starts
/// disabled and an Enabled track switches it on from 2 s to 3 s only.
fn test_keyed_tracks() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(5.0));
    b.output("lifetime", f(1.5));
    b.output("size", f(0.05));
    let dir = b.op("random/unit_vector", &[], "value");
    let vel = b.mul(dir, f(0.6));
    b.output("velocity", vel);
    test_fade(&mut b);
    let keyed = emitter("keyed rate", 192, Local, 712, b.finish(), dot_render(BlendMode::Alpha));

    let mut b = B::default();
    b.output("spawn_rate", f(30.0));
    b.output("lifetime", f(1.0));
    b.output("size", f(0.05));
    b.output("position", v3(1.0, 0.0, 0.0));
    b.output("color", col([0.4, 0.8, 1.0, 1.0]));
    b.output("velocity", v3(0.0, 0.8, 0.0));
    test_fade(&mut b);
    let mut toggled = emitter("toggled", 48, Local, 713, b.finish(), dot_render(BlendMode::Alpha));
    toggled.enabled = false;

    let key = |time: f32, value: PinValue, interp: KeyInterp| ParamKey { time, value, interp };
    let mut asset = effect("Test Keyed Tracks", 4.0, true, vec![keyed, toggled]);
    asset.tracks = vec![
        ParamTrack {
            emitter: 0,
            target: ParamTarget::Pin { node: OUTPUT_ID, pin: "spawn_rate".to_string() },
            keys: vec![
                key(0.0, PinValue::Float(5.0), KeyInterp::Smooth),
                key(2.0, PinValue::Float(80.0), KeyInterp::Smooth),
                key(4.0, PinValue::Float(5.0), KeyInterp::Linear),
            ],
        },
        ParamTrack {
            emitter: 0,
            target: ParamTarget::Burst,
            keys: vec![key(1.0, PinValue::Float(40.0), KeyInterp::Linear), key(3.0, PinValue::Float(40.0), KeyInterp::Linear)],
        },
        ParamTrack {
            emitter: 1,
            target: ParamTarget::Enabled,
            keys: vec![
                key(0.0, PinValue::Bool(false), KeyInterp::Step),
                key(2.0, PinValue::Bool(true), KeyInterp::Step),
                key(3.0, PinValue::Bool(false), KeyInterp::Step),
            ],
        },
    ];
    asset
}

/// The space pair: the same slow fountain of dots, World in one file and
/// Local in the other. Play them with the preview's Move set to Circle: the
/// World one leaves a trail, the Local one moves as a block.
fn test_space(space: SimulationSpace) -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(40.0));
    b.output("lifetime", f(2.5));
    b.output("size", f(0.05));
    b.output("gravity", v3(0.0, -0.2, 0.0));
    b.output("drag", f(0.4));
    let (pos, dir) = shape(&mut b, "shape/cone", &[("angle", PinValue::Float(15.0)), ("radius", PinValue::Float(0.05))]);
    b.output("position", pos);
    let vel = b.mul(dir, f(0.8));
    b.output("velocity", vel);
    test_fade(&mut b);
    let name = match space {
        World => "Test World Space",
        Local => "Test Local Space",
    };
    effect(name, 0.0, true, vec![emitter("dots", 128, space, 714, b.finish(), dot_render(BlendMode::Alpha))])
}

/// 2D, the basics: a spray of 10 px dots from a 20 px circle, in pixels.
fn test_two_d_basic() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(40.0));
    b.output("lifetime", f(1.5));
    b.output("size", f(10.0));
    let (pos, dir) = shape(&mut b, "shape/circle", &[("radius", PinValue::Float(20.0))]);
    b.output("position", pos);
    let speed = rand(&mut b, 60.0, 140.0);
    let vel = b.mul(dir, speed);
    b.output("velocity", vel);
    test_fade(&mut b);
    let mut asset = effect("Test 2D Basic", 0.0, true, vec![emitter("dots", 96, Local, 715, b.finish(), dot_render(BlendMode::Alpha))]);
    asset.dimension = Dimension::TwoD;
    asset
}

/// 2D under gravity: 8 px dots thrown up from a 200 px strip at 200 to 350
/// px/s against 600 px/s^2, so they arc and fall about 100 px below.
fn test_two_d_gravity() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(30.0));
    b.output("lifetime", f(2.0));
    b.output("size", f(8.0));
    b.output("gravity", v3(0.0, -600.0, 0.0));
    let (pos, _) = shape(&mut b, "shape/rect", &[("size", PinValue::Vec2([200.0, 10.0]))]);
    b.output("position", pos);
    let vel = rand3(&mut b, [-80.0, 200.0, 0.0], [80.0, 350.0, 0.0]);
    b.output("velocity", vel);
    test_fade(&mut b);
    let mut asset = effect("Test 2D Gravity", 0.0, true, vec![emitter("dots", 96, Local, 716, b.finish(), dot_render(BlendMode::Alpha))]);
    asset.dimension = Dimension::TwoD;
    asset
}

// ── showcase/ ───────────────────────────────────────────────────────────────
//
// Effects that lean on everything at once: mesh particles wearing project
// materials, SVG and PNG textures, flipbooks, lit and shadowed particles, GPU
// sub-emitters, tube trails, the parity forces and script parameters. They
// expect the project's `particles/showcase/textures/` art and
// `materials/particles/` materials; a missing file draws the plain particle
// look rather than failing, so the files still validate here without them.

const SHOWCASE_TEX: &str = "particles/showcase/textures";
const SHOWCASE_MAT: &str = "materials/particles";

fn tex(name: &str) -> Option<String> {
    Some(format!("{SHOWCASE_TEX}/{name}"))
}

fn mat(name: &str) -> Option<String> {
    Some(format!("{SHOWCASE_MAT}/{name}"))
}

fn sub(trigger: SubEmitterTrigger, target: usize, count: u32, inherit_velocity: f32, inherit_color: bool) -> SubEmitter {
    SubEmitter { trigger, target, count, inherit_velocity, inherit_color }
}

/// A random direction times a random speed, plus `bias`: a burst that leans.
fn burst_velocity(b: &mut B, speed: (f32, f32), bias: [f32; 3]) -> Src {
    let dir = b.op("random/unit_vector", &[], "value");
    let s = rand(b, speed.0, speed.1);
    let v = b.mul(dir, s);
    b.add(v, Src::vec3(bias))
}

/// An Emission curve: HDR brightness over life, for bloom that cools.
fn glow(b: &mut B, from: f32, to: f32) -> Src {
    curve(b, &[ease_out(0.0, from), ease(1.0, to)])
}

fn fade_in_out(b: &mut B, fade_in: f32, hold: f32) -> Src {
    gradient(b, &[(0.0, [1.0, 1.0, 1.0, 0.0]), (fade_in, [1.0, 1.0, 1.0, 1.0]), (hold, [1.0, 1.0, 1.0, 1.0]), (1.0, [1.0, 1.0, 1.0, 0.0])])
}

/// A meteor every four seconds: a white-hot ball dragging a flame trail
/// (Interval sub-emitter) slams down, and on death bursts into molten rock
/// that cools as it ages (a material reading Particle Age), spark trails that
/// light the crater, and lit dust from a PNG flipbook.
fn meteor_impact() -> VfxAsset {
    // The fall is timed to land: from 12 m at 8 m/s down under gravity the
    // ball reaches the ground at about 0.95 s, which is its lifetime, so its
    // death (and every burst it triggers) happens at the crater.
    let mut b = B::default();
    b.output("spawn_rate", f(0.0));
    b.output("burst_count", f(1.0));
    b.output("burst_interval", f(4.0));
    b.output("burst_cycles", f(0.0));
    b.output("position", v3(-5.7, 12.0, -2.85));
    b.output("velocity", v3(6.0, -8.0, 3.0));
    b.output("gravity", v3(0.0, -9.81, 0.0));
    b.output("lifetime", f(0.95));
    b.output("size", f(0.7));
    b.output("color", col([3.0, 1.3, 0.45, 1.0]));
    random_spin(&mut b, 3.0);
    let ball = RenderSettings { blend: BlendMode::Opaque, mesh: Some(ParticleMesh::Sphere), ..Default::default() };
    let mut meteor = emitter("meteor", 2, World, 801, b.finish(), ball);
    meteor.sub_emitters = vec![
        sub(SubEmitterTrigger::Interval(0.02), 1, 2, 0.25, false),
        sub(SubEmitterTrigger::OnDeath, 2, 40, 0.0, false),
        sub(SubEmitterTrigger::OnDeath, 3, 60, 0.0, false),
        sub(SubEmitterTrigger::OnDeath, 4, 14, 0.0, false),
    ];

    // Flame shed along the path, rising a little as it burns off.
    let mut b = B::default();
    let life = rand(&mut b, 0.25, 0.45);
    b.output("lifetime", life);
    let size = rand(&mut b, 0.35, 0.6);
    b.output("size", size);
    let vel = rand3(&mut b, [-0.3, 0.2, -0.3], [0.3, 0.8, 0.3]);
    b.output("velocity", vel);
    b.output("color", col([2.2, 0.95, 0.4, 0.9]));
    let shrink = curve(&mut b, &[ease(0.0, 1.0), ease(1.0, 0.3)]);
    b.output("size_over_life", shrink);
    let e = glow(&mut b, 3.0, 1.0);
    b.output("emission", e);
    let trail = emitter("flame trail", 128, World, 802, b.finish(), flame_render(1.2));

    // Molten rock: tetrahedra wearing the cooling-lava material, tumbling,
    // bouncing on the ground and casting shadows (Opaque casts on Auto).
    let mut b = B::default();
    let vel = burst_velocity(&mut b, (3.0, 7.0), [0.0, 3.5, 0.0]);
    b.output("velocity", vel);
    b.output("gravity", v3(0.0, -9.81, 0.0));
    let life = rand(&mut b, 3.0, 3.8);
    b.output("lifetime", life);
    let size = rand(&mut b, 0.08, 0.22);
    b.output("size", size);
    random_spin(&mut b, 10.0);
    let air = drag(&mut b, 0.0, 0.04);
    b.output("acceleration", air);
    ground(&mut b, 0.3, 0.6, false);
    let rock = RenderSettings {
        blend: BlendMode::Opaque,
        mesh: Some(ParticleMesh::Tetrahedron),
        orient: MeshOrient::Tumble,
        material: mat("fx_molten_debris.material"),
        ..Default::default()
    };
    let debris = emitter("molten debris", 64, World, 803, b.finish(), rock);

    // Sparks as short flat ribbon trails, HDR bright and cooling, with a
    // warm flash of light at the crater while they live.
    let mut b = B::default();
    let vel = burst_velocity(&mut b, (6.0, 13.0), [0.0, 4.0, 0.0]);
    b.output("velocity", vel);
    b.output("gravity", v3(0.0, -9.81, 0.0));
    b.output("drag", f(0.4));
    let life = rand(&mut b, 0.8, 1.6);
    b.output("lifetime", life);
    b.output("size", f(0.04));
    b.output("color", col([3.0, 1.4, 0.5, 1.0]));
    let fade = gradient(&mut b, &[(0.0, [1.0, 1.0, 1.0, 1.0]), (0.7, [1.0, 0.7, 0.4, 1.0]), (1.0, [1.0, 0.4, 0.2, 0.0])]);
    b.output("color_over_life", fade);
    let e = glow(&mut b, 6.0, 1.0);
    b.output("emission", e);
    ground(&mut b, 0.4, 0.3, false);
    let streaks = RenderSettings {
        blend: BlendMode::Additive,
        facing: Facing::Ribbon,
        ribbon: Ribbon { kind: RibbonKind::Trail, points: 10, min_distance: 0.04, ..Default::default() },
        ..Default::default()
    };
    let mut sparks = emitter("impact sparks", 96, World, 804, b.finish(), streaks);
    // Light intensity is per particle (weighted by its brightness), so 60
    // fresh sparks flash at about 20000 lm and the light dies with them.
    sparks.light = warm_light(350.0, 10.0, 0.3, 0.5);

    // Dust from the PNG flipbook: lit by the sun, drawn oldest first so new
    // billows sit on top, drifting on the wind.
    let mut b = B::default();
    let vel = burst_velocity(&mut b, (1.0, 2.5), [0.0, 0.6, 0.0]);
    b.output("velocity", vel);
    b.output("gravity", v3(0.0, 0.15, 0.0));
    b.output("drag", f(1.5));
    let life = rand(&mut b, 2.5, 3.5);
    b.output("lifetime", life);
    let size = rand(&mut b, 0.8, 1.4);
    b.output("size", size);
    let grow = curve(&mut b, &[ease_out(0.0, 0.6), ease(1.0, 2.2)]);
    b.output("size_over_life", grow);
    b.output("color", col([0.42, 0.38, 0.34, 0.75]));
    let fade = fade_in_out(&mut b, 0.1, 0.5);
    b.output("color_over_life", fade);
    random_spin(&mut b, 0.4);
    let swirl = curl(&mut b, 0.4, 0.6, [0.0, 0.2, 0.0]);
    let w = wind(&mut b, 0.6, 0.0);
    accelerate(&mut b, vec![swirl, w]);
    let puffs = RenderSettings {
        texture: tex("smoke_sheet.png"),
        sprite_sheet: SpriteSheet { columns: 4, rows: 4, mode: SheetMode::OverLifetime },
        frame_blend: true,
        blend: BlendMode::Alpha,
        soft_distance: 0.4,
        lighting: Lighting { lit: true, roughness: 1.0, metallic: 0.0 },
        draw_order: DrawOrder::OldestFirst,
        ..Default::default()
    };
    let dust = emitter("impact dust", 32, World, 805, b.finish(), puffs);

    effect("Meteor Impact", 4.0, true, vec![meteor, trail, debris, sparks, dust])
}

/// A whirlwind of autumn leaves: SVG leaves as tumbling alpha-masked quads,
/// lit and casting shadows, held on a ring by Orbit while they rise, with
/// screen-space SVG glints that stay pin-sharp at any distance.
fn leaf_whirlwind() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(25.0));
    let (pos, _) = bed(&mut b, [3.0, 0.2, 3.0]);
    let pos = lift(&mut b, pos, 0.2);
    b.output("position", pos);
    let life = rand(&mut b, 5.0, 7.0);
    b.output("lifetime", life);
    let size = rand(&mut b, 0.18, 0.3);
    b.output("size", size);
    let tint = rand_color(&mut b, [1.0, 1.0, 1.0, 1.0], [0.8, 0.55, 0.4, 1.0]);
    b.output("color", tint);
    let fade = fade_in_out(&mut b, 0.08, 0.85);
    b.output("color_over_life", fade);
    b.output("gravity", v3(0.0, 0.45, 0.0));
    b.output("drag", f(0.5));
    random_spin(&mut b, 4.0);
    let spin = b.op(
        "force/orbit",
        &[("center", v3(0.0, 0.0, 0.0)), ("axis", v3(0.0, 1.0, 0.0)), ("speed", f(1.6)), ("grip", f(1.5)), ("radius", f(1.6)), ("radial_pull", f(1.5))],
        "force",
    );
    let flutter = curl(&mut b, 0.8, 0.7, [0.0, 0.3, 0.0]);
    let w = wind(&mut b, 0.4, 0.0);
    accelerate(&mut b, vec![spin, flutter, w]);
    let leaves_look = RenderSettings {
        texture: tex("leaf.svg"),
        blend: BlendMode::AlphaMask,
        mesh: Some(ParticleMesh::Quad),
        orient: MeshOrient::Tumble,
        lighting: Lighting { lit: true, roughness: 0.8, metallic: 0.0 },
        ..Default::default()
    };
    let mut leaves = emitter("leaves", 200, World, 811, b.finish(), leaves_look);
    leaves.backend = Backend::Gpu;

    // Glints riding the same vortex, sized in pixels.
    let mut b = B::default();
    b.output("spawn_rate", f(30.0));
    let (pos, _) = shape(&mut b, "shape/sphere", &[("radius", PinValue::Float(1.8))]);
    let pos = lift(&mut b, pos, 1.6);
    b.output("position", pos);
    let life = rand(&mut b, 1.2, 2.0);
    b.output("lifetime", life);
    let px = rand(&mut b, 12.0, 24.0);
    b.output("size", px);
    b.output("color", col([1.6, 1.3, 0.7, 1.0]));
    let twinkle = fade_in_out(&mut b, 0.3, 0.5);
    b.output("color_over_life", twinkle);
    b.output("emission", f(2.0));
    let spin = b.op("force/orbit", &[("speed", f(1.6)), ("grip", f(2.0))], "force");
    b.output("acceleration", spin);
    let glints_look = RenderSettings { texture: tex("star.svg"), blend: BlendMode::Additive, screen_space_size: true, ..Default::default() };
    let glints = emitter("glints", 96, World, 812, b.finish(), glints_look);

    effect("Leaf Whirlwind", 0.0, true, vec![leaves, glints])
}

/// A standing portal facing +Z: two SVG sigils turning in place as mesh
/// quads, motes orbiting the rim as glowing tube trails (a material on a
/// ribbon), a core of sparks held on a sphere by Conform To Sphere, streaks
/// falling in through a Gravity Well, and SVG flipbook glyphs rising at its
/// foot. The motes' rate is the `intensity` parameter, for scripts.
fn arcane_portal() -> VfxAsset {
    const CENTER: [f32; 3] = [0.0, 1.6, 0.0];
    let center = || Src::vec3(CENTER);

    let mut b = B::default();
    b.output("spawn_rate", f(0.5));
    b.output("lifetime", f(2.2));
    b.output("position", center());
    b.output("size", f(3.2));
    b.output("color", col([0.6, 0.35, 1.6, 1.0]));
    let fade = fade_in_out(&mut b, 0.25, 0.7);
    b.output("color_over_life", fade);
    b.output("emission", f(2.5));
    let r = rand(&mut b, 0.0, TAU);
    b.output("rotation", r);
    b.output("angular_velocity", f(0.4));
    let sigil_look = RenderSettings {
        texture: tex("rune_circle.svg"),
        blend: BlendMode::Additive,
        mesh: Some(ParticleMesh::Quad),
        orient: MeshOrient::Axis([0.0, 0.0, 1.0]),
        ..Default::default()
    };
    let sigil = emitter("sigil", 4, Local, 821, b.finish(), sigil_look);

    // Orbiting motes. Their spawn rate is a Parameter: a script raises the
    // portal's intensity with vfx_set_param("intensity", 120).
    let mut b = B::default();
    let intensity = b.node("input/parameter");
    b.set(intensity, "name", PinValue::Text("intensity".into()));
    b.set(intensity, "value", PinValue::Float(40.0));
    b.output("spawn_rate", Src::Pin(intensity, "value"));
    let (pos, _) = shape(&mut b, "shape/sphere", &[("radius", PinValue::Float(1.4)), ("surface", PinValue::Bool(true))]);
    let pos = b.add(pos, center());
    b.output("position", pos);
    let life = rand(&mut b, 2.5, 3.5);
    b.output("lifetime", life);
    b.output("size", f(0.05));
    let tint = rand_color(&mut b, [0.7, 0.3, 2.0, 1.0], [0.3, 1.2, 2.0, 1.0]);
    b.output("color", tint);
    let fade = fade_in_out(&mut b, 0.15, 0.8);
    b.output("color_over_life", fade);
    let ring = b.op(
        "force/orbit",
        &[("center", center()), ("axis", v3(0.0, 0.0, 1.0)), ("speed", f(2.5)), ("grip", f(6.0)), ("radius", f(1.25)), ("radial_pull", f(6.0))],
        "force",
    );
    b.output("acceleration", ring);
    let tubes = RenderSettings {
        blend: BlendMode::Additive,
        facing: Facing::Ribbon,
        ribbon: Ribbon {
            kind: RibbonKind::Trail,
            points: 20,
            min_distance: 0.03,
            tail_width: 0.1,
            shape: RibbonShape::Tube,
            radial_segments: 6,
            ..Default::default()
        },
        material: mat("fx_energy_ribbon.material"),
        ..Default::default()
    };
    let mut motes = emitter("motes", 160, Local, 822, b.finish(), tubes);
    motes.backend = Backend::Gpu;

    // The core: sparks drawn onto a 0.45 m sphere and held there.
    let mut b = B::default();
    b.output("spawn_rate", f(50.0));
    let (pos, _) = shape(&mut b, "shape/sphere", &[("radius", PinValue::Float(1.5))]);
    let pos = b.add(pos, center());
    b.output("position", pos);
    let life = rand(&mut b, 3.0, 4.0);
    b.output("lifetime", life);
    b.output("size", f(0.03));
    b.output("color", col([1.2, 0.8, 2.4, 1.0]));
    let fade = fade_in_out(&mut b, 0.2, 0.8);
    b.output("color_over_life", fade);
    b.output("emission", f(3.0));
    let hold = b.op(
        "force/conform_sphere",
        &[("center", center()), ("radius", f(0.45)), ("influence", f(3.0)), ("attraction", f(8.0)), ("max_speed", f(2.5))],
        "force",
    );
    let churn = curl(&mut b, 0.6, 2.0, [0.0, 0.0, 0.4]);
    accelerate(&mut b, vec![churn, hold]);
    let mut core = emitter("core", 220, Local, 823, b.finish(), dot_render(BlendMode::Additive));
    core.light = ParticleLight {
        mode: LightMode::PerEmitter,
        color: [0.6, 0.3, 1.0],
        // Per particle: about 150 live sparks give roughly 15000 lm.
        intensity: 40.0,
        range: 8.0,
        radius: 0.3,
        flicker: 0.15,
        flicker_rate: 3.0,
        offset: CENTER,
        ..Default::default()
    };

    // Streaks falling in from a 3.5 m shell through a softened Gravity Well.
    let mut b = B::default();
    b.output("spawn_rate", f(50.0));
    let (pos, _) = shape(&mut b, "shape/sphere", &[("radius", PinValue::Float(3.5)), ("surface", PinValue::Bool(true))]);
    let pos = b.add(pos, center());
    b.output("position", pos);
    b.output("lifetime", f(1.6));
    b.output("size", f(0.03));
    b.output("color", col([0.5, 0.7, 2.0, 1.0]));
    let fade = fade_in_out(&mut b, 0.3, 0.7);
    b.output("color_over_life", fade);
    let well = b.op(
        "force/gravity_well",
        &[("point", center()), ("strength", f(6.0)), ("softening", f(0.3)), ("max_force", f(30.0))],
        "force",
    );
    b.output("acceleration", well);
    let infall = emitter("infall", 120, Local, 824, b.finish(), streak_render(BlendMode::Additive, 0.08));

    // Glyphs from a 4 x 1 SVG flipbook, rising from the portal's foot.
    let mut b = B::default();
    b.output("spawn_rate", f(12.0));
    let (pos, _) = bed(&mut b, [2.0, 0.1, 0.6]);
    let pos = lift(&mut b, pos, 0.1);
    b.output("position", pos);
    let rise = rand3(&mut b, [0.0, 0.4, 0.0], [0.0, 0.8, 0.0]);
    b.output("velocity", rise);
    let life = rand(&mut b, 2.0, 3.0);
    b.output("lifetime", life);
    let size = rand(&mut b, 0.18, 0.28);
    b.output("size", size);
    b.output("color", col([0.8, 0.5, 2.0, 1.0]));
    let fade = fade_in_out(&mut b, 0.2, 0.6);
    b.output("color_over_life", fade);
    let glyph_look = RenderSettings {
        texture: tex("glyph_sheet.svg"),
        sprite_sheet: SpriteSheet { columns: 4, rows: 1, mode: SheetMode::Fps(6.0) },
        blend: BlendMode::Additive,
        ..Default::default()
    };
    let glyphs = emitter("glyphs", 48, Local, 825, b.finish(), glyph_look);

    effect("Arcane Portal", 0.0, true, vec![sigil, motes, core, infall, glyphs])
}

/// A geyser of crystal shards on the GPU: 120 tumbling mesh shards a second
/// wearing a material that gives each its own hue (Particle Random), and on
/// death each bursts into SVG star glints, a GPU sub-emitter.
fn crystal_geyser() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(120.0));
    let (pos, dir) = shape(&mut b, "shape/cone", &[("angle", PinValue::Float(18.0)), ("radius", PinValue::Float(0.3))]);
    b.output("position", pos);
    let speed = rand(&mut b, 7.0, 11.0);
    let vel = b.mul(dir, speed);
    b.output("velocity", vel);
    b.output("gravity", v3(0.0, -9.81, 0.0));
    let life = rand(&mut b, 2.4, 3.2);
    b.output("lifetime", life);
    let size = rand(&mut b, 0.06, 0.14);
    b.output("size", size);
    random_spin(&mut b, 9.0);
    ground(&mut b, 0.45, 0.3, false);
    let shard_look = RenderSettings {
        blend: BlendMode::Opaque,
        mesh: Some(ParticleMesh::Tetrahedron),
        orient: MeshOrient::Tumble,
        material: mat("fx_crystal_shard.material"),
        ..Default::default()
    };
    let mut shards = emitter("shards", 1200, World, 831, b.finish(), shard_look);
    shards.backend = Backend::Gpu;
    shards.sub_emitters = vec![sub(SubEmitterTrigger::OnDeath, 1, 4, 0.1, false)];
    shards.light = ParticleLight {
        mode: LightMode::PerEmitter,
        color: [0.4, 0.8, 1.0],
        // Per particle: about 330 live shards give roughly 13000 lm.
        intensity: 40.0,
        range: 9.0,
        radius: 0.4,
        offset: [0.0, 2.0, 0.0],
        ..Default::default()
    };

    let mut b = B::default();
    let vel = burst_velocity(&mut b, (0.3, 0.9), [0.0, 0.2, 0.0]);
    b.output("velocity", vel);
    let life = rand(&mut b, 0.4, 0.7);
    b.output("lifetime", life);
    b.output("size", f(0.25));
    b.output("color", col([0.8, 1.4, 2.2, 1.0]));
    let fade = fade_in_out(&mut b, 0.1, 0.4);
    b.output("color_over_life", fade);
    let e = glow(&mut b, 4.0, 0.5);
    b.output("emission", e);
    let star_look = RenderSettings { texture: tex("star.svg"), blend: BlendMode::Additive, ..Default::default() };
    let mut glints = emitter("shatter glints", 640, World, 832, b.finish(), star_look);
    glints.backend = Backend::Gpu;

    effect("Crystal Geyser", 0.0, true, vec![shards, glints])
}

/// Soap bubbles: sphere meshes wearing a fresnel soap-film material, sorted
/// back to front, drifting on the wind, and popping into droplets when they
/// touch the level (Collide Scene kills them, OnDeath spawns the pop).
fn soap_bubbles() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(6.0));
    let (pos, _) = bed(&mut b, [1.0, 0.1, 1.0]);
    let pos = lift(&mut b, pos, 1.0);
    b.output("position", pos);
    let vel = rand3(&mut b, [-0.3, 0.3, -0.3], [0.3, 0.7, 0.3]);
    b.output("velocity", vel);
    // Most pop on their own within a few seconds; the level pops the rest.
    let life = rand(&mut b, 1.5, 6.0);
    b.output("lifetime", life);
    let size = rand(&mut b, 0.12, 0.35);
    b.output("size", size);
    b.output("gravity", v3(0.0, 0.05, 0.0));
    b.output("drag", f(0.6));
    let wobble = curl(&mut b, 0.3, 0.5, [0.0, 0.1, 0.0]);
    let w = wind(&mut b, 0.8, 0.0);
    accelerate(&mut b, vec![wobble, w]);
    let scene = b.node("collide/scene");
    b.set(scene, "radius", PinValue::Float(0.12));
    b.set(scene, "kill", PinValue::Bool(true));
    let bubble_look = RenderSettings {
        blend: BlendMode::Alpha,
        mesh: Some(ParticleMesh::Sphere),
        material: mat("fx_bubble.material"),
        draw_order: DrawOrder::ViewDepth,
        ..Default::default()
    };
    let mut bubbles = emitter("bubbles", 64, World, 841, b.finish(), bubble_look);
    bubbles.sub_emitters = vec![sub(SubEmitterTrigger::OnDeath, 1, 14, 0.3, false)];

    let mut b = B::default();
    let vel = burst_velocity(&mut b, (0.6, 1.4), [0.0, 0.0, 0.0]);
    b.output("velocity", vel);
    b.output("gravity", v3(0.0, -4.0, 0.0));
    let life = rand(&mut b, 0.25, 0.4);
    b.output("lifetime", life);
    b.output("size", f(0.015));
    b.output("color", col([0.9, 0.95, 1.0, 0.8]));
    test_fade(&mut b);
    let pop = emitter("pop droplets", 160, World, 842, b.finish(), dot_render(BlendMode::Alpha));

    effect("Soap Bubbles", 0.0, true, vec![bubbles, pop])
}

/// A factory stack's smoke from the PNG flipbook: lit by the sun, casting
/// soft shadows onto the ground (Cast Shadows On), drawn oldest first and
/// leaning over in the wind.
fn lit_smoke_stack() -> VfxAsset {
    let mut b = B::default();
    b.output("spawn_rate", f(14.0));
    let (pos, dir) = shape(&mut b, "shape/cone", &[("angle", PinValue::Float(10.0)), ("radius", PinValue::Float(0.4))]);
    b.output("position", pos);
    let speed = rand(&mut b, 1.6, 2.4);
    let vel = b.mul(dir, speed);
    b.output("velocity", vel);
    b.output("gravity", v3(0.0, 0.35, 0.0));
    b.output("drag", f(0.5));
    let life = rand(&mut b, 6.0, 8.0);
    b.output("lifetime", life);
    let size = rand(&mut b, 0.9, 1.3);
    b.output("size", size);
    let grow = curve(&mut b, &[ease_out(0.0, 0.7), ease(1.0, 3.5)]);
    b.output("size_over_life", grow);
    b.output("color", col([0.55, 0.53, 0.5, 0.8]));
    let fade = fade_in_out(&mut b, 0.08, 0.45);
    b.output("color_over_life", fade);
    random_spin(&mut b, 0.25);
    let swirl = curl(&mut b, 0.35, 0.35, [0.0, 0.3, 0.0]);
    let w = wind(&mut b, 0.8, 0.0);
    accelerate(&mut b, vec![swirl, w]);
    let look = RenderSettings {
        texture: tex("smoke_sheet.png"),
        sprite_sheet: SpriteSheet { columns: 4, rows: 4, mode: SheetMode::OverLifetime },
        frame_blend: true,
        blend: BlendMode::Alpha,
        soft_distance: 0.5,
        lighting: Lighting { lit: true, roughness: 1.0, metallic: 0.0 },
        shadows: ParticleShadows::On,
        alpha_cutoff: 0.35,
        draw_order: DrawOrder::OldestFirst,
        ..Default::default()
    };
    effect("Lit Smoke Stack", 0.0, true, vec![emitter("smoke", 128, World, 851, b.finish(), look)])
}

// ── The library ─────────────────────────────────────────────────────────────

struct Entry {
    category: &'static str,
    name: &'static str,
    about: &'static str,
    build: fn() -> VfxAsset,
}

const fn entry(category: &'static str, name: &'static str, about: &'static str, build: fn() -> VfxAsset) -> Entry {
    Entry { category, name, about, build }
}

fn library() -> Vec<Entry> {
    let mut all = vec![
        entry("fire", "campfire", "A 0.6 m log campfire: flame core, outer flames, smoke, bouncing embers and a flickering light. Origin on the ground.", campfire),
        entry("fire", "torch", "A hand torch: origin at the top of the shaft, flame licking 0.25 m up, a 4 m light.", torch),
        entry("fire", "candle", "A candle flame: origin at the wick tip, a steady 4 cm flame and a small warm light.", candle),
        entry("fire", "fireplace", "A 1 m hearth fire: wide, low, many tongues over glowing coals, a warm light. Indoors, ignores wind.", fireplace),
        entry("fire", "brazier", "A bowl fire 0.5 m across: origin at the rim, sparks rising, every layer wind-coupled.", brazier),
        entry("smoke", "chimney_smoke", "A slow chimney plume, origin at the pot; lies over in any wind.", chimney_smoke),
        entry("smoke", "ground_fog", "Low fog over 12 x 12 m, slowly drifting, soft against the ground.", ground_fog),
        entry("smoke", "steam_vent", "A fast white steam jet that brakes, swells and dissipates.", steam_vent),
        entry("smoke", "dust_cloud", "A one-shot cloud of dust and grit that settles. Origin on the ground.", dust_cloud),
        entry("water", "fountain", "A fountain jet arcing under gravity from 0.6 m, splashing where it lands.", fountain),
        entry("water", "waterfall_mist", "Spray mist at a waterfall's foot, 4 m wide, riding the wind.", waterfall_mist),
        entry("water", "splash", "A one-shot droplet crown and column. Origin on the water surface.", splash),
        entry("weather", "rain", "Rain over 16 x 16 m with ground splashes; parent it under the camera rig.", rain),
        entry("weather", "snow", "Snow over 16 x 16 m, fluttering and drifting with the wind.", snow),
        entry("weather", "falling_leaves", "Autumn leaves tumbling down from a 6 m canopy, drifting on the wind.", falling_leaves),
        entry("ambient", "dust_motes", "Faint dust specks hanging in a room's air (4 x 2.5 x 4 m).", dust_motes),
        entry("ambient", "fireflies", "Blinking, wandering fireflies over a 6 x 6 m meadow; the 3 brightest cast light.", fireflies),
        entry("impacts", "dust_puff", "A one-shot footstep or landing puff. Origin under the foot.", dust_puff),
        entry("impacts", "debris_burst", "A one-shot burst of stone chunks that bounce and slide to a stop.", debris_burst),
        entry("test", "burst", "Feature test: repeated bursts, no spawn rate.", test_burst),
        entry("test", "rate", "Feature test: a steady spawn rate only.", test_rate),
        entry("test", "wind", "Feature test: the Wind Force node at coupling 1.", test_wind),
        entry("test", "collision", "Feature test: a collide plane and a collide sphere.", test_collision),
        entry("test", "ribbon_trail", "Feature test: Ribbon facing, Trail kind.", test_ribbon_trail),
        entry("test", "ribbon_connect", "Feature test: Ribbon facing, Connect kind.", test_ribbon_connect),
        entry("test", "sub_emitter", "Feature test: OnDeath and Interval sub-emitters.", test_sub_emitter),
        entry("test", "light_per_emitter", "Feature test: a PerEmitter light.", test_light_per_emitter),
        entry("test", "light_brightest", "Feature test: a BrightestN(4) light.", test_light_brightest),
        entry("test", "keyed_tracks", "Feature test: a keyed spawn rate, burst keys and an Enabled toggle.", test_keyed_tracks),
        entry("test", "world_space", "Feature test: World space; compare with local_space while moving.", || test_space(World)),
        entry("test", "local_space", "Feature test: Local space; compare with world_space while moving.", || test_space(Local)),
        entry("test", "two_d_basic", "Feature test: a 2D effect, in pixels.", test_two_d_basic),
        entry("test", "two_d_gravity", "Feature test: a 2D effect under gravity.", test_two_d_gravity),
        entry("showcase", "meteor_impact", "A meteor every 4 s: flame trail, molten debris that cools (material), spark ribbons with a light flash, lit flipbook dust.", meteor_impact),
        entry("showcase", "leaf_whirlwind", "SVG leaves as lit, shadowed, tumbling alpha-mask quads on an Orbit vortex, with pixel-sized SVG glints. Leaves run on the GPU.", leaf_whirlwind),
        entry("showcase", "arcane_portal", "A standing portal facing +Z, centre 1.6 m up: SVG sigils, tube-trail motes (material), a conform-to-sphere core, gravity-well infall, SVG flipbook glyphs. Script: vfx_set_param(\"intensity\", 120).", arcane_portal),
        entry("showcase", "crystal_geyser", "A GPU geyser of crystal shards (material with per-particle hue) that shatter into SVG glints through a GPU sub-emitter.", crystal_geyser),
        entry("showcase", "soap_bubbles", "Fresnel soap-film bubbles (material) sorted by depth, popping into droplets when they touch the level (Collide Scene, Play mode).", soap_bubbles),
        entry("showcase", "lit_smoke_stack", "A smoke stack from a PNG flipbook: lit, casting shadows, oldest first, leaning in the wind.", lit_smoke_stack),
    ];
    all.extend(popular::entries());
    all.extend(stress::entries());
    all
}

// ── Validation ──────────────────────────────────────────────────────────────

const STEP: f32 = 1.0 / 60.0;
const STEPS: usize = 180;

/// What one simulated run saw.
struct Run {
    /// Peak live particles per emitter, by asset index.
    peak: Vec<usize>,
    /// Brightest total light seen in the last second, lumens, by asset index.
    light: Vec<f32>,
    /// Mean x velocity per emitter at the end (None when it has no particles).
    mean_vx: Vec<Option<f32>>,
    dropped: u64,
}

fn simulate(asset: &VfxAsset, frame: &FrameInput) -> Result<Run, String> {
    let (mut sim, failures) = EffectSim::new(asset);
    if let Some(fail) = failures.first() {
        return Err(format!("emitter '{}' does not compile: {:?}", fail.name, fail.errors));
    }
    if let Some(issue) = sim.sub_emitter_issues.first() {
        return Err(format!("sub-emitter refused: {issue}"));
    }
    let n = asset.emitters.len();
    let mut run = Run { peak: vec![0; n], light: vec![0.0; n], mean_vx: vec![None; n], dropped: 0 };
    let mut samples: Vec<LightSample> = Vec::new();
    for step in 0..STEPS {
        sim.step(STEP, frame);
        for e in &sim.emitters {
            let p = &e.particles;
            let name = &asset.emitters[e.emitter_index].name;
            if p.len() > e.capacity as usize {
                return Err(format!("'{name}' holds {} particles over its capacity {}", p.len(), e.capacity));
            }
            let finite = p.position.iter().all(|v| v.is_finite())
                && p.velocity.iter().all(|v| v.is_finite())
                && p.size.iter().all(|v| v.is_finite())
                && p.color.iter().all(|v| v.is_finite())
                && p.rotation.iter().all(|v| v.is_finite());
            if !finite {
                return Err(format!("'{name}' produced a non-finite value at step {step}"));
            }
            let slot = e.emitter_index;
            run.peak[slot] = run.peak[slot].max(p.len());
            let light = &asset.emitters[slot].light;
            if light.mode != LightMode::Off && step >= STEPS - 60 {
                aggregate(p, light, Affine3A::IDENTITY, &mut samples);
                let total: f32 = samples.iter().map(|s| s.intensity).sum();
                if !total.is_finite() {
                    return Err(format!("'{name}' light is not finite"));
                }
                run.light[slot] = run.light[slot].max(total);
            }
        }
    }
    for e in &sim.emitters {
        let p = &e.particles;
        if !p.is_empty() {
            run.mean_vx[e.emitter_index] = Some(p.velocity.iter().map(|v| v.x).sum::<f32>() / p.len() as f32);
        }
    }
    run.dropped = sim.dropped();
    Ok(run)
}

/// Everything an effect must pass before it is written. Returns a one-line
/// summary for the report.
fn validate(asset: &VfxAsset) -> Result<String, String> {
    let calm = simulate(asset, &FrameInput::default())?;
    let breeze = FrameInput { wind: Vec3::new(3.0, 0.0, 0.0), wind_turbulence: 0.5, ..Default::default() };
    let windy = simulate(asset, &breeze)?;
    for (i, e) in asset.emitters.iter().enumerate() {
        if calm.peak[i] == 0 {
            return Err(format!("'{}' never had a live particle in {} s", e.name, STEPS as f32 * STEP));
        }
        if e.light.mode != LightMode::Off && calm.light[i] <= 0.0 {
            return Err(format!("'{}' has a light that never lit", e.name));
        }
        // A wind-coupled emitter must lean downwind in a breeze.
        let coupled = e.graph.nodes.iter().any(|n| n.node_type == "force/wind");
        if coupled {
            if let (Some(c), Some(w)) = (calm.mean_vx[i], windy.mean_vx[i]) {
                if w <= c {
                    return Err(format!("'{}' ignores the wind (mean vx {c:.3} calm, {w:.3} windy)", e.name));
                }
            }
        }
    }
    if calm.dropped > 0 || windy.dropped > 0 {
        return Err(format!("dropped {} spawns for lack of capacity", calm.dropped.max(windy.dropped)));
    }
    let text = write_vfx(asset).map_err(|e| format!("could not serialize: {e}"))?;
    let back = parse_vfx(&text).map_err(|e| format!("does not re-read: {e}"))?;
    if &back != asset {
        return Err("changes on a save and reload".to_string());
    }
    let mut parts: Vec<String> = asset
        .emitters
        .iter()
        .enumerate()
        .map(|(i, e)| format!("{} {}/{}", e.name, calm.peak[i], e.capacity))
        .collect();
    for (i, e) in asset.emitters.iter().enumerate() {
        if e.light.mode != LightMode::Off {
            parts.push(format!("light {:.0} lm", calm.light[i]));
        }
    }
    Ok(parts.join(", "))
}

fn main() {
    const USAGE: &str = "usage: make_effect_library <out_dir> [--force] [--only <category>]";
    let mut out_dir: Option<PathBuf> = None;
    let mut force = false;
    // One category alone, so adding a set to a project does not recreate
    // library files its owner deleted on purpose.
    let mut only: Option<String> = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--force" {
            force = true;
        } else if arg == "--only" {
            let Some(category) = args.next() else {
                eprintln!("{USAGE}");
                std::process::exit(2);
            };
            only = Some(category.to_string_lossy().into_owned());
        } else if out_dir.is_none() {
            out_dir = Some(PathBuf::from(arg));
        } else {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
    let Some(out_dir) = out_dir else {
        eprintln!("{USAGE}");
        std::process::exit(2);
    };
    if !out_dir.is_dir() {
        eprintln!("{} is not a directory", out_dir.display());
        std::process::exit(2);
    }

    // Build and validate everything first: nothing is written unless the
    // whole library passes.
    let mut built = Vec::new();
    let mut failed = 0usize;
    for e in library().into_iter().filter(|e| only.as_deref().is_none_or(|c| c == e.category)) {
        let asset = (e.build)();
        match validate(&asset) {
            Ok(summary) => {
                println!("ok       {}/{}  ({summary})", e.category, e.name);
                built.push((e, asset));
            }
            Err(why) => {
                println!("FAILED   {}/{}: {why}", e.category, e.name);
                failed += 1;
            }
        }
    }
    if failed > 0 {
        eprintln!("\n{failed} effect(s) failed validation; nothing was written");
        std::process::exit(1);
    }

    println!();
    let (mut written, mut skipped) = (0usize, 0usize);
    for (e, asset) in &built {
        let dir = out_dir.join(e.category);
        let path = dir.join(format!("{}.vfx", e.name));
        if path.exists() && !force {
            println!("skipped  {}/{}.vfx (exists; pass --force to overwrite)", e.category, e.name);
            skipped += 1;
            continue;
        }
        let body = match write_vfx(asset) {
            Ok(t) => t,
            Err(err) => {
                eprintln!("could not serialize {}: {err}", e.name);
                std::process::exit(1);
            }
        };
        let text = format!("// {}: {}\n// Generated by make_effect_library.\n{body}\n", asset.name, e.about);
        if let Err(err) = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, text)) {
            eprintln!("could not write {}: {err}", path.display());
            std::process::exit(1);
        }
        println!("wrote    {}", path.display());
        written += 1;
    }
    println!("\n{} effect(s): {written} written, {skipped} skipped, all validated", built.len());
}
