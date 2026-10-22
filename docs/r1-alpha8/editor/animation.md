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

### Animated scenes: doors, machines and fly-through cameras

An FBX can be animated on its **objects** rather than a skeleton: doors that swing, pistons that slide, a camera that flies down a corridor. These import as well.

When an FBX has no skeleton but its nodes are animated, the importer keeps every animated node, and the chain of parents above it, as a named node in the model. Each one carries the geometry that moves with it. Everything that never moves is still merged into one mesh, so a mostly static building stays cheap to draw. The clip gets one track per animated node, under the same name as the node.

Node names are cleaned up the same way the engine names entities (`DOOR_1` becomes `door_1`). Two nodes with the same name get a number on the second one (`sliding_3`, `sliding_3_2`), because a clip finds its node by name and two nodes with one name would leave one of them unanimated.

Dropping the model into the scene adds an **Animator** with the clip as its default, so the scene starts playing straight away.

### Riding an imported camera

A camera in the FBX arrives as an animated node, not as a camera the engine can render from. Dropping the model into the viewport adds a scene camera that rides it, so to watch the fly-through you only press Play.

The added camera is named after the FBX camera with `_camera` on the end (`dynamiccamera2_camera`), and it:

- carries **Follow Node** with its **Target** set to the camera's view node, the FBX camera's name followed by `_view`;
- has the field of view the FBX camera was authored with;
- becomes the scene's **default camera**, taking over the effect settings (bloom, exposure, tonemapping and the rest) of the camera that was the default before, so the scene looks the same through it.

To go back to your own camera, make it the default again from the Hierarchy. The camera is an ordinary scene entity and is saved with the scene. It is only added when the model is dropped, never when a scene loads, so deleting it keeps it deleted; dropping the same model again does not add a second one while the first still follows that node.

The importer writes the `_view` node under every FBX camera, turned to face the way an engine camera looks (FBX cameras look down their own X axis), and records the camera's name and field of view on it. Follow Node copies that node's position and rotation onto its entity every frame, after the animation has been applied, so the camera never lags a frame behind the scene. Scale is not copied: a model's top node often holds the file's unit conversion, and a scaled camera would scale its view with it.

Follow Node works on any entity, not only cameras: anything can ride a moving part by naming it. To set one up by hand, add **Follow Node** and type the node's name into **Target**.

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

