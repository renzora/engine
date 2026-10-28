# Material Editor

A material decides how a surface looks: its colour, how shiny or rough it is, whether it glows. You build one by connecting nodes rather than writing code.

## Dressing an object without the graph

Most of the time you never need the node editor at all. Select an object and open the **Material** section in the Inspector.

<!-- screenshot: material_inspector.png - the Material section in the Inspector with the material slot and texture channel rows -->

### The material slot

The top row shows which material the object uses: a preview square and the material's name with its folder underneath.

Click the field to open a searchable grid of the project's materials, each as a preview tile. Hover a tile to see which folder it is in.

An object with no material shows a **New material** button, which asks for a name and a destination folder, writes the file and binds it.

### Base Color

A built-in shape does not start with a material. It wears the blockout grid tinted by a single colour, which is why a fresh cube is solid red and a sphere solid blue.

That colour is the **Base Color** row directly under the material slot. Click the swatch for a picker, and the object updates as you drag. It is saved with the scene.

The row only appears on built-in shapes with no material. Assign a material and it goes away, because from then on the material owns the surface. Imported models never show it.

Alpha works here, so dropping it below 1 switches the object to blended transparency.

This is the fast way to dress a greybox: spawn the shapes, colour them, and only reach for real materials on the surfaces that need them.

### Texture slots

Once an object has a material, one row per channel appears: **Base Color**, **Normal**, **Roughness**, **Metallic**, **Ambient Occlusion**, **Emissive** and **Displacement**.

Only Base Color shows to start with. A footer beneath counts the rest and unfolds them when clicked.

Drag an image onto a row, or click the row to browse, and it is wired into the material graph for you. The mesh updates immediately.

A filled row carries two buttons:

- The **eye** turns that channel off on the mesh without giving the texture up. Use it to see a mesh without its normal map, or to check what a roughness map is contributing. Muting is stored in the material, so it applies anywhere that material is used.
- The **✕** unwires the channel for real.

### Dropping a whole texture set

Drag several images at once onto the **material slot** and each is routed by its filename. `rock_normal.png` goes to Normal, `rock_rough.png` to Roughness, `rock_basecolor.png` to Base Color, `rock_height.png` to Displacement.

If the object has no material yet, dropping on the slot creates one named after the object with the images already wired in.

### DirectX and OpenGL normal maps

Texture packs often ship both, a `_NormalDX` and a `_NormalGL`. They hold the same data with the green channel inverted, and the engine decodes the OpenGL convention, so reach for the `GL` file.

If you only have the DX one, tick **Flip Green (DirectX)** on the Sample Normal Map node.

Getting this wrong does not look broken so much as subtly off: bumps light as though they were dents, and only along one axis, so it tends to show up when the light moves rather than at first glance.

## The node graph

Switch to the **Materials** workspace, then click a mesh in the viewport to load its material, or double-click a `.material` file in the Assets panel.

![A material node graph: two Sample Texture nodes and a Sample Normal Map node wired by coloured cables into the Surface Output node on the right.](/assets/previews/material_graph.png)

Changes save as you work and the mesh updates live. `Ctrl+S` forces a save.

### Wiring

Drag from a node's output dot on its right edge into another node's input dot on the left. Anything you leave unconnected uses the value typed into the node.

Dots are coloured by pin type, and numeric types interconnect freely. A wire between two different colours draws as a gradient, so you can see where a conversion is happening. Only genuinely incompatible pins refuse to connect.

**Math nodes adapt to their wires.** A Math node starts as a plain float, but wire a four-component value into one input and the whole node follows. The widest wire wins, and unplugging it drops the node back down. Adaptation only flows from what feeds the node, never from what consumes its result.

### Adding nodes

Right-click the graph, or press `Space` with the cursor over it, for a searchable palette at the cursor. Type to filter and press `Enter` to take the first match.

You can also drag a cable off a pin and release on empty space. The palette opens and the node you pick is wired to that pin automatically.

Drag an image straight from the **Assets** panel onto the graph and you get a Sample Texture node with that image already bound.

The palette only offers nodes the material's domain can use. A Sprite material has no Scene Depth, Scene Normal, Scene Color, Motion Vectors or Environment Map, since a sprite is not drawn in a 3D view, and a 3D material has no Sprite Color. A node like that arriving some other way, pasted in or saved by an older version, gets a red header saying why, and wires to or from it are refused.

### Moving nodes

A dragged node snaps its top-left corner to the dots of the background grid. Hold `Alt` while dragging to place it freely.

### Editing values

Every input pin that is not wired shows its editor on the pin's own row, beside its name: a slider field for numbers that fills the rest of the row, X/Y/Z fields for vectors, a colour swatch, a checkbox. Wire a cable in and the editor disappears, because the wire is supplying the value now. A node's inputs and outputs share rows, so nodes stay compact.

The **Material** panel on the left is the same values in list form, with one labelled row per pin. Useful when a node's pins are not obvious, or when you are zoomed out too far to read the node.

### Comments

A comment is a labelled box that groups nodes. Make one with the **Comment** toolbar button, the **Comment** entry at the top of the right-click menu, or `C` over the graph. With nodes selected it wraps them; with nothing selected it adds an empty box (at the cursor for `C` and the right-click menu, in the middle of the view for the button). Click the title to rename it, drag the box to move everything inside it, and use the corner grip to resize it. Comments are visual only and saved with the material. The particle graph works the same way.

## The Surface Output node

This is what shows up on your mesh. The pins you will reach for most:

| Pin | What it does |
|---|---|
| Base Color | The main colour or texture |
| Metallic | `0` for non-metal, `1` for metal |
| Roughness | `0` is mirror-smooth, `1` is fully matte |
| Normal | A normal map, for surface bumps and detail |
| Displacement | A height map, for real parallax |
| Emissive | Makes a surface glow |

