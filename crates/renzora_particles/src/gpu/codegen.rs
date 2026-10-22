//! IR to WGSL: one compute shader per emitter, with a `spawn` and an
//! `update` entry point, and a `deliver` entry point for an emitter other
//! GPU emitters' sub-emitter rules spawn.
//!
//! The IR was built for this ([`crate::ir`]): instruction `i` reads only
//! lower registers, so each becomes one `let` in program order and the result
//! is a valid shader with no scheduling of its own. Every register is a
//! `vec4<f32>` masked to its type after each instruction, exactly as the
//! interpreter stores them, so a `length` or `dot` over four lanes agrees on
//! both backends without either knowing the type.
//!
//! ## What the text depends on
//!
//! Only the graph's *structure*: which ops, in which order, reading which
//! registers. Constants, curve keys and gradient stops are not in the text;
//! they live in the emitter's data table ([`super::layout::pack_data`]) and the
//! shader reads them by index. Dragging a value in the editor therefore
//! rewrites a small buffer instead of compiling a new pipeline, which would
//! otherwise freeze the emitter for the frames a compile takes. Two emitters
//! with one structure share one shader, which the runtime caches by
//! [`GpuProgram::hash`].
//!
//! ## What it mirrors
//!
//! The kernels' bodies are `crate::sim`'s `spawn` and `update`, step for
//! step: born at age 0 with the init program's outputs sanitized the same
//! way and moved into world space the same way; aged, killed past their
//! lifetime, then forces, implicit drag, the speed cap, integration, every
//! collider in node order with the same space changes, and the over-life
//! multipliers. The helpers they call are transliterated in `library.wgsl`.
//!
//! ## Trails and sub-emitters
//!
//! What [`GpuEmitterConfig`] adds is `crate::sim` again. A `Trail` ribbon's
//! history ring is kept exactly as `ParticleBuffer::record_trail` keeps it:
//! the birth position first, then a new point once the particle has moved
//! `min_distance` from the newest. Events are raised where the CPU raises
//! them: a birth in `spawn` (and in `deliver`, for a child's own rules), a
//! death by age before the step moves the particle, intervals after it, and a
//! collider's kill last, each moved into the target's simulation space at
//! once rather than at delivery (the two emitters share one entity, so one
//! transform does both). The rule's count, velocity share, interval and
//! inbox live in the data table like any constant, so editing them rewrites a
//! buffer and not a pipeline.
//!
//! The one deliberate difference is a child's spawn index. The CPU hands
//! children the target's next indices in a sorted event order; a GPU target
//! sees its events in whatever order the parents' threads raised them, so its
//! children take an index hashed from the event instead ([`child_id`] in
//! `library.wgsl`): parent, rule, event and child. Every random a child reads
//! is still fixed by what spawned it, so a seek replays it exactly; it is
//! only not the same number the CPU would have given it.

use std::fmt::Write as _;

use crate::asset::{SimulationSpace, SubEmitterTrigger};
use crate::ir::{
    Attribute, BinaryOp, ColliderShape, CompiledEmitter, Context, Op, Program, Reg, SinkPin, TernaryOp, UnaryOp,
    ValueType,
};

use super::layout::MAX_GPU_PARAMS;

/// Everything about an emitter beyond its graph that shapes its kernels.
#[derive(Clone, Debug, PartialEq)]
pub struct GpuEmitterConfig {
    pub space: SimulationSpace,
    /// [`crate::asset::Emitter::inherited_velocity`]: 0 emits nothing.
    pub inherit_velocity: f32,
    /// A `Trail` ribbon's history: `(points, min_distance)`.
    pub trail: Option<(u32, f32)>,
    /// The sub-emitter rules this emitter's particles raise, in rule order.
    /// Empty for an emitter with none, or whose particles are all past the
    /// generation cap.
    pub rules: Vec<GpuRule>,
    /// Another GPU emitter's rules spawn this one: generate `deliver`.
    pub target: bool,
}

impl GpuEmitterConfig {
    /// An emitter with nothing beyond its graph and space.
    pub fn plain(space: SimulationSpace) -> Self {
        Self { space, inherit_velocity: 0.0, trail: None, rules: Vec::new(), target: false }
    }
}

/// One sub-emitter rule, resolved against its effect's event buffer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuRule {
    pub trigger: SubEmitterTrigger,
    /// The target's inbox: first word and event capacity.
    pub inbox: (u32, u32),
    pub count: u32,
    pub inherit_velocity: f32,
    pub inherit_color: bool,
    pub target_space: SimulationSpace,
    /// Hashed into the children's spawn indices, so two rules from one
    /// parent never give two children the same randoms.
    pub key: u32,
}

