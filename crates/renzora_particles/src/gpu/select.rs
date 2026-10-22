//! Which backend runs an emitter, and why.
//!
//! Pure: what the device can do arrives as [`GpuCaps`], and the answer is a
//! [`BackendChoice`] that carries its reason in words, because the editor
//! shows it ("CPU: its Brightest N light follows single particles") and an
//! explicit `Gpu` that cannot run says the same thing in the Console.
//!
//! ## The rules
//!
//! An emitter can run on the GPU when the device has the compute support the
//! backend needs, the build has a renderer for the effect's dimension, and the
//! emitter uses none of what the backend leaves on the CPU:
//!
//! - a `BrightestN` light (it follows single particles, which would mean
//!   reading positions back),
//! - a Collide Scene node (the scene's colliders are asked through
//!   `renzora::scene_query` with the world in hand, which a kernel has not),
//! - more keyed pins than a step record carries
//!   ([`super::layout::MAX_GPU_PARAMS`]),
//! - a pool, with its trail history, larger than the device's storage
//!   binding allows, or than [`super::layout::MAX_GPU_CAPACITY`],
//! - a `Connect` ribbon in more strips than the state header counts
//!   ([`super::layout::MAX_GPU_RIBBON_GROUPS`]), or on a sub-emitter's target
//!   (its children's spawn indices are hashed from their parent's on the GPU,
//!   so spawn order, which the strip runs in, means nothing there),
//! - a sub-emitter chain deep enough that one emitter's particles straddle
//!   the generation cap ([`generation_blocker`]).
//!
//! Ribbons of both kinds, tubes, and sub-emitters on either side of a rule
//! run on the GPU. A rule joins its two emitters, though: their events stay
//! in one place, so [`link_backends`] moves every emitter a CPU one shares
//! rules with onto the CPU too, and says so.
//!
//! A `PerEmitter` light is supported: it needs only a sum over the particles,
//! which the GPU reduces and reads back a frame late.
//!
//! `Auto` then takes the GPU only from [`AUTO_GPU_MIN_CAPACITY`] particles up,
//! where it pays for its dispatches; `Gpu` takes it at any size; `Cpu` never.

use crate::asset::{Backend, Dimension, Emitter, LightMode, RibbonKind};
use crate::ir::{ColliderShape, CompiledEmitter};

use super::layout::{state_bytes_with_trail, MAX_GPU_CAPACITY, MAX_GPU_PARAMS, MAX_GPU_RIBBON_GROUPS, PARTICLE_STRIDE};

/// The capacity from which `Auto` runs an emitter on the GPU. Below it the
/// CPU interpreter is fast enough, runs on every platform, and keeps its
/// particles where the CPU-side features can read them.
pub const AUTO_GPU_MIN_CAPACITY: u32 = 2048;

/// What the device and the build offer the GPU backend.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuCaps {
    /// Compute shaders, storage buffers readable from a vertex shader, and
    /// the workgroup limits the kernels use. False on WebGL2, on wasm, and
    /// before the render device has been checked.
    pub compute: bool,
    /// The largest pool one storage binding can hold.
    pub max_capacity: u32,
    /// The build draws 3D effects.
    pub renderer_3d: bool,
    /// The build draws 2D effects.
    pub renderer_2d: bool,
    /// Bytes one storage binding may hold. The state buffer grows by a trail
    /// ribbon's history, so a long trail on a big pool can pass it while the
    /// particles alone would not. 0 means "as much as `max_capacity` implies".
    pub max_binding: u64,
}

impl GpuCaps {
    /// No GPU backend: every emitter on the CPU.
    pub fn none() -> Self {
        Self::default()
    }

    /// Everything, for tests that need the GPU path chosen.
    pub fn all() -> Self {
        Self { compute: true, max_capacity: MAX_GPU_CAPACITY, renderer_3d: true, renderer_2d: true, max_binding: u64::MAX }
    }

    /// The pool a device with `max_binding` bytes per storage binding can hold.
    pub fn capacity_for_binding(max_binding: u64) -> u32 {
        (max_binding / PARTICLE_STRIDE).min(u64::from(MAX_GPU_CAPACITY)) as u32
    }

