//! Traversal — the moves a character makes when the floor stops being enough.
//!
//! Vaulting a rail, mantling onto a roof, hanging off a lip and shimmying
//! along it, climbing a ladder, running along a wall and kicking off it,
//! swinging from a rope. One component ([`ParkourController`]) on the
//! character, two optional markers on the level ([`ParkourLadder`],
//! [`ParkourSwingAnchor`]), and five script verbs to drive it.
//!
//! # Why this owns locomotion
//!
//! The engine has no built-in character controller: a kinematic body is moved
//! by a script calling `move_controller()`, which is collide-and-slide and
//! nothing else. That is a fine contract right up until a state needs to place
//! the character somewhere collide-and-slide would never put them — hanging off
//! a lip, pinned to a sphere around a rope anchor, halfway through an arc over
//! a fence. Those are *positions*, not forces, and no amount of velocity gets
//! them exactly right.
//!
//! So [`ParkourController`] takes over the whole loop: gravity, ground
//! contact, jumping and collision response, plus the traversals. A script
//! feeds it intent with `parkour_move()` / `parkour_jump()` / `parkour_action()`
//! and reads `ParkourReadState` back. A script that also calls
//! `move_controller()` on the same entity is fighting it, and the last writer
//! that frame wins — pick one.
//!
//! It is a 3D feature and depends on the avian 3D backend directly. A 2D
//! export drops the whole crate rather than carrying an inert copy.
//!
//! # What the level has to say
//!
//! Almost nothing. Ledges, walls and their heights are found by casting rays
//! (see [`probe`]), so ordinary static geometry is vaultable and mantleable
//! with no authoring at all. Only the two things geometry cannot imply need a
//! marker: a ladder ([`ParkourLadder`]) is a design decision — a ladder and a
//! fence are the same shape — and a swing anchor ([`ParkourSwingAnchor`]) is a
//! point in space with nothing to detect. [`ParkourBlocker`] opts a piece of
//! geometry out when the automatic answer is wrong.
//!
//! # Only in play
//!
//! Every system is gated on scripts running, so the controller does nothing
//! while editing — a character placed in the scene stays exactly where it was
//! put. Use Play or Simulate to see it move.

pub mod anim;
pub mod drive;
pub mod probe;
pub mod read_state;
#[cfg(feature = "scripting")]
pub mod script_extension;
pub mod state;

use avian3d::prelude::Collider;
use bevy::prelude::*;
#[cfg(feature = "editor")]
use renzora::AppEditorExt;
use renzora::PlayModeState;

pub use read_state::ParkourReadState;
pub use state::{ParkourInput, ParkourMotion, ParkourState};

/// A character that can traverse the level.
///
/// Put it on the entity that *is* the character — the one whose `Transform`
/// should move, which is usually the same entity as the animator. It expects
/// the entity to be kinematic (or to have no physics body at all): a dynamic
/// rigid body would have the solver moving it as well, and the two would
/// fight. A collision shape is not required, since the controller sweeps a
/// capsule built from [`radius`](Self::radius) and [`height`](Self::height)
/// rather than whatever collider is authored — but one is still worth having
/// so *other* things can hit the character.
///
/// The defaults describe an adult human: 1.8 m tall, runs at 7 m/s, vaults a
/// 1.2 m rail, mantles a 2.3 m wall.
#[derive(Component, Reflect, Clone, Debug)]
#[cfg_attr(feature = "editor", derive(renzora_macros::Inspectable))]
// `Default` goes in the reflect list, not just the derive list, so a scene
// saved before a field existed still loads: `FromReflect` returns `None` for
// partial data and falls back to `ReflectDefault` for the missing fields.
#[reflect(Component, Default)]
#[cfg_attr(
    feature = "editor",
    inspectable(name = "Parkour Controller", icon = "PERSON_SIMPLE_RUN", category = "physics")
)]
pub struct ParkourController {
    // ── Body ─────────────────────────────────────────────────────────────
    /// Capsule radius, in metres.
    pub radius: f32,
    /// Total standing height, in metres. Reach for a ledge grab is derived
    /// from it, so a shorter character grabs lower ledges.
    pub height: f32,
    /// Distance from the entity's origin *down* to the soles. `0` means the
    /// origin is at the feet, which is how imported characters usually sit;
    /// set it to half the height if the origin is at the hips.
    pub foot_offset: f32,

