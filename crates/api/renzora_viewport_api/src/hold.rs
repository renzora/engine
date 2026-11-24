//! Asking the editor viewport to stop rendering for a while.
//!
//! The viewport already pauses while a game started with Play runs in its own
//! window. Other work wants the same: an export compiles the engine for
//! minutes, and every frame the viewport draws behind the export dialog is GPU
//! and CPU time taken from the build, for a picture nobody can see under the
//! dialog. A crate that wants the viewport paused names a reason here; it
//! renders again, once, when the last reason is released.
//!
//! A contract type, because the viewport cannot depend on the crates that ask
//! (the exporter, and whatever comes next), and they cannot depend on it.

use bevy::prelude::*;

/// Every reason the viewport is currently held paused.
#[derive(Resource, Default, Debug)]
pub struct ViewportHold {
    reasons: Vec<&'static str>,
}

impl ViewportHold {
    /// Hold the viewport paused for `reason` (`held = true`), or release that
    /// reason. Idempotent, so a system may call it every frame with the
    /// current answer; it only changes the list when the answer changes.
    pub fn set(&mut self, reason: &'static str, held: bool) {
        let has = self.reasons.contains(&reason);
        if held && !has {
            self.reasons.push(reason);
        } else if !held && has {
            self.reasons.retain(|r| *r != reason);
        }
    }

    /// Whether anything is holding the viewport paused.
    pub fn is_held(&self) -> bool {
        !self.reasons.is_empty()
    }

    /// What is holding it, for the log.
    pub fn reasons(&self) -> &[&'static str] {
        &self.reasons
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_viewport_is_held_until_every_reason_is_released() {
        let mut hold = ViewportHold::default();
        assert!(!hold.is_held());
        hold.set("export", true);
        hold.set("export", true);
        hold.set("bake", true);
        assert_eq!(hold.reasons(), ["export", "bake"]);
        hold.set("export", false);
        assert!(hold.is_held());
        hold.set("bake", false);
        assert!(!hold.is_held());
    }
}
