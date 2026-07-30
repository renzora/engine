//! Every ember widget the panel path can reach, in one panel.
//!
//! Deliberately does nothing: no systems, no assets, no spawning. If something
//! here looks wrong the fault is in the BSN parser, the widget's component
//! front-end, or the widget itself — there is nothing else it could be.
//!
//! Note what is *not* here: any vocabulary this plugin had to learn. The panel
//! is the same BSN a scene is, and `EmberDropdown` / `EmberTable` /
//! `EmberTimeline` are ordinary components the engine registered — so this
//! plugin names them the same way it would name `Transform`.

use renzora_plugin::prelude::*;

/// Marks the buttons whose clicks come back to [`on_action`].
///
/// A plugin's own component, carrying a number rather than a name: the ABI's
/// field kinds are a closed set, and an `i32` is in it while a `String` is not.
#[derive(Component, Default)]
#[repr(C)]
pub struct Fires {
    pub panel: i32,
    pub action: i32,
}

fn on_action(action: Action) {
    // Nothing to do — the point is that a click arrives at all, with the right
    // id, without taking the editor down.
    info(&format!("widgets: action {} fired", action.name()));
}

pub struct WidgetsPlugin;

impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.register_component::<Fires>().add_panel(
            Panel::new(
                "widgets",
                "Widgets",
                bsn! {
                    Node {
                        flex_direction: Column,
                        row_gap: Px(6.0),
                        padding: { left: Px(4.0), right: Px(4.0), top: Px(4.0), bottom: Px(4.0) },
                    }
                    Children [
                        Text("Buttons"),
                        ( Node { flex_direction: Row, column_gap: Px(6.0) }
                          Children [
                            ( EmberButtonWidget { label: "Primary" }
                              PanelActionId { action: 1 } ),
                            ( EmberButtonWidget { label: "Secondary" }
                              PanelActionId { action: 2 } ),
                          ] ),

                        Text("Input"),
                        // No range named — reads as 0..1, which is what it did
                        // before ranges existed.
                        ( EmberSliderWidget { value: 0.35 } ),
                        // A range in the field's own units. The thumb should sit
                        // at three quarters, not off the right-hand end.
                        ( EmberSliderWidget { value: 30.0, min: 0.0, max: 40.0 } ),
                        ( EmberToggle { on: true } ),
                        ( EmberCheckbox { checked: false } ),
                        ( EmberInput { placeholder: "type here", value: "" } ),

                        Text("Selection"),
                        ( EmberDropdown { options: ["Low", "Medium", "High", "Ultra"], selected: 1 } ),
                        ( EmberTabs { labels: ["Scene", "Render", "Audio"] } ),

                        Text("Readout"),
                        ( EmberProgress { value: 0.6 } ),

                        Text("Data"),
                        ( EmberTable {
                            headers: ["Name", "Type", "Size"],
                            rows: [
                                ["Cube", "Mesh", "1.2 KB"],
                                ["Sun", "Light", "0.1 KB"],
                                ["Terrain", "Mesh", "840 KB"],
                            ],
                          } ),

                        Text("iuggbyui"),
                        ( EmberTimeline {
                            duration: 6.0,
                            tracks: [
                                ( name: "Camera", color: (0, 0, 0),
                                  clips: [ ( start: 0.0, length: 2.5, label: "pan" ) ] ),
                                ( name: "Light", color: (0, 0, 0),
                                  clips: [ ( start: 1.0, length: 3.0, label: "fade" ) ] ),
                                ( name: "Character", color: (0, 0, 0),
                                  clips: [ ( start: 2.0, length: 3.5, label: "walk" ) ] ),
                            ],
                          } ),
                    ]
                },
            )
            .icon("squares-four")
            .on_action(on_action),
        );
    }
}

// Editor scope: this is a UI gallery and has no business in a shipped game.
// Without the declaration it would default to Runtime and load into `runtime.exe`
// alongside the game.
renzora_plugin::add!(WidgetsPlugin, Editor);