    // ── Locomotion ───────────────────────────────────────────────────────
    pub walk_speed: f32,
    pub run_speed: f32,
    /// How fast the character reaches the target speed, in m/s².
    pub acceleration: f32,
    /// Fraction of `acceleration` that still applies in mid-air.
    pub air_control: f32,
    /// Downward acceleration, in m/s². Negative.
    pub gravity: f32,
    /// Fastest the character may fall, in m/s. Negative.
    pub terminal_velocity: f32,
    /// Apex of a standing jump, in metres. The launch speed is derived from
    /// this and `gravity`, so tuning gravity doesn't silently change how high
    /// the character jumps.
    pub jump_height: f32,
    /// Steepest surface that counts as ground, in degrees.
    pub max_slope: f32,
    /// Height the capsule walks straight over without a traversal. Also the
    /// distance the controller will snap down to stay on stairs and slopes.
    pub step_height: f32,
    /// Grace period after walking off an edge during which jump still works.
    pub coyote_time: f32,
    /// How long a jump or action press is remembered before it is discarded.
    pub jump_buffer_time: f32,
    /// Turn the character to face where it is moving.
    pub face_movement: bool,
    /// Turn rate, as a fraction of the remaining angle per second.
    pub turn_speed: f32,
    /// Degrees to rotate the *model* by, on top of the direction the character
    /// is actually facing. Purely cosmetic — it never changes which way they
    /// move, probe or traverse.
    ///
    /// This exists because "forward" is not agreed on. Bevy treats `-Z` as
    /// forward, but glTF characters — anything out of Mixamo especially —
    /// are usually authored facing `+Z`. Imported as-is, such a character
    /// travels in exactly the right direction while appearing to run
    /// backwards. Set this to `180` and they face the way they are going.
    pub facing_offset: f32,

    // ── Traversal ────────────────────────────────────────────────────────
    /// Vault and mantle on contact, without waiting for `parkour_action()`.
    /// On is the livelier default; off gives the game full control over when
    /// a traversal is allowed to start.
    pub auto_traverse: bool,
    /// How far ahead of the capsule obstacles are looked for, in metres.
    pub forward_reach: f32,
    /// Tallest obstacle that can be vaulted *over*, in metres.
    pub vault_max_height: f32,
    /// How far past the lip the ground must drop away for the obstacle to
    /// count as a rail to vault rather than a platform to climb.
    pub vault_max_depth: f32,
    pub vault_duration: f32,
    /// Tallest ledge that can be climbed *onto*, in metres.
    pub mantle_max_height: f32,
    pub mantle_duration: f32,

    // ── Ledges ───────────────────────────────────────────────────────────
    /// Catch ledges when airborne instead of falling past them.
    pub ledge_grab: bool,
    /// Sideways speed while hanging, in m/s.
    pub hang_shimmy_speed: f32,

    // ── Ladders ──────────────────────────────────────────────────────────
    /// Climb speed, in m/s. Scaled per-ladder by [`ParkourLadder`].
    pub climb_speed: f32,

    // ── Walls ────────────────────────────────────────────────────────────
    /// Enables both wall running and wall jumping.
    pub wall_run: bool,
    pub wall_run_speed: f32,
    /// How long a single wall run may last, in seconds.
    pub wall_run_duration: f32,
    /// Gravity during a wall run — much weaker than normal gravity, which is
    /// what makes the run read as running rather than sliding.
    pub wall_run_gravity: f32,
    /// Upward speed from a wall jump, in m/s.
    pub wall_jump_up: f32,
    /// Speed away from the wall on a wall jump, in m/s.
    pub wall_jump_away: f32,

    // ── Swings ───────────────────────────────────────────────────────────
    /// Enables grabbing [`ParkourSwingAnchor`]s.
    pub swing: bool,
    /// Extra upward speed on letting go, in m/s — the flourish at the end of
    /// a swing, and the difference between clearing the gap and not.
    pub swing_release_boost: f32,
}

