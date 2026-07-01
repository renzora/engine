//! Procedural hair groom distribution plugin.
//!
//! Drop a [`Hair`] component on any entity that has a `Mesh3d` (a head, a scalp
//! cap, any surface) and the plugin **generates actual hair strands** over that
//! mesh: it area-samples root points across the surface triangles, grows a short
//! tapered strand from each along the surface normal (drooping toward gravity),
//! verlet-simulates the strands so they sway, and rebuilds a **camera-facing
//! ribbon mesh** every frame so the hair is visible from any angle and lit by
//! the normal PBR material.
//!
//! This is a self-contained groom: it needs no pre-rigged hair bones and no
//! external hair asset. The generated geometry lives on a hidden child render
//! entity (regenerated at runtime, never saved into the scene); only the `Hair`
//! component itself is serialized, so the groom rebuilds deterministically on
//! load.
//!
//! Generation is cheap and re-runs whenever a *shape* field changes (strand
//! count, length, segments, width, droop) — tuning the *look* (`color`) or the
//! *motion* (`stiffness`/`damping`/`gravity`) is live and does not rebuild the
//! strands, so it never resets the simulation.
//!
//! Like ragdoll, the sway only runs in **Play** or **Simulate** mode
//! (`PlayState::is_scripts_running`); while editing, the groom is held in its
//! grown rest shape and simply rides the model.

mod generate;
mod mesh;
mod script_extension;
mod simulate;
mod toggle;

pub use generate::HairGroomData;

use bevy::prelude::*;
#[cfg(feature = "editor")]
use renzora::AppEditorExt;

/// Procedural hair groom. Add it to an entity with a `Mesh3d`; the strands are
/// grown over that mesh's surface. The *shape* fields (strand count → `droop`)
/// rebuild the groom when changed; `color` and the *sim* fields (`stiffness` /
/// `damping` / `gravity`) are applied live without a rebuild.
//
// `renzora::Inspectable` is gated behind `renzora`'s `editor` feature, so the
// derive is applied via `cfg_attr` and routed through `renzora_macros` (always
// present) so the macro name stays resolvable in a lean build — same pattern as
// `Ragdoll`. Counts are `f32` because the Inspectable derive only renders
// float/bool/vec3/string/color editors (an integer field would be read-only).
#[derive(Component, Reflect, Clone)]
#[cfg_attr(feature = "editor", derive(renzora_macros::Inspectable))]
// `Default` in the reflect attribute registers `ReflectDefault` so scenes saved
// before a future field is added still load (missing field → default rather than
// a `from_reflect_with_fallback` panic) — the same gap that bit `Ragdoll`.
#[reflect(Component, Default)]
#[cfg_attr(
    feature = "editor",
    inspectable(name = "Hair", icon = "WIND", category = "physics")
)]
pub struct Hair {
    /// Master switch — generate and render the groom. Off hides the strands.
    pub enabled: bool,
    /// Physically simulate the strands (sway under gravity/motion) vs. hold the
    /// grown rest shape. Toggle live, or via `enable_hair()`/`disable_hair()`.
    pub simulate: bool,
    /// Target number of strands scattered over the mesh (area-weighted, so dense
    /// triangles get proportionally more). Capped at [`generate::MAX_STRANDS`].
    #[cfg_attr(
        feature = "editor",
        field(name = "Strands", speed = 25.0, min = 0.0, max = 50000.0)
    )]
    pub strands: f32,
    /// Strand length in world units (before per-strand jitter).
    #[cfg_attr(
        feature = "editor",
        field(name = "Length", speed = 0.005, min = 0.001, max = 5.0)
    )]
    pub length: f32,
    /// Random per-strand length variation, `0` (all equal) to `1` (down to half
    /// length). Breaks up the flat "helmet" silhouette.
    #[cfg_attr(
        feature = "editor",
        field(name = "Length Jitter", speed = 0.01, min = 0.0, max = 1.0)
    )]
    pub length_jitter: f32,
    /// Points along each strand (more = smoother curves, more geometry).
    #[cfg_attr(
        feature = "editor",
        field(name = "Segments", speed = 1.0, min = 1.0, max = 16.0)
    )]
    pub segments: f32,
    /// Half-width of a strand ribbon at the root, in world units; tapers to a
    /// point at the tip. ~`0.002`–`0.006` reads as fine hair.
    #[cfg_attr(
        feature = "editor",
        field(name = "Width", speed = 0.0005, min = 0.0001, max = 0.1)
    )]
    pub width: f32,
    /// How much a strand bends from the surface normal toward gravity as it
    /// grows, `0` (sticks straight out) to `1` (flops down hard). The rest-shape
    /// droop, distinct from the dynamic gravity below.
    #[cfg_attr(
        feature = "editor",
        field(name = "Droop", speed = 0.01, min = 0.0, max = 1.0)
    )]
    pub droop: f32,
    /// Base hair color as linear/sRGB RGB in `0..1` (multiplied by a small
    /// per-strand shade variation). A `Vec3` rather than `Color` because the
    /// Inspectable derive's colour path needs an `Into<[f32; 3]>` field.
    #[cfg_attr(feature = "editor", field(name = "Color (RGB)", speed = 0.005))]
    pub color: Vec3,
    /// Sim spring-back toward the rest shape, `0` (limp) to `1` (barely moves).
    #[cfg_attr(
        feature = "editor",
        field(name = "Stiffness", speed = 0.01, min = 0.0, max = 1.0)
    )]
    pub stiffness: f32,
    /// Sim velocity bleed-off, `0` (swings forever) to `1` (dead). Frame-rate
    /// normalised so the feel is stable across FPS.
    #[cfg_attr(
        feature = "editor",
        field(name = "Damping", speed = 0.01, min = 0.0, max = 1.0)
    )]
    pub damping: f32,
    /// Sim gravity multiplier. `0` floats; lower suits short, stiff hair.
    #[cfg_attr(
        feature = "editor",
        field(name = "Gravity", speed = 0.01, min = 0.0, max = 3.0)
    )]
    pub gravity: f32,
}

