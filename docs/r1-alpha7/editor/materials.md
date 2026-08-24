# Material Editor

The Material Editor is where you decide how a surface *looks* — its color, how shiny or rough it is, whether it glows. Instead of typing code, you connect little boxes called **nodes** together, and the editor turns that into a real, fast material on your mesh.

## Opening the editor

Switch to the **Materials** workspace in the editor, then pick what you want to edit:

- **Click a mesh in the viewport.** Its material loads straight into the graph, ready to tweak.
- **Or double-click a `.material` file** in the asset browser to open it on its own tab.

Your changes save automatically as you work, and the mesh updates live so you can see the result right away. Press **Ctrl+S** (or the **Apply** button in the panel toolbar) to force a save.

> Materials are saved as `.material` files (plain JSON). When you import a 3D model, every material on it is written out as a `.material` next to the model automatically — so you can open and edit any imported look as a node graph.

## The node graph

You build a material by dragging nodes out of the category menu and **wiring them together**: drag from a node's output dot (on its right edge) into another node's input dot (on its left edge). Anything you leave unconnected just uses the value typed into the node.

Wire dots are coloured by pin type, and any numeric types interconnect freely — a `Vec2` can feed a `Color` pin, a `Float` can feed a `Vec3`, and so on (the compiler inserts the right conversion). A wire between two *different* pin colours draws as a gradient from the source colour to the target colour, so you can see at a glance where a conversion is happening. Only genuinely incompatible pins (bool, texture, sampler) refuse the connection outright.

![A material node graph: two Sample Texture nodes and a Sample Normal Map node wired by colored cables into the Surface Output node on the right](/assets/previews/material_graph.png)

In the shot above, a color texture feeds the **Base Color** pin, another texture drives **Metallic** and **Roughness**, and a normal map plugs into **Normal** — all flowing into the **Surface Output** node on the right. That output node is the heart of every material.

**Adding nodes:** right-click the graph (or press **`Spacebar`** with the cursor over it) to open a **searchable palette** at the cursor — type to filter, **Enter** picks the first match — and the node spawns where you clicked. The toolbar **Add Node** button opens the same palette. You can also **drag a cable off a pin** and release on empty space: the palette opens and the node you pick is auto-wired to that pin.

**Editing values on the node:** every input pin that *isn't* wired to something shows its editor right under the pin, on the node itself — a scrub field for numbers, X/Y/Z/W fields for vectors, a colour swatch, a checkbox, a text box for names. Drag across a number to scrub it, or click it to type. Wire a cable into that pin and the editor disappears (the wire is supplying the value now); unplug the cable and it comes back.

**The Material panel** (left, under the preview) is the same values in list form. Click a node and the panel shows its name, what it does, and one labelled row per input pin — handy when a node's pins aren't obvious from their dots alone, or when you're zoomed out too far to read the node itself. Pins that already have a cable read *(connected)*. It's the same value either way, so editing in the panel updates the node and vice versa. The top of the panel also holds the material's **Name** and its **Domain**.

**The output node included.** Its unwired pins each show a field too, and the ones with a fixed range — Metallic, Roughness, Ambient Occlusion (AO), Alpha, the transmission and clearcoat amounts, IOR — carry a slider under the number, so you can dial a whole material in on the output node without wiring a Constant into every pin. A field you haven't touched is showing the pin's default, not an override: leave it alone and the pin still falls through to the engine's own defaults, exactly as a bare pin always did. Scrub it once and it becomes a value the material carries. **Normal** is the one pin left bare — a direction has no sensible number to type, so it stays wire-only.

**Dropping a texture:** drag an image straight from the **Assets** browser onto the graph and release — a **Sample Texture** node appears under the cursor with that image already bound, ready to wire into a pin. Drop several images at once and you get one node per image, cascaded so they don't stack exactly on top of each other.

**DirectX vs OpenGL normal maps:** texture packs often ship both — a
`_NormalDX` and a `_NormalGL`. They hold the same data with the green channel
inverted, and the engine decodes the **OpenGL** convention, so reach for the
`GL` file. If you only have the DX one, tick **Flip Green (DirectX)** on the
Sample Normal Map node. Getting this wrong doesn't look broken so much as
subtly off: bumps light as though they were dents, and only along one axis, so
it tends to show up when the light moves rather than at first glance.

**Switching a sample's type:** every 2D texture-sample node carries a small **caret** button in its header. Click it to pick a different sampling mode for the same image — **Sample Texture**, **Sample Normal Map**, **Sample Texture LOD**, or **Sample Texture Grad** — without re-dropping it. The bound image is kept; any wires to pins the new mode doesn't have are dropped. (The rest of the header stays a drag handle for moving the node.) The switcher stays within plain-2D modes; array, 3D, and cubemap samples need a matching texture, so add those from the palette.