impl Default for ParkourController {
    fn default() -> Self {
        Self {
            radius: 0.35,
            height: 1.8,
            foot_offset: 0.0,
            walk_speed: 4.0,
            run_speed: 7.0,
            acceleration: 40.0,
            air_control: 0.35,
            gravity: -22.0,
            terminal_velocity: -55.0,
            jump_height: 1.15,
            max_slope: 55.0,
            step_height: 0.4,
            coyote_time: 0.12,
            jump_buffer_time: 0.15,
            face_movement: true,
            turn_speed: 12.0,
            facing_offset: 0.0,
            auto_traverse: true,
            forward_reach: 0.55,
            vault_max_height: 1.2,
            vault_max_depth: 1.1,
            vault_duration: 0.45,
            mantle_max_height: 2.3,
            mantle_duration: 0.7,
            ledge_grab: true,
            hang_shimmy_speed: 1.3,
            climb_speed: 2.2,
            wall_run: true,
            wall_run_speed: 6.5,
            wall_run_duration: 1.5,
            wall_run_gravity: -3.0,
            wall_jump_up: 6.0,
            wall_jump_away: 5.0,
            swing: true,
            swing_release_boost: 2.0,
        }
    }
}

/// Clip names for each parkour state, crossfaded as the state changes.
///
/// Optional: leave the component off and the controller drives no animation at
/// all, which is what a project with its own animation state machine wants —
/// it can read `ParkourReadState.state` and decide for itself. An empty name
/// means "don't drive this one state", so a project can let the controller
/// handle locomotion and keep the traversals for itself.
#[derive(Component, Reflect, Clone, Debug)]
#[cfg_attr(feature = "editor", derive(renzora_macros::Inspectable))]
#[reflect(Component, Default)]
#[cfg_attr(
    feature = "editor",
    inspectable(
        name = "Parkour Animations",
        icon = "PERSON_SIMPLE_WALK",
        category = "animation"
    )
)]
pub struct ParkourAnimations {
    pub idle: String,
    pub walk: String,
    pub run: String,
    /// Played while rising. A character who walked off a ledge is falling, not
    /// jumping, and gets `fall` instead.
    pub jump: String,
    pub fall: String,
    pub vault: String,
    pub mantle: String,
    pub hang: String,
    /// Played instead of `hang` while shimmying sideways.
    pub shimmy: String,
    pub climb: String,
    pub wall_run: String,
    pub swing: String,
    /// Crossfade duration between clips, in seconds.
    pub blend: f32,
}

impl Default for ParkourAnimations {
    fn default() -> Self {
        // The names an imported character most often already has. They are
        // only defaults — nothing breaks if a clip is missing, the crossfade
        // request simply finds nothing to play.
        Self {
            idle: "idle".into(),
            walk: "walk".into(),
            run: "run".into(),
            jump: "jump".into(),
            fall: "fall".into(),
            vault: "vault".into(),
            mantle: "mantle".into(),
            hang: "hang".into(),
            shimmy: "shimmy".into(),
            climb: "climb".into(),
            wall_run: "wall_run".into(),
            swing: "swing".into(),
            blend: 0.15,
        }
    }
}

/// Marks geometry as a ladder.
///
/// Put it on the ladder object (or any ancestor of its collider — the lookup
/// walks up the hierarchy, so a collider buried in an imported model still
/// counts). Nothing about a ladder's *shape* distinguishes it from a fence, so
/// this is the one piece of climbing the level has to state outright.
#[derive(Component, Reflect, Clone, Copy, Debug)]
#[cfg_attr(feature = "editor", derive(renzora_macros::Inspectable))]
#[reflect(Component, Default)]
#[cfg_attr(
    feature = "editor",
    inspectable(name = "Parkour Ladder", icon = "LADDER_SIMPLE", category = "physics")
)]
pub struct ParkourLadder {
    /// Multiplies the controller's `climb_speed` — a rope ladder can be slower
    /// than a steel one without retuning the character.
    pub climb_speed_scale: f32,
    /// Latch on by walking into it. Off requires `parkour_action()`, which is
    /// what a game with a "press E" prompt wants.
    pub auto_attach: bool,
    /// Mantle onto the top when the climb runs out of ladder. Off leaves the
    /// character hanging at the top rung until they jump or let go — right for
    /// a ladder into a hatch, wrong for one up a wall.
    pub exit_at_top: bool,
}

impl Default for ParkourLadder {
    fn default() -> Self {
        Self {
            climb_speed_scale: 1.0,
            auto_attach: true,
            exit_at_top: true,
        }
    }
}

