//! Driving the skeleton from the controller's state.
//!
//! One clip per state, crossfaded when the state changes. The commands go
//! through `renzora_animation`'s queue rather than at `AnimationPlayer`
//! directly, so a parkour crossfade and a script's own `crossfade_animation()`
//! are the same kind of request and resolve in the same order — reaching past
//! the queue is how you get two systems writing the pose and neither winning.
//!
//! Every clip name is optional. An empty name means "this crate doesn't drive
//! that state", which is the right default for a project whose characters are
//! animated by a state machine of its own: leave [`ParkourAnimations`] off the
//! entity entirely and nothing here runs.

use bevy::prelude::*;
use renzora_animation::{AnimationCommand, AnimationCommandQueue};

use crate::state::{ParkourMotion, ParkourState};
use crate::{ParkourAnimations, ParkourController};

/// Pick the clip for the current state and crossfade to it if it changed.
pub fn drive_parkour_animation(
    mut queue: Option<ResMut<AnimationCommandQueue>>,
    mut characters: Query<(
        Entity,
        &ParkourController,
        &ParkourAnimations,
        &mut ParkourMotion,
    )>,
) {
    let Some(queue) = queue.as_mut() else {
        return;
    };

    for (entity, controller, clips, mut motion) in &mut characters {
        let ground_speed = Vec3::new(motion.velocity.x, 0.0, motion.velocity.z).length();
        let (clip, looping) = match motion.state {
            ParkourState::Grounded => {
                if ground_speed < 0.2 {
                    (&clips.idle, true)
                } else if ground_speed < controller.walk_speed * 1.2 {
                    (&clips.walk, true)
                } else {
                    (&clips.run, true)
                }
            }
            // Rising vs falling, rather than "jumped": a character who walked
            // off a ledge is falling, and should not play the jump clip.
            ParkourState::Airborne => {
                if motion.velocity.y > 0.5 {
                    (&clips.jump, false)
                } else {
                    (&clips.fall, true)
                }
            }
            ParkourState::Vaulting => (&clips.vault, false),
            ParkourState::Mantling => (&clips.mantle, false),
            ParkourState::Hanging => {
                // Shimmying keeps a non-zero velocity purely so this (and the
                // read-state mirror) can tell the two apart — a hang never
                // moves by integrating it.
                if ground_speed > 0.05 && !clips.shimmy.is_empty() {
                    (&clips.shimmy, true)
                } else {
                    (&clips.hang, true)
                }
            }
            ParkourState::ClimbingLadder => (&clips.climb, true),
            ParkourState::WallRunning => (&clips.wall_run, true),
            ParkourState::Swinging => (&clips.swing, true),
        };

        if clip.is_empty() || *clip == motion.last_clip {
            continue;
        }
        queue.commands.push(AnimationCommand::Crossfade {
            entity,
            name: clip.clone(),
            duration: clips.blend,
            looping,
        });
        motion.last_clip = clip.clone();
    }
}
