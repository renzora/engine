# Inspector

The Inspector is where you read and tweak everything about the object you have selected — its position, its colors, its lights, its scripts, and more. Click something in your scene and all of its settings show up here, ready to edit.

This page covers the everyday basics. When you want the deep technical reference, the [Scripting API](/docs/r1-alpha7/api/scripting) and the [Inspector Fields](/docs/r1-alpha7/editor-dev/inspector-fields) guides have the full story.

![The Inspector panel showing the selected World Environment object, with collapsible sections for Name, Transform, Visibility, Directional Light, Volumetric Light, and TAA.](/assets/previews/inspector.png)

## Pick something to inspect

The Inspector always follows your **selection**. Click an object in the viewport, or click its name in the Hierarchy, and the Inspector instantly rebuilds to show that object.

Don't have anything to inspect yet? Open the **Add Entity** menu to drop a new object into your scene — a light, a camera, some terrain, an empty object, and so on. Once it's in the scene, select it and it appears in the Inspector.

![The Add Entity menu, with a search box and categories like General, Lighting, and Camera listing objects you can add to your scene.](/assets/previews/add_entity.png)

> Want the Inspector to stay on one object while you click around elsewhere? Use the **lock** toggle at the right of the entity header. It pins the Inspector to the current object until you unlock it.

## The entity header

Directly above the component list is a single fixed row describing the object itself, rather than any component on it. From left to right:

- **Icon** — the glyph this object shows in the Hierarchy. Click it to open a grid of icons and pick one; **Auto (from components)** at the top clears your choice and goes back to letting the object's components decide (a mesh gets a cube, a light gets a bulb). Useful when a scene is full of empty objects that all look alike but mean quite different things — a spawn point, a patrol waypoint, a trigger volume.
- **ID** — the object's unique identifier, and the name scripts use to find it. Typing here sanitizes what you enter (spaces and punctuation become `_`, everything lowercases) and de-duplicates it against every other object, so two things can never end up sharing an ID.
- **Label colour** — the colour of this object's row in the Hierarchy. Click the swatch for a colour picker.
- **Eye** — show or hide the object. A hidden object's eye is crossed out and tinted.
- **Lock** — pin the Inspector to this object, as above.

Both the icon and the label colour are saved with the scene, so they survive a reload and travel with the file to anyone else opening the project.

Some state isn't attached to any object — the time, the editor's own settings, a plugin's configuration. That is a **resource**, and because there is nothing to select, it never shows up here. Open the **Resources** panel instead (Add-Panel picker → *Debug*): it lists every resource in the running world and edits it the same way this panel edits a component. See [Resources & State](/docs/r1-alpha7/engine-core/resources).

## Reading the panel

Each object is made of **components** — small bundles of settings like *Transform* (position/rotation/scale), *Directional Light*, or *Visibility*. The Inspector shows one collapsible section per component.

In a section header you'll find:

