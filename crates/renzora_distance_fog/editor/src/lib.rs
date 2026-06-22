//! Editor-only half of `renzora_distance_fog` — the **Fog** section of the
//! `WorldEnvironment` inspector.
//!
//! Fog is no longer a separately-addable component. It's a section of the one
//! `WorldEnvironment` (see `docs/world-environment-spec.md`): the entry shows
//! whenever the selected entity has a `WorldEnvironment`, its enable toggle
//! drives `WorldEnvironment::fog.enabled`, and the native drawer edits the fog
//! sub-section. No add/remove — it's intrinsic to the environment.

use bevy::prelude::*;
use renzora::{AppEditorExt, InspectorEntry, WorldEnvironment};

fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "world_env_fog",
        display_name: "Fog",
        icon: "cloud-fog",
        category: "rendering",
        has_fn: |world, entity| world.get::<WorldEnvironment>(entity).is_some(),
        // Intrinsic to the WorldEnvironment — not added or removed on its own.
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<WorldEnvironment>(entity)
                .map(|e| e.fog.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut e) = world.get_mut::<WorldEnvironment>(entity) {
                e.fog.enabled = val;
            }
        }),
        fields: vec![],
    }
}

/// Native (bevy_ui) drawer for the fog section: a light-color picker plus bound
/// numeric rows, all editing `WorldEnvironment::fog`.
fn fog_native_ui(world: &mut World, entity: Entity) -> Entity {
    use renzora_ember::inspector::{color_field, inspector_body, inspector_row, inspector_stripe};
    use renzora_ember::reactive::bind_2way;
    use renzora_ember::widgets::{drag_value, DragRange};

    // Read initial values up front (inspector_body borrows World).
    let Some(e) = world.get::<WorldEnvironment>(entity) else {
        return world.spawn(Node::default()).id();
    };
    let f = &e.fog;
    let (start, end, density, exponent) = (f.start, f.end, f.density, f.directional_light_exponent);

    inspector_body(world, move |commands, fonts| {
        let col = commands
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(2.0)),
                ..default()
            })
            .id();
        let mut kids: Vec<Entity> = Vec::new();

        // "Light Color" → HSV picker, two-way bound to the directional-light tint.
        let color = color_field(
            commands,
            move |w| {
                w.get::<WorldEnvironment>(entity)
                    .map(|e| e.fog.directional_light_color)
                    .unwrap_or([0.0; 3])
            },
            move |w, rgb: [f32; 3]| {
                if let Some(mut e) = w.get_mut::<WorldEnvironment>(entity) {
                    e.fog.directional_light_color = rgb;
                }
            },
        );
        kids.push(inspector_row(commands, &fonts.ui, "Light Color", color));

        // Bound numeric rows (label + scrubbable value).
        macro_rules! frow {
            ($label:expr, $field:ident, $init:expr, $speed:expr, $min:expr, $max:expr) => {{
                let dv = drag_value(commands, &fonts.ui, "", (210, 210, 220), $init, $speed);
                commands.entity(dv).insert(DragRange { min: $min, max: $max });
                bind_2way(
                    commands,
                    dv,
                    move |w| {
                        w.get::<WorldEnvironment>(entity)
                            .map(|e| e.fog.$field)
                            .unwrap_or(0.0)
                    },
                    move |w, v: &f32| {
                        if let Some(mut e) = w.get_mut::<WorldEnvironment>(entity) {
                            e.fog.$field = *v;
                        }
                    },
                );
                inspector_row(commands, &fonts.ui, $label, dv)
            }};
        }
        kids.push(frow!("Light Exponent", directional_light_exponent, exponent, 0.1, 1.0, 64.0));
        kids.push(frow!("Start", start, start, 0.5, 0.0, 10000.0));
        kids.push(frow!("End", end, end, 0.5, 0.0, 10000.0));
        kids.push(frow!("Density", density, density, 0.001, 0.0, 1.0));

        for (i, &row) in kids.iter().enumerate() {
            commands
                .entity(row)
                .insert(BackgroundColor(inspector_stripe(i)));
        }
        commands.entity(col).add_children(&kids);
        col
    })
}

/// Editor-scope companion to `renzora_distance_fog::DistanceFogPlugin`.
#[derive(Default)]
pub struct DistanceFogEditorPlugin;

impl Plugin for DistanceFogEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] DistanceFogEditorPlugin");
        app.register_inspector(inspector_entry());
        app.register_native_inspector_ui("world_env_fog", fog_native_ui);
    }
}

renzora::add!(DistanceFogEditorPlugin, Editor);
