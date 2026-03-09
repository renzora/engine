use bevy::prelude::*;
use bevy::pbr::ScreenSpaceAmbientOcclusion;
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    egui_phosphor::regular,
    renzora_editor::{AppEditorExt, InspectorEntry},
};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SsaoSettings {
    pub enabled: bool,
}

impl Default for SsaoSettings {
    fn default() -> Self {
        Self { enabled: true }
    }
}

fn sync_ssao(
    mut commands: Commands,
    query: Query<(Entity, &SsaoSettings), Changed<SsaoSettings>>,
    render_target: Res<renzora_core::RenderTarget>,
) {
    for (entity, settings) in &query {
        let target = render_target.0.unwrap_or(entity);
        if settings.enabled {
            commands.entity(target).insert(ScreenSpaceAmbientOcclusion::default());
        } else {
            commands.entity(target).remove::<ScreenSpaceAmbientOcclusion>();
        }
    }
}

fn cleanup_ssao(
    mut commands: Commands,
    mut removed: RemovedComponents<SsaoSettings>,
    render_target: Res<renzora_core::RenderTarget>,
) {
    for entity in removed.read() {
        if let Some(target) = render_target.0 {
            if let Ok(mut ec) = commands.get_entity(target) {
                ec.remove::<ScreenSpaceAmbientOcclusion>();
            }
        } else if let Ok(mut ec) = commands.get_entity(entity) {
            ec.remove::<ScreenSpaceAmbientOcclusion>();
        }
    }
}

#[cfg(feature = "editor")]
fn ssao_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "ssao",
        display_name: "SSAO",
        icon: regular::CIRCLE_HALF,
        category: "rendering",
        has_fn: |world, entity| world.get::<SsaoSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(SsaoSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(SsaoSettings, ScreenSpaceAmbientOcclusion)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<SsaoSettings>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<SsaoSettings>(entity) { s.enabled = val; }
        }),
        fields: vec![],
        custom_ui_fn: None,
    }
}

pub struct SsaoPlugin;

impl Plugin for SsaoPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<SsaoSettings>();
        app.add_systems(Update, (sync_ssao, cleanup_ssao));
        #[cfg(feature = "editor")]
        app.register_inspector(ssao_entry());
    }
}