/// The shared library every kernel is compiled against.
pub const LIBRARY: &str = include_str!("library.wgsl");
/// The fixed kernels (reset, stats, sort), appended to [`LIBRARY`].
pub const KERNELS: &str = include_str!("kernels.wgsl");

/// Threads per workgroup in every particle kernel. The dispatch arithmetic in
/// the render world divides by it.
pub const WORKGROUP_SIZE: u32 = 64;

/// One emitter's generated shader.
#[derive(Clone, Debug, PartialEq)]
pub struct GpuProgram {
    pub source: String,
    /// FNV-1a of `source`: the shader cache's key. Stable across runs, unlike
    /// the standard hasher, so a log line naming it means the same thing
    /// tomorrow.
    pub hash: u64,
    /// The bit patterns of the constants the text reads, in the data table
    /// order the text indexes them by.
    pub consts: Vec<[u32; 4]>,
}

/// Why an emitter's graph cannot be generated for the GPU.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodegenError {
    /// More keyed pins than the step record has parameter slots.
    TooManyParams(usize),
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodegenError::TooManyParams(n) => {
                write!(f, "it keys {n} pins and the GPU step has room for {MAX_GPU_PARAMS}")
            }
        }
    }
}

/// The fixed kernels' whole source.
pub fn kernels_source() -> String {
    format!("{LIBRARY}\n{KERNELS}")
}

/// FNV-1a, 64 bit.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// The highest parameter slot any program reads, plus one.
pub fn param_count(compiled: &CompiledEmitter) -> usize {
    [&compiled.spawn, &compiled.init, &compiled.update]
        .iter()
        .flat_map(|p| p.instrs.iter())
        .filter_map(|i| match i.op {
            Op::Param(slot) => Some(slot as usize + 1),
            _ => None,
        })
        .max()
        .unwrap_or(0)
}

/// Constants interned across both programs, so one value used by init and
/// update is one table entry.
#[derive(Default)]
struct ConstPool {
    values: Vec<[u32; 4]>,
}

impl ConstPool {
    /// A slot of its own, never shared: the config's values. Shared like a
    /// graph constant, two of them that happen to be equal (a rule's velocity
    /// share and the emitter's) would take one slot, and editing one apart
    /// would move every index after it, which is a new shader.
    fn own(&mut self, bits: [u32; 4]) -> usize {
        self.values.push(bits);
        self.values.len() - 1
    }

    fn index(&mut self, bits: [u32; 4]) -> usize {
        match self.values.iter().position(|v| *v == bits) {
            Some(i) => i,
            None => {
                self.values.push(bits);
                self.values.len() - 1
            }
        }
    }
}

/// What the code emitters need to know about the data table: where the curve
/// and gradient directories start. Constants come first, then the curves'
/// directory, then the gradients'.
struct Tables {
    curve_dir: usize,
    curves: usize,
    gradient_dir: usize,
    gradients: usize,
}

/// Generate the shader of an emitter with nothing beyond its graph and space.
pub fn generate(compiled: &CompiledEmitter, space: SimulationSpace) -> Result<GpuProgram, CodegenError> {
    generate_with(compiled, &GpuEmitterConfig::plain(space))
}

/// Where the config's values sit in the data table.
struct Extras {
    inherit: Option<usize>,
    min_distance: Option<usize>,
    /// Per rule: `(inbox base, inbox capacity, count, key)` as bits, and
    /// `(velocity share, interval seconds, 0, 0)`.
    rules: Vec<(usize, usize)>,
}

