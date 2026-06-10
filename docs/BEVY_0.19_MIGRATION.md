# Bevy 0.19 Migration Plan — Renzora

A forward plan for moving the engine from its current Bevy 0.18 target onto Bevy 0.19: the forced mechanical work to compile, then the feature-driven payoffs that justify the bump.

Status: planning · Current engine target: **Bevy 0.18** · Target: **Bevy 0.19**

> **Update (2026-06-09):** the egui → bevy_ui/ember UI migration is fully complete — there is **no `egui` or `bevy_egui` anywhere in the active workspace** (the editor shell is `renzora_shell` on top of `renzora_ember`, and the markup runtime is `renzora_ember::markup`). That clears the old long-pole dependency gate. The remaining 0.19 blockers are the forced render-graph → systems port, the Parley text migration, and the vendored-crate bumps.

This plan is grounded in the current `crates/` layout: **164 top-level crates** under `crates/` plus **23 nested `editor/` subcrates** (~187 workspace crates), and **six vendored Bevy-ecosystem crates** (`bevy_hanabi`, `bevy_silk`, `bevy_oxr`, `bevy_mod_outline`, `vleue_navigator`, `bevy_hui`).

---

## 0. Migration reality check (read first)

Three things gate everything below: vendored-crate bumps, the render-graph rewrite, and the text-API change. None of them are optional.

### 0.1 Dependency chain (the long pole)

The old long pole was a 0.19-compatible `bevy_egui`. **That is no longer relevant — egui is gone.** The long pole is now the vendored Bevy-ecosystem forks, which all live under `crates/` and must compile against 0.19 before the workspace will build:

| Vendored crate | Wrapped by | Note |
| --- | --- | --- |
| `bevy_hanabi` | `renzora_hanabi` (particles) | Has its own render-graph nodes (`src/graph/node.rs`, `src/render/mod.rs`) — port as part of the bump. |
| `bevy_silk` | `renzora_cloth` | Cloth simulation. |
| `bevy_oxr` | `renzora_xr` (OpenXR) | Own **nested vendored workspace** (`bevy_openxr`/`bevy_webxr`/`bevy_xr`/`bevy_xr_utils`); excluded from the `bevy_*` member globs. |
| `bevy_mod_outline` | `renzora_outline` | Has render-graph nodes (`src/node.rs`, `src/msaa.rs`) — port as part of the bump. |
| `vleue_navigator` | `renzora_navmesh` | Navmesh generation. |
| `bevy_hui` | `renzora_ember::markup` | **Parser only.** ember registers just `bevy_hui::prelude::LoaderPlugin` (the `.html` `AssetLoader` + AST). Must still compile on 0.19, but its runtime (`BuildPlugin`/`CompilePlugin`/etc.) is unused. |

`bevy` itself is shared across the dlopen boundary via `bevy_dylib` (`dynamic_linking` + `prefer-dynamic`), so the host binary, the `renzora_editor` bundle, and every dynamic plugin link **one** `bevy_dylib` and see matching `TypeId`s. Workspace feature unification keeps that guarantee — the only requirement is that **everything is rebuilt against the same 0.19** (see §0.4).

### 0.2 Forced breaking change — Render Graph → Systems

In 0.19 the render-graph node API (`Node` / `ViewNode` + `add_render_graph_node` + `try_add_node_edge`) is removed in favour of ordinary render-world systems ordered relative to the `Core3d` schedule. Every `impl ViewNode` in the workspace must be rewritten as a system. The first-party sites (verified in source):

| File | Node(s) |
| --- | --- |
| `crates/renzora/src/postprocess.rs` | `UnifiedPostProcessNode` — the **single** unified post-process node (small port: one node runs all ~53 effects). |
| `crates/renzora_rt/src/node.rs` | `RtNode` (single-pass SSGI). |
| `crates/renzora_viewport/src/debug_viz.rs` | `DebugVizNode`. |
| `crates/renzora_lumen/src/voxel_cache.rs` | `VoxelClearNode`, `VoxelResolveNode`, `VoxelInjectNode`, `VoxelDebugNode`. |
| `crates/renzora_lumen/src/lumen_trace.rs` | `LumenTraceNode`. |
| `crates/renzora_lumen/src/geometry_voxelize.rs` | `GeometryInjectNode`. |
| `crates/renzora_lumen/src/voxel_downsample.rs` | `VoxelDownsampleNode`. |
| `crates/renzora_lumen/src/screen_reflection.rs` | `ScreenReflectionTraceNode`. |
| `crates/renzora_lumen/src/screen_reflection_blur.rs` | `ScreenReflectionBlurNode`. |
| `crates/renzora_lumen/src/screen_reflection_resolve.rs` | `ScreenReflectionResolveNode`. |

