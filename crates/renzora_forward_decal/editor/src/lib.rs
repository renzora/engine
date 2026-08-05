//! Editor companion to [`renzora_forward_decal`] — the Forward Decal inspector.
//!
//! Split out of the runtime crate because that crate depended on
//! `renzora_editor_framework` unconditionally: the inspector code was behind a
//! `cfg(feature = "editor")`, but the *dependency* was not optional, so the
//! editor framework (and `renzora_ui`, `renzora_theme` with it) compiled into
//! every shipped game. Making the dep optional would not have been enough —
//! cargo unifies features across a `--workspace` build, so the editor binary
//! enabling the feature would enable it for the runtime binary too. Two crates
//! is the only shape that actually separates them.

use bevy::prelude::*;
use bevy::pbr::decal::ForwardDecal;
use renzora_editor_framework::{AppEditorExt, InspectorEntry};
use renzora_forward_decal::{DecalMaterialHandle, DecalSettings};

fn decal_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "forward_decal",
        display_name: "Forward Decal",
        icon: "sticker",
        category: "rendering",
        has_fn: |world, entity| world.get::<DecalSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(DecalSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world
                .entity_mut(entity)
                .remove::<(DecalSettings, ForwardDecal, DecalMaterialHandle)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<DecalSettings>(entity)
                .map(|s| s.enabled)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<DecalSettings>(entity) {
                s.enabled = val;
            }
        }),
        fields: vec![renzora_editor_framework::float_field!(
            "Depth Fade",
            DecalSettings,
            depth_fade_factor,
            0.1,
            0.01,
            50.0
        )],
    }
}

#[derive(Default)]
pub struct DecalEditorPlugin;

impl Plugin for DecalEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] DecalEditorPlugin");
        app.register_inspector(decal_entry());
    }
}

renzora::add!(DecalEditorPlugin, Editor);
