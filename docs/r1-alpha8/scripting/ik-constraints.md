# IK, Constraints & Spring Bones

Procedural animation on top of your clips, provided by the `renzora_ik` plugin. You add components to bones (or to the character) in the Inspector, under the **Animation** page, and every frame they adjust the pose the animation player just wrote, before it reaches the skin, attached props and physics:

| Component | Put it on | What it does |
|---|---|---|
| `TwoBoneIk` | the end bone (hand, foot) | Reaches an arm or leg for a target, bending the elbow/knee toward a pole |
| `LookAt` | a head, eye, or turret bone | Aims one of the bone's axes at a target, within a cone |
| `FootIk` | the character's model root | Plants both feet on uneven ground, lowers the hips, tilts the feet onto slopes |
| `RotationLimit` | any bone | Clamps swing (a cone) and twist (a range) after animation and IK |
| `CopyRotation` / `CopyPosition` | any entity | Follows another entity's world rotation or position, per axis |
| `SpringBones` | the first bone of a chain | Simulates the chain as damped springs: hair, tails, ears, antennae |
| `SpringCollider` | any bone or entity | A sphere spring chains of the same character cannot pass through |

Constraints run in a fixed order each frame: copy constraints, foot IK, two-bone IK, look-at, rotation limits, spring bones. So limits clamp what the IK asked for, and spring chains hang off the finished pose.

> **Only in a running game.** Like animation, cloth and physics, constraints run in Play mode and in a shipped game, not while editing: what you place in the editor is what the scene file says. Leaving Play puts every bone the animation does not drive back where it was.