impl Default for Hair {
    fn default() -> Self {
        Self {
            enabled: true,
            simulate: true,
            strands: 2000.0,
            length: 0.12,
            length_jitter: 0.3,
            segments: 5.0,
            width: 0.0035,
            droop: 0.5,
            color: Vec3::new(0.16, 0.10, 0.06),
            stiffness: 0.12,
            damping: 0.7,
            gravity: 1.0,
        }
    }
}

impl Hair {
    /// Hash of the *shape* fields only. When it changes, [`generate`] rebuilds
    /// the strand set; `color`/sim edits leave it unchanged so they apply live.
    pub(crate) fn shape_signature(&self) -> u64 {
        let mut h = 0xcbf29ce484222325u64;
        for v in [
            self.strands,
            self.length,
            self.length_jitter,
            self.segments,
            self.width,
            self.droop,
        ] {
            h ^= v.to_bits() as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }
}

#[derive(Default)]
pub struct HairPlugin;

impl Plugin for HairPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] HairPlugin (procedural hair groom)");

        // `build_grooms` generates/regenerates strands and the render entity;
        // `simulate_grooms` advances the sim and rebuilds the ribbon mesh.
        // Chained so a freshly-built groom isn't simulated with un-applied
        // commands the same frame. `cleanup_grooms` tears down render entities
        // when a `Hair` component is removed.
        app.add_systems(
            Update,
            (
                generate::build_grooms,
                simulate::simulate_grooms,
                generate::cleanup_grooms,
            )
                .chain(),
        )
        .add_observer(toggle::handle_hair_script_actions);

        #[cfg(feature = "editor")]
        app.register_inspectable::<Hair>();
        #[cfg(not(feature = "editor"))]
        app.register_type::<Hair>();

        let mut extensions = app.world_mut().get_resource_or_insert_with(
            renzora_scripting::extension::ScriptExtensions::default,
        );
        extensions.register(script_extension::HairScriptExtension);
    }
}

renzora::add!(HairPlugin);
