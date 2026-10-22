//! The GPU backend's memory: what each buffer holds, at which offset, and how
//! CPU-side data is packed into it. Pure data and arithmetic, so the tests can
//! hold every size and offset to the WGSL that reads it without a GPU.
//!
//! ## Particles: one packed struct per slot
//!
//! A particle is one 96-byte struct (`Particle` in `library.wgsl`) rather than
//! a column per attribute. Every kernel and the vertex shader read nearly all
//! of a particle at once, so a struct is one or two cache lines per particle
//! where columns would be ten scattered reads; and it is one binding, where
//! columns would need ten, past the eight storage buffers a vertex stage is
//! guaranteed. The CPU keeps columns because its interpreter wants the
//! opposite: one attribute of a thousand particles at a time.
//!
//! ## State: counters, then lists
//!
//! One `u32` buffer per emitter: [`STATE_HEADER_WORDS`] words of counters
//! (live, free, dropped, the reset serial, the frame's light and bounds, the
//! simulation-to-mesh transform the vertex shader reads), then the dead list
//! (`capacity` words), the alive list (`capacity`), the sort keys and the
//! sorted indices (each [`sort_size`] words). The first
//! [`READBACK_WORDS`] are read back every frame for the stats chip, the
//! light and the bounds, a frame late.
//!
//! An emitter drawn as a `Trail` ribbon appends its history to the same
//! buffer ([`trail_word`]): per slot a four-word header (the ring's head and
//! length) and then `points` positions of four words each, the CPU's
//! `ParticleBuffer::trail` ring slot for slot. In the state buffer rather
//! than a buffer of its own because the vertex shader already binds this one:
//! a trail costs the particle material no binding, which is what lets the
//! same material draw quads, ribbons and tubes.
//!
//! ## Events: one buffer per effect
//!
//! Sub-emitter events between GPU emitters go through one storage buffer
//! shared by every GPU emitter of the effect ([`EventLayout`]): an inbox per
//! target emitter, each a four-word header (the events raised this step, and
//! the children refused because the inbox was full) and then
//! [`EVENT_WORDS`]-word entries. A parent's kernel appends to its target's
//! inbox with one atomic add; the target's `deliver` kernel spawns the
//! children and `clear_inbox` empties it for the next step. An inbox holds at
//! most [`MAX_GPU_EVENTS`] events a step and drops (and counts) the rest, so a
//! burst of deaths can never write past it.
//!
//! ## Data: constants and tables
//!
//! The graph's constants, curve keys and gradient stops, as `vec4<f32>`s
//! ([`pack_data`]); see the codegen module docs for why they are data rather
//! than shader text. Keys are kept exactly as the IR has them, not baked to a
//! fixed resolution, and sampled with the same search and easing as
//! `crate::curve`, so a curve's hard step lands on the same particle on both
//! backends.

use bevy::math::{Affine3A, Mat4, UVec4, Vec3, Vec4};
use bevy::render::render_resource::ShaderType;

use crate::curve::Ease;
use crate::ir::CompiledEmitter;
use crate::sim::{FrameInput, GpuStepPlan};

/// Bytes per particle; `Particle` in `library.wgsl`.
pub const PARTICLE_STRIDE: u64 = 96;

/// Keyed-pin slots a step carries; `Step::params` in `library.wgsl`. An
/// emitter keying more runs on the CPU.
pub const MAX_GPU_PARAMS: usize = 16;

/// The largest pool the GPU backend allocates. A million particles is 96 MB
/// of particles plus its lists, and past it a single dispatch of the update
/// kernel would need more workgroups than a dimension is guaranteed.
pub const MAX_GPU_CAPACITY: u32 = 1 << 20;

