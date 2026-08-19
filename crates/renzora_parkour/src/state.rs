//! The controller's own state: what the character is doing right now, and the
//! scratch data each state needs to keep doing it.
//!
//! None of this is authored — it is all runtime scratch, so none of it is
//! `Inspectable` and only [`ParkourState`] is reflected (the read-state mirror
//! surfaces it to scripts as a string instead; see [`crate::read_state`]).

use bevy::prelude::*;

/// What the character is doing. Exactly one of these is true at a time, and
/// each one owns motion completely while it is active — that is the point of
/// the enum. A traversal has to move the capsule along a path that gravity plus
/// collide-and-slide would never produce, so "walking" cannot still be running
/// underneath it.
#[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ParkourState {
    /// Standing/running on walkable ground.
    #[default]
    Grounded,
    /// In the air under gravity — falling, jumping, or just walked off a ledge.
    Airborne,
    /// Playing a vault: over a low obstacle and down the far side.
    Vaulting,
    /// Playing a mantle: up and onto the top of a ledge.
    Mantling,
    /// Hanging from a ledge by the hands. Lateral input shimmies.
    Hanging,
    /// On a ladder. Vertical input climbs.
    ClimbingLadder,
    /// Running along a wall, on the clock.
    WallRunning,
    /// Swinging from a [`crate::ParkourSwingAnchor`] on a fixed-length rope.
    Swinging,
}

impl ParkourState {
    /// Stable lowercase name, for `get("ParkourReadState.state")` and the
    /// `ParkourEvent` payload. Scripts compare against these strings, so they
    /// are API — renaming one is a breaking change.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Grounded => "grounded",
            Self::Airborne => "airborne",
            Self::Vaulting => "vaulting",
            Self::Mantling => "mantling",
            Self::Hanging => "hanging",
            Self::ClimbingLadder => "climbing",
            Self::WallRunning => "wall_running",
            Self::Swinging => "swinging",
        }
    }

    /// The state a freshly spawned controller starts in — see
    /// [`ParkourMotion::default`] for why it is not `Grounded`.
    pub fn default_start() -> Self {
        Self::Airborne
    }

    /// True while an authored traversal owns the capsule. Scripts read the
    /// mirror of this (`ParkourReadState.traversing`) to stop feeding movement
    /// input — not because input would break anything (it is ignored), but so
    /// the game's own camera and aiming can hold still through the move.
    pub fn is_traversal(self) -> bool {
        matches!(self, Self::Vaulting | Self::Mantling)
    }
}

/// Where a traversal hands the character back to when it finishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraversalExit {
    /// Drop back into the normal ground/air states (the usual case).
    Free,
    /// Finish hanging from the ledge that was reached — used when a grab is
    /// what triggered the move, so the character ends up on the lip rather
    /// than standing on top of it.
    Hang,
}

/// A parametric move along a fixed path, ignoring gravity and collision.
///
/// Vaults and mantles are *authored* motion, not simulated motion: the probe
/// that started one has already proved the whole path is clear, so re-testing
/// it every frame can only introduce popping — a capsule sweep collides with
/// the very obstacle the character is deliberately passing over. The curve is a
/// quadratic Bézier so the control point can sit above both ends, which is what
/// makes a vault arc over the obstacle instead of shearing through its corner.
#[derive(Clone, Copy, Debug)]
pub struct Traversal {
    pub start: Vec3,
    /// Bézier control point — not a position the character passes through, it
    /// pulls the curve toward itself.
    pub apex: Vec3,
    pub end: Vec3,
    pub duration: f32,
    pub elapsed: f32,
    pub exit: TraversalExit,
    /// Velocity handed to whichever state takes over, so a vault keeps the
    /// character's momentum instead of dropping it dead at the far side.
    pub exit_velocity: Vec3,
}

impl Traversal {
    /// Position at the current `elapsed`, with both ends eased so the
    /// character neither snaps into motion nor stops dead.
    pub fn sample(&self) -> Vec3 {
        let t = (self.elapsed / self.duration.max(1e-4)).clamp(0.0, 1.0);
        let t = t * t * (3.0 - 2.0 * t); // smoothstep
        let inv = 1.0 - t;
        self.start * (inv * inv) + self.apex * (2.0 * inv * t) + self.end * (t * t)
    }

    pub fn finished(&self) -> bool {
        self.elapsed >= self.duration
    }
}

/// The ledge a hang is anchored to.
#[derive(Clone, Copy, Debug)]
pub struct HangGrip {
    /// World point on the lip the hands are on.
    pub point: Vec3,
    /// Outward normal of the *wall* below the lip — the direction the
    /// character's back faces. Shimmying runs perpendicular to it.
    pub wall_normal: Vec3,
}

