//! Editor-only half of the gaussian-splatting plugin.
//!
//! Compiled only with the `editor` feature. `GaussianSplatPlugin` loads at
//! Runtime scope, so in a shipped game these registrations are harmless no-ops
//! (the editor registries they target don't exist); in the editor they wire:
//!
//! - an **inspector entry** for [`GaussianSplat`] — asset-drop source field
//!   (`.ply` / `.gcloud`) plus the per-cloud opacity / splat-scale tuning;
//! - an **Add-Entity preset** ("Gaussian Splat") so a cloud can be added from
//!   the hierarchy overlay and pointed at a file afterwards, mirroring how the
//!   viewport drag-drop path (`renzora_viewport::gaussian_drop`) spawns one
//!   with the source pre-filled.

use bevy::prelude::*;

use renzora::{
    AppEditorExt, EntityPreset, FieldDef, FieldType, FieldValue, GaussianSplat, InspectorEntry,
};

pub(super) fn register(app: &mut App) {
    info!("[editor] GaussianSplat inspector + preset");
    app.register_inspector(splat_entry());
    app.register_entity_preset(EntityPreset {
        id: "gaussian_splat",
        display_name: "Gaussian Splat",
        icon: "cloud",
        category: "general",
        spawn_fn: |world| {
            world
                .spawn((
                    Name::new("Gaussian Splat"),
                    Transform::default(),
                    Visibility::default(),
                    GaussianSplat::default(),
                ))
                .id()
        },
    });
}

fn splat_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "gaussian_splat",
        display_name: "Gaussian Splat",
        icon: "cloud",
        category: "rendering",
        has_fn: |world, entity| world.get::<GaussianSplat>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(GaussianSplat::default());
        }),
        remove_fn: Some(|world, entity| {
            // The plugin's sync system strips the resolved renderer components
            // when it sees the orphaned bookkeeping, so removing just the
            // contract component here is the whole teardown.
            world.entity_mut(entity).remove::<GaussianSplat>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Source",
                field_type: FieldType::Asset {
                    extensions: vec!["ply".into(), "gcloud".into(), "sog".into(), "ssog".into()],
                },
                get_fn: |w, e| {
                    w.get::<GaussianSplat>(e).map(|s| {
                        FieldValue::Asset((!s.source.is_empty()).then(|| s.source.clone()))
                    })
                },
                set_fn: |w, e, v| {
                    if let (FieldValue::Asset(path), Some(mut s)) =
                        (v, w.get_mut::<GaussianSplat>(e))
                    {
                        s.source = path.unwrap_or_default();
                    }
                },
            },
            renzora::float_field!("Opacity", GaussianSplat, opacity, 0.01, 0.0, 1.0),
            renzora::float_field!("Splat Scale", GaussianSplat, splat_scale, 0.01, 0.0, 10.0),
        ],
    }
}
