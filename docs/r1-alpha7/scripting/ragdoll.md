# Ragdoll Physics

Skeletal ragdoll physics, provided by the `renzora_ragdoll` distribution plugin. It auto-builds an Avian rigid body per bone from your skinned mesh's skeleton, plus a joint between every parent/child bone, and lets scripts toggle the whole skeleton between animated and physically-simulated with a single call.

## Backend

`renzora_ragdoll` ships as a `cdylib` distribution plugin (same model as cloth/lumen) and wraps the same **Avian 3D** backend `renzora_physics` uses. It is not built into the engine by default — drop the built artifact into `<exe>/plugins/` to enable it (see [Building Plugins](/docs/r1-alpha7/extending/plugins)).

## Setting up a ragdoll

Add a `Ragdoll` component to the **skeleton root** — typically the same entity that has `AnimatorComponent` (an ancestor of the model's skinned mesh):

1. The plugin finds the nearest `SkinnedMesh` in the entity's descendants and reads its joint list — that is the bone set, not just any named child entity. Models load asynchronously, so the plugin keeps polling for a few seconds until the skeleton has instantiated rather than giving up if it isn't there on the first frame.
2. For every bone it inserts a `RigidBody::Kinematic` plus a capsule collider spanning the bone to its (averaged) child position — or a small sphere for leaf bones (hands, feet, head tip, …).
3. For every parent/child bone pair it spawns a `SphericalJoint`, anchored at the child's bind-pose offset, so the skeleton doesn't pop when the ragdoll activates.

Bones start `Kinematic`: the `AnimationPlayer` drives them exactly as if the plugin weren't present, while the colliders still participate in physics (so a ragdoll-capable character can still register hits before it ever ragdolls).

> **Ragdoll only simulates in Play or Simulate mode.** It is a physics feature, and the editor pauses the simulation while editing. Toggling `Ragdoll.active` (or calling `enable_ragdoll()`) in edit mode just freezes the pose. Use **[Simulate](/docs/r1-alpha7/editor/viewport#simulate-mode)** (the blue flask, beside Play) to watch the skeleton fall while keeping the editor live — Stop restores the scene afterward.

## Tuning

`Ragdoll` has fields beyond `active` for how the simulated skeleton feels. They're read once, when the bone bodies and joints are first generated (the moment the skeleton finishes loading) — editing them on an already-built ragdoll has no effect. To re-tune, edit the values *before* the skeleton finishes generating (e.g. before the first Play), or remove and re-add the `Ragdoll` component to force regeneration.

| Field | Default | Effect |
|---|---|---|
| `stiffness` | `0.85` | `0` (loose/floppy) to `1` (rigid). Maps to the swing/twist joint compliance Avian uses to resist bending *within* the limits below — this, not the limits alone, is what makes a joint hold its bend instead of flopping freely. |
| `swing_limit_degrees` | `60.0` | Max degrees a joint may swing off its bone's rest axis (an elbow/knee opening, a shoulder lifting). `0` locks the joint straight. |
| `twist_limit_degrees` | `30.0` | Max degrees a joint may twist about its bone's rest axis. `0` locks out twisting entirely. |
| `linear_damping` | `2.0` | Drag on each bone's linear velocity. The main knob against limbs whipping around too fast. |
| `angular_damping` | `4.0` | Drag on each bone's angular velocity — resists spinning limbs independently of `linear_damping`. |
| `gravity_scale` | `0.6` | Per-bone gravity multiplier. Lower falls and settles more slowly — the direct knob against a ragdoll that drops too fast. |

Joint translation (the point itself, not its bend) always stays rigid regardless of `stiffness` — limbs shouldn't visibly pull apart at the socket.

## Scripting

| Function | Lua | Rhai | Effect |
|---|---|---|---|
| `enable_ragdoll()` | yes | — | Flips every bone of the script's entity to `RigidBody::Dynamic`, detaches the bones from the `AnimationPlayer`, and pauses the animator — the avian solver + joints take over. |
| `disable_ragdoll()` | yes | — | Reconnects the bones to the `AnimationPlayer`, flips them back to `Kinematic`, and resumes the animator. |

```lua
-- character.lua
function on_death()
    enable_ragdoll()
end

function on_respawn()
    disable_ragdoll()
end
```

> `enable_ragdoll`/`disable_ragdoll` target the **script's own entity** — call them from a script attached to the same entity the `Ragdoll` component is on. Activation is a simple on/off switch for v1: there is no blended "active ragdoll" (motors driving joints toward an animated pose) yet.

Read the current state from the `Ragdoll` component's `active` field via the reflect path dispatcher, same as any other component:

```lua
if get("Ragdoll.active") then
    -- currently ragdolling
end
```

## Limitations (v1)

- Auto-generated only — there is no manual bone-to-collider mapping UI or API yet.
- Tuning is whole-ragdoll, not per-bone: one `stiffness`/limits/damping/gravity setting applies to every bone and joint a `Ragdoll` generates, not e.g. stiffer arms than legs.
- One joint type (`SphericalJoint`, ball-and-socket) for every bone pair.

## Related

- [Physics](/docs/r1-alpha7/scripting/physics) — the underlying Avian backend, rigid bodies, colliders
- [Building Plugins](/docs/r1-alpha7/extending/plugins) — how distribution plugins like this one are built and loaded
