# Multi-Window Editor (design)

> **Status: design / not yet implemented.** This captures the architecture for
> two related features — **projecting the viewport onto a second monitor** and
> **undocking panels into their own OS windows** — so that when the Bevy 0.19
> migration settles we execute instead of re-deriving. It does **not** describe
> shipped behaviour. Nothing here exists in the editor today.

## Why this gates on Bevy 0.19

Multi-window editor UI leans on capabilities that land or improve with the 0.19
bump (see `docs/BEVY_0.19_MIGRATION.md`): richer `Monitors` / window APIs for
placing a window *on* a chosen display, the render-graph-as-systems port, and
cleaner per-camera UI targeting. **Do not build this on the in-flight
`bevy-0.19-migration` branch before the forced port is through** — it would
stack a new feature on a workspace that does not yet fully build. Sequence:
finish the migration → second-viewport window → undock one panel → generalize.

## The load-bearing fact: one App, one World

In Bevy, **multiple windows are not multiple processes.** The entire editor is a
single `App` with **one `World`**. A second OS window is just another `Window`
*entity* in that same World, displayed by a camera that targets it via
`RenderTarget::Window`.

This collapses the two questions people always ask:

- **"Would the windows share a single instance?"** — There is only ever one
  instance. An undocked Inspector on your second monitor is the *same* Inspector,
  in the *same* process, reading the *same* ECS data. No second engine, no second
  scene copy, no reconciliation.
- **"How do the windows communicate?"** — They don't need IPC. Panels already
  talk through **shared ECS resources**, and that is window-agnostic:
  - Hierarchy click → `EditorSelection::set(entity)`
    (`crates/renzora/src/editor_contract/selection.rs`)
  - Inspector reads `EditorSelection::get()` → shows that entity
  - Viewport reads it to frame on **F**
  - `EditorContext` (`renzora_ui::document_tabs`), `ViewportState`, `Dock`
    (`renzora_ember::dock`) are all global resources

  Move a panel's `Node` tree to a second window's camera and it keeps reading the
  exact same resources. Click in the main-window hierarchy, the floating
  Inspector updates the same frame. This is the decisive advantage over an
  Electron / multi-process editor, where undocking forces an IPC protocol and
  duplicated-state reconciliation — we skip that entirely.

## How the relevant subsystems are shaped today

| Concern | Where | Note |
|---|---|---|
| Dock layout model | `renzora_ember/src/dock.rs` (`DockTree`: Split / Leaf / Empty) | Clean `remove_panel` / `split_at` / `focus_or_add_panel` already exist. |
| Panel content | `renzora_ember/src/panel.rs` (`register_panel_content`) | A panel is a bevy_ui `Node` tree parented under a dock leaf's `content` entity; built lazily when its tab activates. |
| UI camera | one default 2D UI camera → primary window | All chrome renders to the primary window; no per-panel cameras. |
| Viewport render | `renzora_viewport/src/lib.rs` (`setup_viewport`), `native_viewport.rs` | Renders **to an `Image`** (`RenderTarget::Image`), shown as an `ImageNode`. Content is already decoupled from where it's displayed. |
| Viewport slots | `renzora/src/core/viewport_types.rs` (`Viewports`, 4 slots) | Already built for up to 4 cameras/targets — the multi-viewport groundwork. |
| Floating overlays | `renzora_ember/src/widgets/popup.rs` (`OverlaySurface`, `PointerOverOverlay`) | Z-ordered via `GlobalZIndex` within **one** window canvas. |

## Milestone 1 — viewport on a second monitor

Lowest-risk because the viewport **already renders to a decoupled `Image`** and
the `Viewports` array already supports 4 slots. Two flavours:

- **Mirror (cheap):** spawn a `Window`, give it a 2D camera + a full-screen
  `ImageNode` showing the existing slot-0 viewport image. Single render, shown in
  two places. Good for "same view on a projector/TV."
- **Dedicated camera (flexible):** spawn a `Window` and a 3D camera with
  `RenderTarget::Window(that_window)`. Independent pan/zoom, or a true
  full-screen *play* output on monitor 2 while you keep editing on monitor 1.
  Costs a second render pass.

**Monitor placement** is the one genuinely new dependency: query the `Monitors`
resource and place the window via `MonitorSelection` / monitor coordinates.
Bevy 0.19's monitor enumeration is thin; 0.19 is the moment to wire it. Extend
`WindowConfig` (`crates/renzora/src/core/mod.rs`) with an optional target-monitor
index and persist it in project config. Fullscreen today is hardcoded to
`MonitorSelection::Current` in `apply_window_config()`
(`crates/renzora_runtime/src/lib.rs`).

## Milestone 2 — undock one bevy_ui panel

The dock-model side is easy: "undock" = `remove_panel` from the tree + spawn a
`Window` + reparent that panel's `Node` under the new window's UI root.
"Re-dock" = reverse it. The four real challenges are all rendering/plumbing, not
architecture:

1. **Per-window UI targeting.** UI nodes must belong to a camera that targets the
   secondary window (`UiTargetCamera` / per-window UI root). **Verify the 0.19
   story first** — this is the single biggest unknown.
2. **Input routing.** Bevy delivers pointer/keyboard events per window;
   `bevy_picking` and the hover gates (`PointerOverOverlay`,
   `ViewportState.hovered`) assume the primary window. Cursor→world math is
   `screen_position`-relative per slot, so it is *mostly* parameterized already —
   it needs to key off *which* window.
3. **Overlay Z-order is per-window.** `GlobalZIndex` only orders within one
   window. A dropdown opened in a floating Inspector should layer within *that*
   window — usually what you want, but popup code currently assumes one canvas.
4. **Persistence.** `layout.json` (`PersistedLayout`) grows per-window
   position / size / monitor. Natural extension of the existing schema.

## Milestone 3 — generalize

Undock/re-dock across all panels + window-position persistence + restoring the
floating-window set on editor start.

## Suggested sequencing (after the 0.19 migration)

1. Second viewport window (mirror or dedicated) — proves multi-window + the
   monitor-selection API.
2. Undock one bevy_ui panel (e.g. Inspector) — proves UI-on-secondary-camera +
   input routing (challenges 1 & 2 above).
3. Generalize undock/re-dock + persistence.