/// A point to swing from: a rope, a bar, a hook, a vine.
///
/// The entity's world position is the pivot. It needs no collider — swings are
/// found by proximity and a line-of-sight check, not by touching.
#[derive(Component, Reflect, Clone, Copy, Debug)]
#[cfg_attr(feature = "editor", derive(renzora_macros::Inspectable))]
#[reflect(Component, Default)]
#[cfg_attr(
    feature = "editor",
    inspectable(name = "Parkour Swing Anchor", icon = "ANCHOR", category = "physics")
)]
pub struct ParkourSwingAnchor {
    /// Rope length in metres. `0` uses however far away the character was when
    /// they grabbed it, which is what a vine or a long rope wants; a fixed
    /// length is what a trapeze bar wants.
    pub rope_length: f32,
    /// How far away it can be grabbed from, in metres.
    pub max_grab_distance: f32,
    /// Energy lost per second, as a fraction of speed. `0` swings forever.
    pub damping: f32,
}

impl Default for ParkourSwingAnchor {
    fn default() -> Self {
        Self {
            rope_length: 0.0,
            max_grab_distance: 6.0,
            damping: 0.35,
        }
    }
}

/// Opts a piece of geometry out of traversal.
///
/// Ledges are found by casting rays at the world, which is what makes the
/// system work with no authoring — and also what makes it occasionally right
/// about geometry a designer wanted left alone: the lip of a bottomless pit,
/// a decorative railing that should read as impassable, a collision proxy
/// standing in for something soft. Put this on it (or on an ancestor) and the
/// probe ignores every hit on it.
///
/// It carries no settings; the empty body is there because the Inspector's
/// "Add Component" list is generated from named fields, and a unit struct
/// cannot appear in it.
#[derive(Component, Reflect, Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "editor", derive(renzora_macros::Inspectable))]
#[reflect(Component, Default)]
#[cfg_attr(
    feature = "editor",
    inspectable(name = "Parkour Blocker", icon = "PROHIBIT", category = "physics")
)]
pub struct ParkourBlocker {}

/// The capsule the controller sweeps with, rebuilt whenever the controller's
/// dimensions change.
///
/// Deliberately *not* an avian `Collider` component: adding one of those would
/// enrol the character in the simulation a second time. This is a private
/// shape used only for queries, so it holds the collider by hand.
#[derive(Component)]
pub struct ParkourSweep(pub Collider);

/// Something the controller did. Observe it with
/// `app.add_observer(|t: On<ParkourEvent>| …)` for engine-side reactions
/// (footstep audio, camera shake); scripts read the same thing one frame at a
/// time as `ParkourReadState.event`.
#[derive(Event, Clone, Copy, Debug)]
pub struct ParkourEvent {
    pub entity: Entity,
    pub kind: ParkourEventKind,
}

/// The kinds of [`ParkourEvent`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParkourEventKind {
    Jump,
    Land,
    VaultStart,
    VaultEnd,
    MantleStart,
    MantleEnd,
    /// Caught a ledge.
    Grab,
    /// Let go of a ledge.
    Release,
    LadderMount,
    LadderDismount,
    WallRunStart,
    WallRunEnd,
    WallJump,
    SwingGrab,
    SwingRelease,
}

impl ParkourEventKind {
    /// The string form scripts see in `ParkourReadState.event`. These are API;
    /// renaming one breaks the scripts comparing against it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Jump => "jump",
            Self::Land => "land",
            Self::VaultStart => "vault_start",
            Self::VaultEnd => "vault_end",
            Self::MantleStart => "mantle_start",
            Self::MantleEnd => "mantle_end",
            Self::Grab => "grab",
            Self::Release => "release",
            Self::LadderMount => "ladder_mount",
            Self::LadderDismount => "ladder_dismount",
            Self::WallRunStart => "wall_run_start",
            Self::WallRunEnd => "wall_run_end",
            Self::WallJump => "wall_jump",
            Self::SwingGrab => "swing_grab",
            Self::SwingRelease => "swing_release",
        }
    }
}

/// Run condition: the simulation is live (Play or Simulate), or there is no
/// editor to ask. Deliberately not `!is_editing()` — that would keep the
/// controller running while the game is paused.
fn simulation_running(play_mode: Option<Res<PlayModeState>>) -> bool {
    play_mode.is_none_or(|pm| pm.is_scripts_running())
}