That is **13 first-party `ViewNode` impls** — three standalone plus **ten in `renzora_lumen`** (the GI distribution plugin). The vendored forks (`bevy_mod_outline`, `bevy_hanabi`) carry their own nodes and are ported as part of their 0.19 bump (§0.1).

The shape of the port (pattern, not exact 0.19 symbol names):

```rust
// 0.18 — render-graph node
impl ViewNode for RtNode {
    type ViewQuery = /* ... */;
    fn run(&self, graph, ctx, view, world) -> Result<(), NodeRunError> { /* encode pass */ }
}
// + app.add_render_graph_node::<ViewNodeRunner<RtNode>>(Core3d, RtNodeLabel)
//   .add_render_graph_edges(Core3d, (EndMainPass, RtNodeLabel, Tonemapping));

// 0.19 — render system, ordered in the Core3d schedule
fn rt_render_system(/* SystemParam: render device/queue, pipeline cache, view query */) {
    // encode the same pass via a RenderContext / command encoder
}
// + app.add_systems(Core3d, rt_render_system.after(/* EndMainPass */).before(/* Tonemapping */));
```

The `UnifiedPostProcessNode` port benefits from the unified architecture: it is one node running between `Node3d::Tonemapping` and `Node3d::EndMainPassPostProcessing`, so only one ordering relationship moves.

### 0.3 Forced breaking change — Parley text migration

0.19 swaps Bevy's text stack to **Parley**, which reshapes `TextFont` (font size becomes a `FontSize` enum, fonts a `FontSource`). **Every `bevy_ui` `Text` / `TextFont` construction site must be touched.** Since egui is gone, *all* engine text is now bevy_ui text — there is no "egui text is unaffected" carve-out anymore.

Migrate **`renzora_ember` first**, because it owns the shared font helper that most of the editor renders through:

- `crates/renzora_ember/src/font.rs` — the central `EmberFonts { ui, phosphor, mono }` resource and text helpers used by every ember widget and the editor chrome. Fix this and most call sites follow.
- `crates/renzora_ember/src/markup/loader.rs` — the markup loader spawns one real `bevy_ui` entity per node (`Node`/`Text`/`TextFont`/`TextColor`/...).
- `crates/renzora_ember/src/widgets/code_editor/mod.rs`, `widgets/search.rs`, `icons.rs`, and `editor/` (`lib.rs`, `inspector.rs`).

Then the remaining first-party sites:

- `crates/renzora_game_ui/src/spawn.rs`
- `crates/renzora_shell/src/lib.rs`
- `crates/renzora_hierarchy/src/native/row.rs`, `native/scene_starter.rs`
- `crates/renzora_viewport/src/native_header.rs`
- `crates/renzora_settings/src/native.rs`
- `crates/renzora_editor_framework/src/settings.rs`

> Note: `renzora_ui` is **not** in this list. It no longer constructs text — post-migration it holds only runtime-agnostic editor **data types** (dock tree, document tabs, layouts, toasts, drag payloads, panel registry). The active shell is `renzora_shell` on `renzora_ember`.

### 0.4 Forced housekeeping — ABI rebuild

Two ABI guards protect the dlopen boundary, and a bevy bump trips both:

1. **`plugin_bevy_hash()`** (`crates/renzora/src/plugin_meta.rs`) returns `transmute(TypeId::of::<bevy::ecs::world::World>())`. The loader (`dynamic_plugin_loader`) rejects any plugin whose hash differs from the host's. The `World` `TypeId` changes when bevy changes, so this flips automatically — every plugin and the editor bundle must be rebuilt.
2. **`RENZORA_BUILD_HASH`** — `build.rs` emits an FNV-1a hash of `"{version}-{rustc}-bevy0.18"`. The bevy version is a **hardcoded literal**:

```rust
// build.rs:17 — bump the literal as part of the migration
let hash_input = format!("{pkg_version}-{rustc_ver}-bevy0.18"); // -> bevy0.19
```

Change `bevy0.18` → `bevy0.19` so the build hash distinguishes 0.18-built artifacts from 0.19-built ones. There is **no `rust-toolchain.toml`** — the Rust version lives only in `docker/Dockerfile` (`FROM rust:1.93.0-bookworm`); if the 0.19 bump requires a newer rustc, update it there.

Then do a clean `--workspace` rebuild (`cargo build-all`) so the binary, the `renzora_editor` bundle, the shared `bevy_dylib`, and all dlopen plugins land on one matching ABI.

Budget the migration as: **bump vendored forks + port render-graph nodes + migrate text + ABI rebuild**, then harvest the payoffs below.

---

## Tier 1 — Highest leverage (closes real gaps / removes heavy code)

### T1.1 Serializable asset handles → delete the string-path workaround

