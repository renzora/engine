//! The constraint components a user adds in the Inspector.
//!
//! Every reference to another entity (a target, a pole, a foot bone) is a
//! **name**, not an `Entity`. An `Entity` id means nothing in a saved scene and
//! the Inspector has no entity picker, while a name survives a save, a reload
//! and a re-import of the model. Lookups prefer an entity under the same
//! top-level root as the constraint, so two copies of one character each find
//! their own `LookTarget` (see [`crate::names`]).
//!
//! All of them carry `#[reflect(Default)]`, so a scene saved before a field
//! existed fills the missing field from `Default` rather than failing to load.
//! A `weight` of `0` switches a constraint off without removing it.

use bevy::prelude::*;

/// Two-bone IK on an arm or a leg. Put it on the **end** bone (hand, foot):
/// its parent is the mid joint (elbow, knee) and its grandparent the root
/// (shoulder, thigh), the way a glTF skeleton is laid out.
#[derive(Component, Reflect, Clone, Debug)]
#[cfg_attr(feature = "editor", derive(renzora_macros::Inspectable))]
#[reflect(Component, Default)]
#[cfg_attr(feature = "editor", inspectable(name = "Two Bone IK", icon = "BONE", category = "animation"))]
pub struct TwoBoneIk {
    /// Name of the entity to reach for. Empty uses `target_position`.
    #[cfg_attr(feature = "editor", field(name = "Target"))]
    pub target: String,
    /// World position to reach for when `target` is empty.
    #[cfg_attr(feature = "editor", field(name = "Target Position"))]
    pub target_position: Vec3,
    /// Name of an entity the mid joint (elbow, knee) should point toward.
    /// Empty uses `pole_direction`.
    #[cfg_attr(feature = "editor", field(name = "Pole Target"))]
    pub pole_target: String,
    /// Direction the mid joint points, in the space of the root bone's parent
    /// (so it turns with the character). Zero keeps whatever plane the
    /// animation bends the limb in.
    #[cfg_attr(feature = "editor", field(name = "Pole Direction"))]
    pub pole_direction: Vec3,
    /// 0 is the animation untouched, 1 fully solved.
    #[cfg_attr(feature = "editor", field(name = "Weight", speed = 0.01, min = 0.0, max = 1.0))]
    pub weight: f32,
    /// Keep the end bone's animated world rotation, so a planted hand or foot
    /// does not swivel as the limb bends to reach.
    #[cfg_attr(feature = "editor", field(name = "Keep End Rotation"))]
    pub keep_end_rotation: bool,
    /// Take the end bone's world rotation from the target entity instead (a
    /// hand gripping a handle). Overrides `keep_end_rotation`.
    #[cfg_attr(feature = "editor", field(name = "Match Target Rotation"))]
    pub match_target_rotation: bool,
}

impl Default for TwoBoneIk {
    fn default() -> Self {
        Self {
            target: String::new(),
            target_position: Vec3::ZERO,
            pole_target: String::new(),
            pole_direction: Vec3::ZERO,
            weight: 1.0,
            keep_end_rotation: true,
            match_target_rotation: false,
        }
    }
}

/// Aim one of a bone's axes at a target: heads, eyes, turrets.
#[derive(Component, Reflect, Clone, Debug)]
#[cfg_attr(feature = "editor", derive(renzora_macros::Inspectable))]
#[reflect(Component, Default)]
#[cfg_attr(feature = "editor", inspectable(name = "Look At", icon = "EYE", category = "animation"))]
pub struct LookAt {
    /// Name of the entity to look at. Empty uses `target_position`.
    #[cfg_attr(feature = "editor", field(name = "Target"))]
    pub target: String,
    /// World position to look at when `target` is empty.
    #[cfg_attr(feature = "editor", field(name = "Target Position"))]
    pub target_position: Vec3,
    /// The bone-local axis that should point at the target.
    #[cfg_attr(feature = "editor", field(name = "Aim Axis"))]
    pub aim_axis: Vec3,
    /// The bone-local axis kept toward `world_up`.
    #[cfg_attr(feature = "editor", field(name = "Up Axis"))]
    pub up_axis: Vec3,
    /// World direction `up_axis` leans toward. Zero keeps the animated roll,
    /// the natural choice for a head; set it to keep a turret level.
    #[cfg_attr(feature = "editor", field(name = "World Up"))]
    pub world_up: Vec3,
    /// Furthest the aim may turn from where the animation points it, degrees.
    #[cfg_attr(feature = "editor", field(name = "Max Angle", speed = 1.0, min = 0.0, max = 180.0))]
    pub max_angle: f32,
    /// 0 is the animation untouched, 1 fully aimed.
    #[cfg_attr(feature = "editor", field(name = "Weight", speed = 0.01, min = 0.0, max = 1.0))]
    pub weight: f32,
}

