//! Physics for standalone plugins.
//!
//! ```ignore
//! use renzora_plugin::prelude::*;
//! use renzora_plugin::physics::{PhysicsCommands, PhysicsState};
//!
//! fn jump(q: Query<(Entity, &PhysicsState)>, input: Res<Input>, mut cmds: Commands) {
//!     for (entity, phys) in &q {
//!         if phys.is_grounded() && input.just_pressed(Key::Space) {
//!             cmds.entity(entity).apply_impulse(Vec3 { x: 0.0, y: 5.0, z: 0.0 });
//!         }
//!     }
//! }
//! ```
//!
//! A domain module, exactly like [`anim`](crate::anim): it rides on the generic
//! [`CommandKind::Service`](crate::sys::CommandKind::Service) channel and
//! [`sys`](crate::sys) knows nothing about it. See `anim`'s module doc for why
//! that split exists and why adding a domain does not move the ABI version.
//!
//! The engine side is `renzora_physics::plugin_bridge`. If that crate is absent
//! — a 2D-only export, a headless server with physics stripped — these calls
//! are discarded at end of frame and nothing breaks.

use crate::ecs::EntityCommands;
use crate::sys::{self, Vec3};

/// Identifies this service in the host's queue.
pub const SERVICE: u64 = sys::service_id("renzora.physics");

/// Which physics operation a [`PhysicsCommand`] performs.
///
/// A newtype rather than an `enum` for the same soundness reason the
/// mechanism's own enums are — the engine reads this out of plugin memory, and
/// an out-of-range discriminant materialised into a Rust enum is undefined
/// behaviour.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhysicsOp(pub u32);

#[allow(non_upper_case_globals)]
impl PhysicsOp {
    /// Continuous force in `vec`, applied while it keeps being set.
    pub const ApplyForce: Self = Self(0);
    /// One-shot impulse in `vec` — an instantaneous change in momentum.
    pub const ApplyImpulse: Self = Self(1);
    /// Set linear velocity to `vec` outright.
    pub const SetVelocity: Self = Self(2);
    /// Move a kinematic body by `vec`, sliding along surfaces steeper than
    /// `value` degrees rather than climbing them.
    pub const KinematicSlide: Self = Self(3);

    pub const fn is_known(self) -> bool {
        self.0 < 4
    }

    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "ApplyForce",
            1 => "ApplyImpulse",
            2 => "SetVelocity",
            3 => "KinematicSlide",
            _ => "?",
        }
    }
}

impl core::fmt::Debug for PhysicsOp {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

/// One physics operation, as a service payload.
///
/// Flat and plain-old-data: the host copies these bytes without reading them,
/// and a Rust enum with payloads has no guaranteed layout.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PhysicsCommand {
    pub op: PhysicsOp,
    /// Force, impulse, velocity or movement delta, depending on `op`.
    pub vec: Vec3,
    /// Max climbable slope in degrees, for [`PhysicsOp::KinematicSlide`].
    /// Unused by the others.
    pub value: f32,
}

impl PhysicsCommand {
    /// The payload bytes, for [`EntityCommands::call_service`].
    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY: `#[repr(C)]`, no pointers, no `Drop`.
        unsafe {
            core::slice::from_raw_parts(
                (self as *const Self).cast::<u8>(),
                core::mem::size_of::<Self>(),
            )
        }
    }
}

/// A body's state, mirrored for a plugin to read through a query.
///
/// A component rather than a call, so a controller that checks `is_grounded()`
/// every frame makes no calls back into the engine.
///
/// Collision *names* are deliberately absent. The engine's own mirror carries
/// `entered_name` / `exited_name`, but a name is only useful for matching, and
/// matching wants [`Str256`](crate::sys::Str256) comparisons a plugin can do
/// against its own constants — which is a bigger surface than the flags below
/// and is not needed to know that a collision happened.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PhysicsState {
    /// Linear velocity.
    pub velocity: Vec3,
    /// Surface normal under the body; zero when not grounded.
    pub ground_normal: Vec3,
    /// Magnitude of `velocity`, precomputed because controllers ask constantly.
    pub speed: f32,
    /// 1 while standing on something.
    pub grounded: u32,
    /// 1 while overlapping any other collider.
    pub colliding: u32,
    /// 1 on the frame a new contact began.
    pub entered: u32,
    /// 1 on the frame a contact ended.
    pub exited: u32,
    /// Keeps the struct's size a multiple of 8.
    pub _reserved: u32,
}

impl PhysicsState {
    pub const fn is_grounded(&self) -> bool {
        self.grounded != 0
    }
    pub const fn is_colliding(&self) -> bool {
        self.colliding != 0
    }
    /// True only on the frame a contact began.
    pub const fn just_entered(&self) -> bool {
        self.entered != 0
    }
    /// True only on the frame a contact ended.
    pub const fn just_exited(&self) -> bool {
        self.exited != 0
    }
}

// The engine side wraps this very struct `#[repr(transparent)]`, so the two
// layouts cannot drift. The path below is the only thing tying them together —
// nothing links, and `plugin_bridge::install` asserts they agree at startup.
crate::host_component!(
    PhysicsState,
    "renzora_physics::plugin_bridge::PluginPhysicsState"
);

/// Physics methods on [`EntityCommands`].
///
/// An extension trait because `EntityCommands` belongs to the mechanism, which
/// does not know this module exists. Bring it into scope to use any of them.
pub trait PhysicsCommands {
    /// Queue one physics operation. The others are wrappers.
    fn physics(&mut self, cmd: PhysicsCommand) -> &mut Self;

    /// Apply a continuous force. Set it every frame for sustained thrust.
    fn apply_force(&mut self, force: Vec3) -> &mut Self;
    /// Apply a one-shot impulse — a jump, a knockback, a launch.
    fn apply_impulse(&mut self, impulse: Vec3) -> &mut Self;
    /// Set linear velocity outright, ignoring whatever it was.
    fn set_velocity(&mut self, velocity: Vec3) -> &mut Self;
    /// Move a kinematic body, sliding along anything steeper than
    /// `max_slope_degrees` instead of walking up it.
    fn kinematic_slide(&mut self, delta: Vec3, max_slope_degrees: f32) -> &mut Self;
}

impl PhysicsCommands for EntityCommands<'_> {
    fn physics(&mut self, cmd: PhysicsCommand) -> &mut Self {
        self.call_service(SERVICE, cmd.op.0, cmd.as_bytes())
    }

    fn apply_force(&mut self, force: Vec3) -> &mut Self {
        self.physics(PhysicsCommand {
            op: PhysicsOp::ApplyForce,
            vec: force,
            value: 0.0,
        })
    }

    fn apply_impulse(&mut self, impulse: Vec3) -> &mut Self {
        self.physics(PhysicsCommand {
            op: PhysicsOp::ApplyImpulse,
            vec: impulse,
            value: 0.0,
        })
    }

    fn set_velocity(&mut self, velocity: Vec3) -> &mut Self {
        self.physics(PhysicsCommand {
            op: PhysicsOp::SetVelocity,
            vec: velocity,
            value: 0.0,
        })
    }

    fn kinematic_slide(&mut self, delta: Vec3, max_slope_degrees: f32) -> &mut Self {
        self.physics(PhysicsCommand {
            op: PhysicsOp::KinematicSlide,
            vec: delta,
            value: max_slope_degrees,
        })
    }
}