**0.19 feature:** `HandleSerializeProcessor` / `HandleDeserializeProcessor` (store a handle's asset path on serialize, reload on deserialize).

**Current state:** `crates/renzora_engine/src/scene_io.rs` stores asset references as `String` paths and rehydrates handles on load; an observer patches those strings when assets move. `MeshInstanceData.model_path: Option<String>` and `SpriteImagePath` are the workaround carriers. (Scenes serialize to RON — see `docs` / §9 of the architecture brief.)

**Action:**
- Switch serialized assets to `#[derive(Asset, Reflect)] #[reflect(Asset)]`.
- Replace string-path fields with reflected `Handle<T>` fields.
- Register the handle processors on the scene (de)serializers via `TypedReflectSerializer::with_processor` / `TypedReflectDeserializer::with_processor`.
- Remove the `model_path`/`SpriteImagePath` carriers and the asset-path-patching observer.

**Payoff:** the largest boilerplate deletion available; eliminates manual rehydration and path-patching.

### T1.2 Delayed Commands → finish the Blueprint `flow/delay` node

**0.19 feature:** `commands.delayed().secs(t).<command>()`.

**Current state:** the `renzora_blueprint` interpreter (which runs graphs **directly at runtime**, not compiled to Lua) has a `flow/delay` node whose timer starts but never resumes execution. `renzora_scripting` separately polls `ScriptTimers`.

**Action:**
- Implement `flow/delay` using delayed commands.
- Optionally migrate parts of `ScriptTimers` polling to delayed commands.
- **Caveat:** 0.19 delayed commands have **no built-in cancellation**. For "cancel on despawn" cases, embed the originating `Entity` in the command and keep an adapter over the existing named-timer tracking — don't rip out `ScriptTimers` wholesale.

**Payoff:** completes a known-broken feature; reduces manual timer polling.

### T1.3 Interactive Transform Gizmo → shed maintenance from `renzora_gizmo`

**0.19 feature:** a built-in `TransformGizmoPlugin` (`TransformGizmoCamera` + `TransformGizmoFocus`, configured via `TransformGizmoConfig` / `TransformGizmoMode`; input-agnostic by design).

**Current state:** `renzora_gizmo` hand-rolls translate/rotate/scale handle meshes (real always-on-top mesh entities, `ActiveTool = Select/Translate/Rotate/Scale`), ray-picking, screen-delta math, and snapping, plus a lot of value-add (2D tools, collider editing, camera/light/skeleton overlays, `renzora_undo` integration).

**Action:**
- Evaluate replacing the **core** T/R/S handle math with the built-in gizmo; keep the value-add.
- Wire your existing input handling into the input-agnostic plugin.
- 0.19's gizmo shares the `fslabs/bevy_transform_gizmo` lineage, so behavior should feel familiar.

**Payoff:** removes a large maintenance surface; even partial adoption helps.

### T1.4 Render Recovery → stop XR from hard-crashing on device loss

**0.19 feature:** typed render errors + `RenderErrorHandler` / `RenderErrorPolicy` (`Recover` / `StopRendering` / `Ignore`).

**Current state:** `renzora_xr` (OpenXR via the vendored `bevy_oxr`) has **zero** device-loss handling — a compositor reset or headset disconnect crashes the app.

**Action:**
- Insert a `RenderErrorHandler` mapping `DeviceLost → Recover(default())`.
- Choose a policy for `OutOfMemory` / `Validation` / `Internal`.
- **Accessibility note:** test recovery carefully — repeated failures can cause flicker, a photosensitive-epilepsy risk. Start conservative.

**Payoff:** XR survives device loss; safer long-running editor / installations.

---

## Tier 2 — Free or near-free wins (little/no code once on 0.19)

| Win | Feature | Action |
| --- | --- | --- |
| Skinned characters stop vanishing mid-animation | Skinned-mesh culling fix | **None** for glTF-loaded skinned meshes (automatic). Hand-built meshes: add `DynamicSkinnedMeshBounds`. |
| iOS/Mac `StandardMaterial` FPS uplift | Partial bindless on Metal | **None** — you don't use `binding_array` uniforms, so you get it for free. |
| More correct IBL (no env-map seams, no metallic darkening) | White-furnace fixes | **None** — you rely on Bevy's atmosphere cubemap IBL (`renzora_environment_map`). |
| (Optional) in-game diagnostics for shipped non-editor builds | Diagnostics overlay | Low priority — the native `renzora_debugger` panels (11 of them) are far richer. Only useful for the shipped game / runtime / dedicated server, which has no editor bundle. |

---

## Tier 3 — Evaluate, don't rush

- **Infinite Grid** vs `renzora_grid` (a single per-vertex-colored `LineList` mesh + unlit `GridMaterial` distance fade): 0.19's fullscreen-shader grid avoids the "mesh has to end somewhere" horizon-aliasing problem. A/B test; possibly retire the mesh grid.
- **`bevy_settings`** vs `renzora_settings`: persistence is currently decentralized (per-resource, delegated to theme/input/project crates). `SettingsGroup` + `PreferencesPlugin` + `SavePreferencesDeferred` (debounced) gives one consistent TOML-backed mechanism + cross-platform `preferences_dir()`. A consolidation refactor, not a gap-filler.
- **Resources as Components**: 0.19 allows hooks/observers directly on resources — a nice cleanup for `GlobalStore` (which today manually fires `GlobalChanged`) and `SceneLoadState`, not urgent.
- **Vignette / LensDistortion built-ins**: you already have `renzora_vignette` / `renzora_distortion` wired into the inspector/macro/`EffectRouting` system, and `renzora_motion_blur` already proves the "wrap a Bevy-native effect" pattern. Little upside to rerouting unless you want to stop maintaining the shaders.
- **Solari (hardware ray tracing)**: this is the natural fit for Lumen's **`Hwrt` tier**, which is the *only* unbuilt GI tier. The voxel-cone tiers already ship — `LumenQuality::SdfLow`/`SdfHigh` drive the voxel-cone trace in `renzora_lumen`, and `ScreenSpace` delegates to `renzora_rt` SSGI. **`Hwrt` currently renders nothing**: `platform_wgpu_settings()` (`renzora_runtime/src/lib.rs`) requests only `POLYGON_MODE_LINE`, wgpu ray tracing is not enabled, and `bevy_solari` is not wired in. Solari is still experimental — watch it for the `Hwrt` tier *later*; don't build on it yet.
- **Contiguous query access (SIMD)**: only `renzora_physics` plausibly benefits. (There is no `renzora_physics_playground` crate — it's just `renzora_physics`.) Profile first.
- **BSN / next-gen scenes**: you have a custom RON scene + `renzora_blueprint` graph system. BSN is still maturing; watch but don't chase.

---

## Suggested sequencing

1. **Unblock** — bump the vendored forks (`bevy_hanabi`, `bevy_silk`, `bevy_oxr`, `bevy_mod_outline`, `vleue_navigator`, and the `bevy_hui` parser) to 0.19. *(The old "secure a 0.19 `bevy_egui`" step is dead — egui is gone.)*
2. **Forced port** — rewrite the 13 first-party `ViewNode` impls (§0.2) as render systems; migrate `Text`/`TextFont` call sites to Parley starting with `renzora_ember` (§0.3); bump the `bevy0.18`→`bevy0.19` literal in `build.rs` and rebuild all plugins (§0.4).
3. **Tier-1 payoffs** — serializable handles (delete the path workaround) → delayed commands (finish the blueprint delay) → XR render recovery → evaluate the transform-gizmo swap.
4. **Tier 2** lands for free once compiling on 0.19.
5. **Tier 3** as capacity allows.

---

## Forced-change audit checklist

- [ ] Vendored forks bumped to 0.19: `bevy_hanabi`, `bevy_silk`, `bevy_oxr`, `bevy_mod_outline`, `vleue_navigator`, `bevy_hui` (parser)
- [ ] `crates/renzora/src/postprocess.rs` — `UnifiedPostProcessNode` → render system
- [ ] `crates/renzora_rt/src/node.rs` — `RtNode` → render system
- [ ] `crates/renzora_viewport/src/debug_viz.rs` — `DebugVizNode` → render system
- [ ] `crates/renzora_lumen/*` — all **ten** `ViewNode` impls (voxel cache ×4, trace, geometry voxelize, downsample, screen-reflection trace/blur/resolve) → render systems
- [ ] Vendored-fork render-graph nodes ported (`bevy_mod_outline`, `bevy_hanabi`)
- [ ] All `bevy_ui` `Text` / `TextFont` sites migrated to Parley (`FontSize`, `FontSource`) — `renzora_ember` first (esp. `src/font.rs`), then `renzora_game_ui`, `renzora_shell`, `renzora_hierarchy`, `renzora_viewport`, `renzora_settings`, `renzora_editor_framework`, `renzora_ember/.../code_editor`
- [ ] `build.rs:17` — `bevy0.18` → `bevy0.19` in the `RENZORA_BUILD_HASH` input
- [ ] `docker/Dockerfile` Rust version checked/bumped if 0.19 needs newer rustc (no `rust-toolchain.toml` exists)
- [ ] Clean `--workspace` rebuild of binary + `renzora_editor` bundle + `bevy_dylib` + all dlopen plugins (re-syncs `plugin_bevy_hash` + `RENZORA_BUILD_HASH`)
