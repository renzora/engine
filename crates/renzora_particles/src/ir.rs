//! The backend-neutral intermediate representation a graph compiles to.
//!
//! A [`Program`] is a list of instructions in SSA form: instruction `i` writes
//! register `i`, reads only lower registers, and never writes anything else.
//! That one property is what both backends lean on. The CPU interpreter runs
//! the list top to bottom over a whole batch of particles at a time (one
//! instruction, every particle, next instruction), which is the shape a later
//! SIMD pass wants. A GPU backend emits one WGSL `let` per instruction in the
//! same order and gets a valid shader with no scheduling of its own.
//!
//! Every op is something a shader can do in a line or two. There is nothing
//! here that only a CPU could run (no allocation, no data-dependent loops, no
//! callbacks into the world), and the two table lookups ([`Op::Curve`],
//! [`Op::Gradient`]) carry an index into [`CompiledEmitter`]'s tables rather
//! than the table itself, so a GPU backend can bake them into a texture once.
//!
//! Values are always four lanes wide in storage; [`ValueType`] says how many
//! are meaningful. The interpreter zeroes the rest after every instruction so a
//! `length` or `dot` never picks up a stray lane, and so a NaN from a division
//! in a lane nobody reads cannot leak into one somebody does.

use crate::curve::{CurveTable, GradientTable};

/// The four value types a pin can carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ValueType {
    Float,
    Vec2,
    Vec3,
    /// Linear RGBA.
    Color,
}

impl ValueType {
    /// Meaningful lanes.
    pub fn width(self) -> usize {
        match self {
            ValueType::Float => 1,
            ValueType::Vec2 => 2,
            ValueType::Vec3 => 3,
            ValueType::Color => 4,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ValueType::Float => "Float",
            ValueType::Vec2 => "Vec2",
            ValueType::Vec3 => "Vec3",
            ValueType::Color => "Color",
        }
    }

    /// The type two operands of a generic node agree on: equal types are
    /// themselves, and a `Float` widens to the other side (an implicit splat).
    /// Any other pair is a mismatch, deliberately: silently dropping a Color's
    /// alpha to fit a Vec3 is the kind of conversion nobody notices until the
    /// effect looks wrong.
    pub fn unify(self, other: ValueType) -> Option<ValueType> {
        if self == other {
            Some(self)
        } else if self == ValueType::Float {
            Some(other)
        } else if other == ValueType::Float {
            Some(self)
        } else {
            None
        }
    }
}

/// A register: the result of instruction `.0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Reg(pub u32);

/// A value the program reads from outside itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Attribute {
    /// Seconds since the particle was born.
    Age,
    /// `age / lifetime`, 0 at birth and 1 at death.
    Life,
    Lifetime,
    Position,
    Velocity,
    /// The particle's spawn index: 0 for the first particle the emitter ever
    /// spawned, counting up. Stable for the particle's life.
    Index,
    /// Seconds into the current cycle of the effect.
    EmitterTime,
    /// This step's length in seconds.
    DeltaTime,
    /// World-space velocity of the entity the effect is on.
    EmitterVelocity,
    /// The world wind this step in m/s, gust included, in the emitter's
    /// simulation space (so a `Local` emitter on a turned entity still gets
    /// the wind from the right side). One value for the whole emitter; the
    /// per-particle cross-wind is built from it by the wind nodes.
    Wind,
    /// The world wind's turbulence, 0..1: how strong the cross-wind wobble is
    /// relative to the wind speed.
    WindTurbulence,
}

impl Attribute {
    pub fn value_type(self) -> ValueType {
        match self {
            Attribute::Position
            | Attribute::Velocity
            | Attribute::EmitterVelocity
            | Attribute::Wind => ValueType::Vec3,
            _ => ValueType::Float,
        }
    }

    /// Whether the attribute means anything in `ctx`. One that does not reads
    /// as zero: a particle being born has no age yet, and the spawn context is
    /// not about any particle at all.
    pub fn available_in(self, ctx: Context) -> bool {
        match self {
            Attribute::EmitterTime
            | Attribute::DeltaTime
            | Attribute::EmitterVelocity
            | Attribute::Wind
            | Attribute::WindTurbulence => true,
            Attribute::Index => ctx != Context::Spawn,
            Attribute::Age
            | Attribute::Life
            | Attribute::Lifetime
            | Attribute::Position
            | Attribute::Velocity => ctx == Context::Update,
        }
    }
}

