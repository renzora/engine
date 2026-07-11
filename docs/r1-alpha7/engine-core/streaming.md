# Streaming

Streaming is loading and unloading content *while the game runs*, instead of front-loading everything behind a loading screen. Renzora streams at four levels — scene loading, mesh detail (LOD), world content, and texture resolution — and they share one rule: **streaming is active in a running game and in editor Play/Simulate; editor edit mode always keeps the whole world resident** so you can see and edit everything. Dedicated servers never stream (there is no camera, and gameplay needs the full world).

All distance checks measure from the *streaming camera*: the active game camera in a running game or Play mode, and the editor camera in Simulate (streaming follows what you're looking at).

## Async scene loading

`load_scene("scenes/level2.bsn")` from a script no longer stalls the game while the whole scene deserializes and spawns. Scene swaps now stream:

1. The file read and BSN parse run on a background thread.
2. Entities spawn incrementally, a few milliseconds per frame, until the scene is complete.

`SceneLoadState.progress` is a real 0→1 readout during the load (parse ≈ the first 10%, spawning fills the rest), so a loading bar driven by it actually moves. The `SceneLoaded` event still fires exactly once, when the last entity has spawned.

Editor scene opens and the initial boot load stay synchronous — they sit behind loading screens that expect a fully-populated world.

## Mesh LODs

Exports bake simplified variants beside every model: `models/chair_lod1.glb`, `_lod2`, … (each level halves the triangle ratio). The runtime now consumes them: when variants exist for a model — packed in the `.rpak`, or loose beside the `.glb` in your project — the engine spawns them as hidden siblings and tags every mesh with a `VisibilityRange` band, and Bevy switches detail levels by camera distance, per mesh.

Defaults: full detail to 40 m, LOD1 to 100 m, LOD2 to 220 m, abrupt switching, never culled. To tune a model, add the **Mesh LOD** component in the Inspector:

| Field | Meaning |
|---|---|
| enabled (toggle) | Off = always render the base model at full detail |
| LOD1/2/3 Distance | Outer edge of each detail band, in world units |
| Crossfade | Width of the dithered blend between bands. `0` (default) = instant swap. Known issue: non-zero crossfade flashes on the deferred pipeline |
| Cull Distance | Beyond this the model vanishes entirely (0 = never) |

Notes:

- LOD variants are baked at **export** time. To see LODs in the editor, place hand-authored (or pre-baked) `name_lod1.glb` files beside the model — the probe picks up loose files too.
- Only `.glb` models participate; levels must be contiguous (`_lod1`, `_lod2`, …).
- A model with more baked levels than configured distances extends the last band geometrically.

## World streaming

### Streamed scene instances

Any nested scene (a `SceneInstance` root) can stream. Select the instance root and, in its **Scene Instance** section, turn on **Streamed**:

- Within **Load Radius** of the camera, the instance's contents load — asynchronously, through the same budgeted spawner as scene swaps, so crossing a boundary never hitches.
- Beyond **Unload Radius**, the contents despawn. The root entity (with its transform) always stays.
- The gap between the radii is hysteresis; the engine enforces a minimum gap so a boundary instance can't thrash.

This is the building block for open worlds: partition your map into sub-scenes (one per district/interior/region), place them as streamed instances, and only the camera's neighborhood is ever resident. In editor edit mode streamed instances stay expanded for authoring; press Play to watch them stream.

### Terrain chunk streaming

On the terrain's **Terrain** inspector section, enable **Stream Chunks** and set **Stream Radius**. In a running game, chunks whose center is beyond the radius drop their render mesh and physics collider — the heavy memory — while the heightmap data stays resident (it's the authored data, and it's small). Chunks rebuild from their heights as the camera approaches. Stream-out happens one chunk-size beyond the radius, so boundary chunks don't flicker.

## Texture streaming

Every `.rmip` texture is published twice: the full asset and a `#low` subasset holding only the tail of its mip chain (base capped at 256px — a 2048² BC7 drops from ~5.6 MB to ~87 KB of GPU memory). While streaming is active, a background evaluator (2 Hz) checks how close the nearest user of each `StandardMaterial` is:

- nearer than `full_distance` (default 60 m) → full-resolution textures,
- farther than `low_distance` (default 90 m) → `#low` textures; the full-resolution images unload entirely,
- in between → unchanged (hysteresis).

Re-approaching reloads the full texture from disk/rpak; until it lands the material keeps rendering its previous state, so the transition is a brief sharpness pop-in, never a hole. Tune or disable via the `TextureStreamingSettings` resource (`renzora_engine::texture_stream`):

```rust
app.insert_resource(TextureStreamingSettings {
    enabled: true,
    full_distance: 60.0,
    low_distance: 90.0,
});
```

Scope: the five heavyweight `StandardMaterial` slots (base color, normal, metallic-roughness, occlusion, emissive). Custom `GraphMaterial` shader textures and terrain splat layers always stay full resolution.

## What streams when — summary

| Content | Trigger | Unit | Memory reclaimed |
|---|---|---|---|
| Scene swap | `load_scene()` | whole scene, spawned incrementally | old scene despawned |
| Mesh detail | camera distance (per mesh) | `VisibilityRange` band | none (all LODs resident; draw cost drops) |
| Scene instances | camera distance to instance root | instance subtree | entities + their assets |
| Terrain chunks | camera distance to chunk center | mesh + collider | chunk mesh asset + trimesh collider |
| Textures | camera distance to nearest material user | full ↔ `#low` image | full-res GPU texture |
