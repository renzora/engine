# Particle System (`.vfx`)

Renzora has its own particle system for **2D and 3D** effects: fire, smoke,
sparks, confetti, magic. An effect is a `.vfx` file, and what each emitter does
is described by a small **node graph**: where particles appear, how fast they
move, what colour they are and how that changes over their life.

It runs on the CPU, so it works on every platform Renzora exports to, including
the web on WebGL2, and large emitters run on the GPU instead wherever the device
has compute shaders: the same graph, a hundred thousand particles and more (see
[GPU backend](#gpu-backend)). Effects play in the editor viewport and in the
shipped game alike, and every effect is deterministic: the same file and seed
always produce the same particles.

> **The only particle system.** It replaced the older GPU one built on
> bevy_hanabi, which has been removed from the engine along with its editor.
> Effects made for that one (`.particle` files, and scenes that use them) keep
> working: they play through this system, and the editor converts them to
> `.vfx` on its own when a project opens. See
> [Old .particle effects](#old-particle-effects).

## Quick start

1. **Assets panel > New (or right-click) > Particle Effect.** Pick a template;
   the new `.vfx` opens in the [effect editor](#the-effect-editor).
2. Drag the `.vfx` from the Assets panel into the viewport to place it, or use
   **Add Entity > Effects > Particle System** (which plays the built-in
   campfire) and drop the file on its **Effect** slot.
3. Select the entity. The **Particle System** section of the inspector has:

| Field | What it does |
|---|---|
| **Effect** | The `.vfx` file. Drop one from the Assets panel onto the slot. Dropping a `.particle` converts it to a `.vfx` beside it and assigns that ([details](#old-particle-effects)). |
| **Edit Effect** | Opens the file in the effect editor. On a built-in effect it opens the template gallery, so you edit a copy in your project. |
| **Play / Pause / Stop / Restart** | Playback. Stop clears every particle; Pause freezes them where they are. |
| **Time Scale** | Simulation speed. `0.5` is slow motion, `0` freezes. |
| **When Finished** | What a non-looping effect does once it is over: **Keep** (nothing), **Deactivate** (stop), or **Despawn** the entity. |

The header toggle is the same as Play and Stop.

In the viewport every particle system shows a small orange sparkle at its
origin, which stays the same size on screen; click it to select the effect.
Selected, it also draws where each emitter spawns: the sphere, box, cone (with
its spread), circle or rectangle from the graph's shape nodes. The selection
box ignores the particles themselves, so it no longer jumps as they move.

### Built-in effects

These are compiled into the engine, so they work in any project and any export
without copying files:

| Effect | Path | |
|---|---|---|
| Campfire | `embedded://renzora_particles/presets/campfire.vfx` | 3D, looping, the default |
| Realistic Fire | `embedded://renzora_particles/presets/realistic_fire.vfx` | 3D, looping, a layered log fire that reacts to the wind ([below](#realistic-fire)) |
| Sparks Burst | `embedded://renzora_particles/presets/sparks.vfx` | 3D, plays once |
| Smoke | `embedded://renzora_particles/presets/smoke.vfx` | 3D, looping |
| Confetti | `embedded://renzora_particles/presets/confetti.vfx` | 2D, a burst every 3 s |
| Magic Sparkles | `embedded://renzora_particles/presets/sparkles.vfx` | 2D, looping |

Type a path into the Effect field to switch. To make your own, start from one
of them in the template gallery, which copies it into your project.

## The effect editor

Double-click a `.vfx` in the Assets panel (or press **Edit Effect** in the
inspector) and the **Particles** workspace opens on it, with three panels:

- **Effect Preview** on the left: the effect playing live, with an orbit
  camera over a ground for 3D effects and a pixel-unit 2D camera for 2D ones.
  Left-drag orbits, right-drag pans, the wheel zooms.
- **Emitter Settings** under it: the effect's name, dimension, duration and
  looping, and the selected emitter's capacity, [backend](#gpu-backend) (and
  what it is running on), space, seed, shading, the shape (quads or a
  [mesh](#mesh-particles), with its model and orientation), the
  [project material](#project-materials) any 3D particle may wear,
  texture, sprite sheet (and frame blending), blend (and the alpha cutoff),
  [Cast Shadows](#shadows-the-prepass-and-motion-vectors), the
  [render method](#deferred-renderer), [lighting](#lit-particles), facing (and stretch, or the ribbon's trail and
  shape), [size in pixels](#screen-space-size), erosion, soft distance and
  [draw order](#draw-order), the inherited velocity of a `World` emitter,
  its [light](#light) and its [sub-emitters](#sub-emitters).
- **Effect Graph** on the right: the selected emitter's node graph.

Opening a `.vfx` puts these three panels in front. Whatever is behind stays
there as a tab, so your own layout is never replaced. A layout saved by an
older build loses the old Hanabi editor's panels (they no longer exist) and
gets these slotted in the first time an effect opens.

Double-clicking an old `.particle` opens its `.vfx` instead, converting it
first if it has not been converted yet; see
[Old .particle effects](#old-particle-effects).

### The graph

The graph works like the material graph. Right-click the canvas (or press
**Add Node**) for the searchable node palette, grouped by the catalog's
categories, with each node's pins and description. Drag from a pin to an
empty spot to add a node already wired to it. Drag a cable off a pin to
disconnect it, select nodes and press Delete to remove them, and press C or
**Comment** to box the selection in a comment.

Every unwired input has its value editor on the node: scrub fields for floats
and vectors, a swatch for colours, a checkbox for switches, a text field for a
Parameter's name. A **Curve** or
**Gradient** node shows a small preview of its table; click it to open the
editor, where you drag keys (or stops), set the selected one's value, easing
or colour, and add or delete keys.

Right-click a value field and choose **Animate** to key that input over the
effect's time: it gets a track in the [Timeline](../editor/animation.md#particles),
which comes to the front with the track selected. A keyed field is tinted in
the accent colour, because its own value no longer plays; right-click it again
for **Show in Timeline** or **Remove Animation**.

Each emitter ends in its **Particle Output** node, marked with a lock: it
cannot be deleted. A graph that does not compile marks the node at fault with
a red warning (hover it for the reason), shows the first problem in a strip
under the toolbar, and puts a warning on the emitter's tab.

### Emitters

The tabs above the graph are the effect's emitters. Click one to edit it,
**+ Emitter** to add one, and right-click a tab to rename, duplicate or delete
it (the last emitter cannot be deleted). Their names also show as chips under
the preview with each one's live particle count.

### The preview

| Control | |
|---|---|
| **Backdrop** | Dark, Grid or Sky. |
| **Speed** | 0.1x to 2x. |
| **Move** | Still, Circle or Figure 8: moves the emitter so world-space trails and inherited velocity are visible. |
| **Loop** | Replays a one-shot effect when it finishes. |
| **Restart on Edit** | Restarts the effect once an edit settles. Off, an edit appears in place without restarting. |
| **Restart** | Plays from the beginning. |

The stats chip shows particles alive, spawns dropped because an emitter was at
capacity (its border turns amber when any are), the effect's time, and where
its emitters simulate: **CPU**, **GPU** or **CPU + GPU**. A GPU emitter's
counts arrive a frame late.

### Saving

Every edit applies at once to the preview and to every entity in the scene
using the same file, before anything is saved. **Save** (or Ctrl+S) writes the
`.vfx`; a dot beside the file name means there are unsaved edits, and opening
another effect saves the current one first. Undo and redo (Ctrl+Z, Ctrl+Y)
step through the effect's own history.

### Templates

**New Effect** (the toolbar's **New**, the Assets panel's **Particle Effect**,
or the button on an empty panel) opens a gallery: blank 3D and 2D starters and
the built-in effects, grouped by kind. Picking one writes a new `.vfx` into the
Assets panel's current folder (or `effects/`) and opens it. **GPU Stress**
(under GPU) is 200,000 sparks bouncing off a floor, set to the GPU backend: a
load to see what the backend does on your machine.

## The `.vfx` file

A `.vfx` file is [RON](https://github.com/ron-rs/ron) text. Almost every field
has a default, so a file only states what differs. This is a complete effect:
a fountain of white dots.

```ron
(
  version: 1,
  name: "Fountain",
  dimension: ThreeD,
  duration: 2.0,
  looping: true,
  emitters: [
    (
      name: "drops",
      capacity: 300,
      graph: (
        nodes: [
          (id: 1, node_type: "output/particle", input_values: {
            "spawn_rate": Float(50.0),
            "lifetime": Float(1.5),
            "gravity": Vec3((0.0, -9.81, 0.0)),
          }),
          (id: 2, node_type: "random/vec3_range", input_values: {
            "min": Vec3((-1.0, 4.0, -1.0)),
            "max": Vec3((1.0, 6.0, 1.0)),
          }),
        ],
        connections: [
          (from_node: 2, from_pin: "value", to_node: 1, to_pin: "velocity"),
        ],
      ),
      render: (blend: Alpha),
    ),
  ],
)
```

### Effect

| Field | Default | |
|---|---|---|
| `version` | current | The format version. Older files are upgraded on load; a file from a newer engine is refused with a message saying so. |
| `name` | | Shown in the editor. |
| `dimension` | `ThreeD` | `ThreeD` or `TwoD`. Decides which renderer draws it. |
| `duration` | `2.0` | Seconds in one cycle. Emission stops after it unless the effect loops. `0` means no end. |
| `looping` | `true` | Repeat the cycle forever. Bursts and `emitter_time` restart each cycle. |
| `emitters` | | One or more emitters, below. |
| `tracks` | none | Keyed parameters over the effect's time; see [Keyed parameters](#keyed-parameters). Left out when there are none. |

### Emitter

| Field | Default | |
|---|---|---|
| `name` | `"emitter"` | |
| `enabled` | `true` | A disabled emitter is skipped. |
| `capacity` | `256` | Most particles alive at once. Spawns beyond it are dropped (and counted). |
| `space` | `Local` | `Local`: particles move with the entity (a torch flame). `World`: particles stay where they were born (a trail behind a moving emitter). |
| `seed` | `1` | Seeds every random number. Two emitters with different seeds look different; the same seed always looks the same. |
| `graph` | | The behaviour, below. |
| `render` | | How particles are drawn, below. |
| `light` | off | Real scene light from the particles; see [Light](#light). |
| `sub_emitters` | none | Other emitters this one's particles spawn; see [Sub-emitters](#sub-emitters). |
| `backend` | `Auto` | Where the emitter simulates: `Auto`, `Cpu` or `Gpu`; see [GPU backend](#gpu-backend). Left out of the file while `Auto`. |
| `inherit_velocity` | `0.0` | `World` space only: the share of the entity's world velocity each particle adds to its own at birth (**Inherit Velocity** in Emitter Settings), so sparks thrown from a moving cart keep its momentum. 1 is all of it. A `Local` emitter ignores it, since its particles already move with the entity. Left out of the file while 0. |

A moving entity's velocity is **not** passed on to its particles unless the
emitter inherits it (above) or the graph asks for it: add **Emitter
Velocity** to the spawn velocity for a per-particle share.

### Render settings

| Field | Default | |
|---|---|---|
| `texture` | `None` | `Some("textures/flame.png")`, project-relative. With no texture each particle is a soft round dot. |
| `sprite_sheet` | 1 x 1 | `(columns: 4, rows: 4, mode: OverLifetime)` plays the frames once over each particle's life; `mode: Fps(24.0)` loops them at a fixed rate. Frames run left to right, top to bottom. |
| `blend` | `Alpha` | `Alpha` for smoke and confetti, `Additive` for fire, sparks and glows, `Premultiplied` for textures whose colour is already multiplied by alpha, `Multiply` for soot and scorch that darken what is behind, `AlphaMask` for cut-outs (leaves, grass), `AlphaToCoverage` for cut-outs with antialiased edges, and `Opaque` for solid debris; see [Opaque and alpha mask](#opaque-and-alpha-mask). |
| `alpha_cutoff` | `0.5` | `AlphaMask` and `AlphaToCoverage`: fragments whose alpha is below it are thrown away (or, with coverage, where the edge falls). Also where a blended particle's shadow is cut with `shadows: On`. |
| `facing` | `CameraBillboard` | `CameraBillboard` turns every particle to the camera. `VelocityAligned` stretches the particle along its velocity (sparks, rain): give it a tall `size`. `FixedAxis((0.0, 1.0, 0.0))` keeps the particle upright and turns it around that axis. `Ribbon` draws a strip instead of a quad; see [Ribbons](#ribbons). |
| `ribbon` | | `Ribbon` only: `(kind: Trail, points: 16, min_distance: 0.05, groups: 1, tail_width: 0.0, tail_alpha: 0.0)`, plus `shape: Tube` and `radial_segments: 8` for a round tube; see [Ribbons](#ribbons). |
| `z_order` | `0.0` | 2D only: added to the entity's z, to put an effect in front of or behind sprites. |
| `shading` | `Sprite` | What each particle looks like; see [Shading](#shading). |
| `stretch` | `0.0` | `VelocityAligned` only: seconds of travel added to the height, so a fast particle is longer (sparks, flame tongues). |
| `frame_blend` | `false` | Crossfade each sprite-sheet frame into the next instead of cutting, so a flipbook plays smoothly. |
| `erosion` | `0.0` | 0 to 1: dissolve the particle with noise as it ages, so it breaks into wisps instead of fading flat. |
| `soft_distance` | `0.0` | 3D only, metres: fade the particle where it nears the scene behind it, so it never shows a hard line where it crosses the ground. 0 is off. |
| `draw_order` | `Index` | `Alpha` and `Premultiplied` only: the order the particles are drawn in, which decides what shows on top where they overlap: `Index`, `OldestFirst`, `YoungestFirst` or `ViewDepth`; see [Draw order](#draw-order). Left out of the file while `Index`. |
| `sort` | `false` | The older spelling of `draw_order: ViewDepth`, still read: a file with `sort: true` and no `draw_order` sorts back to front. The editor writes both. |
| `mesh` | `None` | 3D only: draw each particle as a mesh instead of a quad: `Some(Cube)`, `Sphere`, `Cylinder`, `Capsule`, `Cone`, `Torus`, `Tetrahedron`, `Quad` (a one-sided card) or `Asset("models/rock.glb")`; see [Mesh particles](#mesh-particles). |
| `orient` | `Tumble` | Mesh particles only: how each mesh is turned. `Tumble`, `Velocity` or `Axis((0.0, 1.0, 0.0))`; see [Mesh particles](#mesh-particles). |
| `material` | `None` | 3D only: `Some("materials/rock.material")` draws the particles (quads, ribbons or meshes) with a project material instead of the particle shader, on either backend; see [Project materials](#project-materials). |
| `lighting` | unlit | 3D only: `(lit: true, roughness: 0.7, metallic: 0.0)` shades the particles with the scene's lights; see [Lit particles](#lit-particles). |
| `screen_space_size` | `false` | The particle's `size` is in pixels on screen, not in world units, so it stays the same size at any distance; see [Screen-space size](#screen-space-size). |
| `shadows` | `Auto` | 3D only: `Auto` (solid blends cast shadows, blended ones do not), `On` (every particle casts, blended ones cut at `alpha_cutoff`) or `Off`; see [Shadows, the prepass and motion vectors](#shadows-the-prepass-and-motion-vectors). Left out of the file while `Auto`. |
| `render_method` | `Auto` | 3D, solid blends only: `Auto` (the project's renderer), `Forward` or `Deferred`; see [Deferred renderer](#deferred-renderer). Transparent blends always draw forward. Left out of the file while `Auto`. |

Particles are unlit unless [lit](#lit-particles) and draw correctly with both
the forward and the [deferred renderer](#deferred-renderer) (an emitter can
still [light the scene](#light)). Solid particles cast shadows unless told not to; blended ones
cast none unless told to. Sizes are in world units, metres in 3D and pixels
with a default 2D camera, unless the emitter sizes in [screen
pixels](#screen-space-size).

Soft particles read the camera's depth prepass, which every engine camera has
in both forward and deferred mode. A camera without one (and the web on
WebGL2, where Bevy cannot copy depth) draws the same particles with hard edges.

### Shading

| `shading` | Draw with | |
|---|---|---|
| `Sprite` | any blend | The texture (or a soft dot), tinted by the colour. |
| `ProceduralFlame` | `Additive`, `VelocityAligned` | A flame tongue drawn from noise: a teardrop pointing along the motion, its edges licked and eroded by rising noise until the top breaks into separate wisps, coloured by temperature along a blackbody ramp (white-yellow core, orange body, red tips, a faint blue base). The colour's **rgb is an HDR tint** (above 1 so the core blooms) and its **alpha is the heat**: 1 burns white-hot, 0.5 orange, 0 nothing. |
| `ProceduralSmoke` | `Alpha` | A soft, billowing, top-lit puff with eroded edges, in the particle's colour and opacity. |

Every particle gets its own noise pattern and both procedural modes animate
continuously, so no two flames match and none repeats. Neither needs a texture.
For a hand-made flipbook instead, use `Sprite` with a sprite sheet and
`frame_blend: true`.

### Mesh particles

With a `mesh`, each particle of a 3D effect is a copy of that mesh instead of a
camera-facing quad: rubble, leaves, coins, shell casings. In the editor this is
the **Shape** row of the Render section: **Quad (Sprite)** is the ordinary
particle, then the built-in shapes, then **Model**, which takes a `.glb` or
`.gltf` dropped from the Assets panel (its first mesh, unless the path names
one: `rock.glb#Mesh2/Primitive0`).

The built-in shapes fit a 1 m cube, so a particle's `size` is its size in
metres: the mesh is scaled by `(size.x, size.y, size.x)`. A model is scaled the
same way from its own units. Each particle's colour tints the mesh, and the
texture, sprite sheet and erosion run over the mesh's UVs. A mesh with no
texture is solid rather than a soft dot.

`orient` turns each mesh, from data every particle already has:

| `orient` | |
|---|---|
| `Tumble` | A random orientation per particle, spun about a random axis by the particle's rotation. Give the graph an **Angular Velocity** and debris tumbles as it flies. |
| `Velocity` | The mesh's +Y along the velocity, rolled about it by the rotation: arrows, casings, raindrops. `stretch` lengthens it with speed, as it does a velocity-aligned quad. |
| `Axis((x, y, z))` | Spun about a fixed axis (emitter space) by the rotation: a coin on its edge, a leaf that stays level. |

A mesh replaces `facing`, which does not apply (a mesh with `facing: Ribbon`
draws meshes, not a ribbon). Mesh particles are 3D only; a 2D effect draws
quads whatever `mesh` says.

A mesh can wear a [project material](#project-materials): debris made of the
same rock as the level.

Keep particle meshes low-poly: every particle is a full copy. A model of more
than 65,536 vertices is refused with a Console line. On the GPU the pool mesh
holds a copy per slot, up to 4 million vertices; past that the pool draws only
as many particles as fit and says so.

### Lit particles

By default a particle's colour is drawn as it is, which is right for fire,
sparks and magic: they are light. Smoke in sunlight, dust and debris are
surfaces. `lighting: (lit: true)` (the **Lit** row) shades them with Bevy's own
PBR lighting: the directional light and its shadows, point and spot lights,
ambient light and the environment map. `roughness` (0 a mirror, 1 chalk) and
`metallic` set the surface.

A quad is lit as a flat card facing the camera; a mesh is lit by its own
normals, from either side. `Additive` particles are never lit, because their
colour is emitted light, and neither are `Multiply` ones, which filter what is
behind rather than being a surface. Lit particles receive shadows, and cast
them as [any particle does](#shadows-the-prepass-and-motion-vectors).
Lighting is 3D only. For the full PBR surface (normal maps, emissive,
clearcoat and the rest) give the particles a [project
material](#project-materials) instead.

### Project materials

Any 3D particle can wear a project `.material`: quads, ribbons and mesh
particles, on the CPU and on the GPU backend alike. Set `material` (the
**Material** row, which takes a `.material` dropped from the Assets panel) and
the particles are drawn with that material's own shader, with everything it
does on a mesh: full PBR lighting, image-based lighting, fog, normal maps,
emissive, every pin its graph drives and every parameter a material instance
overrides. Sparks can be a glowing node graph and debris the same rock as the
level. The material goes through the engine's own material system, so a
material saved in the material editor updates the particles wearing it at
once.

The particle places the material: its facing, size, rotation, orientation,
screen-space size and sprite-sheet cell (which reaches the material as its
UV) still apply, and the emitter's `blend` replaces the material's own alpha
mode, so a rock material can still fade on debris. The particle's colour
reaches the material as the vertex colour, which tints it as it does on a
mesh. What the particle shader draws (the texture, the procedural shading,
erosion, soft distance and the Lit rows) is the material's business instead;
the material graph's Depth Fade node gives a material soft edges.

A material graph reads the particle through four **Particle** input nodes:

| Node | Output | On a particle | Anywhere else |
|---|---|---|---|
| **Particle Color** | `color`, `rgb`, `a` | the particle's colour after its colour over life | white |
| **Particle Age** | `age` | its age over its lifetime, 0 at birth to 1 at death | 0 |
| **Particle Random** | `value` | a 0 to 1 random fixed for the particle's whole life | 0 |
| **Particle Velocity** | `velocity`, `speed`, `direction` | its world-space velocity | zero |

So one material can dress both a level and its own debris, and a spark can
cool from white to red over its life by reading its age.

A material whose file cannot be used on particles (a sprite material, a
`.shader`, one that fails to compile) draws the particles with the particle
shader instead and says so in the Console. A build without the material
system does the same. 2D effects draw through their own renderer, which a 3D
`.material` cannot run in, so `material` is 3D only.

### Opaque and alpha mask

`Opaque` draws particles fully solid, and `AlphaMask` draws them solid where
the alpha is at least `alpha_cutoff` and not at all elsewhere: a leaf texture's
outline, or the soft dot cut into a hard disc. Both write depth and draw in the
scene's opaque and alpha-mask passes, so they are ordered against each other
and the scene by the depth buffer, need no sort, and are the cheapest way to
draw many solid particles. Soft fading does not apply to them. They are in the
depth prepass and cast shadows; see below. Both work in 2D too.

`AlphaToCoverage` is a mask whose edge the camera's MSAA antialiases: the alpha
becomes the share of each pixel's samples covered, so a leaf's outline is
smooth instead of stair-stepped, while still writing depth and needing no
sort. Without MSAA it looks like `AlphaMask`; in 2D, which has no such mode, it
is `AlphaMask`.

`Multiply` darkens what is behind the particle by its colour, faded by its
alpha (white leaves it as it is): soot, scorch marks, the shadow of a swarm.
Multiplication gives the same answer in any order, so it never needs a sort
either. It is never lit.

### Shadows, the prepass and motion vectors

Solid particles (`Opaque`, `AlphaMask`, `AlphaToCoverage`) are geometry as far
as the renderer is concerned:

- they **cast shadows**, from every light that casts them: a quad is turned
  toward the light, as a card is seen from the light's side, and a mask's or
  coverage's cut-out shows in its shadow;
- they write the **depth prepass** and the **normal prepass**, so SSAO, depth
  of field and other emitters' soft fades see them;
- they write **motion vectors**, so TAA and motion blur treat them like any
  moving object. A particle's previous position is worked out from its
  velocity and the frame's simulated time, and for a `Local` emitter the
  entity's own movement too; a paused effect has none. Only travel is
  counted: a particle's spin, growth, and a billboard turning toward a moving
  camera are not.

Blended particles (`Alpha`, `Additive`, `Premultiplied`, `Multiply`) draw in
the transparent pass after the prepass, as every transparent surface does, so
they are in none of the prepass textures and write no motion vectors. They
cast no shadows unless `shadows: On` (the **Cast Shadows** row), where the
shadow is the particle cut at `alpha_cutoff` (a shadow map holds depth, not
opacity). `shadows: Off` turns a solid emitter's shadows off.

All of this holds on both backends, for the particle shader and for a [project
material](#project-materials), which casts and receives as it does on a mesh.
Screen-space sizes are worked out for the camera, so a pixel-sized particle's
shadow is sized for the light's view, not the camera's.

Order-independent transparency is not used for particles: Bevy's OIT
composites each layer with plain alpha blending, which would draw additive and
premultiplied particles wrongly. On a camera with OIT, particles draw in the
ordinary transparent pass, ordered by emitter and their own draw order.

### Deferred renderer

On a project whose rendering mode is deferred, **solid particles write the
G-buffer**, like every other solid surface in the
scene. They are lit by the deferred lighting pass, and everything that reads
the G-buffer sees them: screen-space reflections, SSAO, and Solari. This holds
on both backends, for quads, meshes, ribbons and tubes, and with the particle
shader as well as a [project material](#project-materials).

| Particles | Deferred camera | Forward camera |
|---|---|---|
| `Opaque`, `AlphaMask`, `AlphaToCoverage` | G-buffer | forward, as always |
| `Alpha`, `Additive`, `Premultiplied`, `Multiply` | forward, after the deferred lighting | forward, as always |

What lands in the G-buffer:

- **The particle shader, lit**: the colour times the texture as the base
  colour, the emitter's roughness and metallic, and the card's or the mesh's
  normal, the same ones the forward lighting uses. A colour brighter than 1
  (the Emission pin past 1, an HDR gradient) keeps its glow: the part over 1
  is written as emissive, because the G-buffer's base colour cannot hold it. A
  `ProceduralFlame` is light rather than a surface, so all of it is emissive;
  `ProceduralSmoke` and sprites are surfaces.
- **The particle shader, unlit**: Bevy's unlit flag, so the deferred lighting
  draws the colour as it is, at full brightness, exactly as forward.
- **A project material**: its own deferred output, the same G-buffer the
  material writes on a mesh, with the Particle nodes (colour, age, random and
  velocity) reading the particle there as they do forward. A material graph
  that reads the scene depth, normals, motion vectors, transmission or the
  environment map cannot run in the G-buffer pass, on a particle or on a mesh,
  so it writes its base approximation there instead, and a material that is a
  plain StandardMaterial writes that.

`AlphaToCoverage` goes through the G-buffer too. A deferred camera runs
without MSAA (the G-buffer is one sample per pixel), so there it cuts at
`alpha_cutoff` like `AlphaMask`, as it does on any camera without MSAA.

**Transparent blends always draw forward.** A G-buffer holds one surface per
pixel, and a blended particle has to be composited over whatever is behind
it, so `Alpha`, `Additive`, `Premultiplied` and `Multiply` draw in the forward
transparent pass after the deferred lighting, as transparent surfaces do in
every deferred engine. They look the same on either renderer, and SSR and SSAO
do not see them.

The **Render Method** row (`render_method`) chooses per emitter, for the solid
blends; with a transparent blend the row shows Forward and says why:

| `render_method` | |
|---|---|
| `Auto` | The project's renderer, as every material in the scene follows: the G-buffer on a deferred project, forward on a forward one. A forward project draws exactly as it did before particles could go deferred. |
| `Forward` | Always the forward pass, even on a deferred camera. |
| `Deferred` | The G-buffer, even on a forward project, while every 3D camera has a deferred prepass. A camera without one cannot draw a deferred surface at all, so while any exists the particles draw forward instead. |

Shadows, the depth and normal prepasses and motion vectors are unchanged on
both renderers: on a deferred camera the G-buffer pass writes the depth,
normals and motion vectors the prepass would have.

### Screen-space size

`screen_space_size: true` (the **Size in Pixels** row) makes `size` a size in
**physical pixels**: a particle of size 16 is 16 pixels across at any distance.
For markers, pickups that must read from across the map, and UI-like sparkles.
It is the same measure Hanabi used, so a converted effect keeps its look. It
works with quads, ribbons and meshes, on both backends, in 2D and 3D. Such an
emitter is never frustum culled, since a size in pixels has no world-space box.
A project material is sized the same way, per view, by the particle vertex
shader.

### Draw order

Alpha-blended particles show whichever is drawn last on top, so the order
matters wherever they overlap. **Draw Order** in Emitter Settings
(`draw_order` in the file):

| `draw_order` | Draws | For |
|---|---|---|
| `Index` | however the backend holds them, at no cost | anything where the order cannot be seen |
| `OldestFirst` | the oldest first, so the newest are on top | a fresh puff of smoke over the haze it rose from |
| `YoungestFirst` | the newest first, so the oldest are on top | layers whose first one must stay over the rest |
| `ViewDepth` | back to front from the camera (3D only) | smoke and dust that cross each other in depth |

It applies to `Alpha` and `Premultiplied` blending only: additive light adds
up the same in any order, and `Opaque` and `AlphaMask` are ordered by the
depth buffer. Ribbons are drawn along their strips and ignore it, and a 2D
effect draws `ViewDepth` in index order (it has no depth). Every order but
`Index` costs a sort per frame on either backend: the CPU orders what it
writes, the GPU runs a bitonic sort of the live particles; `ViewDepth` sorts
against the camera that sees the effect (the preview's camera in the
preview). On the CPU ties keep buffer order, so particles born in one step
never swap; the GPU's sort makes no such promise for particles of exactly
the same age or distance.

## The graph

Each emitter's graph has exactly one **Particle Output** node
(`output/particle`). Its inputs are what the emitter does, and each one is
worked out at a different moment, its **context**:

| Context | When | Output pins |
|---|---|---|
| **Spawn** | once per frame, for the whole emitter | `spawn_rate` (per second), `burst_count`, `burst_interval` (seconds), `burst_cycles` (0 = unlimited) |
| **Init** | once per particle, when it is born | `lifetime`, `position`, `velocity`, `size` (Vec2), `rotation` (radians), `angular_velocity`, `color` |
| **Update** | every frame, for every particle | `acceleration`, `gravity`, `drag`, `size_over_life` (multiplies `size`), `color_over_life` (multiplies `color`), `max_speed` (a hard cap on speed after every force; 0 is none), `emission` (an HDR intensity on the colour's rgb, not its alpha) |

A pin with nothing connected uses its value from `input_values`, or its default
(spawn rate 10, burst count 0, lifetime 1, size 0.1, colour white, multipliers
and emission 1, everything else 0). A particle dies once its age passes its
lifetime.

**Emission** is for glow: wire a **Curve** into it (the curve samples the
particle's life unless its `t` is wired) and a spark flares past white into
the bloom when it is born and cools to its plain colour as it dies, while
`color_over_life` stays an ordinary 0 to 1 gradient and the alpha, which is
coverage, is left alone. It runs on both backends, and the emitter's light
follows the brighter colour.

An input that means nothing in a context reads as **0**: a particle has no age
yet when it is born, so **Age** wired into `lifetime` is always 0.

### Pins and types

Pins carry `Float`, `Vec2`, `Vec3` or `Color` (linear RGBA). A `Float` can go
into any pin and fills every component (0.5 into `size` is a 0.5 x 0.5
particle). Anything else must match, and a mismatch is an error naming the
node. Math nodes take whatever they are given: **Multiply** works on floats,
vectors and colours alike.

Inline values are written `Float(1.0)`, `Vec2((1.0, 2.0))`,
`Vec3((1.0, 2.0, 3.0))` and `Color((1.0, 0.5, 0.0, 1.0))`. A `Vec3` is accepted
for a colour, with alpha 1.

### Randomness

Every random node gives a value that is **fixed for each particle** and stable
for its whole life, so a particle picked to be big stays big. Two random nodes
are independent of each other. The `salt` input picks a different value: two
**Particle Random** nodes with the same salt agree, which is how you make, say,
the largest particles also the brightest. Wire `emitter_time` into a salt for a
value that changes every frame.

Because nothing random depends on frame timing, an effect replays identically
from its seed.

### Node catalog

**Output**

| Node | |
|---|---|
| `output/particle` | The sink, above. |

**Shapes.** Each outputs a `position` and a `direction` (Vec3), random per
particle. Wire `position` to the output's position, and multiply `direction`
by a speed for the velocity.

| Node | Inputs | |
|---|---|---|
| `shape/point` | | Starts at the origin; direction is any way at all. |
| `shape/sphere` | `radius` | Inside the sphere, or on its surface with the `surface: Bool(true)` property. Direction points outward. |
| `shape/box` | `size` (Vec3) | Inside a box centred on the origin. |
| `shape/cone` | `angle` (degrees), `radius` | Starts on a disc of `radius` in the XZ plane; direction within `angle` of straight up. |
| `shape/circle` | `radius` | 2D: inside the circle in the XY plane, or on its edge with `edge: Bool(true)`. |
| `shape/rect` | `size` (Vec2) | 2D: inside a rectangle in the XY plane. |

**Inputs**

| Node | Output | |
|---|---|---|
| `input/age` | Float | Seconds since the particle was born. |
| `input/life` | Float | Age over lifetime: 0 at birth, 1 at death. |
| `input/lifetime` | Float | |
| `input/position` | Vec3 | |
| `input/velocity` | Vec3 | |
| `input/index` | Float | The particle's spawn number, from 0. |
| `input/particle_random` | Float | 0..1, fixed per particle; `salt` input. |
| `input/emitter_time` | Float | Seconds into the current cycle. |
| `input/delta_time` | Float | This frame's step in seconds. |
| `input/emitter_velocity` | Vec3 | How fast the entity is moving. |
| `input/wind` | `velocity` (Vec3), `strength` (Float) | The world wind in m/s, gusts included, plus a cross-wind wobble from its turbulence that differs per particle. `scale` multiplies it; `frequency` is the wobble rate in Hz. `strength` is the speed over 12 m/s. See [Wind](#wind). |
| `input/parameter` | same as `value` | A value a script sets by **Name** while the effect plays (`vfx_set_param`); until one does, it is its own `value` (a float, or a vector or colour if you give it one). See [Parameters](#parameters). |

**Random** (all fixed per particle, all with a `salt` input)

| Node | |
|---|---|
| `random/range` | A float between `min` and `max`. |
| `random/vec3_range` | A vector between `min` and `max`, per component. |
| `random/unit_vector` | A random direction. |
| `random/color_between` | A colour between `a` and `b`; output pin `color`. |

**Math**

| Node | |
|---|---|
| `math/float`, `math/vec3`, `math/color` | Constants (set the `value` input). |
| `math/add`, `math/subtract`, `math/multiply`, `math/divide` | Pins `a`, `b`. Division by zero gives 0. |
| `math/min`, `math/max` | Per component. |
| `math/lerp` | `a + (b - a) * t`. |
| `math/clamp` | `value` between `min` and `max`. |
| `math/abs`, `math/negate`, `math/sin`, `math/cos` | Pin `value`; sin and cos in radians. |
| `math/smoothstep` | 0 below `edge0`, 1 above `edge1`, smooth between. |
| `math/remap` | `value` from `in_min..in_max` onto `out_min..out_max`. |
| `math/length`, `math/normalize`, `math/dot` | Vector helpers; normalizing zero gives zero. |
| `math/compose_vec3`, `math/split_vec3` | Build a vector from `x`, `y`, `z`, or take one apart. |

**Curves**

| Node | |
|---|---|
| `curve/curve` | A float curve sampled at `t` (the particle's **life** unless wired). Keys go in the `points` property: `Curve([(t: 0.0, value: 0.0, ease: EaseOut), (t: 1.0, value: 1.0)])`. Easing is `Linear`, `Step`, `Smooth`, `EaseIn`, `EaseOut` or `EaseInOut`, and applies from that key to the next. |
| `curve/gradient` | A colour gradient sampled at `t` (life unless wired); output pin `color`. Stops go in `stops`: `Gradient([(t: 0.0, color: (1.0, 1.0, 1.0, 1.0)), (t: 1.0, color: (1.0, 0.2, 0.0, 0.0))])`. |

Wire a curve into `size_over_life` to grow or shrink particles, and a gradient
into `color_over_life` to fade them out.

**Forces.** Each outputs a `force` (Vec3) to wire into `acceleration` (add them
together for more than one). Their `position` input is the particle's own
position unless something else is wired in.

| Node | Inputs | |
|---|---|---|
| `force/curl_noise` | `strength`, `frequency`, `scroll`, `octaves`, `lacunarity`, `persistence` | Swirling turbulence that never bunches particles up. `scroll` moves the noise over time. `octaves` (1 by default, up to 8, fractions fade the last one in) adds finer swirls, each `lacunarity` (2) times the frequency and `persistence` (0.5) times the strength of the last, summed, so more octaves are a little stronger. |
| `force/speed_limit` | `limit`, `rate` (`velocity`) | Brakes a particle faster than `limit` back toward it at `rate` per second; slower ones are untouched. For a hard cap use the output's `max_speed`. |
| `force/drag` | `linear`, `quadratic` (`velocity`) | Air resistance `-v * (linear + quadratic * speed)`: quadratic drag gives a terminal speed. Stable at any strength. |
| `force/attractor` | `point`, `strength`, `falloff` | Pulls toward `point` (negative pushes). `falloff` weakens it with distance. It never pulls a particle past the point in one step, so particles settle on it at any frame rate. |
| `force/vortex` | `center`, `axis`, `strength` | Spins particles around an axis. |
| `force/wind` | `coupling`, `height_falloff` (`velocity`, `wind`) | Pulls the particle's velocity toward the world wind: `(wind - velocity) * coupling`. See [Wind](#wind). |
| `force/gravity_well` | `point`, `strength`, `softening`, `max_force` (`velocity`) | Real gravity toward `point`: `strength` is the pull at 1 m, a quarter of it at 2 m (negative pushes). Within `softening` (0.25 m) it eases off to nothing at the centre (Plummer softening), and `max_force` caps it (0 is no cap). A particle let go at rest falls through and swings back out as far as it started, pass after pass; even with a tiny softening and a long frame it is never slung out further. |
| `force/orbit` | `center`, `axis`, `speed`, `grip`, `radius`, `radial_pull` (`velocity`) | Circles particles around `axis` through `center` at `speed` radians per second, holding each at its distance from the axis. `grip` (per second) is how fast they reach that speed. With a `radius`, `radial_pull` draws them onto a ring of that radius without overshooting. Exact at any frame rate: a stepped circle neither spirals out nor in. |
| `force/conform_sphere` | `center`, `radius`, `influence`, `attraction`, `max_speed`, `sticky`, `shell` (`velocity`) | Hanabi's conform to sphere: particles within `influence` of the surface (and any inside) are drawn onto it at up to `max_speed`, accelerating at `attraction`, then held on it while they slide around. Inside a shell `shell` thick either side of the surface the pull is `sticky` times stronger, so a fast particle sticks instead of overshooting. Best wired last into `acceleration`, as in Hanabi. |

Every force here is stable on long frames: the Attractor and Gravity Well
never pull a particle through their centre in one step, and the Orbit lands
each step back at the distance it holds. All of them run on the GPU backend
too.

**Collision.** These act by being in the graph: there is nothing to wire
out of them. After a particle moves, each one pushes it back out of its shape,
bounces it by `bounce` (0 to 1, the share of the speed into the surface that
comes back) and slows its slide by `friction` (Coulomb friction, so a resting
particle slides to a stop at the same rate at any frame rate). **Kill On Hit**
kills the particle instead. The shape is in the effect's own space, so it moves
with the entity whatever the emitter's space; **World Space** puts it in world
coordinates instead. A fast particle that crosses a solid shape in one step
still hits it.

| Node | Inputs | |
|---|---|---|
| `collide/plane` | `point`, `normal`, `bounce`, `friction` | An infinite plane; particles stay on the side the normal points to. |
| `collide/sphere` | `center`, `radius`, `bounce`, `friction` | A solid ball, or with **Inside** a bowl that keeps particles in. |
| `collide/box` | `center`, `size`, `bounce`, `friction` | An axis-aligned solid box, or with **Inside** a room. |
| `collide/scene` | `radius`, `bounce`, `friction` | The scene's own physics colliders: the level, props, anything with a collider. `radius` (Particle Radius) keeps a fat particle's edge on the surface rather than its centre. Sensors are ignored. CPU only; see below. |

Selected, the effect's colliders show in the viewport in blue beside its spawn
shapes, and the Conform To Sphere, Orbit and Gravity Well forces in violet.

**Collide Scene** asks the physics engine where each particle's motion this
frame first meets a collider (a ray, or a sphere sweep with a Particle Radius),
in one batch for every effect, after the frame's steps. A 3D effect hits the
3D physics world and a 2D effect the 2D one. It needs the physics plugin in the
build; without it the node does nothing. What to know:

- It runs on the CPU only, so an emitter with it stays there whatever its
  backend says, and **Running On** says "CPU, because it collides with the
  scene, which only the CPU can query".
- At most 4096 casts a frame across every effect. Past that, particles take
  turns; a particle that waited is swept over its whole path when its turn
  comes, so waiting never lets it through a wall.
- A hit lands at the end of the frame rather than of each step, so a particle
  that would have bounced twice in one frame bounces once.
- A seek (scrubbing the timeline, a hot reload) replays without the scene.
- In the editor, physics is paused until you press Play, so a collider moved
  or added since the last physics step is not seen until then.

**Colour**

| Node | |
|---|---|
| `color/blackbody` | The colour a hot body glows at `temperature` kelvin (800 deep red, 1800 orange, 3500 yellow-white, 6500 white), from Planck's law. `intensity` scales it (above 1 for an HDR glow); alpha stays 1. Wire a curve over life into `temperature` for a fire that cools as it rises. |

The gradient editor's **Blackbody** button fills a gradient with a fire's
colour over life the same way: white-yellow cooling through orange to a dim red
and fading out.

### Wind

Particles feel the scene's one [wind](wind.md), the same one the grass and
trees do, through **Wind Force** (`force/wind`) wired into `acceleration`.
It tracks the wind as it is this instant, gusts included, and adds a
per-particle cross-wind from the wind's turbulence. With no wind (or no wind
plugin in the build) the air is still.

The force is drag through moving air: `(wind - velocity) * coupling`. So use
it *instead of* the output's `drag`, which is drag through still air. In calm
air it slows a particle like drag does, and a buoyant particle settles at a
rise speed of `gravity / coupling`. In wind each particle is pulled toward the
wind at its own **coupling** (per second):

| Coupling | Feels like |
|---|---|
| 1 to 2 | Smoke: carried with the wind within a second. |
| 0.5 to 1 | Embers and leaves: drift on it. |
| 0.3 to 0.6 | Flames: lean and bend. |
| 0 to 0.1 | Heavy things: barely stir. |

**Height Falloff** (metres, 0 off) fades the wind toward zero at `y = 0` of the
emitter below that height, so a fire's base stays sheltered while its tips are
blown. A `Local` emitter feels the wind in its own space, so turning the entity
does not turn the wind. For anything else, wire **Wind** (`input/wind`) into
your own maths, or into the force's `wind` pin to scale or redirect it (in 2D,
where units are pixels, scale it up).

Wind is an input rather than part of the replay: seeking an effect
(`seek`, the editor's scrubbing, a hot reload) replays it in the current wind,
not the wind it was played in, so a seek in a gusting wind is close rather
than exact.

### Errors

A graph that does not make sense (a cycle, a missing Particle Output, a Colour
wired into a position) is reported in the Console with the id of the node at
fault, and that emitter is skipped. The rest of the effect still plays. In the
effect editor the node itself is marked.

A graph may also carry `comments: [(id: 9, rect: (0.0, 0.0, 300.0, 200.0),
text: "Embers", color: (88, 110, 150))]`, the editor's comment boxes. The
compiler ignores them.

## Light

Particles are unlit unless [lit](#lit-particles), but an emitter can also
**light the scene** with real point
lights that follow its particles, so a campfire lights the ground and dims as
it dies down. Set it in **Emitter Settings > Light**, or in the file:

```ron
light: (mode: PerEmitter, color: (1.0, 0.52, 0.2), intensity: 3800.0,
        range: 9.0, flicker: 0.35, offset: (0.0, 0.35, 0.0)),
```

| Field | Default | |
|---|---|---|
| `mode` | `Off` | `PerEmitter`: one light at the emitter. `BrightestN(4)`: a light on each of the N brightest particles, following them (at most 16). |
| `color` | white | Tint, multiplied into the particles' average colour. A `ProceduralFlame` colour is a tint, not a glow colour, so give a fire a warm one. |
| `intensity` | `50` | Lumens per fully bright particle (colour luminance 1, alpha 1). The light is the sum over live particles, each weighted by its brightness and alpha. |
| `range`, `radius` | `6`, `0.1` | Metres the light reaches, and the light's own size (softer highlights). |
| `shadows` | `false` | Shadow maps cost a render per light per frame, so they are off unless asked for. |
| `smoothing` | `0.12` | Seconds to settle on a change, so a light does not strobe as particles spawn and die. |
| `flicker`, `flicker_rate` | `0`, `6` | Dims the light by up to this share with a smooth noise of time, at this many Hz. |
| `offset` | origin | `PerEmitter` only: where the light sits, in the entity's space (a fire's light belongs at flame height). |

The lights are hidden children of the effect: not in the hierarchy, never
saved, outside the selection box, on the effect's render layers. 3D only.
They follow the effect's own clock, so a paused fire's light holds still.

**Solari.** Solari does not sample point lights, so it stands every visible
point light in as an emissive sphere of the light's radius and power. Particle
lights are ordinary point lights, so they light a Solari scene the same way,
with nothing to set up.

## Ribbons

`facing: Ribbon` draws a camera-facing strip instead of a quad per particle,
with the emitter's usual material, blend and shading. Its UVs run along the
strip from head to tail, so a texture stretches down it, and `ProceduralFlame`
licks back along it from the particle.

| `ribbon` field | Default | |
|---|---|---|
| `kind` | `Trail` | `Trail`: each particle trails its own last positions (comets, sparks, streaks). `Connect`: one strip through the particles in spawn order, newest at the head (the wake behind a moving `World` emitter). |
| `points` | `16` | `Trail`: positions kept per particle (up to 64), two vertices each. |
| `min_distance` | `0.05` | `Trail`: how far a particle moves before another position is kept. 0 keeps one every step. |
| `groups` | `1` | `Connect`: how many separate strips (particles are dealt round-robin). |
| `tail_width`, `tail_alpha` | `0`, `0` | Width and alpha at the tail, as a share of the head's (the particle's `size` x and colour). |
| `shape` | `Flat` | `Flat`: the camera-facing strip. `Tube`: a round tube whose diameter is the strip's width (**Ribbon Shape** in Emitter Settings), for ropes, tentacles, cables of light. 3D only. Left out of the file while `Flat`. |
| `radial_segments` | `8` | `Tube` only: vertices round the tube, 3 to 32. |

A trail is in the emitter's space: use `World` space for trails that stay
behind a moving emitter.

A **tube** is real geometry: it is lit by its own normals when the emitter is
[lit](#lit-particles), the same thickness from every side, and without a
texture it is solid rather than the soft dot (a texture's u runs round it and
its v down it, as a strip's does). Each ring is turned toward the camera about
the strip's direction, so it never twists however the trail bends; the price
is that a texture's seam stays on the same side as the camera moves. Every
point of a tube costs `radial_segments + 1` vertices instead of a strip's two.
A 2D effect draws a tube flat.

Both kinds and both shapes run on the [GPU backend](#gpu-backend) too. There
each slot keeps its trail in GPU memory, its positions kept by exactly the
CPU's rule (the birth point first, then a new point once the particle has
moved `min_distance`), and the vertex shader builds the strip from it. A
`Connect` ribbon is sorted into strips on the GPU each frame, by spawn order
as on the CPU; the GPU draws up to 32 strips. A GPU trail is never frustum
culled, because its history reaches past the live particles the GPU reports
the bounds of.

## Sub-emitters

An emitter's particles can spawn particles of another emitter in the same
effect: sparks that burst into embers when they die, a rocket that drops smoke
as it flies. Set them in **Emitter Settings > Sub-Emitters** (two slots), or:

```ron
sub_emitters: [(trigger: OnDeath, target: 1, count: 12,
                inherit_velocity: 0.3, inherit_color: true)],
```

| Field | |
|---|---|
| `trigger` | `OnBirth`, `OnDeath` (its life ran out or a collider killed it), or `Interval(0.1)`: every so many seconds while alive. |
| `target` | The emitter to spawn, by its position in the list (the tab number minus one). |
| `count` | Particles per trigger. |
| `inherit_velocity` | 0 to 1: the share of the parent's velocity each child adds to its own. |
| `inherit_color` | Multiply each child's colour by the parent's. |

A target emitter emits only what its parents ask for: its own spawn rate and
bursts are ignored. Its graph still sets each child's lifetime, velocity, size
and colour, and the position it gives is an offset from the parent, so a shape
node spreads the children around it. A loop (A spawns B spawns A) is refused
with a Console warning, and a chain stops four generations down. Children are
seeded by their parent and event, so seeking replays them exactly.

**On the GPU.** Sub-emitters run on the [GPU backend](#gpu-backend) on both
sides of a rule, with every trigger and setting above. A parent's kernel
writes each event (where, how fast, what colour) into its target's inbox in
GPU memory, and the target spawns the children in the same step, so nothing
is read back. What to know:

- A rule's two emitters always run on the same backend. When one of them
  must stay on the CPU (it is set to the CPU, or uses something only the CPU
  has), the other joins it, and **Running On** says "CPU, because it shares
  sub-emitter events with `sparks`, which runs on the CPU". A small `Auto`
  target of a GPU parent moves up to the GPU with it rather than pulling the
  parent down.
- An inbox takes at most 8192 events a step (fewer for a smaller target);
  past that the events are dropped and their children counted as the target's
  drops, so a burst of deaths can never overflow it.
- A child's randoms are fixed by its parent, rule, event and its place in the
  event, so a seek replays it exactly, but its **Index** is a number hashed
  from those rather than the next spawn index the CPU would give it. A
  `Connect` ribbon on a target, which runs in spawn order, therefore stays on
  the CPU.
- A chain five emitters long whose last emitter is reached by paths of
  different lengths (some under the four-generation cap and some over) stays
  on the CPU, which counts generations per particle; the GPU decides per
  emitter.

## Keyed parameters

An effect can change over its own time: a fire that flares, smoke that
thins, sparks that come in bursts. Each change is a **track** on the effect,
keyed in the [Timeline](../editor/animation.md#particles) or written by hand:

```ron
tracks: [
  (emitter: 0, target: Pin(node: 1, pin: "spawn_rate"), keys: [
    (time: 0.0, value: Float(10.0), interp: Smooth),
    (time: 1.5, value: Float(80.0)),
  ]),
  (emitter: 0, target: Burst, keys: [(time: 0.5, value: Float(40.0))]),
  (emitter: 1, target: Enabled, keys: [
    (time: 0.0, value: Bool(false)),
    (time: 1.0, value: Bool(true)),
  ]),
],
```

| Target | Keys | |
|---|---|---|
| `Pin(node, pin)` | the pin's type (`Float`, `Vec2`, `Vec3`, `Color`) | Overrides that input's inline value. A wired input keeps its wire. |
| `Burst` | `Float(count)` | Spawns `count` particles at each key's time, every cycle, on top of the graph's burst pins. |
| `Enabled` | `Bool` | Switches the emitter on and off; particles alive when it switches off live out their life. An emitter with `enabled: false` plays if a track switches it on. |

`interp` is how a value travels to the next key: `Linear` (the default),
`Smooth`, or `Step` (hold until the next key). Before the first key and after
the last, the nearest key's value holds. Tracks run on the effect's own time,
wrapping with `duration` when it loops, so a `.vfx` plays its keys wherever it
is used, with no animator.

A keyed input is read at the moment it matters: a **Spawn** input (spawn
rate, bursts) and an **Update** input (drag, forces) at the current time, an
**Init** input (lifetime, velocity, colour) at the particle's birth, so a
particle born while the colour is red stays red. Seeking replays the tracks
too, so a scrubbed effect is exactly the frame play reaches.

## Parameters

A **Parameter** node (`input/parameter`, under Input) is a knob a script turns
while the effect plays: how hard a fire burns, which way the sparks fly, what
colour a spell is. Give it a **Name** and a **Value**, wire it in like any
value, and call `vfx_set_param("intensity", 3.0)` from a script (see
[Scripting](#scripting)). Until a script sets it, it is its own Value.

```ron
(id: 7, node_type: "input/parameter", input_values: {
  "name": Text("intensity"),
  "value": Float(1.0),
}),
```

- The Value's shape is the parameter's type: a float, or a `Vec3` or `Color`
  to make a vector or colour parameter. A number set on a vector fills every
  component, as a Float wired into a vector pin does.
- Every Parameter node with the name answers one call, in every emitter, so
  one `intensity` can drive the flames' spawn rate and the light's colour at
  once.
- A Parameter can also be keyed in the Timeline. A script's value wins over
  the keys until `vfx_clear_param` gives it back to them.
- A value is read when it matters, as keyed inputs are: an Init input at each
  particle's birth, an Update input every step.
- It costs nothing to change: no recompile, and on the GPU backend no new
  pipeline. Keyed pins and Parameters share the GPU's 16 slots.
- A script's values survive the effect being edited and hot reloaded, but are
  not saved with the scene. From code, insert or change the entity's
  `ParticleParams` component (`set_float`, `set_vec3`, `set_color`, `clear`).

## GPU backend

An emitter can simulate on the GPU instead of the CPU, for counts the CPU
cannot keep up with: 200,000 sparks is an ordinary load there. It runs the
**same graph**: the compiler's output is turned into compute shaders, so there
is nothing to rewrite and no node behaves differently. The CPU stays the
fallback, and an effect never fails to play because of where it runs.

### What runs where

| Part | Where |
|---|---|
| The clock, keyed tracks and the spawn pins (rate, bursts, burst keys) | CPU, per emitter: cheap, and it keeps bursts exact |
| Birth (the Init pins), and every step after it: forces, drag, Max Speed, colliders, size, colour and emission over life, death, inherited velocity | GPU, per particle |
| Trail history, sub-emitter events and their children | GPU, per particle: each slot's ring of trail points, and an inbox per target emitter that parents write and the target spawns from in the same step |
| Drawing | GPU, straight from the particle buffer, with the very shader the CPU path uses: facing, ribbons and tubes of both kinds, sprite sheets and frame blending, procedural flame and smoke, erosion, soft particles, every blend mode, mesh particles, lighting, screen-space size, shadows, the prepass, motion vectors and the draw order look the same. A [project material](#project-materials) is drawn from the same buffer too |
| Live count, drops, a `PerEmitter` light, culling bounds | read back from the GPU a frame late |

### How Auto chooses

`backend` on each emitter (**Backend** in Emitter Settings):

- `Auto` (the default) uses the GPU when the device supports it, the emitter's
  capacity is **2048 or more**, and the emitter uses nothing the GPU backend
  lacks. Below 2048 the CPU is fast enough and runs everywhere.
- `Gpu` uses the GPU at any capacity. If it cannot, the emitter runs on the
  CPU and the Console says why.
- `Cpu` always uses the CPU.

These stay on the CPU in this version, and an emitter using any of them runs
there whatever its setting says:

- a `BrightestN` light (`PerEmitter` works on the GPU),
- a **Collide Scene** node (the scene's colliders are asked on the CPU),
- more than 16 keyed pins and Parameter nodes together,
- a capacity past what the device allows in one buffer (at most 1,048,576),
  or a trail whose history on top of it would pass the device's buffer limit,
- a `Connect` ribbon of more than 32 strips, or on a sub-emitter's target,
- a sub-emitter chain whose generations straddle the depth cap (see
  [Sub-emitters](#sub-emitters)).

Ribbons, tubes and sub-emitters run on the GPU. An emitter joined to a CPU
emitter by a sub-emitter rule runs on the CPU with it.

The **Running On** row under Backend shows what the preview actually runs the
emitter on, and when that is the CPU, why: "CPU, because its Brightest N light
follows single particles", "CPU, because it shares sub-emitter events with
`sparks`, which runs on the CPU", or "CPU, because its capacity of 500 is below
the 2048 Auto moves to the GPU at".

The GPU backend needs compute shaders and storage buffers in the vertex
shader. The web (WebGL2) and devices without them run every emitter on the
CPU; nothing has to change in the effect. It works with both the forward and
the deferred renderer, on every render layer, in the effect editor's preview
as in the scene.

### Seeking and the timeline

A seek resets a GPU emitter and replays it from zero in the same fixed steps
the CPU uses (1/60 s), all within one frame. Up to **10 seconds** is replayed
step for step; a longer seek takes 600 proportionally longer steps instead (the
CPU's limit is 20 seconds at 1200 steps), so scrubbing late into a long effect
stays one frame of work. Scrubbing is stable: the same time always gives the
same particles.

The two backends agree on the graph, not on every bit:

- **Randoms are identical.** Every random is the same hash of the seed, the
  particle's spawn index and the node, on both, so a particle is born with the
  same lifetime, size, colour and direction wherever it runs.
- **Float rounding differs** in `sin`, `cos`, `sqrt` and the noise, by the last
  digit or so. Over seconds of motion that can move a particle visibly, but an
  effect looks the same.
- **When the pool is full**, the CPU drops a step's last spawns and the GPU
  drops whichever threads find no free slot, so the survivors' spawn indices
  can differ.
- A script **burst** spawns at once on both; on the GPU the new particles join
  the draw order's sort from the next step, as do a sub-emitter's children.
- **A sub-emitter's children** have the same randoms on every replay, but on
  the GPU their spawn index is hashed from their parent's event rather than
  counted (see [Sub-emitters](#sub-emitters)), so they are not the very
  particles the CPU spawns.
- **A tube's ring** is turned toward the camera on both backends, so the two
  draw the same tube.

### Sorting

Every [draw order](#draw-order) works on both backends: the CPU orders the
quads it writes, the GPU runs a bitonic sort of the live particles each frame
(by distance from the camera that sees the effect for `ViewDepth`, by age for
the other two). The same sort, keyed by strip and spawn order, lays out a
`Connect` ribbon.

### Memory

What the GPU keeps per emitter, beside the 96-byte particle per slot and its
lists: a `Trail` ribbon adds `16 x (points + 1)` bytes per slot for its
history (54 MB for a 16-point trail on 200,000 slots), and an effect whose GPU
emitters share sub-emitter events adds one buffer of `16 + 64 x events` bytes
per target, at most 512 KB each. The ribbon pool mesh holds a strip per slot
(two vertices a point, or a tube's ring), at most 8 M vertices; past that the
GPU draws only as many strips as fit and the Console says so.

## Realistic Fire

The **Realistic Fire** template (and built-in effect) is a log fire about
0.8 m wide, built the way film and game fires are: several emitters layered
on one spot, each doing one job. Place it on the ground at the logs' base.

| Layer | What it does | Key values |
|---|---|---|
| **flame core** | A few large, very hot tongues from the middle of the logs, rising and licking, narrowing to the tip and breaking into wisps. White-yellow and bright enough to bloom. | `ProceduralFlame`, 22/s, 0.55 to 0.85 s, tint 2.2, heat 1, stretch 0.14 s, wind coupling 0.5 |
| **outer flames** | More, smaller, cooler (orange) tongues across the whole bed, a little lower and longer lived. | `ProceduralFlame`, 55/s, 0.6 to 1 s, tint 1.45, heat 0.8, wind coupling 0.6 |
| **smoke** | A thin grey-brown haze born above the flame tips, warm at first and cooling to grey, rising slowly, swelling and carried off by the wind. | `ProceduralSmoke`, `Alpha`, 8/s, 3 to 4.5 s, opacity 0.3, grows 2.8x, wind coupling 1.4 |
| **embers** | A few tiny white-hot sparks thrown up, flickering, streaked by their speed, drifting on the wind; the heavier ones arc back down and skip off the ground. | `Sprite`, 6/s, HDR 6, stretch 0.035 s, wind coupling 0.8, Collide Plane at the base |
| **coals** | A low, wide, slowly flickering orange-red glow along the bed. | `Sprite`, 26/s soft ovals, wind coupling 0.1 |

Every layer drags through the wind with its own coupling, so in a breeze the
smoke streams away, the embers drift, the flames lean (their bases sheltered
by a height falloff) and the coals stay put. Smoke and embers live in world
space and trail behind a moving fire; flames and coals move with it. The
fire looks its best with bloom on the camera.

The outer flames carry the fire's [light](#light): one warm point light at
flame height, about 120000 lm while the fire is in full flow, flickering up to
35 %. The Campfire has the same.

To tune it: **spawn rate** on the two flame layers is how full the fire is;
their **gravity** (buoyancy) and **lifetime** set its height; the **color**
alpha on the flame layers is the heat (lower it for a dying, redder fire) and
its rgb the brightness; the smoke's **color** alpha is how smoky it is; the
**coupling** on each Wind Force is how much that layer cares about the wind.

## Old .particle effects

Effects made for the old particle system (bevy_hanabi, removed) are `.particle`
files, and scenes that used them carry an old `HanabiEffect` component. Both
keep working, in the editor and in an exported game, with nothing to do by
hand. **Your `.particle` files are never changed, moved or deleted.**

**What happens when you open a project that has them:**

1. Every `.particle` in the project that has no `.vfx` of the same name next
   to it gets one: `fire.particle` gets `fire.vfx`. This runs in the
   background once per project open, and the Console lists each file
   converted, with anything that did not carry over as a warning, and a
   summary line. A `.vfx` that is already there (an earlier conversion,
   perhaps edited since, or one you made yourself) is left exactly as it is,
   and is the one used from then on.
2. A scene's old effect component turns into a **Particle System** as the
   scene loads, with the same playing state, pointing at the `.vfx` beside its
   `.particle`. Save the scene to keep the change; until you do, the scene file
   still holds the old component, which converts again on the next load. If
   the old effect had a light, the point light the old system kept on the
   entity is removed, because the converted effect lights the scene itself.
3. An old effect stored inside the scene rather than in a file is written to
   `effects/<name>.vfx` and the entity points at that.
4. Double-clicking a `.particle` opens its `.vfx` in the effect editor.

**Without the editor**, for example a game exported from a project that was
never reopened, the runtime converts each `.particle` in memory when it loads,
so it plays the same. A Particle System's **Effect** can point at a `.particle`
directly for the same reason.

The old component's rate multiplier, scale multiplier, colour tint, time scale
and variable overrides are not carried over: the old system stored them but
never applied them, so carrying them over would change how the scene looks.

From code, `renzora_particles::convert` turns an old definition into a
`VfxAsset` (`convert_text`, `convert_definition`), and
`renzora_particles::legacy` holds the `.particle` asset loader and the scene
component's migration.

You can also convert by hand:

| From | What happens |
|---|---|
| **Assets panel**, right-click a `.particle` > **Convert to new particle system** | Writes the `.vfx` and opens it in the effect editor. |
| **Assets panel**, right-click a folder (or empty space in it) > **Convert all .particle files here** | Converts every `.particle` directly in that folder; a toast sums up. |
| **The Effect slot** of a Particle System, drop a `.particle` | Converts it (or reuses the earlier conversion) and assigns the `.vfx`. |

Dragging a `.particle` into the viewport places a Particle System that plays
it, and points at its `.vfx` once there is one.

**Names.** The automatic conversion only ever writes `<name>.vfx`, and only
when nothing has that name. The manual ones write `<name>.vfx` too; if that
name is taken by a
different effect it writes `<name> (converted).vfx` instead (then
`(converted 2)`, and so on), so nothing you made is ever overwritten. If the
name already holds this exact conversion it is reused, so converting twice, or
dropping the same `.particle` on two slots, makes one file.

**Warnings.** Anything that has no equivalent yet, or is only approximated, is
listed: in the Console (one warning per item), in the toast (a count), and as
comments at the top of the `.vfx`, so the list stays with the file. Nothing is
dropped without a line saying so.

**A whole project at once** is a command-line tool:

```sh
cargo run --profile dist -p renzora_particles_editor --example convert_particles -- <dir>
```

It walks `<dir>` and its subfolders, converts each `.particle` to a sibling
`.vfx`, and prints each file's warnings and a summary. A `.particle` whose
`<name>.vfx` already exists is skipped (and listed as skipped): a batch run
never touches an effect that may have been edited since.

### What converts

The converted effect has one emitter whose graph does what the old effect did
on screen. The graph is laid out in columns flowing into the Particle Output.

| Old field | Becomes |
|---|---|
| Capacity, simulation space | The emitter's capacity and space. |
| Spawn mode Continuous (rate, duration, cycles) | `spawn_rate`; a duration with cycles becomes a one-shot of their total length, otherwise the effect loops. |
| Single Burst | `burst_count` once at the start; a one-shot effect. |
| Repeated Bursts (bursts per second, cycles) | `burst_count`, `burst_interval` and `burst_cycles`; looping when the cycles are unlimited. |
| Lifetime min / max | A **Random Range** into `lifetime`. |
| Emission shape | **Point**, **Sphere** (surface or volume), **Box** and **Rectangle** nodes; a 3D **Circle** is turned into the XZ plane it used to lie in; a **Cone** is a disc scaled from base to top radius over its height. In 2D a sphere becomes a circle, as before. |
| Velocity Directional (direction, speed or speed range, spread) | A **Random Vec3** with exactly the old range (the old spread was a box around the direction, not a cone). |
| Velocity Radial / Random | The shape's outward direction times the speed (a random range for min/max). |
| Velocity Tangent (axis) | The unit tangent around the axis times the speed. |
| Acceleration, drag | `gravity` and `drag`. |
| Radial and tangent acceleration | An **Attractor** with negative strength, and a normalised **Vortex**. |
| Noise turbulence (octaves, lacunarity) | **Curl Noise** with the same octaves and lacunarity and its old drift as the scroll (in-plane for 2D). |
| Velocity limit | The output's `max_speed`. |
| Kill zones (sphere, box, kill inside or outside) | **Collide Sphere** / **Collide Box** with Kill On Hit (Inside for a zone that killed what left it; World Space for a world-space effect). |
| Ribbon | `facing: Ribbon` with a `Connect` ribbon in the same number of strips. |
| The effect's point light | A `PerEmitter` [light](#light) with its colour, range, shadows and flicker, as bright as before while the effect is in full flow. |
| Conform to sphere | **Conform To Sphere**, every field as it was (the node is Hanabi's modifier). |
| Attractors | **Conform To Sphere** each, which is what the old builder made of them (with Hanabi's default shell and sticky factor). One with a negative strength becomes an **Attractor** push, with a warning. |
| Orbit | **Orbit** at the old angular speed, on the old ring when it had a radial pull. The old one kept adding spin every frame and never let go of it, so particles wound up for as long as they lived; the converted one holds the speed the field asked for. |
| Size start / end, size curve, non-uniform size, random start size | A **Curve** (or a **Lerp** for non-uniform) into `size_over_life`; a random start size becomes a random multiplier. |
| Colour gradient, flat colour, blackbody | A **Gradient** into `color_over_life` (blackbody is the same temperature ramp), or a flat `color`. |
| HDR intensity | The output's `emission`, so the gradient keeps the colours the old editor showed and the glow is one number. |
| Rotation speed | `angular_velocity`. |
| Alpha mode / blend mode | `Additive`, `Premultiplied` or `Alpha` blending; Mask becomes `AlphaMask` with the mask threshold as `alpha_cutoff`, Opaque becomes `Opaque`, and Multiply (either setting) becomes `Multiply`. |
| Screen-space size | `screen_space_size: true`, in the same physical pixels. |
| Orient mode / billboard | `VelocityAligned`, `FixedAxis` (face camera, Y up) or `CameraBillboard`. |
| Texture, flipbook | The texture, and a sprite sheet played over each particle's life. |
| Erosion | The render `erosion` (0.6). |

The old format has no wind, so a converted effect ignores the world wind until
you add a Wind Force.

These have no equivalent yet and are dropped with a warning: noise octaves
beyond eight, a render layer, a fixed (non-facing) billboard, simulate only
when visible, motion integration modes, starting inactive, a cone's surface-only
emission, and exposed variables.

## Saving and hot reload

A scene stores the effect's **path**, never the effect itself, so editing a
`.vfx` updates every entity that uses it. Saving the file in the editor or in
an external text editor reloads it at once, and a running effect keeps its
place rather than restarting.

## From code

The components are `ParticleSystem` (which effect) and `ParticlePlayback` (how
it plays):

```rust
use renzora_particles::{ParticlePlayback, ParticleSystem};

commands.spawn((
    ParticleSystem::from_path("effects/explosion.vfx"),
    Transform::from_xyz(0.0, 1.0, 0.0),
));

fn on_hit(mut q: Query<&mut ParticlePlayback>) {
    for mut playback in &mut q {
        playback.restart();
        playback.burst(30);
    }
}
```

`ParticlePlayback` has `play`, `pause`, `stop`, `restart`, `burst(count)` and
`seek(seconds)`; `time_scale` and `one_shot` are fields. `ParticleStats` on the
same entity reports how many particles are alive and how many spawns were
dropped for lack of capacity.

## Scripting

| Function | |
|---|---|
| `vfx_play()` | Play, or resume after a pause. |
| `vfx_pause()` | Freeze where it is. |
| `vfx_stop()` | Clear every particle and stop. |
| `vfx_restart()` | Clear and play from the beginning. |
| `vfx_burst(count)` | Spawn `count` extra particles from every emitter now. |
| `vfx_set_time_scale(scale)` | Play faster or slower. |
| `vfx_set_param(name, value)` | Set the effect's [Parameter](#parameters) nodes called `name` to a number. |
| `vfx_set_param_vec3(name, x, y, z)` | Set them to a vector. |
| `vfx_set_param_color(name, r, g, b, a)` | Set them to a linear RGBA colour. |
| `vfx_clear_param(name)` | Give them back their own value (or their Timeline keys). |
| `vfx_set_param_on(entity, name, value)` | `vfx_set_param` on the entity named `entity`. Also `vfx_set_param_vec3_on(entity, name, x, y, z)` and `vfx_set_param_color_on(entity, name, r, g, b, a)`. |

Each acts on the script's own entity if it has a particle system, and otherwise
on its direct children that do, so a script on a character can fire an effect
parented under it. The `_on` forms find the entity by its name (as the
Hierarchy shows it) and then do the same there; an empty name is the script's
own entity. A name no Parameter node has is ignored.

```lua
local t = 0

function on_update(dt)
    if is_key_just_pressed("Space") then
        vfx_restart()
        vfx_burst(50)
    end
    -- A campfire with a Parameter node named "intensity" on its spawn rate.
    t = t + dt
    vfx_set_param("intensity", 20 + 10 * math.sin(t))
    vfx_set_param_color_on("Torch", "tint", 1.0, 0.4, 0.1, 1.0)
end
```
