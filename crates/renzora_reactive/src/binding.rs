use bevy::prelude::*;

use crate::source::{BindSink, BindSource};
use crate::value::BoundValue;

/// An identifier declaring which kind of widget the binding is driving.
/// The sync system emits `BindingChanged` events carrying this tag so
/// widget-specific observer systems only do work for their own widget
/// kind — no string matching, no reflection, just a small enum lookup.
///
/// Extend freely as new widget types are added to `renzora_ui`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WidgetKind {
    Label,
    TextInput,
    NumberInput,
    VectorInput,
    ColorInput,
    Toggle,
    EnumPicker,
    AssetInput,
    /// Caller-defined widget — embed your own tag id.
    Custom(u32),
}

/// Attach to a UI widget entity to bind it to a source.
///
/// Invariants:
/// - `last` is only written by `sync_bindings`. Widgets must not touch it.
/// - `source` is effectively immutable after insertion. To re-target a
///   widget, replace the whole `Bound` component; the sync loop re-seeds
///   `last` on the next tick and fires an initial `BindingChanged`.
/// - `sink` is required for editable widgets. Read-only widgets leave it
///   `None`; `apply_commits` warns (once) if they try to emit a commit.
#[derive(Component, Clone)]
pub struct Bound {
    pub source: BindSource,
    pub sink: Option<BindSink>,
    pub widget: WidgetKind,
    /// Last value the sync loop observed. `None` until the first tick,
    /// which always fires a `BindingChanged` so widgets can paint their
    /// initial state without a dedicated seeding path.
    pub last: Option<BoundValue>,
}

impl Bound {
    pub fn read_only(source: BindSource, widget: WidgetKind) -> Self {
        Self { source, sink: None, widget, last: None }
    }

    pub fn read_write(source: BindSource, sink: BindSink, widget: WidgetKind) -> Self {
        Self { source, sink: Some(sink), widget, last: None }
    }
}

/// Emitted by `sync_bindings` when a binding's source value has actually
/// changed (compared by structural equality, not merely `Changed<T>`).
#[derive(Message, Clone, Debug)]
pub struct BindingChanged {
    pub widget: Entity,
    pub kind: WidgetKind,
    pub value: BoundValue,
}

/// Emitted by a widget when the user edited its value. `apply_commits`
/// routes it through the binding's sink; the resulting ECS mutation
/// triggers `Changed<T>`, which the next `sync_bindings` tick fans out
/// to every other widget bound to the same source.
#[derive(Message, Clone, Debug)]
pub struct CommitBinding {
    pub widget: Entity,
    pub value: BoundValue,
}

/// Exclusive system. Each frame:
///  1. Read every `Bound`'s source value.
///  2. Compare to the cached `last`.
///  3. On real change, update `last` and emit `BindingChanged`.
///
/// Exclusive because the source readers take `&World` and we also need
/// `&mut` on `Bound` — holding both simultaneously is a borrow conflict
/// in a parallel system. Cost is one world read per binding; at editor
/// scale (hundreds, not millions) this is cheap.
pub fn sync_bindings(world: &mut World) {
    // Snapshot all binding entities first — we can't hold a Query iter
    // open while mutating via get_mut. Small vec allocation per frame;
    // measure and pool if it ever shows up on a flamegraph.
    let entities: Vec<Entity> = {
        let mut q = world.query_filtered::<Entity, With<Bound>>();
        q.iter(world).collect()
    };

    let mut changes: Vec<BindingChanged> = Vec::new();

    for entity in entities {
        // Clone the small fields we need so we can release the borrow
        // before calling into the world-reading getter.
        let (source, kind, last) = match world.get::<Bound>(entity) {
            Some(b) => (b.source.clone(), b.widget, b.last.clone()),
            None => continue,
        };

        let next = source.read(world);

        let changed = match last {
            None => true,
            Some(prev) => prev != next,
        };

        if changed {
            if let Some(mut bound) = world.get_mut::<Bound>(entity) {
                bound.last = Some(next.clone());
            }
            changes.push(BindingChanged { widget: entity, kind, value: next });
        }
    }

    if !changes.is_empty() {
        let mut events = world.resource_mut::<Messages<BindingChanged>>();
        for change in changes {
            events.write(change);
        }
    }
}

/// Exclusive system. Drains pending `CommitBinding` events, looks up
/// each widget's `Bound.sink`, and applies the new value to the ECS.
///
/// Running this after widget systems means all edits in a frame land
/// before the next frame's `sync_bindings` — so an edit in the inspector
/// is visible in the hierarchy one frame later, consistently.
pub fn apply_commits(world: &mut World) {
    let commits: Vec<CommitBinding> = {
        let mut events = world
            .get_resource_mut::<Messages<CommitBinding>>()
            .expect("CommitBinding Messages resource missing — did you add ReactivePlugin?");
        events.drain().collect()
    };

    for commit in commits {
        let sink = world
            .get::<Bound>(commit.widget)
            .and_then(|b| b.sink.clone());
        match sink {
            Some(sink) => sink.apply(world, commit.value),
            None => {
                warn!(
                    "CommitBinding emitted by read-only widget {:?}; ignoring",
                    commit.widget
                );
            }
        }
    }
}
