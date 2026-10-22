//! The CPU backend's IR interpreter.
//!
//! Batch-oriented on purpose: the outer loop is over instructions and the inner
//! loop over particles, so each inner loop is one operation applied to a
//! contiguous slice. That is slower to write than a per-particle `match` and
//! faster to run, and it is the shape a SIMD pass wants: every inner loop below
//! can become a `wide`/`std::simd` loop without touching the dispatch around
//! it. Correct and simple first; the loops are kept plain for that reason.
//!
//! Registers are `Vec4` columns, one per instruction, sized to the batch and
//! reused across calls so a steady-state step allocates nothing.

use bevy::math::{Vec2, Vec3, Vec4};
use std::f32::consts::TAU;

use crate::ir::{Attribute, BinaryOp, CompiledEmitter, Op, Program, Reg, TernaryOp, UnaryOp, ValueType};
use crate::noise;
use crate::random::{random4, salt_bits};

/// Inputs to one evaluation. Empty slices read as zero, which is how a context
/// without that attribute sees it (the compiler already folded those reads to
/// constants; this is the belt to that brace).
pub struct EvalInputs<'a> {
    pub count: usize,
    pub seed: u64,
    /// Spawn indices, one per particle. Empty in the spawn context, which uses
    /// `spawn_id` for every lane instead.
    pub ids: &'a [u32],
    pub spawn_id: u32,
    pub age: &'a [f32],
    pub lifetime: &'a [f32],
    pub position: &'a [Vec3],
    pub velocity: &'a [Vec3],
    pub emitter_time: f32,
    pub delta_time: f32,
    pub emitter_velocity: Vec3,
    /// The world wind in m/s, already in the emitter's simulation space.
    pub wind: Vec3,
    pub wind_turbulence: f32,
    /// Keyed pins' values this step, by slot ([`Op::Param`]). A slot past the
    /// end reads as zero.
    pub params: &'a [Vec4],
}

/// Scratch registers, reused between evaluations.
#[derive(Default)]
pub struct Registers {
    cols: Vec<Vec<Vec4>>,
}

impl Registers {
    pub fn get(&self, r: Reg) -> &[Vec4] {
        &self.cols[r.0 as usize]
    }
}

/// Zero the lanes a type does not use. Every register satisfies this after its
/// instruction runs, which is what lets `length` and `dot` work on all four
/// lanes without knowing the type.
#[inline]
fn mask(v: Vec4, ty: ValueType) -> Vec4 {
    match ty {
        ValueType::Float => Vec4::new(v.x, 0.0, 0.0, 0.0),
        ValueType::Vec2 => Vec4::new(v.x, v.y, 0.0, 0.0),
        ValueType::Vec3 => Vec4::new(v.x, v.y, v.z, 0.0),
        ValueType::Color => v,
    }
}

#[inline]
fn safe_div(a: f32, b: f32) -> f32 {
    if b.abs() > 1e-20 {
        a / b
    } else {
        0.0
    }
}

#[inline]
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    if e1 == e0 {
        return if x < e0 { 0.0 } else { 1.0 };
    }
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn lanes(f: impl Fn(f32) -> f32, v: Vec4) -> Vec4 {
    Vec4::new(f(v.x), f(v.y), f(v.z), f(v.w))
}

#[inline]
fn lanes2(f: impl Fn(f32, f32) -> f32, a: Vec4, b: Vec4) -> Vec4 {
    Vec4::new(f(a.x, b.x), f(a.y, b.y), f(a.z, b.z), f(a.w, b.w))
}

/// A uniformly distributed point on the unit sphere from two uniforms.
#[inline]
pub fn unit_sphere(u: f32, v: f32) -> Vec3 {
    let z = 1.0 - 2.0 * u;
    let r = (1.0 - z * z).max(0.0).sqrt();
    let phi = TAU * v;
    Vec3::new(r * phi.cos(), r * phi.sin(), z)
}

/// A unit vector within `angle` radians of +Y, uniform over the spherical cap.
#[inline]
pub fn cone_direction(angle: f32, u: f32, v: f32) -> Vec3 {
    let cos_max = angle.clamp(0.0, std::f32::consts::PI).cos();
    let cos_t = 1.0 - u * (1.0 - cos_max);
    let sin_t = (1.0 - cos_t * cos_t).max(0.0).sqrt();
    let phi = TAU * v;
    Vec3::new(sin_t * phi.cos(), cos_t, sin_t * phi.sin())
}