impl Default for LookAt {
    fn default() -> Self {
        Self {
            target: String::new(),
            target_position: Vec3::ZERO,
            aim_axis: Vec3::Z,
            up_axis: Vec3::Y,
            world_up: Vec3::ZERO,
            max_angle: 70.0,
            weight: 1.0,
        }
    }
}

/// Plant both feet on uneven ground and lower the hips so they can reach.
/// Put it on the character's model root (the entity at the feet, whose height
/// the animation treats as the floor).
#[derive(Component, Reflect, Clone, Debug)]
#[cfg_attr(feature = "editor", derive(renzora_macros::Inspectable))]
#[reflect(Component, Default)]
#[cfg_attr(feature = "editor", inspectable(name = "Foot IK", icon = "FOOTPRINTS", category = "animation"))]
pub struct FootIk {
    /// Name of the left ankle/foot bone. Its parent is the knee, its
    /// grandparent the thigh.
    #[cfg_attr(feature = "editor", field(name = "Left Foot"))]
    pub left_foot: String,
    /// Name of the right ankle/foot bone.
    #[cfg_attr(feature = "editor", field(name = "Right Foot"))]
    pub right_foot: String,
    /// Name of the hips/pelvis bone that is lowered.
    #[cfg_attr(feature = "editor", field(name = "Hips"))]
    pub hips: String,
    /// How far above the animated ankle each ray starts, metres: the tallest
    /// step a foot can find.
    #[cfg_attr(feature = "editor", field(name = "Ray Height", speed = 0.01, min = 0.0, max = 5.0))]
    pub ray_height: f32,
    /// How far below the animated ankle each ray reaches, metres.
    #[cfg_attr(feature = "editor", field(name = "Ray Depth", speed = 0.01, min = 0.0, max = 5.0))]
    pub ray_depth: f32,
    /// Ground height used when no physics backend is installed.
    #[cfg_attr(feature = "editor", field(name = "Fallback Ground Height", speed = 0.01))]
    pub ground_height: f32,
    /// Furthest the hips may drop, metres.
    #[cfg_attr(feature = "editor", field(name = "Max Hip Drop", speed = 0.01, min = 0.0, max = 2.0))]
    pub max_hip_drop: f32,
    /// Tilt each foot onto the ground's slope.
    #[cfg_attr(feature = "editor", field(name = "Align To Ground"))]
    pub align_to_ground: bool,
    /// Steepest slope a foot tilts onto, degrees.
    #[cfg_attr(feature = "editor", field(name = "Max Slope", speed = 1.0, min = 0.0, max = 89.0))]
    pub max_slope: f32,
    /// How quickly the feet and hips follow the ground, per second. Lower is
    /// smoother over bumpy ground, higher sticks closer.
    #[cfg_attr(feature = "editor", field(name = "Smoothing", speed = 0.5, min = 0.0, max = 100.0))]
    pub smoothing: f32,
    /// 0 is the animation untouched, 1 fully planted.
    #[cfg_attr(feature = "editor", field(name = "Weight", speed = 0.01, min = 0.0, max = 1.0))]
    pub weight: f32,
}

