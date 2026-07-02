# 2D Lighting

Renzora ships dynamic **2D lighting** — point lights, shadow-casting occluders,
soft shadows, occlusion z-sorting, light banding, and normal maps — as the
optional **`renzora_light2d`** distribution plugin. It wraps the vendored
[bevy_firefly](https://github.com/PVDoriginal/firefly) crate and integrates it
with the editor: everything is authored as ordinary scene data and saved in the
scene file like any other component.

## Quick start

1. **Turn lighting on.** Select your scene's 2D camera — every 2D camera has a
   built-in **2D Lighting** section in the inspector. Flip its toggle on. This
   is the master switch: `Ambient Brightness` is how lit the scene is with no
   lights around (0 = pitch black outside light radii). Toggled off, the
   section costs nothing — no lightmap passes run at all.
2. **Add lights.** Add Entity → 2D Nodes → **Point Light 2D**, or add the
   *Point Light 2D* component to any existing entity. Position it with the
   normal move gizmo; tune `Radius`, `Intensity`, `Color`, and the light core in
   the inspector.
3. **Add shadow casters.** Add Entity → 2D Nodes → **Occluder 2D**. Occluders
   block light and cast shadows; pick a `Shape` (circle, rectangle, rounded
   rectangle, capsule) and size it in the inspector. `Opacity` below 1 lets
   light bleed through for colored, semi-transparent shadows.

The editor viewport previews lighting live: the editor's 2D camera mirrors the
scene camera's *2D Lighting* config (and falls back to a neutral full-ambient
preview when the scene has lights but no configured camera yet, so a freshly
placed light is visible immediately). In-editor play and the exported game use
the config authored on the scene camera.

## Point lights

| Field | Meaning |
|---|---|
| Color / Intensity | Tint and strength of the light. |
| Radius | Outer range — nothing is lit beyond it. |
| Core Radius / Core Boost | The bright inner core. Soft shadow width is derived from the core radius. |
| Falloff / Falloff Intensity | `Inverse Square` (physical), `Linear`, or `None` (constant), with a speed tweak. |
| Inner / Outer Angle | Constrain the light to a cone (flashlight). Direction follows the entity's **up** axis — rotate the entity to aim it. 360/360 = omnidirectional. |
| Cast Shadows | Whether occluders block this light. Shadow casting is the expensive part — turn it off for cheap fill lights. |
| Offset | Lets the light sit off the entity's origin without a child entity. |

## Occluders

Occluders are invisible shapes that block light — pair them with your sprites.
`Shape` edits cover the round shapes; **polygon and polyline occluders** are
built in code or scripts with `Occluder2d::polygon(...)` / `polyline(...)`
(arbitrary concave outlines). The enable toggle on the component header maps to
`Occluder2dEnabled`, so scripts can flick shadows on/off cheaply.

With **Z Sorting** enabled (on both the camera config and the occluder),
shadows only fall on sprites *below* the occluder's z — the standard top-down
setup where a character's shadow darkens the floor but not the character.

## Camera config reference

| Field | Meaning |
|---|---|
| Ambient Color / Brightness | Base illumination added everywhere. |
| Soft Shadows | Penumbra from each light's core radius; off = hard-edged shadows. |
| Z Sorting | Enables occlusion z-sorting (see above). |
| Light Bands | Greater than 0 quantizes the lightmap into bands for a stylized, cel look. |
| Normal Mode / Normal Attenuation | Enables normal-mapped lighting for sprites: `Simple` (side-scroller) or `Top Down` variants (uses `LightHeight`/`SpriteHeight` to fake the third axis). |
| Lightmap Filtering | Off = point-sampled lightmap, for pixel-art. |

Normal maps are attached in code (`NormalMap::new(handle)` on the sprite — the
image must match the sprite's layout exactly), then activated by setting the
camera config's `Normal Mode`.

## Scripting

The components are reflection-registered, so the generic reflection accessors
work from Lua/Rhai — e.g. `set(entity, "PointLight2d", "intensity", 4.0)` for a
flicker, or toggling `Occluder2dEnabled`. No dedicated script functions yet.

## Under the hood

- **Plugin:** `crates/renzora_light2d`, a Runtime-scope `cdylib` in
  `<exe>/plugins/` — delete the file and the feature is gone; the engine still
  boots and scenes still load (unknown components are skipped).
- **Vendored engine:** `crates/bevy_firefly` (upstream `0.19.0`, pinned by
  commit in its `Cargo.toml` header) with one local patch:
  `#[reflect(Component, Default)]` on the public components so the inspector
  and the reflection-driven scene serializer can see them.
- **Renderer:** firefly renders lightmaps inside Bevy's `Core2d` graph after
  the transparent pass — it composes with the sprite/tilemap renderers and
  works against any render target, including the editor's offscreen viewport.
  WebGPU only (no WebGL2 fallback upstream).
- Selection gizmos in the viewport outline the selected light's range and the
  selected occluder's shape while editing.