/// Generate the emitter's shader. The constant pool is gathered first, the
/// config's values with it, so the directories' positions are known before
/// any table read is written.
pub fn generate_with(compiled: &CompiledEmitter, config: &GpuEmitterConfig) -> Result<GpuProgram, CodegenError> {
    let params = param_count(compiled);
    if params > MAX_GPU_PARAMS {
        return Err(CodegenError::TooManyParams(params));
    }
    let mut pool = ConstPool::default();
    for program in [&compiled.init, &compiled.update] {
        for instr in &program.instrs {
            if let Op::Const(bits) = instr.op {
                pool.index(bits);
            }
        }
    }
    let float = |v: f32| if v.is_finite() { v } else { 0.0 };
    let extras = Extras {
        inherit: (config.space == SimulationSpace::World && float(config.inherit_velocity) != 0.0)
            .then(|| pool.own([float(config.inherit_velocity).to_bits(), 0, 0, 0])),
        min_distance: config.trail.map(|(_, d)| pool.own([float(d).max(0.0).to_bits(), 0, 0, 0])),
        rules: config
            .rules
            .iter()
            .map(|r| {
                let every = match r.trigger {
                    SubEmitterTrigger::Interval(s) => float(s),
                    _ => 0.0,
                };
                (
                    pool.own([r.inbox.0, r.inbox.1, r.count, r.key]),
                    pool.own([float(r.inherit_velocity).to_bits(), every.to_bits(), 0, 0]),
                )
            })
            .collect(),
    };
    let tables = Tables {
        curve_dir: pool.values.len(),
        curves: compiled.curves.len(),
        gradient_dir: pool.values.len() + compiled.curves.len(),
        gradients: compiled.gradients.len(),
    };

    let mut src = String::with_capacity(LIBRARY.len() + 8192);
    src.push_str(LIBRARY);
    src.push_str("\n// ── Generated from the emitter's graph ──\n\n");
    write_spawn(&mut src, &compiled.init, &mut pool, &tables, config, &extras);
    write_update(&mut src, compiled, &mut pool, &tables, config, &extras);
    write_deliver(&mut src, config);
    let hash = fnv1a(src.as_bytes());
    Ok(GpuProgram { source: src, hash, consts: pool.values })
}

fn reg_name(ctx: Context, r: Reg) -> String {
    match ctx {
        Context::Init => format!("i{}", r.0),
        _ => format!("u{}", r.0),
    }
}

fn mask_fn(ty: ValueType) -> &'static str {
    match ty {
        ValueType::Float => "m1",
        ValueType::Vec2 => "m2",
        ValueType::Vec3 => "m3",
        ValueType::Color => "",
    }
}

const LANES: [&str; 4] = ["x", "y", "z", "w"];

fn attr_expr(a: Attribute, ctx: Context) -> String {
    // The init context reads what the interpreter's init call passes: no
    // particle yet, and an emitter time and step of zero.
    let zero = "vec4<f32>(0.0)".to_string();
    match (ctx, a) {
        (_, Attribute::EmitterVelocity) => "vec4<f32>(step.emitter_velocity.xyz, 0.0)".into(),
        (_, Attribute::Wind) => "vec4<f32>(step.wind.xyz, 0.0)".into(),
        (_, Attribute::WindTurbulence) => "vec4<f32>(step.time.z, 0.0, 0.0, 0.0)".into(),
        (Context::Init, Attribute::Index) => "vec4<f32>(f32(id), 0.0, 0.0, 0.0)".into(),
        (Context::Init, _) => zero,
        (_, Attribute::Age) => "vec4<f32>(p.age, 0.0, 0.0, 0.0)".into(),
        (_, Attribute::Lifetime) => "vec4<f32>(p.lifetime, 0.0, 0.0, 0.0)".into(),
        (_, Attribute::Life) => "vec4<f32>(clamp(safe_div1(p.age, p.lifetime), 0.0, 1.0), 0.0, 0.0, 0.0)".into(),
        (_, Attribute::Position) => "vec4<f32>(p.position, 0.0)".into(),
        (_, Attribute::Velocity) => "vec4<f32>(p.velocity, 0.0)".into(),
        (_, Attribute::Index) => "vec4<f32>(f32(p.id), 0.0, 0.0, 0.0)".into(),
        (_, Attribute::EmitterTime) => "vec4<f32>(step.time.y, 0.0, 0.0, 0.0)".into(),
        (_, Attribute::DeltaTime) => "vec4<f32>(step.time.x, 0.0, 0.0, 0.0)".into(),
    }
}

