//! The component front-ends actually build their widget.
//!
//! A hook that never fires is invisible: the component is present, the entity
//! exists, and the panel simply looks empty. This asserts a child appears.

use bevy::prelude::*;
use renzora_ember::widgets::{EmberButtonWidget, EmberDropdown, EmberTable};

/// Enough of an app for the hooks to have fonts and a command queue.
///
/// `EmberFonts` is normally built once the Phosphor font has loaded, which needs
/// a real asset pipeline. Default handles are enough here: nothing under test
/// rasterises anything, and the widgets only need the resource to exist.
fn test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .init_asset::<bevy::text::Font>();

    let handle: Handle<bevy::text::Font> = Handle::default();
    app.insert_resource(renzora_ember::font::EmberFonts {
        ui: handle.clone().into(),
        phosphor: handle.clone(),
        mono: handle.clone().into(),
        default_ui: handle.clone().into(),
        default_mono: handle.into(),
    });
    app
}

#[test]
fn a_button_component_builds_a_button() {
    let mut app = test_app();
    let e = app.world_mut().spawn(EmberButtonWidget { label: "Go".into() }).id();
    // One update so the hook's queued command applies.
    app.update();

    let children = app.world().entity(e).get::<Children>();
    assert!(
        children.is_some_and(|c| !c.is_empty()),
        "the insert hook produced no children — the widget was never built"
    );
}

#[test]
fn a_dropdown_component_builds_a_dropdown() {
    let mut app = test_app();
    let e = app
        .world_mut()
        .spawn(EmberDropdown {
            options: vec!["Low".into(), "High".into()],
            selected: 0,
        })
        .id();
    app.update();

    assert!(
        app.world().entity(e).get::<Children>().is_some_and(|c| !c.is_empty()),
        "dropdown built nothing"
    );
}

#[test]
fn a_table_component_builds_a_table() {
    let mut app = test_app();
    let e = app
        .world_mut()
        .spawn(EmberTable {
            headers: vec!["Name".into()],
            rows: vec![vec!["Cube".into()]],
        })
        .id();
    app.update();

    assert!(
        app.world().entity(e).get::<Children>().is_some_and(|c| !c.is_empty()),
        "table built nothing"
    );
}

/// A UI entity without a `Node` is skipped by layout, and its children go with
/// it — the widget builds correctly and renders nothing, which is invisible
/// unless something asserts it.
#[test]
fn a_widget_component_brings_its_own_node() {
    let mut app = test_app();
    let e = app.world_mut().spawn(EmberButtonWidget { label: "Go".into() }).id();
    app.update();

    assert!(
        app.world().entity(e).get::<Node>().is_some(),
        "no `Node` — the widget would build children that never lay out"
    );
}
