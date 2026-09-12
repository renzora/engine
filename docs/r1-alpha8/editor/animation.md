# Animation

Renzora animates two things, and a single clip file can carry both.

**Skeletal animation** comes from a rigged model: bone tracks imported from a `.glb`, `.fbx` or `.bvh`.

**Property animation** is authored in the editor: keyframe any component field, a light's intensity, a camera's position, a sprite's frame. It needs no skeleton, so you can animate a primitive cube.

Both live in `.anim` clip files, share the same transport, and play back identically in the editor and in an exported game.

Everything is keyed off **clip slot names** such as `idle` or `walk`, not file paths. You give each clip a slot name, and scripts, state machines and layers all refer to that name.

## Importing animations

Clips are extracted when you import a model. The importer pulls embedded animations out of `glb`, `gltf`, `fbx`, `usd` and `bvh` files and writes them into your project as `.anim` files. `.bvh` is animation only and carries no mesh.

1. Import an animated model.
2. The importer converts the mesh and extracts each embedded clip.
3. Add an **Animator** component to the model and register the clips as named slots.

For characters that ship without embedded animations, which is the common Mixamo pattern of one mesh plus separate animation files, the runtime drives the side-loaded clips against the skeleton beneath the model.

## The Animator component

Attach **Animator** to a model and it holds everything about how that model animates.

| Field | What it is |
|---|---|
| Clips | Named clip slots |
| Default Clip | The slot to auto-play on spawn |
| Blend Duration | Default crossfade time, in seconds |
| State Machine | An optional `.animsm` file |
| Layers | Optional animation layers, with the base layer first |

Each clip slot has a **name**, a **path**, a **looping** toggle, a **speed**, and optional **blend in** and **blend out** times.

The Inspector drawer covers the common workflow without leaving the panel: every slot with play, rename, speed, loop and remove controls, plus a drop field for adding an `.anim` file as a new slot.

<!-- screenshot: animation_panel.png - the Animation panel showing the clip library for a selected model -->

## The Animation workspace

Open it from the ribbon. Five panels follow whichever entity is selected.

| Panel | What it does |
|---|---|
| Animation | Clip library, state machine states and transitions, parameters and layers |
| Timeline | Transport, time ruler, scrubber, track lanes and keyframe editing |
| State Machine | A visual view of states and transition conditions |
| Parameters | Live parameter values fed into the state machine |
| Studio Preview | An isolated render of the model with an orbit camera and skeleton overlay |

Selection is forgiving. Clicking a mesh child or an individual bone resolves to the ancestor carrying the animator, so every panel follows along.

Editing a parameter in the **Parameters** panel pushes it straight into the running animator, so you can preview transitions without playing the game.

### When a panel has nothing to show

The panels offer the next step rather than sitting empty.

- **Animation candidates** lists every entity in the scene that looks animatable. Click one to select it.
- **Scan for clips** re-runs discovery on the model's folder and registers what it finds. This rescues a model placed in the scene before its animations finished importing.
- **Create State Machine** writes a starter `.animsm` beside the model with one state per clip, ready for transitions.

## The Timeline

<!-- screenshot: timeline.png - the Timeline panel with track lanes, keyframes and the playhead -->

| Action | How |
|---|---|
| Retime a keyframe | Drag it horizontally. Snap-aware. |
| Select a keyframe | Click it. The playhead jumps to it and its value shows in the toolbar. |
| Delete | `Delete` with a key selected, or right-click the key |
| Zoom the time axis | Mouse wheel |
| Set clip length | The **Length** field in the toolbar |

Dense keyframe runs draw as range bars rather than thousands of diamonds. Zoom in and they split back into editable keys.

### Shortcuts

While the cursor is over the Timeline:

| Key | Action |
|---|---|
| `Space` | Play or pause |
| `Home` / `End` | Jump to start or end |
| `←` / `→` | Step one frame |
| `K` | Add a keyframe on every track at the playhead |
| `N` | New track |
| `Delete` | Delete the selected keyframe |

## Animating a property

1. **Select the entity.**
2. If it has no clip, click **Create Animation**. That writes an empty clip and attaches an Animator. To add more clips to the same entity, type a name in the Timeline toolbar's new-clip field and click **+**. One entity can hold several clips, one per facing direction for example, and the selector dropdown switches between them.
3. **Add a track** with **+ Add Track**, then pick the property from its dropdown. The picker hides properties already used, so you cannot make duplicate tracks.
4. **Key it.** Move the playhead, pose the object, then press **Add Key** for every track or the **◆** on a single row. You can also right-click an empty spot on a track and choose **Add keyframe here**, or right-click a key and choose **Set to current pose**.
5. **Scrub** or press Play to preview.

**Record mode**, the red toggle, auto-keys any pose change at the playhead.

Edits auto-save, so Play mode sees them.

### Posing is not keying

Moving an object only changes its live transform. Nothing is written until you capture it.

