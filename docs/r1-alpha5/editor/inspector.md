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

Inside each section are the editable fields. The first two sections are always **Name** and **Transform**, so the things you reach for most are right at the top.

At the very top is an **Add** button and a **filter box** — start typing a component name to hide everything else. There's a second Add button at the bottom of the list for convenience.

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

## Adding and removing components

- **Add** — click **Add Component** (top or bottom of the panel) to open a list of everything you can add, grouped by category. Type to filter.
- **Remove** — click the **trash** button in a component's header.
- **Turn off** — flip the header toggle to disable a component without removing it.

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

Then call `app.register_inspectable::<Health>();` from your plugin. The contract types live in the `renzora` crate behind its `editor` feature (engine built on **Bevy 0.18**).

That's the short version. For per-field attributes, custom widgets, and fully native drawers, see [Inspector Fields](/docs/r1-alpha5/editor-dev/inspector-fields).
