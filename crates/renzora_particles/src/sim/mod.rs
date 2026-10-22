//! The CPU backend: structure-of-arrays particle storage and the step that
//! drives the compiled programs over it.
//!
//! Runs everywhere, including wasm on WebGL2, which has no compute shaders and
//! is the reason a CPU backend exists at all. A GPU backend compiled from the
//! same IR is a later step; nothing here is shaped around it except that all
//! per-particle work goes through the interpreter, so there is no behaviour the
//! GPU would have to reproduce from Rust code.
//!
//! ## Time
//!
//! Stepping is explicit. [`EffectSim::step`] advances by exactly the `dt` it is
//! given, [`EffectSim::advance`] splits a frame's delta into bounded steps for
//! normal play, and [`EffectSim::simulate_to`] replays from zero in fixed
//! [`SEEK_STEP`]s. Because every random is a hash of the particle's spawn index
//! (see [`crate::random`]) and nothing else carries state between steps except
//! the particles themselves, stepping `n` times by `SEEK_STEP` and seeking to
//! `n * SEEK_STEP` produce identical particles. That is the property the editor
//! timeline will scrub on.
//!
//! ## Space
//!
//! A `Local` emitter keeps particles in the entity's space and the renderer
//! draws them through the entity's transform, so they follow it. A `World`
//! emitter transforms each particle into world space at birth and never again,
//! so a moving emitter leaves particles behind. A `World` emitter adds its
//! `inherit_velocity` share of the entity's velocity to each own spawn; a
//! `Local` one never does (it already carries them), and a graph that wants
//! more adds `input/emitter_velocity` to its spawn velocity.
//!
//! ## Wind
//!
//! [`FrameInput::wind`] is the world wind this frame, gust included (the
//! runtime fills it from `renzora::wind::WindState`, dead calm without one).
//! Each emitter sees it in its own simulation space, so a `Local` emitter on an
//! entity turned to face the wind is pushed backwards, not sideways. Wind is
//! an input, not state: [`EffectSim::simulate_to`] replays every step with the
//! wind it is given *now*, because no history of past wind is kept. A seek is
//! therefore exact in calm air and an approximation in a gusting one, where the
//! replayed smoke drifts on today's wind rather than the wind it was emitted
//! into. Nothing scrubs a timeline against live weather, so that is the right
//! trade against carrying a wind log per effect.
//!
//! ## After integration
//!
//! Three things happen to a particle once it has moved, in this order: the
//! output's `max_speed` caps its velocity (before the move, so the step itself
//! obeys it), the collision nodes push it back out of what it ran into
//! ([`collide`]), and an emitter drawn as a trail keeps its new position in a
//! small per-particle ring ([`ParticleBuffer::record_trail`]). Then every
//! sub-emitter event the step raised (births, deaths, intervals) is delivered
//! in a fixed order ([`sub`]). All of it is a function of the particles and
//! the step alone, so a seek still replays exactly.
//!
//! The one exception is a Collide Scene node, which needs the scene's
//! colliders and so the world. The step only remembers what the hit will
//! need; the runtime casts every particle's motion since it was last checked
//! in one batch after the frame's steps ([`scene`]). A seek has no world to
//! ask and replays without scene hits.
//!
//! ## GPU emitters
//!
//! An emitter the GPU backend runs ([`crate::gpu`]) keeps its clock here and
//! nothing else. Its step still fills the tracks and runs the spawn program,
//! because both are per emitter and cheap, and because bursts, keyed bursts
//! and the spawn accumulator are then exactly the CPU's; but instead of
//! touching particles it records a [`GpuStepPlan`] (how long, how many to
//! spawn, which spawn indices, the parameters) for the render world to
//! execute. Its particle buffer stays empty; its live count comes back from
//! the GPU a frame late ([`GpuEmitterState::readout`]). A GPU emitter's
//! sub-emitter events never pass through here: they are raised and delivered
//! on the GPU, between emitters [`EffectSim::with_backends`] has put on one
//! backend, and a GPU target is planned an empty step for every step its
//! parents take so the render world can run them in lockstep.

pub mod collide;
pub mod interp;
pub mod scene;
pub mod sub;

use std::sync::Arc;

use bevy::math::{Affine3A, Vec2, Vec3, Vec4};

use crate::asset::{Facing, RibbonKind, SimulationSpace, SubEmitterTrigger, VfxAsset};
use crate::compile::{compile_graph_with_params, CompileError};
use crate::ir::{CompiledEmitter, SinkPin};
use crate::params::{self, EmitterTracks};
use interp::{as_vec2, as_vec3, eval, EvalInputs, Registers};

/// Fixed step `simulate_to` replays with.
pub const SEEK_STEP: f32 = 1.0 / 60.0;
/// Most steps one `simulate_to` may take. A longer seek uses proportionally
/// longer steps, trading accuracy for a bounded cost: scrubbing to minute ten
/// of a looping fire must not stall the editor for a second.
pub const MAX_SEEK_STEPS: u32 = 1200;
/// [`MAX_SEEK_STEPS`] for an effect with a GPU emitter: ten seconds of seek
/// steps, which the render world runs in one frame (two dispatches a step).
/// A longer seek takes longer steps rather than spreading over frames, so a
/// scrub drawn mid-seek never shows a time the playhead is not at.
pub const GPU_MAX_SEEK_STEPS: u32 = 600;
/// Longest single step normal play takes; a longer frame is split. Large steps
/// make drag and forces overshoot.
pub const MAX_PLAY_STEP: f32 = 1.0 / 30.0;
/// Longest frame normal play honours at all. A hitch (a breakpoint, a window
/// drag) would otherwise dump seconds of particles in one frame.
pub const MAX_FRAME_DELTA: f32 = 0.25;
/// Particles evaluated per interpreter call. Bounds scratch memory to
/// `registers * BATCH * 16` bytes however large the emitter.
pub const BATCH: usize = 1024;

/// Where the effect is this step, from the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameInput {
    /// The entity's world transform. Places `World`-space particles at birth.
    pub transform: Affine3A,
    /// The entity's world-space velocity, for `input/emitter_velocity`.
    pub emitter_velocity: Vec3,
    /// The world wind in m/s, gust included. See the module docs.
    pub wind: Vec3,
    /// The world wind's turbulence, 0..1.
    pub wind_turbulence: f32,
}

impl Default for FrameInput {
    fn default() -> Self {
        Self {
            transform: Affine3A::IDENTITY,
            emitter_velocity: Vec3::ZERO,
            wind: Vec3::ZERO,
            wind_turbulence: 0.0,
        }
    }
}

impl FrameInput {
    /// A frame at `transform` with no motion and no wind.
    pub fn at(transform: Affine3A) -> Self {
        Self { transform, ..Default::default() }
    }