/// When a sink pin is evaluated, and so what its program may read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Context {
    /// Once per step for the whole emitter: how many to spawn.
    Spawn,
    /// Once per particle, at birth.
    Init,
    /// Once per particle, every step.
    Update,
}

/// Lane-wise unless noted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Neg,
    Abs,
    Sin,
    Cos,
    /// Negative inputs give 0.
    Sqrt,
    Cbrt,
    /// Float result.
    Length,
    /// Zero stays zero rather than becoming NaN.
    Normalize,
    /// Float in, every lane of the result type set to it.
    Splat,
    /// A uniformly distributed unit vector from the first two lanes of a
    /// random value (0..1 each).
    UnitSphere,
    /// `(cos 2 pi x, sin 2 pi x, 0)` from a 0..1 float: a point on the unit
    /// circle in the XY plane.
    UnitCircle,
    /// Curl of a smooth 3D noise field at the given point: divergence-free, so
    /// particles swirl without bunching up or thinning out.
    Curl,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    /// Division by (near) zero gives 0 rather than infinity: a graph that
    /// divides by a lifetime that happens to be 0 should lose that particle's
    /// value, not poison its position.
    Div,
    Min,
    Max,
    /// Float result.
    Dot,
    /// Vec3 only.
    Cross,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TernaryOp {
    /// `a + (b - a) * c`.
    Lerp,
    /// `min(max(a, b), c)`.
    Clamp,
    /// `smoothstep(a, b, c)` with edges `a`, `b` and input `c`. Equal edges
    /// are a hard step (0 below, 1 at or above), which WGSL leaves undefined,
    /// so a GPU backend must special-case it: `force/wind` relies on it.
    Smoothstep,
    /// A unit vector inside a cone of half-angle `a` (radians) around +Y,
    /// uniform over the cap, from two 0..1 floats `b` and `c`.
    ConeDirection,
    /// Fractal curl noise at the Vec3 `a`: `b` octaves (fractional, the last
    /// one faded in by the fraction, so a keyed octave count glides rather
    /// than pops; at most [`crate::noise::MAX_OCTAVES`]), each `c.x`
    /// (lacunarity) times the frequency and `c.y` (persistence) times the
    /// amplitude of the one before, summed unnormalised. A loop with a
    /// constant bound, which a shader runs as is.
    CurlFbm,
}

/// One instruction. Constants are stored as bit patterns so instructions can be
/// hashed and compared exactly, which is how the compiler shares common
/// subexpressions: two identical instructions over identical registers are the
/// same value, and the second is never emitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Op {
    Const([u32; 4]),
    Attr(Attribute),
    /// Four independent uniform randoms in 0..1, from a stateless hash of the
    /// emitter seed, the particle's spawn index, the bits of `salt`, and
    /// `stream`. Nothing is carried between steps, so a replay is identical and
    /// a GPU backend computes the very same numbers from the same inputs.
    Random { salt: Reg, stream: u32 },
    Unary(UnaryOp, Reg),
    Binary(BinaryOp, Reg, Reg),
    Ternary(TernaryOp, Reg, Reg, Reg),
    /// One lane of a vector, as a Float.
    Extract(Reg, u8),
    /// Three Floats into a Vec3.
    Compose3(Reg, Reg, Reg),
    /// Two Floats into a Vec2.
    Compose2(Reg, Reg),
    /// Sample [`CompiledEmitter::curves`]`[index]` at the Float `t`.
    Curve { index: u32, t: Reg },
    /// Sample [`CompiledEmitter::gradients`]`[index]` at the Float `t`.
    Gradient { index: u32, t: Reg },
    /// Parameter slot `.0`: a keyed pin's value this step, filled from the
    /// effect's tracks before the programs run (see [`crate::params`]). The
    /// same for every particle of a step, like an attribute of the emitter.
    Param(u32),
}

