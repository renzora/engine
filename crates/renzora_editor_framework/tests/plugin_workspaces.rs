//! The drain that turns a plugin's `register_workspace` into a real entry in
//! the editor's layout switcher.
//!
//! Worth a test of its own rather than trusting a launch: the drain is gated on
//! `SplashState::Editor`, so a headless run that never opens a project never
//! reaches it, and "the Debug workspace is missing" is exactly the symptom a
//! silent failure here would produce. Driving the system directly is both faster
//! and stricter than looking for a log line.

use bevy::prelude::*;
use renzora_ember::dock::DockTree;
use renzora_ember::workspace::{PendingWorkspaces, RegisterWorkspace};
use renzora_ui::LayoutManager;

/// Build an app holding just the two resources the drain moves between, plus
/// the drain itself. `renzora_editor_framework` exposes the system through its
/// plugin, so the test re-adds it by the same path the plugin does.
fn harness() -> App {
    let mut app = App::new();
    app.init_resource::<PendingWorkspaces>();
    app.insert_resource(LayoutManager::default());
    app.add_systems(Update, renzora_editor_framework::install_plugin_workspaces);
    app
}

#[test]
fn a_registered_workspace_reaches_the_layout_manager() {
    let mut app = harness();
    let before = app.world().resource::<LayoutManager>().layouts.len();

    app.register_workspace("Debug", DockTree::leaf("performance"));
    app.update();

    let manager = app.world().resource::<LayoutManager>();
    assert_eq!(manager.layouts.len(), before + 1);
    assert!(manager.layouts.iter().any(|l| l.name == "Debug"));
    // Drained, not merely copied: leaving it queued would re-install it every
    // frame and mark the layout resource changed every frame with it.
    assert!(app.world().resource::<PendingWorkspaces>().0.is_empty());
}

/// A native plugin is rebuilt and re-initialised whenever its source moves, so
/// re-registering is the ordinary case rather than the exotic one. The layout
/// switcher must not grow a second "Debug" every time.
#[test]
fn re_registering_replaces_rather_than_appending() {
    let mut app = harness();
    app.register_workspace("Debug", DockTree::leaf("performance"));
    app.update();
    let after_first = app.world().resource::<LayoutManager>().layouts.len();

    app.register_workspace("Debug", DockTree::leaf("ecs_stats"));
    app.update();

    let manager = app.world().resource::<LayoutManager>();
    assert_eq!(manager.layouts.len(), after_first, "should replace, not append");
    let debug = manager.layouts.iter().find(|l| l.name == "Debug").unwrap();
    match &debug.tree {
        renzora_ui::dock_tree::DockTree::Leaf { tabs, .. } => {
            assert_eq!(tabs, &vec!["ecs_stats".to_string()], "the later tree should win")
        }
        other => panic!("expected a leaf, got {other:?}"),
    }
}

/// The two `DockTree` types are structurally identical but distinct, and the
/// conversion is hand-written, so the shape has to survive the crossing.
#[test]
fn the_tree_shape_survives_conversion() {
    let mut app = harness();
    app.register_workspace(
        "Debug",
        DockTree::horizontal(
            DockTree::leaf("hierarchy"),
            DockTree::vertical(DockTree::leaf("viewport"), DockTree::leaf("inspector"), 0.65),
            0.15,
        ),
    );
    app.update();

    let manager = app.world().resource::<LayoutManager>();
    let debug = manager.layouts.iter().find(|l| l.name == "Debug").unwrap();
    use renzora_ui::dock_tree::{DockTree as Out, SplitDirection as Dir};
    let Out::Split {
        direction,
        ratio,
        first,
        second,
    } = &debug.tree
    else {
        panic!("expected a split at the root");
    };
    assert!(matches!(direction, Dir::Horizontal));
    assert!((ratio - 0.15).abs() < f32::EPSILON);
    assert!(matches!(**first, Out::Leaf { .. }));
    assert!(matches!(**second, Out::Split { .. }));
}

/// A plugin picks its own numbers, unlike the drag handler, so both the ratio
/// and the active-tab index are pinned into range on the way through. An
/// out-of-range `active_tab` would be used to index `tabs` by the shell.
#[test]
fn out_of_range_values_are_pinned_rather_than_trusted() {
    let mut app = harness();
    app.register_workspace(
        "Silly",
        DockTree::Split {
            direction: renzora_ember::dock::SplitDirection::Vertical,
            ratio: 40.0,
            first: Box::new(DockTree::Leaf {
                tabs: vec!["a".into(), "b".into()],
                active_tab: 99,
            }),
            second: Box::new(DockTree::Empty),
        },
    );
    app.update();

    let manager = app.world().resource::<LayoutManager>();
    let silly = manager.layouts.iter().find(|l| l.name == "Silly").unwrap();
    use renzora_ui::dock_tree::DockTree as Out;
    let Out::Split { ratio, first, .. } = &silly.tree else {
        panic!("expected a split");
    };
    assert!((0.1..=0.9).contains(ratio), "ratio {ratio} should be clamped");
    match &**first {
        Out::Leaf { tabs, active_tab } => {
            assert!(*active_tab < tabs.len(), "active_tab {active_tab} is out of range")
        }
        other => panic!("expected a leaf, got {other:?}"),
    }
}
