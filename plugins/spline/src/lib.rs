//! Splines — control-point paths with Catmull-Rom evaluation.
//!
//! The interesting thing about this plugin is how little is in it. `SplinePath`
//! and its curve maths live in the **contract crate** (`renzora::spline`), and
//! all that remains here is registering the type for reflection.
//!
//! That split is deliberate and worth more than it looks. A spline is not a
//! feature so much as a *shape other things read*: a road builder, a camera
//! rail, a fence generator, a patrol path, a particle emitter track. Every one
//! of those is a plausible separate plugin, and they can only cooperate if they
//! agree on one `SplinePath` — one `TypeId`, one scene representation, one
//! definition of where `t = 1.7` falls on the curve.
//!
//! Had the type stayed here, each of those plugins would have had to define its
//! own, and none could read another's: a path authored by one would be invisible
//! to the next, and the editor's gizmo overlay — which draws control points and
//! the smooth curve for anything carrying a `SplinePath` — would only work for
//! whichever one happened to be linked. In the contract crate it is an
//! interchange format instead, and a plugin that spawns one gets the editing UI
//! for free.

use bevy::prelude::*;

/// Re-exported so this plugin reads as owning the concept even though the type
/// itself is shared, and so a dependent can name it without also naming the
/// contract crate.
pub use renzora::SplinePath;

#[derive(Default)]
pub struct SplinePlugin;

impl Plugin for SplinePlugin {
    fn build(&self, app: &mut App) {
        info!("[spline] native plugin");
        // The registration is the whole job: it is what lets a `SplinePath`
        // round-trip through a scene file and show up in the inspector.
        app.register_type::<SplinePath>();
    }
}

// `Runtime`, explicitly. A spline is scene content a game evaluates, and
// `plugin!` defaults to `Editor` where `add!` defaulted to `Runtime` — omitting
// this would stop shipping splines to games with nothing to show for it.
renzora::plugin!(SplinePlugin, Runtime);
