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

/// Pick a gait and switch to it only when it actually changes.
fn drive_gait(q: Query<(Entity, &Locomotion, &AnimState)>, mut cmds: Commands) {
    for (entity, loco, anim) in &q {
        let want = if loco.speed >= loco.run_at {
            "run"
        } else if loco.speed >= loco.walk_at {
            "walk"
        } else {
            "idle"
        };

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
