# Planar Reflections

A **Planar Reflection** turns a flat surface into a real mirror: a wall mirror,
a shop window, a polished floor, a puddle, a still lake. It is Renzora's
equivalent of Unreal's Planar Reflection actor.

The engine renders the scene a second time, from your camera mirrored across
the surface, and lays that image over the surface's own material. Because it is
an ordinary render of the scene, everything that is in the scene shows up in
it: particles, glass, smoke, characters behind you, things off the edge of the
screen.

## Quick start

- **Add Entity > Rendering > Mirror** spawns an upright 2 x 2 mirror standing on
  the ground, facing +Z, reflecting fully from every angle.
- Or select any flat mesh and **Add Component > Rendering > Planar
  Reflection**. Set **Normal Axis** to the mesh's face normal: **Y** for the
  engine's Plane primitive (floors, water, puddles, and the Mirror preset, which
  is that plane stood upright), **Z** for a mesh modelled facing +Z, as most
  imported mirror and window meshes are.

With **Fit to Mesh** on (the default) the reflection plane is found from the
mesh itself, so the entity's origin can be anywhere. With it off, the plane
passes through the entity's origin.

The surface keeps its own material. A window stays glass with a reflection on
it, a car door stays painted metal; the reflection is blended on top with the
strength, tint and fresnel you set.

## Planar reflection or SSR?

| | Planar Reflection | SSR |
|---|---|---|
| Shows what is off screen or behind the camera | Yes | No |
| Shows particles, glass, smoke, anything transparent | Yes | No |
| Works on any surface shape | Flat surfaces only | Any surface |
| Cost | One extra scene render per visible mirror | One screen-space pass for the whole view |
| Needs the deferred renderer | No | Yes |

Use a planar reflection where the reflection is the point: mirrors, windows you
look at head on, a marble floor in a hall, a calm water surface. Use SSR for the
general sheen on many surfaces at once (wet streets, glossy props), where
missing off-screen content goes unnoticed.

Solari's ray-traced GI does not reflect particles or transparent objects
either: they are not in its ray-tracing scene. A planar reflection does, so a
campfire's sparks and smoke appear in a mirror beside it. The engine's particle
system draws camera-facing quads that turn toward whichever camera renders
them, so in the reflection they face the mirrored camera and look right.

## Settings

| Field | Default | What it does |
|---|---|---|
| Normal Axis | Y | Which local axis is the surface normal (see above). |
| Flip | Off | Reflect toward the other side of that axis, for a mesh whose axis points into the surface. |
| Fit to Mesh | On | The plane is found from the mesh's own triangles instead of the entity's origin and axis: its largest flat sheet within 60 degrees of Normal Axis, at that glass's exact angle, facing the way the glass faces. Normal Axis then only says which glass to look for, so a window wall turned off the model's axes still reflects straight. For a merged mesh holding windows on several walls, that is the wall with the most glass. When the mesh's geometry is not readable it falls back to the middle of the mesh's bounds. Imported models often keep a window's vertices metres from its origin, so with this off the scene is mirrored about the wrong place. |
| Plane Offset | 0 | Moves the plane along its normal, in metres, after fitting. |
| Resolution Scale | 0.5 | Capture resolution as a fraction of the viewing camera's, 0.25 to 1. The main cost knob: 0.5 is a quarter of the pixels. |
| Strength | 1.0 | How much reflection is laid over the material, 0 to 1. |
| Tint | White | Multiplied into the reflection. Dark for smoked glass, coloured for metal. |
| Fresnel | 0.5 | How much more it reflects at grazing angles. 0 reflects the same from every angle (a mirror); 1 follows the curve of water and glass, almost transparent head on and a full mirror at grazing. |
| Blur | 0 | Softens the reflection for frosted, wet or rough surfaces, 0 to 1. |
| Clip Offset | 0.02 | How far in front of the surface the capture starts, in world units. Stops the surface reflecting itself (shimmer and acne); objects touching the mirror lose that sliver. |
| Max Distance | 100 | Beyond this distance from the camera the surface shows only its material. 0 means no limit. |
| Update Every | 1 | Re-render every N frames. 2 halves the cost; the reflection then trails fast motion by a frame. |

The header toggle turns the reflection off without removing the component.

**One flat surface per component.** Select the surface and the viewport draws the plane it reflects about and an arrow toward the side it reflects. Red means the entity has no mesh. Amber means the mesh is not one flat surface, for example the merged glass of a whole building front with windows on several walls, and the Console says so. Fit to Mesh then reflects about the wall with the most glass, and the glass on every other wall reflects wrongly. For a correct result use a mesh that holds just the one window or floor, or lay an **Add Entity > Mirror** plane over the glass you want reflecting.

Planar reflections are saved with the scene like any other component.

## Cost

Every mirror that renders costs **one extra render of the scene**, from the
mirrored camera, at **Resolution Scale** of your camera's resolution. That render
includes a depth and normal prepass and its own directional-light shadow
cascades (point and spot light shadows are shared). The overlay drawn over the
surface is cheap by comparison: one transparent pass over the surface's pixels,
plus twelve texture reads per pixel where Blur is above 0.

A mirror costs nothing while it:

- is off screen,
- faces away from the camera (a mirror reflects from its front only; from
  behind you see its material),
- is beyond its Max Distance,
- is switched off.

At most **4** mirrors render at once, nearest first; the rest show their
material until they are among the nearest. In the editor each docked viewport
is its own camera with its own reflections, and the limit covers them all. The
limit is the `PlanarReflectionSettings` resource (`max_active`), which a game
can raise or lower.

Ways to keep it cheap:

- Lower **Resolution Scale** for floors and water, where the reflection is seen
  at an angle and blurred by distance. 0.25 is often enough.
- Set **Update Every** to 2 or 3 for mirrors the player does not stare into.
- Give each mirror a sensible **Max Distance**, so a mirror at the end of a
  long corridor costs nothing until you walk up to it.

## How it works

- The mirror camera is your camera reflected across the plane, with the same
  field of view. Its projection uses an **oblique near plane** (Lengyel's
  technique): the near clipping plane is the mirror itself, so nothing behind
  the mirror is drawn into the reflection, and frustum culling skips those
  objects altogether.
- It renders HDR with tonemapping off. Your camera's main pass blends the
  reflection in and then tonemaps the whole image once, so bloom, exposure and
  colour grading apply to the reflection exactly as they do to everything else.
- The mirror camera renders the scene's layers only, never the editor's grid or
  gizmos, and it is excluded from per-camera effects (bloom, ambient
  occlusion, anti-aliasing, Solari).
- Mirrors do not recurse: a mirror seen in another mirror shows its material.

## Limitations

- Flat surfaces only. A curved surface reflects as if it were the plane through
  its origin.
- Perspective cameras only; an orthographic camera shows the material.
- In the reflection the scene is lit by ordinary rasterized lighting, so with
  Solari on, the reflection does not carry ray-traced GI.
- The sky in the reflection is the baked environment cubemap, as in the
  editor's secondary viewports, rather than the live atmosphere pass.
- Gaussian splats do not appear in reflections.
