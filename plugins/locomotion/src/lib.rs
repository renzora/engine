#![no_std]
//! Drives an animator from movement speed, as a standalone C-ABI plugin.
//!
//! This is the worked example for the animation surface, and it exists to show
//! the half people get wrong: **reading** animation state.
//!
//! The naive version of this plugin is three lines and works badly —
//!
//! ```ignore
//! if loco.speed > loco.run_at { cmds.entity(e).crossfade_animation("run", 0.2); }
//! ```
//!
//! — because it re-issues the crossfade every frame the character is running,
//! so the blend restarts sixty times a second and the animation never actually
//! plays. The fix is not to track "what did I last request?" in the plugin, which
//! goes wrong the moment anything else drives the same animator. It is to ask
//! the animator what it is doing, which is what [`AnimState`] is for:
//!
//! ```ignore
//! if !anim.is_clip(want) { cmds.entity(e).crossfade_animation(want, 0.2); }
//! ```
//!
//! That read costs nothing. `AnimState` arrives as an ordinary query cell, so a
//! system checking it every frame makes no calls back into the engine at all.
//!
//! Names cross as hashes, so `is_clip("run")` is a comparison against a value
//! folded at compile time — the plugin never sees the string and does not need
//! to. What it cannot do is *discover* a clip name it was not already looking
//! for; see `sys::AnimState` for why that trade is the right one.

extern crate alloc;

// Supplies the global allocator and panic handler that `std` would have. Expands
// to nothing under `std` or `static_link`, so this is safe whichever way the
// plugin ends up linked.
renzora_plugin::no_std_runtime!();

use renzora_plugin::prelude::*;
// Animation is a feature-gated domain module, not part of the ABI, so it is not
// in the prelude. `AnimCommands` is an extension trait — the boundary owns
// `EntityCommands` and has never heard of animation — so it must be in scope for
// `crossfade_animation` to exist at all.
use renzora_plugin::anim::{AnimCommands, AnimState};

/// Attach to an animated character. `speed` is written by whatever moves it —
/// another plugin, a host system, or the inspector while you watch.
#[derive(Component)]
#[component(name = "Locomotion")]
#[repr(C)]
pub struct Locomotion {
    /// Current ground speed, in units per second.
    #[field(min = 0.0, max = 12.0, speed = 0.05)]
    pub speed: f32,
    /// Above this, walk.
    #[field(min = 0.0, max = 12.0, speed = 0.05)]
    pub walk_at: f32,
    /// Above this, run.
    #[field(min = 0.0, max = 12.0, speed = 0.05)]
    pub run_at: f32,
    /// Blend time between gaits, in seconds.
    #[field(min = 0.0, max = 1.0, speed = 0.01)]
    pub blend: f32,
}

impl Default for Locomotion {
    fn default() -> Self {
        Self { speed: 0.0, walk_at: 0.1, run_at: 4.0, blend: 0.2 }
    }
}

/// Which clip a given speed calls for.
///
/// Split out of [`drive_gait`] so the thresholds can be tested without a host: a
/// plugin's `Query` is backed by the interface table, so the system itself only
/// runs inside a real engine, while this — the part that can actually be wrong —
/// is ordinary arithmetic.
///
/// `run` is checked first, so a configuration with `run_at` below `walk_at`
/// resolves to `run` rather than to whichever branch happens to come first.
fn gait_for(speed: f32, walk_at: f32, run_at: f32) -> &'static str {
    if speed >= run_at {
        "run"
    } else if speed >= walk_at {
        "walk"
    } else {
        "idle"
    }
}

/// Pick a gait and switch to it only when it actually changes.
fn drive_gait(q: Query<(Entity, &Locomotion, &AnimState)>, mut cmds: Commands) {
    for (entity, loco, anim) in &q {
        let want = gait_for(loco.speed, loco.walk_at, loco.run_at);

        // The whole point of the example. Without this guard the crossfade
        // restarts every frame and nothing ever finishes blending.
        if anim.is_clip(want) {
            continue;
        }
        cmds.entity(entity).crossfade_animation(want, loco.blend);
    }
}