    /// The same frame with the world wind from `wind`.
    pub fn with_wind(mut self, wind: &renzora::wind::WindState) -> Self {
        let v = wind.velocity();
        self.wind = if v.is_finite() { v } else { Vec3::ZERO };
        self.wind_turbulence = if wind.turbulence.is_finite() { wind.turbulence.max(0.0) } else { 0.0 };
        self
    }
}

/// Particles, one column per attribute.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParticleBuffer {
    pub id: Vec<u32>,
    pub age: Vec<f32>,
    pub lifetime: Vec<f32>,
    pub position: Vec<Vec3>,
    pub velocity: Vec<Vec3>,
    pub base_size: Vec<Vec2>,
    /// `base_size * size_over_life`, as of the last step.
    pub size: Vec<Vec2>,
    pub rotation: Vec<f32>,
    pub angular_velocity: Vec<f32>,
    pub base_color: Vec<Vec4>,
    /// `base_color * color_over_life`, as of the last step.
    pub color: Vec<Vec4>,
    /// How many sub-emitter hops away from an emitter's own spawning the
    /// particle is: 0 for its own, 1 for a child, and so on. See [`sub`].
    pub generation: Vec<u8>,
    /// Kept positions of each particle's trail, `trail_stride` per particle in
    /// one flat ring per particle; empty unless the emitter draws a
    /// `RibbonKind::Trail`. See [`ParticleBuffer::trail`].
    pub trail: Vec<Vec3>,
    /// How many of a particle's ring slots hold a position.
    pub trail_len: Vec<u16>,
    /// The ring slot of a particle's newest kept position.
    pub trail_head: Vec<u16>,
    /// Ring slots per particle; 0 keeps no trails.
    pub trail_stride: usize,
    /// One per particle while `scene_tracking` is on (the emitter has a
    /// Collide Scene node), else empty. See [`scene`].
    pub scene: Vec<ScenePoint>,
    pub scene_tracking: bool,
}

/// What the scene collision pass ([`scene`]) needs to know about one particle
/// between frames. It runs after the step, so the step leaves this behind.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScenePoint {
    /// Where the particle was when the scene last saw it, in the emitter's
    /// simulation space. The next cast runs from here to where it is now, so a
    /// particle the per-frame cap skipped is still swept over its whole path
    /// when its turn comes.
    pub from: Vec3,
    /// The last step's acceleration times the step, for the resting-contact
    /// rule in [`collide::respond`].
    pub accel_dt: Vec3,
    pub bounce: f32,
    pub friction: f32,
    pub radius: f32,
}

/// A newly spawned particle's starting values.
pub(crate) struct Birth {
    pub id: u32,
    pub lifetime: f32,
    pub position: Vec3,
    pub velocity: Vec3,
    pub size: Vec2,
    pub rotation: f32,
    pub angular_velocity: f32,
    pub color: Vec4,
    pub generation: u8,
}

impl ParticleBuffer {
    pub fn len(&self) -> usize {
        self.id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.id.is_empty()
    }

    /// Remove every particle. The trail stride is a setting, not a particle,
    /// and stays.
    pub fn clear(&mut self) {
        self.id.clear();
        self.age.clear();
        self.lifetime.clear();
        self.position.clear();
        self.velocity.clear();
        self.base_size.clear();
        self.size.clear();
        self.rotation.clear();
        self.angular_velocity.clear();
        self.base_color.clear();
        self.color.clear();
        self.generation.clear();
        self.trail.clear();
        self.trail_len.clear();
        self.trail_head.clear();
        self.scene.clear();
    }

    pub(crate) fn push(&mut self, b: Birth) {
        self.id.push(b.id);
        self.age.push(0.0);
        self.lifetime.push(b.lifetime);
        self.position.push(b.position);
        self.velocity.push(b.velocity);
        self.base_size.push(b.size);
        self.size.push(b.size);
        self.rotation.push(b.rotation);
        self.angular_velocity.push(b.angular_velocity);
        self.base_color.push(b.color);
        self.color.push(b.color);
        self.generation.push(b.generation);
        if self.trail_stride > 0 {
            // The birth position is the trail's first kept point, so a trail
            // starts where the particle did rather than one step later.
            self.trail.push(b.position);
            self.trail.extend(std::iter::repeat_n(b.position, self.trail_stride - 1));
            self.trail_len.push(1);
            self.trail_head.push(0);
        }
        if self.scene_tracking {
            self.scene.push(ScenePoint { from: b.position, ..Default::default() });
        }
    }

    /// Remove particle `i` by moving the last one into its slot. Order is not
    /// preserved, but it changes the same way on every replay.
    pub(crate) fn swap_remove(&mut self, i: usize) {
        self.id.swap_remove(i);
        self.age.swap_remove(i);
        self.lifetime.swap_remove(i);
        self.position.swap_remove(i);
        self.velocity.swap_remove(i);
        self.base_size.swap_remove(i);
        self.size.swap_remove(i);
        self.rotation.swap_remove(i);
        self.angular_velocity.swap_remove(i);
        self.base_color.swap_remove(i);
        self.color.swap_remove(i);
        self.generation.swap_remove(i);
        if self.trail_stride > 0 {
            let s = self.trail_stride;
            let last = self.trail_len.len() - 1;
            if i != last {
                self.trail.copy_within(last * s..(last + 1) * s, i * s);
            }
            self.trail.truncate(last * s);
            self.trail_len.swap_remove(i);
            self.trail_head.swap_remove(i);
        }
        if self.scene_tracking {
            self.scene.swap_remove(i);
        }
    }

    /// Keep `position` as particle `i`'s newest trail point if it has moved
    /// at least `min_distance` from the last one kept, dropping the oldest
    /// once the ring is full.
    pub fn record_trail(&mut self, i: usize, position: Vec3, min_distance: f32) {
        let s = self.trail_stride;
        if s == 0 || i >= self.trail_len.len() {
            return;
        }
        let head = self.trail_head[i] as usize;
        let newest = self.trail[i * s + head];
        if self.trail_len[i] > 0 && newest.distance_squared(position) < min_distance * min_distance {
            return;
        }
        let next = (head + 1) % s;
        self.trail[i * s + next] = position;
        self.trail_head[i] = next as u16;
        self.trail_len[i] = (self.trail_len[i] + 1).min(s as u16);
    }

    /// Particle `i`'s kept trail points, newest first. Not including where the
    /// particle is now: a drawn trail starts from there and follows these.
    pub fn trail(&self, i: usize) -> impl Iterator<Item = Vec3> + '_ {
        let s = self.trail_stride;
        let (len, head) = match (self.trail_len.get(i), self.trail_head.get(i)) {
            (Some(&l), Some(&h)) if s > 0 => (l as usize, h as usize),
            _ => (0, 0),
        };
        (0..len).map(move |k| self.trail[i * s + (head + s - k) % s])
    }
}

