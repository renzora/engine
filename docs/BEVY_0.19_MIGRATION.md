# Bevy 0.19 Migration Plan — Renzora

A forward plan for moving the engine from its current Bevy 0.18 target onto Bevy 0.19: the forced mechanical work to compile, then the feature-driven payoffs that justify the bump.

Status: planning · Current engine target: **Bevy 0.18** · Target: **Bevy 0.19** *(now released — <https://bevy.org/news/bevy-0-19/>)*

> **Update (2026-06-19):** Bevy **0.19 is released**. The full release-notes feature set is confirmed and mapped to engine actions in the **Wishlist coverage** table below. Two items previously under-scoped are now first-class: **Physically Based SSR** (it overlaps the hand-rolled `renzora_lumen` screen-reflection pipeline — a real *evaluate-replace* decision) and **richer text / text input** (which the code editor and ember text helpers can harvest directly). The forced work (vendored-fork bumps, render-graph→systems, Parley text, ABI rebuild) is unchanged.

> **Update (2026-06-09):** the egui → bevy_ui/ember UI migration is fully complete — there is **no `egui` or `bevy_egui` anywhere in the active workspace** (the editor shell is `renzora_shell` on top of `renzora_ember`, and the markup runtime is `renzora_ember::markup`). That clears the old long-pole dependency gate. The remaining 0.19 blockers are the forced render-graph → systems port, the Parley text migration, and the vendored-crate bumps.

This plan is grounded in the current `crates/` layout: **164 top-level crates** under `crates/` plus **23 nested `editor/` subcrates** (~187 workspace crates), and **six vendored Bevy-ecosystem crates** (`bevy_hanabi`, `bevy_silk`, `bevy_oxr`, `bevy_mod_outline`, `vleue_navigator`, `bevy_hui`).

---

## Wishlist coverage (every requested 0.19 feature → engine verdict)

Confirmed against the 0.19 release notes. **Effort** is incremental work *after* the forced port (§0). **Tier** links to the detail section.

| 0.19 feature | Engine verdict | Effort | Where |
| --- | --- | --- | --- |
| **BSN / next-gen scenes** | **COMMITTED — replaces RON, non-negotiable.** 0.19 BSN is code-only, so Renzora ships its own *interim* `.bsn` loader/saver now (mirroring upstream's format) and swaps it for the first-party loader when [bevy#23576] lands. | High | §BSN |
| **Contact shadows** | **New gap-fill.** No screen-space contact shadows today; opt-in per light, cheap detail win. | Low | T1.5 |
| **Text input (`EditableText`)** | **Code-editor win.** `code_editor/{edit,systems}.rs` hand-rolls caret/selection/input — adopt `EditableText` for the primitives. | Med | T1.6 |
| **Richer text / variable fonts / responsive sizing / letter spacing** | Harvest in `renzora_ember::font` during the forced Parley migration — these *are* the Parley payoff. | Low* | T1.7 |
| **Skinned mesh culling** | Free for glTF skinned meshes; `DynamicSkinnedMeshBounds` for hand-built. | None | Tier 2 |
| **Solari improvements** | Watch for Lumen's unbuilt `Hwrt` tier *later*; still experimental. | — | Tier 3 |
| **Physically Based SSR** | **Evaluate-replace.** Overlaps `renzora_lumen` 4-stage screen-reflection pipeline (trace/blur/resolve). | Med–High | T1.4-R |
| **Rectangular area lights** | **New capability.** Engine leans on Bevy light types; expose `RectAreaLight`/LTC in inspector. | Low | T2-R |
| **App Settings (`bevy_settings`)** | Consolidation refactor of decentralized `renzora_settings`, not a gap-fill. | Med | Tier 3 |
| **Vignette & lens distortion (built-in)** | You already ship `renzora_vignette`/`renzora_distortion`; little upside unless retiring shaders. | — | Tier 3 |
| **Render Recovery** | **Stops XR hard-crash on device loss** (`renzora_xr` has none today). | Low | T1 (existing) |
| **Render Graph as Systems** | **Forced.** 13 first-party `ViewNode` impls + vendored forks. | High | §0.2 |
| **Parallax Corrected Cubemaps** | **New capability.** `renzora_environment_map` has only the global atmosphere cubemap — add local reflection probes. | Med | T2-R |
| **Partial bindless** | Free FPS uplift on Metal/iOS. | None | Tier 2 |
| **Contiguous query access** | Only `renzora_physics` plausibly benefits; profile first. | — | Tier 3 |
| **Delayed Commands** | **Finishes the broken Blueprint `flow/delay` node.** | Low | T1 (existing) |
| **Text Gizmos** | World-space debug labels for `renzora_debugger`/gizmo overlays (light/camera/skeleton names). | Low | T2-R |
| **Cancellable Web Tasks** | WASM-only: cancel in-flight asset loads on the web build (nav-away / tab switch). | Low | Tier 3 |
| **Asset Saving** | Runtime asset serialization — candidate to back the import/bake write path (`.rmip`, prefab/material writes). | Med | Tier 3 |
| **Resources as Components** | Cleanup for `GlobalStore`/`SceneLoadState` (hooks/observers on resources). | Low | Tier 3 |
| **Built-in Infinite Grid** | A/B vs `renzora_grid` mesh grid (fixes horizon aliasing). | Low | Tier 3 |
| **White Furnace Test** | Free IBL correctness (no env-map seams / metallic darkening). | None | Tier 2 |
| **Observer Run Conditions** | Conditional observers — minor cleanup where observers self-gate today. | Low | Tier 3 |
| **Serializing asset handles** | **Largest deletion available** — kills the `model_path`/`SpriteImagePath` string-path workaround. | Med | T1.1 |
| **Self-Referential Relationships** | Minor: any relationship that could point at self (rare in current graph). | — | Tier 3 |
| **Accessible Label Component** | a11y labeling on editor widgets; aligns with the existing accessibility note in T1.4. | Low | Tier 3 |

\* "Low*" = mostly absorbed by the forced Parley migration you must do anyway.

---

## BSN — the scene-format successor (vital, but staged)

**Goal:** replace the custom RON scene format (`renzora_engine/src/scene_io.rs`) with Bevy Scene Notation.

**0.19 reality (verified against the release notes):** BSN in 0.19 is **code-driven only**.

> "while Bevy 0.19 technically supports scene assets, we aren't yet shipping a first-party `.bsn` asset loader. This release focuses on the code-driven workflow." — Bevy 0.19 notes. The official `.bsn` asset loader is tracked in [bevy#23576] and "planned for a future release." BSN itself is "still hot off the presses … expect some rough edges and missing features."

**Decision: BSN replaces RON. Non-negotiable.** `.bsn` becomes Renzora's on-disk scene format. Because 0.19 ships BSN code-only (no first-party `.bsn` asset loader yet — tracked in [bevy#23576]), Renzora ships its **own interim `.bsn` asset loader/saver now** and swaps it for the first-party one when it lands. The whole point of the interim loader is to be **throwaway**: written to mirror upstream's BSN format exactly so the eventual swap is a deletion, not a rewrite.

**The one rule that makes "throwaway" true — stay on upstream's format, don't fork it:**
- **Reuse Bevy's BSN parsing/types, don't hand-roll a grammar.** Build the loader on whatever BSN crate exposes at runtime (the `bsn!` macro's parser/AST and reflection-construction path) rather than inventing a parser. If a piece is macro-only in 0.19, wrap it minimally and isolate that wrapper so it's the *only* thing that changes when upstream opens it up. **A hand-rolled grammar is the failure mode** — it drifts the moment Bevy tweaks the syntax, and then the swap isn't free.
- **Match the file format byte-for-byte with what the `bsn!` macro accepts.** Same syntax, same component/template semantics, same asset-reference form. If a `.bsn` you write loads under the future first-party loader unchanged, you did it right.
- **Isolate the loader behind a trait** (`SceneSerializer` or similar) in `scene_io.rs` so the interim impl and the future first-party impl are swappable without touching the editor/scene-tab/hot-reload call sites.

**Build order:**
1. **T1.1 serializable asset handles first** — BSN scenes reference assets by handle; killing the `model_path`/`SpriteImagePath` string-path carriers (reflect real `Handle<T>`) is the prerequisite for a clean `.bsn` round-trip. This is the on-ramp, do it first.
2. **`bsn!` macro for code-defined trees now** — editor chrome spawns, default-scene construction, tutorial fixtures. Proves the BSN construction path end-to-end before the file format rides on it.
3. **Interim `.bsn` loader + saver** — `AssetLoader` for `.bsn` (load) + a serializer that emits the same syntax (save), both reusing Bevy's BSN reflection/parse types. Wire into the scene-tab system + hot-reload.
4. **Cut over** — `.bsn` becomes the saved format; provide a one-shot **RON→BSN converter** so existing project scenes survive (see the recovered-scenes history — don't strand saved work).
5. **Reconcile with `renzora_blueprint`** — BSN describes the entity tree; the blueprint graph stays the behavior layer on top.

**Then, when [bevy#23576] merges:** delete the interim loader, drop in the first-party one behind the same `SceneSerializer` trait. If rule #1 held, this is a few-line change. Watch that PR — but it's a *cleanup* trigger now, not a blocker.

[bevy#23576]: https://github.com/bevyengine/bevy/pull/23576

---

## Replace-with-built-in audit (shed the luggage)

Goal: delete hand-rolled code that 0.19 now provides upstream. **But "built-in" only helps where you actually duplicate it** — and the audit found that for post-processing you *already* delegate to Bevy built-ins, so most of the perceived luggage isn't luggage. Verdicts:

### Post-process — you already use the built-ins (mostly nothing to do)

~14 effect crates (`renzora_motion_blur`, `renzora_bloom_effect`, `renzora_auto_exposure`, `renzora_dof`, `renzora_ssao`, `renzora_ssr`, `renzora_tonemapping`, `renzora_oit`, `renzora_volumetric_fog`, `renzora_distance_fog`, `renzora_antialiasing`, …) are **thin routers** (~80–270 lines each) that sync settings onto Bevy's native `Bloom`/`MotionBlur`/`DepthOfField`/`Tonemapping`/etc. and add the inspector + `EffectRouting` (settings survive camera switches). **Verdict: KEEP all of them.** They *are* the built-in; the wrapper is the editor integration, not a reimplementation. Deleting them would delete the inspector + routing, not save duplicated rendering code.

### Custom-shader effects — only two have a 0.19 built-in

| Crate | 0.19 built-in? | Verdict |
| --- | --- | --- |
| `renzora_vignette` (186 + 30 wgsl) | **Yes — built-in vignette** | **SWAP** to native, or convert to a thin router like the others. Lose the custom `.wgsl`. |
| `renzora_distortion` (38 + 31 wgsl) | **Yes — built-in lens distortion** | **SWAP / re-route** to native lens distortion. |
| `renzora_chromatic_aberration`, `renzora_film_grain`, `renzora_god_rays`, `renzora_light_streaks`, `renzora_tilt_shift`, `renzora_toon`, `renzora_sharpen`, post-process `renzora_outline` | **No Bevy equivalent** | **KEEP** — these are genuinely yours. (`renzora_sharpen` overlaps CAS in `renzora_antialiasing` — consider retiring sharpen in favor of CAS.) |

### SSR — you have *two* implementations; 0.19 PBSSR clarifies the dedupe

- `renzora_ssr` is already a thin wrapper over Bevy's native `ScreenSpaceReflections` (with a `DeferredPrepass` validation guard). **0.19's PBSSR upgrades exactly this native path — KEEP and inherit the improvement for free.**
- `renzora_lumen`'s `screen_reflection{,_blur,_resolve}.rs` (~976 lines + WGSL) is a *separate* hand-rolled SSR feeding voxel GI. **Decide (T1.4-R):** if PBSSR's quality is enough, **delete the lumen SSR trio** — which also removes 3 of the forced §0.2 render-graph ports. If the GI coupling matters, keep it.

### Editor tooling — PARTIAL (adopt the primitive, keep the value-add)

| System | Lines | Verdict |
| --- | --- | --- |
| `renzora_gizmo` | ~6,970 | **PARTIAL.** Built-in transform gizmo covers 3D T/R/S only. Keep collider editing, 2D tools, skeleton/light/camera overlays, modal G/R/S numeric input, undo integration. Swap only the core T/R/S handle math. |
| `renzora_settings` | ~2,540 | **PARTIAL.** `bevy_settings` is a persistence framework, not the UI. Could back the *storage* layer (consolidate today's decentralized per-resource saves); keep the 10-tab UI + plugin registry. Consolidation, not deletion. |
| `renzora_debugger` | ~4,050 | **KEEP (richer).** Stock diagnostics overlay only does FPS/frame-time/entity-count — you already pull those from `DiagnosticsStore`. Your GPU-pass attribution, memory-trend, ECS-archetype, camera/culling panels have no upstream equivalent. |
| code editor + single-line inputs | ~2,150 | **PARTIAL.** Adopt `EditableText` for caret/selection/IME primitives (T1.6); keep highlighting/search/rename UX. |

### Core / ECS / assets — clean deletions + one big one

| Workaround | Lines | 0.19 replacement | Verdict |
| --- | --- | --- | --- |
| `scene_io.rs` string-path asset handles (`model_path`, `SpriteImagePath`, rehydration observers) | ~500–600 | Serializable asset handles | **SWAP — biggest deletion** (T1.1). Also the BSN on-ramp. |
| `renzora_globals` `GlobalStore` manual `changed_keys` + `emit_global_changed` | ~25 | Resources-as-Components hooks | **SWAP — clean**, self-contained. |
| `renzora_scripting` `ScriptTimers` polling (`timers.rs`) | ~98 | Delayed Commands | **PARTIAL** — adopt for fire-and-forget; keep named-timer tracking for cancellable cases (0.19 delayed commands have no built-in cancel). *(Note: the audit found the broken blueprint `flow/delay` node referenced earlier doesn't actually exist as a node — delays live only in `ScriptTimers`. Correct T1.2 accordingly.)* |
| `renzora_scripting` `http.rs` `std::thread` HTTP (no cancel, no WASM) | ~91 | Cancellable Web Tasks + task pool | **PARTIAL** — port to the task pool to unblock WASM + add cancellation; low urgency. |
| observer early-return guards (sprite/mesh-path observers) | ~15–20 | Observer Run Conditions | **SWAP — cosmetic**, batch with other refactors. |
| `.rmip` baking (`renzora_rmip`), `PendingAssemblyWrites` | ~360 | (Asset Saving is unrelated) | **KEEP** — these aren't 0.18 workarounds; Asset Saving doesn't replace them. |

**Bottom line:** the genuinely deletable luggage is **vignette + lens-distortion custom shaders**, the **`scene_io` string-path handle workaround** (big), **`GlobalStore` manual events**, the **observer guards**, and — pending the T1.4-R decision — the **lumen SSR trio**. Everything else is either already-the-built-in (post-process routers) or value-add that replacement would regress (gizmo, debugger, code editor, skybox/sun/atmosphere).

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

Change `bevy0.18` → `bevy0.19` so the build hash distinguishes 0.18-built artifacts from 0.19-built ones. There is **no `rust-toolchain.toml`** — the Rust version lives only in `docker/base/Dockerfile` (`FROM rust:1.93.0-bookworm`); if the 0.19 bump requires a newer rustc, update it there.

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

**Current state (corrected by audit):** there is **no `flow/delay` blueprint node** — `renzora_blueprint` has event/math/transform/flow-branch/variable/debug/animation nodes but no delay/timer node. All delay behavior lives in `renzora_scripting`'s `ScriptTimers` (`timers.rs`, ~98 lines): a `HashMap<String, ScriptTimer>` hand-ticked each frame by `update_script_timers`, polled via `get_just_finished()`.

**Action:**
- Migrate the fire-and-forget `ScriptTimers` cases to delayed commands.
- *(Optional)* add a real `flow/delay` blueprint node backed by delayed commands — it doesn't exist yet, so this is new work, not a fix.
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

### T1.4-R Physically Based SSR → decide the fate of the hand-rolled reflection pipeline

**0.19 feature:** built-in *Physically Based Screen Space Reflections* (energy-conserving, roughness-aware).

**Current state — direct overlap.** `renzora_lumen` already hand-rolls a Godot/UE5-style 4-stage SSR pipeline at half resolution:
- `screen_reflection.rs` — half-res world-space trace → `Rgba16Float` color+validity
- `screen_reflection_blur.rs` — separable Gaussian blur
- `screen_reflection_resolve.rs` — bilateral upsample resolve, consumed in `lumen_trace`

These are **already three of the ten lumen `ViewNode` impls** you must port in §0.2.

**Action — evaluate, don't reflexively port both:**
- Stand up Bevy's PBSSR on a test scene and A/B against the lumen pipeline for quality (rough reflections, contact hardening) and cost.
- If PBSSR wins, **delete `screen_reflection{,_blur,_resolve}.rs`** — that *removes three nodes from the §0.2 port burden* instead of rewriting them. Re-point `lumen_trace`'s resolve consumption at the built-in result.
- If the lumen pipeline wins (it's tuned to feed voxel-cone GI, which PBSSR isn't aware of), keep it and just port the nodes.

**Payoff:** either a quality upgrade or a net code deletion that *shrinks* the forced render-graph port. Decide this **before** porting those three nodes.

### T1.5 Contact Shadows → cheap detail the engine has never had

**0.19 feature:** screen-space contact shadows (short ray-march in depth) per light, for fine contact detail shadow maps miss.

**Current state:** no screen-space/contact shadow path exists anywhere in the workspace — shadows are Bevy's shadow maps only.

**Action:** opt light entities into contact shadows; surface the toggle + distance in the light inspector (`renzora_inspector` light drawer). Pure additive feature.

**Payoff:** noticeably grounded objects (feet, small props) for near-zero authoring cost.

### T1.6 Text Input (`EditableText`) → shrink the code editor's hand-rolled editing core

**0.19 feature:** `EditableText` component providing caret, selection, and text-entry primitives for `bevy_ui`.

**Current state:** `crates/renzora_ember/src/widgets/code_editor/{edit,systems}.rs` hand-roll caret movement, selection, and key handling on top of raw input.

**Action:**
- Adopt `EditableText` for the low-level caret/selection/IME primitives; **keep** the editor's value-add (syntax highlighting, gutter, multi-cursor, find/replace) on top.
- Audit other single-line inputs (ember `search.rs`, inspector text fields, rename-in-hierarchy) to share the same primitive.

**Caveat:** the code editor must still do its own line-buffer/highlight layout, so this is a *primitives* swap, not a wholesale replacement — scope it as "stop maintaining caret/selection math," not "delete the editor."

**Payoff:** less fragile editing core; IME/selection correctness handled upstream. (Directly addresses the "improve code editor" goal.)

### T1.7 Richer Text → harvest the Parley migration you're already forced to do

**0.19 feature:** font selection, **variable font properties**, **responsive font sizing**, and **letter spacing** — the Parley text stack (§0.3).

**Current state:** §0.3 already forces every `TextFont` site to migrate to Parley (`FontSize`, `FontSource`). The richer-text knobs are *new fields on the same structs you're already touching*.

**Action (fold into §0.3, don't schedule separately):**
- In `renzora_ember/src/font.rs`, expose letter-spacing + variable-axis weight on the central text helpers so the whole editor inherits them.
- Use **responsive font sizing** for DPI/scale-aware UI text (interacts with the existing `editor-dpi-scaling` base-scale-factor approach — re-verify that still composes).
- Offer variable-weight/letter-spacing in the **Theme typography** settings (the ember theme system already carries typography).

**Payoff:** crisper, themeable editor typography for essentially the cost of the migration you can't avoid anyway.

---

## Tier 2 — Free or near-free wins (little/no code once on 0.19)

| Win | Feature | Action |
| --- | --- | --- |
| Skinned characters stop vanishing mid-animation | Skinned-mesh culling fix | **None** for glTF-loaded skinned meshes (automatic). Hand-built meshes: add `DynamicSkinnedMeshBounds`. |
| iOS/Mac `StandardMaterial` FPS uplift | Partial bindless on Metal | **None** — you don't use `binding_array` uniforms, so you get it for free. |
| More correct IBL (no env-map seams, no metallic darkening) | White-furnace fixes | **None** — you rely on Bevy's atmosphere cubemap IBL (`renzora_environment_map`). |
| (Optional) in-game diagnostics for shipped non-editor builds | Diagnostics overlay | Low priority — the native `renzora_debugger` panels (11 of them) are far richer. Only useful for the shipped game / runtime / dedicated server, which has no editor bundle. |

### New-capability rendering wins (small, additive — no existing code to fight)

These don't remove code; they expose 0.19 capabilities the engine simply lacked. Each is a small inspector/spawn wiring job once on 0.19.

- **T2-R Rectangular Area Lights** — the engine leans on Bevy's stock light types (there is no custom light enum in `renzora` core). Add the new LTC-based `RectAreaLight` (or 0.19's exact name) to the light spawn menu + inspector light drawer. Soft, physically plausible fill lighting for interiors/studio scenes; near-zero cost beyond the inspector field.
- **T2-R Parallax Corrected Cubemaps** — `renzora_environment_map` today only drives the **single global** atmosphere cubemap (`AtmosphereEnvironmentMapLight`). 0.19's parallax-corrected reflection probes let you place **local** probes with a correction volume so reflections track room geometry instead of looking infinitely far. New component + inspector + a bake/capture path; high visual payoff for interiors. Pairs naturally with the T1.4-R reflection decision.
- **T2-R Text Gizmos** — world-space debug text. Wire into the existing gizmo/debug overlays (`renzora_viewport` debug viz, `renzora_debugger`) to label lights, cameras, bones, and entity names in the viewport without bevy_ui billboards.

---

## Tier 3 — Evaluate, don't rush

- **Infinite Grid** vs `renzora_grid` (a single per-vertex-colored `LineList` mesh + unlit `GridMaterial` distance fade): 0.19's fullscreen-shader grid avoids the "mesh has to end somewhere" horizon-aliasing problem. A/B test; possibly retire the mesh grid.
- **`bevy_settings`** vs `renzora_settings`: persistence is currently decentralized (per-resource, delegated to theme/input/project crates). `SettingsGroup` + `PreferencesPlugin` + `SavePreferencesDeferred` (debounced) gives one consistent TOML-backed mechanism + cross-platform `preferences_dir()`. A consolidation refactor, not a gap-filler.
- **Resources as Components**: 0.19 allows hooks/observers directly on resources — a nice cleanup for `GlobalStore` (which today manually fires `GlobalChanged`) and `SceneLoadState`, not urgent.
- **Vignette / LensDistortion built-ins**: you already have `renzora_vignette` / `renzora_distortion` wired into the inspector/macro/`EffectRouting` system, and `renzora_motion_blur` already proves the "wrap a Bevy-native effect" pattern. Little upside to rerouting unless you want to stop maintaining the shaders.
- **Solari (hardware ray tracing)**: this is the natural fit for Lumen's **`Hwrt` tier**, which is the *only* unbuilt GI tier. The voxel-cone tiers already ship — `LumenQuality::SdfLow`/`SdfHigh` drive the voxel-cone trace in `renzora_lumen`, and `ScreenSpace` delegates to `renzora_rt` SSGI. **`Hwrt` currently renders nothing**: `platform_wgpu_settings()` (`renzora_runtime/src/lib.rs`) requests only `POLYGON_MODE_LINE`, wgpu ray tracing is not enabled, and `bevy_solari` is not wired in. Solari is still experimental — watch it for the `Hwrt` tier *later*; don't build on it yet.
- **Contiguous query access (SIMD)**: only `renzora_physics` plausibly benefits. (There is no `renzora_physics_playground` crate — it's just `renzora_physics`.) Profile first.
- **BSN / next-gen scenes**: promoted out of Tier 3 — it's a **committed Tier-1 initiative**, see **§BSN above**. Replaces RON via a Renzora-built interim `.bsn` loader, swapped for the first-party loader when [bevy#23576] lands.
- **Asset Saving**: 0.19's runtime asset serialization is a candidate to back the import/bake **write** path — `.rmip` texture baking, prefab/material source writes (`PendingAssemblyWrites`, `save_prefab_source`, material override caches). Today these are hand-rolled writers; migrating onto the official saver is a consolidation, not a gap-fill. Evaluate after the forced port.
- **Cancellable Web Tasks**: WASM-only. The web build currently can't cancel an in-flight asset load when the user navigates away / switches scene; 0.19 cancellation closes that. Low priority unless the web target is active.
- **Resources as Components**: hooks/observers directly on resources — cleans up `GlobalStore` (manually fires `GlobalChanged`) and `SceneLoadState`. Not urgent.
- **Observer Run Conditions**: where observers currently self-gate with an early `return`, a run condition is tidier. Pure cleanup.
- **Self-Referential Relationships**: only relevant if a relationship in the graph/hierarchy needs to legitimately point at itself — rare in the current model. Note and move on.
- **Accessible Label Component**: attach a11y labels to editor widgets (buttons, panels, inspector fields) independent of their visible text. Aligns with the photosensitivity/accessibility notes already in T1.4; do it as an editor-wide pass, not per-feature.

---

## Suggested sequencing

1. **Unblock** — bump the vendored forks (`bevy_hanabi`, `bevy_silk`, `bevy_oxr`, `bevy_mod_outline`, `vleue_navigator`, and the `bevy_hui` parser) to 0.19. *(The old "secure a 0.19 `bevy_egui`" step is dead — egui is gone.)*
2. **Decide PBSSR first (§T1.4-R).** Before porting the three lumen screen-reflection nodes, A/B Bevy's built-in PBSSR — a win there *deletes* three of the §0.2 nodes instead of rewriting them.
3. **Forced port** — rewrite the (≤13) first-party `ViewNode` impls (§0.2) as render systems; migrate `Text`/`TextFont` call sites to Parley starting with `renzora_ember` (§0.3) **and harvest the richer-text knobs in the same pass (§T1.7)**; bump the `bevy0.18`→`bevy0.19` literal in `build.rs` and rebuild all plugins (§0.4).
4. **Tier-1 payoffs** — serializable handles (delete the path workaround) → delayed commands (finish the blueprint delay) → XR render recovery → `EditableText` in the code editor (§T1.6) → contact shadows (§T1.5) → evaluate the transform-gizmo swap.
5. **Tier 2** lands for free once compiling on 0.19; the new-capability rendering wins (area lights, parallax cubemaps, text gizmos) are small additive inspector jobs.
6. **Tier 3** as capacity allows.

---

## Forced-change audit checklist

- [ ] Vendored forks bumped to 0.19: `bevy_hanabi`, `bevy_silk`, `bevy_oxr`, `bevy_mod_outline`, `vleue_navigator`, `bevy_hui` (parser)
- [ ] `crates/renzora/src/postprocess.rs` — `UnifiedPostProcessNode` → render system
- [ ] `crates/renzora_rt/src/node.rs` — `RtNode` → render system
- [ ] `crates/renzora_viewport/src/debug_viz.rs` — `DebugVizNode` → render system
- [ ] **PBSSR decision (§T1.4-R) made before** touching the screen-reflection nodes (a win deletes three of them)
- [ ] `crates/renzora_lumen/*` — the `ViewNode` impls (voxel cache ×4, trace, geometry voxelize, downsample, and screen-reflection trace/blur/resolve **unless replaced by PBSSR**) → render systems
- [ ] Richer-text knobs (letter spacing / variable weight / responsive sizing) surfaced in `renzora_ember/src/font.rs` + Theme typography during the Parley pass
- [ ] `EditableText` adopted for caret/selection in `code_editor` (and shared single-line inputs)
- [ ] Contact shadows exposed in the light inspector drawer
- [ ] New-capability rendering wired: rectangular area lights, parallax-corrected reflection probes, text gizmos
- [ ] Vendored-fork render-graph nodes ported (`bevy_mod_outline`, `bevy_hanabi`)
- [ ] All `bevy_ui` `Text` / `TextFont` sites migrated to Parley (`FontSize`, `FontSource`) — `renzora_ember` first (esp. `src/font.rs`), then `renzora_game_ui`, `renzora_shell`, `renzora_hierarchy`, `renzora_viewport`, `renzora_settings`, `renzora_editor_framework`, `renzora_ember/.../code_editor`
- [ ] `build.rs:17` — `bevy0.18` → `bevy0.19` in the `RENZORA_BUILD_HASH` input
- [ ] `docker/base/Dockerfile` Rust version checked/bumped if 0.19 needs newer rustc (no `rust-toolchain.toml` exists)
- [ ] Clean `--workspace` rebuild of binary + `renzora_editor` bundle + `bevy_dylib` + all dlopen plugins (re-syncs `plugin_bevy_hash` + `RENZORA_BUILD_HASH`)
