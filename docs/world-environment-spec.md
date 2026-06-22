# WorldEnvironment — architecture spec

> Locked design for unifying the scattered environment/render-effect system into
> one owned, crash-safe model. Internal spec; not published. Derived from the
> rendering-architecture investigation (atmosphere toggle crashes, the multi-writer
> flicker, the mesh-view bind-group layout-lock).

---

## 1. The core constraint (why this exists)

Bevy's PBR **mesh-view bind group** (`@group(0)`, bindings 0–38 + the `@group(1)`
array) is the **one shared, toggle-sensitive layout** — every mesh draw, the
prepass, and deferred lighting are all specialized against it. Removing a binding
from it at runtime (by removing the component that gates it) restructures a layout
that live pipelines depend on → wgpu crash. This is the root of every environment
crash (atmosphere binding 31–33, contact shadows 16, etc.).

**Rule that follows:** features that couple *into* that shared group must be
**resident** (attached once at spawn, never removed) and **gated** (work skipped +
neutral resource bound when "off"). The layout never changes → no crash. Features
that run as their **own pass** (independent layout) can be added/removed freely.

---

## 2. The three buckets (ownership — LOCKED)

```
LIGHTS — scene entities you place
  Sun (DirectionalLight + Sun) · point · spot
  └─ contact shadows = a light's shadow feature (lives on the Sun)

WORLD ENVIRONMENT — ONE entity, the world's look (shading-coupled, resident+gated)
  background: Color | Procedural(atmosphere) | Skybox   ← enum, not separate components
  ambient / IBL · SSAO · SSR · fog · GI / irradiance
  (transform-free / planet-center, so it can host `Atmosphere` directly)

CAMERA / IMAGE QUALITY — per-camera (independent passes / lens attributes)
  tonemapping · exposure (EV100) · DOF · motion blur · TAA / FXAA / MSAA
```

Key consequences:
- **Sun is its own entity**, NOT on the WorldEnvironment. Reason: one entity has
  one `Transform`; the sun needs it as a *rotation* (to aim the light), the
  atmosphere needs the host's `GlobalTransform` as a *no-rotation planet center*.
  Splitting the sun out lets WorldEnvironment host `Atmosphere` directly and
  **dissolves the hidden `AtmospherePlanet` + `sync_atmosphere` babysitting**.
  Coupling stays by *reference* (atmosphere reads the directional light's
  direction for the sun disk/scatter).
- **Background is an enum** (Color / Procedural / Skybox) — kills the
  skybox-vs-atmosphere "both present, both rendering" fight by construction.
- **TAA/tonemapping/exposure/DOF are camera-bucket, not WorldEnvironment.** They
  describe a camera's image, not the world. (renzora bundles `TaaSettings` etc.
  on the World Environment today — that's the conflation being removed.)

---

## 3. Residency contract (the one rule the reconcile enforces)

For every WorldEnvironment feature:
1. Its component/binding is attached **once at camera spawn and NEVER removed.**
2. **"Off"** means two safe things: **(a)** bind a *neutral* resource so the
   shader sampling that slot produces no effect, and **(b)** **skip the work
   passes**. The layout is identical on/off → no re-specialization → no crash.
3. A single `reconcile_world_environment` system is the **only writer** of
   camera-side environment state. It reads the WorldEnvironment's per-section
   enables and flips each feature's gate. It replaces the ~15 independent
   per-effect sync systems and their 4 disagreeing teardown strategies.

### Per-feature gate manifest

| Feature | mesh-view binding | "Off" = neutral resource + skip | Status / difficulty |
|---|---|---|---|
| IBL / env map | grp1 0–2 | **already done** — `gate_environment_generation` (the template) | ✅ reference impl |
| Distance fog | 13 | uniform resident, density 0 / disabled flag (no pass) | 🟢 easy — **slice 1** |
| SSAO | 17 | bind white (no-occlusion) tex, skip passes | 🟡 medium |
| SSR | 15 | bind black (no-reflection), skip passes | 🟡 medium |
| Irradiance volumes | grp1 3–4 | bind neutral, skip if unused | 🟡 medium |
| Atmosphere | 31–33 | neutral LUT + skip LUT/sky passes + decouple visible sky into its own pass | 🔴 hard — last |
| Contact shadows | 16 | `seed_contact_shadows_offset` keeps specialization in lockstep | 🟡 nearly done (Sun-side) |

