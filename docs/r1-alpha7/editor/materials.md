# Material Editor

The Material Editor is where you decide how a surface *looks* — its color, how shiny or rough it is, whether it glows. Instead of typing code, you connect little boxes called **nodes** together, and the editor turns that into a real, fast material on your mesh.

## Opening the editor

Switch to the **Materials** workspace in the editor, then pick what you want to edit:

- **Click a mesh in the viewport.** Its material loads straight into the graph, ready to tweak.
- **Or double-click a `.material` file** in the asset browser to open it on its own tab.

Your changes save automatically as you work, and the mesh updates live so you can see the result right away. There's also an **Apply** button in the panel toolbar if you want to force a save.

> Materials are saved as `.material` files (plain JSON). When you import a 3D model, every material on it is written out as a `.material` next to the model automatically — so you can open and edit any imported look as a node graph.

## The node graph

You build a material by dragging nodes out of the category menu and **wiring them together**: drag from a node's output dot (on its right edge) into another node's input dot (on its left edge). Anything you leave unconnected just uses the value typed into the node.

![A material node graph: two Sample Texture nodes and a Sample Normal Map node wired by colored cables into the Surface Output node on the right](/assets/previews/material_graph.png)

In the shot above, a color texture feeds the **Base Color** pin, another texture drives **Metallic** and **Roughness**, and a normal map plugs into **Normal** — all flowing into the **Surface Output** node on the right. That output node is the heart of every material.

**Adding nodes:** right-click the graph (or press **`Spacebar`** with the cursor over it) to open a **searchable palette** at the cursor — type to filter, **Enter** picks the first match — and the node spawns where you clicked. The toolbar **Add Node** button opens the same palette. You can also **drag a cable off a pin** and release on empty space: the palette opens and the node you pick is auto-wired to that pin.

**Dropping a texture:** drag an image straight from the **Assets** browser onto the graph and release — a **Sample Texture** node appears under the cursor with that image already bound, ready to wire into a pin. Drop several images at once and you get one node per image, cascaded so they don't stack exactly on top of each other.

**Switching a sample's type:** every 2D texture-sample node carries a small **caret** button in its header. Click it to pick a different sampling mode for the same image — **Sample Texture**, **Sample Normal Map**, **Sample Texture LOD**, or **Sample Texture Grad** — without re-dropping it. The bound image is kept; any wires to pins the new mode doesn't have are dropped. (The rest of the header stays a drag handle for moving the node.) The switcher stays within plain-2D modes; array, 3D, and cubemap samples need a matching texture, so add those from the palette.

**Comments / groups:** select some nodes and press **`C`** to wrap them in a labelled **comment box**. Dragging the box moves every node inside it; drag the corner grip to resize, edit the header to rename, and **✕** deletes the box (keeping its nodes). Comments are visual only and saved in the `.material` file.

### The Surface Output node

The Surface Output node is what shows up on your mesh. The pins you'll reach for most often are:

- **Base Color** — the main color or texture.
- **Metallic** — usually `0` for non-metal, `1` for metal.
- **Roughness** — `0` is mirror-smooth, `1` is fully matte.
- **Normal** — plug in a normal map for surface bumps and detail.
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

The top row shows which `.material` the object uses: a preview thumbnail, the
material's name, and three buttons — **browse** the project's materials,
**open** this one in the Material Editor, and **remove** it. Click the name to
pick from a searchable list of every material in the project, or drag a
`.material` file from the **Assets** browser onto the row.

### Texture slots

Below that is one row per PBR channel — **Base Color**, **Normal**,
**Roughness**, **Metallic**, **Ambient Occlusion** and **Emissive**.

**Drag an image onto a row** (or click the row to browse for one) and it is
wired into the material graph for you: the sampler node is created, connected to
the matching pin on the Surface Output node, and the material is recompiled and
saved. The mesh updates immediately. The **✕** on a filled row unwires that
channel again and tidies away the node it was using.

If the object has no material yet, the first drop creates one for you — a new
`.material` named after the object, in the project's `materials/` folder.

**Dropping a whole texture set:** drag several images at once onto the *material
slot* at the top and each one is routed by its filename — `rock_normal.png` goes
to Normal, `rock_rough.png` to Roughness, `rock_basecolor.png` to Base Color, and
so on. Packed maps are understood too: an `ORM`/`ARM` file fills occlusion,
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

- **Shape selector** — swap between sphere, cube, cylinder, torus, and plane.
- **Auto-rotate** — spin the shape slowly so you can judge it from every angle.
- **Background** — flip the flat backdrop between dark and light.
- **Backdrop** — a switch that turns on a built-in **HDRI environment**. With it on, the preview shows a real outdoor sky behind the shape and lights the material with image-based reflections — the quickest way to see how metal, glass, or glossy surfaces actually catch the light. Turn it off to fall back to the plain dark/light background.

> The HDRI is built into the editor, so the backdrop works in any project with nothing to set up.

## Editing the compiled shader by hand

Saving a material writes two files next to the `.material`: a compiled **`.wgsl`** and a `.wgsl.meta` sidecar describing its textures and parameters. The engine watches your project for changes to those `.wgsl` files, so if you open one in an external editor and save, the viewport picks it up within about a fifth of a second — no restart, and no need to touch the graph.

This is a one-way door, though: the next time you press **Apply** in the graph editor, the `.wgsl` is regenerated from the nodes and your hand edits are overwritten. Treat it as a way to experiment with shader code quickly, not as a place to keep changes.

> Watching is editor-only — a shipped game reads the compiled `.wgsl` as-is and never watches for changes.

## Tips

- **Keep roughness above ~0.05.** Perfectly smooth surfaces can sparkle with artifacts.
- **UV math needs the full path.** A material that samples a texture straight from the mesh's UVs compiles to the engine's fast path. Wire anything into a sampler's **UV** input — UV Scale for tiling, a panner, a rotator, or plain arithmetic — and it compiles as a custom shader instead. That's what makes tiling work, and it costs a little more per material, which is why the fast path is used whenever it can express the graph exactly.
- **Metallic is usually 0 or 1.** In-between values are rarely realistic.
- **Reuse materials and instances.** Objects sharing a material draw faster, and instances of one master share a single compiled shader.
- **Keep graphs simple when you can.** A plain texture-plus-color material runs on the engine's fast path; heavy procedural nodes are a little more expensive.
