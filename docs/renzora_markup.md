# Renzora Markup — Architecture & Status

The next-gen markup system: **markup is just a serialization format for a
`bevy_ui` entity tree.** Edit the markup, edit the tree — both round-trip
through each other. Replaces bevy_hui's "opaque builder + per-frame style
re-assertion" model.

This doc supersedes §6 of [`ui_plan.md`](./ui_plan.md). The rest of that doc
(scripting bridge, editor wiring, Cinder, shader effects) still stands; this is
the runtime/loader layer underneath.

> **Actual implementation status.** The new model is the live model: the
> `renzora_hui` loader spawns real `bevy_ui` entity trees and powers the
> editor's HUI panels. Phases A, B, and D are **complete**; Phase C is
> **partial** (inspector→`.html` writeback works via `writeback.rs`, but there
> is *no* hot-reload-on-`Modified` respawn, so the round-trip loop isn't
> closed); Phase E (**cleanup**) is **not started** — the vendored `bevy_hui`
> fork is still full and `renzora_hui` is not yet renamed to `renzora_markup`.
> The rest of this doc is the design rationale; §5 carries the per-phase status.

---

## 1. The shift

### Old model (bevy_hui's)
- `<template>` → one `HtmlNode` entity with `HtmlStyle.computed` (target values).
- A per-frame system (`update_node_style`) continuously clones `computed.node`
  onto `Node`, `computed.background` onto `BackgroundColor`, etc.
- Hot-reload calls `retain::<KeepComps>()` which strips *every* non-bevy_hui
  component on the host entity.

This model produced every UX bug we hit: dragging snapped back because
`update_node_style` overwrote `Node` every frame; custom components got stripped
on hot-reload; the inspector showed `HtmlNode`/`HtmlStyle`/`Tags` rather than
the entity tree the designer actually wrote; selection/handles/click-through
all had to fight the "one black-box entity per template" assumption.

### New model
- `<node flex_direction="row" width="260px">` → **one real entity** with
  `Node { display: Flex, flex_direction: Row, width: Px(260), .. }` directly.
- `<text font_color="#FFF">Hello</text>` → entity with `Text("Hello")`,
  `TextFont`, `TextColor`.
- The hierarchy panel literally shows the markup. The inspector shows the
  styles as bevy_ui components you edit normally.
- No `HtmlStyle`, no `computed`, no per-frame fight. Components hold the truth.
- The editor's drag/resize/edit writes back to the `.html` file. File watcher
  reloads → walk the AST → respawn the tree. Round-trip.

This is the model Unity's UI Toolkit and Godot's `.tscn` both use internally:
the markup file is the persistence layer for an entity/node tree the editor
operates on directly.

---

## 2. What we keep, what we throw away

The fork is already in `crates/bevy_hui/` (pristine 0.6.0, vendored). We'll
gut it incrementally.

### Keep — the parser (~2000 lines, well-tested nom)
- `parse.rs` — XML+attr parsing.
- `data.rs` — `HtmlTemplate`, `NodeType`, `StyleAttr`, `Attribute`, `Action`.
- `error.rs` — parse errors.
- `util.rs` — SlotMap.

### Throw away — the runtime model
- `build.rs` — `BuildPlugin`, `spawn_ui`, `hotreload`, `TemplateBuilder`.
- `styles.rs` — `HtmlStyle`, `ComputedStyle`, `update_node_style`,
  `apply_computed`, `apply_interpolated`, `InteractionTimer`.
