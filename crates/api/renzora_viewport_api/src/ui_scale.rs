//! The player's game UI scale: a multiplier on top of the canvas fit.
//!
//! Game UI already scales itself: each frame ember's canvas scaler works out
//! how far the top screen canvas's reference size has to grow or shrink to fit
//! the window, and writes that into Bevy's `UiScale`. That fit is the author's
//! decision. What it left no room for is the *player's*: "UI size: 125%" in a
//! settings menu, for a large TV across the room or a small laptop screen,
//! because the scaler overwrote `UiScale` every frame and so any value a script
//! put there lasted one frame.
//!
//! [`GameUiScale`] is that room. The scaler multiplies its fit by
//! [`GameUiScale::factor`], so the canvas still fits and the player's choice
//! rides on top of it, applied live. It is a contract type so a Rust script
//! reaches it as `renzora::GameUiScale`, and Lua reaches it through
//! `set_setting("ui.scale", 1.25)`.

use bevy::prelude::*;

/// Multiplier on the game UI's fitted scale. `1.0` is the canvas exactly as
/// designed.
///
/// Read every frame by the canvas scaler, so changing it takes effect on the
/// next frame:
///
/// ```ignore
/// ctx.world().resource_mut::<renzora::GameUiScale>().set(1.25);
/// ```
///
/// Not persisted: a game that wants the player's choice to survive a restart
/// saves it itself and sets it again on load.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Reflect)]
#[reflect(Resource)]
pub struct GameUiScale {
    pub factor: f32,
}

impl GameUiScale {
    /// The smallest and largest factors [`Self::effective`] passes through.
    ///
    /// Past these the UI stops being usable rather than merely large or
    /// small: at a tenth every label is a smear, and at four times a 1080p
    /// HUD no longer fits on a 4K screen. Clamped where it is read, not where
    /// it is written, so a script that writes 0 (or NaN) gets a readable UI
    /// rather than an invisible one, and reads back what it wrote.
    pub const MIN: f32 = 0.25;
    pub const MAX: f32 = 4.0;

    /// Set the factor.
    pub fn set(&mut self, factor: f32) {
        self.factor = factor;
    }

    /// The factor actually applied: [`Self::factor`] clamped to
    /// [`Self::MIN`]..=[`Self::MAX`], with anything that is not a finite
    /// number treated as `1.0`.
    pub fn effective(&self) -> f32 {
        if self.factor.is_finite() {
            self.factor.clamp(Self::MIN, Self::MAX)
        } else {
            1.0
        }
    }

    /// `fit` (the canvas scaler's own result) with the player's factor on top.
    pub fn apply(&self, fit: f32) -> f32 {
        fit * self.effective()
    }
}

impl Default for GameUiScale {
    fn default() -> Self {
        Self { factor: 1.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_leaves_the_fit_alone() {
        assert_eq!(GameUiScale::default().apply(0.75), 0.75);
    }

    #[test]
    fn the_factor_multiplies_the_fit() {
        let s = GameUiScale { factor: 1.5 };
        assert!((s.apply(0.5) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn nonsense_factors_still_give_a_readable_ui() {
        assert_eq!(GameUiScale { factor: 0.0 }.effective(), GameUiScale::MIN);
        assert_eq!(GameUiScale { factor: 100.0 }.effective(), GameUiScale::MAX);
        assert_eq!(GameUiScale { factor: f32::NAN }.effective(), 1.0);
    }
}
