# Editor egui → bevy_ui Migration Plan

Status of migrating the **editor chrome** (dock, panels, viewport header) from
egui to a native bevy_ui framework, and the remaining work.

> This is the **editor UI** migration. It is distinct from
> [`ui_plan.md`](./ui_plan.md), which covers the **game-facing** markup UI
> (bevy_hui / canvas / Cinder). It is the prerequisite for
> [`editor-runtime-plugin-architecture.md`](./editor-runtime-plugin-architecture.md):
> once the editor is bevy_ui-only, the editor + runtime can be built from one
> `--workspace` build sharing a single `bevy_dylib` hash, which is what lets a
> community plugin load in both.

**Legend:** ✅ done · 🟡 in progress · ⬜ not started · 🧪 compiles, runtime-unverified

Branch: `ui_refactor`. Crate: **`renzora_ember`** (the bevy_ui framework).
Backends toggle at runtime with **F10** (Egui ↔ BevyUi) so migration is incremental.

---

## 1. Why a native framework, not "just use egui forever"

A community plugin built once must load in **both** the shipped editor and the
shipped runtime, which requires both to share an identical `bevy_dylib`. The
runtime has no egui; the editor is soaked in it. The path that unblocks the ABI
goal is: migrate the editor off egui onto bevy_ui (ember), then one workspace
build serves both. See the architecture doc for the full rationale (rename /
feature-alignment approaches were dead ends).

---

## 2. Infrastructure (ember) — ✅ done

- ✅ **Reactive layer** (`renzora_ember::reactive`): `bind_text/bind_bg/bind_display`,
  generic `bind_with`, raw `react`, granular `keyed_list`/`KeyedSnapshot` (`<For>`).
- ✅ **Two-way binding**: `Bound<T>` component + `bind_2way` (value-diffed both
  directions, external state wins ties). Widgets carry `Bound` (slider, fader,
  knob, checkbox, drag_value, vu_meter).
- ✅ **Panel sugar**: `register_panel_content(id, scroll, |commands, fonts| -> Entity)`.
- ✅ **Widgets**: button, checkbox, slider, fader, knob, drag_value (+ `drag_value_flat`,
  `DragRange`), mixer_button, popover/icon_popover, collapsible, scroll_view,
  text_input, line_chart_live, icon_text/icon_glyph, vec3_edit, color_picker.
- ✅ **Dock**: split/resize, tabs (drag, close, add), divider cursor.
- ✅ **Theme**: ember palette; the viewport header reads the live egui
  `ThemeManager` so chrome matches when both backends are visible.

---

## 3. Panel inventory (every editor panel)

~65 egui panels are registered. Status per panel below. "Native" = a
`register_panel_content` / `register_native_*` exists.

| Domain | Panel | Status | Notes |
|---|---|---|---|
| **Core** | Hierarchy | ✅ | |
| | History | ✅ | |
| | Inspector | ⬜ | **big**; per-component reflectors → registry rework (§5) |
| | Scenes | ⬜ | |
| | Level Presets | ⬜ | |
| **Assets** | Asset Browser | ✅ | grid, thumbnails, tree, ops, context menu, drag |
| **Viewport** | Viewport (3D) | ✅ | image + header A1–A6a; see §4 |
| | Camera Preview | ⬜ | Phase F |
| **Debug** | ECS / Memory / System / Performance / Render Stats | ✅ | chart-driven |
| | Camera Debug, Culling, Material Resolver, Lumen, Scripting | ✅ | |
| | Render Pipeline | ⬜ | |
| | Gamepad | ✅ | |
| **Console** | Console | ✅ | |
| **Code editor** | Code Editor (text surface) | ⬜ | highlighting/gutter/cursor; rewrite was reverted once |
| | Outline | ✅ | |
| | Scripts-on-Entity | ✅ | |
| | Problems | ⬜ | |
| | Palette / Shader Code | ⬜ | |
| **Scripting** | Script Variables | ✅ | |
| | Scene Diagnostics | ✅ | |
| **Mixer/Audio** | Mixer | ✅ | strips, buses, routing |
| | DAW, Sequencer | ⬜ | |
| **Animation** | Animation, Timeline, Animator Params, State Machine, Studio Preview | ⬜ | whole domain |
| **Blueprint** | Graph, Properties | ⬜ | node-graph UI |
| **Material editor** | Graph, Inspector, Preview | ⬜ | node-graph UI |
| **Particle** | Editor, Graph, Preview | ⬜ | node-graph UI |
| **Physics** | Physics Debug | ✅ | |
| | Playground, Properties, Forces, Metrics, Scenarios, Arena Presets | ⬜ | |
| **Network** | Monitor, Entities, Settings | ⬜ | |
| **Navmesh** | NavMesh | ⬜ | |
| **Hub** | Store, Library | ⬜ | |
| **Record** | Record | ⬜ | |
| **HUI demo** | Widget Palette, Gauges | ⬜ | demo panels |