- A **caret** to fold the section open or closed.
- An **icon** and the component's name.
- An **on/off toggle** (on components that support it) so you can switch a feature off without deleting it.
- A **trash** button to remove the component entirely. (**Scripts** and **Material** don't have one — they manage their own contents, with a per-script remove and the material binding controls instead.)

Inside each section are the editable fields. The most-used components are always pinned to the top in a fixed order — **Transform**, then **Scripts** and **Material** when present — so the things you reach for most are right where you expect them, no matter what else is on the object. Every other component follows below.

> The object's ID, icon, label colour and visibility are *not* in this list — they aren't components you can add or remove, so they live in the [entity header](#the-entity-header) above it instead.

The top bar holds three things: the **Add Component** button, a **filter box** — start typing a component name to hide everything else — and an **expand/collapse-all** button on the right. Click that once to open every section, again to collapse them all; it resets when you select a different object.

### Which sections start open

By default every section starts expanded. Hit the collapse-all button to fold them, or change the starting state in **Settings → Interface → Inspector → Default Expand**:

- **All Open** *(default)* — every section starts expanded.
- **Essentials Only** — Transform and Scripts open; the rest closed.
- **All Closed** — every section starts collapsed.

**What All Open costs.** A collapsed section is not merely hidden — its rows are despawned and the space reserved with a placeholder, so it genuinely costs nothing to have. Expanding is the expensive direction: on a scene with a world environment, terrain and camera, selecting an entity with everything open added ~1,082 bevy_ui nodes, and bevy_ui walks every node in the tree every frame whether or not anything changed. That measured ~3 ms/frame — about 72 fps down to 59. If a long component list starts costing you frames, **Essentials Only** is the setting to reach for.

This sets the *starting* state each time the Inspector rebuilds for a new selection — you can still fold any section by hand, and the expand/collapse-all button overrides it for the current object.

> Your edits apply live. Drag a value or flip a toggle and the change takes effect immediately — no Apply button, no waiting.

## Editing fields

Different settings get different controls, picked automatically to match the value:

- **Numbers** — drag left/right to scrub the value, or click to type an exact number.
- **X / Y / Z** — three colored drag boxes for things like position and rotation.
- **Toggles** — a simple on/off switch.
- **Colors** — a color picker (with an alpha option where it makes sense).
- **Text** — a single-line text box.
- **Dropdowns** — pick from a fixed list of choices.
- **Asset slots** — drag a file from the Asset Browser onto the slot (it only accepts the right file types).

> Every editable field has a small **reset** button (the circular ↺ arrow) just to its right. Click it to snap that field back to its default value — `0` for numbers, off for toggles, empty for text, white for colors, and so on. Action buttons and read-only fields don't show one, since there's nothing to reset.

> **Keyframe button.** When an animation clip is open in the Timeline for the selected entity, every animatable field gains an amber **◆** button just left of its reset button. Clicking it keys that field's current value at the playhead — and if the field isn't animated yet, it creates the track first, so you can start animating a property straight from the inspector. See [Animation → Authoring workflow](animation.md#authoring-workflow).

## Adding and removing components

- **Add** — click **Add Component** in the panel's top bar to open a list of everything you can add, grouped by category. Type to filter. A few sections are *inherent* rather than addable and so never show up in this list — **Scripts** on every entity, **2D Lighting** on a 2D camera — because they're always present already.
- **Remove** — click the **trash** button in a component's header. **Scripts** and **Material** intentionally have no header trash; remove individual scripts from their own section headers instead. Removing the last script also drops the underlying component, so an entity you never scripted carries nothing.
- **Turn off** — flip the header toggle to disable a component without removing it.

## Material

The **Material** section is the fastest way to dress a mesh. Its top row is the
material reference — thumbnail, name picker, and buttons to create a new
material (**+**), browse, open the Material Editor, or clear it. Below that is one drop slot per PBR channel (Base
Color, Normal, Roughness, Metallic, Ambient Occlusion, Emissive): drag an image
onto a slot and it's wired into the material graph and applied straight away.
Drag a whole texture set onto the material row and each file is routed to the
channel its name suggests. Full details in
[Materials](/docs/r1-alpha7/editor/materials#putting-a-material-on-an-object).

## Text & fonts

Any entity with text exposes two text sections:

- **Text Font** — pick the **Font** from a dropdown that auto-populates from your
  project's `fonts/` folder (drop a `.ttf`/`.otf` there and it appears) plus the
  built-in faces. Set the **Size**, and — for variable fonts — the **Weight**
  (100–900), **Width** (condensed ↔ expanded), **Spacing** (letter spacing, in
  px), and **Line** height (× font size).
- **Rich Text** — build *styled spans*: multiple runs of text on one line, each
  with its own text and color. Click **Add span** to append a run, edit its text
  and R/G/B inline, and use the **trash** button to remove it. Spans render in
  order after the base text, so you can mix colors and weights in a single label.

Fonts you use are saved into the scene and packed into the exported game (only
the fonts actually referenced are bundled — see [Exporting](/docs/r1-alpha7/exporting/overview)).

## Script properties

Attaching a script is one of the most useful things you can do in the Inspector. Drag a `.lua` or `.rs` file from the Asset Browser onto the **Drop to add script** target, or click the **+** button on the target's right edge to pick one from a scrolling list of your project's scripts.

Each attached script gets its own **collapsible section** — a header with a caret, a **file icon**, the script's file name, an **enable toggle**, and a per-script **trash** button — so an entity carrying several scripts stays tidy. Click a header to fold that script's variables away; the fold state is remembered while you work.

The header's **file icon is a button**: click it to open that script straight in the **Code Editor** (the panel is added to your layout if it isn't already open), so you can jump from tuning a variable to editing the code behind it in one click.

Any variable your script declares in its `props()` function shows up as an editable field — so you can tune gameplay values (speed, jump height, color, a team name) right in the Inspector, with no code changes.

```lua
-- player.lua
function props()
    return {
        speed     = { value = 5.0, hint = "Walk speed (units/s)" },
        can_jump  = { value = true },
        team      = { value = "red" },
    }
end
```

Each entry just needs a `value` (which sets both the default and the field type) and, optionally, a `hint` for a helpful tooltip. Numbers become draggable fields, `true`/`false` becomes a toggle, text becomes a text box, and so on.

Whatever you set in the Inspector is saved per-object and feeds straight back into the running script.

See [Scripting Overview](/docs/r1-alpha7/scripting/overview) to get started writing scripts, and the [Scripting API](/docs/r1-alpha7/api/scripting) for the full list of functions you can call.

## For programmers: custom components

Made your own Bevy component and want it to show up here automatically? Add `#[derive(Inspectable)]` and register it — Renzora generates the field rows for you.

```rust
use bevy::prelude::*;
use renzora::{AppEditorExt, Inspectable};

#[derive(Component, Default, Reflect, Inspectable)]
#[inspectable(name = "Health", icon = "HEART", category = "gameplay")]
pub struct Health {
    #[field(speed = 1.0, min = 0.0, max = 10000.0)]
    pub current: f32,
    pub max: f32,
}
```

Then call `app.register_inspectable::<Health>();` from your plugin. The contract types live in the `renzora` crate behind its `editor` feature (engine built on **Bevy 0.19**).

That's the short version. For per-field attributes, custom widgets, and fully native drawers, see [Inspector Fields](/docs/r1-alpha7/editor-dev/inspector-fields).