/// One step of a GPU emitter, planned here and executed by the render world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuStepPlan {
    pub dt: f32,
    /// Seconds into the cycle at the start of the step.
    pub emitter_time: f32,
    pub spawn_count: u32,
    /// The spawn index of the first particle born this step; the rest follow
    /// in order, as on the CPU.
    pub first_id: u32,
    /// The parameter slots' values this step.
    pub params: [Vec4; crate::gpu::layout::MAX_GPU_PARAMS],
    pub frame: FrameInput,
    /// The world wind in the emitter's simulation space.
    pub wind: Vec3,
    /// A script burst: spawn, but do not step. The CPU's `burst` spawns
    /// without updating, and so does this.
    pub spawn_only: bool,
}

/// What a GPU emitter keeps on the CPU side.
#[derive(Clone, Debug, Default)]
pub struct GpuEmitterState {
    /// Steps planned since the runtime last took them.
    pub plans: Vec<GpuStepPlan>,
    /// The pool must be emptied before `plans` run.
    pub reset: bool,
    /// Counts resets, so a readback from before the latest can be ignored.
    pub serial: u32,
    /// The last readback that belongs to `serial`.
    pub readout: crate::gpu::layout::GpuReadout,
    /// A readback of `serial` has arrived, so `readout` describes the pool
    /// rather than the zeros a reset leaves until the GPU has run it. A
    /// one-shot whose kernels are still compiling must not count as finished
    /// for having no particles yet.
    pub synced: bool,
}

/// One emitter's state.
pub struct EmitterSim {
    /// Index of the emitter in the asset it came from.
    pub emitter_index: usize,
    pub compiled: Arc<CompiledEmitter>,
    pub particles: ParticleBuffer,
    pub seed: u64,
    pub capacity: u32,
    pub space: SimulationSpace,
    /// Spawns refused because the emitter was full, since the last reset.
    pub dropped: u64,
    next_id: u32,
    spawn_accumulator: f32,
    step_index: u32,
    regs: Registers,
    /// The emitter's keyed parameters, enabled keys and burst keys. Empty for
    /// an effect nobody keyed, which then steps exactly as it always did.
    pub tracks: EmitterTracks,
    /// This step's parameter slot values, filled from `tracks`.
    params: Vec<Vec4>,
    /// `RibbonKind::Trail` only: how far a particle moves before its trail
    /// keeps another point.
    pub trail_min_distance: f32,
    /// This emitter's sub-emitter rules, resolved to emitter slots.
    pub rules: Vec<sub::Rule>,
    /// Some other emitter's rules spawn this one, so it emits only what they
    /// ask for: its own spawn pins, burst keys and script bursts are ignored.
    pub driven: bool,
    /// The share of the entity's world velocity an own spawn adds to its
    /// velocity; 0 for a `Local` emitter ([`crate::asset::Emitter::inherited_velocity`]).
    /// Children of a sub-emitter rule take the parent's velocity instead.
    pub inherit_velocity: f32,
    /// The fewest and most sub-emitter generations this emitter's particles
    /// can be ([`sub::generation_spans`]); `(0, 0)` for one nothing targets.
    pub generations: (u8, u8),
    /// Events raised this step, delivered by the effect at its end.
    pending: Vec<sub::Event>,
    /// Particles a `kill` collider hit this step, removed after the update.
    killed: Vec<usize>,
    /// Which backend runs the emitter and why ([`crate::gpu::select`]).
    pub backend: crate::gpu::select::BackendChoice,
    /// `Some` while the GPU runs it: see the module docs.
    pub gpu: Option<GpuEmitterState>,
    /// Where the next scene pass starts in the particle list, so a pool larger
    /// than one frame's share of casts is covered round-robin ([`scene`]).
    scene_cursor: usize,
}

/// What one step looks like from inside an emitter.
struct StepTiming {
    /// Effect time at the start of the step, from 0, not wrapped.
    t0: f32,
    dt: f32,
    duration: f32,
    looping: bool,
}

impl StepTiming {
    /// Seconds into the current cycle at the start of the step.
    fn cycle_time(&self) -> f32 {
        if self.looping && self.duration > 0.0 {
            self.t0.rem_euclid(self.duration)
        } else {
            self.t0
        }
    }

    /// The parts of `[t0, t0 + dt)` during which the effect emits, in cycle
    /// time. Up to a few segments when a step crosses a loop boundary.
    fn emitting_segments(&self, out: &mut Vec<(f32, f32)>) {
        out.clear();
        let (t0, t1) = (self.t0, self.t0 + self.dt);
        if self.duration <= 0.0 {
            out.push((t0, t1));
            return;
        }
        if !self.looping {
            if t0 < self.duration {
                out.push((t0, t1.min(self.duration)));
            }
            return;
        }
        let d = self.duration;
        let mut cycle = (t0 / d).floor();
        // A step longer than many cycles is not a case worth exact bursts; cap
        // the walk so a nonsense duration cannot spin here.
        for _ in 0..64 {
            let start = cycle * d;
            let a = t0.max(start) - start;
            let b = t1.min(start + d) - start;
            if b > a {
                out.push((a, b));
            }
            if start + d >= t1 {
                break;
            }
            cycle += 1.0;
        }
    }
}

impl EmitterSim {
    pub fn new(emitter_index: usize, compiled: Arc<CompiledEmitter>, seed: u64, capacity: u32, space: SimulationSpace) -> Self {
        let scene_tracking = scene::first_scene_collider(&compiled).is_some();
        Self {
            emitter_index,
            compiled,
            particles: ParticleBuffer { scene_tracking, ..Default::default() },
            seed,
            capacity,
            space,
            dropped: 0,
            next_id: 0,
            spawn_accumulator: 0.0,
            step_index: 0,
            regs: Registers::default(),
            tracks: EmitterTracks::default(),
            params: Vec::new(),
            trail_min_distance: 0.0,
            rules: Vec::new(),
            driven: false,
            inherit_velocity: 0.0,
            generations: (0, 0),
            pending: Vec::new(),
            killed: Vec::new(),
            backend: crate::gpu::select::BackendChoice::default(),
            gpu: None,
            scene_cursor: 0,
        }
    }

    /// Run on the GPU (`true`) or the CPU. Either way the emitter starts over:
    /// particles do not move between the two.
    pub fn set_gpu(&mut self, on: bool) {
        self.gpu = on.then(GpuEmitterState::default);
        self.reset();
    }

    pub fn is_gpu(&self) -> bool {
        self.gpu.is_some()
    }

    /// Particles alive now: the buffer's length on the CPU, the last readback
    /// on the GPU.
    pub fn alive(&self) -> usize {
        match &self.gpu {
            Some(g) => g.readout.alive as usize,
            None => self.particles.len(),
        }
    }

    /// Spawns refused for want of room since the last reset.
    pub fn dropped_total(&self) -> u64 {
        match &self.gpu {
            Some(g) => u64::from(g.readout.dropped),
            None => self.dropped,
        }
    }