impl Default for FootIk {
    fn default() -> Self {
        Self {
            left_foot: "LeftFoot".into(),
            right_foot: "RightFoot".into(),
            hips: "Hips".into(),
            ray_height: 0.5,
            ray_depth: 0.6,
            ground_height: 0.0,
            max_hip_drop: 0.4,
            align_to_ground: true,
            max_slope: 40.0,
            smoothing: 15.0,
            weight: 1.0,
        }
    }
}

/// Swing/twist limits on one bone, applied after animation and IK.
///
/// The limits are measured from the bone's rotation when the component was
/// added (for a model straight out of its glTF, the rest pose).
#[derive(Component, Reflect, Clone, Debug)]
#[cfg_attr(feature = "editor", derive(renzora_macros::Inspectable))]
#[reflect(Component, Default)]
#[cfg_attr(feature = "editor", inspectable(name = "Rotation Limit", icon = "LOCK", category = "animation"))]
pub struct RotationLimit {
    /// The bone's own axis, local. glTF bones point down +Y.
    #[cfg_attr(feature = "editor", field(name = "Twist Axis"))]
    pub twist_axis: Vec3,
    /// Widest the bone may swing off its rest direction, degrees.
    #[cfg_attr(feature = "editor", field(name = "Swing Limit", speed = 1.0, min = 0.0, max = 180.0))]
    pub swing_limit: f32,
    /// Twist range about the bone's axis, degrees.
    #[cfg_attr(feature = "editor", field(name = "Twist Min", speed = 1.0, min = -180.0, max = 180.0))]
    pub twist_min: f32,
    #[cfg_attr(feature = "editor", field(name = "Twist Max", speed = 1.0, min = -180.0, max = 180.0))]
    pub twist_max: f32,
    /// 0 ignores the limits, 1 enforces them fully.
    #[cfg_attr(feature = "editor", field(name = "Weight", speed = 0.01, min = 0.0, max = 1.0))]
    pub weight: f32,
}

impl Default for RotationLimit {
    fn default() -> Self {
        Self { twist_axis: Vec3::Y, swing_limit: 60.0, twist_min: -30.0, twist_max: 30.0, weight: 1.0 }
    }
}

/// Copy another entity's world rotation onto this one.
#[derive(Component, Reflect, Clone, Debug)]
#[cfg_attr(feature = "editor", derive(renzora_macros::Inspectable))]
#[reflect(Component, Default)]
#[cfg_attr(feature = "editor", inspectable(name = "Copy Rotation", icon = "COPY", category = "animation"))]
pub struct CopyRotation {
    /// Name of the entity to copy from.
    #[cfg_attr(feature = "editor", field(name = "Source"))]
    pub source: String,
    /// Copy the world X/Y/Z Euler angle. All three copies the whole rotation.
    #[cfg_attr(feature = "editor", field(name = "X"))]
    pub x: bool,
    #[cfg_attr(feature = "editor", field(name = "Y"))]
    pub y: bool,
    #[cfg_attr(feature = "editor", field(name = "Z"))]
    pub z: bool,
    /// Extra rotation after copying, Euler XYZ degrees, world.
    #[cfg_attr(feature = "editor", field(name = "Offset", speed = 1.0))]
    pub offset: Vec3,
    #[cfg_attr(feature = "editor", field(name = "Weight", speed = 0.01, min = 0.0, max = 1.0))]
    pub weight: f32,
}

impl Default for CopyRotation {
    fn default() -> Self {
        Self { source: String::new(), x: true, y: true, z: true, offset: Vec3::ZERO, weight: 1.0 }
    }
}

/// Copy another entity's world position onto this one.
#[derive(Component, Reflect, Clone, Debug)]
#[cfg_attr(feature = "editor", derive(renzora_macros::Inspectable))]
#[reflect(Component, Default)]
#[cfg_attr(feature = "editor", inspectable(name = "Copy Position", icon = "COPY", category = "animation"))]
pub struct CopyPosition {
    /// Name of the entity to copy from.
    #[cfg_attr(feature = "editor", field(name = "Source"))]
    pub source: String,
    /// Copy the world X/Y/Z coordinate.
    #[cfg_attr(feature = "editor", field(name = "X"))]
    pub x: bool,
    #[cfg_attr(feature = "editor", field(name = "Y"))]
    pub y: bool,
    #[cfg_attr(feature = "editor", field(name = "Z"))]
    pub z: bool,
    /// Added to the copied position, world.
    #[cfg_attr(feature = "editor", field(name = "Offset"))]
    pub offset: Vec3,
    #[cfg_attr(feature = "editor", field(name = "Weight", speed = 0.01, min = 0.0, max = 1.0))]
    pub weight: f32,
}