/// Counter words ahead of the lists in the state buffer.
pub const STATE_HEADER_WORDS: u32 = 64;
/// The state binding's `min_binding_size`. The kernels see the buffer as
/// `struct State { counters: array<atomic<u32>, 64>, words: array<u32> }`,
/// and wgpu sizes a struct ending in a runtime array with one element of it,
/// so the minimum is the header plus one word. Declaring the bare header
/// (256) made every compute pipeline fail validation the moment a GPU
/// emitter appeared ("Buffer structure size 260 ... greater than the given
/// `min_binding_size`, which is 256"), which is unrecoverable and closed the
/// editor. Every real state buffer holds the header and lists, so it always
/// meets this.
pub const STATE_MIN_BINDING_BYTES: u64 = (STATE_HEADER_WORDS as u64 + 1) * 4;
/// The counters the CPU reads back each frame.
pub const READBACK_WORDS: u32 = 16;

/// Header word indices, as `library.wgsl` names them.
pub const C_ALIVE: usize = 0;
pub const C_DEAD: usize = 1;
pub const C_DROPPED: usize = 2;
pub const C_SERIAL: usize = 3;
pub const C_LIGHT: usize = 4;
pub const C_MIN: usize = 8;
pub const C_MAX_RADIUS: usize = 11;
pub const C_MAX: usize = 12;
pub const C_MAX_SPEED: usize = 15;
/// Sixteen floats, column-major: simulation space to the renderer's mesh
/// space, written by the CPU each frame and read by the vertex shader.
pub const C_TO_MESH: usize = 16;
/// A `Connect` ribbon's live particles per strip, one word per group from
/// here to the end of the header, counted by the `sort_keys` kernel and read
/// by the vertex shader to place each point along its strip.
pub const C_GROUPS: usize = 32;
/// The most `Connect` strips the GPU draws: one header word each.
pub const MAX_GPU_RIBBON_GROUPS: u32 = STATE_HEADER_WORDS - C_GROUPS as u32;

/// Step flags (`Step::counts.w`).
pub const FLAG_LAST: u32 = 1;
pub const FLAG_INVERSE_OK: u32 = 2;

/// The fixed-point scale of the light sums; `LIGHT_SCALE` in `kernels.wgsl`.
pub const LIGHT_SCALE: f32 = 64.0;

/// What the `sort_keys` kernel orders the alive list by (`Step::sub.z`).
/// Zero runs no sort at all.
pub const SORT_NONE: u32 = 0;
/// Back to front from the camera.
pub const SORT_DEPTH: u32 = 1;
/// Oldest first.
pub const SORT_OLDEST: u32 = 2;
/// Youngest first.
pub const SORT_YOUNGEST: u32 = 3;
/// A `Connect` ribbon: by strip, newest first within each.
pub const SORT_CONNECT: u32 = 4;

/// Words per event entry; `EVENT_WORDS` in `library.wgsl`. Position and
/// child count, velocity and flags, colour, then the parent's spawn index,
/// the rule's key and the event's index, from which the children's spawn
/// indices are hashed.
pub const EVENT_WORDS: u32 = 16;
/// Words ahead of an inbox's entries: the event count and the overflow.
pub const EVENT_HEADER_WORDS: u32 = 4;
/// The most events one inbox takes in one step. Past it events are dropped
/// and their children counted as the target's drops; a step that kills
/// eight thousand parents at once has long since filled any target anyway.
pub const MAX_GPU_EVENTS: u32 = 8192;
/// `Step::sub.x` for an emitter nothing targets.
pub const NO_INBOX: u32 = u32::MAX;

/// The bitonic sort's length: the pool rounded up to a power of two.
pub fn sort_size(capacity: u32) -> u32 {
    capacity.max(1).next_power_of_two()
}

/// The state buffer's size in bytes for a pool of `capacity`, without a
/// trail history.
pub fn state_bytes(capacity: u32) -> u64 {
    state_bytes_with_trail(capacity, 0)
}

/// The state buffer's size in bytes for a pool of `capacity` keeping
/// `trail_points` positions per slot (0 for none).
pub fn state_bytes_with_trail(capacity: u32, trail_points: u32) -> u64 {
    let words = u64::from(trail_word(capacity)) + trail_words(capacity, trail_points);
    words * 4
}

/// The absolute word index of the trail history: after the sorted list.
pub fn trail_word(capacity: u32) -> u32 {
    STATE_HEADER_WORDS + 2 * capacity + 2 * sort_size(capacity)
}