    /// Take a readback from the GPU, unless it was made before the latest
    /// reset (its serial is older), when it describes particles that no
    /// longer exist.
    pub fn apply_readout(&mut self, readout: crate::gpu::layout::GpuReadout) {
        if let Some(g) = &mut self.gpu {
            if readout.serial == g.serial {
                g.readout = readout;
                g.synced = true;
            }
        }
    }

    /// Whether [`EmitterSim::alive`] can be trusted: always on the CPU, and on
    /// the GPU once a readback since the last reset has arrived.
    pub fn settled(&self) -> bool {
        self.gpu.as_ref().is_none_or(|g| g.synced)
    }

    /// Keep a trail of `points` positions per particle, a new one each time a
    /// particle has moved `min_distance`. Clears the particles, whose rings
    /// would not fit the new stride.
    pub fn set_trail(&mut self, points: usize, min_distance: f32) {
        self.particles.clear();
        self.particles.trail_stride = points;
        self.trail_min_distance = if min_distance.is_finite() { min_distance.max(0.0) } else { 0.0 };
    }

    pub fn reset(&mut self) {
        self.particles.clear();
        self.dropped = 0;
        self.next_id = 0;
        self.spawn_accumulator = 0.0;
        self.step_index = 0;
        self.pending.clear();
        self.killed.clear();
        if let Some(g) = &mut self.gpu {
            g.plans.clear();
            g.reset = true;
            g.serial = g.serial.wrapping_add(1);
            g.readout = crate::gpu::layout::GpuReadout { serial: g.serial, ..Default::default() };
            g.synced = false;
        }
    }

    /// Record a GPU step that spawns `count` (taking spawn indices as the CPU
    /// would) and, unless `spawn_only`, steps the pool by `dt`.
    fn plan_gpu(&mut self, dt: f32, emitter_time: f32, count: u32, frame: &FrameInput, spawn_only: bool) {
        let wind = self.sim_wind(frame);
        let first_id = self.next_id;
        self.next_id = self.next_id.wrapping_add(count);
        let mut params = [Vec4::ZERO; crate::gpu::layout::MAX_GPU_PARAMS];
        for (slot, v) in params.iter_mut().zip(&self.params) {
            *slot = *v;
        }
        if let Some(g) = &mut self.gpu {
            g.plans.push(GpuStepPlan {
                dt,
                emitter_time,
                spawn_count: count,
                first_id,
                params,
                frame: *frame,
                wind,
                spawn_only,
            });
        }
    }

    /// Record `rule`'s event for particle `i` if the rule fires on `trigger`
    /// and the particle is not too many generations down to spawn.
    fn raise(&mut self, i: usize, matches: impl Fn(&sub::Rule) -> Option<std::ops::Range<u32>>) {
        let p = &self.particles;
        let generation = p.generation[i];
        if generation >= sub::MAX_SUB_DEPTH {
            return;
        }
        for (r, rule) in self.rules.iter().enumerate() {
            let Some(indices) = matches(rule) else { continue };
            for index in indices {
                self.pending.push(sub::Event {
                    rule: r,
                    parent: p.id[i],
                    index,
                    generation: generation + 1,
                    position: p.position[i],
                    velocity: p.velocity[i],
                    color: p.color[i],
                });
            }
        }
    }

    fn raise_birth(&mut self, i: usize) {
        if !self.rules.is_empty() {
            self.raise(i, |r| (r.trigger == SubEmitterTrigger::OnBirth).then_some(0..1));
        }
    }

    fn raise_death(&mut self, i: usize) {
        if !self.rules.is_empty() {
            self.raise(i, |r| (r.trigger == SubEmitterTrigger::OnDeath).then_some(0..1));
        }
    }

    fn raise_intervals(&mut self, i: usize, age_before: f32) {
        if self.rules.is_empty() {
            return;
        }
        let age = self.particles.age[i];
        self.raise(i, |r| match r.trigger {
            SubEmitterTrigger::Interval(every) => Some(sub::interval_events(age_before, age, every)),
            _ => None,
        });
    }

    /// The world wind in this emitter's simulation space. A `Local` emitter
    /// takes it through the inverse of the entity's transform, which also
    /// divides out the entity's scale, matching the units its velocities are
    /// in. An entity that cannot be inverted (zero scale) gets no wind.
    fn sim_wind(&self, frame: &FrameInput) -> Vec3 {
        if frame.wind == Vec3::ZERO || self.space == SimulationSpace::World {
            return sanitize3(frame.wind);
        }
        sanitize3(frame.transform.inverse().transform_vector3(frame.wind))
    }

    fn step(&mut self, timing: &StepTiming, frame: &FrameInput, segments: &mut Vec<(f32, f32)>) {
        // Tracks first, at the step's cycle time: every program this step
        // (and every particle born in it) reads the same values. A keyed-off
        // emitter spawns nothing but keeps aging what it already has.
        let t = timing.cycle_time();
        self.tracks.fill(t, &mut self.params);
        let own = self.tracks.enabled_at(t) && !self.driven;
        let count = if own { self.spawn_count(timing, frame, segments) } else { 0 };
        if self.gpu.is_some() {
            self.plan_gpu(timing.dt, t, count, frame, false);
        } else {
            self.spawn(count, frame, None, 0);
            self.update(timing, frame);
        }
        self.step_index = self.step_index.wrapping_add(1);
    }

    /// How many particles the spawn context asks for this step.
    fn spawn_count(&mut self, timing: &StepTiming, frame: &FrameInput, segments: &mut Vec<(f32, f32)>) -> u32 {
        timing.emitting_segments(segments);
        if segments.is_empty() {
            return 0;
        }
        let inputs = EvalInputs {
            count: 1,
            seed: self.seed,
            ids: &[],
            spawn_id: self.step_index,
            age: &[],
            lifetime: &[],
            position: &[],
            velocity: &[],
            emitter_time: timing.cycle_time(),
            delta_time: timing.dt,
            emitter_velocity: frame.emitter_velocity,
            wind: self.sim_wind(frame),
            wind_turbulence: frame.wind_turbulence,
            params: &self.params,
        };
        let compiled = self.compiled.clone();
        let program = &compiled.spawn;
        eval(program, &compiled, &inputs, &mut self.regs);
        let read = |pin: SinkPin, regs: &Registers| -> f32 {
            program
                .output(pin)
                .map(|r| regs.get(r)[0].x)
                .filter(|v| v.is_finite())
                .unwrap_or(0.0)
        };
        let rate = read(SinkPin::SpawnRate, &self.regs).max(0.0);
        let burst_count = read(SinkPin::BurstCount, &self.regs).max(0.0).round();
        let interval = read(SinkPin::BurstInterval, &self.regs);
        let cycles = read(SinkPin::BurstCycles, &self.regs).max(0.0).round() as u64;

        let active: f32 = segments.iter().map(|(a, b)| b - a).sum();
        self.spawn_accumulator += rate * active;
        let from_rate = self.spawn_accumulator.floor();
        self.spawn_accumulator -= from_rate;

        let mut bursts: u64 = 0;
        if burst_count > 0.0 {
            for &(a, b) in segments.iter() {
                bursts += bursts_in(a, b, interval, cycles);
            }
        }
        let keyed = self.tracks.burst_count(segments);
        let total = from_rate as f64 + bursts as f64 * burst_count as f64 + keyed as f64;
        total.min(u32::MAX as f64) as u32
    }

