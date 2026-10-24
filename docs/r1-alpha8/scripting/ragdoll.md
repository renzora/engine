# Ragdoll Physics

Skeletal ragdoll physics, provided by the `renzora_ragdoll` plugin. It auto-builds a rigid body per bone from your skinned mesh's skeleton, plus a joint between every parent/child bone, and lets scripts toggle the whole skeleton between animated and physically-simulated with a single call.

## Physics engine

`renzora_ragdoll` is an in-workspace plugin compiled into the engine. Its bones and joints are ordinary [physics API](/docs/r1-alpha8/scripting/physics) data (`PhysicsBodyData`, `CollisionShapeData` and spherical `PhysicsJoint`s), so a ragdoll runs on whichever engine simulates the project's 3D world: the bundled avian, or Rapier from its plugin. It is optional in the feature graph, so the lean exporter leaves it out of a game with no ragdolls in it.

The bones' generated physics data is never saved with the scene: the ragdoll builds it again on load.

## Setting up a ragdoll

Add a `Ragdoll` component to the **skeleton root** — typically the same entity that has `AnimatorComponent` (an ancestor of the model's skinned mesh):

1. The plugin finds the nearest `SkinnedMesh` in the entity's descendants and reads its joint list — that is the bone set, not just any named child entity. Models load asynchronously, so the plugin keeps polling for a few seconds until the skeleton has instantiated rather than giving up if it isn't there on the first frame.
2. For every bone it inserts a kinematic body plus a capsule collider spanning the bone to its (averaged) child position, or a small sphere for leaf bones (hands, feet, head tip, …). Each bone's mass comes from its collider's volume.
3. For every parent/child bone pair it spawns a spherical `PhysicsJoint`, anchored at the child's bind-pose offset, so the skeleton doesn't pop when the ragdoll activates.

Bones start `Kinematic`: the `AnimationPlayer` drives them exactly as if the plugin weren't present, while the colliders still participate in physics (so a ragdoll-capable character can still register hits before it ever ragdolls).

> **Ragdoll only simulates in a running game.** It is a physics feature, and the editor pauses the simulation while editing. Toggling `Ragdoll.active` (or calling `enable_ragdoll()`) in the editor just freezes the pose. Press **Play** to watch the skeleton fall.

## Tuning

`Ragdoll` has fields beyond `active` for how the simulated skeleton feels. They're read once, when the bone bodies and joints are first generated (the moment the skeleton finishes loading) — editing them on an already-built ragdoll has no effect. To re-tune, edit the values *before* the skeleton finishes generating (e.g. before the first Play), or remove and re-add the `Ragdoll` component to force regeneration.

| Field | Default | Effect |
|---|---|---|
| `stiffness` | `0.85` | `0` (loose/floppy) to `1` (rigid). Maps to the joints' compliance, which resists bending *within* the limits below: this, not the limits alone, is what makes a joint hold its bend instead of flopping freely. On Rapier, whose joints soften by springs rather than compliance, the joints are rigid whatever this says. |
| `swing_limit_degrees` | `60.0` | Max degrees a joint may swing off its bone's rest axis (an elbow/knee opening, a shoulder lifting). `0` locks the joint straight. |
| `twist_limit_degrees` | `30.0` | Max degrees a joint may twist about its bone's rest axis. `0` locks out twisting entirely. |
| `linear_damping` | `2.0` | Drag on each bone's linear velocity. The main knob against limbs whipping around too fast. |
| `angular_damping` | `4.0` | Drag on each bone's angular velocity — resists spinning limbs independently of `linear_damping`. |
| `gravity_scale` | `0.6` | Per-bone gravity multiplier. Lower falls and settles more slowly — the direct knob against a ragdoll that drops too fast. |

Joint translation (the point itself, not its bend) always stays rigid regardless of `stiffness` — limbs shouldn't visibly pull apart at the socket.

## Partial and blended ragdolls

Three more fields control *how much* of the character ragdolls. Unlike the tuning fields above, they are read live, so a script can change them at any time.

| Field | Default | Effect |
|---|---|---|
| `bone` (Partial Bone) | empty | Name of the bone whose subtree ragdolls: an arm hit by a shot is its upper-arm bone. Empty ragdolls the whole skeleton. The rest of the skeleton keeps animating, and the ragdolled part hangs off it by its joint. A name that matches no bone ragdolls nothing (with a warning) rather than everything. |
| `blend` (Physics Blend) | `1.0` | How much of the pose the physics owns while active. `1` is a full ragdoll; lower mixes the simulated bones with the still-playing animation (a stagger, a flinch, a character bracing against a hit). |
| `get_up_time` | `0.5` | Seconds to blend back to the animation when the ragdoll switches off, from wherever the body fell to. `0` snaps. |

A full ragdoll (`blend` of `1`, no `bone`) behaves as before: the bones are detached from the `AnimationPlayer` and the animator is paused. A partial or blended one keeps the animation playing, because the blend and the bones that are not ragdolling both need it.

> The get-up blend eases the bones back into the animation where the character *is*: it does not move the character to where its body fell. To have a character stand up at the spot it collapsed, move its root there (and play a get-up clip) as you disable the ragdoll.

## Scripting

| Function | Effect |
|---|---|
| `enable_ragdoll()` | Flips every bone of the script's entity to a dynamic body, detaches the bones from the `AnimationPlayer`, and pauses the animator: the physics solver and joints take over. Always the whole skeleton at full blend, whatever `bone`/`blend` held before. |
| `disable_ragdoll()` | Reconnects the bones to the `AnimationPlayer`, flips them back to kinematic, resumes the animator, and blends back to the animation over `get_up_time`. |
| `enable_partial_ragdoll(bone, blend)` | Ragdoll only `bone`'s subtree (`""` for all of it), mixed with the animation by `blend` (0..1). |
| `set_ragdoll_blend(blend)` | Change how much of the pose the physics owns while ragdolling. |
| `set_ragdoll_get_up_time(seconds)` | Change how long the blend back to the animation takes. |

```lua
-- hit reaction: the shot arm goes limp for a moment, the rest keeps running
function on_hit(bone)
    enable_partial_ragdoll("LeftArm", 0.8)
end

function on_recover()
    disable_ragdoll()
end
```

```lua
-- character.lua
function on_death()
    enable_ragdoll()
end

function on_respawn()
    disable_ragdoll()
end
```

> The ragdoll functions target the **script's own entity**: call them from a script attached to the same entity the `Ragdoll` component is on. `blend` mixes two finished poses; there is no powered "active ragdoll" (joint motors driving the bodies toward the animated pose) yet.

Read the current state from the `Ragdoll` component's `active` field via the reflect path dispatcher, same as any other component:

```lua
if get("Ragdoll.active") then
    -- currently ragdolling
end
```

## Limitations (v1)

- Auto-generated only — there is no manual bone-to-collider mapping UI or API yet.
- Tuning is whole-ragdoll, not per-bone: one `stiffness`/limits/damping/gravity setting applies to every bone and joint a `Ragdoll` generates, not e.g. stiffer arms than legs.
- One joint type (spherical, ball-and-socket) for every bone pair.

## Related

- [Physics](/docs/r1-alpha8/scripting/physics): rigid bodies, colliders and joints
- [IK, Constraints & Spring Bones](/docs/r1-alpha8/scripting/ik-constraints): procedural animation on top of clips
- [Building Plugins](/docs/r1-alpha8/extending/plugins) — how in-workspace plugins like this one are declared and wired in