/// One `let` per instruction.
fn write_program(src: &mut String, program: &Program, ctx: Context, pool: &mut ConstPool, tables: &Tables) {
    let r = |reg: Reg| reg_name(ctx, reg);
    for (i, instr) in program.instrs.iter().enumerate() {
        let expr = match instr.op {
            Op::Const(bits) => format!("data[{}u]", pool.index(bits)),
            Op::Attr(a) => attr_expr(a, ctx),
            Op::Random { salt, stream } => {
                let id = if ctx == Context::Init { "id" } else { "p.id" };
                format!("random4({id}, salt_bits({}.x), {stream}u)", r(salt))
            }
            Op::Unary(op, a) => {
                let a = r(a);
                match op {
                    UnaryOp::Neg => format!("-{a}"),
                    UnaryOp::Abs => format!("abs({a})"),
                    UnaryOp::Sin => format!("sin({a})"),
                    UnaryOp::Cos => format!("cos({a})"),
                    UnaryOp::Sqrt => format!("sqrt(max({a}, vec4<f32>(0.0)))"),
                    UnaryOp::Cbrt => format!("cbrt4({a})"),
                    UnaryOp::Length => format!("vec4<f32>(length({a}), 0.0, 0.0, 0.0)"),
                    UnaryOp::Normalize => format!("normalize4({a})"),
                    UnaryOp::Splat => format!("vec4<f32>({a}.x)"),
                    UnaryOp::UnitSphere => format!("vec4<f32>(unit_sphere({a}.x, {a}.y), 0.0)"),
                    UnaryOp::UnitCircle => format!("unit_circle({a}.x)"),
                    UnaryOp::Curl => format!("vec4<f32>(curl({a}.xyz), 0.0)"),
                }
            }
            Op::Binary(op, a, b) => {
                let (a, b) = (r(a), r(b));
                match op {
                    BinaryOp::Add => format!("({a} + {b})"),
                    BinaryOp::Sub => format!("({a} - {b})"),
                    BinaryOp::Mul => format!("({a} * {b})"),
                    BinaryOp::Div => format!("safe_div({a}, {b})"),
                    BinaryOp::Min => format!("min({a}, {b})"),
                    BinaryOp::Max => format!("max({a}, {b})"),
                    BinaryOp::Dot => format!("vec4<f32>(dot({a}, {b}), 0.0, 0.0, 0.0)"),
                    BinaryOp::Cross => format!("vec4<f32>(cross({a}.xyz, {b}.xyz), 0.0)"),
                }
            }
            Op::Ternary(op, a, b, c) => {
                let (a, b, c) = (r(a), r(b), r(c));
                match op {
                    TernaryOp::Lerp => format!("({a} + ({b} - {a}) * {c})"),
                    TernaryOp::Clamp => format!("min(max({a}, {b}), {c})"),
                    TernaryOp::Smoothstep => format!("smoothstep4({a}, {b}, {c})"),
                    TernaryOp::ConeDirection => format!("vec4<f32>(cone_direction({a}.x, {b}.x, {c}.x), 0.0)"),
                    TernaryOp::CurlFbm => format!("vec4<f32>(curl_fbm({a}.xyz, {b}.x, {c}.x, {c}.y), 0.0)"),
                }
            }
            Op::Extract(a, lane) => {
                format!("vec4<f32>({}.{}, 0.0, 0.0, 0.0)", r(a), LANES[(lane as usize).min(3)])
            }
            Op::Compose3(x, y, z) => format!("vec4<f32>({}.x, {}.x, {}.x, 0.0)", r(x), r(y), r(z)),
            Op::Compose2(x, y) => format!("vec4<f32>({}.x, {}.x, 0.0, 0.0)", r(x), r(y)),
            Op::Curve { index, t } => {
                if (index as usize) < tables.curves {
                    format!("vec4<f32>(sample_curve({}u, {}.x), 0.0, 0.0, 0.0)", tables.curve_dir + index as usize, r(t))
                } else {
                    "vec4<f32>(1.0, 0.0, 0.0, 0.0)".to_string()
                }
            }
            Op::Gradient { index, t } => {
                if (index as usize) < tables.gradients {
                    format!("sample_gradient({}u, {}.x)", tables.gradient_dir + index as usize, r(t))
                } else {
                    "vec4<f32>(1.0)".to_string()
                }
            }
            Op::Param(slot) => {
                if (slot as usize) < MAX_GPU_PARAMS {
                    format!("step.params[{slot}]")
                } else {
                    "vec4<f32>(0.0)".to_string()
                }
            }
        };
        let name = reg_name(ctx, Reg(i as u32));
        let mask = mask_fn(instr.ty);
        let _ = writeln!(src, "    let {name} = {mask}({expr});");
    }
}

/// The register holding `pin`, or `fallback` for a pin the program lacks.
fn out(program: &Program, ctx: Context, pin: SinkPin, fallback: &str) -> String {
    program.output(pin).map(|r| reg_name(ctx, r)).unwrap_or_else(|| fallback.to_string())
}

/// The data table index of a rule's first entry, as WGSL.
fn rule_meta(extras: &Extras, r: usize) -> usize {
    extras.rules[r].0
}

