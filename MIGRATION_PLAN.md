# Renzora UI migration: egui → bevy_ui + bevy_feathers

Living document. Captures the why, the architecture, the state of work, and the remaining steps so this migration can be picked up cold on a different machine.

## 1. Why retained

Renzora is a shipping engine targeting external devs across 7 platforms. Immediate-mode (`bevy_egui`) is great for internal tools and fast iteration, but over the project's lifetime it will hold back:

- **Node graphs** (blueprint, shader, material, particle editors). Per-node drag state, connection rubber-banding, zoom/pan, and connection lines are awkward in IMGUI; mature in retained.
- **Timelines** (animation editor, DAW). Persistent scrub state, clip drag, interpolation editing — retained is the natural fit.
- **Theming and polish**. An engine used by others will be compared to Unreal/Godot/Unity. egui has a theming ceiling that's hard to exceed.
- **Accessibility**. egui has no story for screen readers or OS-level automation. bevy_ui integrates with AccessKit.
- **Animation and transitions**. Hover fades, expand/collapse, tab slide — trivial in retained, fiddly in IMGUI.
- **Editor + in-game UI unification**. With bevy_ui, both the editor and the game HUD can share primitives — ECS-native widgets work in both.

Trade-off accepted: slower per-panel iteration, more code for the same inspector, and riding Feathers' experimental API (pre-stable, breaks with each Bevy release).

## 2. Stack choice

| Layer | Choice | Status |
|---|---|---|
| Retained UI foundation | `bevy_ui` (Bevy 0.18) | Stable |
| Widget library | `bevy_feathers` | Experimental, gated behind feature flag in Bevy 0.18. Expect API churn per release. |
| 2D path rendering (node graphs, timelines) | `bevy_vello` (or meshes + Gizmos overlay for transient) | To adopt when specialized editors land |
| Animations / transitions | Custom `Transition<T>` component + systems, or `bevy_tweening` | To build |
| Cross-panel state sync | `renzora_reactive` (this repo) | **Done — 5/5 tests pass** |

Feathers is not production-ready. The bet is that aligning with the Bevy editor effort gets you the ecosystem improvements automatically, in exchange for a ~half-week tax per Bevy release to catch up to API changes.

## 3. State management: `renzora_reactive`

The load-bearing piece. Built first because every panel depends on it.

### Principle

ECS is the only source of truth. Widgets never cache values. One sync loop fans every mutation out to every widget that cares. Panels stop knowing about each other.

### Primitives

- **`Bound`** — component on a UI widget entity, holding a `source` (how to read), optional `sink` (how to write), and cached `last` value (what the sync loop last observed).
- **`BindSource`** — enum: `EntityField { entity, getter }`, `ResourceField { getter }`, `SelectedField { getter }` (follows editor selection), `Computed { getter }`. Getters are `fn` pointers (not closures) so the binding stays `Clone + Send + Sync`.
- **`BindSink`** — matching enum for write-back. Read-only bindings omit it.
- **`BoundValue`** — enum carrying the value: `Bool`, `F32`, `Vec3`, `Color`, `String`, `Entity`, `Asset`, `EnumTag`, etc. Mirrors the shape of `FieldValue` in the inspector registry so inspector getters lift into bindings verbatim.
- **`BindingChanged`** / **`CommitBinding`** — Bevy messages (0.18 renamed `Event` → `Message` for queued events).

### Flow

1. Panel spawns widget entity with a `Bound` component.
2. `sync_bindings` (Update, exclusive) reads every binding's source, compares to `last`, emits `BindingChanged` only on actual change.
3. Widget systems observe `BindingChanged` filtered by `WidgetKind` and update their visible components.
4. On user edit, widget emits `CommitBinding { widget, value }`.
5. `apply_commits` (Last, exclusive) runs the binding's sink. ECS mutation triggers `Changed<T>`. Next tick's `sync_bindings` picks up the new value and fans it to **every other widget** bound to the same source — automatic cross-panel propagation.