**Migrated: ~22.** The fastest remaining wins are leaf/data panels (Scenes, the
remaining Physics panels, Network, Hub, Record). The structurally hard ones are the
**Inspector** and the four **node-graph editors** (Blueprint / Material / Particle /
Animation State Machine) — those need a bevy_ui graph-canvas widget (pan/zoom,
nodes, wires) that ember doesn't have yet, plus the registry rework (§5).

## 3a. How interactive functionality transfers once egui is gone

This is the important mental model. **egui was almost never where the behaviour
lived — it was only the *input source* and the *capture arbiter*.** The behaviours
(camera orbit, gizmo drag, picking, drop spawning, render toggles) are already
plain bevy systems that act on shared resources. They keep working unchanged; we
only re-source their *input* and re-implement egui's *"who owns the pointer"*.

**Worked example — orbiting the camera.** The editor camera controller lives in
`renzora_camera` and is a **pure bevy system**: it reads `MouseMotion`,
`MouseWheel`, and `ButtonInput<MouseButton>` directly, and gates on
`ViewportState.hovered`. That `hovered` flag is mirrored by the viewport resolver
from `ViewportResizeRequest`, which the **native panel publishes** (from its
`RelativeCursorPosition`). So mouse-drag look / orbit / scroll-zoom works in the
bevy_ui viewport with **no egui involved** — the behaviour never had to move. The
same holds for the transform gizmo, selection picking, and the drop *spawn* logic.

> ⚠️ **The seam where this broke (now fixed).** Two native-panel feed bugs made
> the camera/selection dead in the bevy_ui dock even though the systems were fine:
> (1) the resolver computed `docked` only from the **egui** `DockingState`, so in
> the bevy_ui dock the viewport read as "not docked" → `hovered` forced false →
> every gated system bailed. Fixed by OR-ing ember `Dock.tree.is_active_tab`.
> (2) `screen_position` was derived from a UI `GlobalTransform` of ambiguous
> coordinate space, throwing the picking ray off; now derived from
> `cursor + RelativeCursorPosition.normalized`. **Lesson: "moving functionality
> over" = making sure the native panel feeds the shared signals correctly, not
> rewriting the behaviour.**

So the three things egui actually provided, and their bevy_ui replacements:

1. **Per-widget input** (clicking a toolbar button, dragging a slider) →
   bevy_ui `Interaction` + small drag systems reading `ButtonInput`/cursor. Already
   done for every migrated widget (header buttons, `drag_value`, slider, divider).
2. **Pointer-capture arbitration** — egui's `wants_pointer_input()` /
   `is_pointer_over_area()` told the camera "a UI widget is using the mouse, don't
   orbit." The bevy_ui equivalent: the viewport `ImageNode`'s own `Interaction`
   (occlusion-aware — a panel on top steals it) and/or a global "any ember widget
   is `Pressed`/`Hovered`" check, folded into the `hovered`/`InputFocusState`
   signal. **This is the one genuinely new bit of glue** (Phase B), needed so the
   camera doesn't orbit when you drag a dropdown that overlaps the 3D view.
3. **Keyboard-focus gating** — don't fire shortcuts while typing in a field.
   egui's `wants_keyboard_input()` → track ember `text_input` focus into
   `InputFocusState`.

**Takeaway:** "make the UI functional" rarely means re-writing the behaviour — it
means (a) put `Interaction` + a small drag system on the new widget, and (b) for
viewport tools, make sure the panel keeps publishing hover/rect and add the
pointer-capture flag. Drop *detection* (release-over-viewport) is the other
egui-pass bit: swap `pointer.any_released()` for `ButtonInput::just_released` +
viewport hover + `AssetDragPayload` (Phase D).

---

## 4. Viewport (`renzora_viewport`)

**Key insight:** the 3D *display + interaction* are decoupled from the egui
chrome. The editor camera renders to an offscreen image
(`Viewports.slots[i].image`); every interactive system (gizmo, drop, nav) acts
through screen geometry published in `ViewportResizeRequest`. So a native panel
only has to (1) show the image and (2) report its rect/hover.

### Done
- ✅ **Native viewport panel** (`native_viewport.rs`): `ImageNode` of the slot's
  render target + `report_viewport_geometry` publishing size/hover/screen-rect to
  `ViewportResizeRequest` (all 4 slots). Scene visible + resizes with the panel.
