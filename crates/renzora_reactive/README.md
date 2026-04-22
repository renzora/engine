# renzora_reactive

Single primitive for every cross-panel sync problem in the Renzora editor. Attach a `Bound` component to a UI widget, point it at a source in the ECS, and the reactive layer handles the rest.

## Why this exists

In immediate-mode UI, "sync" was free — every panel rebuilt itself from the same state each frame. In retained UI on `bevy_ui`, every widget caches its own display, and without a central binding layer every panel would grow its own ad-hoc sync code (queries, `Changed<T>` filters, manual notifications). That path leads to drift: edit a name in the inspector and watch the hierarchy show the old one because someone forgot a system.

This crate says: **ECS is the only source of truth, widgets never cache, and one sync loop fans every mutation out to every widget that cares**. Panels stop knowing about each other.

## Flow

1. A panel spawns a widget entity with a `Bound` component.
2. `sync_bindings` (every frame) reads every binding's source, compares to the cached last value, emits a `BindingChanged` event only when the value actually changed.
3. Widget-specific systems observe `BindingChanged { kind }` for their widget kind and update their visible components (text, colour, node width, …).
4. When the user edits a widget, the widget emits `CommitBinding { widget, value }`.
5. `apply_commits` (Last schedule, exclusive) runs the binding's sink. ECS mutation triggers `Changed<T>` upstream; next tick's `sync_bindings` fans the update out to *every other widget* bound to the same source — automatic cross-panel propagation.

## Patterns

### Read-only label bound to an entity's name

```rust
commands.spawn((
    // ... your widget's visual components (Node, Text, ...) ...
    Bound::read_only(
        BindSource::entity_name(entity),
        WidgetKind::Label,
    ),
));
```

### Editable text input bound to an entity's name

```rust
commands.spawn((
    // ... your TextInput widget's visual components ...
    Bound::read_write(
        BindSource::entity_name(entity),
        BindSink::entity_name(entity),
        WidgetKind::TextInput,
    ),
));
```

When the user commits an edit, the widget emits:

```rust
messages.write(CommitBinding {
    widget: my_entity,
    value: BoundValue::String(new_text),
});
```

The inspector and hierarchy both holding `Label`s on the same entity's name will refresh automatically next frame. No cross-panel code.

### Inspector field (follows current selection)

```rust
commands.spawn((
    Bound::read_write(
        BindSource::selected_translation(),
        BindSink::selected_translation(),
        WidgetKind::VectorInput,
    ),
));
```

When the user picks a different entity, the same widget retargets automatically — `SelectedField` resolves the current selection at read time. No re-spawning, no re-binding.

### Custom field (component the helpers don't cover)

Write a getter and setter. They're `fn` pointers, not closures, so the binding stays `Clone + Send + Sync`:

```rust
fn read_my_field(world: &World, e: Entity) -> BoundValue {
    world.get::<MyComponent>(e)
        .map(|c| BoundValue::F32(c.my_float))
        .unwrap_or(BoundValue::Unit)
}

fn write_my_field(world: &mut World, e: Entity, value: BoundValue) {
    if let BoundValue::F32(v) = value {
        if let Some(mut c) = world.get_mut::<MyComponent>(e) {
            c.my_float = v;
        }
    }
}

commands.spawn(Bound::read_write(
    BindSource::EntityField { entity: e, getter: read_my_field },
    BindSink::EntityField { entity: e, setter: write_my_field },
    WidgetKind::NumberInput,
));
```

### Derived / computed value (FPS, mem, selection count)

```rust
commands.spawn(Bound::read_only(
    BindSource::Computed {
        getter: |world| {
            let fps = world.resource::<DiagnosticsStore>()
                .get(&FrameTimeDiagnosticsPlugin::FPS)
                .and_then(|d| d.smoothed())
                .unwrap_or(0.0);
            BoundValue::F32(fps as f32)
        },
    },
    WidgetKind::Label,
));
```

## Rules for panel authors

- **Never cache values locally.** If you find yourself storing a `String` on a panel struct to avoid re-reading the ECS, stop — spawn a `Bound` instead.
- **Never write cross-panel notifications.** If you're tempted to emit a `HierarchyNeedsRefresh` event because the inspector edited a name, you're fighting the layer. The binding already handles it.
- **Observe `BindingChanged` by widget kind.** Your widget update systems filter on `kind == WidgetKind::TextInput` (or whatever) so you don't pay the cost of other widget kinds' updates.
- **Read-only widgets leave `sink: None`.** The commit system warns on commits to sink-less widgets; use that warning to catch widget implementation bugs.

## Performance notes

- `sync_bindings` is `O(bindings)` per frame. At editor scale (hundreds of widgets) this is inconsequential. If a panel spawns thousands of bindings (e.g. a list view without virtualization), the fix is virtualization, not the binding layer.
- The `last: Option<BoundValue>` field on `Bound` is the per-widget change-detection cache. It's small — same size as the value itself plus an enum tag.
- Structural equality on `BoundValue` handles filtering redundant updates. If you need cheaper equality for a hot path (e.g. a giant `String`), add a `BoundValue::Hashed` variant carrying a `u64` and compare hashes.

## What's not here yet

- **Reflection-driven field bindings.** The generic "bind to `Foo.bar.baz`" via `bevy_reflect` is not implemented. Inspector uses explicit `fn` pointers today. Worth adding once we start migrating components with dozens of fields.
- **Batched resource-field bindings.** Resource bindings work, but there's no shortcut for "every field on this resource" — you spell each one out. Add a macro if it becomes tedious.
- **Animation bridge.** Widgets currently snap to new values on `BindingChanged`. A "tween toward target" variant will land when we pick up the animation story.