/// Give a new controller its runtime scratch, and keep the sweep capsule in
/// step with the authored dimensions.
#[allow(clippy::type_complexity)]
fn init_parkour(
    mut commands: Commands,
    fresh: Query<(Entity, &Transform), (With<ParkourController>, Without<ParkourMotion>)>,
    resized: Query<(Entity, &ParkourController), Changed<ParkourController>>,
) {
    for (entity, transform) in &fresh {
        commands.entity(entity).try_insert((
            ParkourMotion {
                // Start facing where the entity was placed, so the first turn
                // is from the authored pose rather than from due north.
                facing: drive::yaw_of(*transform.forward()),
                ..Default::default()
            },
            ParkourInput::default(),
        ));
    }
    // `Changed` fires on insert too, so this covers first-time setup as well
    // as an Inspector edit to radius/height.
    for (entity, controller) in &resized {
        let segment = (controller.height - controller.radius * 2.0).max(0.05);
        commands
            .entity(entity)
            .try_insert(ParkourSweep(Collider::capsule(controller.radius, segment)));
    }
}

/// Turn the `parkour_*` script actions into movement intent.
///
/// Same indirection every other domain crate uses: the script says
/// `parkour_jump()`, the backend turns it into a `renzora::ScriptAction`, and
/// this observer writes it onto the entity's [`ParkourInput`]. The drive
/// system is the only thing that reads it, so a blueprint node firing the same
/// action behaves identically to a Lua call.
fn handle_parkour_script_actions(
    trigger: On<renzora::ScriptAction>,
    mut inputs: Query<&mut ParkourInput>,
) {
    use renzora::ScriptActionValue;
    let action = trigger.event();
    let Ok(mut input) = inputs.get_mut(action.entity) else {
        return;
    };
    let number = |key: &str| -> f32 {
        match action.args.get(key) {
            Some(ScriptActionValue::Float(v)) => *v,
            Some(ScriptActionValue::Int(v)) => *v as f32,
            _ => 0.0,
        }
    };
    match action.name.as_str() {
        "parkour_move" => {
            input.move_dir = Vec3::new(number("x"), number("y"), number("z"));
        }
        "parkour_sprint" => {
            input.sprint = match action.args.get("on") {
                Some(ScriptActionValue::Bool(v)) => *v,
                // Tolerate `parkour_sprint(1)` from a language without a
                // distinct boolean.
                Some(ScriptActionValue::Float(v)) => *v != 0.0,
                Some(ScriptActionValue::Int(v)) => *v != 0,
                _ => true,
            };
        }
        "parkour_jump" => input.jump_pressed = true,
        "parkour_action" => input.action_pressed = true,
        "parkour_release" => input.release_pressed = true,
        _ => {}
    }
}

#[derive(Default)]
pub struct ParkourPlugin;

impl Plugin for ParkourPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] ParkourPlugin (traversal character controller)");

        app.register_type::<ParkourState>()
            .register_type::<ParkourReadState>();

        #[cfg(feature = "editor")]
        {
            app.register_inspectable::<ParkourController>();
            app.register_inspectable::<ParkourAnimations>();
            app.register_inspectable::<ParkourLadder>();
            app.register_inspectable::<ParkourSwingAnchor>();
            app.register_inspectable::<ParkourBlocker>();
        }
        #[cfg(not(feature = "editor"))]
        {
            app.register_type::<ParkourController>();
            app.register_type::<ParkourAnimations>();
            app.register_type::<ParkourLadder>();
            app.register_type::<ParkourSwingAnchor>();
            app.register_type::<ParkourBlocker>();
        }

        // Chained: the drive system queries the scratch components `init` adds
        // and the sweep capsule it rebuilds, and the animation driver reads the
        // state the drive system just settled on.
        app.add_systems(
            Update,
            (
                init_parkour,
                read_state::auto_init_read_state,
                drive::drive_parkour,
                anim::drive_parkour_animation,
            )
                .chain()
                .run_if(simulation_running),
        );

        app.add_observer(handle_parkour_script_actions);

        // Script functions owned by the parkour crate. Braced so the `cfg` has
        // a single item to sit on.
        #[cfg(feature = "scripting")]
        {
            let mut extensions = app.world_mut().get_resource_or_insert_with(
                renzora_scripting::extension::ScriptExtensions::default,
            );
            extensions.register(script_extension::ParkourScriptExtension);
        }
    }
}

renzora::add!(ParkourPlugin);