impl Default for CopyPosition {
    fn default() -> Self {
        Self { source: String::new(), x: true, y: true, z: true, offset: Vec3::ZERO, weight: 1.0 }
    }
}

/// Spring (jiggle) bones. Put it on the first bone of a chain: it and its
/// first child, that child's first child and so on to a leaf are simulated as
/// damped springs toward the animated pose. For a bone with several chains
/// hanging off it (a head of hair strands), add one to each strand's root.
#[derive(Component, Reflect, Clone, Debug)]
#[cfg_attr(feature = "editor", derive(renzora_macros::Inspectable))]
#[reflect(Component, Default)]
#[cfg_attr(feature = "editor", inspectable(name = "Spring Bones", icon = "WAVE_SINE", category = "animation"))]
pub struct SpringBones {
    /// Pull back toward the animated shape. Higher is stiffer.
    #[cfg_attr(feature = "editor", field(name = "Stiffness", speed = 1.0, min = 0.0, max = 5000.0))]
    pub stiffness: f32,
    /// Damping of wobble relative to the animation. Higher settles faster.
    #[cfg_attr(feature = "editor", field(name = "Damping", speed = 0.1, min = 0.0, max = 200.0))]
    pub damping: f32,
    /// Air drag, per second.
    #[cfg_attr(feature = "editor", field(name = "Drag", speed = 0.05, min = 0.0, max = 50.0))]
    pub drag: f32,
    /// Acceleration on every joint, world (gravity, or a steady wind).
    #[cfg_attr(feature = "editor", field(name = "Gravity"))]
    pub gravity: Vec3,
    /// Joint radius against `SpringCollider`s, metres.
    #[cfg_attr(feature = "editor", field(name = "Radius", speed = 0.005, min = 0.0, max = 1.0))]
    pub radius: f32,
    /// A jump of the chain's root further than this in one frame (a
    /// teleport, a respawn) snaps the chain to the pose instead of whipping.
    #[cfg_attr(feature = "editor", field(name = "Teleport Distance", speed = 0.05, min = 0.0, max = 100.0))]
    pub teleport_distance: f32,
    /// 0 is the animation untouched, 1 fully simulated.
    #[cfg_attr(feature = "editor", field(name = "Weight", speed = 0.01, min = 0.0, max = 1.0))]
    pub weight: f32,
}

impl Default for SpringBones {
    fn default() -> Self {
        Self {
            stiffness: 150.0,
            damping: 8.0,
            drag: 0.5,
            gravity: Vec3::new(0.0, -3.0, 0.0),
            radius: 0.02,
            teleport_distance: 1.0,
            weight: 1.0,
        }
    }
}

/// A sphere spring bones cannot pass through (a head under hair, a body under
/// a tail). It affects the spring chains of the same character: those under
/// the same top-level root.
#[derive(Component, Reflect, Clone, Debug)]
#[cfg_attr(feature = "editor", derive(renzora_macros::Inspectable))]
#[reflect(Component, Default)]
#[cfg_attr(feature = "editor", inspectable(name = "Spring Collider", icon = "CIRCLE", category = "animation"))]
pub struct SpringCollider {
    /// Metres. Scaled by the entity's own world scale.
    #[cfg_attr(feature = "editor", field(name = "Radius", speed = 0.005, min = 0.0, max = 10.0))]
    pub radius: f32,
    /// Centre, in the entity's local space.
    #[cfg_attr(feature = "editor", field(name = "Offset"))]
    pub offset: Vec3,
}

impl Default for SpringCollider {
    fn default() -> Self {
        Self { radius: 0.1, offset: Vec3::ZERO }
    }
}