impl Op {
    pub fn constant(v: [f32; 4]) -> Op {
        Op::Const(v.map(f32::to_bits))
    }

    /// Registers this op reads, for validation and for a backend's liveness.
    pub fn operands(&self) -> Vec<Reg> {
        match *self {
            Op::Const(_) | Op::Attr(_) | Op::Param(_) => vec![],
            Op::Random { salt, .. } => vec![salt],
            Op::Unary(_, a) | Op::Extract(a, _) => vec![a],
            Op::Curve { t, .. } | Op::Gradient { t, .. } => vec![t],
            Op::Binary(_, a, b) | Op::Compose2(a, b) => vec![a, b],
            Op::Ternary(_, a, b, c) | Op::Compose3(a, b, c) => vec![a, b, c],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Instr {
    pub op: Op,
    /// The result's type.
    pub ty: ValueType,
}

/// The inputs of the `output/particle` node, each evaluated in a fixed context.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SinkPin {
    SpawnRate,
    BurstCount,
    BurstInterval,
    BurstCycles,
    Lifetime,
    Position,
    Velocity,
    Size,
    Rotation,
    AngularVelocity,
    Color,
    Acceleration,
    Gravity,
    Drag,
    SizeOverLife,
    ColorOverLife,
    /// A hard cap on speed, applied after integration; 0 or less is no cap.
    MaxSpeed,
    /// An HDR intensity multiplied into the colour's rgb (not its alpha)
    /// after `color_over_life`: a curve over life here makes a spark flare
    /// past white into the bloom and cool back down, without the gradient's
    /// stops having to carry numbers above 1. Last, so every pin before it
    /// keeps its place.
    Emission,
}

impl SinkPin {
    pub const ALL: [SinkPin; 18] = [
        SinkPin::SpawnRate,
        SinkPin::BurstCount,
        SinkPin::BurstInterval,
        SinkPin::BurstCycles,
        SinkPin::Lifetime,
        SinkPin::Position,
        SinkPin::Velocity,
        SinkPin::Size,
        SinkPin::Rotation,
        SinkPin::AngularVelocity,
        SinkPin::Color,
        SinkPin::Acceleration,
        SinkPin::Gravity,
        SinkPin::Drag,
        SinkPin::SizeOverLife,
        SinkPin::ColorOverLife,
        SinkPin::MaxSpeed,
        SinkPin::Emission,
    ];

    /// The pin's name on the `output/particle` node.
    pub fn name(self) -> &'static str {
        match self {
            SinkPin::SpawnRate => "spawn_rate",
            SinkPin::BurstCount => "burst_count",
            SinkPin::BurstInterval => "burst_interval",
            SinkPin::BurstCycles => "burst_cycles",
            SinkPin::Lifetime => "lifetime",
            SinkPin::Position => "position",
            SinkPin::Velocity => "velocity",
            SinkPin::Size => "size",
            SinkPin::Rotation => "rotation",
            SinkPin::AngularVelocity => "angular_velocity",
            SinkPin::Color => "color",
            SinkPin::Acceleration => "acceleration",
            SinkPin::Gravity => "gravity",
            SinkPin::Drag => "drag",
            SinkPin::SizeOverLife => "size_over_life",
            SinkPin::ColorOverLife => "color_over_life",
            SinkPin::MaxSpeed => "max_speed",
            SinkPin::Emission => "emission",
        }
    }

    pub fn from_name(name: &str) -> Option<SinkPin> {
        SinkPin::ALL.into_iter().find(|p| p.name() == name)
    }

    pub fn context(self) -> Context {
        match self {
            SinkPin::SpawnRate
            | SinkPin::BurstCount
            | SinkPin::BurstInterval
            | SinkPin::BurstCycles => Context::Spawn,
            SinkPin::Lifetime
            | SinkPin::Position
            | SinkPin::Velocity
            | SinkPin::Size
            | SinkPin::Rotation
            | SinkPin::AngularVelocity
            | SinkPin::Color => Context::Init,
            SinkPin::Acceleration
            | SinkPin::Gravity
            | SinkPin::Drag
            | SinkPin::SizeOverLife
            | SinkPin::ColorOverLife
            | SinkPin::MaxSpeed
            | SinkPin::Emission => Context::Update,
        }
    }

    pub fn value_type(self) -> ValueType {
        match self {
            SinkPin::Position | SinkPin::Velocity | SinkPin::Acceleration | SinkPin::Gravity => {
                ValueType::Vec3
            }
            SinkPin::Size | SinkPin::SizeOverLife => ValueType::Vec2,
            SinkPin::Color | SinkPin::ColorOverLife => ValueType::Color,
            _ => ValueType::Float,
        }
    }
}

/// The instructions for one context, and which register each of its sink pins
/// ended up in. Every sink pin of the context is present: an unconnected one
/// still gets a register holding its inline value, so a backend never has to
/// know the defaults.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Program {
    pub instrs: Vec<Instr>,
    pub outputs: Vec<(SinkPin, Reg)>,
}

