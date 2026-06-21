# Render Composition

> Status: **design + initial implementation** (Bevy 0.19). This is the
> centralized, data-driven render-pass ordering layer that replaces the per-crate
> ad-hoc `.before(other_system)` coupling left over after Bevy removed the render
> graph.

## Why this exists

Bevy 0.18 ordered render passes with an explicit **render graph** (nodes + edges
+ sub-graphs + its own executor). Bevy 0.19 **deleted** that and moved to plain
ECS systems ordered by `SystemSet`s (`Core3dSystems` = Prepass → MainPass →
EarlyPostProcess → PostProcess). That's more debuggable, but it left renzora with
no single source of truth for "what order do the ~50 view-target passes run in."

The symptom: any two passes that ping-pong the view target via
`post_process_write()` in the same `Core3dSystems` phase have **undefined** order
→ they scramble each other's ping-pong chain. Lumen GI vs bevy's TAA produced an
SDF grey screen and SSGI flicker exactly this way. The naive fix —
`renzora_lumen` doing `.before(temporal_anti_alias)` — hard-couples Lumen to
bevy's TAA and doesn't scale to N passes.

## What this is (and isn't)

This is **not** a re-implementation of the render graph. There are no `Node`
objects, no edge wiring, no sub-graphs, no separate executor. Passes stay plain
systems / handlers. What this adds is a **data-driven ordering registry** plus a
small dispatcher — the order lives in a resource (data), not in scattered Rust
`.before/.after` calls.

Because the order is **data**, it is the substrate a future *render-pipeline node
editor* can edit: reorder renzora passes, toggle them, and splice in custom WGSL
passes — without recompiling. Bevy's own built-ins (TAA, tonemapping, bloom,
FXAA/SMAA) are compiled systems that cannot be reordered at runtime, so they are
**fixed anchors** at known phase boundaries; users compose the flexible renzora
passes *between* those anchors.

## Data model (the node-ready core)

```
RenderPhase   // coarse, ordered stages — the fixed skeleton
  Gi          // HDR/linear: GI composite, SSR     (EarlyPostProcess, BEFORE TAA)
  ── anchor: bevy temporal_anti_alias ──
  HdrPost     // HDR: bloom, DOF, motion blur       (after TAA)
  ── anchor: bevy tonemapping ──
  LdrPost     // LDR: color grade, vignette, …      (PostProcess, after tonemap)
  ── anchor: bevy fxaa / smaa ──
  Overlay     // debug overlays, gizmo composites   (last)

RenderPassEntry          // one registered pass = DATA
  id:      &'static str  // stable id for UI + reorder
  phase:   RenderPhase   // which stage
  order:   f32           // sort key within the phase (user-editable later)
  enabled: bool          // global toggle (per-ENTITY enable stays component-driven)
  handler: Box<dyn RenderPassHandler>

RenderComposition (render-world Resource)
  passes: Vec<RenderPassEntry>   // sorted by (phase, order)

trait RenderPassHandler {
  fn execute(&self, world, ctx: &mut RenderContext, view_target: &ViewTarget, view: Entity);
}
```

## Execution

One **dispatcher system per `RenderPhase`**, each placed in the matching
`Core3dSystems` set and ordered around the bevy anchors with system sets:

```
EarlyPostProcess:  dispatch(Gi)  →  [temporal_anti_alias]  →  dispatch(HdrPost)
PostProcess:       [tonemapping] →  dispatch(LdrPost)  →  [fxaa/smaa]  →  dispatch(Overlay)
```

Each dispatcher iterates `RenderComposition.passes` filtered to its phase, in
`order`, and calls `handler.execute(...)` — only for passes whose per-entity
component is present on the view (same gate `unified_post_process` already uses).
Reordering = re-sort the `Vec` (a resource mutation), no schedule rebuild.

This **generalizes the existing `unified_post_process`** (which already iterates
LDR post-process handlers in registry order) from one phase to all phases.

## Registration

A pass joins the composition by intent, never referencing another pass:

```rust
app.add_render_pass(RenderPassDesc {
    id: "lumen.gi",
    phase: RenderPhase::Gi,
    order: 0.0,
    handler: ...,   // or a #[derive]-driven handler for PostProcessEffect types
});
```

The crate that owns a bevy built-in binds that built-in to a phase boundary in
**one** place (e.g. `renzora_antialiasing` anchors `temporal_anti_alias` between
`Gi` and `HdrPost`). No other crate imports it.

## Adoption status

**Every renzora-owned view-target pass is on the framework** (or correctly
anchored). What runs where:

| Pass | Where | Notes |
|---|---|---|
| All `PostProcessEffect`s (vignette, color grade, underwater, …) | `LdrPost` | auto-registered by `PostProcessPlugin<T>` |
| Lumen GI / SSR | `Gi` | voxel chain + trace; before TAA |
| Lumen voxel debug | `Overlay` | post-AA debug |
| `renzora_rt` (SSGI) | `Gi` | alt GI backend, mutually exclusive with Lumen |
| `renzora_viewport` debug-viz | `Overlay` | post-AA debug |
| bevy TAA / tonemapping / FXAA / SMAA | — | **anchors**: the framework positions the phases around these (one place) |
| bevy bloom / DOF / motion-blur (via `renzora_*` toggles) | — | bevy built-ins at bevy's own placement (don't clash; left as anchors) |
| `renzora_ssao` | bevy `Prepass`→`MainPass` | not post-process; bevy-correct |
| `renzora_ssr` | bevy `MainPass` (deferred) | not post-process; bevy-correct |
| `bevy_mod_outline` (selection) | bevy `PostProcess`, before FXAA | **vendored** crate — stays decoupled from renzora, anchored to bevy's own tonemapping/fxaa/smaa (same anchors the framework uses) |

The LDR order is `tonemapping → LdrPost → fxaa/smaa → Overlay` (the framework
brackets the AA passes).

## Not yet (future)

- A node-editor panel (built on `node_graph_view`) that edits `RenderComposition`
  — reorder, toggle, and insert custom WGSL passes at runtime.
- If renzora ever wants its own HDR passes between TAA and tonemapping, that's
  the (currently empty) `HdrPost` phase, ready to receive them.

## Notes / constraints

- Bevy's schedule is static: runtime reordering is possible **only** through the
  dispatcher (one system running passes in data order). Passes that can't be
  expressed as a `RenderPassHandler` (the bevy built-ins) are fixed anchors.
- Per-entity enable/disable, add-to-any-entity, and `EffectRouting` are unchanged
  — this layer only owns **ordering**, which was the missing half.