    /// The binding limit to hold a buffer to.
    fn binding_limit(&self) -> u64 {
        if self.max_binding > 0 {
            self.max_binding
        } else {
            u64::from(self.max_capacity) * PARTICLE_STRIDE
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ActiveBackend {
    #[default]
    Cpu,
    Gpu,
}

impl ActiveBackend {
    pub fn label(self) -> &'static str {
        match self {
            ActiveBackend::Cpu => "CPU",
            ActiveBackend::Gpu => "GPU",
        }
    }
}

/// The backend an emitter runs on, and, when that is the CPU although the GPU
/// was allowed, why.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BackendChoice {
    pub active: ActiveBackend,
    /// Why the CPU: `None` when the setting asked for it, or for the GPU.
    pub reason: Option<String>,
    /// The setting said `Gpu` and got the CPU: worth a Console warning.
    pub fell_back: bool,
}

impl BackendChoice {
    /// The CPU for `emitter`, because of `why` (a clause that reads after
    /// "runs on the CPU because"). An emitter set to the CPU keeps the plain
    /// choice, which carries no reason.
    pub fn cpu_because(emitter: &Emitter, why: String) -> Self {
        if emitter.backend == Backend::Cpu {
            return Self::default();
        }
        Self { active: ActiveBackend::Cpu, reason: Some(why), fell_back: emitter.backend == Backend::Gpu }
    }
}

/// Everything that keeps `emitter` off the GPU, in a fixed order, each a
/// clause that reads after "runs on the CPU because". `driven` says another
/// emitter's sub-emitter rules spawn it. `two_d` is the effect's dimension.
pub fn gpu_blockers(emitter: &Emitter, compiled: &CompiledEmitter, driven: bool, two_d: bool, caps: &GpuCaps) -> Vec<String> {
    let mut out = Vec::new();
    if !caps.compute {
        out.push("this device or platform has no GPU particle support".to_string());
    }
    let has_renderer = if two_d { caps.renderer_2d } else { caps.renderer_3d };
    if caps.compute && !has_renderer {
        out.push(format!("this build has no {} renderer for it", if two_d { "2D" } else { "3D" }));
    }
    let dimension = if two_d { Dimension::TwoD } else { Dimension::ThreeD };
    let ribbon = emitter.render.draws_ribbon(dimension).then_some(emitter.render.ribbon);
    if let Some(r) = ribbon.filter(|r| r.kind == RibbonKind::Connect) {
        if r.groups > MAX_GPU_RIBBON_GROUPS {
            out.push(format!(
                "its Connect ribbon has {} strips, more than the {MAX_GPU_RIBBON_GROUPS} the GPU draws",
                r.groups
            ));
        }
        if driven {
            out.push("its Connect ribbon runs in spawn order, which a GPU sub-emitter's children do not keep".to_string());
        }
    }
    // A project material is no blocker: `render::surface` draws it from the
    // pool with the particle vertex expansion, on quads, ribbons and meshes.
    if matches!(emitter.light.mode, LightMode::BrightestN(_)) {
        out.push("its Brightest N light follows single particles".to_string());
    }
    if compiled.colliders.iter().any(|c| c.shape == ColliderShape::Scene) {
        out.push("it collides with the scene, which only the CPU can query".to_string());
    }
    let params = super::codegen::param_count(compiled);
    if params > MAX_GPU_PARAMS {
        out.push(format!("it keys {params} pins, more than the {MAX_GPU_PARAMS} a GPU step carries"));
    }
    let limit = caps.max_capacity.min(MAX_GPU_CAPACITY);
    if caps.compute && emitter.capacity > limit {
        out.push(format!("its capacity of {} is past the GPU limit of {limit}", emitter.capacity));
    } else if caps.compute {
        let points = trail_points(emitter, dimension);
        let bytes = state_bytes_with_trail(emitter.capacity, points);
        if points > 0 && bytes > caps.binding_limit() {
            out.push(format!(
                "its trail history of {} MB is past what this device binds in one buffer",
                bytes.div_ceil(1 << 20)
            ));
        }
    }
    out
}

/// Positions the GPU keeps per slot for `emitter`'s trail: the ribbon's
/// points for a `Trail`, 0 for anything else.
pub fn trail_points(emitter: &Emitter, dimension: Dimension) -> u32 {
    let r = &emitter.render;
    if r.draws_ribbon(dimension) && r.ribbon.kind == RibbonKind::Trail {
        r.ribbon.trail_points() as u32
    } else {
        0
    }
}

/// Why an emitter with sub-emitter rules (`has_rules`) whose particles can be
/// `generations` (fewest, most) cannot raise its events on the GPU, if it
/// cannot. The GPU decides per emitter whether its particles are under the
/// depth cap (see `sim::sub::generation_spans`), which is exact unless one
/// emitter's particles fall on both sides of it.
pub fn generation_blocker(generations: (u8, u8), has_rules: bool) -> Option<String> {
    let cap = crate::sim::sub::MAX_SUB_DEPTH;
    (has_rules && generations.0 < cap && generations.1 >= cap).then(|| {
        format!("its sub-emitter chain reaches past {cap} generations on some paths and not others, which only the CPU counts per particle")
    })
}

/// Put every pair of emitters a sub-emitter rule joins on one backend.
/// `edges` are (source, target) slots, `names` each slot's emitter name,
/// `explicit` whether its setting was `Gpu` (so a move to the CPU is a
/// fallback worth a warning), and `promotable` whether it is an `Auto`
/// emitter on the CPU only for being small.
///
/// First a small `Auto` emitter joined to a GPU one moves up to the GPU: the
/// capacity rule is about what pays for its dispatches alone, and a target of
/// a few hundred embers spawned by a GPU pool of sparks is cheaper beside it
/// than dragging the pool down to the CPU. Then every GPU emitter still
/// joined to a CPU one moves down, until no rule joins the two backends.
pub fn link_backends(
    choices: &mut [BackendChoice],
    edges: &[(usize, usize)],
    names: &[&str],
    explicit: &[bool],
    promotable: &[bool],
) {
    loop {
        let mut moved = false;
        for &(a, b) in edges {
            let (Some(ca), Some(cb)) = (choices.get(a), choices.get(b)) else { continue };
            if ca.active == cb.active {
                continue;
            }
            let cpu = if ca.active == ActiveBackend::Gpu { b } else { a };
            if promotable.get(cpu).copied().unwrap_or(false) {
                choices[cpu] = BackendChoice { active: ActiveBackend::Gpu, reason: None, fell_back: false };
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    loop {
        let mut moved = false;
        for &(a, b) in edges {
            let (Some(ca), Some(cb)) = (choices.get(a), choices.get(b)) else { continue };
            if ca.active == cb.active {
                continue;
            }
            let (gpu, cpu) = if ca.active == ActiveBackend::Gpu { (a, b) } else { (b, a) };
            choices[gpu] = BackendChoice {
                active: ActiveBackend::Cpu,
                reason: Some(format!("it shares sub-emitter events with `{}`, which runs on the CPU", names[cpu])),
                fell_back: explicit.get(gpu).copied().unwrap_or(false),
            };
            moved = true;
        }
        if !moved {
            return;
        }
    }
}

/// The backend for `emitter`; see the module docs.
pub fn choose(emitter: &Emitter, compiled: &CompiledEmitter, driven: bool, caps: &GpuCaps) -> BackendChoice {
    choose_in(emitter, compiled, driven, false, caps)
}

/// [`choose`] for an effect of known dimension. This is the emitter on its
/// own; [`link_backends`] then joins it to the emitters it shares rules with.
pub fn choose_in(emitter: &Emitter, compiled: &CompiledEmitter, driven: bool, two_d: bool, caps: &GpuCaps) -> BackendChoice {
    if emitter.backend == Backend::Cpu {
        return BackendChoice::default();
    }
    let blockers = gpu_blockers(emitter, compiled, driven, two_d, caps);
    let explicit = emitter.backend == Backend::Gpu;
    if blockers.is_empty() && (explicit || emitter.capacity >= AUTO_GPU_MIN_CAPACITY) {
        return BackendChoice { active: ActiveBackend::Gpu, reason: None, fell_back: false };
    }
    let reason = if blockers.is_empty() {
        format!("its capacity of {} is below the {AUTO_GPU_MIN_CAPACITY} Auto moves to the GPU at", emitter.capacity)
    } else {
        blockers.join(", and ")
    };
    BackendChoice { active: ActiveBackend::Cpu, reason: Some(reason), fell_back: explicit }
}
