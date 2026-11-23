//! Work a freshly loaded scene still has to finish before it is ready to be
//! seen, reported by the crates doing it.
//!
//! A scene load spawns entities in a frame, but much of a level fills in
//! after: a terrain builds its chunks a few a frame, a model's meshes arrive
//! from its file. The asset-load progress a loading screen reads
//! (`asset_progress()`) only counted model files, so the screen came down
//! while the ground was still appearing behind it. Each owner now keeps a
//! count here under its own id, and the progress tracker waits for every
//! count to reach zero after a scene loads.
//!
//! Report what is still to do, not what is done, and keep reporting zero when
//! there is nothing: an owner that stops writing leaves its last count behind.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;

/// Things still to be built for the scene, by owner.
#[derive(Resource, Debug, Clone, Default)]
pub struct SceneLoadWork {
    pending: HashMap<&'static str, u32>,
}

impl SceneLoadWork {
    /// Set `id`'s count of things still to do. Zero removes it.
    pub fn set(&mut self, id: &'static str, pending: u32) {
        if pending == 0 {
            self.pending.remove(id);
        } else {
            self.pending.insert(id, pending);
        }
    }

    /// Everything still to do, across every owner.
    pub fn total(&self) -> u32 {
        self.pending.values().sum()
    }

    /// `id`'s count, for an owner deciding whether its write would change it.
    pub fn get(&self, id: &str) -> u32 {
        self.pending.get(id).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_add_up_and_zero_clears() {
        let mut work = SceneLoadWork::default();
        work.set("terrain", 12);
        work.set("materials", 3);
        assert_eq!(work.total(), 15);
        work.set("terrain", 0);
        assert_eq!(work.total(), 3);
        assert_eq!(work.get("terrain"), 0);
    }
}
