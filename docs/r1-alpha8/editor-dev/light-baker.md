# Light baker: plan

Status: planned, nothing built yet.

## Why

A scene lit by realtime lights pays for its shadows every frame. Measured on the mcp_test apartment (Solari off, RTX 3060): four shadowed point lights, one shadowed spot and the sun made 26 shadow views. Bevy records about five command buffers per shadow view, so the render thread prepared and submitted 162 command buffers a frame, about 22 ms, for 7.9 ms of GPU work. The editor ran at 40 fps. With those four point lights' shadows off it ran at 58 (the 60 Hz cap), 46 command buffers a frame.

Baking moves the static part of that lighting into textures once. Lights that are baked stop casting realtime shadows on static geometry, bounce light comes for free, and the result runs on any GPU, including in a shipped game, where Solari is not an option on most hardware.

Solari stays the answer for fully dynamic lighting on ray-tracing GPUs. The baker is for everything else.

## Why the companion

A bake is minutes of CPU work over a snapshot of the scene, with progress to report and a result to hand back. That is the companion server's job model exactly:

- It runs off the editor, so a bake never drops a frame and a crash in it takes nothing else down.
- The server has no Bevy and no GPU device, and the baker needs neither: it is a CPU ray tracer over plain triangle data.
- The queue already does progress, cancel, and holding work while Play runs or a profile records.

A GPU baker on Solari's ray-tracing scene could come later as a fast path on ray-tracing GPUs. It would take the same input and write the same files.

## Shape

```
editor                                   companion server (no Bevy)
  Bake button                              light_bake job
  gather static scene  ---- bake file ---->  BVH over every static triangle
  (meshes, UV_1, materials,                  trace texels on all cores
   lights, settings)                         denoise, dilate seams
                       <--- Progress ------  passes / percent
  attach Lightmap      <--- Done ----------  lightmap textures written
  components, reload                         beside the scene
```

1. **Gather (editor).** Everything the baker needs, taken from the ECS at the moment Bake is pressed, written to one binary file in the project's cache:
   - every static mesh instance: world-space positions and normals, lightmap UVs (`UV_1`), its lightmap's texel size, and its material's base colour (a small downsample of the texture, or the constant) and emissive;
   - every light marked Baked or Mixed: kind, position or direction, colour, intensity, range, cone, radius;
   - the sky as an environment (its colour, or a small equirect of the atmosphere);
   - the settings: texels per metre, samples, bounces.
2. **Bake (server).** A `light_bake` job kind in `work::KINDS`:
   - build a BVH over every triangle (a pure-Rust BVH crate; `obvhs` is the candidate, to be confirmed). Plain Rust, no Bevy, so it comes from crates.io like any other plain crate;
   - rasterise each mesh into its lightmap rectangle in `UV_1` space to find each texel's world position and normal (conservative, so thin triangles still get texels);
   - per texel: direct light from each light with a shadow ray, then indirect light by path tracing a few bounces, with the materials' base colour and emissive;
   - progressive passes, so Progress can report a percentage and an early stop still leaves a usable result;
   - denoise (an edge-aware filter keyed on position and normal to start; Open Image Denoise is a later option), then dilate texels past each chart's edge so bilinear filtering never reads black at a seam.
3. **Apply (editor).** On Done, attach Bevy's `Lightmap` (image, `uv_rect`) to each instance, set the baked lights' `affects_lightmapped_mesh_diffuse` to false so they do not light those meshes twice, and turn their shadows off where they only fall on static geometry.

## What has to exist first

- **Lightmap UVs.** A second, non-overlapping UV layout per mesh. `.rvg` already stores `uv1`. Meshes without one get it generated (xatlas is the usual library; its Rust bindings need checking for licence and build). Generated once per mesh asset at import or at first bake, so instances share it; each instance gets its own rectangle in an atlas through `Lightmap::uv_rect`.
- **Virtual geometry draws lightmaps.** `.rvg` models are the default model format, and the VG path currently writes a zero lightmap rectangle for every instance (`renzora_virtual_geometry/src/render/scene.rs`, `lightmap_uv_rect: [0; 2]`). It needs the lightmap bound and sampled.
- **Node-graph materials draw lightmaps.** Bevy applies a `Lightmap` to `StandardMaterial`. Whether `GraphMaterial`'s generated shader picks it up has to be checked; nothing in `renzora_shader` mentions lightmaps today.
- **An HDR texture format.** Lightmaps hold values above 1. `.rmip` has no HDR format; adding BC6H to it (the texture compressor it uses already supports BC6H) keeps lightmaps compressed and loading like every other texture. KTX2 is the alternative.

## Editor workflow

- **Static** on an entity (inherited by its children): it receives a lightmap and casts in the bake. Moving a static object marks the bake stale.
- **Light mode** on every light: **Realtime** (today's behaviour), **Mixed** (baked into static geometry, still lights moving objects and gives highlights), **Baked** (only in the lightmaps).
- **Bake** in the Lighting section of the World Environment, with texels per metre, quality (samples, bounces), and a progress bar fed by the job's Progress. Cancel goes through the queue's existing Cancel.
- Results saved beside the scene (`<scene>.lightmaps/`), referenced from the scene, and loaded with it.

## Moving objects

Lightmaps only cover static geometry. A character walking through a baked room needs the baked light too, or it stands out lit only by the realtime lights. Bevy's `IrradianceVolume` (a 3D grid of baked light) is the piece for that: the same job bakes a grid of probes over the scene's bounds with the same tracer. A later step.

## Order of work

Each step leaves everything working.

1. **Display.** Lightmaps drawn by the VG path and node-graph materials; BC6H in `.rmip`. Testable with a lightmap baked in Blender.
2. **Lightmap UVs.** Generation for meshes without `UV_1`, stored in `.rvg`; per-instance atlas rectangles.
3. **The baker.** The `light_bake` job in the server: gather, BVH, direct light, bounces, denoise, dilate. A command first, the UI after.
4. **Workflow.** Static flag, light modes, the Bake button and progress, stale detection, saving with the scene.
5. **Irradiance volumes** for moving objects.
6. **GPU fast path** (optional): the same bake on Solari's ray-tracing scene when the GPU can ray trace.

## Open questions

- **Bake time.** A CPU path tracer over a room is expected to take minutes at good quality. To be measured on the apartment once step 3 runs; that number decides whether the GPU fast path is worth doing.
- **Specular.** Lightmaps are diffuse only. Mixed lights keep realtime highlights; fully Baked lights lose them. Directional lightmaps (storing a dominant direction per texel) would bring some back at twice the memory.
- **Lightmap memory.** At 20 texels per metre a room's surfaces are a few million texels; at BC6H's one byte per texel that is a few MB, which is fine. Large outdoor levels need a lower density or streaming.
- **Terrain.** Terrain chunks have no `UV_1`; their own UVs are already non-overlapping and could be used directly.
