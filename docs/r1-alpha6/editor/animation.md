# Animation

Renzora's animation system is built on Bevy's `AnimationGraph` and adds RON clip files, state machines, blend trees, layers, and procedural tweens — all authored in the editor and driven from Lua or Rhai.

## How it works

Animation lives in the `renzora_animation` crate (`AnimationPlugin`), which self-registers as a runtime plugin, so it runs both in the editor and in exported games. The editor tooling lives in `renzora_animation_editor` (`AnimationEditorPlugin`, editor-only).

At runtime the plugin:

- Loads `.anim` clip files (`AnimClipLoader`) into Bevy `AnimationClip` assets.
- Loads `.animsm` state-machine files (`AnimSmLoader`) into `AnimationStateMachine` assets.
- Reads the scene-serializable `AnimatorComponent`, builds an `AnimationGraph` from its clips, finds (or inserts) the `AnimationPlayer` in the model hierarchy, and tags skeleton bones with `AnimationTargetId` + `AnimatedBy`.
- Mirrors live playback into a read-only `AnimatorReadState` component that scripts and blueprints can poll.

> The whole system is keyed off **clip slot names** (e.g. `"idle"`, `"walk"`), not file paths. You give each `.anim` file a slot name on the `AnimatorComponent`, then everything else — scripts, state machines, layers — refers to that name.

## Importing animations

Animation clips are extracted at **import time** from 3D model files. The importer pulls embedded clips out of `glb`, `gltf`, `fbx`, `usd`, and `bvh` files and writes them into your project as `.anim` RON files. (`.bvh` is animation-only — it carries no mesh.)

1. Drop an animated model into your project's `assets/` folder.
2. The import pipeline converts the mesh to GLB and extracts each embedded animation clip to a `.anim` file.
3. Add an **Animator** component to the model entity in the Inspector and register the extracted clips as named slots (see below).

> Only `.glb`/`.gltf` load directly at runtime. Other model formats — including FBX and the USD/Alembic/Collada/BVH/Blend family — are import-time conversions, not runtime loaders.

For characters that ship without embedded animations (a common Mixamo workflow — one mesh plus separate animation files), the runtime inserts an `AnimationPlayer` on the model entity itself and drives the side-loaded `.anim` clips against the skeleton beneath it.

## The Animator component

`AnimatorComponent` is the scene-persistent controller you attach to a model. It is reflected and serialized, so it round-trips through `.ron` scenes.

| Field | Type | Purpose |
|-------|------|---------|
| `clips` | `Vec<AnimClipSlot>` | Named clip slots (see below). |
| `default_clip` | `Option<String>` | Slot name to auto-play on spawn. |
| `blend_duration` | `f32` | Default crossfade time in seconds (default `0.2`). |
| `state_machine` | `Option<String>` | Asset path to a `.animsm` file. |
| `layers` | `Vec<AnimationLayer>` | Optional animation layers (base layer is index 0). |

Each `AnimClipSlot` is a named reference to one `.anim` file:

| Field | Type | Purpose |
|-------|------|---------|
| `name` | `String` | Slot label used by scripts, layers, and the state machine. |
| `path` | `String` | Asset-relative path to the `.anim` file (e.g. `animations/walk.anim`). |
| `looping` | `bool` | Whether the clip loops by default. |
| `speed` | `f32` | Default playback speed. |
| `blend_in` | `Option<f32>` | Crossfade time when transitioning *into* this clip. |
| `blend_out` | `Option<f32>` | Crossfade time when transitioning *out of* this clip. |

### Editing in the Inspector

The Animator component has a full native Inspector drawer, so the common workflow never leaves the Inspector panel:

- **Clip library** — every slot with play, rename, speed, loop toggle, and remove controls.
- **Drop field** — drag an `.anim` file from the asset panel to add it as a new slot.
- **Default clip** — a dropdown over the slot names; picking one also plays it for instant feedback.
- **Blend time** — the animator's global crossfade duration.
- **State machine** — assign or clear the `.animsm` file.

### The `.anim` file format

An `.anim` file is RON-serialized `AnimClip`: a name, a duration in seconds, and one `BoneTrack` per animated bone. Each track holds time-stamped translation, rotation (quaternion XYZW), and scale keyframes. A channel needs at least two keyframes to produce a curve.