Every component has a **Weight** from `0` (the animation untouched) to `1` (fully applied). `0` switches a constraint off without removing it, and a script can ramp it (see [Scripting](#scripting)).

## Targets are names

Targets, poles, sources and foot bones are given by **entity name**, not picked: a name survives saving, reloading and re-importing the model. When several entities share a name (two copies of one character both have a `Head`), the lookup prefers the one under the same top-level parent as the constraint, so each character finds its own `LookTarget`. Foot and hip bones are only ever looked up inside the character that owns the `FootIk`.

An empty target name uses the component's **Target Position** (world space) instead, which is what scripts usually set.

## Two-bone IK

Add `TwoBoneIk` to the hand or foot. Its parent is taken as the elbow/knee and its grandparent as the shoulder/thigh, the way glTF skeletons are laid out. A target out of reach is reached for as far as the limb goes.

| Field | Default | Effect |
|---|---|---|
| Target | empty | Name of the entity to reach for |
| Target Position | `0, 0, 0` | World position used when Target is empty |
| Pole Target | empty | Name of an entity the elbow/knee points toward |
| Pole Direction | `0, 0, 0` | Direction the elbow/knee points, in the space of the shoulder/thigh's parent, so it turns with the character. Zero keeps the bend plane the animation has |
| Weight | `1` | Animation (0) to fully solved (1) |
| Keep End Rotation | on | The hand/foot keeps its animated world rotation while the limb bends |
| Match Target Rotation | off | The hand/foot takes the target entity's rotation instead (gripping a handle) |

If your rig has twist bones between the elbow and the wrist, the "parent is the mid joint" rule picks the twist bone; put the IK on a bone whose parent and grandparent are the real joints.

## Look-at / aim

| Field | Default | Effect |
|---|---|---|
| Target / Target Position | empty / origin | What to look at |
| Aim Axis | `0, 0, 1` | The bone-local axis that points at the target |
| Up Axis | `0, 1, 0` | The bone-local axis kept toward World Up |
| World Up | `0, 0, 0` | Zero keeps the animated roll (right for heads). Set `0, 1, 0` to keep a turret level |
| Max Angle | `70` | Furthest the aim turns away from where the animation points it, degrees. A target outside the cone is followed to its edge |
| Weight | `1` | |

The limit is measured from the *animated* direction, so a clip that already turns the head keeps its turn and the look-at adds at most Max Angle to it. For a natural head turn, put a `LookAt` on the neck at a low weight and another on the head.

## Foot IK

Put `FootIk` on the character's model root: the entity at its feet, whose height the walk cycle treats as the floor. Each frame a ray is cast down under each animated ankle, through the [physics query API](../extending/physics-backends.md), so it works with whichever physics engine the project uses. The character's own colliders (its ragdoll bodies, a controller capsule on a parent) are ignored. **With no physics engine installed**, the ground is a flat plane at **Fallback Ground Height**.

How far the ground under each foot sits above or below the character's floor is added to that ankle, so a lifted foot mid-stride stays lifted over the step it is crossing. The hips drop by the lower foot's amount (never rise), and each leg is solved with two-bone IK to reach.

| Field | Default | Effect |
|---|---|---|
| Left Foot / Right Foot | `LeftFoot` / `RightFoot` | Ankle bone names. Each one's parent is the knee and grandparent the thigh. Leave one empty for a single foot |
| Hips | `Hips` | The pelvis bone lowered to keep both feet reachable |
| Ray Height | `0.5` | How far above the ankle the ray starts: the tallest step a foot can find |
| Ray Depth | `0.6` | How far below the ankle it reaches |
| Fallback Ground Height | `0` | Ground height with no physics engine |
| Max Hip Drop | `0.4` | Furthest the hips go down, metres |
| Align To Ground | on | Tilt each foot onto the slope it stands on |
| Max Slope | `40` | Steepest slope a foot tilts onto, degrees |
| Smoothing | `15` | How quickly feet and hips follow the ground, per second. Lower is smoother on bumpy ground |
| Weight | `1` | Set it to `0` while the character is airborne |

## Rotation limits

`RotationLimit` clamps a bone's local rotation, measured from the rotation the bone had **when the component was added** (for a model straight out of its glTF, its rest pose):

| Field | Default | Effect |
|---|---|---|
| Twist Axis | `0, 1, 0` | The bone's own axis, local. glTF bones point down +Y |
| Swing Limit | `60` | Widest the bone may swing off its rest direction, degrees |
| Twist Min / Twist Max | `-30` / `30` | Twist range about the bone's axis, degrees |
| Weight | `1` | |

Swing and twist are clamped separately, so an over-twisted forearm keeps pointing where it pointed and only its roll is corrected.

## Copy constraints

`CopyRotation` and `CopyPosition` make an entity follow another one's world rotation or position. **X/Y/Z** pick which world axes are copied (Euler angles for rotation; with all three on the whole rotation is copied). **Offset** is added after copying (degrees for rotation, metres for position).

## Spring bones

Add `SpringBones` to the first bone of a chain. That bone, its first child, that child's first child and so on down to a leaf are simulated as damped springs pulled back toward the animated pose, so moving the character, or the clip, makes the chain lag, swing and settle. The last bone of the chain is its tip and only follows; if the final segment should swing too, give it an end bone (most exporters already do). A bone with several chains hanging off it, like a head of hair strands, needs one `SpringBones` on each strand.

| Field | Default | Effect |
|---|---|---|
| Stiffness | `150` | Pull back toward the animated shape. Higher is stiffer |
| Damping | `8` | Damping of wobble relative to the animation. Higher settles faster |
| Drag | `0.5` | Air drag on all motion, per second |
| Gravity | `0, -3, 0` | Acceleration on every joint, world space. A steady wind works too |
| Radius | `0.02` | Joint radius against Spring Colliders |
| Teleport Distance | `1` | A jump of the chain's root further than this in one frame (a respawn) snaps the chain to the pose instead of whipping it |
| Weight | `1` | `0` switches the simulation off; turning it back on starts from rest |

The simulation runs at a fixed 120 steps per second whatever the frame rate, so hair behaves the same at 30 fps and 144 fps. Only the bones' rotations are changed: a chain bends and swings but never stretches.

`SpringCollider` (Radius, Offset in the entity's local space) keeps spring chains of the **same character** (the same top-level parent) out of a sphere: put one on the head under the hair, on the hips under a tail.

## Scripting

Every function takes a **bone name** first, looked up under the script's own entity, so a script on the character reaches its own bones and never another character's bone of the same name. An empty name (`""`) means every constraint of that kind on the entity and below it.

| Function | Effect |
|---|---|
| `ik_set_target(bone, x, y, z)` | Reach a `TwoBoneIk` for a world position (clears its Target name) |
| `ik_set_target_entity(bone, name)` | Reach for the entity with this name |
| `ik_set_pole(bone, x, y, z)` | Set the Pole Direction (clears its Pole Target) |
| `ik_set_weight(bone, weight)` | Blend a `TwoBoneIk`, 0..1 |
| `look_at_set_target(bone, x, y, z)` | Aim a `LookAt` at a world position |
| `look_at_set_target_entity(bone, name)` | Aim at the entity with this name |
| `look_at_set_weight(bone, weight)` | Blend a `LookAt`, 0..1 |
| `foot_ik_set_weight(weight)` | Blend the `FootIk` on this entity (or below it) |
| `copy_constraint_set_weight(bone, weight)` | Blend a `CopyRotation`/`CopyPosition` |
| `rotation_limit_set_weight(bone, weight)` | Blend a `RotationLimit` |
| `spring_bones_set_weight(bone, weight)` | Blend a spring chain; `0` switches it off |
| `spring_bones_reset(bone)` | Snap a spring chain to the animated pose, at rest |

```lua
-- character.lua
function on_ready()
    -- Grip the door handle with the right hand, watch the player.
    ik_set_target_entity("RightHand", "DoorHandle")
    look_at_set_target_entity("Head", "Player")
end

function on_jump()
    foot_ik_set_weight(0.0) -- feet free in the air
end

function on_land()
    foot_ik_set_weight(1.0)
end
```

Read any field back through reflection as usual: `get("TwoBoneIk.weight")`.

## Related

- [Ragdoll Physics](ragdoll.md): full, partial and blended ragdolls
- [Animation](../editor/animation.md): clips and the animator
- [Building Plugins](../extending/plugins.md): how in-workspace plugins like this one are declared
