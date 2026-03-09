use bevy::prelude::*;
use bevy::camera::Camera3d;
use bevy::core_pipeline::oit::OrderIndependentTransparencySettings;
use bevy::render::render_resource::TextureUsages;
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    bevy_egui::egui,
    egui_phosphor::regular,
    renzora_editor::{inline_property, AppEditorExt, EditorCommands, InspectorEntry},
    renzora_theme::Theme,
};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct OitSettings {
    pub layer_count: i32,
    pub alpha_threshold: f32,
    pub enabled: bool,
}

impl Default for OitSettings {
    fn default() -> Self {
        Self {
            layer_count: 8,
            alpha_threshold: 0.0,
            enabled: true,
        }
    }
}

fn sync_oit(
    mut commands: Commands,
    mut query: Query<(Entity, &OitSettings, &mut Camera3d), Changed<OitSettings>>,
    render_target: Res<renzora_core::RenderTarget>,
) {
    for (entity, settings, mut cam3d) in &mut query {
        let target = render_target.0.unwrap_or(entity);
        if settings.enabled {
            let mut usages = TextureUsages::from(cam3d.depth_texture_usages);
            usages |= TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING;
            cam3d.depth_texture_usages = usages.into();

            commands.entity(target)
                .insert(Msaa::Off)
                .insert(OrderIndependentTransparencySettings {
                    layer_count: settings.layer_count,
                    alpha_threshold: settings.alpha_threshold,
                });
        } else {
            commands.entity(target).remove::<OrderIndependentTransparencySettings>();
        }
    }
}

fn cleanup_oit(
    mut commands: Commands,
    mut removed: RemovedComponents<OitSettings>,
    render_target: Res<renzora_core::RenderTarget>,
) {
    for entity in removed.read() {
        if let Some(target) = render_target.0 {
            if let Ok(mut ec) = commands.get_entity(target) {
                ec.remove::<OrderIndependentTransparencySettings>();
            }
        } else if let Ok(mut ec) = commands.get_entity(entity) {
            ec.remove::<OrderIndependentTransparencySettings>();
        }
    }
}

#[cfg(feature = "editor")]
fn oit_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "oit",
        display_name: "OIT Transparency",
        icon: regular::STACK,
        category: "rendering",
        has_fn: |world, entity| world.get::<OitSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(OitSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(OitSettings, OrderIndependentTransparencySettings)>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<OitSettings>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<OitSettings>(entity) { s.enabled = val; }
        }),
        fields: vec![],
        custom_ui_fn: Some(oit_custom_ui),
    }
}

#[cfg(feature = "editor")]
fn oit_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(settings) = world.get::<OitSettings>(entity) else { return };
    let mut row = 0;

    let mut layers = settings.layer_count;
    inline_property(ui, row, "Layers", theme, |ui| {
        let orig = layers;
        ui.add(egui::DragValue::new(&mut layers).speed(1).range(1..=32));
        if layers != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<OitSettings>(entity) { s.layer_count = layers; }
            });
        }
    });
    row += 1;

    let mut threshold = settings.alpha_threshold;
    inline_property(ui, row, "Alpha Threshold", theme, |ui| {
        let orig = threshold;
        ui.add(egui::DragValue::new(&mut threshold).speed(0.01).range(0.0..=1.0));
        if threshold != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<OitSettings>(entity) { s.alpha_threshold = threshold; }
            });
        }
    });
}

pub struct OitPlugin;

impl Plugin for OitPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<OitSettings>();
        app.add_systems(Update, (sync_oit, cleanup_oit));
        #[cfg(feature = "editor")]
        app.register_inspector(oit_entry());
    }
}