**Comments / groups:** select some nodes and press **`C`** to wrap them in a labelled **comment box**. Dragging the box moves every node inside it; drag the corner grip to resize, edit the header to rename, and **✕** deletes the box (keeping its nodes). Comments are visual only and saved in the `.material` file.

### The Surface Output node

The Surface Output node is what shows up on your mesh. The pins you'll reach for most often are:

- **Base Color** — the main color or texture.
- **Metallic** — usually `0` for non-metal, `1` for metal.
- **Roughness** — `0` is mirror-smooth, `1` is fully matte.
- **Normal** — plug in a normal map for surface bumps and detail.
- **Displacement (Height)** — plug in a height map and the surface gets real
  parallax: bricks occlude their own mortar, and the relief shifts correctly as
  you move around it. White is the peak, black the valley — the way every PBR
  texture set ships its Displacement or Height map, so it goes straight in with
  nothing to invert. **Displacement Scale** next to it sets how deep the effect
  reads; `0.05` is a good starting point and much past `0.1` starts to swim at
  grazing angles. Two things to know: the mesh needs **tangents** (imported
  models have them; primitive shapes don't), and this is a shading trick, not
  geometry — the silhouette of the object doesn't change. A constant typed into
  the pin does nothing; parallax needs a map to march through.
- **Emissive** — makes a surface glow (great for screens, lava, neon).

A minimal material only needs **Base Color** — everything you leave unplugged just keeps its sensible default. There are more advanced pins too (clearcoat for car paint, transmission for glass and water, anisotropy for brushed metal), all listed in the [Material API reference](/docs/r1-alpha7/api/material).

### Nodes you can wire in

There are around 150 node types, grouped into friendly categories. You don't need to learn them all — here's the shape of what's available:

- **Input** — UVs, time, world position, vertex colors.
- **Texture** — sample an image, a normal map, or do triplanar projection.
- **Math & Vector** — add, multiply, blend (`lerp`), and other building blocks.
- **Color** — palettes, fresnel rim glow, hue shifts, blends.
- **Procedural** — noise, checkerboard, brick, and other patterns with no texture needed.
- **Animation** — scroll UVs, wind sway, flipbook frames.

For the complete catalog — every node, with its inputs, outputs, and what it does — see the [Material Node Reference](/docs/r1-alpha7/api/material-node-reference).

## Material types (domains)

Every material has a **domain** that decides what it's for. Pick this when you create a new graph:

| Domain | Use it for |
|--------|-----------|
| **Surface** (default) | Normal props, walls, characters — standard PBR |
| **Terrain Layer** | A paintable layer on terrain |
| **Vegetation** | Surfaces that sway in the wind (grass, leaves) |
| **Unlit** | Flat color with no lighting (UI bits, effects) |

Two more switches live on the material itself, not on a node:

- **Alpha mode** — `Opaque` (default), `Mask` for cut-out edges like leaves and fences, or `Blend` for see-through glass and smoke.
- **Double sided** — render the back faces too, handy for thin surfaces like paper or foliage.

## Putting a material on an object

Select the object and open the **Inspector**. The **Material** card is the quick
way in — most of the time you never need the node graph at all.

### The material slot

The top of the drawer shows which `.material` the object uses: a preview square,
and next to it a field naming the material with its folder underneath.

**Click the field** to slide the picker open — a searchable grid of the
project's materials, each shown as a preview tile with its name under it. It
opens *in place* rather than covering the drawer; the field takes on an accent
tint and the caret flips while it's showing. The texture slots fold away while
it's open — they describe the material you're in the middle of replacing — and
come back when you choose or close. Hover a tile to see which folder it's in;
click one to bind that
material and slide the picker shut again. Click the field a second time to close
it without choosing. The grid re-flows with the width of the Inspector, so a
wider dock shows more per row.

The picker shows **at most twelve** materials at a time and doesn't scroll — the
Inspector already does, and a second scrollbar inside the first was more
confusing than useful. Type in the search box to narrow the list; when there are
more matches than fit, a line under the grid tells you how many.

You can also **drag a `.material` file from the Assets browser onto the slot**,
which does the same thing.

Inside the field, left of the caret, are two buttons: the **pencil** opens this
material in the Material Editor, and **×** removes it from the object.

An object with no material yet shows a **New material** button under the field
instead. It asks where the file should go — type a name and pick a destination
folder from the project tree — then writes the `.material` there and binds it.
It does *not* jump you to the Material Editor: the texture slots that appear
underneath are where a new material usually gets filled in, and the pencil is
there when you do want the node graph.

> A preview that shows a plain sphere glyph rather than a rendered ball just
> means the thumbnail hasn't been rendered yet — they're captured in the
> background the first time a material comes into view and cached in the project
> from then on.

### Texture slots

Once the object has a material, one row per PBR channel appears below the
field — **Base Color**, **Normal**, **Roughness**, **Metallic**, **Ambient
Occlusion**, **Emissive** and **Displacement**. There's no heading over them;
each row names its own channel.

**Drag an image onto a row** (or click the row to browse for one) and it is
wired into the material graph for you: the sampler node is created, connected to
the matching pin on the Surface Output node, and the material is recompiled and
saved. The mesh updates immediately.

A filled row carries two buttons, and the difference between them matters:

- The **eye** turns that channel off *on the mesh* without giving the texture
  up. The row stays filled — dimmed, with the thumbnail faded — and clicking the
  eye again puts the texture straight back. Use it to see the mesh without its
  normal map, or to check what a roughness map is actually contributing.
- The **✕** unwires the channel for real and tidies away the node it was using.
  There's nothing to click to get it back.

Muting is stored in the `.material`, so it survives saving and reloading and
applies anywhere that material is used — it's a property of the material, not a
per-object view setting.

If the object has no material yet there are no channel rows to aim at, but you
don't need them: drop the image on the **material slot** at the top and one is
created for you — a new `.material` named after the object, in the project's
`materials/` folder — with the image already wired in.

**Dropping a whole texture set:** drag several images at once onto the *material
slot* at the top and each one is routed by its filename — `rock_normal.png` goes
to Normal, `rock_rough.png` to Roughness, `rock_basecolor.png` to Base Color,
`rock_displacement.png` (or `_height`) to Displacement, and so on. One
exception worth knowing: `_bump` is routed to **Normal**, because enough packs
ship a `_Bump` that is really a normal map — if yours is a true height map, drop
it on the Displacement row by hand. Packed maps are understood too: an `ORM`/`ARM` file fills occlusion,
roughness *and* metallic from a single sampler (which is also the fastest way for
the engine to render it), and a `metallicRoughness` file fills those two. A
single image whose name says nothing in particular is treated as base color; in a
multi-file drop, files that don't name a channel are skipped, so drop those onto
the row you want by hand.

Base color also carries opacity: its alpha channel is wired to the material's
Alpha pin automatically, so a cut-out texture works with **Alpha mode: Mask**
without a trip to the graph.

These rows are a *view* of the graph, not a separate list. A texture you wire by
hand in the Material Editor shows up here, and a channel driven by something more
involved than a plain sampler — noise, math, a blend — shows as empty here and is
left alone. Nothing you do in the graph can be silently overwritten from the
inspector.

To swap an object to a different material later, just change the reference at the
top — there's no runtime "swap material" command in scripts.

> A **material instance** shows its master's **Overrides** instead of texture
> slots. The graph belongs to the master, so editing it from one instance would
> change every other instance too; open the master to change its textures.

## Reusing one look in many flavors (instances)

Often you want lots of materials that share the same setup but differ in a couple of values — say the same wood shader in five different stains. That's what **material instances** are for.

You author named **Parameter** nodes (like `BaseColor` or `Roughness`) on a master material, then create instances that only override those named values. Every instance reuses the master's compiled shader, which keeps things fast.

```json
{
  "master": "models/Wood/materials/Wood.material",
  "overrides": {
    "BaseColor": { "Color": [0.45, 0.22, 0.10, 1.0] },
    "Roughness": { "Float": 0.85 }
  }
}
```

You'll usually set these up visually in the editor rather than by hand. See the [Material API reference](/docs/r1-alpha7/api/material) for the full instance and `.material` file format.

## Textures: which image files work

Texture nodes point at an image on disk. Stick to these formats so they load correctly in your game:

- **Loads at runtime:** PNG, JPEG, HDR.
- **Don't ship these as textures:** BMP, TGA, WebP, KTX2, DDS, and especially **EXR** — they won't decode in the running game.

## Changing color from a script

If you just need to tint a material at runtime, Lua can recolor the **base color** of the object the script is attached to:

```lua
function on_update()
    -- RGBA in 0.0-1.0; alpha is optional (defaults to 1.0)
    set_material_color(1.0, 0.0, 0.0, 1.0)
end
```

> `set_material_color` is **Lua-only** and only changes base color on the script's own entity. For anything richer — animated patterns, glows that react to gameplay — build it in the node graph with `Parameter` nodes and material instances. See the [Scripting overview](/docs/r1-alpha7/scripting/overview) for what scripts can do.

## Previewing your material

The **Preview** panel renders your material on a test shape with an orbit camera — **drag** to rotate, **right-drag** to pan, **scroll** to zoom. Its toolbar gives you:

- **Shape selector** — swap between sphere, cube, cylinder, torus, and plane. When you reached the editor by selecting a mesh (in the hierarchy or the viewport), the list gains a **Selected Mesh** entry at the top that previews the material on that object's own geometry instead of a primitive — the honest test for anything authored around a specific model's UVs, so it's what the preview picks by default. The mesh is scaled and recentred to fit the same framing as the primitives, so switching between them doesn't move the camera. Choosing a primitive sticks while you keep editing that object; selecting a different one starts you back on its own mesh. The entry disappears entirely when there's no mesh behind the material — a `.material` opened from the asset browser with nothing selected in the scene — and the preview falls back to the sphere.
- **Auto-rotate** — spin the shape slowly so you can judge it from every angle.
- **Background** — flip the flat backdrop between dark and light.
- **Backdrop** — a switch that turns on a built-in **HDRI environment**. With it on, the preview shows a real outdoor sky behind the shape and lights the material with image-based reflections — the quickest way to see how metal, glass, or glossy surfaces actually catch the light. Turn it off to fall back to the plain dark/light background.

> The HDRI is built into the editor, so the backdrop works in any project with nothing to set up.

The preview renders anti-aliased, with SMAA on top of a render target larger than the panel displays — so silhouettes stay clean and a normal map's specular doesn't sparkle at you while you're trying to judge it. There's nothing to switch on; it's how the panel always draws.

## What a saved material contains

A `.material` is one file holding two things: the node graph you author, and the shader that graph compiles to. Pressing **Apply** recompiles the shader and stores it back inside the same file. There are no loose `.wgsl` or `.wgsl.meta` files beside it to keep track of, move together, or accidentally leave behind.

The compiled shader isn't meant to be edited. A graph and a hand-edited copy of its output can disagree, and once they do there's no way to tell which half is right — so the graph stays the single source of truth, and pressing Apply always regenerates from the nodes.

If you want to write shader code directly, that's a **standalone shader**, not a material: create a `.wgsl` or `.shader` asset and point a mesh at it. Those are yours to edit freely, in the built-in Shader Editor or any external editor — the engine watches your project and the viewport picks up a save within about a fifth of a second, with no restart. Watching is editor-only; a shipped game reads the shader as-is.

> Materials written by an older version — the ones with a `.wgsl` and `.wgsl.meta` next to them — still load exactly as before. The next time you save one, its shader moves inside the `.material` and the two stale files are removed for you.

Only a **write** counts. Opening a `.wgsl`, reading it, or touching its timestamp does not reload the material. The editor itself has the file open, so a reload triggered by a read re-triggers itself.

## When a material fails to compile

A graph can be well-formed and still produce WGSL the shader compiler rejects — most easily through a **Custom Code** node, whose contents nothing checks until the compiler sees them. The material then falls back to a plain surface in the viewport, and the compiler's errors show up in three places:

- **On the node itself.** The offending node gets a red warning icon in its title bar. Hover the node for the compiler's message.
- **The Problems panel**, as a row per distinct error, listed against the `.material` file it came from. Click a row to jump to that file if you have it open.
- **The console**, once per compile — not once per frame, so a broken material does not flood the log.

All three clear on their own as soon as the material compiles again. There is nothing to dismiss and no restart involved. Fix the graph, press **Apply**, and the row and the icon disappear.

Some errors appear only under certain conditions — a mesh with vertex colors, or a camera with an environment map. Those rows carry a `[defines: …]` prefix naming the configurations that fail, because the configuration is part of what is wrong. An error with no prefix fails everywhere.

## Tips

- **Keep roughness above ~0.05.** Perfectly smooth surfaces can sparkle with artifacts.
- **UV math needs the full path.** A material that samples a texture straight from the mesh's UVs compiles to the engine's fast path. Wire anything into a sampler's **UV** input — UV Scale for tiling, a panner, a rotator, or plain arithmetic — and it compiles as a custom shader instead. That's what makes tiling work, and it costs a little more per material, which is why the fast path is used whenever it can express the graph exactly.
- **Metallic is usually 0 or 1.** In-between values are rarely realistic.
- **Reuse materials and instances.** Objects sharing a material draw faster, and instances of one master share a single compiled shader.
- **Keep graphs simple when you can.** A plain texture-plus-color material runs on the engine's fast path; heavy procedural nodes are a little more expensive.
