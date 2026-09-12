# Inspector

The Inspector is where you read and change everything about the object you have selected: its position, its colours, its lights, its scripts.

![The Inspector panel showing a selected object with collapsible sections for Name, Transform, Visibility, Directional Light, Volumetric Light and TAA.](/assets/previews/inspector.png)

It always follows your selection. Click an object in the viewport or in the Hierarchy and the panel rebuilds to show it.

Nothing to inspect yet? Open **Add Entity** and drop something into the scene.

![The Add Entity menu with a search box and categories such as General, Lighting and Camera.](/assets/previews/add_entity.png)

## The entity header

The fixed row above the component list describes the object itself rather than any component on it.

| Control | What it does |
|---|---|
| Icon | The glyph this object shows in the Hierarchy. Click to pick one, or **Auto** to let its components decide. |
| ID | The object's unique identifier, and the name scripts use to find it. Spaces and punctuation become `_`, and duplicates are resolved for you. |
| Label colour | The colour of this object's row in the Hierarchy. |
| Eye | Show or hide the object. |
| Lock | Pin the Inspector to this object while you click around elsewhere. |

The icon and the label colour are saved with the scene, so they travel with the file.

Setting the icon by hand is useful when a scene is full of empty objects that all look alike but mean different things: a spawn point, a patrol waypoint, a trigger volume.

## Reading the panel

Each object is made of **components**, and the Inspector shows one collapsible section per component.

A section header carries a **grip** for dragging it up or down, a **caret** to fold it, an **icon** and name, an **on/off toggle** on components that support one, and a **trash** button to remove it.

Scripts and Material have no trash button. They manage their own contents instead.

Transform comes first, then Scripts and Material when present, so the things you reach for most are where you expect them. Everything else follows below.

The top bar holds **Add Component**, a **filter box** that hides everything not matching what you type, and an **expand or collapse all** button.

<!-- screenshot: inspector_add_component.png - the Add Component menu open, grouped by category -->

### Rearranging

Drag the grip on any header. A line shows where the section will land. The sections do not shuffle under your cursor while you drag, and dragging near an edge scrolls the list.

The order is remembered per component type, for you, not per object. Put Material above Transform once and it stays there on every object that has both, in every project. Components you have never dragged keep the neighbour they had.

### Which sections start open

By default, all of them. Change it under **Settings > Interface > Inspector > Default Expand**.

| Option | Starts with |
|---|---|
| All Open | Every section expanded. The default. |
| Essentials Only | Transform and Scripts open, the rest closed. |
| All Closed | Everything collapsed. |

A collapsed section costs nothing, because its rows are not built at all. If a long component list starts costing you frames, **Essentials Only** is the setting to reach for.

This sets the starting state each time the Inspector rebuilds. You can still fold anything by hand.

## Editing fields

Each setting gets the control that fits it.

| Kind | How to edit it |
|---|---|
| Numbers | Drag to scrub, or click to type. Hold `Shift` while dragging to scrub ten times finer. |
| X / Y / Z | Three coloured drag boxes, for position and rotation. |
| Toggles | An on/off switch. |
| Colours | A colour picker. |
| Text | A single-line box. |
| Dropdowns | A fixed list of choices. |
| Asset slots | Drag a file from the Assets panel onto the slot. It only accepts the right types. |

A number with a range fills as it rises, so the field is empty at the minimum and full at the maximum. A column of them reads at a glance without comparing digits.

Every editable field has a small **reset** button to its right, which snaps it back to its default.

Edits apply live. There is no Apply button.

### Keyframing from the Inspector

When an animation clip is open in the Timeline for the selected entity, every animatable field gains an amber **◆** button beside its reset. Clicking it keys that field's current value at the playhead, creating the track first if there is not one.

See [Animation](/docs/r1-alpha8/editor/animation).

## Adding and removing components

**Add Component** opens a list of everything you can add, grouped by category. Type to filter.

A few sections are inherent rather than addable and never appear in that list, because they are always present: Scripts on every entity, 2D Lighting on a 2D camera.

The **trash** button in a header removes a component. The header toggle turns one off without removing it.

Removing the last script also drops the underlying component, so an entity you never scripted carries nothing.

## Material

The **Material** section is the fastest way to dress a mesh.

Its top row is the material reference: a thumbnail, a name picker, and buttons to create a new material, browse, open the Material Editor, or clear it.

Below that is one drop slot per channel: Base Color, Normal, Roughness, Metallic, Ambient Occlusion and Emissive. Drag an image onto a slot and it is wired in and applied straight away. Drag a whole texture set onto the material row and each file is routed to the channel its name suggests.

See [Materials](/docs/r1-alpha8/editor/materials).

## Text and fonts

Any entity with text gets two sections.

**Text Font** picks the font from a dropdown that fills itself from your project's `fonts/` folder plus the built-in faces. Drop a `.ttf` or `.otf` there and it appears. Set the size, and for variable fonts the weight, width, letter spacing and line height.

**Rich Text** builds styled spans: several runs of text on one line, each with its own text and colour. **Add span** appends one. Spans render in order after the base text.

Only the fonts you actually use are packed into an exported game.

## Scripts

Drag a script from the Assets panel onto the **Drop to add script** target, or click the **+** on its right edge to pick from a list of your project's scripts.

<!-- screenshot: script_component.png - the Scripts component with a script attached, showing its header controls -->

Each attached script gets its own collapsible section with a file icon, its name, an enable toggle and a trash button, so an entity carrying several stays tidy.

The file icon is a button. Click it to open that script in the **Code Editor**, adding the panel to your layout if it is not already there.

There is also a per-script **play button**, which runs just that script while you stay in edit mode. It is how you test one behaviour without running the game.

See [Rust Scripts](/docs/r1-alpha8/scripting/rust-scripts).

## Resources

Some state is not attached to any object: the time, the editor's own settings, a plugin's configuration. That is a **resource**, and because there is nothing to select it never appears here.

Open the **Resources** panel instead, from the Add Panel picker. It lists every resource in the running world and edits them the same way this panel edits a component.

## Custom components

Components you write yourself can appear here automatically. See [Custom Inspector Fields](/docs/r1-alpha8/editor-dev/inspector-fields).