- `compile.rs` — `CompilePlugin`, `CompileContextEvent`, expression compiler.
- `animation.rs` — atlas/flipbook animation.
- `bindings.rs` — `FunctionBindings`, `HtmlComponents`, `HtmlFunctions`.
  (We'll rebuild a smaller version of this for our Lua callback bridge.)
- `auto.rs` — folder-autoload of components.

Everything in the second list goes away because in the new model there's no
intermediate `HtmlStyle`/`computed` to manage, no per-frame style update, no
opaque "scope root" entity that needs preserving across rebuilds.

---

## 3. What we build

A small new runtime that turns the parsed AST into entities and back. Crate
layout (using the existing `crates/renzora_hui/`, to be renamed later):

```
crates/renzora_hui/
├── src/
│   ├── lib.rs            – HuiPlugin (registers loader + watcher)
│   ├── loader.rs         – AST → entity tree (the new core)
│   ├── writeback.rs      – span-tracked attribute writeback to .html (inspector edits)
│   ├── transitions.rs    – small hover/pressed transition system
│   ├── lua_bridge.rs     – on_ui Lua hook (kept as-is)
│   ├── template.rs       – HtmlTemplatePath observer (now triggers the loader)
│   ├── binding.rs        – reactive data bindings
│   ├── interactions.rs   – Interaction → MarkupOnPress/etc. dispatch
│   ├── input_field.rs    – text input widget
│   ├── widgets.rs        – built-in widget components
│   ├── foreach.rs        – list/repeat (`<for>`) expansion
│   ├── drag.rs           – element drag handling
│   ├── dnd.rs            – drag-and-drop
│   ├── cursor.rs / cursor_icon.rs – cursor state + icon
│   ├── icons.rs / phosphor_map.rs – icon tags + phosphor glyph map
│   ├── vector.rs         – vector graphics
│   ├── decor.rs          – decorative styling helpers
│   └── provenance.rs     – MarkupSource span provenance (powers writeback)
```

There is no `saver.rs` — attribute writeback lives in `writeback.rs`, and
hot-reload is not yet its own module (see §5 Phase C). The editor-only
registrations (`editor.rs`) live in a separate `crates/renzora_hui/editor/`
subcrate rather than under `src/`.

### Loader (`loader.rs`) — the core
Walks the parsed AST and spawns one entity per markup node:

| Markup | Entity gets |
|---|---|
| `<node display="grid" gap="8px" width="100%">` | `Node { display: Grid, row_gap: 8px, .. }` |
| `<text font_size="14" font_color="#FFF">Hi</text>` | `Text("Hi")`, `TextFont { font_size: 14, .. }`, `TextColor(#FFF)` |
| `<image src="ui/panel.png" image_mode="slice">` | `ImageNode { image, mode: Sliced(..), .. }` |
| `<button on_press="start_game">` | as a `<node>` + `Interaction` + a `MarkupOnPress("start_game")` component |
| `id="menu_root"` on any node | `MarkupId("menu_root")` for stable identity / find-by-id |
| `class="primary"` | `MarkupClass("primary")` for the future CSS layer |

Property substitution (`{label}`) and slot insertion happen at load time, not
runtime — the substitution result is just baked into the attribute values
before they become component fields.

### Writeback (`writeback.rs`)
The dual of the loader: when the inspector edits an attribute on a
markup-built entity, `write_attr_to_markup` looks up the entity's
`MarkupSource` (from `provenance.rs`), finds the recorded byte span for that
attribute in the cached `HtmlTemplate::source`, patches the bytes, and
rewrites the `.html` on disk. Preserves comments, formatting, and attribute
order; downstream spans are delta-shifted so subsequent edits in the same
session keep targeting the right ranges.

### Hot-reload (not yet implemented)
The intended design: file change → despawn the old subtree under the template
entity → re-run the loader. Simple because there's no "in-place state to
preserve" — every rebuild just walks the new file. Not wired up yet (see §5
Phase C); writing the file via `writeback.rs` does *not* currently trigger a
respawn.

### Transitions (`transitions.rs`) — small replacement for `HtmlStyle.hover`
For `hover:background="#X"` etc. we add a small `Transitions` component on the
entity declaring them:
```rust
struct Transitions {
    hover: Vec<(StyleField, AttrValue)>,
    pressed: Vec<(StyleField, AttrValue)>,
}
```
A system reads `Interaction` + `Transitions` and lerps the affected components.
Coupling is local — no HtmlStyle-wide `computed` indirection.