/// Words one slot's trail takes: its header and its ring.
pub fn trail_slot_words(trail_points: u32) -> u32 {
    if trail_points == 0 {
        0
    } else {
        4 * (trail_points + 1)
    }
}

/// Words the whole trail history takes.
pub fn trail_words(capacity: u32, trail_points: u32) -> u64 {
    u64::from(capacity) * u64::from(trail_slot_words(trail_points))
}

/// The events an inbox holds for a target of `capacity`: no more than the
/// target could ever spawn children for in one step, and no more than
/// [`MAX_GPU_EVENTS`].
pub fn inbox_capacity(capacity: u32) -> u32 {
    capacity.clamp(1, MAX_GPU_EVENTS)
}

/// Where each target's inbox sits in an effect's event buffer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EventLayout {
    /// `(base word, capacity)` per emitter slot of the effect; `None` for an
    /// emitter nothing on the GPU targets.
    pub inboxes: Vec<Option<(u32, u32)>>,
    /// The buffer's length in words, never zero.
    pub words: u32,
}

impl EventLayout {
    /// Inboxes for the emitters whose `capacity` is `Some` (the GPU targets),
    /// one after another in slot order.
    pub fn new(targets: &[Option<u32>]) -> Self {
        let mut words = 0u32;
        let inboxes = targets
            .iter()
            .map(|t| {
                t.map(|capacity| {
                    let cap = inbox_capacity(capacity);
                    let base = words;
                    words += EVENT_HEADER_WORDS + cap * EVENT_WORDS;
                    (base, cap)
                })
            })
            .collect();
        Self { inboxes, words: words.max(EVENT_HEADER_WORDS) }
    }

    pub fn bytes(&self) -> u64 {
        u64::from(self.words) * 4
    }
}

/// The particle buffer's size in bytes.
pub fn particle_bytes(capacity: u32) -> u64 {
    u64::from(capacity.max(1)) * PARTICLE_STRIDE
}

/// The absolute word index of the sorted list, for the vertex shader.
pub fn sorted_word(capacity: u32) -> u32 {
    STATE_HEADER_WORDS + 2 * capacity + sort_size(capacity)
}

/// One step as the kernels read it; `Step` in `library.wgsl`. 496 bytes,
/// which a dynamic uniform buffer pads to 512.
#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub struct GpuStep {
    /// x step length, y seconds into the cycle, z wind turbulence.
    pub time: Vec4,
    /// x spawn count, y first spawn index, z capacity, w flags.
    pub counts: UVec4,
    /// x seed low word, y seed high word, z sort size, w reset serial.
    pub seed: UVec4,
    pub emitter_velocity: Vec4,
    /// The world wind in the emitter's simulation space.
    pub wind: Vec4,
    /// The sorting camera in simulation space.
    pub camera: Vec4,
    pub transform: Mat4,
    pub inverse: Mat4,
    pub params: [Vec4; MAX_GPU_PARAMS],
    /// x this emitter's inbox (base word, or [`NO_INBOX`]), y its capacity,
    /// z the sort mode (`SORT_*`), w the next spawn index (a `Connect`
    /// ribbon's newest particle is the one before it).
    pub sub: UVec4,
}

/// What every step of one emitter shares.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmitterConsts {
    pub capacity: u32,
    pub seed: u64,
    pub serial: u32,
    /// The emitter's inbox in its effect's event buffer, `(base, capacity)`.
    pub inbox: Option<(u32, u32)>,
    /// `SORT_*`.
    pub sort_mode: u32,
    /// A `Connect` ribbon's strips, which the frame record carries in
    /// `counts.y` for `sort_keys` (a frame record spawns nothing, so the
    /// first spawn index it would otherwise hold means nothing there).
    pub groups: u32,
}

impl Default for EmitterConsts {
    fn default() -> Self {
        Self { capacity: 0, seed: 0, serial: 0, inbox: None, sort_mode: SORT_NONE, groups: 1 }
    }
}

