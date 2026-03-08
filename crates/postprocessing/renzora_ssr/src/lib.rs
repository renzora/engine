use bevy::prelude::*;
use bevy::pbr::ScreenSpaceReflections;
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    egui_phosphor::regular,
    renzora_editor::{AppEditorExt, InspectorEntry},
};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SsrSettings {
    pub enabled: bool,
}

impl Default for SsrSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

fn sync_ssr(
    mut commands: Commands,
    query: Query<(Entity, &SsrSettings), Changed<SsrSettings>>,
) {
    for (entity, settings) in &query {
        if settings.enabled {
            commands.entity(entity).insert(ScreenSpaceReflections::default());
        } else {
            commands.entity(entity).remove::<ScreenSpaceReflections>();
        }
    }
}

fn cleanup_ssr(mut commands: Commands, mut removed: RemovedComponents<SsrSettings>) {
    for entity in removed.read() {
        if let Ok(mut ec) = commands.get_entity(entity) {
            ec.remove::<ScreenSpaceReflections>();
        }
    }
}

#[cfg(feature = "editor")]
fn ssr_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "ssr",
        display_name: "SSR",
        icon: regular::SWAP,
        category: "rendering",
        has_fn: |world, entity| world.get::<SsrSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(SsrSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(SsrSettings, ScreenSpaceReflections)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<SsrSettings>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<SsrSettings>(entity) { s.enabled = val; }
        }),
        fields: vec![],
        custom_ui_fn: None,
    }
}

pub struct SsrPlugin;

impl Plugin for SsrPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<SsrSettings>();
        app.add_systems(Update, (sync_ssr, cleanup_ssr));
        #[cfg(feature = "editor")]
        app.register_inspector(ssr_entry());
    }
}
