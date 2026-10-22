//! The GPU backend: the same graphs, simulated by compute shaders, for
//! emitters of a hundred thousand particles and more.
//!
//! The CPU interpreter ([`crate::sim`]) stays the reference and the fallback:
//! it runs everywhere (WebGL2 and wasm have no compute, and this module's
//! runtime half is not even compiled there), and it covers the features this
//! backend does not yet ([`select`]). An emitter moves here only when its
//! [`crate::asset::Backend`] setting and [`select::choose`] agree.
//!
//! ## What runs where
//!
//! - **CPU, per emitter, per step.** The clock, the keyed tracks and the
//!   spawn program, which decide how many particles a step spawns and which
//!   spawn indices they get ([`crate::sim::GpuStepPlan`]). Cheap, and it
//!   keeps bursts, keyed bursts and the spawn accumulator exactly the CPU's.
//! - **GPU, per particle.** The init program at birth (`spawn` kernel) and the
//!   update program every step (`update` kernel): ageing and death, forces,
//!   drag, the speed cap, integration, colliders, the over-life multipliers,
//!   a trail ribbon's history ring, and sub-emitter events, which a parent
//!   writes into its target's inbox and the target's `deliver` kernel spawns
//!   in the same step ([`plan`] lays an effect's emitters out for it).
//!   All are generated from the IR ([`codegen`]) against a shared library
//!   that transliterates the CPU's random hash, noise, curves and collisions.
//! - **GPU, per frame.** A reduction for the CPU to read back a frame late
//!   (live count, drops, a `PerEmitter` light's sums, the bounds the renderer
//!   culls by), and, when the emitter has a draw order or a `Connect`
//!   ribbon, a bitonic sort of the live particles by it.
//! - **Drawing.** Straight from the particle buffer: the emitter's render
//!   child is an ordinary mesh entity with one quad per pool slot, and the
//!   particle material's vertex shader, under `GPU_PARTICLES`, pulls each
//!   quad's particle out of the storage buffer instead of out of vertex
//!   attributes. Everything after the fetch (facing, flipbooks, procedural
//!   flame and smoke, soft particles, blending, tonemapping) is the very
//!   shader the CPU path draws with, so the two cannot drift.
//!
//! ## Buffers
//!
//! Per emitter: the particle pool (one packed struct per slot), the state
//! buffer (counters, free list, alive list, sort scratch), the data table
//! (constants, curve keys, gradient stops) and one shared dynamic uniform of
//! step records per frame. See [`layout`].
//!
//! ## Determinism
//!
//! Every random is the same stateless hash of (seed, spawn index, salt,
//! stream) as on the CPU, so a particle with a given spawn index is born
//! with the same values on both backends and in every replay. Where the
//! backends differ is listed in the docs' GPU section: float rounding in
//! transcendental functions, which slot a particle lands in, and, once the
//! pool is full, which spawn indices are dropped.

pub mod codegen;
pub mod layout;
pub mod plan;
pub mod select;

#[cfg(all(not(target_arch = "wasm32"), any(feature = "render_3d", feature = "render_2d")))]
pub mod material;
#[cfg(all(not(target_arch = "wasm32"), any(feature = "render_3d", feature = "render_2d")))]
pub mod render;
#[cfg(all(not(target_arch = "wasm32"), any(feature = "render_3d", feature = "render_2d")))]
pub mod runtime;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod trail_and_event_tests;

pub use select::{ActiveBackend, BackendChoice, GpuCaps, AUTO_GPU_MIN_CAPACITY};

use std::sync::{Arc, Mutex};

/// What the device offers the GPU backend, shared by the main and render
/// worlds. The render world fills it once it has a device (`RenderStartup`,
/// which may be frames after the app starts); until then, and for ever on a
/// build without the backend, it is `None` and every emitter runs on the
/// CPU. The runtime rebuilds effects when it first turns `Some`, so an
/// effect loaded before the device existed still moves to the GPU.
#[derive(bevy::prelude::Resource, Clone, Default)]
pub struct GpuSupport(pub Arc<Mutex<Option<GpuCaps>>>);

impl GpuSupport {
    pub fn caps(&self) -> Option<GpuCaps> {
        self.0.lock().ok().and_then(|c| *c)
    }

    pub fn set(&self, caps: GpuCaps) {
        if let Ok(mut c) = self.0.lock() {
            *c = Some(caps);
        }
    }
}
