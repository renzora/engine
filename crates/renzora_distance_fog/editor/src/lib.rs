//! Editor-only half of `renzora_distance_fog` — the Distance Fog inspector
//! entry plus the native (bevy_ui) drawer.
//!
//! `renzora_distance_fog` compiles lean (no `editor` feature, no egui-phosphor,
//! no renzora_ember). This crate holds the inspector (renzora editor contract +
//! Phosphor icon) and the native drawer (renzora_ember), registered
//! `renzora::add!(DistanceFogEditorPlugin, Editor)` and linked only by the
//! editor bundle.

use bevy::pbr::DistanceFog;
use bevy::prelude::*;
use renzora::{AppEditorExt, InspectorEntry};
use renzora_distance_fog::DistanceFogSettings;

fn inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "distance_fog",
        display_name: "Distance Fog",
        icon: "cloud-fog",
        category: "rendering",
        has_fn: |world, entity| world.get::<DistanceFogSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .insert(DistanceFogSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<(DistanceFogSettings, DistanceFog)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<DistanceFogSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<DistanceFogSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![],
    }
}

/// Native (bevy_ui) inspector drawer — the bevy_ui analog of `fog_custom_ui`.
/// Demonstrates custom UI that declarative fields can't compose: a "Light Color"
/// swatch built from three separate float channels, plus bound numeric rows.
fn fog_native_ui(world: &mut World, entity: Entity) -> Entity {
    use renzora_ember::inspector::{color_field, inspector_body, inspector_row, inspector_stripe};
    use renzora_ember::reactive::bind_2way;
    use renzora_ember::widgets::{drag_value, DragRange};

    // Read initial values up front (inspector_body borrows World).
    let Some(s) = world.get::<DistanceFogSettings>(entity) else {
        return world.spawn(Node::default()).id();
    };
    let (start, end, density, exponent) =
        (s.start, s.end, s.density, s.directional_light_exponent);

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

        // "Light Color" → a proper HSV picker, two-way bound to the three
        // directional-light channels.
        let color = color_field(
            commands,
            move |w| {
                w.get::<DistanceFogSettings>(entity)
                    .map(|s| {
                        [
                            s.directional_light_color_r,
                            s.directional_light_color_g,
                            s.directional_light_color_b,
                        ]
                    })
                    .unwrap_or([0.0; 3])
            },
            move |w, rgb: [f32; 3]| {
                if let Some(mut s) = w.get_mut::<DistanceFogSettings>(entity) {
                    s.directional_light_color_r = rgb[0];
                    s.directional_light_color_g = rgb[1];
                    s.directional_light_color_b = rgb[2];
                }
            },
        );
        kids.push(inspector_row(commands, &fonts.ui, "Light Color", color));

        // Bound numeric rows (label + scrubbable value, two-way bound).
        macro_rules! frow {
            ($label:expr, $field:ident, $init:expr, $speed:expr, $min:expr, $max:expr) => {{
                let dv = drag_value(commands, &fonts.ui, "", (210, 210, 220), $init, $speed);
                commands.entity(dv).insert(DragRange { min: $min, max: $max });
                bind_2way(
                    commands,
                    dv,
                    move |w| w.get::<DistanceFogSettings>(entity).map(|s| s.$field).unwrap_or(0.0),
                    move |w, v: &f32| {
                        if let Some(mut s) = w.get_mut::<DistanceFogSettings>(entity) {
                            s.$field = *v;
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
        // The bevy_ui inspector uses this native drawer (conditional/composed
        // UI that declarative fields can't express).
        app.register_native_inspector_ui("distance_fog", fog_native_ui);
    }
}

renzora::add!(DistanceFogEditorPlugin, Editor);