    /// Spawn `count` particles now, at the start of a step. Spawns past the
    /// capacity are counted in [`EmitterSim::dropped`] and discarded.
    ///
    /// With an `origin` they are a sub-emitter's children: the init program's
    /// position and velocity become offsets from the parent's (see
    /// [`sub::Origin`]), and `generation` is theirs.
    fn spawn(&mut self, count: u32, frame: &FrameInput, origin: Option<&sub::Origin>, generation: u8) {
        if count == 0 {
            return;
        }
        if self.gpu.is_some() {
            // Only a script burst reaches here: a GPU target's children are
            // spawned on the GPU from its event inbox, never through this,
            // and its own steps plan their spawns.
            self.plan_gpu(0.0, 0.0, count, frame, true);
            return;
        }
        let room = (self.capacity as usize).saturating_sub(self.particles.len());
        let n = (count as usize).min(room);
        self.dropped += (count as usize - n) as u64;
        if n == 0 {
            return;
        }
        let compiled = self.compiled.clone();
        let program = &compiled.init;
        let mut ids = Vec::with_capacity(BATCH.min(n));
        let mut done = 0;
        while done < n {
            let len = (n - done).min(BATCH);
            ids.clear();
            ids.extend((0..len as u32).map(|k| self.next_id.wrapping_add(k)));
            let inputs = EvalInputs {
                count: len,
                seed: self.seed,
                ids: &ids,
                spawn_id: 0,
                age: &[],
                lifetime: &[],
                position: &[],
                velocity: &[],
                emitter_time: 0.0,
                delta_time: 0.0,
                emitter_velocity: frame.emitter_velocity,
                wind: self.sim_wind(frame),
                wind_turbulence: frame.wind_turbulence,
                params: &self.params,
            };
            eval(program, &compiled, &inputs, &mut self.regs);
            let col = |pin: SinkPin| program.output(pin).map(|r| r.0 as usize);
            let (lifetime, position, velocity, size, rotation, angular, color) = (
                col(SinkPin::Lifetime),
                col(SinkPin::Position),
                col(SinkPin::Velocity),
                col(SinkPin::Size),
                col(SinkPin::Rotation),
                col(SinkPin::AngularVelocity),
                col(SinkPin::Color),
            );
            let read = |c: Option<usize>, k: usize, regs: &Registers| -> Vec4 {
                c.map(|c| regs.get(crate::ir::Reg(c as u32))[k]).unwrap_or(Vec4::ZERO)
            };
            for (k, &id) in ids.iter().enumerate() {
                let mut pos = as_vec3(read(position, k, &self.regs));
                let mut vel = as_vec3(read(velocity, k, &self.regs));
                let mut col = read(color, k, &self.regs);
                match origin {
                    None => {
                        if self.space == SimulationSpace::World {
                            pos = frame.transform.transform_point3(pos);
                            vel = frame.transform.transform_vector3(vel);
                            // World space is world velocity, so the entity's
                            // adds straight on (0 for a Local emitter).
                            vel += frame.emitter_velocity * self.inherit_velocity;
                        }
                    }
                    Some(o) => {
                        // An offset from the parent, turned with the entity
                        // like an own spawn's position is, but not moved to
                        // it: the parent is already where it is.
                        if self.space == SimulationSpace::World {
                            pos = frame.transform.transform_vector3(pos);
                            vel = frame.transform.transform_vector3(vel);
                        }
                        pos += o.position;
                        vel += o.velocity;
                        if let Some(c) = o.color {
                            col *= c;
                        }
                    }
                }
                let size = as_vec2(read(size, k, &self.regs));
                self.particles.push(Birth {
                    id,
                    lifetime: sanitize(read(lifetime, k, &self.regs).x).max(0.0),
                    position: sanitize3(pos),
                    velocity: sanitize3(vel),
                    size,
                    rotation: sanitize(read(rotation, k, &self.regs).x),
                    angular_velocity: sanitize(read(angular, k, &self.regs).x),
                    color: col,
                    generation,
                });
                let i = self.particles.len() - 1;
                self.raise_birth(i);
            }
            self.next_id = self.next_id.wrapping_add(len as u32);
            done += len;
        }
    }