/// Raise every rule of `config` that `fires` for the particle whose
/// simulation-space position, velocity and colour are the expressions given,
/// as the CPU's `EmitterSim::raise` does: the velocity scaled by the rule's
/// share, both moved into the target's space. A `World` source raising into a
/// `Local` target needs the entity's inverse, and raises nothing without one,
/// as `EffectSim::deliver` skips such an event.
#[allow(clippy::too_many_arguments)]
fn write_raises(
    src: &mut String,
    config: &GpuEmitterConfig,
    extras: &Extras,
    fires: impl Fn(&SubEmitterTrigger) -> bool,
    pos: &str,
    vel: &str,
    col: &str,
    parent: &str,
    index: &str,
    indent: &str,
) {
    for (r, rule) in config.rules.iter().enumerate() {
        if !fires(&rule.trigger) {
            continue;
        }
        let (meta, vals) = (rule_meta(extras, r), extras.rules[r].1);
        let scaled = format!("({vel}) * data[{vals}u].x");
        let (p, v, guard) = match (config.space, rule.target_space) {
            (SimulationSpace::Local, SimulationSpace::World) => {
                (format!("point3(step.transform, {pos})"), format!("vector3(step.transform, {scaled})"), false)
            }
            (SimulationSpace::World, SimulationSpace::Local) => {
                (format!("point3(step.inverse, {pos})"), format!("vector3(step.inverse, {scaled})"), true)
            }
            _ => (pos.to_string(), scaled, false),
        };
        let flags = u32::from(rule.inherit_color);
        let call = format!("raise_event({meta}u, {p}, {v}, {col}, {flags}u, {parent}, {index});");
        if guard {
            let _ = writeln!(src, "{indent}if (step.counts.w & FLAG_INVERSE_OK) != 0u {{\n{indent}    {call}\n{indent}}}");
        } else {
            let _ = writeln!(src, "{indent}{call}");
        }
    }
}

fn fires_on_birth(t: &SubEmitterTrigger) -> bool {
    *t == SubEmitterTrigger::OnBirth
}

fn fires_on_death(t: &SubEmitterTrigger) -> bool {
    *t == SubEmitterTrigger::OnDeath
}

/// `birth`: the init program and everything `EmitterSim::spawn` does with
/// its outputs, for one slot and spawn index. One function both entry points
/// call, so an own spawn and a sub-emitter's child cannot drift apart: `child`
/// makes the init position and velocity offsets from the event's `origin` and
/// `ovel` (turned with the entity for a `World` emitter but not moved by it),
/// and `tint` multiplies the event's colour in.
fn write_birth(src: &mut String, init: &Program, pool: &mut ConstPool, tables: &Tables, config: &GpuEmitterConfig, extras: &Extras) {
    src.push_str(
        "// One particle's birth into `slot`; see codegen.rs's `write_birth`.\n\
         fn birth(slot: u32, id: u32, child: bool, origin: vec3<f32>, ovel: vec3<f32>, ocol: vec4<f32>, tint: bool) {\n",
    );
    write_program(src, init, Context::Init, pool, tables);
    let o = |pin| out(init, Context::Init, pin, "vec4<f32>(0.0)");
    let _ = writeln!(src, "    var pos = {}.xyz;", o(SinkPin::Position));
    let _ = writeln!(src, "    var vel = {}.xyz;", o(SinkPin::Velocity));
    if config.space == SimulationSpace::World {
        src.push_str(
            "    if child {\n        pos = vector3(step.transform, pos);\n    } else {\n        pos = point3(step.transform, pos);\n    }\n\
             \x20   vel = vector3(step.transform, vel);\n",
        );
        if let Some(k) = extras.inherit {
            let _ = writeln!(src, "    if !child {{\n        vel = vel + step.emitter_velocity.xyz * data[{k}u].x;\n    }}");
        }
    }
    let _ = writeln!(src, "    var col = {};", o(SinkPin::Color));
    src.push_str(
        "    if child {\n        pos = pos + origin;\n        vel = vel + ovel;\n        if tint {\n            col = col * ocol;\n        }\n    }\n",
    );
    let _ = writeln!(src, "    let size = {}.xy;", o(SinkPin::Size));
    src.push_str("    var q: Particle;\n");
    src.push_str("    q.position = san3(pos);\n    q.velocity = san3(vel);\n    q.age = 0.0;\n");
    let _ = writeln!(src, "    q.lifetime = max(san({}.x), 0.0);", o(SinkPin::Lifetime));
    src.push_str("    q.size = size;\n    q.base_size = size;\n");
    let _ = writeln!(src, "    q.rotation = san({}.x);", o(SinkPin::Rotation));
    let _ = writeln!(src, "    q.angular_velocity = san({}.x);", o(SinkPin::AngularVelocity));
    src.push_str("    q.color = col;\n    q.base_color = col;\n    q.id = id;\n    q.seed = particle_seed(id);\n    particles[slot] = q;\n");
    if let Some((points, _)) = config.trail {
        // The birth position is the trail's first kept point, as
        // `ParticleBuffer::push` keeps it.
        let _ = writeln!(src, "    trail_start(slot, {}u, q.position);", trail_slot(points));
    }
    write_raises(src, config, extras, fires_on_birth, "q.position", "q.velocity", "q.color", "id", "0u", "    ");
    src.push_str("}\n\n");
}