impl GpuStep {
    /// The record the fixed kernels read: no spawn, no step, the camera, and
    /// the spawn index the next step would start at.
    pub fn frame(consts: EmitterConsts, frame: &FrameInput, camera: Vec3, next_id: u32) -> Self {
        let mut s = Self::base(consts, frame);
        s.camera = camera.extend(1.0);
        s.counts.y = consts.groups.max(1);
        s.sub.w = next_id;
        s
    }

    /// One planned step. `last` marks the frame's last executed step, the one
    /// that rebuilds the alive list.
    pub fn from_plan(consts: EmitterConsts, plan: &GpuStepPlan, last: bool) -> Self {
        let mut s = Self::base(consts, &plan.frame);
        s.time = Vec4::new(plan.dt, plan.emitter_time, plan.frame.wind_turbulence, 0.0);
        s.counts.x = plan.spawn_count;
        s.counts.y = plan.first_id;
        if last {
            s.counts.w |= FLAG_LAST;
        }
        s.wind = plan.wind.extend(0.0);
        for (slot, value) in s.params.iter_mut().zip(&plan.params) {
            *slot = *value;
        }
        s.sub.w = plan.first_id.wrapping_add(plan.spawn_count);
        s
    }

    fn base(consts: EmitterConsts, frame: &FrameInput) -> Self {
        let transform = Mat4::from(frame.transform);
        let inverse = frame.transform.inverse();
        let inverse_ok = inverse.is_finite();
        let inverse = if inverse_ok { Mat4::from(inverse) } else { Mat4::IDENTITY };
        Self {
            time: Vec4::new(0.0, 0.0, frame.wind_turbulence, 0.0),
            counts: UVec4::new(0, 0, consts.capacity, if inverse_ok { FLAG_INVERSE_OK } else { 0 }),
            seed: UVec4::new(consts.seed as u32, (consts.seed >> 32) as u32, sort_size(consts.capacity), consts.serial),
            emitter_velocity: frame.emitter_velocity.extend(0.0),
            wind: frame.wind.extend(0.0),
            camera: Vec4::ZERO,
            transform,
            inverse,
            params: [Vec4::ZERO; MAX_GPU_PARAMS],
            sub: match consts.inbox {
                Some((base, cap)) => UVec4::new(base, cap, consts.sort_mode, 0),
                None => UVec4::new(NO_INBOX, 0, consts.sort_mode, 0),
            },
        }
    }
}

/// The bitonic sort's passes for a sort of `n` (a power of two), in order:
/// `(j, k)` for every block size `k` from 2 to `n` and partner distance `j`
/// from `k / 2` down to 1. The list for `n` is a prefix of the list for `2n`,
/// which is what lets one table of passes serve every emitter.
pub fn sort_passes(n: u32) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let mut k = 2;
    while k <= n {
        let mut j = k / 2;
        while j > 0 {
            out.push((j, k));
            j /= 2;
        }
        k *= 2;
    }
    out
}

/// How many passes [`sort_passes`] lists for `n`: `m (m + 1) / 2` for
/// `n = 2^m`.
pub fn sort_pass_count(n: u32) -> usize {
    let m = n.max(1).next_power_of_two().trailing_zeros() as usize;
    m * (m + 1) / 2
}

/// `ease` in `library.wgsl` reads the curve's easing as this number.
pub fn ease_code(ease: Ease) -> f32 {
    match ease {
        Ease::Linear => 0.0,
        Ease::Step => 1.0,
        Ease::Smooth => 2.0,
        Ease::EaseIn => 3.0,
        Ease::EaseOut => 4.0,
        Ease::EaseInOut => 5.0,
    }
}