### Why it's granular

Bevy's `Changed<T>` fires at component granularity. `renzora_reactive` adds *value* granularity on top: the sync loop diffs the actual returned `BoundValue` against the cached one, so editing `Transform.translation` doesn't wake widgets bound to `Transform.rotation` even though both live on the same component.

### Design decisions recorded

- **Exclusive systems, not parallel.** `sync_bindings` needs `&World` (for getters) and `&mut Bound` (for `last` updates) — would be a borrow conflict in a parallel system. Editor scale (hundreds of widgets) doesn't need parallelism.
- **`fn` pointers, not closures.** Binding sources live on components; closures would require boxing per-widget. `fn` pointers keep `Bound: Clone + Send + Sync` for free.
- **No reflection yet.** Getters/setters are hand-written. Reflection-driven field paths (`"transform.translation.x"`) are TODO — add when inspector migration hits components with dozens of fields.
- **`SelectionProvider` resource.** The reactive crate doesn't depend on `renzora_editor_framework` (cycle). Instead, the framework installs a small `SelectionProvider { get: fn(&World) -> Option<Entity> }` resource that the reactive crate reads for `SelectedField` bindings.
- **Read-only widgets that emit commits warn once.** Indicates a widget-implementation bug; surfaces the issue without crashing.

### Helpers shipping today

- `BindSource::entity_name(e)` / `BindSink::entity_name(e)` — entity `Name` component
- `BindSource::selected_name()` / `BindSink::selected_name()` — current selection's Name
- `BindSource::entity_translation(e)` / `BindSink::entity_translation(e)` — `Transform.translation` as Vec3
- `BindSource::selected_translation()` / `BindSink::selected_translation()` — selection-tracked
- `BindSource::entity_visibility(e)` / `BindSink::entity_visibility(e)` — `Visibility` as Bool

Add more as panel migrations touch new components.

### Tests (5/5 passing)

- `initial_sync_fires_binding_changed` — first sync always emits so widgets paint initial state
- `no_event_when_value_unchanged` — idempotent ticks are silent
- `cross_panel_name_sync` — **load-bearing**: edit via widget A's sink, widget B receives the change automatically
- `selection_tracked_binding_follows_selection` — changing the selection resource swaps which entity the widget reads from
- `commit_to_read_only_widget_is_ignored` — defensive: no panic, no mutation, warning logged

Run with: `cargo test -p renzora_reactive --lib`

## 4. Painting & animation (not yet built)

Notes captured during the design conversation so we don't have to re-derive.

### Painting (what replaces `ui.painter()`)

| egui | bevy_ui |
|---|---|
| `rect_filled(rect, radius, color)` | Entity with `Node + BackgroundColor + BorderRadius` |
| `rect_stroke(...)` | Same + `BorderColor + Outline` |
| `image(handle)` | `ImageNode` |
| Styled text | `Text` entity with `TextSpan` children |

**Arbitrary 2D paths** (node-graph beziers, marquee rect, timeline curves):

- `Gizmos` API — transient per-frame lines/rects/circles, good for marquee, hover highlights, viewport overlays
- `Mesh2d` / sprites — persistent, regenerate on change, good for committed node-graph connections
- `bevy_vello` — GPU path rasterizer, best quality for bezier curves; adopt when specialized editors land

### Animation

- **Transition component** to build: `Transition<T> { from, to, progress, duration, easing }`; one system per animated property lerps `progress` into the visible component. Self-removes on completion.
- Alternative: `bevy_tweening` crate — production-tested.
- Integration with reactive layer: a widget receiving `BindingChanged` inserts a `Transition` toward the new value instead of snapping. Binding layer doesn't need to know about animation — it just delivers the target.

## 5. Migration plan (phased)

### Phase 0 — Foundation ⏳ (partially done)