    /// Age, kill, then run the update context, integrate, cap the speed,
    /// collide, keep trails, and raise interval and death events.
    fn update(&mut self, timing: &StepTiming, frame: &FrameInput) {
        let dt = timing.dt;
        for age in &mut self.particles.age {
            *age += dt;
        }
        let mut i = 0;
        while i < self.particles.len() {
            if self.particles.age[i] > self.particles.lifetime[i] {
                self.raise_death(i);
                self.particles.swap_remove(i);
            } else {
                i += 1;
            }
        }
        if self.particles.is_empty() {
            return;
        }

        let compiled = self.compiled.clone();
        let program = &compiled.update;
        let col = |pin: SinkPin| program.output(pin).map(|r| r.0 as usize);
        let (accel, gravity, drag, sol, col_ol, max_speed, emission) = (
            col(SinkPin::Acceleration),
            col(SinkPin::Gravity),
            col(SinkPin::Drag),
            col(SinkPin::SizeOverLife),
            col(SinkPin::ColorOverLife),
            col(SinkPin::MaxSpeed),
            col(SinkPin::Emission),
        );
        // Collider geometry is in the entity's space, or the world's (see
        // `ColliderIr`). A particle is taken from its simulation space into
        // the collider's for the test and back out after, which is exact for
        // every shape however the entity is turned or scaled. `None` is the
        // identity; an entity that cannot be inverted collides with nothing
        // that needs the inverse.
        let colliders = &compiled.colliders;
        let inverse = if colliders.is_empty() {
            None
        } else {
            let inv = frame.transform.inverse();
            inv.is_finite().then_some(inv)
        };
        let into = |world: bool| -> Option<Option<(Affine3A, Affine3A)>> {
            match (world, self.space) {
                (false, SimulationSpace::Local) | (true, SimulationSpace::World) => Some(None),
                (false, SimulationSpace::World) => inverse.map(|inv| Some((inv, frame.transform))),
                (true, SimulationSpace::Local) => inverse.map(|inv| Some((frame.transform, inv))),
            }
        };
        let spaces: Vec<Option<Option<(Affine3A, Affine3A)>>> = colliders.iter().map(|c| into(c.world)).collect();
        // The scene is hit after the step, by the runtime (see `scene`); all
        // the step does is leave each particle's response settings behind.
        let scene_collider = scene::first_scene_collider(&compiled).filter(|_| self.particles.scene_tracking);
        let has_intervals = self.rules.iter().any(|r| matches!(r.trigger, SubEmitterTrigger::Interval(_)));
        self.killed.clear();
        let total = self.particles.len();
        let mut start = 0;
        while start < total {
            let end = (start + BATCH).min(total);
            let p = &self.particles;
            let inputs = EvalInputs {
                count: end - start,
                seed: self.seed,
                ids: &p.id[start..end],
                spawn_id: 0,
                age: &p.age[start..end],
                lifetime: &p.lifetime[start..end],
                position: &p.position[start..end],
                velocity: &p.velocity[start..end],
                emitter_time: timing.cycle_time(),
                delta_time: dt,
                emitter_velocity: frame.emitter_velocity,
                wind: self.sim_wind(frame),
                wind_turbulence: frame.wind_turbulence,
                params: &self.params,
            };
            eval(program, &compiled, &inputs, &mut self.regs);
            let regs = &self.regs;
            let read = |c: Option<usize>, k: usize, fallback: Vec4| -> Vec4 {
                c.map(|c| regs.get(crate::ir::Reg(c as u32))[k]).unwrap_or(fallback)
            };
            let p = &mut self.particles;
            for k in 0..(end - start) {
                let i = start + k;
                let a = sanitize3(as_vec3(read(accel, k, Vec4::ZERO)) + as_vec3(read(gravity, k, Vec4::ZERO)));
                let d = read(drag, k, Vec4::ZERO).x.max(0.0);
                let mut v = p.velocity[i] + a * dt;
                // Implicit drag: unconditionally stable, so a large drag stops
                // a particle rather than flinging it backwards.
                v /= 1.0 + sanitize(d) * dt;
                // A hard cap, after every force, so nothing this step can
                // carry a particle past it.
                let cap = sanitize(read(max_speed, k, Vec4::ZERO).x);
                if cap > 0.0 && v.length_squared() > cap * cap {
                    v = v.normalize_or_zero() * cap;
                }
                let p0 = p.position[i];
                let mut p1 = p0 + v * dt;
                if !colliders.is_empty() {
                    let mut dead = false;
                    for (c, space) in colliders.iter().zip(&spaces) {
                        let Some(space) = space else { continue };
                        let (mut q0, mut q1, mut w, mut adt) = (p0, p1, v, a * dt);
                        if let Some((there, _)) = space {
                            q0 = there.transform_point3(p0);
                            q1 = there.transform_point3(p1);
                            w = there.transform_vector3(v);
                            adt = there.transform_vector3(adt);
                        }
                        let input = collide::ColliderInput {
                            shape: c.shape,
                            contain: c.contain,
                            kill: c.kill,
                            a: read(Some(c.a.0 as usize), k, Vec4::ZERO),
                            b: read(Some(c.b.0 as usize), k, Vec4::ZERO),
                            bounce: read(Some(c.bounce.0 as usize), k, Vec4::ZERO).x,
                            friction: read(Some(c.friction.0 as usize), k, Vec4::ZERO).x,
                        };
                        let hit = collide::collide(&input, q0, &mut q1, &mut w, adt);
                        if hit == collide::Hit::None {
                            continue;
                        }
                        if let Some((_, back)) = space {
                            q1 = back.transform_point3(q1);
                            w = back.transform_vector3(w);
                        }
                        p1 = sanitize3(q1);
                        v = sanitize3(w);
                        if hit == collide::Hit::Killed {
                            dead = true;
                            break;
                        }
                    }
                    if dead {
                        self.killed.push(i);
                    }
                }
                if let Some(c) = scene_collider {
                    if let Some(s) = p.scene.get_mut(i) {
                        s.accel_dt = a * dt;
                        s.bounce = read(Some(c.bounce.0 as usize), k, Vec4::ZERO).x;
                        s.friction = read(Some(c.friction.0 as usize), k, Vec4::ZERO).x;
                        s.radius = read(Some(c.b.0 as usize), k, Vec4::ZERO).x;
                    }
                }
                p.velocity[i] = v;
                p.position[i] = p1;
                p.rotation[i] += p.angular_velocity[i] * dt;
                p.size[i] = p.base_size[i] * as_vec2(read(sol, k, Vec4::ONE));
                // Emission scales the light, not the coverage: alpha keeps
                // what the colour over life says.
                let glow = sanitize(read(emission, k, Vec4::ONE).x);
                let c = p.base_color[i] * read(col_ol, k, Vec4::ONE);
                p.color[i] = (c.truncate() * glow).extend(c.w);
                if p.trail_stride > 0 {
                    p.record_trail(i, p1, self.trail_min_distance);
                }
            }
            start = end;
        }
        if has_intervals {
            for i in 0..self.particles.len() {
                let before = self.particles.age[i] - dt;
                self.raise_intervals(i, before);
            }
        }
        // Collision deaths, highest index first: a swap-remove only ever moves
        // a particle from past the one removed, which is then one already
        // dealt with, so every index in the list stays valid.
        if !self.killed.is_empty() {
            let mut killed = std::mem::take(&mut self.killed);
            killed.sort_unstable_by(|a, b| b.cmp(a));
            killed.dedup();
            for &i in &killed {
                self.raise_death(i);
                self.particles.swap_remove(i);
            }
            killed.clear();
            self.killed = killed;
        }
    }

    /// Spawn `rule.count` children for one delivered event.
    fn spawn_children(&mut self, count: u32, origin: &sub::Origin, generation: u8, frame: &FrameInput) {
        self.spawn(count, frame, Some(origin), generation);
    }
}

/// Bursts at `k * interval` (`k = 0, 1, ...`, fewer than `cycles` unless that
/// is 0, which means unlimited) that fall in `[a, b)`. A non-positive interval
/// means a single burst at 0.
fn bursts_in(a: f32, b: f32, interval: f32, cycles: u64) -> u64 {
    if !interval.is_finite() || interval <= 0.0 {
        return u64::from(a <= 0.0 && 0.0 < b && cycles != 0);
    }
    let first = (a / interval).ceil().max(0.0) as u64;
    let mut n = 0;
    let mut k = first;
    while (k as f32) * interval < b {
        if cycles != 0 && k >= cycles {
            break;
        }
        n += 1;
        k += 1;
        if n > 100_000 {
            break;
        }
    }
    n
}

