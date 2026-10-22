# Inspector

The Inspector is where you read and change everything about the object you have selected: its position, its colours, its lights, its scripts.

![The Inspector panel showing a selected object with collapsible sections for Name, Transform, Visibility, Directional Light, Volumetric Light and TAA.](/assets/previews/inspector.png)

It always follows your selection. Click an object in the viewport or in the Hierarchy and the panel rebuilds to show it.

Nothing to inspect yet? Open **Add Entity** and drop something into the scene.

![The Add Entity menu with a search box and categories such as General, Lighting and Camera.](/assets/previews/add_entity.png)

## The entity header

The fixed row at the top of the panel describes the object itself rather than any component on it.

| Control | What it does |
|---|---|
| Icon tile | The glyph this object shows in the Hierarchy, on a wash of its label colour (the accent colour when it has none). Click it to change either. |
| Name | The object's unique identifier, and the name scripts use to find it, in a text field that is always there. Click into it and type; **Enter** or clicking away saves, **Escape** puts the old name back. Spaces and punctuation become `_`, and duplicates are resolved for you. |
| Kind and source | The line under the name: what the object is (Mesh, Camera, Light, Model, Empty) and, for a model or any part of one, the model file it came from. |
| Eye | Show or hide the object. On the name field's line, beside it. |
| Lock | Pin the Inspector to this object while you click around elsewhere. It turns red while the Inspector is pinned, since your selection is then being ignored. |

Clicking the icon tile opens one popover for both settings:

- **Colour**: a row of swatches. The first sets no colour, the rest are preset colours, and the last (the rainbow) opens a full colour picker underneath. The current colour has a ring.
- **Icon**: a grid of icons with a search box under it, and **Auto** to let the object's components decide again. The current icon is highlighted.

The icon and the label colour are saved with the scene, so they travel with the file.

Setting the icon by hand is useful when a scene is full of empty objects that all look alike but mean different things: a spawn point, a patrol waypoint, a trigger volume.

## With nothing selected

Click on empty space and the Inspector shows the scene instead of an empty panel. The header names the active scene, with how many entities it holds and whether it is the boot scene. Under it:

- **World**: the world environment's own cards (sky, atmosphere, fog, sun and the rest of its Environment and Lighting components), editable right there without finding the `world_environment` entity in the Hierarchy. A scene without one shows a button to add it.
- **Game**: the scene the game boots into and the camera it starts on, with **Set as Boot Scene** when the scene you are in is saved and is not the boot scene already.
- **Counts**: entities, meshes and triangles in the scene, refreshed every second.

Select anything to go back to its own cards.

## Pages

Under the header runs a strip of **pages**. Each page holds one kind of component, so a busy object is not one long list:

| Page | Holds |
|---|---|
| Object | Transform, and anything describing the object itself |
| Rendering | Meshes, materials, decals, LODs, text, splats, sprites |
| Lighting | Lights, light probes, ray-traced GI |
| Camera | The camera and its image effects: bloom, tonemapping, anti-aliasing, depth of field, motion blur |
| Environment | Sky, atmosphere, clouds, fog, water, wind, terrain |
| Post Process | SSAO, SSR and shader post effects |
| Physics | Bodies, colliders, joints, ragdolls |
| Animation | Animators and clips |
| Scripting | Scripts and blueprints |
| Audio | Players and listeners |
| Navigation | Navmesh volumes, obstacles and agents |
| Networking | Replication |
| UI | Game UI nodes |
| Particles | Particle effects |

A page appears only when the object has something on it, apart from Object, which is always there. Each button is an icon in its page's colour; the page you are on also shows its name and how many components it holds, and hovering any other tells you the same. On a narrow panel the strip scrolls sideways with the mouse wheel.

The first button, **All**, shows every component card on one page. It is where the Inspector starts.

The page you pick stays picked as you select other objects. If the next object has no such page, the Inspector shows All, and goes back to your page on the next object that has one.

At the right end of the strip are two pages about the object rather than its components:

- **Info**: the object's entity id and every component it carries, including the ones with no card.
- **Relations**: its parent and its children.

## Reading the panel

Each object is made of **components**, and a page shows one collapsible section per component.

Each section is a **card**: a panel with a faint dark edge whose coloured header band names the component, with its fields underneath. An action button, such as Reset Temporal History, spans the whole row rather than sitting beside a label. A folded section shrinks to just its header. Opening one that runs past the bottom of the panel scrolls down just far enough to show it, keeping its header in view.

A section header carries a **grip** for dragging it up or down, a **caret** to fold it, an **icon** and name, an **on/off toggle** on components that support one, and a **trash** button to remove it.

Scripts and Material have no trash button. They manage their own contents instead.

Transform leads the Object page and Material leads the Rendering page, so the things you reach for most are where you expect them. Everything else follows below.

The toolbar above the cards holds **+ Add** (Add Component), a **filter box** that hides every component not matching what you type, and an **expand or collapse all** button. The filter searches every page at once: while it has text in it, matching cards from all pages show, and pages with no match fade on the strip.

<!-- screenshot: inspector_add_component.png - the Add Component menu open, grouped by category -->

### Rearranging

Drag the grip on any header. A line shows where the section will land. The sections do not shuffle under your cursor while you drag, and dragging near an edge scrolls the list. Dragging rearranges the page you are on; the other pages keep their own order.

The order is remembered per component type, for you, not per object. Put Material above Mesh once and it stays there on every object that has both, in every project. Components you have never dragged keep the neighbour they had.

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

**+ Add** opens a grid of everything you can add, three to a row: each component's icon in its page's colour beside its name, grouped under the same pages as the strip. It opens scrolled to the page you are looking at, with every other page above and below it; clicking a page in the sidebar scrolls to it, and typing searches all of them. Press Enter to add the first match. Once a component is added, the Inspector turns to its page.

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

Drag a script from the Assets panel onto the **Drop to add script** target, which outlines itself while a script is over it. Or click the **+** in the Scripts header: it opens a picker of your project's scripts with a search field at the top, so typing part of a name narrows the list. Scripts outside the plain `scripts` folder show their folder beside the name. Click one to attach it; Escape, a click outside or the X closes the picker without adding anything.

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
