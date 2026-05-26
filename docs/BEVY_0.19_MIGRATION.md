# Bevy 0.19 Migration Plan — Renzora

Status: planning · Current engine target: **Bevy 0.18** · Target: **Bevy 0.19**

This plan covers (1) the forced mechanical work to compile on 0.19 and (2) the
feature-driven payoffs that justify the migration, prioritized by leverage.
Findings are grounded in the current `crates/` layout (158 `renzora_*` crates +
vendored bevy-ecosystem crates).

---

## 0. Migration reality check (read first)

Two things gate everything below.

### 0.1 Dependency chain (the long pole)
The editor is entirely egui-driven, so nothing moves until the UI stack has a
0.19-compatible release:

| Dependency | Current | Blocker |
| --- | --- | --- |
| `bevy` | 0.18 | target 0.19 |
| `bevy_egui` / `egui` | 0.39 / 0.33 | **needs a 0.19-compatible bevy_egui** — primary gate |
| `bevy_hanabi` (vendored) | — | bump to 0.19 |
| `bevy_silk` (vendored) | — | bump to 0.19 |
| `bevy_oxr` (vendored) | — | bump to 0.19 |
| `bevy_mod_outline` (vendored) | — | bump to 0.19 |
| `vleue_navigator` (vendored) | — | bump to 0.19 |

The `dlopen` plugin system's `plugin_bevy_hash()` TypeId check stays valid as
long as host and plugins compile against the *same* bevy — workspace feature
unification already guarantees this. No action needed there beyond a clean
rebuild of all plugins.

### 0.2 Forced breaking changes (must do regardless of features)
- **Render Graph → Systems.** `Node`/`ViewNode` + `add_render_graph_node` +
  `try_add_node_edge` are gone. Must rewrite as systems ordered with
  `.after(Core3dSystems::...)`. Affected:
  - `crates/renzora/src/postprocess.rs` — `UnifiedPostProcessNode` (single node;
    small port thanks to the unified architecture)
  - `crates/renzora_rt/src/node.rs`
  - audit any other `impl ViewNode`/`impl Node` (e.g. ssao, lumen, gaussian_blur)
- **Parley text migration.** `TextFont` / `font_size` change shape (`FontSize`
  enum, `FontSource`). Touch every Bevy `Text` construction site — game UI in
  `renzora_ui` / `renzora_game_ui`. Editor egui text is unaffected.

Budget the migration as: **port forced changes + bump deps**, then harvest the
payoffs below.

---

## Tier 1 — Highest leverage (closes real gaps / removes heavy code)

