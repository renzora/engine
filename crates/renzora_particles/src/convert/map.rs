//! Field by field: a `HanabiEffectDefinition` as an emitter graph.
//!
//! The rule throughout is **what the old effect did on screen**, read off
//! `renzora_hanabi::builder` (the code that turned a definition into a Hanabi
//! effect, removed with Hanabi itself; the behaviour it had is recorded in the
//! comments here), not what the field names suggest. Where the two disagree the
//! builder wins, and the comment says so: a directional spread was a box of
//! `speed * sin(spread)` around the direction, not a cone, and the converted
//! velocity is that box exactly, so the effect looks the same after.
//!
//! Three places deliberately follow the author's intent over the old runtime,
//! because the old runtime silently ignored a field the editor let you set:
//! `blend_mode: Additive` (the builder only read `alpha_mode`), `texture_path`
//! and `billboard_mode` (never read at all), and a random start size (the old
//! size-over-lifetime overwrote it every frame). Converting those as "nothing"
//! would carry the old bug into a system that can finally honour them.
//!
//! The orbit is the one place the old runtime did something it should not
//! have: its modifier kept adding spin and never took any away, so particles
//! wound up for as long as they lived. The field is an angular speed, and the
//! converted Orbit holds it (see `forces`).
//!
//! Anything with no node yet is not guessed at: it becomes a [`Warning`]
//! naming the feature, so a conversion never loses something without saying.

use super::format::{
    BillboardMode, BlendMode as HBlend, HanabiEffectDefinition, HanabiEmitShape, KillZone, MotionIntegrationMode,
    ParticleAlphaMode, ParticleColorBlendMode, ParticleOrientMode, ShapeDimension, SimulationCondition,
    SimulationSpace as HSpace, SpawnMode, VelocityMode,
};
use crate::asset::{
    BlendMode, Dimension, Emitter, Facing, LightMode, ParticleLight, PinValue, RenderSettings, Ribbon, RibbonKind,
    SheetMode, SimulationSpace, SpriteSheet, VfxAsset, VFX_VERSION,
};
use crate::curve::{CurvePoint, Ease, GradientStop};

use super::build::{GraphBuilder, Src};
use super::{Conversion, Warning};

/// Convert one definition. Never fails: every field either maps or warns.
pub fn convert_definition(def: &HanabiEffectDefinition) -> Conversion {
    let mut cx = Cx {
        def,
        g: GraphBuilder::default(),
        warnings: Vec::new(),
        two_d: def.plane_2d,
    };
    let (duration, looping) = cx.spawn();
    cx.lifetime();
    let shape = cx.shape();
    cx.g.output("position", shape.position.clone());
    cx.velocity(&shape);
    cx.forces();
    cx.size();
    cx.color();
    if def.rotation_speed.abs() > 1e-4 {
        // The builder's rotation was `age * rotation_speed`: a constant spin.
        cx.g.output("angular_velocity", Src::float(def.rotation_speed));
    }
    let render = cx.render();
    let light = cx.light();
    cx.everything_else();

    let Cx { g, warnings, .. } = cx;
    let emitter = Emitter {
        name: "main".to_string(),
        enabled: true,
        capacity: def.capacity.max(1),
        space: match def.simulation_space {
            HSpace::Local => SimulationSpace::Local,
            HSpace::World => SimulationSpace::World,
        },
        seed: 1,
        graph: g.finish(),
        render,
        light,
        sub_emitters: Vec::new(),
        // Hanabi ran everything on the GPU; Auto keeps a large converted
        // effect there and a small one on the CPU, which plays everywhere.
        backend: Default::default(),
        // The old format had no inherited velocity; a graph that wants one
        // adds Emitter Velocity, as before.
        inherit_velocity: 0.0,
    };
    Conversion {
        asset: VfxAsset {
            version: VFX_VERSION,
            name: def.name.clone(),
            dimension: if def.plane_2d { Dimension::TwoD } else { Dimension::ThreeD },
            duration,
            looping,
            emitters: vec![emitter],
            tracks: Vec::new(),
        },
        warnings,
    }
}

struct Cx<'a> {
    def: &'a HanabiEffectDefinition,
    g: GraphBuilder,
    warnings: Vec<Warning>,
    two_d: bool,
}

/// What the emission shape produced: where a particle starts, and the
/// "outward" direction a radial velocity pushes it along.
struct Shape {
    position: Src,
    direction: Src,
    /// A box, whose outward direction has depth even in a 2D effect.
    is_box: bool,
}

/// A Conform To Sphere node's inputs, one per field of Hanabi's modifier.
struct Conform {
    center: [f32; 3],
    radius: f32,
    influence: f32,
    attraction: f32,
    max_speed: f32,
    sticky: f32,
    shell: f32,
}