/// Words per slot of a trail of `points`, as WGSL reads it.
fn trail_slot(points: u32) -> u32 {
    super::layout::trail_slot_words(points.max(1))
}

fn write_spawn(src: &mut String, init: &Program, pool: &mut ConstPool, tables: &Tables, config: &GpuEmitterConfig, extras: &Extras) {
    write_birth(src, init, pool, tables, config, extras);
    src.push_str(
        "// Birth: one thread per particle asked for this step. Free slots are\n\
         // popped from the dead list; a thread that finds it empty counts a drop.\n\
         @compute @workgroup_size(64)\n\
         fn spawn(@builtin(global_invocation_id) gid: vec3<u32>) {\n\
         \x20   let t = gid.x;\n\
         \x20   let count = step.counts.x;\n\
         \x20   let capacity = step.counts.z;\n\
         \x20   let launched = min(count, capacity);\n\
         \x20   if t == 0u {\n\
         \x20       if (step.counts.w & FLAG_LAST) != 0u {\n\
         \x20           atomicStore(&state.counters[C_ALIVE], 0u);\n\
         \x20       }\n\
         \x20       if count > launched {\n\
         \x20           atomicAdd(&state.counters[C_DROPPED], count - launched);\n\
         \x20       }\n\
         \x20   }\n\
         \x20   if t >= launched {\n\
         \x20       return;\n\
         \x20   }\n\
         \x20   let prev = bitcast<i32>(atomicSub(&state.counters[C_DEAD], 1u));\n\
         \x20   if prev <= 0 {\n\
         \x20       atomicAdd(&state.counters[C_DEAD], 1u);\n\
         \x20       atomicAdd(&state.counters[C_DROPPED], 1u);\n\
         \x20       return;\n\
         \x20   }\n\
         \x20   let slot = state.words[u32(prev - 1)];\n\
         \x20   birth(slot, step.counts.y + t, false, vec3<f32>(0.0), vec3<f32>(0.0), vec4<f32>(1.0), false);\n\
         }\n\n",
    );
}

/// `deliver`, for a target: one thread per event in the inbox, each spawning
/// the event's children from the dead list (counting as drops those it finds
/// no room for, and the children an overflowing inbox refused). Runs after
/// every parent's `update` in the step, which is when the CPU's
/// `EffectSim::deliver` runs, and so in topological order: a child's own
/// birth events reach its targets' inboxes before their `deliver`.
fn write_deliver(src: &mut String, config: &GpuEmitterConfig) {
    if !config.target {
        return;
    }
    src.push_str(
        "// A sub-emitter's children, from this emitter's inbox; see codegen.rs's\n\
         // `write_deliver`.\n\
         @compute @workgroup_size(64)\n\
         fn deliver(@builtin(global_invocation_id) gid: vec3<u32>) {\n\
         \x20   let base = step.sub.x;\n\
         \x20   if base == NO_INBOX {\n\
         \x20       return;\n\
         \x20   }\n\
         \x20   let t = gid.x;\n\
         \x20   if t == 0u {\n\
         \x20       let over = atomicLoad(&events[base + 1u]);\n\
         \x20       if over > 0u {\n\
         \x20           atomicAdd(&state.counters[C_DROPPED], over);\n\
         \x20       }\n\
         \x20   }\n\
         \x20   let n = min(atomicLoad(&events[base]), step.sub.y);\n\
         \x20   if t >= n {\n\
         \x20       return;\n\
         \x20   }\n\
         \x20   let e = base + EVENT_HEADER_WORDS + t * EVENT_WORDS;\n\
         \x20   let origin = event_vec3(e);\n\
         \x20   let count = min(atomicLoad(&events[e + 3u]), step.counts.z);\n\
         \x20   let ovel = event_vec3(e + 4u);\n\
         \x20   let flags = atomicLoad(&events[e + 7u]);\n\
         \x20   let ocol = vec4<f32>(event_vec3(e + 8u), bitcast<f32>(atomicLoad(&events[e + 11u])));\n\
         \x20   let parent = atomicLoad(&events[e + 12u]);\n\
         \x20   let key = atomicLoad(&events[e + 13u]);\n\
         \x20   let index = atomicLoad(&events[e + 14u]);\n\
         \x20   for (var k = 0u; k < count; k = k + 1u) {\n\
         \x20       let prev = bitcast<i32>(atomicSub(&state.counters[C_DEAD], 1u));\n\
         \x20       if prev <= 0 {\n\
         \x20           atomicAdd(&state.counters[C_DEAD], 1u);\n\
         \x20           atomicAdd(&state.counters[C_DROPPED], count - k);\n\
         \x20           return;\n\
         \x20       }\n\
         \x20       let slot = state.words[u32(prev - 1)];\n\
         \x20       birth(slot, child_id(parent, key, index, k), true, origin, ovel, ocol, (flags & 1u) != 0u);\n\
         \x20   }\n\
         }\n\n",
    );
}