/// Feed the state machine too, so a character rigged with one behaves the same.
///
/// Setting a parameter unconditionally is fine — unlike a crossfade, writing the
/// same float twice is not a restart — so this needs no guard and no read.
fn drive_params(q: Query<(Entity, &Locomotion)>, mut cmds: Commands) {
    for (entity, loco) in &q {
        cmds.entity(entity)
            .set_anim_param("speed", loco.speed)
            .set_anim_bool("moving", loco.speed >= loco.walk_at);
    }
}

pub struct LocomotionPlugin;

impl Plugin for LocomotionPlugin {
    fn build(&self, app: &mut App) {
        // `AnimState` is a HOST component — `renzora_animation` maintains it —
        // so registering it here resolves its id rather than creating anything.
        // Without this the query term has no id and the system is refused.
        app.register_component::<Locomotion>()
            .register_component::<AnimState>()
            .add_systems(Update, drive_gait)
            .add_systems(Update, drive_params);
    }
}

renzora_plugin::add!(LocomotionPlugin);

#[cfg(test)]
mod tests {
    use super::*;

    fn gait(speed: f32) -> &'static str {
        let d = Locomotion::default();
        gait_for(speed, d.walk_at, d.run_at)
    }

    #[test]
    fn a_standing_character_idles() {
        assert_eq!(gait(0.0), "idle");
    }

    #[test]
    fn speed_selects_walk_then_run() {
        assert_eq!(gait(1.0), "walk");
        assert_eq!(gait(8.0), "run");
    }

    /// The comparisons are `>=`, so a speed sitting exactly on a threshold takes
    /// the faster gait. Flipping either to `>` leaves a character moving at
    /// exactly `walk_at` playing the idle clip — feet planted while it slides.
    #[test]
    fn a_speed_exactly_on_a_threshold_takes_the_faster_gait() {
        let d = Locomotion::default();
        assert_eq!(gait_for(d.walk_at, d.walk_at, d.run_at), "walk");
        assert_eq!(gait_for(d.run_at, d.walk_at, d.run_at), "run");
    }

    #[test]
    fn just_below_a_threshold_keeps_the_slower_gait() {
        let d = Locomotion::default();
        assert_eq!(gait_for(d.walk_at - 0.001, d.walk_at, d.run_at), "idle");
        assert_eq!(gait_for(d.run_at - 0.001, d.walk_at, d.run_at), "walk");
    }

    /// The thresholds are inspector fields with independent ranges, so nothing
    /// stops a user dragging `run_at` below `walk_at`. Checking `run` first means
    /// that resolves to `run` — one consistent answer — rather than depending on
    /// branch order.
    #[test]
    fn inverted_thresholds_still_give_one_answer() {
        assert_eq!(gait_for(5.0, 8.0, 2.0), "run");
        assert_eq!(gait_for(1.0, 8.0, 2.0), "idle");
    }

    /// A character driven backwards has a negative speed, and must not be
    /// treated as running.
    #[test]
    fn a_negative_speed_idles() {
        assert_eq!(gait(-3.0), "idle");
    }

    /// Every gait this returns must be a clip name the character actually has,
    /// and `is_clip`/`crossfade_animation` match on the string.
    #[test]
    fn only_the_three_known_clip_names_are_ever_returned() {
        for speed in [-10.0f32, 0.0, 0.05, 0.1, 1.0, 3.9, 4.0, 100.0] {
            assert!(
                matches!(gait(speed), "idle" | "walk" | "run"),
                "speed {speed} produced {:?}",
                gait(speed)
            );
        }
    }

    /// Defaults have to be ordered, or a fresh component walks and runs at the
    /// same speed and the gait never settles.
    #[test]
    fn default_thresholds_are_ordered_and_start_idle() {
        let d = Locomotion::default();
        assert!(d.walk_at < d.run_at, "walk_at {} !< run_at {}", d.walk_at, d.run_at);
        assert!(d.walk_at > 0.0, "a walk_at of 0 would walk while standing still");
        assert!(d.blend > 0.0, "a blend of 0 snaps between clips");
        assert_eq!(gait_for(d.speed, d.walk_at, d.run_at), "idle");
    }
}