The Inspector drawer covers the common workflow without leaving the panel. Each slot is a small card: a play button, the name (click to rename), a **Speed** field (1 is the clip's own speed, from 0.05 to 5), a loop toggle that lights while the clip loops, and remove, with the clip's file under them. Drop `.anim` files on the area under the list to add them as new slots; a multi-selection from the Assets panel adds every clip in it.

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

Selection is forgiving. Clicking a mesh child or an individual bone resolves to the ancestor carrying the animator, so every panel follows along. Deselecting (clicking empty space, `Escape`) leaves the panels on the entity they were showing; they switch only when you pick another entity. Each entity remembers the clip you last had open on it, so selecting it again brings that clip back rather than its default.

Editing a parameter in the **Parameters** panel pushes it straight into the running animator, so you can preview transitions without playing the game.

### When a panel has nothing to show

The panels offer the next step rather than sitting empty.

- **Animation candidates** lists every entity in the scene that looks animatable. Click one to select it.
- **Scan for clips** re-runs discovery on the model's folder and registers what it finds. This rescues a model placed in the scene before its animations finished importing.
- **Create State Machine** writes a starter `.animsm` beside the model with one state per clip, ready for transitions.

## The Timeline

<!-- screenshot: timeline.png - the Timeline panel with track lanes, keyframes and the playhead -->

### The toolbar

Everything sits on one row.

- **Left:** the clip picker, showing the entity and clip (click it to switch clips, **Rename clip...** the current one, start a **New clip...**, or **Use an existing clip...** from the project; picking a clip frames it to fill the lanes), a **+** that creates a new clip straight away, and the red **Record** toggle.
- **Centre:** the transport (first frame, previous key, play/pause, next key, last frame, loop) and the time readout: the playhead's time, the clip's length and the frame number, each in a fixed-width slot so the buttons never shift as the numbers change. First frame goes to the loop start, and pressed again there to `0`; last frame goes to the loop end, then to the clip's end.
- **Right:** playback speed, snap, the **Keys / Curves** switch, and **⋯**.

The **⋯** menu holds what used to crowd the bar: the clip **Length**, a name field and **Add at playhead** for event markers, zoom in, zoom out and **Frame all**, the **Row height** (**Compact**, **Normal** or **Roomy**), **Add property track**, **Key every property track**, and **Save now**.

### Tracks and keys

The track list on the left has a filter box, **+ Track**, a right edge you can drag to widen it for long bone names, and an **All tracks** row (marked with a stack icon) whose lane summarises every key in the clip: a diamond at each key time on any track, and a tinted bar wherever some track keys continuously (every frame or two, as an imported clip does). Drag a summary diamond to retime every key at that moment on every track at once; it snaps like any key drag and can be undone. Diamonds closer together than a few pixels are thinned out; zoom in to reach each one. Bones nest under their parent bone; click the caret to fold a subtree. Each bone row shows **T**, **R** and **S** chips for the translation, rotation and scale channels it animates, tinted to match their keys. Lanes are flat. A scrollbar along the bottom scrolls through time and one down the right scrolls the rows; drag a thumb, or click the bar beside it to jump there. Dragging a scrollbar never moves the playhead.

Right-click a bone or property row in the track list for its menu:

- **Delete track** removes the track and its keys from the clip.
- **Disable track** keeps the track and its keys but leaves it out of playback, in the editor preview and in play mode, so you can see how the clip plays without it. A disabled row and its lane are dimmed and the row shows an eye-slash; **Enable track** in the same menu turns it back on. The flag is saved in the `.anim` file as `muted: true`.

Both are ordinary clip edits, so they can be undone.

A stretch of every-frame keys, which is what an imported clip is made of, draws as one bar with a diamond at each end. Zoom in until the keys are far enough apart and the inner keys become diamonds you can pick. Lone keys always draw as diamonds. Selected keys are amber with a ring.

| Action | How |
|---|---|
| Select a key | Click it. `Shift` + click adds or removes it from the selection. |
| Box-select | Drag on empty lane space. `Shift` adds to the selection. |
| Retime | Drag a selected key; every selected key moves with it. Snap-aware. |
| Edit a key | Click a key without dragging to open its popover |
| Move the playhead | Click empty lane space, press or drag in the lower half of the ruler, or drag the playhead itself by its needle or its time badge |
| Set the loop region | Drag its start or end handle in the upper half of the ruler, drag the blue band to move it whole, or press `I` / `O` to set its start / end at the playhead. The clip's keys are not changed. |
| Set clip length | Drag the clip-length marker (the tab on the ruler's bottom edge), or the **Length** field in **⋯** |
| Scroll the tracks | Mouse wheel, or the right-hand scrollbar |
| Pan the time axis | `Shift` + wheel, a sideways trackpad swipe, or the bottom scrollbar |
| Zoom the time axis | `Ctrl` + wheel, about the cursor. Deep enough that one frame spans over a hundred pixels. |
| Taller or shorter rows | `Alt` + wheel, or **Row height** in **⋯** |
| Pan freely | Middle-drag anywhere over the lanes, or right-drag from empty lane space (a right-click that does not move still opens the menu) |

The ruler has two halves. The **upper half** holds the preview loop region: a blue band between a start handle and an end handle, brighter and with a thin line along its top while looping is on. Drag either handle to resize it, or drag the band to move it, keeping its length. The **lower half** scrubs, and carries the **clip-length marker**: a small tab in the muted text colour at the clip's end, which is the one control here that changes the clip itself. On a fresh clip the region is the whole clip, so the loop end sits right above the length marker; the upper half takes the loop end and the lower half the length. While the band still spans the whole clip it has nowhere to move, so the upper half scrubs too.

With looping on, preview playback runs to the loop end and wraps back to the loop start. Started from past the loop end, it plays on to the clip's end first and then wraps. The region only affects the editor preview, never the clip's keys or runtime playback, and it resets to the whole clip when you switch clips. A region left at the clip's end follows it when the length changes.

Where a loop handle and the playhead overlap, the handle is taken; grab the playhead by the side of its badge instead. Dragging the badge does not jump the playhead under the cursor first. The time labels run across the whole ruler, past the clip's end too, so none disappear while a handle is dragged. The cursor is a grab hand over a key, the playhead or the band, a resize arrow over a handle or the length marker, and the ordinary pointer over empty space.

### Particles

When the selected entity, or the entity the Timeline resolved it to, has a [Particle System](../rendering/particle-system.md) (on itself or on a child), its effect's [keyed parameters](../rendering/particle-system.md#keyed-parameters) appear under the animation tracks as a **Particles · *effect name*** group, in the same rows, diamonds and bars, with a sparkle where a bone row has its bone:

- one row per **emitter**, whose bar shows where it is on (a key on it switches it on or off);
- one row per **burst** track, a diamond per burst;
- one row per keyed **parameter**, named like **Flames · Spawn Rate**.

**+ Track** then offers **Property track** and a **Particle parameter** submenu: each emitter's **Enabled**, **Burst**, and every unwired number, vector or colour input of its graph. Keys work as everywhere else: right-click a row's lane for **Add keyframe here** (on a parameter it keys the value it has there; on an emitter row it switches the emitter), drag to retime, `Delete` to remove, click for the key card (time, value, and **Linear**, **Smooth** or **Step**), or right-click a key for delete and interpolation. `K` keys every parameter at the playhead. Right-click a row in the track list to delete it.

The playhead drives the effect: scrub and it shows exactly that moment (the effect is replayed to it, so it is deterministic); press Play and it plays with the animation from the same clock, wrapping with the loop region. Until you scrub or play, a selected effect keeps playing on its own, and it gets its own playback back when you select something else.

An entity with a Particle System and no animation clip still gets the Timeline: the clip picker shows the effect's name and the ruler is the effect's duration (dragging the length marker changes it). On a `.vfx` document tab the Timeline shows that effect. Edits go through the effect editor's document, so they show in the viewport at once, are undone with `Ctrl` + `Z`, and are saved with the effect (Save in either panel, or automatically when the effect is not open in a tab). A built-in effect is shown but cannot be keyed: copy it into the project first.

### The key popover

Clicking a key opens a small card beside it with the key's **Time** and its values (bone rotations as degrees), six easing buttons (Linear, Smooth, Ease in, Ease out, Step, Bounce), and **Copy** and **Delete**. Easing applies to property keys; bone keys always blend linearly. An easing button applies to every selected key. Every edit made here can be undone. `Escape` closes it.

### Curves

**Curves** swaps the lanes for a value graph of the selected track: a labelled value grid aligned with the ruler, the sampled curve, and its keys. For a bone, the toolbar picks which channel to plot (Position, Rotation or Scale); for a property track it offers **Smooth**, **Linear** and **Step** for the selected keys. **Frame all** fits the clip. Hovering reads out the time and value under the cursor, and keys can be dragged here too.

### No clip yet

With an animatable entity selected but no clip open, the Timeline offers three cards:

- **Record** creates a clip with translation, rotation and scale tracks and arms record mode, so posing the object keys it.
- **Key a property** creates a clip and opens the property picker for its first track.
- **Use an existing clip** opens an `.anim` file from the project.

### Shortcuts

While the cursor is over the Timeline:

| Key | Action |
|---|---|
| `Space` | Play or pause |
| `Home` / `End` | Jump to the loop region's start or end (the clip's, until a region is set) |
| `Shift` + `Home` / `Shift` + `End` | Jump to the clip's start or end |
| `I` / `O` | Set the loop region's start / end at the playhead |
| `←` / `→` (or `,` / `.`) | Step one frame |
| `K` | Add a keyframe on every track at the playhead |
| `N` | New track |
| `Delete` / `Backspace` | Delete the selected keys |
| `Ctrl` + `C` / `Ctrl` + `V` | Copy the selected keys / paste them at the playhead |
| `Escape` | Close the popover, then clear the selection |

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
