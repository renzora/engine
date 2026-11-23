//! Multi-image sprites — the switchable-sheet half of 2D animation.
//!
//! A single [`SpriteImagePath`](super::SpriteImagePath) shows one image; a
//! [`SpriteSheet`](super::SpriteSheet) slices it into cells and picks one via
//! `frame`. Animating `SpriteSheet.frame` on the property timeline already gives
//! a flipbook — but a real character's actions often live in **separate** sheets
//! (`Idle.png`, `Run.png`, `Jump.png`). Switching between them can't be a
//! keyframed `SpriteImagePath` because that's a *string*, and the property
//! timeline only keyframes numbers.
//!
//! [`SpriteImages`] solves that: it holds a *list* of image paths plus an
//! `active` **index**. A render system binds `Sprite.image` to `images[active]`,
//! and because `active` is a plain reflected `u32`, the property timeline
//! keyframes it exactly like `SpriteSheet.frame` — so an animation clip is just
//! a `frame` track (which cell) plus, when a clip lives in a different sheet, an
//! `active` track (which image). The whole thing rides the engine's existing
//! animation system; there is no bespoke sprite-clip runtime.
//!
//! The Sprite Animation editor panel is a *bridge* over this: dropping a sheet
//! appends to `images`, a dropdown sets `active`, and picking cells writes those
//! two tracks into a `.anim` clip on the entity's `AnimatorComponent`.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// A set of sprite-sheet images for one entity, with a switchable/keyframeable
/// active index.
///
/// When present it *owns* the entity's `Sprite.image` (an engine system sets it
/// from `images[active]`), so an animated character uses this instead of the
/// single [`SpriteImagePath`](super::SpriteImagePath). `active` is a reflected
/// `u32` so the property timeline can keyframe "which sheet" — a clip that plays
/// out of `Run.png` just pins `active` to that image's index for its duration.
#[derive(Component, Reflect, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SpriteImages {
    /// Asset-relative paths of the sheets, in a stable order — the index space
    /// `index` and any animation `index` track address.
    pub images: Vec<String>,
    /// Which entry of `images` is currently shown. Wraps modulo the list length
    /// (so a keyframe past the end is clamped rather than blanking the sprite).
    /// Kept a plain `u32` so it round-trips the reflection scene serializer and
    /// is keyframeable on the property timeline — a clip's "which sheet" track.
    pub index: u32,
}

impl SpriteImages {
    /// The currently-indexed image path (clamped), or `None` when empty.
    pub fn active_path(&self) -> Option<&str> {
        if self.images.is_empty() {
            return None;
        }
        let idx = (self.index as usize) % self.images.len();
        Some(self.images[idx].as_str())
    }
}
