# Gaussian Splatting

Renzora renders **3D Gaussian splats** — photorealistic point-cloud captures
produced by 3DGS training pipelines — through the
**`renzora_gaussian_splatting`** distribution plugin, which wraps the vendored
[`bevy_gaussian_splatting`](https://github.com/mosure/bevy_gaussian_splatting)
renderer. Drop a scanned scene or object into your level and it renders with
full view-dependent lighting baked into the capture, no meshes or materials
involved.

## Supported formats

| Extension | What it is |
|---|---|
| `.ply` | The de-facto 3DGS capture format (as exported by the original INRIA pipeline, Polycam, Luma, gsplat, etc.) |
| `.gcloud` | `bevy_gaussian_splatting`'s compact binary cloud format |
| `.sog` | PlayCanvas/SuperSplat's compressed bundle (Spatially Ordered Gaussians, ~15–20× smaller than PLY) — the default SuperSplat download |
| `.ssog` | Loaded as a renamed `.sog` bundle (no such format officially exists) |

> **`.ply` is shared with mesh data.** The importer sniffs the PLY header: files
> carrying 3DGS spherical-harmonics properties (`f_dc_0`, …) import as splat
> clouds (copied verbatim), while mesh PLYs (with faces) keep converting to GLB
> like any other model.

**Plain point clouds work too.** A faceless `.ply` that's just colored points
(CloudCompare exports, LiDAR scans, Sketchfab's converted downloads) loads as a
splat cloud as well — the engine synthesizes an isotropic splat per point,
sized from the cloud's point density. Use the component's *Splat Scale* to
fatten or thin the result. Note `.splat` (antimatter15) and `.spz` (Niantic)
files are **not** supported — re-export those as `.ply` or `.sog` (SuperSplat
can). *Streamed* SOG (an unbundled `lod-meta.json` + chunk-directory tree for
web LOD streaming) is also not supported — export a plain bundled `.sog`.

## Getting a splat into a scene

1. **Import** the `.ply` / `.gcloud` file (Import overlay or drag it into the
   Asset Browser). Splat files are copied as-is into your project.
2. **Drag it from the Asset Browser into the 3D viewport.** The drop spawns an
   entity at the cursor's ground position with a **Gaussian Splat** component
   pointing at the file. (Splats are 3D content — there is no 2D-view drop.)

Alternatively use **Add Entity → Gaussian Splat** in the hierarchy and point the
component's *Source* field at a file afterwards.

## The Gaussian Splat component

The scene stores a serializable `GaussianSplat` component — a project-relative
source path plus per-cloud tuning; the plugin resolves it into the live
renderer state at runtime (the same path-in-component pattern models, audio,
and particles use, so scenes stay portable).

| Field | Meaning |
|---|---|
| **Source** | Project-relative path to the `.ply` / `.gcloud` file (accepts asset drag-drop) |
| **Opacity** | Uniform opacity multiplier over every splat (0–1) |
| **Splat Scale** | Uniform size multiplier over every splat — fattens/thins the gaussians without moving them |

Position, rotate, and scale the cloud with the ordinary transform gizmos — the
entity `Transform` applies to the whole cloud.

## How it works

- The plugin ships as a `cdylib` in `plugins/` (Runtime scope): the same code
  renders splats in the editor viewport, in-editor play, and the shipped game.
  Remove it from `plugins/` and `GaussianSplat` components become inert data —
  scenes still load and save cleanly.
- Splats draw only through cameras tagged with the renderer's `GaussianCamera`
  marker. The plugin tags every 3D camera automatically once a scene contains a
  cloud, so editor viewports and game cameras "just work"; isolated utility
  cameras (material preview, thumbnails) are left untouched.
- Splats are depth-sorted per view every frame (GPU radix sort by default).
  Very large captures cost sort time proportional to their splat count — prefer
  `.gcloud` for big scenes and trim captures in your 3DGS tool where possible.
- After the blended color draw, each cloud runs a second depth-only pass that
  writes the solid core of every splat (soft fringes excluded) into the view
  depth buffer. This is what lets depth-reading effects — volumetric fog,
  depth of field — treat splats as real surfaces instead of sky, and lets
  transparent geometry drawn later depth-test against them.

## Limitations

- Splat clouds don't cast or receive engine lighting or shadows — lighting is
  baked into the capture. Standard geometry occludes splats correctly (and vice
  versa) via depth.
- 4D (spacetime / animated) gaussians and 2DGS surfel modes exist upstream but
  are not yet exposed through the editor component.
- Physics does not see splats; add colliders by hand where needed.