Camera bucket (tonemapping/exposure/DOF/AA) gate per-camera as independent passes —
not part of this contract.

---

## 4. Out of scope (do NOT touch)

- **Layer B** — `RenderPhase` / `RenderComposition` pass ordering. Sound; leave it.
- **Prepass attachment list** (depth/normal/motion/deferred) — already resident at
  spawn; keep that pattern.
- **The ~50 `#[post_process]` shader effects** — independent passes, stay modular
  (open marketplace set).
- **cfg-gated bindings** (area lights 35–36, DFG 37–38, STBN 34) — compile-time,
  always present, never a runtime concern.

---

## 5. Open questions (decide as we go, not blocking slice 1)

- **EffectRouting's future.** Slice 1 reuses it to *find* target cameras; longer
  term the reconcile may own camera-side state directly and routing shrinks to
  "which cameras are environment targets."
- **BSN scene migration.** Existing scenes have a 15-component "World Environment"
  entity. Need a load-time migration: old separate components → one
  `WorldEnvironment` (+ split the Sun onto its own entity). Full migration lands
  with the atmosphere slice; slice 1 can handle the default/new-scene case.
- **Sun split timing.** Not needed until the atmosphere slice (that's what it
  unblocks). Slices 1–2 leave the Sun where it is.

---

## 6. Build order (slices)

1. **Slice 1 — skeleton + fog** (this slice). Lowest risk, proves the whole
   architecture end-to-end. No sun split, no atmosphere.
2. **Slice 2 — SSAO + SSR.** Proves the "skip pass + placeholder texture" gate.
3. **Slice 3 — atmosphere + sun split + background enum.** Hardest; uses
   everything learned + the IBL template; lifts the visible sky into its own pass.

---

## 7. Slice 1 — concrete scope

**Goal:** validate the WorldEnvironment skeleton, the single-reconcile-writer
pattern, the resident-and-gate mechanism, and the unified inspector — on the
safest feature (fog), with nothing that can crash.

**Tasks:**
1. **`WorldEnvironment` component** in the `renzora` contract crate (crosses the
   dylib boundary). Slice 1 fields: `fog: FogSection { enabled, color, density,
   directional_* }`. Stub the future sections (`background`, `ibl`, `ssao`, …) as
   commented placeholders so the shape is visible but unbuilt.
2. **`reconcile_world_environment` system** — the single writer. For fog: ensure
   `DistanceFog` is resident on the target cameras (never remove it); when the fog
   section is enabled, copy its params; when disabled, set density 0 / disabled.
   Reuse `EffectRouting` to find target cameras for now.
3. **Camera spawn:** attach `DistanceFog` (disabled/0-density) at spawn so binding
   13 is always in the layout, regardless of WorldEnvironment state.
4. **Inspector:** one "World Environment" panel with a **Fog** section + enable
   toggle (reuse the section enable-toggle from the inspector expand/disable work).
   Fog stops being a separately-addable component.
5. **Retire the standalone fog path:** `reconcile_world_environment` owns fog;
   remove `renzora_distance_fog`'s insert/remove sync (the toggle that changed the
   layout).

**Acceptance criteria (GPU-verifiable — `renzora run`):**
- Toggling the Fog section on/off repeatedly **never crashes** and **never
  flickers**.
- Fog renders correctly when on, fully absent when off.
- Binding 13 stays resident in the layout the whole time (no re-specialization on
  toggle).

**Explicitly NOT in slice 1:** atmosphere, the sun split, the background enum,
SSAO/SSR, BSN migration of old scenes, removing EffectRouting.
