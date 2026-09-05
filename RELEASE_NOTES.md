# Renzora Engine `r1-alpha7`

The largest release the engine has had. Two months of work turned the plugin
system into a real ABI, gave the editor a second dimension, put the whole thing
in the browser, and moved the first-party plugins out of this repository and
into the marketplace.

|                     |                                              |
| ------------------- | -------------------------------------------- |
| **Previous release**| [`r1-alpha6`](https://github.com/renzora/engine/releases/tag/r1-alpha6) · 29 Jun 2026 |
| **Commits**         | 700 (694 direct, 6 merged pull requests)     |
| **Features**        | 303                                          |
| **Fixes**           | 223                                          |
| **Refactors**       | 58                                           |
| **Performance**     | 15                                           |
| **Docs**            | 41                                           |
| **Changed**         | 2,014 files · +363,792 / −187,921            |
| **Plugin ABI**      | 4.10 (the C ABI is new in this release)      |
| **Contributors**    | 8                                            |

---

## Plugins are a real ABI now

The headline of the release. `renzora_plugin` grew from a thin extension point
into a versioned C ABI that a plugin can compile against **without depending on
Bevy at all** — which is what makes a plugin built by one rustc load into an
engine built by another.

- **C-ABI plugin system** — write plugins in Rust with no Bevy dependency.
- **The ABI covers real work now:** physics, HTTP (including streamed responses
  and request headers), audio backends, textures you create and write per frame,
  custom shaded materials, render passes that run inside the render graph,
  keyboard/mouse/cursor input, native file dialogs, and geometry a plugin either
  reads from the world or uploads itself.
- **Query surface:** `Added<T>`, `Changed<T>`, `RemovedComponents`, optional
  query data and `Or` filters, plugin-owned resources, and string fields on
  plugin components.
- **Editor integration:** panels declared in BSN and rendered by ember, widgets
  bound to plugin resource fields, plugin-registered Settings sections, and
  panel contents a plugin can replace at run time.
- **Hot-swap** — rebuild a plugin and it reloads without restarting the editor.
  The editor watches sources (via `notify`, not polling), rebuilds on change,
  reloads a shader without rebuilding its pipeline, and redraws the plugin's
  panel when it comes back.
- **`no_std` build mode** for standalone plugins, and a pinned rustc resolved
  for the build rather than whatever happens to be on `PATH`.
- **The interface table carries a hash of its own shape**, so a mismatched
  plugin is refused instead of calling into the wrong function pointer.

**Native plugins** — Bevy plugins shipped as Rust *source* and compiled on the
user's machine — landed alongside, with scopes, link modes, and a declared list
of targets a plugin cannot build for.

### First-party plugins left the repository

`refactor: take plugins out of the repository`. **The marketplace ships plugins
now.** This repository builds the engine; everything that existed only to build
the other thing went with them — the C-ABI cargo loop, `cargo renzora plugin
<name>`, both staging passes, the `--plugins` coverage scope, CI's two plugin
jobs, and `build_plugins` in the container script.

Before that move, a long series of changes had already pushed engine built-ins
out of the binary and into standalone plugins: clouds, auto exposure, gamepad,
spline, mesh draw, night stars, procedural trees, 3D text, vignette, pool water,
AI chat, and Tracy. The Lua interpreter moved to `plugins/lua`, and the HTTP
client became a C-ABI plugin.

> **If you build plugins:** the ABI is at **4.10** and two majors in this
> release are breaking. See [Breaking changes](#breaking-changes).

---

## Rust scripts

Per-entity native code with full `World` access — a third scripting option
beside Lua and Rhai, and the only one with no interpreter between your code and
the ECS.

- Rust scripts **run in exported games**, not just the editor.
- Lifecycle hooks, scene-load and broadcast-event hooks.
- An `on_draw(g)` canvas API for immediate-mode 2D drawing from scripts.
- The export packs the assets a Rust script references.
- A `ScriptBackend` trait drives every language, so a language plugin registers
  itself through the ABI rather than being wired into the engine.

---

## A 2D editor

2D stopped being a special case of 3D and became its own mode.

- **avian2d physics backend**, merged tile colliders, and 2D collider edit
  handles.
- **Tilemaps** — sprite-entity tiles, panel-owned import, composite objects,
  multi-tile picking, multi-select, erase mode, and a Randomise button that
  scatters a selection into a forest.
- **Sprite animation** — a dedicated panel, multi-sheet `SpriteImages`, Flip X/Y
  on the Sprite inspector entry.
- **2D lighting plugin**, per-viewport light markers, and a 2D overlays
  dropdown.
- **Independent 2D camera per viewport**, with per-view grid, rulers, cursor
  coordinates in the status bar, and status-bar zoom.
- **2D particle effects** — `plane_2d` authoring, 2D drop and pick, effect
  library.

---

## A UI workspace

Building a menu no longer takes over the scene view.

- A dedicated **UI workspace** and a **UI Hierarchy** panel showing canvases and
  their template nodes.
- **World-space UI** — game-UI templates on 3D quads, unified into `UiCanvas`
  with a shared SDF text mesh, scaled to their reference resolution at runtime.
- Every UI canvas is backed by an **auto-created HTML template**.
- **UI editor**: element palette you drag from, design-space rulers that track
  the pointer, hover highlighting, drop slots shown in flow, free placement,
  drag-to-arrange with a snap pill, editable text, undoable markup edits, and a
  name badge on selection.
- Image attributes, **gamepad navigation**, and textured bars.

---

## Rendering

- **3D gaussian splatting** — vendored `bevy_gaussian_splatting` plus a
  `renzora_gaussian_splatting` plugin.
- **FFT ocean simulation**, and **one world wind** driving foliage, cloth and
  the ocean together.
- **Volumetric clouds** — a raymarched deck that skips empty space and sets with
  the sun.
- **Foliage** drawn with GPU instancing, paintable grass height, chunks
  scattered in parallel.
- **Solari** — emissive proxies for point and spot lights, a filtered ray-traced
  scene, dead shadow maps dropped.
- **Streaming** — async scene streaming, mesh LODs, world streaming, texture
  tiers.
- **Graphics-quality tiers**, applied to shipped games as well as the editor,
  plus SSAO quality and thickness controls in the World Environment.

### Materials

Shaders are validated and their errors attributed. Compile errors surface in the
Console and are marked on the offending graph node with a tooltip. Pin editors
draw on the nodes, math nodes vectorize to the widest wire, the compiled shader
bakes into the `.material`, displacement landed, and the graph saves with
`Ctrl+S`.

---

## VR

**VR play mode** — an OpenXR boot flag, a rebuilt `renzora_xr`, and a VR Headset
play target in the editor. Eye cameras are fully decorated (atmosphere, IBL,
prepasses, fog in-headset), the editor goes quiet during VR play, and the
in-headset environment is live.

---

## The web editor

The **editor itself** now compiles and links for `wasm32`, boots, renders, and
can open a real project — assets, scenes and materials — from a folder you pick.
The web lane builds it, there is a page to load it from, and `cargo renzora
wasm` builds and stages the bundles locally. The dlopen and file-watch stacks
are no longer shipped to the browser.

---

## Editor

- **Full undo/redo across every panel.**
- **Floating panel windows and multi-monitor docking.**
- **Blender-style mesh modeling and sculpting.**
- **In-editor Simulate mode**, selectable from the Play dropdown, and a
  Play-target dropdown that previews in the viewport or in a real runtime
  window.
- **A global bottom panel** — overlaid, pinned, resizable to the top bar, with
  panel sets you can drag to reorder and an Overlay/Layout mode.
- **Terrain** — generate from a placed region or a heightmap rather than only
  noise, make a terrain from a plane you already placed, a brush shelf with
  toolbar settings, a region tool and a settings overlay.
- **Audio** — the Renzora audio engine as a standalone plugin, a full mixer with
  device routing, colour coding and inline rename, and a playing indicator in
  the hierarchy.
- **Hierarchy** — rubber-band drag selection, arrow-key walk and fold,
  quick-add from a right-click, and attach a script, blueprint or material by
  dropping it on a row.
- **Inspector** — keyframe any animatable field (creating its track), a
  Resources panel for reflected ECS resources, a panel for plugin-owned
  resources, presets, and auto-added collision shapes when you add a physics
  body.
- **`i18n`** — an engine-wide localization system with 20 languages.
- **Chapter-based onboarding**, shown once per install rather than per project.
- Scene tabs moved into the viewport, document tabs into the window chrome, and
  open tabs persist per project.

---

## Export

- **Lean web export**, and lean container builds.
- **Capability auto-detection**, nested capabilities, a split 3D pipeline, and a
  gate on every subsystem the lean export can strip.
- **Native plugins link into the exported binary**; bundles ship as `.AppImage`
  and `.app`.
- **Modding on by default**, with the export telling you which mode to ship.
- Presets, icons, per-export lean profile knobs, and **UPX compression**.

---

## Releases and the updater

The engine now publishes itself. Nightlies build from `main` every night that
something lands; a pushed `r1-alpha*` tag cuts a full release. The editor
updates **in place**: a version list with a status card and live icons, a
top-bar chip when an update is waiting, selectable channels, a skip-a-version
control, nightlies gated on Dev Mode, and installing over a source checkout
behind an explicit two-step confirmation.

---

## Breaking changes

**The C ABI did not exist in `r1-alpha6`** — it was introduced, and then revised
twice, inside this cycle. If you built a plugin against an `r1-alpha7` nightly,
rebuild it against **4.10**; two of those revisions were majors:

| Change | What moved |
| --- | --- |
| `fix(plugin)!: repair the Interface table — ABI 3.0` | Two functions had been *inserted* into the middle of the `Interface` struct and recorded as appended, so a plugin built at 2.5–2.10 called the slot it compiled against and landed in a different function. The table was repaired and the major bumped. |
| `fix(plugin)!: ABI 4.0 — the three crossing enums become newtypes` | The three enums that cross the boundary became newtypes, so an unknown discriminant is data rather than undefined behaviour. |

**First-party plugins are no longer in this repository** and are not built by
`cargo renzora`. Install them from the marketplace. `cargo renzora plugin
<name>` is gone.

**Two features were reverted before release** and are *not* in `r1-alpha7`: live
collaborative editing sessions, and the version-control panel.

---

## Performance

- **Release binaries are ~87% smaller.** A size-optimised release profile
  (`opt-level = "s"` + thin LTO) plus UPX packing takes `renzora.exe` from
  187.0 MB to 24.9 MB and `renzora-editor.exe` from 265.6 MB to 35.1 MB. The
  installed tree drops from ~470 MB to ~77 MB.
- 23 unused Bevy features stripped; `prefer-dynamic` dropped.
- Editor chrome excluded from scene-facing systems **at the archetype level**;
  panel systems gated, inspector sections lazy and off-screen rows culled,
  bindings gated on declared deps.
- The post-process pass stopped scanning the world for every unused effect.
- The code editor rebuilds only the rows an edit changed.
- `cargo renzora` skips plugin builds, SDK staging and the SDK's second cargo
  run when nothing changed.

---

## Fixes

223 of them. The largest clusters were the plugin ABI (34), the editor UI
(`ui-editor` 11, `editor` 9, `ember` 7, `dock` 4), the viewport (10), CI (9),
shaders (7), and the marketplace (6).

---

## Contributors

Thanks to everyone who landed a change in this release.

- **Kassinity** — most of the shader- and material-error story: compile errors
  attributed to the graph node that caused them, generated shaders validated,
  compilation logged to the Console, erroneous nodes marked and tooltipped,
  node-graph pins grouped by compatibility, math nodes vectorized to the widest
  wire, `Ctrl+S` on the material graph, and WGSL validation pulled through one
  `renzora::wgsl` seam.
- **Lucas Mundim** — stubbed unwind symbols so `no_std` plugins load again,
  exact civil-date conversion in crash-report timestamps, and tolerating a
  missing `SuppressShadowMaps` on Solari's first extract.
- **saki2fifty** — snap settings honoured in modal G/R/S transforms, the rotate
  HUD readout snapped, and material-graph Apply round-tripped into UV-aware
  meshes.
- **dreamersilly** — deleted entities restored under their original parent on
  undo, and a green CI build after it had been red since 14 June 2026.
- **Umut Faruk** — directory drag-and-drop import.

Merged pull requests: [#89](https://github.com/renzora/engine/pull/89),
[#92](https://github.com/renzora/engine/pull/92),
[#96](https://github.com/renzora/engine/pull/96),
[#98](https://github.com/renzora/engine/pull/98),
[#102](https://github.com/renzora/engine/pull/102).

---

## Upgrading

- **Rebuild every prebuilt plugin** against ABI 4.10.
- **Reinstall first-party plugins from the marketplace** — they no longer ship
  with the engine, and `cargo renzora` will not build them.
- Existing projects load as they are. The editor offers the update in
  **Help ▸ Check for Updates**, or from the chip in the top bar.