```ron
(
    name: "walk",
    duration: 1.0,
    tracks: [
        (
            bone_name: "Hips",
            translations: [
                (0.0, (0.0, 1.00, 0.0)),
                (0.5, (0.0, 1.05, 0.0)),
                (1.0, (0.0, 1.00, 0.0)),
            ],
            rotations: [],
            scales: [],
        ),
        (
            bone_name: "Spine",
            translations: [],
            rotations: [
                (0.0, (0.0, 0.0, 0.0, 1.0)),
                (1.0, (0.0, 0.05, 0.0, 0.998)),
            ],
            scales: [],
        ),
    ],
)
```

> Bone names must match the `Name` of the corresponding entity in the imported skeleton — that's how curves are routed to bones (`AnimationTargetId::from_name`).

## Editor panels

Open the **Animation** workspace from the ribbon. It hosts five dockable panels that follow whichever entity is selected in the hierarchy:

| Panel id | Title | What it does |
|----------|-------|--------------|
| `animation` | Animation | Clip library, state-machine states/transitions, parameters, and layers for the selected animator. |
| `timeline` | Timeline | Transport bar, time ruler, scrubber, track lanes, and keyframe editing for the selected clip. |
| `animator_state_machine` | State Machine | Visual view of states and transition conditions. |
| `animator_params` | Parameters | Live float/bool/trigger parameter values fed into the state machine. |
| `studio_preview` | Studio Preview | Isolated offscreen render of the selected model with an orbit camera and skeleton overlay. |

The panels share an `AnimationEditorState` (selected clip, scrub time, preview speed/looping, snap interval, timeline zoom). Editing a parameter in the **Parameters** panel pushes a live command into the running animator so you can preview transitions without playing the game.

Selection is forgiving: clicking a mesh child or an individual bone resolves to the ancestor that carries the animator/model, so every panel follows along. The studio preview also searches the selection's descendants and can infer the model's GLB from the animator's clip paths; when nothing previewable is selected it overlays a hint chip on the studio backdrop instead of going blank.

### Guided setup

The panels are never a dead end — when the current selection has nothing to show, they offer the next step instead of sitting empty:

- **Animation candidates** — with nothing selected, the Animation panel lists every entity in the scene that looks animatable (models with skeletons, clips, or an existing animator). Click one to select it and continue.
- **Scan for clips** — re-runs `.anim` discovery on the model's folder and registers what it finds as clip slots. This rescues models that were placed in the scene before their animations finished importing (or that ship animations as separate files).
- **Create State Machine** — writes a starter `.animsm` next to the model with one state per clip slot and assigns it to the animator, ready to add transitions.

### Editing keyframes in the timeline

The Timeline panel edits the selected clip directly:

- **Drag** a keyframe horizontally to retime it — dragging respects the snap interval.
- **Right-click** a keyframe to delete it.
- **Save** writes the modified clip back to its `.anim` file; the button tracks dirty state so you can see when there are unsaved changes.
- **Mouse wheel** zooms the time axis.
- Dense keyframe runs draw as per-channel **range bars** rather than thousands of individual diamonds; zoom in and they split back into editable keys.

## State machines

A state machine (`.animsm`) automates transitions so you don't have to script every clip change. It is RON-serialized `AnimationStateMachine`:

- **States** — each has a `name`, a `motion`, a playback `speed`, and a `looping` flag.
- **Transitions** — each has a `from` state, a `to` state, a `condition`, and a `blend_duration`.
- **`default_state`** — the state entered on startup.

Point the `AnimatorComponent.state_machine` field at the file, and the runtime evaluates the active state's outgoing transitions every frame.

```ron
(
    default_state: "idle",
    states: [
        (name: "idle", motion: Clip("idle"), speed: 1.0, looping: true),
        (name: "walk", motion: Clip("walk"), speed: 1.0, looping: true),
        (name: "jump", motion: Clip("jump"), speed: 1.0, looping: false),
    ],
    transitions: [
        (from: "idle", to: "walk", condition: FloatGreater("speed", 0.1), blend_duration: 0.2),
        (from: "walk", to: "idle", condition: FloatLess("speed", 0.1),    blend_duration: 0.2),
        (from: "idle", to: "jump", condition: Trigger("jump"),            blend_duration: 0.1),
        (from: "jump", to: "idle", condition: TimeElapsed(0.8),           blend_duration: 0.2),
    ],
)
```