/// The anchor a swing is attached to.
#[derive(Clone, Copy, Debug)]
pub struct SwingLink {
    pub anchor: Entity,
    /// Rope length, fixed at grab time. Letting it change mid-swing would pump
    /// energy into the pendulum, so it is captured once.
    pub length: f32,
}

/// Runtime scratch for [`crate::ParkourController`]. Inserted automatically;
/// never authored, never saved.
#[derive(Component, Debug)]
pub struct ParkourMotion {
    pub state: ParkourState,
    /// Seconds spent in the current state. Wall running and traversals are on
    /// the clock, and the animation driver uses it to avoid restarting a clip.
    pub state_time: f32,
    pub velocity: Vec3,
    pub traversal: Option<Traversal>,
    /// Grace period after walking off an edge during which a jump still works.
    pub coyote: f32,
    /// Grace period *before* landing during which a jump press is remembered.
    pub jump_buffer: f32,
    /// Buffered context action, same idea as `jump_buffer`: pressing "vault" a
    /// fraction of a second early should still vault.
    pub action_buffer: f32,
    pub hang: Option<HangGrip>,
    pub ladder: Option<Entity>,
    /// Where the character latched onto the ladder. The climb only moves in Y
    /// from here — a ladder is a rail, and re-deriving the hug position from
    /// the ladder's own transform every frame would need to know which face
    /// of it is climbable, which nothing states.
    pub ladder_grip: Option<HangGrip>,
    pub swing: Option<SwingLink>,
    /// Surface normal of the wall a wall-run is riding.
    pub wall_normal: Vec3,
    /// Blocks re-attaching to a wall immediately after leaving one. Without it
    /// a wall jump puts the character straight back on the wall it just left,
    /// and they climb it in place.
    pub wall_cooldown: f32,
    /// Set when a wall run starts, cleared on touching the ground. Without it
    /// `wall_run_duration` limits nothing: the run ends, the character falls a
    /// few centimetres, the same wall is still there, and it starts again — so
    /// holding forward carries them along an endless wall forever. A *different*
    /// wall (the far side of a corridor) is still allowed straight away, which
    /// is the trick the limit exists to permit.
    pub wall_used: bool,
    /// Blocks re-grabbing a ledge or a swing anchor for a moment after letting
    /// go of one. Dropping off a ledge would otherwise re-grab the same lip on
    /// the next frame, and the character would hang there forever.
    pub grab_cooldown: f32,
    /// Yaw the character is turning toward, in radians.
    pub facing: f32,
    /// Clip the animation driver last asked for, so it crossfades on a real
    /// change rather than every frame.
    pub last_clip: String,
    /// What the probe saw this frame, and the direction it was cast in. Kept
    /// only so the editor gizmo can draw what the controller actually decided
    /// from, rather than re-casting its own rays and showing something subtly
    /// different from the state machine's view of the world.
    pub last_probe: crate::probe::ParkourProbe,
    pub last_forward: Vec3,
}

impl Default for ParkourMotion {
    fn default() -> Self {
        Self {
            // Airborne, not Grounded: a character that spawns slightly above
            // the floor should fall to it. The first ground probe promotes it
            // to Grounded on the same frame if it is already standing.
            state: ParkourState::default_start(),
            state_time: 0.0,
            velocity: Vec3::ZERO,
            traversal: None,
            coyote: 0.0,
            jump_buffer: 0.0,
            action_buffer: 0.0,
            hang: None,
            ladder: None,
            ladder_grip: None,
            swing: None,
            wall_normal: Vec3::ZERO,
            wall_cooldown: 0.0,
            wall_used: false,
            grab_cooldown: 0.0,
            facing: 0.0,
            last_clip: String::new(),
            last_probe: crate::probe::ParkourProbe::default(),
            last_forward: Vec3::NEG_Z,
        }
    }
}

impl ParkourMotion {
    /// Switch state, resetting the per-state clock. Callers that need the old
    /// state (to emit an event) read it before calling.
    pub fn enter(&mut self, state: ParkourState) {
        if self.state != state {
            self.state = state;
            self.state_time = 0.0;
        }
    }
}

/// Movement intent for one frame, written by the `parkour_*` script actions
/// and consumed by the drive system.
///
/// The one-shot fields are *buffered*, not level-triggered: the drive system
/// clears them when it consumes them, so a press that arrives after the drive
/// system has already run this frame survives into the next one instead of
/// being lost. Nothing here is authored or saved.
#[derive(Component, Default, Debug)]
pub struct ParkourInput {
    /// Desired movement in world space. The XZ part is a direction whose
    /// length scales speed (so an analogue stick works as-is); `y` is the
    /// vertical intent ladders and hangs read.
    pub move_dir: Vec3,
    pub sprint: bool,
    pub jump_pressed: bool,
    pub action_pressed: bool,
    pub release_pressed: bool,
}
