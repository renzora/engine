#![no_std]
//! Film Grain post-process effect.
//!
//! Converted from the Bevy-linking `crates/renzora_film_grain`. Links no Bevy, so it
//! rebuilds in about a second and hot-reloads, shader included. See `plugins/crt`
//! for the conversion notes.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;

const WGSL: &str = include_str!("film_grain.wgsl");

#[derive(Component)]
#[component(name = "Film Grain")]
#[repr(C)]
pub struct FilmGrain {
    #[field(min = 0.0, max = 2.0, speed = 0.01)]
    pub intensity: f32,
    /// Width of one grain cell, in **pixels**. Below 1.0 there is nothing left to
    /// resolve — one cell per pixel is as fine as the grain gets — so the range
    /// starts there rather than at the 0.1 it used to allow.
    #[field(min = 1.0, max = 10.0, speed = 0.1)]
    pub grain_size: f32,
    /// Seconds, advanced by [`sync_time`]. Skipped in the inspector but *not*
    /// engine-driven: nothing in the host writes this, and leaving it at its
    /// default is what froze the grain into a fixed dirt layer.
    #[field(skip)]
    pub time: f32,
}

impl Default for FilmGrain {
    fn default() -> Self {
        Self {
            intensity: 0.3,
            grain_size: 1.5,
            time: 0.0,
        }
    }
}

/// Drives the shader's clock.
///
/// A post-process plugin gets no time for free — `add_post_process` registers a
/// pass and a uniform, and the bridge uploads the component's bytes verbatim
/// without interpreting a single field. An animated effect has to tick its own
/// value, the way `plugins/pulse` and `plugins/ripple` do.
fn sync_time(mut q: Query<&mut FilmGrain>, time: Res<Time>) {
    for g in &mut q {
        // Accumulated and wrapped rather than assigned from `elapsed_secs`. The
        // shader quantises this to a 24 Hz frame index, and an f32 second count
        // stops resolving those steps after a few hours of uptime; wrapping keeps
        // the seed small. A grain pattern that repeats every ~17 minutes is not
        // something anyone can see.
        g.time += time.delta_secs();
        if g.time > 1024.0 {
            g.time -= 1024.0;
        }
    }
}

pub struct FilmGrainPlugin;

impl Plugin for FilmGrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_post_process::<FilmGrain>("film_grain", WGSL, RenderPhase::LdrPost, 0.0)
            .add_systems(Update, sync_time);
    }
}

renzora_plugin::add!(FilmGrainPlugin);

#[cfg(test)]
mod tests {
    use super::*;

    /// The Rust struct and the shader must agree byte for byte. Nothing enforces
    /// it at run time — the host copies these bytes straight into the uniform
    /// buffer and the shader reads them back by offset — so a mismatch is not an
    /// error, it is a wrong picture: every field from the mismatch onward reads
    /// its neighbour's value.
    #[test]
    fn the_uniform_matches_the_shader() {
        renzora_plugin::uniform_check::assert_uniform_matches::<FilmGrain>(WGSL, "FilmGrainSettings");
    }
}