- [x] **renzora_reactive** binding layer + 5 passing tests + README
- [ ] Add `bevy/bevy_feathers` feature flag to workspace `[workspace.dependencies]` bevy spec
- [ ] Rewrite `renzora_theme` to use `bevy::prelude::Color` instead of `egui::Color32` (hex serialization layer stays the same)
- [ ] Extract exact theme tokens from the reference screenshot into the dark theme (currently approximate):
  - Background: ~#0b0b11 (window), ~#1a1a1f (panel)
  - Accent tab underline: subtle, not saturated
  - Green accent for enabled toggles
  - Orange/amber for labels (~#f2a640)
  - Section chevrons + category accent colours (match current `CategoryColors` table — already correct)
- [ ] Install `SelectionProvider` resource in `renzora_editor_framework` startup, pointing at existing `EditorSelection`

### Phase 1 — Core widgets in `renzora_ui`

Everything `bevy_ui + feathers` lacks. Each widget exposes a `Bound`-compatible constructor.

- [ ] Tab bar (horizontal + vertical, closable tabs, hover, active underline)
- [ ] Collapsible section (header + body, chevron animation, stored expanded state — bind to per-entity `Expanded(bool)` component)
- [ ] Property row (label left, editor right, reset button, 2-column grid support)
- [ ] Search input (prefix icon, clear button, binding-friendly)
- [ ] Icon button (phosphor icons — use `bevy_ui` equivalent or SDF font atlas)
- [ ] Scrollable tree (hierarchy-shaped, virtualized rows, expand/collapse per node)
- [ ] Scrollable grid (asset browser-shaped, virtualized, multi-select)
- [ ] Dockable panel container (tab group + splitter)
- [ ] Splitter (horizontal + vertical, draggable divider)
- [ ] Context menu primitive
- [ ] Modal / popup primitive
- [ ] Toast (slides in from corner, auto-dismiss)

### Phase 2 — Editor shell (`renzora_editor_framework`)

- [ ] Top menu bar: File / Edit / Help
- [ ] Mode tab bar: Scene, Blueprints, Scripting, Animation, Materials, Particles, Shaders, UI, Physics, Audio, Debug
- [ ] Project dropdown (top-right), settings icon, renzora branding
- [ ] Status bar: FPS, memory, GPU info, right-aligned status items
- [ ] Main dock layout: persist to disk, restore on launch (existing `LayoutManager` logic ports over — replace `egui_dock` with custom split layout on bevy_ui)
- [ ] Window chrome (custom titlebar, min/max/close)
- [ ] Splash screen (on boot, before editor UI loads)

### Phase 3 — Pilot panel (proves the pattern end-to-end)

- [ ] **Hierarchy** — search, +Add, tree with per-row visibility/favorite icons, chevrons, drag reorder, drag-reparent, marquee selection, label colours. Binds every row to its entity's `Name` via `BindSource::entity_name`.

### Phase 4 — Inspector (proves reflection-driven grid)

- [ ] Reflection-based property grid: given a component, walk `bevy_reflect` fields and spawn bound widgets per field. Getters/setters become auto-generated. This unlocks every "mostly fields" panel.
- [ ] Inspector panel: Name editable, Transform collapsible, per-component collapsibles with enable toggle + remove, +Add Component overlay.
- [ ] Per-field reset button → sets via sink to a `FieldValue::type_default()` equivalent.

### Phase 5 — Forms-and-lists panels (the easy 60%)

Can go in any order after hierarchy + inspector prove the patterns. All use property grid, scroll list, or scroll tree.

- [ ] Settings, Keybindings
- [ ] System Monitor (uses `BindSource::Computed` for FPS/mem/GPU stats)
- [ ] Command Palette, Context Menu, Widget Gallery
- [ ] Scene Manager, Console, Import UI
- [ ] Physics Playground, Level Presets
- [ ] Foliage Editor (list side), Terrain Editor (list side), Network Editor (list side)
- [ ] Debugger

### Phase 6 — Viewport area

- [ ] Viewport tabs (viewport, code editor, node_explorer)
- [ ] Transform tool toolbar, play/pause, snap controls, render mode toggles
- [ ] Camera preview inset
- [ ] Shapes palette grid (drag-to-spawn)

### Phase 7 — Bottom dock

- [ ] Assets browser (tree + virtualized grid + search + import)
- [ ] Marketplace, Console (reuse), Mixer

### Phase 8 — Specialized widgets

These are real software projects on their own — not mechanical translations.

- [ ] **Node graph** — pannable canvas, bezier connections (`bevy_vello`), per-node drag, multi-select marquee, connection rubber-band, hit-testing, zoom, grid backdrop. Used by blueprint, shader, material, particle editors.
- [ ] **Timeline** — horizontal tracks, keyframes, clips, scrub, zoom, snap. Used by animation editor + DAW.
- [ ] **Code editor** — text buffer, syntax highlighting, folding, selection, IME, large-doc virtualization. Equivalent to the current VSCode-style egui implementation — biggest single widget.

### Phase 9 — Specialized editors on top

- [ ] Blueprint editor
- [ ] Shader editor
- [ ] Material editor
- [ ] Particle editor
- [ ] Animation editor
- [ ] DAW
- [ ] Code editor panel (wire up the widget)

### Phase 10 — Retire egui

- [ ] Remove `bevy_egui` and `egui-phosphor` from `[workspace.dependencies]`
- [ ] Delete remaining egui imports
- [ ] Full workspace build + run
- [ ] Remove `bevy_egui` from `renzora_ui`, `renzora_theme`, `renzora_editor_framework` Cargo.tomls

## 6. Gotchas and notes

- **Bevy 0.18 renamed queued events to Messages.** `#[derive(Event)]` is now for observer-triggered events. Use `#[derive(Message)]` + `MessageReader/Writer` + `Messages<T>` + `app.add_message::<T>()` for the old `Event` shape. The reactive crate already does this.
- **`&World` as a system param conflicts with `Query<&mut T>` in a parallel system.** Use exclusive systems (`&mut World`) when both are needed. The reactive crate uses this pattern.
- **Feathers is experimental.** Gated behind a Bevy feature flag. Every Bevy release will likely break something — budget a half-week per bump to catch up.
- **`bevy_ui` has no built-in animation.** Must be written (see §4) or pulled in via `bevy_tweening`.
- **`bevy_ui` has no built-in arbitrary 2D paths.** For node graphs, pull in `bevy_vello` — don't try to build beziers out of `Node` entities.
- **`Cargo.lock` checked in.** Workspace-wide dependency version lock.

## 7. State of this branch (`retainer_mode`)

On commit:

```
crates/renzora_reactive/
├── Cargo.toml
├── README.md
└── src/
    ├── binding.rs     # Bound, BindingChanged, CommitBinding, sync_bindings, apply_commits
    ├── helpers.rs     # BindSource::entity_name, ::selected_translation, etc.
    ├── lib.rs         # public API
    ├── plugin.rs      # ReactivePlugin registers messages + systems
    ├── source.rs      # BindSource, BindSink, SelectionProvider
    ├── tests.rs       # 5 tests, all passing
    └── value.rs       # BoundValue enum
```

Workspace `Cargo.toml` has `crates/renzora_reactive` added to `[workspace.members]`.

Nothing else in the repo is touched on this branch. The egui editor continues to work on `main`; the reactive layer is additive.

## 8. Next session, start here

1. `cargo test -p renzora_reactive --lib` — confirm 5/5 still pass.
2. Phase 0 checklist above — feathers feature flag, theme rewrite to `bevy::Color`, SelectionProvider install.
3. When you reach Phase 3 (hierarchy pilot), the reactive layer stops being theoretical and becomes load-bearing — that's the validation point for the whole approach.
