use bevy::prelude::*;
use bevy::core_pipeline::prepass::DepthPrepass;
use bevy::pbr::decal::{
    ForwardDecal, ForwardDecalMaterial, ForwardDecalMaterialExt,
};
use serde::{Deserialize, Serialize};

#[cfg(feature = "editor")]
use {
    bevy_egui::egui,
    egui_phosphor::regular,
    renzora_editor_framework::{inline_property, AppEditorExt, EditorCommands, InspectorEntry},
    renzora_theme::Theme,
};

/// Wrapper settings for a forward decal entity.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct DecalSettings {
    pub base_color: Color,
    pub depth_fade_factor: f32,
    pub enabled: bool,
}

impl Default for DecalSettings {
    fn default() -> Self {
        Self {
            base_color: Color::WHITE,
            depth_fade_factor: 8.0,
            enabled: true,
        }
    }
}

/// Marker to track whether we've already created the decal components.
#[derive(Component)]
struct DecalMaterialHandle(Handle<ForwardDecalMaterial<StandardMaterial>>);

fn sync_decals(
    mut commands: Commands,
    query: Query<(Entity, &DecalSettings), Changed<DecalSettings>>,
    handles: Query<&DecalMaterialHandle>,
    mut materials: ResMut<Assets<ForwardDecalMaterial<StandardMaterial>>>,
) {
    for (entity, settings) in &query {
        if settings.enabled {
            if let Ok(handle) = handles.get(entity) {
                // Update existing material
                if let Some(mat) = materials.get_mut(&handle.0) {
                    mat.base.base_color = settings.base_color;
                    mat.extension.depth_fade_factor = settings.depth_fade_factor;
                }
            } else {
                // Create new material and insert decal components
                let mat_handle = materials.add(ForwardDecalMaterial {
                    base: StandardMaterial {
                        base_color: settings.base_color,
                        alpha_mode: AlphaMode::Blend,
                        ..default()
                    },
                    extension: ForwardDecalMaterialExt {
                        depth_fade_factor: settings.depth_fade_factor,
                    },
                });
                commands.entity(entity).insert((
                    ForwardDecal,
                    MeshMaterial3d(mat_handle.clone()),
                    DecalMaterialHandle(mat_handle),
                ));
            }
        } else {
            // Remove decal components but keep settings
            commands.entity(entity).remove::<(
                ForwardDecal,
                MeshMaterial3d<ForwardDecalMaterial<StandardMaterial>>,
                DecalMaterialHandle,
            )>();
        }
    }
}

fn cleanup_decals(
    mut commands: Commands,
    mut removed: RemovedComponents<DecalSettings>,
) {
    for entity in removed.read() {
        if let Ok(mut ec) = commands.get_entity(entity) {
            ec.remove::<(
                ForwardDecal,
                MeshMaterial3d<ForwardDecalMaterial<StandardMaterial>>,
                DecalMaterialHandle,
            )>();
        }
    }
}

/// Ensures all routed camera targets have DepthPrepass (required for forward decals).
fn ensure_depth_prepass(
    mut commands: Commands,
    cameras: Query<Entity, (With<Camera3d>, Without<DepthPrepass>)>,
    decals: Query<(), With<DecalSettings>>,
    routing: Res<renzora::core::EffectRouting>,
) {
    if decals.is_empty() {
        return;
    }
    if routing.routes.is_empty() {
        for cam in &cameras {
            commands.entity(cam).insert(DepthPrepass);
        }
    } else {
        for (target, _) in routing.iter() {
            commands.entity(*target).insert(DepthPrepass);
        }
    }
}

#[cfg(feature = "editor")]
fn decal_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "forward_decal",
        display_name: "Forward Decal",
        icon: regular::STICKER,
        category: "rendering",
        has_fn: |world, entity| world.get::<DecalSettings>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(DecalSettings::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<(
                DecalSettings,
                ForwardDecal,
                DecalMaterialHandle,
            )>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world.get::<DecalSettings>(entity).map(|s| s.enabled).unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut s) = world.get_mut::<DecalSettings>(entity) { s.enabled = val; }
        }),
        fields: vec![],
        custom_ui_fn: Some(decal_custom_ui),
    }
}

#[cfg(feature = "editor")]
fn decal_custom_ui(
    ui: &mut egui::Ui,
    world: &World,
    entity: Entity,
    cmds: &EditorCommands,
    theme: &Theme,
) {
    let Some(settings) = world.get::<DecalSettings>(entity) else { return };
    let mut row = 0;

    // Base color
    let col = settings.base_color.to_srgba();
    let mut rgba = [col.red, col.green, col.blue, col.alpha];
    inline_property(ui, row, "Color", theme, |ui| {
        let orig = rgba;
        ui.color_edit_button_rgba_unmultiplied(&mut rgba);
        if rgba != orig {
            let c = Color::srgba(rgba[0], rgba[1], rgba[2], rgba[3]);
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<DecalSettings>(entity) { s.base_color = c; }
            });
        }
    });
    row += 1;

    // Depth fade factor
    let mut fade = settings.depth_fade_factor;
    inline_property(ui, row, "Depth Fade", theme, |ui| {
        let orig = fade;
        ui.add(egui::DragValue::new(&mut fade).speed(0.1).range(0.01..=50.0));
        if fade != orig {
            cmds.push(move |world: &mut World| {
                if let Some(mut s) = world.get_mut::<DecalSettings>(entity) { s.depth_fade_factor = fade; }
            });
        }
    });
}

#[derive(Default)]
pub struct DecalPlugin;

impl Plugin for DecalPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] DecalPlugin");
        app.register_type::<DecalSettings>();
        app.add_systems(Update, (sync_decals, cleanup_decals, ensure_depth_prepass));
        #[cfg(feature = "editor")]
        app.register_inspector(decal_entry());
    }
}

renzora::add!(DecalPlugin);