/// Run `program` over `inputs.count` lanes. Results stay in `regs` until the
/// next call.
pub fn eval(program: &Program, tables: &CompiledEmitter, inputs: &EvalInputs, regs: &mut Registers) {
    let n = inputs.count;
    if regs.cols.len() < program.instrs.len() {
        regs.cols.resize_with(program.instrs.len(), Vec::new);
    }
    for (i, instr) in program.instrs.iter().enumerate() {
        let (lower, upper) = regs.cols.split_at_mut(i);
        let dst = &mut upper[0];
        dst.clear();
        dst.resize(n, Vec4::ZERO);
        let src = |r: Reg| -> &[Vec4] { &lower[r.0 as usize] };
        let ty = instr.ty;
        match instr.op {
            Op::Const(bits) => {
                let v = mask(Vec4::from_array(bits.map(f32::from_bits)), ty);
                dst.fill(v);
            }
            Op::Attr(a) => eval_attr(a, inputs, dst),
            Op::Random { salt, stream } => {
                let salt = src(salt);
                for (k, out) in dst.iter_mut().enumerate() {
                    let id = inputs.ids.get(k).copied().unwrap_or(inputs.spawn_id);
                    *out = Vec4::from_array(random4(inputs.seed, id, salt_bits(salt[k].x), stream));
                }
            }
            Op::Unary(op, a) => {
                let a = src(a);
                for (out, &v) in dst.iter_mut().zip(a) {
                    *out = mask(unary(op, v), ty);
                }
            }
            Op::Binary(op, a, b) => {
                let (a, b) = (src(a), src(b));
                for ((out, &x), &y) in dst.iter_mut().zip(a).zip(b) {
                    *out = mask(binary(op, x, y), ty);
                }
            }
            Op::Ternary(op, a, b, c) => {
                let (a, b, c) = (src(a), src(b), src(c));
                for (k, out) in dst.iter_mut().enumerate() {
                    *out = mask(ternary(op, a[k], b[k], c[k]), ty);
                }
            }
            Op::Extract(a, lane) => {
                let a = src(a);
                let lane = (lane as usize).min(3);
                for (out, v) in dst.iter_mut().zip(a) {
                    *out = Vec4::new(v[lane], 0.0, 0.0, 0.0);
                }
            }
            Op::Compose3(x, y, z) => {
                let (x, y, z) = (src(x), src(y), src(z));
                for (k, out) in dst.iter_mut().enumerate() {
                    *out = Vec4::new(x[k].x, y[k].x, z[k].x, 0.0);
                }
            }
            Op::Compose2(x, y) => {
                let (x, y) = (src(x), src(y));
                for (k, out) in dst.iter_mut().enumerate() {
                    *out = Vec4::new(x[k].x, y[k].x, 0.0, 0.0);
                }
            }
            Op::Curve { index, t } => {
                let t = src(t);
                match tables.curves.get(index as usize) {
                    Some(curve) => {
                        for (out, v) in dst.iter_mut().zip(t) {
                            *out = Vec4::new(curve.sample(v.x), 0.0, 0.0, 0.0);
                        }
                    }
                    None => dst.fill(Vec4::X),
                }
            }
            Op::Gradient { index, t } => {
                let t = src(t);
                match tables.gradients.get(index as usize) {
                    Some(gradient) => {
                        for (out, v) in dst.iter_mut().zip(t) {
                            *out = gradient.sample(v.x);
                        }
                    }
                    None => dst.fill(Vec4::ONE),
                }
            }
            Op::Param(slot) => {
                let v = inputs.params.get(slot as usize).copied().unwrap_or(Vec4::ZERO);
                dst.fill(mask(v, ty));
            }
        }
    }
}