### Transition conditions

Conditions are evaluated against the machine's runtime parameters and the time spent in the current state. The first matching transition (in declaration order) wins.

| Condition | Fires when |
|-----------|-----------|
| `FloatGreater("name", x)` | float param `name` > `x` |
| `FloatLess("name", x)` | float param `name` < `x` |
| `BoolTrue("name")` | bool param `name` is true |
| `BoolFalse("name")` | bool param `name` is false |
| `Trigger("name")` | one-shot trigger `name` was fired (consumed when the transition is taken) |
| `TimeElapsed(secs)` | the current state has run for at least `secs` seconds |
| `Always` | immediately (useful for pass-through states) |

### Driving parameters from scripts

Set the machine's parameters from a script attached to the animated entity. Float and bool parameters persist until you change them; triggers are one-shot and are consumed by the transition that reads them.

```lua
function on_update()
    -- input_x / input_y are context globals (the movement axes)
    local speed = math.sqrt(input_x * input_x + input_y * input_y)
    set_anim_param("speed", speed)     -- float parameter
    set_anim_bool("grounded", true)    -- bool parameter

    if mouse_left_just_pressed then
        set_anim_trigger("jump")       -- one-shot trigger (Lua alias for trigger_anim)
    end
end
```

The same script in Rhai (use `trigger_anim` — `set_anim_trigger` is a Lua-only alias):

```rhai
fn on_update() {
    let speed = (input_x * input_x + input_y * input_y).sqrt();
    set_anim_param("speed", speed);
    set_anim_bool("grounded", true);
}
```

## Blend trees

`BlendTree` lets a state blend multiple clips. A state references one with `motion: BlendTree("name")`. The tree itself is recursive:

| Node | Blends |
|------|--------|
| `Clip("name")` | a single clip slot |
| `Lerp { a, b, param }` | linearly between `a` and `b` by a float param (0 = A, 1 = B) |
| `BlendSpace2D { entries, param_x, param_y }` | clips placed in a 2D parameter space |
| `Additive { base, overlay, param }` | an overlay on top of a base, weighted by a param |

> Blend trees are part of the `.animsm` format and editor today, but the runtime currently only plays single-clip states (`Clip(...)`). A state whose `motion` is a `BlendTree` will still be entered, but blend-tree weight evaluation is not yet wired into playback. For now, prefer `Clip` states plus crossfades or layers.

## Animation layers

Layers let you stack motion — for example, an upper-body wave on top of a full-body run. Each `AnimationLayer` carries:

| Field | Type | Purpose |
|-------|------|---------|
| `name` | `String` | Layer label (e.g. `"base"`, `"upper_body"`). |
| `weight` | `f32` | Blend weight, 0.0–1.0. |
| `mask` | `Option<Vec<String>>` | Optional list of bone names this layer affects. |
| `blend_mode` | `LayerBlendMode` | `Override` (default) or `Additive`. |
| `current_clip` | `Option<String>` | Clip slot currently playing on the layer. |

Adjust a layer's weight at runtime by name:

```lua
function on_update()
    -- fade the upper-body layer in while aiming
    set_layer_weight("upper_body", aiming and 1.0 or 0.0)
end
```

> The runtime applies each layer's `weight` to its `current_clip`'s graph node. Bone masking and additive blend modes are stored on the layer but are not yet fully evaluated at runtime.

## Playing animations from scripts

These functions are registered in **both** Lua and Rhai. They act on the **entity the script is attached to** — there is no entity argument. Refer to clips by their slot `name`.

| Function | Lua | Rhai | Notes |
|----------|-----|------|-------|
| `play_animation(name [, looping [, speed]])` | ✅ | `play_animation(name)` | Lua `looping` defaults to `true`, `speed` to `1.0`. Rhai always loops. |
| `crossfade_animation(name, duration [, looping])` | ✅ | `crossfade_animation(name, duration)` | Smoothly blend to a clip over `duration` seconds. |
| `stop_animation()` | ✅ | ✅ | Stop the current clip. |
| `pause_animation()` / `resume_animation()` | ✅ | ✅ | Pause / resume playback. |
| `set_animation_speed(speed)` | ✅ | ✅ | `1.0` = normal, `2.0` = double, negative = reverse. |
| `set_anim_param(name, value)` | ✅ | ✅ | Set a state-machine float parameter. |
| `set_anim_bool(name, value)` | ✅ | ✅ | Set a state-machine bool parameter. |
| `trigger_anim(name)` | ✅ | ✅ | Fire a one-shot trigger. (`set_anim_trigger` is a Lua-only alias.) |
| `set_layer_weight(layer_name, weight)` | ✅ | ✅ | Set a layer's blend weight. |
| `get_animation_length(name)` | ✅ | ❌ | Clip length in seconds (`0` if not loaded). Lua only. |

