//! Shuffling the order systems run in, to find the ones that only work by luck.
//!
//! Two systems with no ordering between them run in whatever order the schedule
//! builder happens to pick, and that order is stable from one launch to the
//! next. So a system that quietly depends on running after another one works
//! every single time, until an unrelated change moves it, and then it breaks
//! somewhere nobody was looking. Several of the editor's menu and popup bugs
//! were exactly that.
//!
//! Setting `RENZORA_SHUFFLE_SEED=<number>` makes Bevy pick a different, still
//! valid, order for every schedule: every `before`/`after`/`chain` is honoured,
//! only the freedom left over is shuffled. A bug that appears under one seed
//! and not another is an ordering bug, and the same seed reproduces it.
//!
//! Off unless the variable is set, so a normal launch is unchanged.

use bevy::prelude::*;

/// The environment variable holding the seed.
pub const SEED_VAR: &str = "RENZORA_SHUFFLE_SEED";

/// Installs the shuffle when [`SEED_VAR`] holds a number. Add it after every
/// other plugin: the seed is applied in `cleanup`, which runs after every
/// plugin's `finish`, so schedules a plugin only creates there are covered too.
pub fn install(app: &mut App) {
    let Ok(raw) = std::env::var(SEED_VAR) else {
        return;
    };
    match raw.trim().parse::<u64>() {
        Ok(seed) => {
            app.add_plugins(ScheduleShufflePlugin { seed });
        }
        Err(_) => warn!("[shuffle] {SEED_VAR}={raw:?} is not a whole number; systems run in their usual order"),
    }
}

/// Gives every schedule, in the main world and in every sub-app (the render
/// world included), the same shuffle seed.
pub struct ScheduleShufflePlugin {
    pub seed: u64,
}

impl Plugin for ScheduleShufflePlugin {
    fn build(&self, _app: &mut App) {}

    fn cleanup(&self, app: &mut App) {
        let mut count = 0;
        count += seed_schedules(app.world_mut(), self.seed);
        for sub_app in app.sub_apps_mut().iter_mut() {
            count += seed_schedules(sub_app.world_mut(), self.seed);
        }
        warn!(
            "[shuffle] system order shuffled with seed {} across {count} schedules; \
             unset {SEED_VAR} for the normal order",
            self.seed
        );
    }
}

fn seed_schedules(world: &mut World, seed: u64) -> usize {
    let Some(mut schedules) = world.get_resource_mut::<Schedules>() else {
        return 0;
    };
    let mut count = 0;
    for (_, schedule) in schedules.iter_mut() {
        let mut settings = schedule.get_build_settings();
        settings.shuffle_seed = Some(seed);
        schedule.set_build_settings(settings);
        count += 1;
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Order(Vec<u8>);

    fn first(mut order: ResMut<Order>) {
        order.0.push(1);
    }
    fn second(mut order: ResMut<Order>) {
        order.0.push(2);
    }
    fn third(mut order: ResMut<Order>) {
        order.0.push(3);
    }

    fn run_with(seed: Option<u64>, chained: bool) -> Vec<u8> {
        let mut app = App::new();
        app.init_resource::<Order>();
        if chained {
            app.add_systems(Update, (first, second, third).chain());
        } else {
            app.add_systems(Update, (first, second, third));
        }
        if let Some(seed) = seed {
            app.add_plugins(ScheduleShufflePlugin { seed });
        }
        app.finish();
        app.cleanup();
        app.update();
        std::mem::take(&mut app.world_mut().resource_mut::<Order>().0)
    }

    /// Unordered systems land in a different order under some seed, which is
    /// the whole point.
    #[test]
    fn a_seed_reorders_unordered_systems() {
        let usual = run_with(None, false);
        let moved = (0..64).any(|seed| run_with(Some(seed), false) != usual);
        assert!(moved, "no seed changed the order of three unordered systems");
    }

    /// Declared ordering is never shuffled away.
    #[test]
    fn a_seed_keeps_declared_order() {
        for seed in 0..64 {
            assert_eq!(run_with(Some(seed), true), vec![1, 2, 3], "seed {seed}");
        }
    }

    /// The same seed gives the same order, or a failure could not be replayed.
    #[test]
    fn a_seed_is_reproducible() {
        for seed in 0..16 {
            assert_eq!(run_with(Some(seed), false), run_with(Some(seed), false));
        }
    }
}