#[inline]
fn sanitize(v: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

#[inline]
fn sanitize3(v: Vec3) -> Vec3 {
    if v.is_finite() {
        v
    } else {
        Vec3::ZERO
    }
}

/// A whole effect: its emitters and the clock they share.
pub struct EffectSim {
    pub emitters: Vec<EmitterSim>,
    /// Seconds since the effect started, not wrapped.
    pub time: f32,
    pub duration: f32,
    pub looping: bool,
    segments: Vec<(f32, f32)>,
    /// Sub-emitter rules that were refused (a loop, a missing target), one
    /// sentence each, for the runtime to report. The effect plays without
    /// them.
    pub sub_emitter_issues: Vec<String>,
    /// Scratch for delivering events.
    events: Vec<(usize, sub::Event)>,
}

/// An emitter that failed to compile, and why. The effect still runs without
/// it.
pub struct EmitterErrors {
    pub emitter_index: usize,
    pub name: String,
    pub errors: Vec<CompileError>,
}

impl EffectSim {
    /// Compile every enabled emitter of `asset`, all on the CPU.
    pub fn new(asset: &VfxAsset) -> (Self, Vec<EmitterErrors>) {
        Self::with_backends(asset, &crate::gpu::select::GpuCaps::none())
    }

    /// Compile every enabled emitter of `asset` and give each the backend its
    /// [`crate::asset::Backend`] setting, its features and `caps` allow
    /// ([`crate::gpu::select::choose`]).
    ///
    /// Emitters joined by a sub-emitter rule share their events on one
    /// backend: a GPU parent's events live in GPU memory, which a CPU child
    /// cannot read without a readback a frame late, and a CPU parent's events
    /// would have to be uploaded the other way. So once each emitter has its
    /// own answer, a CPU one pulls every emitter it shares rules with onto the
    /// CPU too, transitively, and each says why ([`crate::gpu::select::link_backends`]).
    pub fn with_backends(asset: &VfxAsset, caps: &crate::gpu::select::GpuCaps) -> (Self, Vec<EmitterErrors>) {
        let (mut sim, failures) = Self::build(asset);
        let two_d = asset.dimension == crate::asset::Dimension::TwoD;
        use crate::gpu::select::{self as select, ActiveBackend, BackendChoice};
        let mut choices: Vec<BackendChoice> = Vec::with_capacity(sim.emitters.len());
        let mut promotable: Vec<bool> = Vec::with_capacity(sim.emitters.len());
        for e in &sim.emitters {
            let emitter = &asset.emitters[e.emitter_index];
            let mut choice = select::choose_in(emitter, &e.compiled, e.driven, two_d, caps);
            let generation = select::generation_blocker(e.generations, !e.rules.is_empty());
            if let (ActiveBackend::Gpu, Some(why)) = (choice.active, generation.clone()) {
                choice = BackendChoice::cpu_because(emitter, why);
            }
            promotable.push(
                emitter.backend == crate::asset::Backend::Auto
                    && choice.active == ActiveBackend::Cpu
                    && generation.is_none()
                    && select::gpu_blockers(emitter, &e.compiled, e.driven, two_d, caps).is_empty(),
            );
            choices.push(choice);
        }
        let edges: Vec<(usize, usize)> =
            sim.emitters.iter().enumerate().flat_map(|(from, e)| e.rules.iter().map(move |r| (from, r.target))).collect();
        let names: Vec<&str> = sim.emitters.iter().map(|e| asset.emitters[e.emitter_index].name.as_str()).collect();
        let explicit: Vec<bool> =
            sim.emitters.iter().map(|e| asset.emitters[e.emitter_index].backend == crate::asset::Backend::Gpu).collect();
        select::link_backends(&mut choices, &edges, &names, &explicit, &promotable);
        for (e, choice) in sim.emitters.iter_mut().zip(choices) {
            e.set_gpu(choice.active == crate::gpu::select::ActiveBackend::Gpu);
            e.backend = choice;
        }
        (sim, failures)
    }

    fn build(asset: &VfxAsset) -> (Self, Vec<EmitterErrors>) {
        let mut emitters = Vec::new();
        let mut failures = Vec::new();
        for (index, emitter) in asset.emitters.iter().enumerate() {
            // A switched-off emitter is skipped, unless an `Enabled` track
            // switches it on somewhere: then the track decides, every step.
            if !emitter.enabled && !params::has_enabled_track(asset, index) {
                continue;
            }
            let keyed = params::keyed_pins(asset, index);
            match compile_graph_with_params(&emitter.graph, &keyed) {
                Ok(compiled) => {
                    let mut sim = EmitterSim::new(index, Arc::new(compiled), emitter.seed, emitter.capacity, emitter.space);
                    sim.tracks = EmitterTracks::from_asset(asset, index);
                    sim.inherit_velocity = emitter.inherited_velocity();
                    let r = &emitter.render;
                    if r.facing == Facing::Ribbon && r.ribbon.kind == RibbonKind::Trail {
                        sim.set_trail(r.ribbon.trail_points(), r.ribbon.min_distance);
                    }
                    emitters.push(sim);
                }
                Err(errors) => failures.push(EmitterErrors {
                    emitter_index: index,
                    name: emitter.name.clone(),
                    errors,
                }),
            }
        }

        // Sub-emitter rules, resolved from asset indices to the slots of the
        // emitters that are actually running. A rule whose source or target
        // was skipped (disabled, or failed to compile) has nothing to do.
        let (accepted, sub_emitter_issues) = sub::accepted_rules(asset);
        let slot_of = |asset_index: usize| emitters.iter().position(|e: &EmitterSim| e.emitter_index == asset_index);
        let mut resolved: Vec<(usize, sub::Rule)> = Vec::new();
        for (source, i) in accepted {
            let rule = &asset.emitters[source].sub_emitters[i];
            if let (Some(from), Some(to)) = (slot_of(source), slot_of(rule.target)) {
                resolved.push((
                    from,
                    sub::Rule {
                        trigger: rule.trigger,
                        target: to,
                        count: rule.count,
                        inherit_velocity: if rule.inherit_velocity.is_finite() { rule.inherit_velocity } else { 0.0 },
                        inherit_color: rule.inherit_color,
                    },
                ));
            }
        }
        let edges: Vec<(usize, usize)> = resolved.iter().map(|(from, rule)| (*from, rule.target)).collect();
        for (from, rule) in resolved {
            emitters[rule.target].driven = true;
            emitters[from].rules.push(rule);
        }
        let spans = sub::generation_spans(emitters.len(), &edges);
        for (e, span) in emitters.iter_mut().zip(spans) {
            e.generations = span;
        }

        (
            Self {
                emitters,
                time: 0.0,
                duration: if asset.duration.is_finite() { asset.duration.max(0.0) } else { 0.0 },
                looping: asset.looping,
                segments: Vec::new(),
                sub_emitter_issues,
                events: Vec::new(),
            },
            failures,
        )
    }

    /// Deliver every sub-emitter event the emitters raised, and the events
    /// their children raise in turn, in the fixed order the [`sub`] docs
    /// describe. Each round is one generation, so the rounds end by
    /// [`sub::MAX_SUB_DEPTH`] whatever the rules say.
    fn deliver(&mut self, frame: &FrameInput) {
        let inverse = frame.transform.inverse();
        let inverse = inverse.is_finite().then_some(inverse);
        for _ in 0..=sub::MAX_SUB_DEPTH {
            let mut events = std::mem::take(&mut self.events);
            events.clear();
            for (slot, e) in self.emitters.iter_mut().enumerate() {
                events.extend(e.pending.drain(..).map(|ev| (slot, ev)));
            }
            if events.is_empty() {
                self.events = events;
                return;
            }
            events.sort_by(|(sa, a), (sb, b)| (sa, a.rule, a.parent, a.index).cmp(&(sb, b.rule, b.parent, b.index)));
            for &(slot, ev) in &events {
                let source = &self.emitters[slot];
                let rule = source.rules[ev.rule];
                let target_space = self.emitters[rule.target].space;
                // Source simulation space to world, then world to target.
                let (mut position, mut velocity) = (ev.position, ev.velocity * rule.inherit_velocity);
                if source.space == SimulationSpace::Local {
                    position = frame.transform.transform_point3(position);
                    velocity = frame.transform.transform_vector3(velocity);
                }
                if target_space == SimulationSpace::Local {
                    let Some(inv) = inverse else { continue };
                    position = inv.transform_point3(position);
                    velocity = inv.transform_vector3(velocity);
                }
                let origin = sub::Origin {
                    position: sanitize3(position),
                    velocity: sanitize3(velocity),
                    color: rule.inherit_color.then_some(ev.color),
                };
                self.emitters[rule.target].spawn_children(rule.count, &origin, ev.generation, frame);
            }
            self.events = events;
        }
        // Whatever is still pending is past the depth cap.
        for e in &mut self.emitters {
            e.pending.clear();
        }
    }

    pub fn reset(&mut self) {
        self.time = 0.0;
        for e in &mut self.emitters {
            e.reset();
        }
    }

    /// Advance by exactly `dt`.
    pub fn step(&mut self, dt: f32, frame: &FrameInput) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let timing = StepTiming {
            t0: self.time,
            dt,
            duration: self.duration,
            looping: self.looping,
        };
        for e in &mut self.emitters {
            e.step(&timing, frame, &mut self.segments);
        }
        self.deliver(frame);
        self.time += dt;
    }

    /// Advance by a frame's delta for normal play: clamped to
    /// [`MAX_FRAME_DELTA`] and split into steps of at most [`MAX_PLAY_STEP`].
    pub fn advance(&mut self, dt: f32, frame: &FrameInput) {
        let dt = if dt.is_finite() { dt.clamp(0.0, MAX_FRAME_DELTA) } else { 0.0 };
        if dt <= 0.0 {
            return;
        }
        let steps = (dt / MAX_PLAY_STEP).ceil().max(1.0);
        let step = dt / steps;
        for _ in 0..steps as u32 {
            self.step(step, frame);
        }
    }

    /// Replay from zero to `t` in fixed steps, so the result depends only on
    /// `t` and the asset. Exactly equal to calling [`EffectSim::step`] with
    /// [`SEEK_STEP`] the same number of times when `t` is a whole number of
    /// steps; otherwise the last step is the remainder. Every replayed step
    /// uses `frame`'s wind: past wind is not recorded (see the module docs).
    pub fn simulate_to(&mut self, t: f32, frame: &FrameInput) {
        self.reset();
        if !t.is_finite() || t <= 0.0 {
            return;
        }
        let exact = t / SEEK_STEP;
        let whole = exact.round();
        let (step, steps, remainder) = if (exact - whole).abs() < 1e-3 {
            (SEEK_STEP, whole as u32, 0.0)
        } else {
            let whole = exact.floor();
            (SEEK_STEP, whole as u32, t - whole * SEEK_STEP)
        };
        let cap = if self.emitters.iter().any(EmitterSim::is_gpu) { GPU_MAX_SEEK_STEPS } else { MAX_SEEK_STEPS };
        let (step, steps, remainder) = if steps > cap {
            (t / cap as f32, cap, 0.0)
        } else {
            (step, steps, remainder)
        };
        for _ in 0..steps {
            self.step(step, frame);
        }
        if remainder > 1e-6 {
            self.step(remainder, frame);
        }
        self.scene_seen_all();
    }

    /// Spawn `count` particles from every emitter right now, outside the
    /// schedule the graph sets. The script `burst` call. A sub-emitter's
    /// target is skipped: it emits only what its parents ask for.
    pub fn burst(&mut self, count: u32, frame: &FrameInput) {
        if count == 0 {
            return;
        }
        // Keyed init inputs read their value now, as a step would: a burst
        // before the first step would otherwise find its slots empty.
        let t = if self.looping && self.duration > 0.0 { self.time.rem_euclid(self.duration) } else { self.time };
        for e in self.emitters.iter_mut() {
            if e.driven {
                // A GPU target still gets the burst's step, with nothing to
                // spawn of its own: its kernels run in lockstep with its
                // parents' (`gpu::render`), and the burst's children reach it
                // through its inbox in that step.
                if e.is_gpu() {
                    e.plan_gpu(0.0, 0.0, 0, frame, true);
                }
                continue;
            }
            e.tracks.fill(t, &mut e.params);
            e.spawn(count, frame, None, 0);
        }
        self.deliver(frame);
    }

    /// Set (or with `None` clear) every Parameter node named `name`, in every
    /// emitter, from the next step on. Whether any emitter has one.
    pub fn set_parameter(&mut self, name: &str, value: Option<Vec4>) -> bool {
        let mut found = false;
        for e in &mut self.emitters {
            found |= e.tracks.set_parameter(name, value);
        }
        found
    }

    /// Make the script parameter values exactly `values` (see
    /// [`crate::params::EmitterTracks::apply_parameters`]).
    pub fn apply_parameters(&mut self, values: &std::collections::BTreeMap<String, Vec4>) {
        for e in &mut self.emitters {
            e.tracks.apply_parameters(values);
        }
    }

    /// Particles alive across every emitter; a GPU emitter's count is a
    /// frame old.
    pub fn alive(&self) -> usize {
        self.emitters.iter().map(EmitterSim::alive).sum()
    }

    pub fn dropped(&self) -> u64 {
        self.emitters.iter().map(EmitterSim::dropped_total).sum()
    }

    /// A non-looping effect with a duration is finished once that duration has
    /// passed and its last particle has died. A looping one, or one with no
    /// duration, never finishes.
    pub fn is_finished(&self) -> bool {
        !self.looping
            && self.duration > 0.0
            && self.time >= self.duration
            && self.emitters.iter().all(EmitterSim::settled)
            && self.alive() == 0
    }
}