fn eval_attr(a: Attribute, inputs: &EvalInputs, dst: &mut [Vec4]) {
    let scalar = |s: &[f32], dst: &mut [Vec4]| {
        for (k, out) in dst.iter_mut().enumerate() {
            *out = Vec4::new(s.get(k).copied().unwrap_or(0.0), 0.0, 0.0, 0.0);
        }
    };
    let vector = |s: &[Vec3], dst: &mut [Vec4]| {
        for (k, out) in dst.iter_mut().enumerate() {
            *out = s.get(k).copied().unwrap_or(Vec3::ZERO).extend(0.0);
        }
    };
    match a {
        Attribute::Age => scalar(inputs.age, dst),
        Attribute::Lifetime => scalar(inputs.lifetime, dst),
        Attribute::Life => {
            for (k, out) in dst.iter_mut().enumerate() {
                let age = inputs.age.get(k).copied().unwrap_or(0.0);
                let life = inputs.lifetime.get(k).copied().unwrap_or(0.0);
                *out = Vec4::new(safe_div(age, life).clamp(0.0, 1.0), 0.0, 0.0, 0.0);
            }
        }
        Attribute::Position => vector(inputs.position, dst),
        Attribute::Velocity => vector(inputs.velocity, dst),
        Attribute::Index => {
            for (k, out) in dst.iter_mut().enumerate() {
                let id = inputs.ids.get(k).copied().unwrap_or(0);
                *out = Vec4::new(id as f32, 0.0, 0.0, 0.0);
            }
        }
        Attribute::EmitterTime => dst.fill(Vec4::new(inputs.emitter_time, 0.0, 0.0, 0.0)),
        Attribute::DeltaTime => dst.fill(Vec4::new(inputs.delta_time, 0.0, 0.0, 0.0)),
        Attribute::EmitterVelocity => dst.fill(inputs.emitter_velocity.extend(0.0)),
        Attribute::Wind => dst.fill(inputs.wind.extend(0.0)),
        Attribute::WindTurbulence => dst.fill(Vec4::new(inputs.wind_turbulence, 0.0, 0.0, 0.0)),
    }
}

#[inline]
fn unary(op: UnaryOp, v: Vec4) -> Vec4 {
    match op {
        UnaryOp::Neg => -v,
        UnaryOp::Abs => v.abs(),
        UnaryOp::Sin => lanes(f32::sin, v),
        UnaryOp::Cos => lanes(f32::cos, v),
        UnaryOp::Sqrt => lanes(|x| x.max(0.0).sqrt(), v),
        UnaryOp::Cbrt => lanes(f32::cbrt, v),
        UnaryOp::Length => Vec4::new(v.length(), 0.0, 0.0, 0.0),
        UnaryOp::Normalize => {
            let len = v.length();
            if len > 1e-12 {
                v / len
            } else {
                Vec4::ZERO
            }
        }
        UnaryOp::Splat => Vec4::splat(v.x),
        UnaryOp::UnitSphere => unit_sphere(v.x, v.y).extend(0.0),
        UnaryOp::UnitCircle => {
            let a = TAU * v.x;
            Vec4::new(a.cos(), a.sin(), 0.0, 0.0)
        }
        UnaryOp::Curl => noise::curl(v.truncate()).extend(0.0),
    }
}

#[inline]
fn binary(op: BinaryOp, a: Vec4, b: Vec4) -> Vec4 {
    match op {
        BinaryOp::Add => a + b,
        BinaryOp::Sub => a - b,
        BinaryOp::Mul => a * b,
        BinaryOp::Div => lanes2(safe_div, a, b),
        BinaryOp::Min => a.min(b),
        BinaryOp::Max => a.max(b),
        BinaryOp::Dot => Vec4::new(a.dot(b), 0.0, 0.0, 0.0),
        BinaryOp::Cross => a.truncate().cross(b.truncate()).extend(0.0),
    }
}

#[inline]
fn ternary(op: TernaryOp, a: Vec4, b: Vec4, c: Vec4) -> Vec4 {
    match op {
        TernaryOp::Lerp => a + (b - a) * c,
        TernaryOp::Clamp => a.max(b).min(c),
        TernaryOp::Smoothstep => Vec4::new(
            smoothstep(a.x, b.x, c.x),
            smoothstep(a.y, b.y, c.y),
            smoothstep(a.z, b.z, c.z),
            smoothstep(a.w, b.w, c.w),
        ),
        TernaryOp::ConeDirection => cone_direction(a.x, b.x, c.x).extend(0.0),
        TernaryOp::CurlFbm => noise::curl_fbm(a.truncate(), b.x, c.x, c.y).extend(0.0),
    }
}

/// Read a register as the type a sink pin wants.
#[inline]
pub fn as_vec3(v: Vec4) -> Vec3 {
    v.truncate()
}

#[inline]
pub fn as_vec2(v: Vec4) -> Vec2 {
    Vec2::new(v.x, v.y)
}
