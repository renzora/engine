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

### The Surface Output node

The Surface Output node is what shows up on your mesh. The pins you'll reach for most often are:

- **Base Color** — the main color or texture.
- **Metallic** — usually `0` for non-metal, `1` for metal.
- **Roughness** — `0` is mirror-smooth, `1` is fully matte.
- **Normal** — plug in a normal map for surface bumps and detail.
- **Emissive** — makes a surface glow (great for screens, lava, neon).

A minimal material only needs **Base Color** — everything you leave unplugged just keeps its sensible default. There are more advanced pins too (clearcoat for car paint, transmission for glass and water, anisotropy for brushed metal), all listed in the [Material API reference](/docs/r1-alpha5/api/material).

### Nodes you can wire in

There are around 150 node types, grouped into friendly categories. You don't need to learn them all — here's the shape of what's available:

- **Input** — UVs, time, world position, vertex colors.
- **Texture** — sample an image, a normal map, or do triplanar projection.
- **Math & Vector** — add, multiply, blend (`lerp`), and other building blocks.
- **Color** — palettes, fresnel rim glow, hue shifts, blends.
- **Procedural** — noise, checkerboard, brick, and other patterns with no texture needed.
- **Animation** — scroll UVs, wind sway, flipbook frames.

For the complete node catalog, see the [Material API reference](/docs/r1-alpha5/api/material).

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

Select the object, open the **Inspector**, and use the **Material** card to point it at a `.material` file. To swap an object to a different material later, just change that reference — there's no runtime "swap material" command in scripts.

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

You'll usually set these up visually in the editor rather than by hand. See the [Material API reference](/docs/r1-alpha5/api/material) for the full instance and `.material` file format.

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

> `set_material_color` is **Lua-only** and only changes base color on the script's own entity. For anything richer — animated patterns, glows that react to gameplay — build it in the node graph with `Parameter` nodes and material instances. See the [Scripting overview](/docs/r1-alpha5/scripting/overview) for what scripts can do.

## Tips

- **Keep roughness above ~0.05.** Perfectly smooth surfaces can sparkle with artifacts.
- **Metallic is usually 0 or 1.** In-between values are rarely realistic.
- **Reuse materials and instances.** Objects sharing a material draw faster, and instances of one master share a single compiled shader.
- **Keep graphs simple when you can.** A plain texture-plus-color material runs on the engine's fast path; heavy procedural nodes are a little more expensive.