/// `[x, y, z] * s`.
fn scale(v: [f32; 3], s: f32) -> [f32; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

fn normalize_or_zero(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len > 1e-6 && len.is_finite() {
        scale(v, 1.0 / len)
    } else {
        [0.0; 3]
    }
}

/// The same blackbody approximation the old builder used (Tanner Helland's),
/// copied because the builder's is private and the gradient must match it.
fn kelvin_to_rgb(kelvin: f32) -> [f32; 3] {
    let t = (kelvin / 100.0).clamp(10.0, 400.0);
    let r = if t <= 66.0 {
        255.0
    } else {
        (329.698_73 * (t - 60.0).powf(-0.133_204_76)).clamp(0.0, 255.0)
    };
    let g = if t <= 66.0 {
        (99.470_8 * t.ln() - 161.119_57).clamp(0.0, 255.0)
    } else {
        (288.122_17 * (t - 60.0).powf(-0.075_514_85)).clamp(0.0, 255.0)
    };
    let b = if t >= 66.0 {
        255.0
    } else if t <= 19.0 {
        0.0
    } else {
        (138.517_73 * (t - 10.0).ln() - 305.044_8).clamp(0.0, 255.0)
    };
    [r / 255.0, g / 255.0, b / 255.0]
}

impl Cx<'_> {
    fn warn(&mut self, feature: &str, detail: impl Into<String>) {
        self.warnings.push(Warning {
            feature: feature.to_string(),
            detail: detail.into(),
        });
    }

    // ── Spawn ───────────────────────────────────────────────────────────────

    /// The output's spawn pins, and the effect's `(duration, looping)`.
    ///
    /// The old spawner had a duration and a cycle count where the new effect
    /// has one duration and a loop flag, so each mode is mapped by what it
    /// emitted over time rather than field for field.
    fn spawn(&mut self) -> (f32, bool) {
        let def = self.def;
        let cycles = def.spawn_cycle_count;
        match def.spawn_mode {
            SpawnMode::Rate => {
                self.g.output("spawn_rate", Src::float(def.spawn_rate.max(0.0)));
                // `SpawnerSettings::new(rate, d, d, cycles)`: emit for `d`
                // seconds per cycle with no gap between cycles, `cycles`
                // times (0 meaning forever). With no duration the old builder
                // used f32::MAX, which is "forever" too.
                if def.spawn_duration > 0.0 && cycles > 0 {
                    (def.spawn_duration * cycles as f32, false)
                } else if def.spawn_duration > 0.0 {
                    (def.spawn_duration, true)
                } else {
                    (2.0, true)
                }
            }
            SpawnMode::Burst => {
                // One burst at the start. More than one cycle had a zero
                // period, so every cycle landed in the same instant: the same
                // as one burst of all of them.
                let count = def.spawn_count as f32 * cycles.max(1) as f32;
                self.g.output("spawn_rate", Src::float(0.0));
                self.g.output("burst_count", Src::float(count));
                self.g.output("burst_interval", Src::float(0.0));
                self.g.output("burst_cycles", Src::float(1.0));
                (def.lifetime_min.max(def.lifetime_max).max(0.5), false)
            }
            SpawnMode::BurstRate => {
                // `spawn_rate` was bursts per second here.
                let period = if def.spawn_rate > 0.0 { 1.0 / def.spawn_rate } else { 1.0 };
                self.g.output("spawn_rate", Src::float(0.0));
                self.g.output("burst_count", Src::float(def.spawn_count as f32));
                self.g.output("burst_interval", Src::float(period));
                if cycles > 0 {
                    self.g.output("burst_cycles", Src::float(cycles as f32));
                    (period * cycles as f32, false)
                } else {
                    // Unlimited bursts. A loop of a whole number of periods
                    // near two seconds keeps the cadence across the loop
                    // seam and gives the editor's timeline a useful length.
                    self.g.output("burst_cycles", Src::float(0.0));
                    let n = (2.0 / period).round().max(1.0);
                    (n * period, true)
                }
            }
        }
    }

    fn lifetime(&mut self) {
        let (lo, hi) = (self.def.lifetime_min, self.def.lifetime_max);
        let src = self.g.random_range(lo, hi);
        self.g.output("lifetime", src);
    }

    // ── Shape ───────────────────────────────────────────────────────────────

    fn shape(&mut self) -> Shape {
        let def = self.def;
        match &def.emit_shape {
            HanabiEmitShape::Point => {
                if self.two_d {
                    // A zero circle: still at the origin, but its direction
                    // stays in the XY plane a 2D camera sees.
                    let id = self.g.node("shape/circle");
                    self.g.set(id, "radius", PinValue::Float(0.0));
                    self.node_shape(id)
                } else {
                    let id = self.g.node("shape/point");
                    self.node_shape(id)
                }
            }
            HanabiEmitShape::Circle { radius, dimension } => self.circle(*radius, *dimension),
            // The old builder flattened a sphere to a disc in 2D.
            HanabiEmitShape::Sphere { radius, dimension } if self.two_d => self.circle(*radius, *dimension),
            HanabiEmitShape::Sphere { radius, dimension } => {
                let id = self.g.node("shape/sphere");
                self.g.set(id, "radius", PinValue::Float(*radius));
                if *dimension == ShapeDimension::Surface {
                    self.g.set(id, "surface", PinValue::Bool(true));
                }
                self.node_shape(id)
            }
            HanabiEmitShape::Cone {
                base_radius,
                top_radius,
                height,
                dimension,
            } => self.cone(*base_radius, *top_radius, *height, *dimension),
            // The old builder ignored a rectangle's surface flag and filled it,
            // so the filled rectangle is what it looked like.
            HanabiEmitShape::Rect { half_extents, .. } => {
                let id = self.g.node("shape/rect");
                self.g.set(id, "size", PinValue::Vec2([half_extents[0] * 2.0, half_extents[1] * 2.0]));
                self.node_shape(id)
            }
            HanabiEmitShape::Box { half_extents } => {
                let id = self.g.node("shape/box");
                self.g.set(id, "size", PinValue::Vec3(scale(*half_extents, 2.0)));
                let mut s = self.node_shape(id);
                s.is_box = true;
                s
            }
        }
    }

    fn node_shape(&self, id: u64) -> Shape {
        Shape {
            position: Src::Pin(id, "position"),
            direction: Src::Pin(id, "direction"),
            is_box: false,
        }
    }

    /// A circle. In 2D it is the catalog's XY circle as is; in 3D the old one
    /// lay flat on the ground (axis Y), so the XY circle is turned into XZ.
    fn circle(&mut self, radius: f32, dimension: ShapeDimension) -> Shape {
        let id = self.g.node("shape/circle");
        self.g.set(id, "radius", PinValue::Float(radius));
        if dimension == ShapeDimension::Surface {
            self.g.set(id, "edge", PinValue::Bool(true));
        }
        if self.two_d {
            return self.node_shape(id);
        }
        let split = self.g.node("math/split_vec3");
        self.g.wire(id, "position", split, "value");
        let flat = self.g.node("math/compose_vec3");
        self.g.wire(split, "x", flat, "x");
        self.g.wire(split, "y", flat, "z");
        let position = Src::Pin(flat, "value");
        let direction = self.g.normalize(position.clone());
        Shape {
            position,
            direction,
            is_box: false,
        }
    }

    /// A truncated cone along +Y, `base` radius at the bottom and `top` at
    /// `height`, filled. Built as a disc scaled from one radius to the other by
    /// a random height, which matches the old volume closely; the old
    /// "surface" variant (the slanted wall only) has no node.
    fn cone(&mut self, base: f32, top: f32, height: f32, dimension: ShapeDimension) -> Shape {
        if dimension == ShapeDimension::Surface {
            self.warn("cone surface", "the cone now emits from its whole volume, not only its wall");
        }
        let r = base.max(top).max(0.0);
        let id = self.g.node("shape/cone");
        self.g.set(id, "radius", PinValue::Float(r));
        let t = self.g.random_range(0.0, 1.0);
        let mut position = Src::Pin(id, "position");
        if r > 0.0 && (base - top).abs() > 1e-4 {
            let s = self.g.op(
                "math/lerp",
                &[("a", Src::float(base / r)), ("b", Src::float(top / r)), ("t", t.clone())],
                "value",
            );
            position = self.g.mul(position, s);
        }
        if height > 0.0 {
            let h = self.g.mul(t, Src::float(height));
            let up = self.g.op("math/compose_vec3", &[("y", h)], "value");
            position = self.g.add(position, up);
        }
        let direction = self.g.normalize(position.clone());
        Shape {
            position,
            direction,
            is_box: false,
        }
    }

    // ── Velocity ────────────────────────────────────────────────────────────

    /// The old builder's speed range: `speed_min..speed_max` when a max was
    /// set, otherwise exactly `velocity_magnitude`.
    fn speed_range(&self) -> (f32, f32) {
        let d = self.def;
        if d.velocity_speed_max > 0.0 {
            (d.velocity_speed_min.min(d.velocity_speed_max), d.velocity_speed_min.max(d.velocity_speed_max))
        } else {
            (d.velocity_magnitude, d.velocity_magnitude)
        }
    }

    fn velocity(&mut self, shape: &Shape) {
        let def = self.def;
        let (lo, hi) = self.speed_range();
        match def.velocity_mode {
            VelocityMode::Directional => {
                let dir = normalize_or_zero(def.velocity_direction);
                if def.velocity_spread > 0.001 {
                    // Not a cone: the old spread was a box of half-size
                    // `avg_speed * sin(spread)` around the direction, per
                    // component, kept in-plane in 2D. Reproduced exactly.
                    let spread = (lo + hi) * 0.5 * def.velocity_spread.sin();
                    let s = if self.two_d { [spread, spread, 0.0] } else { [spread; 3] };
                    let min = scale(dir, lo);
                    let max = scale(dir, hi);
                    let min = [min[0] - s[0], min[1] - s[1], min[2] - s[2]];
                    let max = [max[0] + s[0], max[1] + s[1], max[2] + s[2]];
                    let v = self.g.op("random/vec3_range", &[("min", Src::vec3(min)), ("max", Src::vec3(max))], "value");
                    self.g.output("velocity", v);
                } else if (lo - hi).abs() > 1e-6 {
                    let v = self.g.op(
                        "random/vec3_range",
                        &[("min", Src::vec3(scale(dir, lo))), ("max", Src::vec3(scale(dir, hi)))],
                        "value",
                    );
                    self.g.output("velocity", v);
                } else if lo != 0.0 && dir != [0.0; 3] {
                    self.g.output("velocity", Src::vec3(scale(dir, lo)));
                }
            }
            // The builder treated both alike: outward from the emitter's
            // centre, through the particle's own start point. A point emitter
            // has no "outward", which the old GPU code turned into NaN; a
            // random direction is what such an effect was plainly meant to do.
            VelocityMode::Radial | VelocityMode::Random => {
                if hi <= 0.0 && lo <= 0.0 {
                    return;
                }
                let mut dir = shape.direction.clone();
                if self.two_d && shape.is_box {
                    // A box's outward direction has depth; 2D motion along Z
                    // is sort order, not movement.
                    let flat = self.g.mul(dir, Src::vec3([1.0, 1.0, 0.0]));
                    dir = self.g.normalize(flat);
                }
                let speed = self.g.random_range(lo, hi);
                let v = self.g.mul(dir, speed);
                self.g.output("velocity", v);
            }
            // Around `velocity_axis` through the origin, at the given speed:
            // the unit tangent `normalize(axis x position)`. The vortex node is
            // that cross product; it reads the particle's position, which does
            // not exist yet at birth, so the shape's start point is wired in.
            VelocityMode::Tangent => {
                if hi <= 0.0 && lo <= 0.0 {
                    return;
                }
                let swirl = self.g.op(
                    "force/vortex",
                    &[
                        ("position", shape.position.clone()),
                        ("axis", Src::vec3(def.velocity_axis)),
                        ("strength", Src::float(1.0)),
                    ],
                    "force",
                );
                let dir = self.g.normalize(swirl);
                let speed = self.g.random_range(lo, hi);
                let v = self.g.mul(dir, speed);
                self.g.output("velocity", v);
            }
        }
    }

    // ── Forces ──────────────────────────────────────────────────────────────

    fn forces(&mut self) {
        let def = self.def;
        if def.acceleration != [0.0; 3] {
            self.g.output("gravity", Src::vec3(def.acceleration));
        }
        if def.linear_drag > 0.0 {
            // The old drag was `v -= v * drag * dt`; the new one is its stable
            // implicit form, the same for any sensible step.
            self.g.output("drag", Src::float(def.linear_drag));
        }

        let mut forces: Vec<Src> = Vec::new();
        if def.radial_acceleration.abs() > 0.001 {
            // A constant push away from the origin is an attractor with a
            // negative strength and no falloff, exactly.
            forces.push(self.g.op(
                "force/attractor",
                &[("strength", Src::float(-def.radial_acceleration))],
                "force",
            ));
        }
        if def.tangent_acceleration.abs() > 0.001 {
            // Constant-magnitude spin: the vortex's tangent normalised, then
            // scaled (the vortex alone grows with distance from the axis).
            let swirl = self.g.op(
                "force/vortex",
                &[("axis", Src::vec3(def.tangent_accel_axis)), ("strength", Src::float(1.0))],
                "force",
            );
            let unit = self.g.normalize(swirl);
            forces.push(self.g.mul(unit, Src::float(def.tangent_acceleration)));
        }
        if let Some(c) = &def.conform_to_sphere {
            // The node is Hanabi's modifier line for line, so every field
            // carries over as it is.
            forces.push(self.conform(Conform {
                center: c.origin,
                radius: c.radius,
                influence: c.influence_dist,
                attraction: c.attraction_accel,
                max_speed: c.max_attraction_speed,
                sticky: c.sticky_factor,
                shell: c.shell_half_thickness,
            }));
        }
        let mut repelling = 0;
        for a in &def.attractors {
            if a.strength >= 0.0 {
                // The old builder made each attractor a conform-to-sphere
                // modifier on its core radius, with Hanabi's own shell and
                // sticky defaults: exactly a Conform To Sphere node.
                forces.push(self.conform(Conform {
                    center: a.position,
                    radius: a.radius.max(0.0),
                    influence: a.influence_dist,
                    attraction: a.strength,
                    max_speed: a.max_speed,
                    sticky: crate::nodes::CONFORM_STICKY,
                    shell: crate::nodes::CONFORM_SHELL,
                }));
            } else {
                // A negative acceleration in that modifier pushed particles
                // the wrong way round its clamp rather than away; what the
                // field promised ("negative = repel") is a push.
                forces.push(self.attractor(a.position, a.strength, a.influence_dist));
                repelling += 1;
            }
        }
        if repelling > 0 {
            self.warn(
                "repelling attractors",
                format!("{repelling} attractor(s) with a negative strength became an Attractor push; their core radius and max speed are not modelled"),
            );
        }
        if def.noise_amplitude > 0.001 && def.noise_frequency > 0.001 {
            // The old turbulence summed `octaves` octaves, each `lacunarity`
            // times the frequency and half the amplitude of the one before,
            // starting at amplitude 1, unnormalised: exactly the Curl Noise
            // node's octaves at its default persistence of 0.5. It also drifted
            // through the field at half a unit per second on every axis, which
            // is the node's scroll.
            let mut inputs = vec![
                ("strength", Src::float(def.noise_amplitude)),
                ("frequency", Src::float(def.noise_frequency)),
                ("scroll", Src::vec3([0.5, 0.5, 0.5])),
            ];
            let octaves = def.noise_octaves.min(crate::noise::MAX_OCTAVES as u32);
            if octaves != 1 {
                inputs.push(("octaves", Src::float(octaves as f32)));
            }
            if octaves > 1 && (def.noise_lacunarity - 2.0).abs() > 1e-6 && def.noise_lacunarity.is_finite() {
                inputs.push(("lacunarity", Src::float(def.noise_lacunarity)));
            }
            let mut noise = self.g.op("force/curl_noise", &inputs, "force");
            if self.two_d {
                // The old 2D noise was planar: no push along Z, which in 2D is
                // draw order rather than depth.
                noise = self.g.mul(noise, Src::vec3([1.0, 1.0, 0.0]));
            }
            forces.push(noise);
            if def.noise_octaves > octaves {
                self.warn(
                    "noise octaves",
                    format!("{} octaves capped at {octaves}, the most Curl Noise sums", def.noise_octaves),
                );
            }
        }
        if let Some(o) = &def.orbit {
            // The Orbit node, at the old angular speed, on the old ring when
            // the old one pulled toward it. Not its arithmetic: the old
            // modifier added `speed * r * dt` of tangential velocity every
            // step and never took any away, so a particle spun faster for as
            // long as it lived (and its undamped radial pull rang round the
            // ring for ever). The field says radians per second, and a steady
            // circle at that speed is what an orbit is.
            let axis = normalize_or_zero(o.axis);
            let mut inputs = vec![
                ("center", Src::vec3(o.center)),
                ("axis", Src::vec3(if axis == [0.0; 3] { [0.0, 1.0, 0.0] } else { axis })),
                ("speed", Src::float(o.speed)),
            ];
            if o.radial_pull > 0.0 && o.orbit_radius > 0.0 {
                inputs.push(("radius", Src::float(o.orbit_radius)));
                inputs.push(("radial_pull", Src::float(o.radial_pull)));
            }
            forces.push(self.g.op("force/orbit", &inputs, "force"));
        }
        if let Some(sum) = self.g.sum(forces) {
            self.g.output("acceleration", sum);
        }

        if def.velocity_limit > 0.001 {
            // The old limit clamped the speed after every update, which is
            // what the output's Max Speed does.
            self.g.output("max_speed", Src::float(def.velocity_limit));
        }
        self.kill_zones();
    }

    /// Each kill zone as a collision node that kills on contact. A sphere or
    /// box that killed what was inside it is a solid shape (a particle inside
    /// it is hit, and so killed, at once); one that killed what was outside
    /// it is the same shape with Inside set, which kills whatever leaves.
    fn kill_zones(&mut self) {
        let def = self.def;
        for zone in &def.kill_zones {
            let (node, center, inside) = match zone {
                KillZone::Sphere { center, radius, kill_inside } => {
                    let id = self.g.node("collide/sphere");
                    self.g.set(id, "radius", PinValue::Float(radius.abs()));
                    (id, *center, !kill_inside)
                }
                KillZone::Aabb { center, half_size, kill_inside } => {
                    let id = self.g.node("collide/box");
                    self.g.set(id, "size", PinValue::Vec3(scale(half_size.map(f32::abs), 2.0)));
                    (id, *center, !kill_inside)
                }
            };
            self.g.set(node, "center", PinValue::Vec3(center));
            self.g.set(node, "kill", PinValue::Bool(true));
            if inside {
                self.g.set(node, "inside", PinValue::Bool(true));
            }
            // The old zones were tested against the particle's position in
            // its simulation space, which for a world-space effect is the
            // world: a zone that stays put while the effect moves.
            if def.simulation_space == HSpace::World {
                self.g.set(node, "world", PinValue::Bool(true));
            }
        }
    }

    /// A Conform To Sphere node with every field set.
    fn conform(&mut self, c: Conform) -> Src {
        self.g.op(
            "force/conform_sphere",
            &[
                ("center", Src::vec3(c.center)),
                ("radius", Src::float(c.radius)),
                ("influence", Src::float(c.influence)),
                ("attraction", Src::float(c.attraction)),
                ("max_speed", Src::float(c.max_speed)),
                ("sticky", Src::float(c.sticky)),
                ("shell", Src::float(c.shell)),
            ],
            "force",
        )
    }

    /// A pull toward `point` that halves at `influence` distance, which is the
    /// nearest the attractor node's inverse-square falloff comes to the old
    /// "influence distance".
    fn attractor(&mut self, point: [f32; 3], strength: f32, influence: f32) -> Src {
        let falloff = if influence > 1e-4 { 1.0 / (influence * influence) } else { 0.0 };
        self.g.op(
            "force/attractor",
            &[
                ("point", Src::vec3(point)),
                ("strength", Src::float(strength)),
                ("falloff", Src::float(falloff)),
            ],
            "force",
        )
    }

    // ── Size ────────────────────────────────────────────────────────────────

    /// The old size-over-lifetime was the absolute size (it overwrote the start
    /// size), so it becomes `size_over_life` with `size` 1. A random start size
    /// range, which the old runtime overwrote and so never showed, becomes a
    /// random multiplier on that curve, scaled so the start lands in the range.
    fn size(&mut self) {
        let def = self.def;
        // `screen_space_size` maps onto the render setting of the same name
        // (see `render`), in the same physical pixels Hanabi used.
        let random = def.size_start_min > 0.0
            && def.size_start_max > 0.0
            && (def.size_start_min - def.size_start_max).abs() > 1e-4;
        let (rlo, rhi) = (def.size_start_min.min(def.size_start_max), def.size_start_min.max(def.size_start_max));

        // `start` is the size at birth, for scaling the random range.
        let (curve, start): (Option<Src>, f32) = if !def.size_curve.is_empty() {
            let mut points: Vec<CurvePoint> = def
                .size_curve
                .iter()
                .map(|p| CurvePoint {
                    t: p.time.clamp(0.0, 1.0),
                    value: p.value.max(0.0),
                    ease: Ease::Linear,
                })
                .collect();
            points.sort_by(|a, b| a.t.total_cmp(&b.t));
            let start = points.first().map_or(0.0, |p| p.value);
            let peak = points.iter().map(|p| p.value).fold(0.0, f32::max);
            let id = self.g.node("curve/curve");
            self.g.set(id, "points", PinValue::Curve(points));
            (Some(Src::Pin(id, "value")), if start > 0.0 { start } else { peak })
        } else if def.size_non_uniform {
            let life = self.g.op("input/life", &[], "value");
            let lerp = self.g.op(
                "math/lerp",
                &[
                    ("a", Src::Value(PinValue::Vec2([def.size_start_x, def.size_start_y]))),
                    ("b", Src::Value(PinValue::Vec2([def.size_end_x, def.size_end_y]))),
                    ("t", life),
                ],
                "value",
            );
            (Some(lerp), def.size_start_x.max(def.size_start_y))
        } else if (def.size_start - def.size_end).abs() > 1e-6 {
            let id = self.g.node("curve/curve");
            self.g.set(
                id,
                "points",
                PinValue::Curve(vec![
                    CurvePoint { t: 0.0, value: def.size_start, ease: Ease::Linear },
                    CurvePoint { t: 1.0, value: def.size_end, ease: Ease::Linear },
                ]),
            );
            (Some(Src::Pin(id, "value")), def.size_start.max(def.size_end))
        } else {
            (None, def.size_start)
        };

        match curve {
            Some(curve) => {
                self.g.output("size_over_life", curve);
                let size = if random && start > 0.0 {
                    self.g.random_range(rlo / start, rhi / start)
                } else {
                    Src::float(1.0)
                };
                self.g.output("size", size);
            }
            None => {
                let size = if random { self.g.random_range(rlo, rhi) } else { Src::float(def.size_start) };
                self.g.output("size", size);
            }
        }
    }

    // ── Colour ──────────────────────────────────────────────────────────────

    fn color(&mut self) {
        let def = self.def;
        let hdr = if def.use_hdr_color { def.hdr_intensity.max(1.0) } else { 1.0 };
        if def.color_blend_mode == ParticleColorBlendMode::Add {
            self.warn("colour blend Add", "the colour now multiplies (Modulate) instead of adding");
        }
        // The old HDR intensity was one multiplier on top of the colour, which
        // is what the output's Emission pin is: the gradient keeps the stops
        // the old editor showed, and the glow stays one number to turn.
        if hdr != 1.0 {
            self.g.output("emission", Src::float(hdr));
        }
        if def.use_flat_color {
            self.g.output("color", Src::Value(PinValue::Color(def.flat_color)));
            return;
        }
        let stops: Vec<GradientStop> = if let Some([k0, k1]) = def.blackbody {
            // The builder's ramp: seven temperatures from start to end, alpha
            // easing out as `1 - f^2`.
            (0..=6)
                .map(|i| {
                    let f = i as f32 / 6.0;
                    let [r, g, b] = kelvin_to_rgb(k0 + (k1 - k0) * f);
                    GradientStop { t: f, color: [r, g, b, (1.0 - f * f).clamp(0.0, 1.0)] }
                })
                .collect()
        } else {
            let mut s: Vec<GradientStop> = def
                .color_gradient
                .iter()
                .map(|s| GradientStop { t: s.position, color: s.color })
                .collect();
            s.sort_by(|a, b| a.t.total_cmp(&b.t));
            s
        };
        if stops.is_empty() {
            return;
        }
        let id = self.g.node("curve/gradient");
        self.g.set(id, "stops", PinValue::Gradient(stops));
        self.g.output("color_over_life", Src::Pin(id, "color"));
    }

    // ── Render ──────────────────────────────────────────────────────────────

    fn render(&mut self) -> RenderSettings {
        let def = self.def;
        let texture = def.texture_path.clone().filter(|t| !t.trim().is_empty());
        let additive_intent = def.blend_mode == HBlend::Additive;
        let blend = match def.alpha_mode {
            ParticleAlphaMode::Add => BlendMode::Additive,
            ParticleAlphaMode::Premultiply => BlendMode::Premultiplied,
            ParticleAlphaMode::Blend if additive_intent => BlendMode::Additive,
            ParticleAlphaMode::Blend => BlendMode::Alpha,
            // Hanabi's mask and opaque modes are these two exactly: depth
            // written, in the opaque phases, the mask cut at its threshold.
            ParticleAlphaMode::Mask => BlendMode::AlphaMask,
            ParticleAlphaMode::Opaque => BlendMode::Opaque,
            // Hanabi's multiply is Bevy's, which is the particles' own now.
            ParticleAlphaMode::Multiply => BlendMode::Multiply,
        };
        // The old blend mode said the same thing a second way; it only
        // matters where the alpha mode left plain alpha blending.
        let blend = if def.blend_mode == HBlend::Multiply && blend == BlendMode::Alpha { BlendMode::Multiply } else { blend };

        let facing = if def.orient_mode == ParticleOrientMode::AlongVelocity {
            Facing::VelocityAligned
        } else {
            match def.billboard_mode {
                BillboardMode::FaceCamera => Facing::CameraBillboard,
                BillboardMode::FaceCameraY => Facing::FixedAxis([0.0, 1.0, 0.0]),
                BillboardMode::Velocity => Facing::VelocityAligned,
                BillboardMode::Fixed => {
                    self.warn("fixed billboard", "particles now face the camera");
                    Facing::CameraBillboard
                }
            }
        };

        let sprite_sheet = match (&def.flipbook, &texture) {
            (Some(fb), Some(_)) => SpriteSheet {
                columns: fb.grid_columns.max(1),
                rows: fb.grid_rows.max(1),
                mode: SheetMode::OverLifetime,
            },
            (Some(_), None) => {
                self.warn("flipbook", "dropped: the effect has no texture to slice into frames");
                SpriteSheet::default()
            }
            _ => SpriteSheet::default(),
        };
        if def.render_layer != 0 {
            self.warn("render layer", format!("layer {} dropped; particles draw on the default layer", def.render_layer));
        }
        // Only worth a warning with a texture. Without one the old runtime drew
        // its soft round sprite, which already faded out before the corners
        // roundness would have cut, and the new untextured dot is the same
        // shape: nothing visible is lost, and 200-odd shipped effects would
        // otherwise all carry a warning that means nothing.
        if def.roundness > 0.001 && texture.is_some() {
            self.warn("roundness", "dropped: the texture is drawn as a full quad");
        }
        // The old ribbon linked the particles into strips in age order, the
        // width from each particle's size and the colour from its own, with no
        // taper: the Connect ribbon with full width and alpha to the tail.
        let (facing, ribbon) = match def.ribbon {
            Some(r) => (
                Facing::Ribbon,
                Ribbon { kind: RibbonKind::Connect, groups: r.groups.max(1), tail_width: 1.0, tail_alpha: 1.0, ..Ribbon::default() },
            ),
            None => (facing, Ribbon::default()),
        };
        RenderSettings {
            texture,
            sprite_sheet,
            blend,
            facing,
            ribbon,
            z_order: 0.0,
            // The old erosion was an on/off noise dissolve as a particle
            // faded; this is the same idea with a strength, and 0.6 eats
            // about as much as the old one did by the end of a life.
            erosion: if def.erosion { 0.6 } else { 0.0 },
            alpha_cutoff: if def.alpha_mask_threshold.is_finite() {
                def.alpha_mask_threshold.clamp(0.0, 1.0)
            } else {
                crate::asset::DEFAULT_ALPHA_CUTOFF
            },
            screen_space_size: def.screen_space_size,
            ..Default::default()
        }
    }

    /// The effect's point light as the emitter's per-emitter light.
    ///
    /// The old light was one fixed brightness at the effect; the new one sums
    /// its live particles, each weighted by how bright it is. So the old
    /// lumens are divided by how much particle the emitter holds on average
    /// (the particles alive in its steady state times their average weight
    /// over a life), which gives the old brightness while the effect is in
    /// full flow and dims it as the effect dies down. The old flicker swung
    /// both ways around the base; the new one only dims, so the base is raised
    /// to keep the same average.
    fn light(&self) -> ParticleLight {
        let def = self.def;
        let Some(l) = &def.light else { return ParticleLight::default() };
        let flicker = l.flicker.clamp(0.0, 1.0);
        let presence = (self.steady_count() * self.mean_weight()).max(1e-3);
        ParticleLight {
            mode: LightMode::PerEmitter,
            color: l.color,
            intensity: l.intensity.max(0.0) / presence / (1.0 - 0.5 * flicker),
            range: l.range.max(0.0),
            shadows: l.shadows,
            flicker,
            // The old two sines ran at about 1.8 and 3.7 Hz.
            flicker_rate: 3.0,
            ..ParticleLight::default()
        }
    }

    /// Particles alive once the spawner has run long enough, capped by the
    /// capacity.
    fn steady_count(&self) -> f32 {
        let d = self.def;
        let life = 0.5 * (d.lifetime_min + d.lifetime_max).max(0.0);
        let n = match d.spawn_mode {
            SpawnMode::Rate => d.spawn_rate.max(0.0) * life,
            SpawnMode::Burst => (d.spawn_count * d.spawn_cycle_count.max(1)) as f32,
            SpawnMode::BurstRate => d.spawn_count as f32 * (d.spawn_rate.max(0.0) * life).max(1.0),
        };
        n.clamp(1.0, d.capacity.max(1) as f32)
    }

    /// A particle's light weight ([`crate::light::weight`]),
    /// averaged over its life from the colour the conversion gives it.
    fn mean_weight(&self) -> f32 {
        let d = self.def;
        let hdr = if d.use_hdr_color { d.hdr_intensity.max(1.0) } else { 1.0 };
        let at = |t: f32| -> [f32; 4] {
            if d.use_flat_color || (d.color_gradient.is_empty() && d.blackbody.is_none()) {
                return d.flat_color;
            }
            if let Some([k0, k1]) = d.blackbody {
                let [r, g, b] = kelvin_to_rgb(k0 + (k1 - k0) * t);
                return [r, g, b, (1.0 - t * t).clamp(0.0, 1.0)];
            }
            let mut stops: Vec<&super::format::GradientStop> = d.color_gradient.iter().collect();
            stops.sort_by(|a, b| a.position.total_cmp(&b.position));
            let after = stops.iter().position(|s| s.position >= t).unwrap_or(stops.len() - 1);
            let b = stops[after];
            let a = stops[after.saturating_sub(1)];
            let span = b.position - a.position;
            let f = if span > 1e-6 { ((t - a.position) / span).clamp(0.0, 1.0) } else { 1.0 };
            std::array::from_fn(|i| a.color[i] + (b.color[i] - a.color[i]) * f)
        };
        const SAMPLES: usize = 16;
        let sum: f32 = (0..SAMPLES)
            .map(|k| {
                let c = at((k as f32 + 0.5) / SAMPLES as f32);
                crate::light::weight(bevy::math::Vec4::new(c[0] * hdr, c[1] * hdr, c[2] * hdr, c[3]))
            })
            .sum();
        sum / SAMPLES as f32
    }

    /// The fields that shape nothing the graph can say.
    fn everything_else(&mut self) {
        let def = self.def;
        if def.simulation_condition == SimulationCondition::WhenVisible {
            self.warn("simulate when visible", "the effect now simulates whether or not it is on screen");
        }
        if def.motion_integration != MotionIntegrationMode::PostUpdate {
            self.warn(
                "motion integration",
                format!("{:?} dropped; particles always move by their velocity", def.motion_integration),
            );
        }
        if !def.spawn_starts_active {
            self.warn("starts inactive", "the effect now plays on spawn; pause it from a script if needed");
        }
        if !def.variables.is_empty() {
            let mut names: Vec<&str> = def.variables.keys().map(String::as_str).collect();
            names.sort_unstable();
            self.warn("variables", format!("not supported yet: {}", names.join(", ")));
        }
    }
}
