//! Turning the scene's environment off for the viewport shading modes that do
//! not use it.
//!
//! # Why this is a shared flag and not a local one
//!
//! The sky is produced by two crates that have nothing to do with each other —
//! `renzora_skybox` inserts a `Skybox` on every camera and takes over the clear
//! colour, `renzora_atmosphere` maintains Bevy's `Atmosphere` — and both
//! *reconcile* every frame from their own source of truth rather than reacting
//! to events. That is deliberate and correct on their side: it is what makes a
//! despawned source tear the sky down reliably.
//!
//! It also means nothing outside them can remove the sky by removing a
//! component: whatever you take off, the next frame puts back. The only way to
//! suppress it is to have both reconcilers agree to stand down, which is what
//! this is.
//!
//! # What it is for
//!
//! Wireframe shading draws topology and nothing else — no materials, no
//! lighting, and so no environment either. Solid shading is the matcap clay,
//! which ignores scene lighting by design, so a scene-lit sky behind it is the
//! same contradiction. Both set this; Material and Rendered clear it.
//!
//! Anything else that later wants a bare viewport gets it the same way, rather
//! than growing a second mechanism.

use bevy::prelude::*;

/// Set while the viewport is in a shading mode that shows no environment.
///
/// Default is "not suppressed", so an editor or game that never touches this
/// behaves exactly as it did before it existed.
/// Registered for reflection so the flag can be read from outside the process
/// while diagnosing "the sky is still there" — which is otherwise a guess
/// between "nothing set it" and "something ignored it".
#[derive(Resource, Reflect, Clone, Copy, Debug)]
#[reflect(Resource)]
pub struct EnvironmentSuppressed {
    /// Stand down: no skybox, no atmosphere.
    pub active: bool,
    /// What the camera clears to instead.
    ///
    /// Carried here rather than decided by the sky crates because the mode that
    /// suppressed the environment is the thing that knows what should be behind
    /// it, and it is the only party that can match the editor's theme. A sky
    /// crate picking its own colour would be guessing.
    pub clear: Color,
}

impl Default for EnvironmentSuppressed {
    fn default() -> Self {
        Self {
            active: false,
            // Never used while `active` is false; a mid grey rather than black
            // so a caller that forgets to set one still gets something wires
            // and gizmos read against.
            clear: Color::srgb(0.13, 0.13, 0.15),
        }
    }
}