- ✅ **Header — Phase A** (`native_header.rs`):
  - ✅ A1 left actions (undo/redo/save/play/scripts) + maximize
  - ✅ A2 View (3D/2D/UI) + Mode (Edit/Sculpt/Paint/Animate) dropdowns
  - ✅ A3 Display dropdown (visualization + render toggles + overlays + collision)
  - ✅ A4 Camera dropdown (projection, view-angle presets, sensitivities, reset) +
    Snap dropdown (object/floor snap, edge/scale-bottom)
  - ✅ A5 inline snap pairs (translate/rotate/scale) + camera speed (3D-only hide)
  - ✅ A6a registry-driven tool buttons (`ToolbarRegistry` → Transform/Terrain/Custom)

### Remaining
- ⬜ **A6b — mode/tool header drawers** *(hard, cross-cutting)*. `ModeOptionsDrawer`/
  `ToolOptionsDrawer` = `fn(&mut egui::Ui, &World)` registered by **plugin crates**
  (`renzora_mesh_edit` Edit header; `renzora_terrain_editor` sculpt/paint options).
  In egui these replace the inline snap strip when active. Needs: a native drawer
  contract (`fn(&mut Commands,&EmberFonts)->Entity` + rebuild on mode/tool change),
  reimplementing each plugin's controls in bevy_ui, and swapping snap-strip ↔ drawer.
  **This is the same registry rework the Inspector needs.**
- ✅/⬜ **Phase B — input glue.** Mouse-drag look/orbit and scroll-zoom **already
  work** (bevy system in `renzora_camera` gated on `ViewportState.hovered`, which
  the native panel publishes). View/render/snap/play keyboard shortcuts also
  already work. Remaining: (a) **pointer-capture flag** — the bevy_ui analog of
  egui's `wants_pointer_input` so the camera doesn't orbit when the cursor is over
  a UI panel overlapping the 3D view; (b) egui keyboard-focus → `InputFocusState`
  (track ember `text_input` focus); (c) viewport-hover crosshair + brush
  cursor-hide.
- ⬜ **Phase C — overlays** (egui-painted today): axis-orientation gizmo (3D-projected,
  clickable), nav overlay pan/zoom/grid/icons buttons, modal-transform HUD
  (bottom-center), model-load progress (bottom-left), play-mode console logs
  (top-left), resolution text (top-right), init placeholder, external-runtime
  full-screen overlay.
- ⬜ **Phase D — asset drops**: model/material/shape/sprite/scene/html drop
  *detection* is in egui `ViewportPanel::ui`; the preview/spawn systems are already
  bevy. Native path = detect from `AssetDragPayload` + viewport hover + release
  (lifecycle already prototyped in the asset browser's `asset_drag`).
- ⬜ **Phase F — CameraPreviewPanel**: `ImageNode` of the camera preview + name overlay.

### Already backend-agnostic (no work)
render_systems (wireframe/lighting/shadows/mesh/texture toggles, visualization
modes), debug_viz, play_mode, external_runtime polling, effect_routing,
persistence, model_flatten, `sync_viewport_camera_activation`, selection outline +
transform gizmo (bevy-rendered). The Display-dropdown *toggles* were the only
egui part of those.

---

## 5. The registry / drawer rework (the load-bearing remaining piece)

Both the **Inspector** and the viewport **A6b drawers** depend on the same thing:
plugins currently extend editor UI by registering **egui closures**
(`fn(&mut egui::Ui, &World)`). bevy_ui needs an equivalent contract so plugins
can contribute panels/drawers without egui. Designing this once unblocks:
- viewport mode/tool drawers (A6b),
- the Inspector's per-component reflectors,
- any future plugin-contributed editor UI.

Open decision: a build-closure contract (`fn(&mut Commands,&EmberFonts)->Entity`
rebuilt on state change) vs. a retained system-driven contract. Until this lands,
A6b and the Inspector stay blocked.

---

## 6. Cross-cutting polish (⬜ low-priority)
- Dropdowns don't close on click-outside (only on re-click / select).
- Header right-strip uses a flex spacer; egui caches a measured width.
- ember chrome (dock) uses a fixed palette; only the viewport header tracks the
  live `ThemeManager`. Unify if both-backends-visible parity matters.

---

## 7. Constraints (apply to all of the above)
- **No feature flags in ember** — shifts the `bevy_dylib` SVH/ABI.
- Can't build the full GPU editor locally; verify per-crate with
  `cargo clippy -p <crate> --no-deps -- -D warnings -A clippy::too_many_arguments -A clippy::type_complexity`.
  User verifies visually at runtime.
- Fix pre-existing clippy lints in touched crates when `-D warnings` flags them.
- Commit/push to `ui_refactor`, no Claude co-author trailer.
