# Inspector

The Inspector is where you read and tweak everything about the object you have selected — its position, its colors, its lights, its scripts, and more. Click something in your scene and all of its settings show up here, ready to edit.

This page covers the everyday basics. When you want the deep technical reference, the [Scripting API](/docs/r1-alpha5/api/scripting) and the [Inspector Fields](/docs/r1-alpha5/editor-dev/inspector-fields) guides have the full story.

![The Inspector panel showing the selected World Environment object, with collapsible sections for Name, Transform, Visibility, Directional Light, Volumetric Light, and TAA.](/assets/previews/inspector.png)

## Pick something to inspect

The Inspector always follows your **selection**. Click an object in the viewport, or click its name in the Hierarchy, and the Inspector instantly rebuilds to show that object.

Don't have anything to inspect yet? Open the **Add Entity** menu to drop a new object into your scene — a light, a camera, some terrain, an empty object, and so on. Once it's in the scene, select it and it appears in the Inspector.

![The Add Entity menu, with a search box and categories like General, Lighting, and Camera listing objects you can add to your scene.](/assets/previews/add_entity.png)

> Want the Inspector to stay on one object while you click around elsewhere? Use the **lock** toggle in a section header. It pins the Inspector to the current object until you unlock it.

## Reading the panel

Each object is made of **components** — small bundles of settings like *Transform* (position/rotation/scale), *Directional Light*, or *Visibility*. The Inspector shows one collapsible section per component.

In a section header you'll find:

- A **caret** to fold the section open or closed.
- An **icon** and the component's name.
- An **on/off toggle** (on components that support it) so you can switch a feature off without deleting it.
- A **trash** button to remove the component entirely.

Inside each section are the editable fields. The most-used components are always pinned to the top in a fixed order — **Name**, **Transform**, then **Scripts** and **Material** when present — so the things you reach for most are right where you expect them, no matter what else is on the object. Every other component follows below.

To focus a single component, use the **component filter**. It comes in two styles, switched in **Settings → Interface → Inspector → Component Filter**:

- **Dropdown** *(default)* — a single dropdown in the top bar listing the components on the object; pick one to filter, or **All components** to clear it.
- **Vertical Menu** — one icon button per component down the left edge, plus an **All** entry at the top. Hover a button for a tooltip naming the component (the bubble stays on-screen, flipping to the other side or sliding up/down near an edge). Click to filter the panel to just that component; click it again (or **All**) to show everything. The active button is highlighted.

At the very top is a **filter box** — start typing a component name to hide everything else — and an **expand/collapse-all** button on the right. Click it once to open every section, again to collapse them all; it resets when you select a different object. (The bottom of the list has an **Add Component** button.)

### Which sections start open

By default, only the **Name**, **Transform**, and **Scripts** sections start expanded when you select something — everything else starts collapsed so a busy object stays scannable. You can change this in **Settings → Interface → Inspector → Default Expand**:

- **Essentials Only** *(default)* — Name, Transform, and Scripts open; the rest closed.
- **All Open** — every section starts expanded.
- **All Closed** — every section starts collapsed.

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

## Adding and removing components

- **Add** — click **Add Component** (top or bottom of the panel) to open a list of everything you can add, grouped by category. Type to filter.
- **Remove** — click the **trash** button in a component's header.
- **Turn off** — flip the header toggle to disable a component without removing it.

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
the fonts actually referenced are bundled — see [Exporting](/docs/r1-alpha6/exporting/overview)).

## Script properties

Attaching a script is one of the most useful things you can do in the Inspector. Drag a `.lua` or `.rhai` file from the Asset Browser onto the **Drop to add script** target, or click **Add Script** to pick one from your project. A **Script** section then appears.

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

`props()` works in **both** Lua and Rhai. Whatever you set in the Inspector is saved per-object and feeds straight back into the running script.

See [Scripting Overview](/docs/r1-alpha5/scripting/overview) to get started writing scripts, and the [Scripting API](/docs/r1-alpha5/api/scripting) for the full list of functions you can call.

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

That's the short version. For per-field attributes, custom widgets, and fully native drawers, see [Inspector Fields](/docs/r1-alpha5/editor-dev/inspector-fields).