```lua
function on_ready()
    play_animation("idle")          -- loop the idle clip
end

function on_update()
    local speed = math.sqrt(input_x * input_x + input_y * input_y)
    if speed > 0.1 then
        crossfade_animation("run", 0.2)
    else
        crossfade_animation("idle", 0.3)
    end
end
```

To play a clip once instead of looping, pass `false` for `looping` in Lua, or mark the clip slot `looping: false`:

```lua
-- one-shot attack (Lua: looping = false)
play_animation("attack", false)
```

### Reading animator state

Live playback is mirrored into an `AnimatorReadState` component, read through reflection with `get(...)`:

| Path | Value |
|------|-------|
| `get("AnimatorReadState.current_clip")` | name of the playing clip slot |
| `get("AnimatorReadState.current_state")` | current state-machine state |
| `get("AnimatorReadState.state_time")` | seconds spent in the current state |
| `get("AnimatorReadState.clip_lengths.<clip>")` | duration of a loaded clip |
| `get("AnimatorReadState.params.<name>")` | a float parameter |
| `get("AnimatorReadState.bool_params.<name>")` | a bool parameter |

### Reacting when a clip finishes

There is **no keyframe-event system and no `on_anim_event` hook**. To react to a clip ending, either:

- Poll the read state — compare `state_time` (or your own timer) against `get_animation_length(name)`, or watch `current_clip` change.
- Use a **visual blueprint** — the `animation/on_finished` event node fires when a non-looping clip finishes. (The runtime detects the finished clip and surfaces it to the blueprint interpreter; this is the supported event path.)

```lua
function on_update()
    if get("AnimatorReadState.current_clip") == "attack"
       and get("AnimatorReadState.state_time") >= get_animation_length("attack") then
        crossfade_animation("idle", 0.2)
    end
end
```

## Procedural tweens

For one-off transform animations (move a door, pop a UI element) without authoring a clip, fire a tween through `action()`. Tweens interpolate `Transform` over a duration with an easing curve, then remove themselves.

```lua
function on_ready()
    action("tween_position", { target = vec3(0.0, 3.0, 0.0), duration = 1.5, easing = "ease_out_back" })
    action("tween_rotation", { target = vec3(0.0, 180.0, 0.0), duration = 1.0, easing = "ease_in_out" })
    action("tween_scale",    { target = vec3(2.0, 2.0, 2.0),   duration = 0.5, easing = "ease_out_bounce" })
end
```

> Tweens run through `action()`, which is **Lua-only** — Rhai has no `action()` verb. `tween_rotation` takes Euler angles in degrees. Easing defaults to `ease_in_out` if the name is unrecognized.

Available easing names: `linear`, `ease_in`, `ease_out`, `ease_in_out`, `ease_in_quad`, `ease_out_quad`, `ease_in_out_quad`, `ease_in_cubic`, `ease_out_cubic`, `ease_in_out_cubic`, `ease_in_back`, `ease_out_back`, `ease_in_out_back`, `ease_in_elastic`, `ease_out_elastic`, `ease_in_bounce`, `ease_out_bounce`.

## Notes

- **Crossfade duration** of 0.15–0.3 seconds works well for most character transitions.
- **Per-clip blends:** a clip slot's `blend_in` / `blend_out` override the animator's global `blend_duration` when transitioning into or out of that clip.
- **Speed can be negative** to play a clip in reverse.
- **Scripts on the dedicated server:** animation scripts also run headless on a `--server` build, so server-authoritative logic can drive the same parameters.
- **Rhai is a subset of Lua.** The core playback functions above all work in Rhai, but `action()`-based tweens, `set_anim_trigger`, and `get_animation_length` are Lua-only, and Rhai supports only the `props` / `on_ready` / `on_update` hooks.
