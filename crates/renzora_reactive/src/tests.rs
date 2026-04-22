//! End-to-end sanity tests for the reactive layer.
//!
//! The `cross_panel_name_sync` test is the load-bearing one: it
//! demonstrates that an edit applied through one widget's sink
//! propagates to *every* other widget bound to the same source, with
//! no cross-panel wiring. If this test passes, the pattern works for
//! 44 panels just as well as for 2.

use bevy::prelude::*;

use crate::binding::{BindingChanged, Bound, CommitBinding, WidgetKind};
use crate::plugin::ReactivePlugin;
use crate::source::{BindSink, BindSource, SelectionProvider};
use crate::value::BoundValue;

/// Build a minimal test app with just the reactive plugin and the
/// bits of Bevy state our sources/sinks need. No rendering, no winit —
/// we drive the schedule manually with `app.update()`.
fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(ReactivePlugin);
    app
}

/// Drain all pending `BindingChanged` messages into a plain Vec so we
/// can assert on their contents.
fn drain_changes(app: &mut App) -> Vec<BindingChanged> {
    let mut messages = app
        .world_mut()
        .resource_mut::<Messages<BindingChanged>>();
    messages.drain().collect()
}

#[test]
fn initial_sync_fires_binding_changed() {
    // A freshly-spawned widget should emit exactly one BindingChanged on
    // its first sync tick, carrying the current source value. This is
    // how widgets paint their initial state without a separate seed path.
    let mut app = test_app();

    let entity = app.world_mut().spawn(Name::new("Camera")).id();
    let widget = app
        .world_mut()
        .spawn(Bound::read_only(BindSource::entity_name(entity), WidgetKind::Label))
        .id();

    app.update();

    let changes = drain_changes(&mut app);
    assert_eq!(changes.len(), 1, "expected one initial BindingChanged");
    let c = &changes[0];
    assert_eq!(c.widget, widget);
    assert_eq!(c.kind, WidgetKind::Label);
    assert_eq!(c.value, BoundValue::String("Camera".into()));
}

#[test]
fn no_event_when_value_unchanged() {
    // After the initial sync, subsequent ticks with no ECS mutation
    // must NOT fire BindingChanged — spurious events would wake every
    // widget observer for no reason.
    let mut app = test_app();

    let entity = app.world_mut().spawn(Name::new("Camera")).id();
    app.world_mut().spawn(Bound::read_only(
        BindSource::entity_name(entity),
        WidgetKind::Label,
    ));

    app.update();
    let _ = drain_changes(&mut app);

    app.update();
    app.update();
    let changes = drain_changes(&mut app);
    assert!(
        changes.is_empty(),
        "expected no events on no-op ticks, got {changes:?}"
    );
}

#[test]
fn cross_panel_name_sync() {
    // The load-bearing test. Two widgets on two (conceptual) panels
    // both bind to the same entity's Name. A CommitBinding on widget A
    // mutates the Name; the next sync tick must fire BindingChanged on
    // widget B with the new value — *without* widget A or B knowing
    // anything about each other.
    let mut app = test_app();

    let entity = app.world_mut().spawn(Name::new("Camera")).id();

    let widget_a = app
        .world_mut()
        .spawn(Bound::read_write(
            BindSource::entity_name(entity),
            BindSink::entity_name(entity),
            WidgetKind::TextInput,
        ))
        .id();

    let widget_b = app
        .world_mut()
        .spawn(Bound::read_only(
            BindSource::entity_name(entity),
            WidgetKind::Label,
        ))
        .id();

    // First tick: both widgets seed their initial state.
    app.update();
    let initial = drain_changes(&mut app);
    assert_eq!(initial.len(), 2);

    // User "edits" widget A by emitting a CommitBinding with a new value.
    app.world_mut()
        .resource_mut::<Messages<CommitBinding>>()
        .write(CommitBinding {
            widget: widget_a,
            value: BoundValue::String("MainCamera".into()),
        });

    // Two updates: first runs apply_commits (Last schedule), next runs
    // sync_bindings (Update) and picks up the ECS change.
    app.update();
    app.update();

    let changes = drain_changes(&mut app);

    // Both widgets must have received the new value. Order is not
    // guaranteed, so check membership rather than index.
    let for_a = changes.iter().find(|c| c.widget == widget_a);
    let for_b = changes.iter().find(|c| c.widget == widget_b);

    assert!(for_a.is_some(), "widget A (editor) did not receive change");
    assert!(for_b.is_some(), "widget B (observer) did not receive change");
    assert_eq!(
        for_a.unwrap().value,
        BoundValue::String("MainCamera".into())
    );
    assert_eq!(
        for_b.unwrap().value,
        BoundValue::String("MainCamera".into())
    );

    // And the underlying ECS actually holds the new name.
    let name = app.world().get::<Name>(entity).unwrap();
    assert_eq!(name.as_str(), "MainCamera");
}

#[test]
fn selection_tracked_binding_follows_selection() {
    // A SelectedField binding reads whatever is the current selection.
    // Change the selection → next sync emits a change with the new
    // entity's value. No rebinding required.
    let mut app = test_app();

    // Tiny selection model: a resource holding an Option<Entity>.
    #[derive(Resource, Default)]
    struct TestSelection(Option<Entity>);

    app.world_mut().init_resource::<TestSelection>();
    app.world_mut().insert_resource(SelectionProvider {
        get: |world| world.resource::<TestSelection>().0,
    });

    let a = app.world_mut().spawn(Name::new("Alpha")).id();
    let b = app.world_mut().spawn(Name::new("Beta")).id();

    let widget = app
        .world_mut()
        .spawn(Bound::read_only(
            BindSource::selected_name(),
            WidgetKind::Label,
        ))
        .id();

    // Select Alpha.
    app.world_mut().resource_mut::<TestSelection>().0 = Some(a);
    app.update();
    let changes = drain_changes(&mut app);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].widget, widget);
    assert_eq!(changes[0].value, BoundValue::String("Alpha".into()));

    // Switch to Beta.
    app.world_mut().resource_mut::<TestSelection>().0 = Some(b);
    app.update();
    let changes = drain_changes(&mut app);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].value, BoundValue::String("Beta".into()));

    // Clear selection → Unit (widgets treat this as "nothing to show").
    app.world_mut().resource_mut::<TestSelection>().0 = None;
    app.update();
    let changes = drain_changes(&mut app);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].value, BoundValue::Unit);
}

#[test]
fn commit_to_read_only_widget_is_ignored() {
    // A widget with no sink that emits a commit must not panic and
    // must not mutate anything. We also want this logged so it's
    // diagnosable, but warn! doesn't assert well in tests — we just
    // check the no-crash, no-mutation invariant.
    let mut app = test_app();

    let entity = app.world_mut().spawn(Name::new("Keep")).id();
    let widget = app
        .world_mut()
        .spawn(Bound::read_only(
            BindSource::entity_name(entity),
            WidgetKind::Label,
        ))
        .id();

    app.update();
    let _ = drain_changes(&mut app);

    app.world_mut()
        .resource_mut::<Messages<CommitBinding>>()
        .write(CommitBinding {
            widget,
            value: BoundValue::String("ShouldNotApply".into()),
        });

    app.update();
    app.update();

    let name = app.world().get::<Name>(entity).unwrap();
    assert_eq!(name.as_str(), "Keep", "name must be unchanged");
}