### Custom components
A registry maps tag name (`<menu_button>`) → an `HtmlTemplate` (the
component's `.html`). When the loader hits a custom tag, it instantiates that
template as a sub-tree with property substitution applied. Same semantics as
bevy_hui but spawning real entity components instead of an HtmlNode child.

### Lua bridge (`lua_bridge.rs`)
Unchanged — the `on_ui(name, args, entity)` Lua hook continues to work. The
trigger source becomes our own `MarkupOnPress` interaction handler instead of
bevy_hui's.

---

## 4. Editor benefits unlocked

These all fall out of the new model "for free" — no per-feature workarounds:

- **Inspector shows real components.** Click a `<text>` → `Text`, `TextFont`,
  `TextColor` in the inspector. Edit them like any other entity. Saver
  writes back.
- **Hierarchy mirrors markup.** Designer-readable tree.
- **Per-element select/drag/edit.** Click the speedometer's RPM bar
  specifically; drag it; the saver writes its attribute changes.
- **Per-element scripts.** Attach a `ScriptComponent` to a specific button
  entity. No glue code needed.
- **Click-through transparent gaps.** Falls out of hit-testing real entities.
- **No "two entity" workarounds.** One entity per markup node. Period.

---

## 5. Phased implementation — status

### Phase A — vertical slice (proves the model) — COMPLETE
1. `loader.rs` now handles far more than `<node>`/`<text>`: full `<node>` /
   `<text>` / `<image>` / `<button>` styling (flex + grid layout, box model,
   colors, borders, font size/color, text content), plus icons, input fields,
   and widget tags.
2. `template::on_template_path_inserted` calls the new loader instead of
   bevy_hui's runtime.
3. Existing demo templates (`health_bar`, `speedometer`, `scoreboard`,
   `inventory`, `hud`) render through it.
4. Bevy_hui's `BuildPlugin`/`TransitionPlugin` are no longer added to the app.
5. Compile-clean. The renzora_hui crate still depends on bevy_hui *just for
   the parser+data types*.

### Phase B — composition — COMPLETE
1. Property substitution (`{label}` → attribute value, via `AttrTokens::compile`,
   in both attribute values and text content).
2. Slot insertion (`<slot/>` → caller's children get reparented here at load
   time).
3. Custom component registry (`<menu_button>`, `<stat_bar>`, … → looked up in
   `ComponentRegistry` and instantiated as a sub-tree with merged overrides).

### Phase C — round-trip + hot-reload — PARTIAL
1. **Not implemented.** No hot-reload system on
   `AssetEvent<HtmlTemplate>::Modified` → despawn subtree, re-run loader.
   Writing the file does not currently trigger a respawn (see the NOT-here note
   in `writeback.rs`).
2. **Done.** Writeback: inspector attribute edits → span-tracked patch written
   to `.html` (`writeback.rs` + `provenance.rs`).
3. The bidirectional file-watcher loop depends on (1) and is therefore not yet
   closed.

### Phase D — interaction — COMPLETE
1. `transitions.rs`: hover/pressed background + border overrides resolve
   `pressed → hover → base` against `Interaction` (no per-frame `HtmlStyle`
   re-assertion).
2. `interactions.rs`: `Interaction` → `MarkupOnPress`/etc. → the existing Lua
   `on_ui` hook.

### Phase E — cleanup — NOT STARTED
1. The vendored bevy_hui fork is still full (`build.rs`, `styles.rs`,
   `compile.rs`, `animation.rs`, `bindings.rs`, `auto.rs`, … all present); it
   has not been stripped down to `parse.rs`/`data.rs`/`error.rs`/`util.rs`.
2. `crates/bevy_hui/` has not been renamed to `crates/renzora_markup/`.
3. The `renzora_hui` / `bevy_hui` name split therefore still stands.

---

## 6. What's committed now

The `renzora_hui` loader (`loader.rs`) is the live runtime: it walks the parsed
`HtmlTemplate` and spawns real `bevy_ui` entity trees, and bevy_hui's
`BuildPlugin`/`TransitionPlugin` are no longer added to the app. Composition
(property substitution, slots, custom components), transitions, interaction
dispatch, and inspector→`.html` writeback are all in place.

The vendored `bevy_hui` fork is still present (full, not yet stripped) and the
workspace points at `crates/bevy_hui/` via a `path = "../bevy_hui"` dep on
`renzora_hui` — `renzora_hui` consumes it only for the parser + data types
(`parse.rs`/`data.rs`/`error.rs`/`util.rs`). Remaining work: close the
hot-reload round-trip (Phase C) and the bevy_hui strip-down + rename (Phase E).

---

## 7. Non-goals (for now)

- **Full CSS support** — external stylesheets, selectors, `@keyframes`. Tracked
  in `ui_plan.md` §7-8 as a later phase.
- **Atlas/flipbook animation** — bevy_hui had it (animation.rs); we'll skip in
  v1 and add later as a sprite system if needed.
- **bevy_hui parity for `tag:` attributes** — those become a generic
  `MarkupTags` component, same semantics, but no special runtime.