/// The data table for `compiled` and the constants its shader reads, in the
/// order the codegen indexed them: constants, the curves' directory (one
/// `(offset, count)` per curve), the gradients' directory, then the keys. A
/// curve key is `(t, value, ease, 0)`; a gradient stop is `(t, 0, 0, 0)` and
/// then its colour. Never empty: a binding of zero bytes is invalid.
pub fn pack_data(compiled: &CompiledEmitter, consts: &[[u32; 4]]) -> Vec<[f32; 4]> {
    let mut out: Vec<[f32; 4]> = consts.iter().map(|c| c.map(f32::from_bits)).collect();
    let dirs = out.len();
    out.resize(dirs + compiled.curves.len() + compiled.gradients.len(), [0.0; 4]);
    for (i, curve) in compiled.curves.iter().enumerate() {
        out[dirs + i] = [out.len() as f32, curve.points.len() as f32, 0.0, 0.0];
        out.extend(curve.points.iter().map(|p| [p.t, p.value, ease_code(p.ease), 0.0]));
    }
    let gdirs = dirs + compiled.curves.len();
    for (i, gradient) in compiled.gradients.iter().enumerate() {
        out[gdirs + i] = [out.len() as f32, gradient.stops.len() as f32, 0.0, 0.0];
        for s in &gradient.stops {
            out.push([s.t, 0.0, 0.0, 0.0]);
            out.push(s.color);
        }
    }
    if out.is_empty() {
        out.push([0.0; 4]);
    }
    out
}

/// `order_bits` in `kernels.wgsl`: floats to u32s that sort the same way.
pub fn order_bits(f: f32) -> u32 {
    let b = f.to_bits();
    if b & 0x8000_0000 != 0 {
        !b
    } else {
        b | 0x8000_0000
    }
}

/// The inverse of [`order_bits`].
pub fn unorder_bits(u: u32) -> f32 {
    if u & 0x8000_0000 != 0 {
        f32::from_bits(u & 0x7FFF_FFFF)
    } else {
        f32::from_bits(!u)
    }
}

/// The header words the CPU writes before the frame's stats kernel: light
/// sums at zero, the minimums at the top of the order and the maximums at the
/// bottom, so the first particle sets both.
pub fn cleared_stats() -> [u32; 12] {
    let mut w = [0u32; 12];
    for k in 0..3 {
        w[C_MIN - C_LIGHT + k] = u32::MAX;
        w[C_MAX - C_LIGHT + k] = 0;
    }
    w[C_MAX_RADIUS - C_LIGHT] = 0;
    w[C_MAX_SPEED - C_LIGHT] = 0;
    w
}

/// One emitter's numbers as read back from its state buffer.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GpuReadout {
    pub alive: u32,
    pub dropped: u32,
    pub serial: u32,
    /// Summed light weight and weighted colour; see `crate::light`.
    pub light_weight: f32,
    pub light_rgb: Vec3,
    /// Simulation-space bounds of the live particles, the largest particle
    /// radius and the fastest speed; `None` when nothing was alive.
    pub bounds: Option<(Vec3, Vec3, f32, f32)>,
}

impl GpuReadout {
    /// Decode the first [`READBACK_WORDS`] words of a state buffer.
    pub fn decode(words: &[u32]) -> Option<Self> {
        if words.len() < READBACK_WORDS as usize {
            return None;
        }
        let fixed = |w: u32| w as f32 / LIGHT_SCALE;
        let lo = Vec3::new(unorder_bits(words[C_MIN]), unorder_bits(words[C_MIN + 1]), unorder_bits(words[C_MIN + 2]));
        let hi = Vec3::new(unorder_bits(words[C_MAX]), unorder_bits(words[C_MAX + 1]), unorder_bits(words[C_MAX + 2]));
        let bounds = (lo.cmple(hi).all() && lo.is_finite() && hi.is_finite())
            .then(|| (lo, hi, unorder_bits(words[C_MAX_RADIUS]).max(0.0), unorder_bits(words[C_MAX_SPEED]).max(0.0)));
        Some(Self {
            alive: words[C_ALIVE],
            dropped: words[C_DROPPED],
            serial: words[C_SERIAL],
            light_weight: fixed(words[C_LIGHT]),
            light_rgb: Vec3::new(fixed(words[C_LIGHT + 1]), fixed(words[C_LIGHT + 2]), fixed(words[C_LIGHT + 3])),
            bounds,
        })
    }
}

/// The sixteen floats of a simulation-to-mesh transform, as the vertex shader
/// reads them from [`C_TO_MESH`].
pub fn to_mesh_words(to_mesh: Affine3A) -> [u32; 16] {
    Mat4::from(to_mesh).to_cols_array().map(f32::to_bits)
}
