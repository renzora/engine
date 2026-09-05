# Making Edits Undoable

How to route a panel's edits through the shared undo/redo system so they land on the right history and show in the History panel.

## The model: one command stack, per-context

Undo lives in `renzora_undo`. It is a **command** system, not a snapshot-of-the-world system: every user action is an `UndoCommand` with an `execute` and an `undo`. Commands are stored on per-context stacks (`UndoStacks`), keyed by `UndoContext`:

- `UndoContext::Scene` — the viewport, hierarchy, inspector and terrain all share this one.
- `UndoContext::MaterialGraph(path)` / `Blueprint(path)` — a self-contained document editor's own stack.
- `UndoContext::Other(path)` — any other asset document.

`Ctrl+Z`/`Ctrl+Y` are handled centrally (`UndoPlugin`) and always act on `UndoStacks::active`. You do **not** write a keyboard handler — a plugin that hard-codes its own `Ctrl+Z` (terrain used to) fights the router and stays invisible to the History panel.

`route_undo_context` keeps `active` pointed at the focused document (from `EditorContext` + the dock's `FocusedPanel`), so you rarely set it yourself.

## The one rule: don't mutate the world directly — record a command

Instead of writing the change straight into the world from your panel, build a command and hand it to `renzora_undo`:

```rust
// You already applied the change during a gesture → record it (no re-apply):
renzora_undo::record(world, ctx, cmd);

// You have NOT applied it yet → execute runs it, then records:
renzora_undo::execute(world, ctx, cmd);
```

Use `renzora_undo::active_context(world)` for `ctx` when the edit belongs to "whatever the user is looking at" (the inspector does this); pass an explicit context for a document editor.

## Pick the right command

**A field/property edit** → reuse `FieldChangeCmd`. It wraps the inspector's `get`/`set` fn-pointer signature and captures old/new. This is how every inspector field is undoable:

```rust
renzora_undo::execute(world, renzora_undo::active_context(world),
    Box::new(renzora_undo::FieldChangeCmd { entity, field_name, old, new, set_fn }));
```

`FieldChangeCmd::merge` folds consecutive edits of the *same* field into one step, so a drag that fires every frame is a single entry.

**A blob edit** (heightmaps, tilemap grids, particle/animation assets — anything awkward to express as a fine-grained command) → use `SnapshotCmd<S>`. Capture a `before` blob when the gesture starts and an `after` blob when it ends, plus a `restore` fn; `record` it (the mutation already happened live):

```rust
renzora_undo::record(world, renzora_undo::UndoContext::Scene,
    Box::new(renzora_undo::SnapshotCmd {
        label: "Terrain".to_string(), before, after, restore: restore_terrain,
    }));
```

`fn restore_terrain(world: &mut World, blob: &TerrainUndoEntry)` writes the blob back. `undo` restores `before`, redo restores `after`. See `renzora_terrain_editor::systems::terrain_stroke_end_system` for a full example.

**Deleting entities** → call `renzora_undo::delete_entities_with_undo(world, &entities)`. It snapshots each entity's whole subtree (all components + children) to a BSN string before despawning, so undo restores lights, cameras, imported models, 2D nodes and groups faithfully — never hand-roll a `despawn` for scene entities. (Restoring one component's value — e.g. to undo a component removal — uses `renzora::core::reflection::capture_component` / `insert_component_reflected`.)

**Several changes that are one user action** (multi-reparent, paste) → wrap them in `CompoundCmd` so they undo as one step.

For anything bespoke, implement `UndoCommand` directly (see the built-ins in `renzora_undo/src/lib.rs`).

## The change-observer pattern (for single-buffer editors)

When an editor edits **one cheaply-`Clone` "document"** through many scattered code paths (a material/blueprint graph, a `.particle` buffer, a `.anim` clip), don't wrap every edit site. Instead run one observer system that snapshots the document whenever it changes:

1. Keep a shadow copy (in a `Resource`) of the document plus a cheap identity of *which* document it is (path / entity / selection key).
2. Each frame, compare the current document to the shadow. Use `PartialEq` if the type derives it, or a serialized string (e.g. `ron::to_string`) when it doesn't — full serialization catches every field.
3. If the **identity** changed (a different document loaded), reseed the shadow and return — loading is not an edit.
4. If the document changed, `record` a `SnapshotCmd` with `merge_key: Some(...)` so per-frame scrub spam collapses into one step, then update the shadow.
5. In the `SnapshotCmd`'s `restore`, apply the blob **and update the shadow to match** — otherwise undo/redo looks like a fresh edit and feeds back into the stack.

This is how the material, blueprint, particle and animation editors are wired (see `renzora_material_editor/src/native_graph.rs`'s `material_undo_observer`). It covers every present and future edit path from one place.

Tilemaps are the exception: their "document" is a set of child sprite entities, so they snapshot the layer's tiles at paint-stroke boundaries instead (`renzora_tilemap_editor`'s `tilemap_stroke_begin`/`_end`), mirroring the terrain stroke pattern.

## Seal at gesture boundaries

Merging means two *separate* drags on the same field would otherwise fold together. `renzora_undo` seals the active stack automatically on mouse-release and on `Enter`/`Esc`, so most gestures are split for free. If your editor has its own commit point, call it explicitly:

```rust
renzora_undo::seal(world, &ctx); // the next record starts a fresh step
```

## Checklist for a new panel

1. Add `renzora_undo` to the crate's `Cargo.toml`.
2. Replace direct world mutations in your edit handlers with `record`/`execute` + the right command.
3. Push into the correct context (`active_context` for scene-attached edits; an explicit `MaterialGraph`/`Blueprint`/`Other(path)` for a document editor).
4. `seal` at your commit boundary if the built-in mouse/keyboard seal doesn't cover it.
5. Confirm entries appear in the **History** panel and `Ctrl+Z`/`Ctrl+Y` walk them.