A minimal material only needs Base Color. Everything you leave unplugged keeps its default.

There are more advanced pins too: clearcoat for car paint, transmission for glass and water, anisotropy for brushed metal. See the [Material API](/docs/r1-alpha8/api/material).

### Displacement

Plug in a height map and the surface gets real parallax. Bricks occlude their own mortar, and the relief shifts correctly as you move around it.

White is the peak and black the valley, which is how every PBR texture set ships its height map, so it goes straight in with nothing to invert.

**Displacement Scale** sets how deep the effect reads. `0.05` is a good starting point, and much past `0.1` starts to swim at grazing angles.

Two things to know: the mesh needs tangents, which imported models have and primitive shapes do not, and this is a shading trick rather than geometry, so the silhouette does not change. A constant typed into the pin does nothing, because parallax needs a map to march through.

## Node categories

There are around 150 node types. You do not need to learn them all.

| Category | Holds |
|---|---|
| Input | UVs, time, world position, vertex colours |
| Texture | Sample an image, a normal map, or triplanar projection |
| Math and Vector | Add, multiply, blend, and the other building blocks |
| Color | Palettes, fresnel rim glow, hue shifts, blends |
| Procedural | Noise, checkerboard, brick and other patterns with no texture needed |
| Animation | Scroll UVs, wind sway, flipbook frames |

The complete catalogue is in the [Material Node Reference](/docs/r1-alpha8/api/material-node-reference).

## Domains

Every material has a domain that decides what it is for. Pick this when you create the graph.

| Domain | Use it for |
|---|---|
| Surface | Props, walls, characters. Standard PBR, and the default. |
| Terrain Layer | A paintable layer on terrain |
| Vegetation | Surfaces that sway in the wind |
| Unlit | Flat colour with no lighting |
| Sprite | A 2D sprite. Made with **Add ▸ Sprite Material**; see [Sprite materials](/docs/r1-alpha8/api/material#sprite-materials) |

The domain is stored in the `.material` file, so one file type covers them all. In the Assets panel a Sprite material's thumbnail is the sample sprite wearing it, tagged **2D** in the corner, where a 3D material shows a sphere.

Two more settings live on the material rather than on a node. They are in the **Material** panel's Material section, under Name and Domain, and the preview shows a change straight away.

**Alpha Mode** says how the output's Alpha is used:

| Mode | For |
|---|---|
| Opaque | Solid surfaces; alpha is ignored. The default |
| Mask | Cut-outs: each pixel is drawn or not, at the **Cutoff** (shown for this mode). Leaves, fences, hard-edged decals |
| Blend | See-through surfaces that fade as a whole: smoke, fades, soft decals |
| Glass | See-through surfaces that still reflect at full strength: windows, lenses |
| Dithered | Cut-outs with soft edges: hair, fur, wispy foliage |

**Dithered** is for hair cards. Their textures are mostly clear with thin, partly transparent strands, and under Blend they draw as a pale see-through veil: blended layers are sorted per mesh, never build up into a solid mass, and let what is behind show through. Dithered first sharpens alpha: below **Clear** a pixel is never drawn, above **Solid** it always is, and between the two it ramps. Each pixel is then drawn whole or not at all against noise that changes every frame, which TAA averages back into soft strand edges. Being a cut-out, it writes depth and casts shadows like an opaque surface. Lower Solid for thicker hair, raise Clear to thin out a fuzzy fringe. It needs TAA on the camera; without it the fringe shows as grain. In the file it is `"alpha_mode": { "Dithered": { "low": 0.1, "high": 0.5 } }`.

The difference between Blend and Glass is reflections. Blend scales everything the surface draws by its alpha, reflections included, so clear glass at 25% reflects a quarter of what it should. Glass uses alpha only for how much of what is behind is covered, and adds what the surface draws, its reflections above all, at full strength, the way real glass reflects however clear it is. Give glass a dark base colour, no metal and low roughness; the base colour is added too, as a haze. In the file Glass is `"alpha_mode": "Premultiplied"`.

**Double Sided** renders back faces too, which thin surfaces like paper, foliage and glass seen from inside need.

## Previewing

The **Material Preview** panel shows the material on a test shape, lit by an environment map, so you can judge it without hunting for a surface in your scene.

A Sprite material is shown instead on a flat sample sprite over a checkerboard, so changes to colour, transparency and UVs are all visible. It follows every edit live, the mouse wheel zooms, and the moon/sun button switches the checkerboard between dark and light.

<!-- screenshot: material_preview.png - the Material Preview panel showing a material on a sphere -->

## Imported models

When you import a model, every material on it is written out as a `.material` beside the model. Those extracted graphs are what the engine renders, in the editor and in an exported game alike.

The model file itself holds no reference to them. The link is rebuilt each time the model spawns, by matching each mesh's material name to the `.material` of that name. **Renaming a `.material` file breaks that link** and the mesh falls back to whatever the source file said. Edit the graph, not the filename.

This matters most for models exported from the older specular-glossiness workflow, common on asset sites. The engine converts those on import, and the converted result lives only in the `.material`. Re-import a model if its look ever regresses to flat white.

A spec-gloss model imported before 2026-10-19 has its glossiness routed through a maths node in each graph, which renders correctly but costs a generated shader per material. Re-importing it bakes the glossiness into a real roughness texture instead, so its materials become plain StandardMaterials, and surfaces whose specular is black (brick, plaster) come out matte as authored.

## Supported image formats

`png`, `jpg`, `jpeg`, `bmp`, `tga`, `webp`, `hdr`, `exr`, `ktx2` and `dds`.