fn write_update(
    src: &mut String,
    compiled: &CompiledEmitter,
    pool: &mut ConstPool,
    tables: &Tables,
    config: &GpuEmitterConfig,
    extras: &Extras,
) {
    let space = config.space;
    let update = &compiled.update;
    src.push_str(
        "// Every slot, every step: age, die, move, collide. A live slot is added to\n\
         // the alive list on the frame's last step, which is what the sort and the\n\
         // live count read.\n\
         @compute @workgroup_size(64)\n\
         fn update(@builtin(global_invocation_id) gid: vec3<u32>) {\n\
         \x20   let slot = gid.x;\n\
         \x20   if slot >= step.counts.z {\n\
         \x20       return;\n\
         \x20   }\n\
         \x20   var p = particles[slot];\n\
         \x20   if p.lifetime < 0.0 {\n\
         \x20       return;\n\
         \x20   }\n\
         \x20   let dt = step.time.x;\n\
         \x20   p.age = p.age + dt;\n\
         \x20   if p.age > p.lifetime {\n",
    );
    // A death by age is raised where it was last seen, before this step
    // would have moved it, as `EmitterSim::update` raises it.
    write_raises(src, config, extras, fires_on_death, "p.position", "p.velocity", "p.color", "p.id", "0u", "        ");
    src.push_str("        free_slot(slot);\n        return;\n    }\n");
    write_program(src, update, Context::Update, pool, tables);
    let o = |pin, fallback| out(update, Context::Update, pin, fallback);
    let _ = writeln!(
        src,
        "    let a = san3({}.xyz + {}.xyz);",
        o(SinkPin::Acceleration, "vec4<f32>(0.0)"),
        o(SinkPin::Gravity, "vec4<f32>(0.0)")
    );
    let _ = writeln!(src, "    let d = max({}.x, 0.0);", o(SinkPin::Drag, "vec4<f32>(0.0)"));
    src.push_str("    var v = p.velocity + a * dt;\n    v = v / (1.0 + san(d) * dt);\n");
    let _ = writeln!(src, "    let cap = san({}.x);", o(SinkPin::MaxSpeed, "vec4<f32>(0.0)"));
    src.push_str(
        "    if cap > 0.0 && dot(v, v) > cap * cap {\n        v = normalize_or_zero(v) * cap;\n    }\n\
         \x20   let p0 = p.position;\n    var p1 = p0 + v * dt;\n    var dead = false;\n",
    );
    for (n, c) in compiled.colliders.iter().enumerate() {
        write_collider(src, n, c, space);
    }
    src.push_str("    p.velocity = v;\n    p.position = p1;\n    p.rotation = p.rotation + p.angular_velocity * dt;\n");
    let _ = writeln!(src, "    p.size = p.base_size * {}.xy;", o(SinkPin::SizeOverLife, "vec4<f32>(1.0)"));
    let _ = writeln!(src, "    let lit = p.base_color * {};", o(SinkPin::ColorOverLife, "vec4<f32>(1.0)"));
    // Emission scales the light and leaves the coverage, as on the CPU.
    let _ = writeln!(src, "    p.color = vec4<f32>(lit.xyz * san({}.x), lit.w);", o(SinkPin::Emission, "vec4<f32>(1.0)"));
    // Intervals after the move, for every particle the step leaves alive or
    // a collider is about to kill, then the kill: the CPU's order.
    for (r, rule) in config.rules.iter().enumerate() {
        if let SubEmitterTrigger::Interval(_) = rule.trigger {
            let vals = extras.rules[r].1;
            let _ = writeln!(src, "    {{\n        let span = interval_range(p.age - dt, p.age, data[{vals}u].y);");
            src.push_str("        for (var k = span.x; k < span.y; k = k + 1u) {\n");
            let one = GpuEmitterConfig { rules: vec![*rule], ..config.clone() };
            let shifted = Extras { inherit: None, min_distance: None, rules: vec![extras.rules[r]] };
            write_raises(src, &one, &shifted, |_| true, "p.position", "p.velocity", "p.color", "p.id", "k", "            ");
            src.push_str("        }\n    }\n");
        }
    }
    src.push_str("    if dead {\n");
    write_raises(src, config, extras, fires_on_death, "p.position", "p.velocity", "p.color", "p.id", "0u", "        ");
    src.push_str("        free_slot(slot);\n        return;\n    }\n");
    if let (Some((points, _)), Some(md)) = (config.trail, extras.min_distance) {
        let _ = writeln!(src, "    trail_record(slot, {}u, {}u, p.position, data[{md}u].x);", trail_slot(points), points.max(1));
    }
    src.push_str(
        "    particles[slot] = p;\n\
         \x20   if (step.counts.w & FLAG_LAST) != 0u {\n\
         \x20       let k = atomicAdd(&state.counters[C_ALIVE], 1u);\n\
         \x20       state.words[alive_base() + k] = slot;\n\
         \x20   }\n}\n",
    );
}

