//! Editor-only half of `renzora_ssao` — the **SSAO** section of the
//! `WorldEnvironment` inspector.
//!
//! `renzora_ssao` compiles lean (no `editor` feature, no egui-phosphor).
//! This crate holds the inspector (renzora editor contract + Phosphor icon),
//! registered `renzora::add!(SsaoEditorPlugin, Editor)` and linked only by the
//! editor bundle.
//!
//! Like Fog, SSAO is not a separately-addable component — it's a section of the
//! one `WorldEnvironment` (see `docs/world-environment-spec.md`): the entry
//! shows whenever the selected entity has a `WorldEnvironment`, its enable
//! toggle drives `WorldEnvironment::ssao.enabled`, and the native drawer edits
//! the sub-section.

use bevy::prelude::*;
use renzora::{AppEditorExt, InspectorEntry, SsaoQuality, WorldEnvironment};

fn ssao_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "world_env_ssao",
        display_name: "SSAO",
        icon: "circle-half",
        category: "rendering",
        has_fn: |world, entity| world.get::<WorldEnvironment>(entity).is_some(),
        // Intrinsic to the WorldEnvironment — not added or removed on its own.
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<WorldEnvironment>(entity)
                .map(|e| e.ssao.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut e) = world.get_mut::<WorldEnvironment>(entity) {
                e.ssao.enabled = val;
            }
        }),
        fields: vec![],
    }
}

/// Native (bevy_ui) drawer for the SSAO section: a quality preset dropdown, the
/// object-thickness scrubber, and the two raw sample counts — which are shown
/// only under `Custom`, because every preset overrides them and leaving them
/// visible reads as four live knobs when only two are.
fn ssao_native_ui(world: &mut World, entity: Entity) -> Entity {
    use renzora_ember::inspector::{inspector_body, inspector_row, inspector_stripe};
    use renzora_ember::reactive::tracked::{bind_2way, bind_display};
    use renzora_ember::widgets::{drag_value, dropdown, DragRange, DragSnap};

    // Read initial values up front (inspector_body borrows World).
    let Some(e) = world.get::<WorldEnvironment>(entity) else {
        return world.spawn(Node::default()).id();
    };
    let s = &e.ssao;
    let (quality, thickness, slices, samples) = (
        s.quality.index(),
        s.constant_object_thickness,
        s.slice_count as f32,
        s.samples_per_slice_side as f32,
    );

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
        let mut rows: Vec<Entity> = Vec::new();

        // Quality preset.
        let dd = dropdown(commands, fonts, &SsaoQuality::LABELS, quality);
        bind_2way(
            commands,
            dd,
            move |w| {
                w.get::<WorldEnvironment>(entity)
                    .map(|e| e.ssao.quality.index())
                    .unwrap_or_else(|| SsaoQuality::default().index())
            },
            move |w, v: &usize| {
                if let Some(mut e) = w.get_mut::<WorldEnvironment>(entity) {
                    e.ssao.quality = SsaoQuality::from_index(*v);
                }
            },
        );
        rows.push(inspector_row(commands, &fonts.ui, "Quality", dd));

        // Object thickness — a float, so it scrubs continuously.
        let dv = drag_value(commands, &fonts.ui, "", (210, 210, 220), thickness, 0.01);
        commands.entity(dv).insert(DragRange { min: 0.0, max: 10.0 });
        bind_2way(
            commands,
            dv,
            move |w| {
                w.get::<WorldEnvironment>(entity)
                    .map(|e| e.ssao.constant_object_thickness)
                    .unwrap_or(0.0)
            },
            move |w, v: &f32| {
                if let Some(mut e) = w.get_mut::<WorldEnvironment>(entity) {
                    e.ssao.constant_object_thickness = *v;
                }
            },
        );
        rows.push(inspector_row(commands, &fonts.ui, "Object Thickness", dv));

        // The two `Custom` counts. `DragSnap(1.0)` quantizes the widget's own
        // model to whole numbers as well as the value written back — without it
        // `bind_2way` would compare a fractional model (3.4) against the `u32`
        // read-back (3) and treat its own scrub as an external change.
        macro_rules! count_row {
            ($label:expr, $field:ident, $init:expr, $min:expr, $max:expr) => {{
                let dv = drag_value(commands, &fonts.ui, "", (210, 210, 220), $init, 1.0);
                commands.entity(dv).insert((
                    DragRange {
                        min: $min,
                        max: $max,
                    },
                    DragSnap(1.0),
                ));
                bind_2way(
                    commands,
                    dv,
                    move |w| {
                        w.get::<WorldEnvironment>(entity)
                            .map(|e| e.ssao.$field as f32)
                            .unwrap_or(0.0)
                    },
                    move |w, v: &f32| {
                        if let Some(mut e) = w.get_mut::<WorldEnvironment>(entity) {
                            e.ssao.$field = v.round().max(0.0) as u32;
                        }
                    },
                );
                inspector_row(commands, &fonts.ui, $label, dv)
            }};
        }
        let slice_row = count_row!("Slice Count", slice_count, slices, 1.0, 16.0);
        let sample_row = count_row!("Samples / Slice", samples_per_slice_side, samples, 1.0, 8.0);
        rows.push(slice_row);
        rows.push(sample_row);

        for (i, &row) in rows.iter().enumerate() {
            commands
                .entity(row)
                .insert(BackgroundColor(inspector_stripe(i)));
        }
        // Hidden under a preset. These are the LAST two rows on purpose — hiding
        // a row in the middle would leave two same-colored stripes adjacent.
        for row in [slice_row, sample_row] {
            bind_display(commands, row, move |w| {
                w.get::<WorldEnvironment>(entity)
                    .map(|e| e.ssao.quality == SsaoQuality::Custom)
                    .unwrap_or(false)
            });
        }

        commands.entity(col).add_children(&rows);
        col
    })
}

/// Editor-scope companion to `renzora_ssao::SsaoPlugin`.
#[derive(Default)]
pub struct SsaoEditorPlugin;

impl Plugin for SsaoEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] SsaoEditorPlugin");
        app.register_inspector(ssao_entry());
        app.register_native_inspector_ui("world_env_ssao", ssao_native_ui);
    }
}

renzora::add!(SsaoEditorPlugin, Editor);