impl Program {
    pub fn output(&self, pin: SinkPin) -> Option<Reg> {
        self.outputs.iter().find(|(p, _)| *p == pin).map(|(_, r)| *r)
    }

    pub fn ty(&self, reg: Reg) -> ValueType {
        self.instrs[reg.0 as usize].ty
    }
}

/// The shape of a collision node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ColliderShape {
    /// An infinite plane through `a` with normal `b`; particles stay on the
    /// side the normal points to.
    Plane,
    /// A sphere at `a` with radius `b` (a Float).
    Sphere,
    /// An axis-aligned box centred on `a` with full size `b` (a Vec3).
    Box,
    /// The scene's own physics colliders, through `renzora::scene_query`;
    /// `b` is the particle's radius (a Float) and `a` is unused. Not tested
    /// in the step like the others: the scene can only be asked with the
    /// world in hand, so the runtime casts every particle's frame of motion
    /// in one batch after the step ([`crate::scene`]). The CPU interpreter
    /// only records what that pass needs; the GPU backend refuses the node.
    Scene,
}

/// One collision node, lowered. Collisions are not values a pin can carry:
/// they change a particle's position and velocity after it has moved, which
/// no force can do exactly. So a collision node is a second kind of sink: the
/// compiler finds every one in the graph, wired or not, lowers its inputs into
/// the update program, and the simulation applies them after integration, in
/// node-list order. Every register here belongs to [`CompiledEmitter::update`],
/// and every one may differ per particle (a random bounce is fine).
///
/// The geometry is in the effect entity's own space whatever the emitter's
/// simulation space, so "a ground plane at the emitter's base" means the same
/// for a `Local` flame and a `World` ember, and moves with the entity. With
/// `world` set it is in world coordinates instead: a floor that stays put
/// while the effect moves over it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ColliderIr {
    /// The node it came from, for errors and the editor's gizmo.
    pub node: u64,
    pub shape: ColliderShape,
    /// Sphere and box: keep particles inside instead of outside.
    pub contain: bool,
    /// Kill a particle that hits instead of bouncing it.
    pub kill: bool,
    /// The geometry is in world space rather than the entity's.
    pub world: bool,
    /// Plane point, or sphere or box centre (Vec3).
    pub a: Reg,
    /// Plane normal (Vec3), sphere radius (Float) or box size (Vec3).
    pub b: Reg,
    /// 0..1: the share of the speed into the surface that comes back out.
    pub bounce: Reg,
    /// Coulomb friction coefficient: tangential speed lost per unit of the
    /// speed the hit took out of the normal direction.
    pub friction: Reg,
}

/// Everything one emitter's graph compiled to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompiledEmitter {
    pub spawn: Program,
    pub init: Program,
    pub update: Program,
    pub curves: Vec<CurveTable>,
    pub gradients: Vec<GradientTable>,
    /// Collision nodes, in node-list order; see [`ColliderIr`].
    pub colliders: Vec<ColliderIr>,
}

impl CompiledEmitter {
    pub fn program(&self, ctx: Context) -> &Program {
        match ctx {
            Context::Spawn => &self.spawn,
            Context::Init => &self.init,
            Context::Update => &self.update,
        }
    }
}