/// One collider, unrolled in node order. The space change is the
/// interpreter's: into the collider's space for the test and back out after,
/// skipped when it needs an inverse the entity does not have.
fn write_collider(src: &mut String, n: usize, c: &crate::ir::ColliderIr, space: SimulationSpace) {
    let r = |reg: Reg| reg_name(Context::Update, reg);
    let shape = match c.shape {
        ColliderShape::Plane => 0,
        ColliderShape::Sphere => 1,
        ColliderShape::Box => 2,
        // The scene is asked on the CPU, with the world in hand; a kernel
        // cannot. `select::gpu_blockers` keeps such an emitter on the CPU, so
        // this only runs for a shader generated anyway (a test, a tool), which
        // then simply has no scene to hit.
        ColliderShape::Scene => {
            let _ = writeln!(src, "    // collider {n} (node {}): the scene, CPU only", c.node);
            return;
        }
    };
    // (into the collider's space, back out), or `None` when the particle is
    // already in it: entity geometry for a Local emitter, world geometry for
    // a World one.
    let change = match (c.world, space) {
        (false, SimulationSpace::Local) | (true, SimulationSpace::World) => None,
        (false, SimulationSpace::World) => Some(("step.inverse", "step.transform")),
        (true, SimulationSpace::Local) => Some(("step.transform", "step.inverse")),
    };
    let _ = writeln!(src, "    // collider {n} (node {})", c.node);
    let guard = if change.is_some() { " && (step.counts.w & FLAG_INVERSE_OK) != 0u" } else { "" };
    let _ = writeln!(src, "    if !dead{guard} {{");
    match change {
        Some((there, _)) => {
            let _ = writeln!(
                src,
                "        let q0 = point3({there}, p0);\n        let q1 = point3({there}, p1);\n        let w = vector3({there}, v);\n        let adt = vector3({there}, a * dt);"
            );
        }
        None => src.push_str("        let q0 = p0;\n        let q1 = p1;\n        let w = v;\n        let adt = a * dt;\n"),
    }
    let _ = writeln!(
        src,
        "        let hit = collide({shape}u, {}, {}, {}, {}, {}.x, {}.x, q0, q1, w, adt);",
        c.contain,
        c.kill,
        r(c.a),
        r(c.b),
        r(c.bounce),
        r(c.friction)
    );
    src.push_str("        if hit.hit != 0u {\n");
    match change {
        Some((_, back)) => {
            let _ = writeln!(src, "            p1 = san3(point3({back}, hit.p1));\n            v = san3(vector3({back}, hit.v));");
        }
        None => src.push_str("            p1 = san3(hit.p1);\n            v = san3(hit.v);\n"),
    }
    src.push_str("            if hit.hit == 2u {\n                dead = true;\n            }\n        }\n    }\n");
}
