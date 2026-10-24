# Animator: Blending, Layers and Root Motion

The [Animation](/docs/r1-alpha8/editor/animation) page covers clips, the timeline and keying. This page covers what the **Animator** does with clips at runtime: crossfades, blend trees, layers, root motion, retargeting and event markers. Everything here runs in play mode and in an exported game; while editing, the timeline preview owns the pose.

An animator plays a **base motion** (one clip, a crossfade between clips, or a blend tree), and any number of **layers** stack on top of it in order.

## State machines

A `.animsm` file holds states, transitions and an entry state. Each state plays either a clip or a blend tree. The entry state starts playing when the game starts; a transition crossfades into its target over its blend time.

In the **State Machine** panel (the Logic mode stage), each state has a motion combo:

| Motion | What the state plays |
|---|---|
| **Clip** | One clip slot, picked from the clip combo beside it |
| **1D blend** | Clips along one float parameter (see below) |
| **2D blend** | Clips placed on a plane of two float parameters |

Switching a clip state to a blend keeps its clip as the first entry. A blend state shows its parameter name(s) and one row per entry, each with a clip and its position, and **Add entry** below them. Save writes the file.

An animator with a state machine leaves the base to it, so the **Default Clip** does not also auto-play.

While editing, the machine still follows its parameters (the **Parameters** panel shows the state change and the timeline shows the new state's clip), but it only drives the character in play mode.

## Blend trees

A blend tree mixes clips by parameters you set from a script (`set_anim_param("speed", v)`).

### 1D blend

Entries sit at **thresholds** along one parameter, in the parameter's own units, so you can feed raw speed in metres a second. Between two thresholds their clips share the weight; past either end the end clip plays alone.

A locomotion state:

```ron
(
    name: "move",
    motion: Blend(Blend1D(
        param: "speed",
        entries: [
            (clip: "idle", threshold: 0.0),
            (clip: "walk", threshold: 1.5),
            (clip: "run", threshold: 4.0),
        ],
    )),
    speed: 1.0,
    looping: true,
)
```

### 2D blend

Entries sit at points on a plane of two parameters, typically `move_x` and `move_y` for strafing locomotion. Weights use gradient band interpolation: standing on an entry plays that clip alone, and an entry's weight falls to zero at its neighbours. Put an idle at the origin and walks or runs around it.

```ron
motion: Blend(BlendSpace2D(
    param_x: "move_x",
    param_y: "move_y",
    entries: [
        (clip: "idle", x: 0.0, y: 0.0),
        (clip: "walk_fwd", x: 0.0, y: 1.0),
        (clip: "walk_back", x: 0.0, y: -1.0),
        (clip: "strafe_left", x: -1.0, y: 0.0),
        (clip: "strafe_right", x: 1.0, y: 0.0),
    ],
)),
```

### Trees in the file

Two more node kinds can be written by hand and nest anywhere: `Lerp(a: .., b: .., param: "t")` blends two subtrees by a 0 to 1 parameter, and `Additive(base: .., overlay: .., param: "amount")` adds an overlay's motion on top of its base (a breathing or recoil clip over any locomotion). The panel shows such a state as a nested tree without editing it.

### How a tree plays

- Clips of different lengths are kept in step: each runs at the speed that makes it as long as the weighted mix, so a walk and a run blended together plant their feet together. The state's **speed** scales the whole tree.
- Only the most heavily weighted clip of a tree fires its [markers](#event-markers), so a walk and a run do not each play a footstep.
- `AnimatorReadState.current_clip` reports that heaviest clip.

## Layers

A layer plays its own clip on top of everything below it. Layers are set on the Animator component:

| Field | What it does |
|---|---|
| Name | How scripts refer to it |
| Weight | 0 to 1: how much of the layer shows |
| Mask | The bones it may move (below); none means every bone |
| Blend Mode | **Override** replaces the pose below, **Additive** adds to it |
| Current Clip | The slot it plays |

Changing **Current Clip** crossfades the layer to the new clip over the animator's **Blend Duration**; onto an empty layer the clip arrives at once, and the layer's weight is the fade.

From a script:

```lua
play_layer_animation("upper_body", "wave")
set_layer_weight("upper_body", 0.8)
stop_layer_animation("upper_body")
```

or from Rust, `AnimationCommand::PlayOnLayer { entity, layer_name, name }` and `AnimationCommand::StopLayer { entity, layer_name }`.

### Masks

A mask is a list of bone names, each standing for that bone **and everything under it**. `mixamorig:Spine1` takes the whole upper body. An entry starting with `!` carves a subtree back out, and the deepest entry on a bone's path wins:

```ron
mask: Some(["mixamorig:Spine1", "!mixamorig:LeftShoulder"]),
```

is the upper body except the left arm. Names match either spelling (`mixamorig:Spine1` and `mixamorig_spine1`). Masks use Bevy's animation mask groups, so only the first 64 layers can be masked.

### Additive layers

An additive layer plays its clip as a difference from the clip's own first frame, added onto the pose below. Author an additive clip starting from the pose it was keyed against: a head turn that starts looking straight ahead adds just the turn. Additive only adds to bones something below animates.

## Root motion

A walk clip moves the hips forward. Played as is, the mesh walks away from its entity and snaps back on every loop. With root motion on, the root bone's horizontal movement is taken off the bone and given to the entity, so it travels as far as its feet say.

Set it on the Animator's **Root Motion**:

| Field | What it does |
|---|---|
| Mode | **Off**; **Transform** moves the entity's `Transform`; **Extract** leaves the `Transform` and publishes the movement for your own controller |
| Bone | The root bone. Empty finds the first bone named like hips or a pelvis |
| Yaw | Also carry the root's turn about the up axis over to the entity |

- The bone keeps its vertical movement (jumps, crouches) and is held at its rest position horizontally, so the mesh stays centred on the entity.
- Loops carry on seamlessly: the motion is read from the clips, which know the end of one loop joins the start of the next.
- During a crossfade or a blend tree the entity moves by the weighted mix of every playing clip's motion.
- In **Extract** mode read `AnimatorReadState.root_motion_delta` (the last frame's movement, in the entity's parent space) and `root_motion_yaw` (radians), and apply them to a character controller or physics body yourself.

## Retargeting

Clips bind to bones by name, so a clip plays on any rig that names its bones the same way. That is enough when the rigs also *rest* the same way. A clip from another rig, with other bone orientations (an A-pose against a T-pose, another axis convention) or other proportions, needs the rest pose of the skeleton it was authored on.

That rest pose is a `.skel` file. The importer writes one for every skinned model it extracts clips from (`animations/<model>.skel`), and clip discovery points the slots of clips from another model at it (see [Importing animations](/docs/r1-alpha8/editor/animation#importing-animations)). Point a clip slot's **Source Skeleton** at it and the clip is retargeted onto the animator's skeleton when the animator loads:

- every bone's rotation is carried over as a change from its rest pose, measured in the space of the hips' parent, so a raised arm stays a raised arm whatever the two rigs' bone axes;
- the hips' translation is scaled by the ratio of the two rigs' leg lengths (upper leg to foot, found by name; failing that, the hips' rest heights), so a short character does not slide;
- other bones keep the target's own bone lengths.

Bones either skeleton lacks play as before, by name, so a partly matching rig still plays what matches. A slot without a source skeleton plays exactly as it always did.

For rigs whose bones are named differently, fill the Animator's **Bone Map** with source name to target name pairs (`mixamorig:Hips` to `pelvis`). The map applies with or without a source skeleton.

### The `.skel` format

```ron
(
    bones: [
        (name: "mixamorig:Hips", parent: None, translation: (0.0, 1.0, 0.0), rotation: (0.0, 0.0, 0.0, 1.0), scale: (1.0, 1.0, 1.0)),
        (name: "mixamorig:Spine", parent: Some("mixamorig:Hips"), translation: (0.0, 0.1, 0.0), rotation: (0.0, 0.0, 0.0, 1.0)),
    ],
    clips: ["Walking.anim"],
)
```

Each bone's transform is its **local** rest transform, relative to its parent; `scale` may be left out. `clips` (optional) names the `.anim` files beside it that were keyed on this skeleton; discovery reads it to fill in slots' source skeletons. An imported `.skel` holds every joint of the model's skin and every node above them, so an armature node above the hips is part of the pose. Blend shape tracks are not changed by retargeting: only their mesh name goes through the **Bone Map**. To write one from a model that is in its rest pose (just spawned), call `renzora_animation::capture_rest_pose(world, model_entity)` and save the result with `renzora::write_skeleton_file`, for example from a [Rust script](/docs/r1-alpha8/scripting/rust-scripts).

The target's rest pose is read off the model when its animator first loads, which is why a model must be spawned unposed.

## Event markers

Markers fire the `AnimationEvent` hook (see [Event markers](/docs/r1-alpha8/editor/animation#event-markers)) for every playing clip, skeletal or property-only, on the base or on a layer:

- each marker fires once each time playback crosses it, including when a loop wraps past it; a marker at time 0 fires when the clip starts;
- a clip played at another speed, or in reverse, fires in the order playback meets its markers;
- seeking does not fire the markers it jumps over, and a paused clip fires nothing;
- a clip faded to zero weight fires nothing.
