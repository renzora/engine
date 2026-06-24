//! Editor-only half of the Solari plugin: the "Add Component" inspector entry
//! for [`SolariGi`].
//!
//! Compiled only with the `editor` feature. `SolariPlugin` loads at Runtime
//! scope, so in a shipped game (no editor framework) this registration is a
//! harmless no-op; in the editor it lets the user attach Solari GI to the World
//! Environment and toggle it. The entry is registered even when GPU ray tracing
//! is unavailable so the component stays discoverable and serializable — the
//! plugin's systems simply don't run in that case.

use bevy::prelude::*;
use bevy::solari::realtime::SolariLighting;
use renzora::{AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry, SolariGi};

pub(crate) fn register_inspectors(app: &mut App) {
    app.register_inspector(solari_inspector_entry());
}

fn solari_inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "solari_gi",
        display_name: "Solari Ray-Traced GI",
        icon: "lightning",
        category: "lighting",
        has_fn: |world, entity| world.get::<SolariGi>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(SolariGi::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<SolariGi>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<SolariGi>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<SolariGi>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![
            // Solari's realtime component (`SolariLighting`) exposes exactly one
            // runtime knob: `reset`, which clears the temporal history. Surface
            // it as a button — handy after a camera cut or when ghosting/dark
            // trails appear while the estimate re-converges. It pokes every
            // active Solari camera; `clear_solari_reset` flips the flag back off
            // after the frame is extracted (see lib.rs).
            FieldDef {
                name: "Reset Temporal History",
                field_type: FieldType::Button { icon: "arrow-counter-clockwise" },
                get_fn: |_w, _e| None,
                set_fn: |world, _e, v| {
                    if let FieldValue::Bool(true) = v {
                        let mut q = world.query::<&mut SolariLighting>();
                        for mut s in q.iter_mut(world) {
                            s.reset = true;
                        }
                    }
                },
            },
        ],
    }
}