While a keyframe is selected, posing updates that key. The preview never overwrites a manual pose, because grabbing the object pauses playback so your edit sticks.

### Keying from the Inspector

While a clip is open on the selected entity, every animatable Inspector field grows an amber **◆** button beside its reset. Click it to key that value at the playhead, and if the field is not animated yet this creates its track first.

### Interpolation

Right-click a key and pick its curve.

| Curve | What it does |
|---|---|
| Linear | Even blend toward the next key |
| Stepped | Hold this value until the next key, with no blend |
| Eased | Remap the blend through an easing curve: Smooth, Ease In, Ease Out, Ease In-Out, Back Out, Bounce Out, Elastic Out |

Easing applies to float, vector and colour tracks. The scrub preview and runtime playback use the identical curve.

Rotation is keyed as Euler degrees so a full 0 to 360 spin works. For a continuous spin, keep keys less than 180 degrees apart, such as 0, 120, 240, 360.

Clips authored before easing existed load with every key Linear.

### Sprite flipbooks

2D sprite-sheet animation is just a property track.

Add a **Sprite Sheet** component to the sprite and set its **H Frames** and **V Frames** to the sheet's grid. Then add a track bound to `SpriteSheet · Frame`.

For evenly timed frames, key `Frame = 0` at the start and `Frame = N` at the end with Linear interpolation. The frame index wraps past the last cell, so a looping clip cycles the whole sheet. For hand-timed frames with holds and anticipation, drop a Stepped key per frame.

**Directional sheets.** When each row of the sheet is a facing direction, do not try to put every direction on one timeline. They would all fight over the single Frame field. Direction is runtime state, not a track.

Two ways to handle it: make one clip per facing, each sweeping only its row, and switch clips from a script or a state machine. Or skip the timeline for locomotion and compute the frame from the direction each update.

See [Sprite Animation](/docs/r1-alpha8/editor/sprite-animation).

## Event markers

Markers are named points on the timeline that fire a script callback when playback crosses them. Footsteps, hit frames, spawn cues.

Type a name in the toolbar's marker field, then click the flag button to drop one at the playhead. Markers draw as labelled flags. Right-click one to delete it.

At runtime, crossing a marker delivers the `AnimationEvent` hook to every script on the entity:

```rust
fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if let ScriptHook::AnimationEvent { name, .. } = hook {
        if *name == "footstep" {
            // play a footstep sound
        }
    }
}
```

See [Rust Scripts](/docs/r1-alpha8/scripting/rust-scripts#lifecycle-hooks).

## State machines

A state machine picks which clip plays based on parameters you set from a script.

<!-- screenshot: animator_state_machine.png - the State Machine panel showing states and transitions -->

States are clips. Transitions connect them and carry conditions on float, bool and trigger parameters. The **Parameters** panel shows the live values, and editing one there previews the transition immediately.

<!-- screenshot: animator_params.png - the Parameters panel with live float and bool values -->

## Blend trees and layers

A **blend tree** mixes several clips by a parameter, which is how a walk blends into a run as speed rises.

**Layers** play clips on top of the base layer, masked to part of the skeleton, which is how a character waves while still walking. The base layer is layer 0.

## Driving animation from a script

Push an `AnimationCommand` onto the queue.

```rust
use bevy::prelude::*;
use renzora::ScriptCtx;
use renzora_animation::{AnimationCommand, AnimationCommandQueue};

fn update(ctx: &mut ScriptCtx) {
    let me = ctx.entity();
    if let Some(mut queue) = ctx.get_resource_mut::<AnimationCommandQueue>() {
        queue.push(AnimationCommand::Crossfade {
            entity: me,
            name: "run".into(),
            duration: 0.2,
            looping: true,
        });
    }
}
```

The commands are `Play`, `Stop`, `Pause`, `Resume`, `SetSpeed`, `Seek`, `Crossfade`, `SetParam`, `SetBoolParam` and `Trigger`.

### Reading the animator back

`AnimatorReadState` is a read-only mirror of live playback, updated every frame.

| Field | What it holds |
|---|---|
| `current_clip` | The playing clip slot |
| `current_state` | The state machine's current state |
| `state_time` | Seconds spent in that state |
| `time` | Current playback time |
| `playing` | Whether anything is actually playing |
| `clip_lengths` | Each clip's duration, once loaded |
| `params` / `bool_params` | Live state machine parameters |

```rust
if let Some(state) = ctx.get::<renzora_animation::AnimatorReadState>() {
    if state.current_clip == "jump" && !state.playing {
        // the jump finished
    }
}
```

## Studio Preview

An isolated render of the selected model with an orbit camera and a skeleton overlay, so you can judge a clip without finding the character in your level.

<!-- screenshot: studio_preview.png - the Studio Preview panel showing a character with its skeleton overlay -->

It searches the selection's descendants and can work out the model from the animator's clip paths. When nothing previewable is selected it shows a hint rather than going blank.