### T1.1 Serializable asset handles → delete the string-path workaround
**0.19 feature:** `HandleSerializeProcessor` / `HandleDeserializeProcessor`
(stores a handle's asset path on serialize, reloads on deserialize).

**Current state:** `crates/renzora_engine/src/scene_io.rs` stores asset paths as
`String` and rehydrates handles on load (`rehydrate_mesh_instances`, ~lines
654–700). An observer (`apply_asset_path_changes_to_mesh_instances`) patches
those strings when assets move. `MeshInstanceData.model_path: Option<String>`
and `SpriteImagePath` are the workaround carriers.

**Action:**
- Switch serialized assets to `#[derive(Asset, Reflect)] #[reflect(Asset)]`.
- Replace string-path fields with reflected `Handle<T>` fields.
- Register `HandleSerializeProcessor`/`HandleDeserializeProcessor` on the
  scene (de)serializers via `TypedReflectSerializer::with_processor` /
  `TypedReflectDeserializer::with_processor`.
- Remove `model_path`/`SpriteImagePath` string carriers and the
  `apply_asset_path_changes_to_mesh_instances` observer.

**Payoff:** largest boilerplate deletion available; eliminates manual rehydration
and path-patching.

### T1.2 Delayed Commands → finish the Blueprint `flow/delay` node
**0.19 feature:** `commands.delayed().secs(t).<command>()`.

**Current state:** `renzora_blueprint` interpreter has a literal
`// TODO: wire delay completion through timer system` — the delay timer starts
but never resumes execution. `renzora_scripting` manually polls
`ScriptTimers::get_just_finished()`.

**Action:**
- Implement `flow/delay` using delayed commands.
- Optionally migrate parts of `ScriptTimers` polling to delayed commands.
- **Caveat:** 0.19 delayed commands have **no built-in cancellation**. For
  "cancel on despawn" cases, embed the originating `Entity` in the command and
  keep an adapter over the existing named-timer tracking — don't rip out
  `ScriptTimers` wholesale.

**Payoff:** completes a known-broken feature; reduces manual timer polling.

### T1.3 Interactive Transform Gizmo → shed maintenance from `renzora_gizmo`
**0.19 feature:** built-in `TransformGizmoPlugin` (`TransformGizmoCamera` +
`TransformGizmoFocus`, configured via `TransformGizmoConfig` /
`TransformGizmoMode`; input-agnostic by design).

**Current state:** `renzora_gizmo` is ~7,400 lines hand-rolling T/R/S handle
meshes, ray-picking, screen-delta math, and snapping (plus 2D tools, collider
editing, camera/light/skeleton overlays, undo integration).

**Action:**
- Evaluate replacing the **core** translate/rotate/scale handle math with the
  built-in gizmo; keep the value-add (2D tools, collider editing, overlays,
  undo).
- Wire your existing input handling into the input-agnostic plugin.
- 0.19's gizmo shares the `fslabs/bevy_transform_gizmo` lineage, so behavior
  should feel familiar.

**Payoff:** removes a large maintenance surface; even partial adoption helps.

### T1.4 Render Recovery → stop XR from hard-crashing on device loss
**0.19 feature:** typed render errors + `RenderErrorHandler` /
`RenderErrorPolicy` (`Recover` / `StopRendering` / `Ignore`).

**Current state:** `renzora_xr` (OpenXR via `bevy_oxr`) has **zero** device-loss
handling — a compositor reset / headset disconnect crashes the app.

**Action:**
- Insert a `RenderErrorHandler` mapping `DeviceLost → Recover(default())`.
- Choose policy for `OutOfMemory` / `Validation` / `Internal`.
- **Accessibility note:** test recovery carefully — repeated failures can cause
  flicker, a photosensitive-epilepsy risk. Start conservative.

**Payoff:** XR survives device loss; safer long-running editor / installations.

---

## Tier 2 — Free or near-free wins (little/no code once on 0.19)

| Win | Feature | Action |
| --- | --- | --- |
| Skinned characters stop vanishing mid-animation | Skinned mesh culling fix | **None** for glTF-loaded skinned meshes (automatic). Hand-built meshes: add `DynamicSkinnedMeshBounds`. |
| iOS/Mac `StandardMaterial` FPS uplift | Partial bindless on Metal | **None** — you don't use `binding_array` uniforms, so you get it for free. |
| More correct IBL (no env-map seams, no metallic darkening) | White furnace fixes | **None** — you rely on Bevy's atmosphere cubemap IBL (`renzora_environment_map`). |
| (Optional) in-game diagnostics for non-egui builds | Diagnostics overlay | Low priority — your egui debugger panels are richer. Only for shipped game/runtime/server. |

---

## Tier 3 — Evaluate, don't rush

- **Infinite Grid** vs `renzora_grid` (294-line mesh+material grid): 0.19's
  fullscreen-shader grid avoids the "mesh has to end somewhere" horizon-aliasing
  problem. A/B test; possibly retire ~300 lines.
- **bevy_settings** vs `renzora_settings`: your persistence is decentralized
  (per-resource, delegated to theme/input/project crates). `SettingsGroup` +
  `PreferencesPlugin` + `SavePreferencesDeferred` (debounced) gives one
  consistent TOML-backed mechanism + cross-platform `preferences_dir()`.
  Consolidation refactor, not a gap-filler.
- **Resources as Components**: 434 resource types; `GlobalStore` manually fires
  `GlobalChanged`. 0.19 allows hooks/observers directly on resources — nice
  cleanup for `GlobalStore`/`SceneLoadState`, not urgent.
- **Vignette / LensDistortion built-ins**: you already have `renzora_vignette` /
  `renzora_distortion` wired into the inspector/macro/`EffectRouting` system, and
  `renzora_motion_blur` already proves the "wrap a Bevy-native effect" pattern.
  Little upside to rerouting unless you want to stop maintaining the shaders.
- **Solari**: you have custom SSGI (`renzora_rt`) + a Lumen scaffold
  (`renzora_lumen`) with an explicitly-reserved `Hwrt` tier (Phases 5–10). Solari
  is the natural fit for that hardware-RT tier *later* — still experimental, so
  watch, don't build on it yet.
- **Contiguous query access (SIMD)**: only `renzora_physics` /
  `renzora_physics_playground` plausibly benefit. Profile first.
- **BSN / next-gen scenes**: you have a custom RON scene + blueprint system. BSN
  is still maturing; watch but don't chase.

---

## Suggested sequencing

1. **Unblock** — confirm/secure 0.19-compatible `bevy_egui` and vendored crates
   (or plan to bump them yourself). This is the gate.
2. **Forced port** — convert render-graph nodes → systems; migrate
   `Text`/`TextFont` call sites to Parley APIs.
3. **Tier-1 payoffs** — serializable handles (delete path workaround) → delayed
   commands (finish blueprint delay) → XR render recovery → evaluate transform
   gizmo swap.
4. **Tier 2** lands for free once compiling on 0.19.
5. **Tier 3** as capacity allows.

---

## Forced-change audit checklist

- [ ] `bevy_egui` 0.19-compatible release secured
- [ ] Vendored crates bumped: `bevy_hanabi`, `bevy_silk`, `bevy_oxr`,
      `bevy_mod_outline`, `vleue_navigator`
- [ ] `crates/renzora/src/postprocess.rs` — `UnifiedPostProcessNode` → system
- [ ] `crates/renzora_rt/src/node.rs` → system
- [ ] Grep all `impl ViewNode` / `impl Node` / `add_render_graph_node` /
      `try_add_node_edge` and port each
- [ ] All Bevy `Text` / `TextFont` / `font_size` sites migrated to Parley
      (`FontSize`, `FontSource`) — focus `renzora_ui`, `renzora_game_ui`
- [ ] Clean rebuild of all `dlopen` plugins against 0.19 (TypeId hash re-sync)
